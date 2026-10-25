//! Painel da biblioteca no modo imersivo: lista da aba visível, a fila
//! "a seguir" e a grade de álbuns, com miniaturas de capa.
//!
//! A lista é virtualizada à mão: a UI recebe só uma janela de linhas ao redor
//! do trecho visível e avisa quando rola (`rolou`). A grade recebe todos os
//! cartões (são poucos), mas só os à vista ganham imagem. Miniaturas existem
//! só para o que está na janela, na fila e na grade à vista; o resto fica no
//! cache em disco. O painel inteiro só existe enquanto está aberto.

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;

use slint::{Image, Model, ModelRc, VecModel};

use crate::miniaturas::{Carregador, Pedido};
use crate::track::GrupoAlbum;
use crate::translations::Translations;
use crate::{CartaoAlbum, LinhaPainel, MainWindow};

/// Linhas carregadas além das visíveis, acima e abaixo.
const MARGEM: usize = 15;
/// Guia da grade de álbuns (0 = biblioteca, 1 = a seguir).
pub const GUIA_ALBUNS: i32 = 2;

/// O que o painel precisa saber de uma faixa da lista.
pub struct Item {
    pub caminho: PathBuf,
    /// Índice da faixa na pasta (para a fila saber o que tocar).
    pub indice: usize,
    pub titulo: String,
    pub artista: Option<String>,
    pub album: Option<String>,
    /// Segundos (0 = desconhecida).
    pub duracao: u32,
    pub tocando: bool,
    pub favorita: bool,
    /// Capa baixada da web (se a faixa e a pasta não têm uma).
    pub capa_web: Option<PathBuf>,
}

/// Miniatura de uma faixa.
struct Info {
    capa: Option<Image>,
    /// Tem capa (no cache); a imagem pode ter sido solta por estar longe.
    tem_capa: bool,
}

pub struct Painel {
    carregador: Carregador,
    infos: HashMap<PathBuf, Info>,
    total: usize,
    /// Índice (na lista) da primeira linha da janela.
    primeira: usize,
    /// Primeira linha visível e quantas cabem, como a UI informou.
    visivel: (usize, usize),
    janela: Vec<Item>,
    linhas: Rc<VecModel<LinhaPainel>>,
    fila: Vec<Item>,
    linhas_fila: Rc<VecModel<LinhaPainel>>,
    /// Grade de álbuns à vista (guia aberta).
    mostrando_albuns: bool,
    albuns: Vec<GrupoAlbum>,
    /// De onde vem a capa de cada álbum (a primeira faixa).
    capas_albuns: Vec<Option<Pedido>>,
    /// Cartões à vista, como a UI informou.
    albuns_visiveis: Range<usize>,
    cartoes: Rc<VecModel<CartaoAlbum>>,
}

impl Painel {
    pub fn novo(ui: &MainWindow) -> Self {
        let linhas = Rc::new(VecModel::default());
        let linhas_fila = Rc::new(VecModel::default());
        let cartoes = Rc::new(VecModel::default());
        ui.set_imv_painel_linhas(ModelRc::from(linhas.clone()));
        ui.set_imv_fila(ModelRc::from(linhas_fila.clone()));
        ui.set_imv_albuns(ModelRc::from(cartoes.clone()));
        Self {
            carregador: Carregador::novo(),
            infos: HashMap::new(),
            total: 0,
            primeira: 0,
            visivel: (0, 20),
            janela: Vec::new(),
            linhas,
            fila: Vec::new(),
            linhas_fila,
            mostrando_albuns: false,
            albuns: Vec::new(),
            capas_albuns: Vec::new(),
            albuns_visiveis: 0..12,
            cartoes,
        }
    }

    /// A UI rolou. Devolve `true` quando o trecho visível saiu da janela
    /// carregada e é preciso publicar de novo.
    pub fn rolou(&mut self, primeira_visivel: usize, quantas: usize) -> bool {
        self.visivel = (primeira_visivel, quantas);
        let carregadas = self.primeira..self.primeira + self.janela.len();
        let pedidas = intervalo(self.visivel, self.total, 0);
        pedidas.start < carregadas.start || pedidas.end > carregadas.end
    }

    /// Índice na pasta da faixa `posicao` da fila.
    pub fn indice_da_fila(&self, posicao: usize) -> Option<usize> {
        self.fila.get(posicao).map(|i| i.indice)
    }

    /// Refaz a janela de linhas. `item(pos)` dá a faixa na posição `pos` da
    /// lista visível.
    pub fn publicar(
        &mut self,
        ui: &MainWindow,
        total: usize,
        item: impl Fn(usize) -> Option<Item>,
    ) {
        self.total = total;
        let faixa = intervalo(self.visivel, total, MARGEM);
        self.primeira = faixa.start;
        self.janela = faixa.filter_map(item).collect();
        let linhas: Vec<LinhaPainel> = self.janela.iter().map(|i| self.linha(i)).collect();
        self.linhas.set_vec(linhas);
        ui.set_imv_painel_total(total as i32);
        ui.set_imv_painel_primeira(self.primeira as i32);
        self.pedir_faltantes();
    }

    pub fn publicar_fila(&mut self, itens: Vec<Item>) {
        self.fila = itens;
        let linhas: Vec<LinhaPainel> = self.fila.iter().map(|i| self.linha(i)).collect();
        self.linhas_fila.set_vec(linhas);
        self.pedir_faltantes();
    }

    pub fn mostrando_albuns(&self) -> bool {
        self.mostrando_albuns
    }

    /// Abre ou fecha a grade. Fechada, solta os cartões.
    pub fn mostrar_albuns(&mut self, mostrar: bool) {
        self.mostrando_albuns = mostrar;
        if !mostrar {
            self.albuns.clear();
            self.capas_albuns.clear();
            self.cartoes.set_vec(Vec::new());
            self.pedir_faltantes();
        }
    }

    /// Troca os álbuns da grade. `capa_de(i)` dá o arquivo da faixa `i`.
    pub fn publicar_albuns(
        &mut self,
        grupos: Vec<GrupoAlbum>,
        lang: &Translations,
        capa_de: impl Fn(usize) -> Option<Pedido>,
    ) {
        if grupos == self.albuns {
            return;
        }
        self.capas_albuns = grupos
            .iter()
            .map(|g| g.faixas.first().and_then(|&i| capa_de(i)))
            .collect();
        let cartoes: Vec<CartaoAlbum> = grupos
            .iter()
            .enumerate()
            .map(|(i, g)| self.cartao(i, g, lang))
            .collect();
        self.albuns = grupos;
        self.cartoes.set_vec(cartoes);
        self.pedir_faltantes();
    }

    /// A grade rolou: `quantos` cartões à vista a partir de `primeiro`.
    pub fn albuns_rolou(&mut self, primeiro: usize, quantos: usize) {
        let novo = primeiro..primeiro + quantos;
        if novo == self.albuns_visiveis {
            return;
        }
        self.albuns_visiveis = novo;
        self.pedir_faltantes();
        self.atualizar_cartoes();
    }

    /// Esquece a miniatura da faixa (chegou uma capa nova da web).
    pub fn esquecer(&mut self, caminho: &std::path::Path) {
        self.infos.remove(caminho);
        self.pedir_faltantes();
    }

    pub fn album(&self, posicao: usize) -> Option<&GrupoAlbum> {
        self.albuns.get(posicao)
    }

    /// Chamado pelo timer do modo: aplica as miniaturas que chegaram.
    pub fn tick(&mut self) {
        let chegadas: Vec<_> = self.carregador.recebidos().collect();
        if chegadas.is_empty() {
            return;
        }
        for (caminho, capa) in chegadas {
            self.infos.insert(
                caminho,
                Info {
                    tem_capa: capa.is_some(),
                    capa: capa.map(Image::from_rgba8),
                },
            );
        }
        atualizar_linhas(&self.linhas, &self.janela, &self.infos);
        atualizar_linhas(&self.linhas_fila, &self.fila, &self.infos);
        self.atualizar_cartoes();
    }

    fn linha(&self, item: &Item) -> LinhaPainel {
        linha(item, self.infos.get(&item.caminho))
    }

    fn cartao(&self, i: usize, g: &GrupoAlbum, lang: &Translations) -> CartaoAlbum {
        let t = |k: &str| lang.get(k).copied().unwrap_or("").to_string();
        let artista = if g.varios {
            Some(t("various_artists"))
        } else {
            g.artista.clone()
        };
        let n = g.faixas.len();
        let faixas = format!(
            "{n} {}",
            t(if n == 1 {
                "album_track"
            } else {
                "album_tracks"
            })
        );
        let detalhe = match artista {
            Some(a) => format!("{a} • {faixas}"),
            None => faixas,
        };
        let nome = if g.nome.is_empty() {
            t("no_album")
        } else {
            g.nome.clone()
        };
        let capa = self.capa_do_album(i);
        CartaoAlbum {
            nome: nome.into(),
            detalhe: detalhe.into(),
            tem_capa: capa.is_some(),
            capa: capa.unwrap_or_default(),
        }
    }

    /// Imagem do cartão `i`, só se ele está à vista.
    fn capa_do_album(&self, i: usize) -> Option<Image> {
        if !self.albuns_visiveis.contains(&i) {
            return None;
        }
        let (caminho, _) = self.capas_albuns.get(i)?.as_ref()?;
        self.infos.get(caminho)?.capa.clone()
    }

    fn atualizar_cartoes(&self) {
        for i in 0..self.cartoes.row_count() {
            let Some(mut c) = self.cartoes.row_data(i) else {
                continue;
            };
            let capa = self.capa_do_album(i);
            if c.tem_capa != capa.is_some() || capa.as_ref().is_some_and(|img| *img != c.capa) {
                c.tem_capa = capa.is_some();
                c.capa = capa.unwrap_or_default();
                self.cartoes.set_row_data(i, c);
            }
        }
    }

    /// Pede ao carregador o que falta (visíveis primeiro, depois a fila, a
    /// grade e a margem) e solta as miniaturas de quem saiu de vista.
    fn pedir_faltantes(&mut self) {
        let visiveis = intervalo(self.visivel, self.total, 0);
        let (perto, longe): (Vec<_>, Vec<_>) = (self.primeira..)
            .zip(&self.janela)
            .partition(|(pos, _)| visiveis.contains(pos));
        let albuns = self
            .capas_albuns
            .get(self.albuns_visiveis.start.min(self.capas_albuns.len())..)
            .unwrap_or_default()
            .iter()
            .take(self.albuns_visiveis.len())
            .flatten();
        let pedido = |i: &Item| (i.caminho.clone(), i.capa_web.clone());
        let ordem = perto
            .into_iter()
            .map(|(_, i)| pedido(i))
            .chain(self.fila.iter().map(pedido))
            .chain(albuns.cloned())
            .chain(longe.into_iter().map(|(_, i)| pedido(i)));

        let mut vistos = HashSet::new();
        let mut pedidos = Vec::new();
        for (caminho, web) in ordem {
            if vistos.contains(&caminho) {
                continue;
            }
            let falta = match self.infos.get(&caminho) {
                None => true,
                Some(info) => info.tem_capa && info.capa.is_none(),
            };
            if falta {
                pedidos.push((caminho.clone(), web));
            }
            vistos.insert(caminho);
        }
        self.carregador.pedir(pedidos);

        for (caminho, info) in &mut self.infos {
            if !vistos.contains(caminho) {
                info.capa = None;
            }
        }
    }
}

/// Linhas `[primeira - margem, primeira + quantas + margem)`, cortadas no total.
fn intervalo((primeira, quantas): (usize, usize), total: usize, margem: usize) -> Range<usize> {
    let inicio = primeira.saturating_sub(margem).min(total);
    let fim = (primeira + quantas + margem).min(total);
    inicio..fim.max(inicio)
}

fn linha(item: &Item, info: Option<&Info>) -> LinhaPainel {
    let detalhe = [item.artista.as_deref(), item.album.as_deref()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" • ");
    let duracao = match item.duracao as u64 {
        0 => String::new(),
        s => crate::formatar_tempo(s, s >= 3600),
    };
    let capa = info.and_then(|i| i.capa.clone());
    LinhaPainel {
        titulo: item.titulo.as_str().into(),
        detalhe: detalhe.into(),
        duracao: duracao.into(),
        tem_capa: capa.is_some(),
        capa: capa.unwrap_or_default(),
        tocando: item.tocando,
        favorita: item.favorita,
        video: crate::ffmpeg::e_video(&item.caminho),
    }
}

fn atualizar_linhas(
    modelo: &VecModel<LinhaPainel>,
    itens: &[Item],
    infos: &HashMap<PathBuf, Info>,
) {
    for (i, item) in itens.iter().enumerate() {
        let nova = linha(item, infos.get(&item.caminho));
        if modelo.row_data(i).is_some_and(|atual| atual != nova) {
            modelo.set_row_data(i, nova);
        }
    }
}

/// Solta o que o painel pôs na UI.
pub fn limpar_ui(ui: &MainWindow) {
    ui.set_imv_painel_linhas(ModelRc::default());
    ui.set_imv_fila(ModelRc::default());
    ui.set_imv_albuns(ModelRc::default());
    ui.set_imv_painel_total(0);
    ui.set_imv_painel_primeira(0);
    ui.set_imv_guia(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intervalo_respeita_margem_e_limites() {
        assert_eq!(intervalo((0, 10), 100, 5), 0..15);
        assert_eq!(intervalo((50, 10), 100, 5), 45..65);
        assert_eq!(intervalo((95, 10), 100, 5), 90..100);
        assert_eq!(intervalo((0, 10), 3, 5), 0..3);
        assert_eq!(intervalo((10, 10), 0, 5), 0..0);
    }

    #[test]
    fn linha_junta_artista_e_album() {
        let mut item = Item {
            caminho: "a.mp3".into(),
            indice: 0,
            titulo: "Faixa".into(),
            artista: Some("Banda".into()),
            album: Some("Disco".into()),
            duracao: 201,
            tocando: true,
            favorita: true,
            capa_web: None,
        };
        let l = linha(&item, None);
        assert_eq!(l.detalhe, "Banda • Disco");
        assert_eq!(l.duracao, "03:21");
        assert!(l.tocando && l.favorita && !l.tem_capa);
        // Sem álbum nem duração: só o artista
        item.album = None;
        item.duracao = 0;
        let l = linha(&item, None);
        assert_eq!((l.detalhe.as_str(), l.duracao.as_str()), ("Banda", ""));
    }
}
