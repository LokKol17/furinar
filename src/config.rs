use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
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
    pub volume: f32,
    pub modo_loop: u8,
    /// Campo antigo (só ligado/desligado). Existe apenas para migrar configs antigas.
    #[serde(default, skip_serializing)]
    pub shuffle: bool,
    /// 0 = desligado, 1 = shuffle, 2 = shuffle inteligente.
    #[serde(default)]
    pub modo_shuffle: u8,
    pub indice_atual: Option<usize>,
    pub tempo_atual: Option<u64>,
    #[serde(default)]
    pub escanear_subpastas: bool,
    /// Tema claro ligado. `false` = escuro (padrão, mantém quem já usa o app).
    #[serde(default)]
    pub tema_claro: bool,
    #[serde(default = "default_idioma")]
    pub idioma: String,
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
        let mut config = if let Ok(conteudo) = fs::read_to_string(&config_path) {
            serde_json::from_str(&conteudo).unwrap_or_default()
        } else {
            Self {
                volume: 0.8,
                ..Default::default()
            }
        };
        config.migrar();
        config
    }

    /// Migra configs antigas: o campo `pasta` (singular) vira uma entrada em
    /// `pastas`, e essa pasta era necessariamente a que estava tocando. O
    /// campo `shuffle` (bool) vira `modo_shuffle` (0/1/2).
    pub fn migrar(&mut self) {
        if self.pastas.is_empty() {
            if let Some(antiga) = self.pasta.take() {
                self.pastas.push(antiga);
                self.pasta_reproducao_salva = Some(0);
            }
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
