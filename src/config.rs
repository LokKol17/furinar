use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct AppConfig {
    /// Campo antigo (uma pasta só). Existe apenas para migrar configs antigas.
    #[serde(default, skip_serializing)]
    pub pasta: Option<String>,
    /// Pastas abertas, na ordem das abas.
    #[serde(default)]
    pub pastas: Vec<String>,
    /// Campos antigos (posições nas listas): só para migrar configs antigas.
    /// Posição não sobrevive a uma faixa nova na pasta; hoje vale o caminho.
    #[serde(default, skip_serializing)]
    pub aba_visivel_salva: usize,
    #[serde(default, skip_serializing)]
    pub pasta_reproducao_salva: Option<usize>,
    #[serde(default, skip_serializing)]
    pub indice_atual: Option<usize>,
    /// Aba exibida e lista de onde vinha a faixa (caminho da pasta, ou a
    /// chave de uma lista especial).
    #[serde(default)]
    pub aba_salva: Option<String>,
    #[serde(default)]
    pub lista_salva: Option<String>,
    /// Faixa que tocava (caminho absoluto) e a posição, em segundos.
    #[serde(default)]
    pub faixa_salva: Option<String>,
    /// Sem `default` o JSON de uma versão antiga não parseava e a configuração
    /// inteira era descartada — com volume virando 0.0 (mudo).
    #[serde(default = "default_volume")]
    pub volume: f32,
    #[serde(default)]
    pub modo_loop: u8,
    /// Campo antigo (só ligado/desligado). Existe apenas para migrar configs antigas.
    #[serde(default, skip_serializing)]
    pub shuffle: bool,
    /// 0 = desligado, 1 = shuffle, 2 = shuffle inteligente.
    #[serde(default)]
    pub modo_shuffle: u8,
    #[serde(default)]
    pub tempo_atual: Option<u64>,
    /// Próximas do shuffle já sorteadas (caminhos absolutos), para retomar a
    /// mesma fila.
    #[serde(default)]
    pub fila_shuffle: Vec<String>,
    /// Ordem escolhida em cada aba (chave da lista → `Ordem::indice`).
    #[serde(default)]
    pub ordens: HashMap<String, u8>,
    /// Crossfade em segundos (0 = só gapless).
    #[serde(default)]
    pub crossfade: u8,
    /// Buscar a letra na web (LRCLIB) quando a faixa não tem. Desligado
    /// até o usuário ligar: manda título/artista a um serviço de fora.
    #[serde(default)]
    pub letras_web: bool,
    /// Chave de aplicativo do AcoustID (impressão digital).
    #[serde(default)]
    pub chave_acoustid: String,
    #[serde(default = "verdadeiro")]
    pub replaygain: bool,
    /// Fundo do modo imersivo pulsando com os graves.
    #[serde(default = "verdadeiro")]
    pub fundo_reativo: bool,
    #[serde(default)]
    pub escanear_subpastas: bool,
    /// Tema claro ligado. `false` = escuro (padrão, mantém quem já usa o app).
    #[serde(default)]
    pub tema_claro: bool,
    #[serde(default = "default_idioma")]
    pub idioma: String,
    /// Campo antigo (um fundo só). Existe apenas para migrar configs antigas.
    #[serde(default, skip_serializing)]
    pub fundo_tela_cheia: Option<String>,
    /// Fundos do modo imersivo (imagem, GIF/WebP ou vídeo); com mais de um,
    /// trocam a cada faixa.
    #[serde(default)]
    pub fundos_tela_cheia: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            pasta: None,
            pastas: Vec::new(),
            aba_visivel_salva: 0,
            pasta_reproducao_salva: None,
            indice_atual: None,
            aba_salva: None,
            lista_salva: None,
            faixa_salva: None,
            volume: default_volume(),
            modo_loop: 0,
            shuffle: false,
            modo_shuffle: 0,
            tempo_atual: None,
            fila_shuffle: Vec::new(),
            ordens: HashMap::new(),
            crossfade: 0,
            letras_web: false,
            chave_acoustid: String::new(),
            replaygain: true,
            fundo_reativo: true,
            escanear_subpastas: false,
            tema_claro: false,
            idioma: default_idioma(),
            fundo_tela_cheia: None,
            fundos_tela_cheia: Vec::new(),
        }
    }
}

fn default_volume() -> f32 {
    0.8
}

fn verdadeiro() -> bool {
    true
}

fn default_idioma() -> String {
    "pt-br".to_string()
}

pub fn config_path() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            PathBuf::from(xdg)
                .join("furinar")
                .join("furinar_config.json")
        } else {
            if let Ok(home) = std::env::var("HOME") {
                PathBuf::from(home)
                    .join(".config")
                    .join("furinar")
                    .join("furinar_config.json")
            } else {
                PathBuf::from("furinar_config.json")
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        PathBuf::from("furinar_config.json")
    }
}

impl AppConfig {
    pub fn carregar() -> Self {
        let config_path = config_path();
        if let Some(parent) = config_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let mut config = fs::read_to_string(&config_path)
            .map(|conteudo| Self::desde_json(&conteudo))
            .unwrap_or_default();
        config.migrar();
        config
    }

    /// Lê o JSON preservando tudo o que der. Campo ausente cai no padrão e,
    /// só se o arquivo estiver irrecuperável, volta à configuração inteira.
    fn desde_json(conteudo: &str) -> Self {
        let mut config: Self = serde_json::from_str(conteudo).unwrap_or_default();
        config.sanitizar();
        config
    }

    /// Garante que o volume salvo seja usável: fora de 0.0..=1.0 ou não
    /// finito (NaN/infinito, vindo de JSON corrompido) volta ao padrão.
    fn sanitizar(&mut self) {
        if !self.volume.is_finite() {
            self.volume = default_volume();
        } else {
            self.volume = self.volume.clamp(0.0, 1.0);
        }
    }

    /// Migra configs antigas: o campo `pasta` (singular) vira uma entrada em
    /// `pastas`, e essa pasta era necessariamente a que estava tocando. O
    /// campo `shuffle` (bool) vira `modo_shuffle` (0/1/2).
    pub fn migrar(&mut self) {
        if self.pastas.is_empty()
            && let Some(antiga) = self.pasta.take()
        {
            self.pastas.push(antiga);
            self.pasta_reproducao_salva = Some(0);
        }
        if self.modo_shuffle == 0 && self.shuffle {
            self.modo_shuffle = 1;
        }
        if self.fundos_tela_cheia.is_empty()
            && let Some(antigo) = self.fundo_tela_cheia.take()
        {
            self.fundos_tela_cheia.push(antigo);
        }
    }

    pub fn salvar(&self) {
        let config_path = config_path();
        if let Some(parent) = config_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(&config_path, json);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_sem_volume_carrega_padrao_e_preserva_o_restante() {
        let config = AppConfig::desde_json(r#"{"pastas":["/musica"],"tema_claro":true}"#);

        assert_eq!(config.volume, 0.8);
        assert_eq!(config.modo_loop, 0);
        assert_eq!(config.pastas, vec!["/musica".to_string()]);
        assert!(config.tema_claro);
        // Opções novas ligadas para quem vem de uma versão antiga
        assert!(config.replaygain && config.fundo_reativo);
        assert_eq!(config.crossfade, 0);
    }

    #[test]
    fn json_irrecuperavel_cai_no_padrao_com_volume_audivel() {
        let config = AppConfig::desde_json("{isso nao e json");

        assert_eq!(config.volume, 0.8, "config corrompida não pode deixar mudo");
        assert!(config.pastas.is_empty());
        assert_eq!(config.idioma, "pt-br");
    }

    #[test]
    fn volume_fora_da_faixa_e_limitado_ao_slider() {
        let config = AppConfig::desde_json(r#"{"volume":1.5}"#);
        assert_eq!(config.volume, 1.0);

        let config = AppConfig::desde_json(r#"{"volume":-3}"#);
        assert_eq!(config.volume, 0.0);
    }

    #[test]
    fn volume_nao_finito_volta_ao_padrao() {
        let mut config = AppConfig {
            volume: f32::NAN,
            ..Default::default()
        };
        config.sanitizar();
        assert_eq!(config.volume, 0.8);
    }

    #[test]
    fn migra_pasta_antiga_para_a_lista() {
        let mut config = AppConfig::desde_json(r#"{"pasta":"/antiga"}"#);
        config.migrar();

        assert_eq!(config.pastas, vec!["/antiga".to_string()]);
        assert_eq!(config.pasta_reproducao_salva, Some(0));
    }

    #[test]
    fn migra_fundo_unico_para_a_lista() {
        let mut config = AppConfig::desde_json(r#"{"fundo_tela_cheia":"/f.gif"}"#);
        config.migrar();
        assert_eq!(config.fundos_tela_cheia, vec!["/f.gif".to_string()]);
    }

    #[test]
    fn migra_shuffle_antigo_para_o_modo() {
        let mut config = AppConfig::desde_json(r#"{"shuffle":true}"#);
        config.migrar();

        assert_eq!(config.modo_shuffle, 1);
    }
}
