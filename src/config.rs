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
    /// Aba que estava sendo exibida.
    #[serde(default)]
    pub aba_visivel_salva: usize,
    /// Pasta de onde vinha a faixa que estava tocando.
    #[serde(default)]
    pub pasta_reproducao_salva: Option<usize>,
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
    pub indice_atual: Option<usize>,
    #[serde(default)]
    pub tempo_atual: Option<u64>,
    #[serde(default)]
    pub escanear_subpastas: bool,
    /// Tema claro ligado. `false` = escuro (padrão, mantém quem já usa o app).
    #[serde(default)]
    pub tema_claro: bool,
    #[serde(default = "default_idioma")]
    pub idioma: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            pasta: None,
            pastas: Vec::new(),
            aba_visivel_salva: 0,
            pasta_reproducao_salva: None,
            volume: default_volume(),
            modo_loop: 0,
            shuffle: false,
            modo_shuffle: 0,
            indice_atual: None,
            tempo_atual: None,
            escanear_subpastas: false,
            tema_claro: false,
            idioma: default_idioma(),
        }
    }
}

fn default_volume() -> f32 {
    0.8
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
    fn migra_shuffle_antigo_para_o_modo() {
        let mut config = AppConfig::desde_json(r#"{"shuffle":true}"#);
        config.migrar();

        assert_eq!(config.modo_shuffle, 1);
    }
}
