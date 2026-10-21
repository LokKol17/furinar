//! Favoritas e contagem de reproduções, guardadas em
//! `furinar_biblioteca.json` ao lado da configuração.
//!
//! As faixas são identificadas pelo caminho absoluto (`chave_faixa`). Arquivo
//! à parte da configuração: a contagem cresce com a biblioteca, e a config é
//! regravada a cada clique em loop, shuffle, tema...

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Quantas faixas a lista "Mais tocadas" mostra.
pub const TAMANHO_MAIS_TOCADAS: usize = 100;

#[derive(Default, Serialize, Deserialize)]
pub struct Biblioteca {
    /// Na ordem em que foram marcadas (a ordem "padrão" da aba).
    #[serde(default)]
    favoritas: Vec<String>,
    #[serde(default)]
    reproducoes: HashMap<String, u32>,
    /// Espelho de `favoritas` para consultar linha a linha sem varrer o Vec.
    #[serde(skip)]
    conjunto: HashSet<String>,
    #[serde(skip)]
    caminho: Option<PathBuf>,
}

/// Identidade da faixa: o caminho absoluto, com `/` (o mesmo arquivo dá a
/// mesma chave venha da pasta ou de uma lista especial).
pub fn chave_faixa(raiz: &Path, relativo: &str) -> String {
    raiz.join(relativo).to_string_lossy().replace('\\', "/")
}

impl Biblioteca {
    pub fn carregar() -> Self {
        let caminho = crate::config::config_path().with_file_name("furinar_biblioteca.json");
        let mut b = Self::desde_json(&fs::read_to_string(&caminho).unwrap_or_default());
        b.caminho = Some(caminho);
        b
    }

    fn desde_json(conteudo: &str) -> Self {
        let mut b: Self = serde_json::from_str(conteudo).unwrap_or_default();
        // Repetida num JSON editado à mão contaria duas vezes na lista
        let mut vistas = HashSet::new();
        b.favoritas.retain(|f| vistas.insert(f.clone()));
        b.conjunto = vistas;
        b
    }

    pub fn salvar(&self) {
        let Some(caminho) = &self.caminho else {
            return;
        };
        if let Ok(json) = serde_json::to_string(self) {
            let temporario = caminho.with_extension("json.tmp");
            if fs::write(&temporario, json).is_ok() {
                let _ = fs::rename(&temporario, caminho);
            }
        }
    }

    pub fn e_favorita(&self, chave: &str) -> bool {
        self.conjunto.contains(chave)
    }

    /// Marca ou desmarca. Devolve o estado novo.
    pub fn alternar_favorita(&mut self, chave: &str) -> bool {
        if self.conjunto.remove(chave) {
            self.favoritas.retain(|f| f != chave);
            false
        } else {
            self.conjunto.insert(chave.to_string());
            self.favoritas.push(chave.to_string());
            true
        }
    }

    pub fn favoritas(&self) -> &[String] {
        &self.favoritas
    }

    pub fn reproducoes(&self, chave: &str) -> u32 {
        self.reproducoes.get(chave).copied().unwrap_or(0)
    }

    pub fn contar(&mut self, chave: &str) {
        *self.reproducoes.entry(chave.to_string()).or_insert(0) += 1;
    }

    /// As `n` mais tocadas, da mais para a menos (empate: pelo caminho, para
    /// a lista não mudar de ordem sozinha).
    pub fn mais_tocadas(&self, n: usize) -> Vec<&str> {
        let mut todas: Vec<(&str, u32)> = self
            .reproducoes
            .iter()
            .filter(|(_, v)| **v > 0)
            .map(|(k, v)| (k.as_str(), *v))
            .collect();
        todas.sort_unstable_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        todas.into_iter().take(n).map(|(k, _)| k).collect()
    }
}

/// Uma faixa conta como ouvida depois de metade da duração ou de 4 minutos, o
/// que vier antes (a regra do Last.fm). Faixa sem duração conhecida conta aos
/// 30 s.
pub fn conta_como_ouvida(tocado: f64, duracao: u64) -> bool {
    let limite = if duracao == 0 {
        30.0
    } else {
        (duracao as f64 / 2.0).min(240.0)
    };
    tocado >= limite
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn favoritas_alternam_e_mantem_a_ordem() {
        let mut b = Biblioteca::default();
        assert!(b.alternar_favorita("/m/a.mp3"));
        assert!(b.alternar_favorita("/m/b.mp3"));
        assert!(b.e_favorita("/m/a.mp3"));
        assert!(!b.alternar_favorita("/m/a.mp3"));
        assert!(!b.e_favorita("/m/a.mp3"));
        assert!(b.alternar_favorita("/m/a.mp3"));
        assert_eq!(b.favoritas(), ["/m/b.mp3", "/m/a.mp3"]);
    }

    #[test]
    fn mais_tocadas_pela_contagem() {
        let mut b = Biblioteca::default();
        for _ in 0..3 {
            b.contar("/m/b.mp3");
        }
        b.contar("/m/a.mp3");
        b.contar("/m/c.mp3");
        assert_eq!(b.mais_tocadas(2), ["/m/b.mp3", "/m/a.mp3"]);
        assert_eq!(b.reproducoes("/m/b.mp3"), 3);
        assert_eq!(b.reproducoes("/m/x.mp3"), 0);
    }

    #[test]
    fn json_volta_igual_e_sem_repetidas() {
        let b = Biblioteca::desde_json(r#"{"favoritas":["/a","/b","/a"],"reproducoes":{"/a":2}}"#);
        assert_eq!(b.favoritas(), ["/a", "/b"]);
        assert!(b.e_favorita("/b"));
        assert_eq!(b.reproducoes("/a"), 2);
        let lido = Biblioteca::desde_json(&serde_json::to_string(&b).unwrap());
        assert_eq!(lido.favoritas(), b.favoritas());
        // Corrompido: começa vazio em vez de derrubar o app
        assert!(Biblioteca::desde_json("{lixo").favoritas().is_empty());
    }

    #[test]
    fn ouvida_na_metade_ou_aos_quatro_minutos() {
        assert!(!conta_como_ouvida(59.0, 120));
        assert!(conta_como_ouvida(60.0, 120));
        assert!(conta_como_ouvida(240.0, 1200));
        assert!(!conta_como_ouvida(29.0, 0));
        assert!(conta_como_ouvida(30.0, 0));
    }

    #[test]
    fn chave_igual_pela_pasta_ou_pelo_caminho_absoluto() {
        let raiz = Path::new("/musica");
        let absoluta = chave_faixa(raiz, "sub/a.mp3");
        assert_eq!(absoluta, "/musica/sub/a.mp3");
        assert_eq!(chave_faixa(Path::new(""), &absoluta), absoluta);
    }
}
