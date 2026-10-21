//! Painel da biblioteca no modo imersivo: lista da aba visível e a fila
//! "a seguir", com miniaturas de capa.
//!
//! A lista é virtualizada à mão: a UI recebe só uma janela de linhas ao redor
//! do trecho visível e avisa quando rola (`rolou`). Miniaturas existem só para
//! as linhas da janela e da fila; o resto fica no cache em disco. O painel
//! inteiro só existe enquanto está aberto.

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;

use slint::{Image, Model, ModelRc, VecModel};

use crate::miniaturas::Carregador;
use crate::{LinhaPainel, MainWindow};

/// Linhas carregadas além das visíveis, acima e abaixo.
const MARGEM: usize = 15;

/// O que o painel precisa saber de uma faixa da lista.
pub struct Item {
    pub caminho: PathBuf,
    /// Índice da faixa na pasta (para a fila saber o que tocar).
    pub indice: usize,
    pub titulo: String,
    pub artista: Option<String>,
    pub tocando: bool,
}

struct Info {
    album: Option<String>,
    duracao: Option<u64>,
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
}

impl Painel {
    pub fn novo(ui: &MainWindow) -> Self {
        let linhas = Rc::new(VecModel::default());
        let linhas_fila = Rc::new(VecModel::default());
        ui.set_imv_painel_linhas(ModelRc::from(linhas.clone()));
        ui.set_imv_fila(ModelRc::from(linhas_fila.clone()));
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

    /// Chamado pelo timer do modo: aplica as miniaturas que chegaram.
    pub fn tick(&mut self) {
        let chegadas: Vec<_> = self.carregador.recebidos().collect();
        if chegadas.is_empty() {
            return;
        }
        for (caminho, meta) in chegadas {
            let tem_capa = meta.capa.is_some();
            self.infos.insert(
                caminho,
                Info {
                    album: meta.album,
                    duracao: meta.duracao,
                    capa: meta.capa.map(Image::from_rgba8),
                    tem_capa,
                },
            );
        }
        atualizar_linhas(&self.linhas, &self.janela, &self.infos);
        atualizar_linhas(&self.linhas_fila, &self.fila, &self.infos);
    }

    fn linha(&self, item: &Item) -> LinhaPainel {
        linha(item, self.infos.get(&item.caminho))
    }

    /// Pede ao carregador o que falta na janela e na fila (visíveis primeiro)
    /// e solta as miniaturas de quem saiu delas.
    fn pedir_faltantes(&mut self) {
        let visiveis = intervalo(self.visivel, self.total, 0);
        let (perto, longe): (Vec<_>, Vec<_>) = (self.primeira..)
            .zip(&self.janela)
            .partition(|(pos, _)| visiveis.contains(pos));
        let ordem = perto
            .into_iter()
            .map(|(_, i)| i)
            .chain(&self.fila)
            .chain(longe.into_iter().map(|(_, i)| i));

        let mut vistos = HashSet::new();
        let mut pedidos = Vec::new();
        for item in ordem {
            if !vistos.insert(&item.caminho) {
                continue;
            }
            let falta = match self.infos.get(&item.caminho) {
                None => true,
                Some(info) => info.tem_capa && info.capa.is_none(),
            };
            if falta {
                pedidos.push(item.caminho.clone());
            }
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
    let album = info.and_then(|i| i.album.as_deref());
    let detalhe = [item.artista.as_deref(), album]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" • ");
    let duracao = info
        .and_then(|i| i.duracao)
        .map(|s| crate::formatar_tempo(s, s >= 3600))
        .unwrap_or_default();
    let capa = info.and_then(|i| i.capa.clone());
    LinhaPainel {
        titulo: item.titulo.as_str().into(),
        detalhe: detalhe.into(),
        duracao: duracao.into(),
        tem_capa: capa.is_some(),
        capa: capa.unwrap_or_default(),
        tocando: item.tocando,
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
    ui.set_imv_painel_total(0);
    ui.set_imv_painel_primeira(0);
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
        let item = Item {
            caminho: "a.mp3".into(),
            indice: 0,
            titulo: "Faixa".into(),
            artista: Some("Banda".into()),
            tocando: true,
        };
        let info = Info {
            album: Some("Disco".into()),
            duracao: Some(201),
            capa: None,
            tem_capa: false,
        };
        let l = linha(&item, Some(&info));
        assert_eq!(l.detalhe, "Banda • Disco");
        assert_eq!(l.duracao, "03:21");
        assert!(l.tocando && !l.tem_capa);
        // Antes dos metadados chegarem: só o artista, sem duração
        let l = linha(&item, None);
        assert_eq!((l.detalhe.as_str(), l.duracao.as_str()), ("Banda", ""));
    }
}
