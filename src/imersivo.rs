//! Modo imersivo: estado que só existe enquanto a tela cheia está ligada.
//!
//! `EstadoImersivo` vive num `Option` dentro do estado do app; `None` quer dizer
//! modo desligado e nada alocado. Ao sair, o `Drop` derruba threads e imagens
//! e as propriedades da UI voltam ao vazio.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Instant;

use slint::{Image, SharedString};

use crate::MainWindow;
use crate::capa::{self, Carregado};
use crate::fundo::{self, Reprodutor};
use crate::translations;

/// O que o modo precisa saber da faixa atual.
pub struct Faixa {
    pub caminho: PathBuf,
    /// Pasta da aba de onde a faixa vem (onde também se procura fundo).
    pub pasta_raiz: PathBuf,
    pub titulo: String,
    pub artista: Option<String>,
    /// Fundo escolhido nas configurações.
    pub fundo_global: Option<PathBuf>,
}

pub struct EstadoImersivo {
    /// Incrementa a cada troca de faixa; resultado de carga antigo é descartado.
    geracao: u64,
    tx: Sender<(u64, Carregado)>,
    rx: Receiver<(u64, Carregado)>,
    /// Faixa exibida (para não recarregar a mesma num seek).
    faixa: Option<PathBuf>,
    artista: Option<String>,
    /// Qual camada de fundo está visível (crossfade A/B).
    mostra_a: bool,
    /// Fundo personalizado tocando. `None`: usa a capa desfocada.
    fundo: Option<Reprodutor>,
    /// O próximo quadro do fundo é o primeiro: entra com crossfade.
    fundo_novo: bool,
}

impl EstadoImersivo {
    pub fn novo() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            geracao: 0,
            tx,
            rx,
            faixa: None,
            artista: None,
            mostra_a: true,
            fundo: None,
            fundo_novo: false,
        }
    }

    /// Mostra a faixa atual. A leitura de tags e a decodificação da capa vão
    /// para uma thread; o resultado chega no `tick`.
    pub fn trocar_faixa(&mut self, ui: &MainWindow, faixa: Option<Faixa>) {
        let Some(faixa) = faixa else {
            self.faixa = None;
            limpar_faixa(ui);
            return;
        };
        if self.faixa.as_ref() == Some(&faixa.caminho) {
            return;
        }

        // Título na hora; o resto quando a thread terminar
        ui.set_imv_titulo(faixa.titulo.as_str().into());
        ui.set_imv_creditos(faixa.artista.as_deref().unwrap_or("").into());
        ui.set_imv_tecnico(SharedString::new());

        self.geracao += 1;
        self.faixa = Some(faixa.caminho.clone());
        self.artista = faixa.artista;

        // Mesmo fundo da faixa anterior (mesma pasta): continua tocando sem
        // reiniciar. Vídeo sem ffmpeg no PATH não abre e cai para a capa.
        let pasta_faixa = faixa.caminho.parent().unwrap_or(&faixa.pasta_raiz);
        let alvo = fundo::resolver(
            pasta_faixa,
            &faixa.pasta_raiz,
            faixa.fundo_global.as_deref(),
        );
        if self.fundo.as_ref().map(|r| &r.caminho) != alvo.as_ref() {
            self.fundo = alvo.as_deref().and_then(Reprodutor::abrir);
            self.fundo_novo = self.fundo.is_some();
        }

        let geracao = self.geracao;
        let tx = self.tx.clone();
        let caminho = faixa.caminho;
        std::thread::spawn(move || {
            let _ = tx.send((geracao, capa::carregar(&caminho)));
        });
    }

    /// Chamado pelo timer do modo (~30 fps).
    pub fn tick(&mut self, ui: &MainWindow, idioma: &str) {
        while let Ok((geracao, carregado)) = self.rx.try_recv() {
            if geracao == self.geracao {
                self.aplicar(ui, carregado, idioma);
            }
        }

        let quadro = self.fundo.as_mut().and_then(|r| r.proximo(Instant::now()));
        if let Some(q) = quadro {
            let pixelado = q.pixels.width() <= fundo::LARGURA_PIXEL_ART;
            let imagem = Image::from_rgba8(q.pixels);
            if self.fundo_novo {
                self.fundo_novo = false;
                self.trocar_fundo(ui, imagem, pixelado);
            } else if self.mostra_a {
                // Quadros seguintes vão direto na camada visível
                ui.set_imv_fundo_a(imagem);
            } else {
                ui.set_imv_fundo_b(imagem);
            }
        }
    }

    fn aplicar(&mut self, ui: &MainWindow, c: Carregado, idioma: &str) {
        let lang = translations::get_translations(idioma);
        ui.set_imv_creditos(capa::linha_creditos(self.artista.as_deref(), &c.detalhes).into());
        ui.set_imv_tecnico(capa::linha_tecnica(&c.detalhes, &lang).into());

        match c.capa {
            Some(buf) => {
                ui.set_imv_capa(Image::from_rgba8(buf));
                ui.set_imv_tem_capa(true);
            }
            None => ui.set_imv_tem_capa(false),
        }

        match c.cor {
            Some((r, g, b)) => {
                ui.set_imv_cor(slint::Color::from_rgb_u8(r, g, b));
                ui.set_imv_tem_cor(true);
            }
            None => ui.set_imv_tem_cor(false),
        }

        // Sem fundo personalizado, a capa desfocada faz o papel
        if self.fundo.is_none() {
            match c.desfocada {
                Some(buf) => self.trocar_fundo(ui, Image::from_rgba8(buf), false),
                None => ui.set_imv_tem_fundo(false),
            }
        }
    }

    /// Crossfade: escreve na camada oculta e inverte qual aparece.
    fn trocar_fundo(&mut self, ui: &MainWindow, imagem: Image, pixelado: bool) {
        if self.mostra_a {
            ui.set_imv_fundo_b(imagem);
        } else {
            ui.set_imv_fundo_a(imagem);
        }
        self.mostra_a = !self.mostra_a;
        ui.set_imv_mostra_a(self.mostra_a);
        ui.set_imv_fundo_pixelado(pixelado);
        ui.set_imv_tem_fundo(true);
    }
}

fn limpar_faixa(ui: &MainWindow) {
    ui.set_imv_titulo(SharedString::new());
    ui.set_imv_creditos(SharedString::new());
    ui.set_imv_tecnico(SharedString::new());
    ui.set_imv_capa(Image::default());
    ui.set_imv_tem_capa(false);
    ui.set_imv_tem_cor(false);
}

/// Solta tudo que o modo pôs na UI, para as imagens serem liberadas.
pub fn limpar_ui(ui: &MainWindow) {
    limpar_faixa(ui);
    ui.set_imv_fundo_a(Image::default());
    ui.set_imv_fundo_b(Image::default());
    ui.set_imv_mostra_a(true);
    ui.set_imv_tem_fundo(false);
    ui.set_imv_fundo_pixelado(false);
    ui.set_imv_barras(slint::ModelRc::default());
}
