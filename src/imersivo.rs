//! Modo imersivo: estado que só existe enquanto a tela cheia está ligada.
//!
//! `EstadoImersivo` vive num `Option` dentro do estado do app; `None` quer dizer
//! modo desligado e nada alocado. Ao sair, o `Drop` derruba threads e imagens
//! e as propriedades da UI voltam ao vazio.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Instant;

use slint::{Image, Model, ModelRc, SharedString, VecModel};

use crate::MainWindow;
use crate::capa::{self, Carregado};
use crate::fundo::{self, Reprodutor};
use crate::painel::{self, Painel};
use crate::translations;
use crate::visualizador::{self, Analisador, Captura};

/// O que o modo precisa saber da faixa atual.
pub struct Faixa {
    pub caminho: PathBuf,
    /// Pasta da aba de onde a faixa vem (onde também se procura fundo).
    pub pasta_raiz: PathBuf,
    pub titulo: String,
    pub artista: Option<String>,
    /// Fundos escolhidos nas configurações (rodízio a cada faixa).
    pub fundos_globais: Vec<PathBuf>,
    /// Capa baixada da web (só usada sem capa na faixa e na pasta).
    pub capa_web: Option<PathBuf>,
    /// Álbum e ano corrigidos pela web (por cima das tags).
    pub album: Option<String>,
    pub ano: Option<u16>,
}

pub struct EstadoImersivo {
    /// Incrementa a cada troca de faixa; resultado de carga antigo é descartado.
    geracao: u64,
    tx: Sender<(u64, Carregado)>,
    rx: Receiver<(u64, Carregado)>,
    /// Faixa exibida (para não recarregar a mesma num seek).
    faixa: Option<PathBuf>,
    artista: Option<String>,
    /// Álbum e ano corrigidos da faixa exibida.
    album: Option<String>,
    ano: Option<u16>,
    /// Qual camada de fundo está visível (crossfade A/B).
    mostra_a: bool,
    /// Fundo personalizado tocando. `None`: usa a capa desfocada.
    fundo: Option<Reprodutor>,
    /// O próximo quadro do fundo é o primeiro: entra com crossfade.
    fundo_novo: bool,
    analisador: Analisador,
    /// Modelo das barras, reaproveitado (só os valores mudam a cada tick).
    barras: Rc<VecModel<f32>>,
    /// Painel da biblioteca; só existe aberto.
    pub painel: Option<Painel>,
    /// Pulso do fundo (0..1), seguindo os graves.
    pulso: f32,
}

/// Barras do visualizador que contam como graves (de 40 Hz até ~150 Hz).
const BARRAS_GRAVES: usize = 6;
/// Quanto o pulso cai por tick quando o grave some (sobe na hora).
const QUEDA_PULSO: f32 = 0.88;

/// Próximo valor do pulso: segue o grave na subida e cai devagar. Arredonda
/// para centésimos, para a UI só repintar quando muda de verdade.
fn pulso(anterior: f32, barras: &[f32]) -> f32 {
    let graves = barras.iter().take(BARRAS_GRAVES).sum::<f32>() / BARRAS_GRAVES as f32;
    // Ao quadrado: o pulso só aparece nas batidas, não no grave contínuo
    let alvo = (graves * graves).min(1.0);
    let v = alvo.max(anterior * QUEDA_PULSO);
    (v * 100.0).round() / 100.0
}

impl EstadoImersivo {
    pub fn novo(ui: &MainWindow) -> Self {
        let (tx, rx) = mpsc::channel();
        let barras = Rc::new(VecModel::from(vec![0.0f32; visualizador::BARRAS]));
        ui.set_imv_barras(ModelRc::from(barras.clone()));
        Self {
            geracao: 0,
            tx,
            rx,
            faixa: None,
            artista: None,
            album: None,
            ano: None,
            mostra_a: true,
            fundo: None,
            fundo_novo: false,
            analisador: Analisador::default(),
            barras,
            painel: None,
            pulso: 0.0,
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

        // Mesmo fundo da faixa anterior (mesma pasta): continua tocando sem
        // reiniciar. Vídeo sem ffmpeg no PATH não abre e cai para a capa.
        let pasta_faixa = faixa.caminho.parent().unwrap_or(&faixa.pasta_raiz);
        let candidatos = fundo::resolver(pasta_faixa, &faixa.pasta_raiz, &faixa.fundos_globais);
        let atual = self.fundo.as_ref().map(|r| r.caminho.as_path());
        let alvo = fundo::escolher(&candidatos, atual, rand::random::<u32>() as usize);
        if self.fundo.as_ref().map(|r| &r.caminho) != alvo.as_ref() {
            self.fundo = alvo.as_deref().and_then(Reprodutor::abrir);
            self.fundo_novo = self.fundo.is_some();
        }
        self.mostrar_dados(ui, faixa);
    }

    /// Os dados da mesma faixa mudaram (correção da web): título, créditos e
    /// capa de novo, sem mexer no fundo.
    pub fn atualizar_dados(&mut self, ui: &MainWindow, faixa: Option<Faixa>) {
        match faixa {
            Some(f) if self.faixa.as_ref() == Some(&f.caminho) => self.mostrar_dados(ui, f),
            outra => self.trocar_faixa(ui, outra),
        }
    }

    fn mostrar_dados(&mut self, ui: &MainWindow, faixa: Faixa) {
        // Título na hora; o resto quando a thread terminar
        ui.set_imv_titulo(faixa.titulo.as_str().into());
        ui.set_imv_creditos(faixa.artista.as_deref().unwrap_or("").into());
        ui.set_imv_tecnico(SharedString::new());

        self.geracao += 1;
        self.faixa = Some(faixa.caminho.clone());
        self.artista = faixa.artista;
        self.album = faixa.album;
        self.ano = faixa.ano;

        let geracao = self.geracao;
        let tx = self.tx.clone();
        let caminho = faixa.caminho;
        let web = faixa.capa_web;
        std::thread::spawn(move || {
            let _ = tx.send((geracao, capa::carregar(&caminho, web.as_deref())));
        });
    }

    /// Chamado pelo timer do modo (~30 fps).
    pub fn tick(
        &mut self,
        ui: &MainWindow,
        idioma: &str,
        captura: &Captura,
        tocando: bool,
        reativo: bool,
    ) {
        if let Some(barras) = self.analisador.quadro(captura, tocando) {
            for (i, b) in barras.into_iter().enumerate() {
                if self.barras.row_data(i) != Some(b) {
                    self.barras.set_row_data(i, b);
                }
            }
            let novo = if reativo {
                pulso(self.pulso, &barras)
            } else {
                0.0
            };
            if novo != self.pulso {
                self.pulso = novo;
                ui.set_imv_pulso(novo);
            }
        }

        while let Ok((geracao, carregado)) = self.rx.try_recv() {
            if geracao == self.geracao {
                self.aplicar(ui, carregado, idioma);
            }
        }

        if let Some(p) = self.painel.as_mut() {
            p.tick();
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

    fn aplicar(&mut self, ui: &MainWindow, mut c: Carregado, idioma: &str) {
        let lang = translations::get_translations(idioma);
        if self.album.is_some() {
            c.detalhes.album = self.album.clone();
        }
        if self.ano.is_some() {
            c.detalhes.ano = self.ano;
        }
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
    ui.set_imv_painel_aberto(false);
    ui.set_imv_so_letra(false);
    ui.set_imv_config_aberta(false);
    ui.set_imv_relogio(SharedString::new());
    ui.set_imv_proxima(SharedString::new());
    ui.set_imv_pulso(0.0);
    painel::limpar_ui(ui);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pulso_sobe_na_batida_e_cai_devagar() {
        let batida = [1.0f32; 48];
        let silencio = [0.0f32; 48];
        let p = pulso(0.0, &batida);
        assert_eq!(p, 1.0);
        let depois = pulso(p, &silencio);
        assert!(depois < 1.0 && depois > 0.8, "{depois}");
        // Grave fraco quase não mexe
        let mut fraco = [0.0f32; 48];
        fraco[..6].fill(0.3);
        assert!(pulso(0.0, &fraco) < 0.1);
        assert_eq!(pulso(0.0, &silencio), 0.0);
    }
}
