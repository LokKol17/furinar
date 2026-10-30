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
    /// Buscar a letra na web (LRCLIB) quando a faixa não tem. Ligado em
    /// instalações novas; configurações antigas sem o campo ficam desligadas.
    #[serde(default)]
    pub letras_web: bool,
    /// Última escolha de "Também salvar no arquivo" na tela de informações.
    #[serde(default)]
    pub gravar_tags_info: bool,
    /// Chave de aplicativo do AcoustID (impressão digital).
    #[serde(default)]
    pub chave_acoustid: String,
    /// Mostrar a faixa tocando no perfil do Discord.
    #[serde(default)]
    pub discord: bool,
    /// Scrobble: token do ListenBrainz e conta do Last.fm (chave e segredo da
    /// "API account" do usuário, sessão obtida na conexão).
    #[serde(default)]
    pub listenbrainz_token: String,
    #[serde(default)]
    pub lastfm_chave: String,
    #[serde(default)]
    pub lastfm_segredo: String,
    #[serde(default)]
    pub lastfm_sessao: String,
    #[serde(default)]
    pub lastfm_usuario: String,
    #[serde(default = "verdadeiro")]
    pub replaygain: bool,
    /// Fundo do modo imersivo pulsando com os graves. Desligado por padrão:
    /// cada pulso repinta a tela inteira no renderer de software.
    #[serde(default)]
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
            letras_web: true,
            gravar_tags_info: false,
            chave_acoustid: String::new(),
            discord: false,
            listenbrainz_token: String::new(),
            lastfm_chave: String::new(),
            lastfm_segredo: String::new(),
            lastfm_sessao: String::new(),
            lastfm_usuario: String::new(),
            replaygain: true,
            fundo_reativo: false,
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
    "en".to_string()
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
    #[cfg(target_os = "windows")]
    {
        let exe = std::env::current_exe().ok();
        let appdata = std::env::var_os("APPDATA").map(PathBuf::from);
        pasta_windows(
            exe.as_deref().and_then(std::path::Path::parent),
            appdata.as_deref(),
            std::path::Path::exists,
        )
        .join("furinar_config.json")
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        PathBuf::from("furinar_config.json")
    }
}

/// Pasta de música do usuário: `XDG_MUSIC_DIR` (user-dirs.dirs), ou `~/Music`.
#[cfg(not(target_os = "windows"))]
fn pasta_de_musica() -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let configuracao = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"));
    fs::read_to_string(configuracao.join("user-dirs.dirs"))
        .ok()
        .and_then(|conteudo| musica_xdg(&conteudo, &home))
        .or_else(|| Some(home.join("Music")))
}

/// O `XDG_MUSIC_DIR` de um user-dirs.dirs. Apontar para a própria home é o
/// jeito de desligar a pasta: aí não vale.
#[cfg(not(target_os = "windows"))]
fn musica_xdg(conteudo: &str, home: &std::path::Path) -> Option<PathBuf> {
    let valor = conteudo
        .lines()
        .find_map(|l| l.trim().strip_prefix("XDG_MUSIC_DIR="))?
        .trim()
        .trim_matches('"');
    let caminho = match valor.strip_prefix("$HOME") {
        Some(resto) => home.join(resto.trim_start_matches('/')),
        None => PathBuf::from(valor),
    };
    (caminho.is_absolute() && caminho != home).then_some(caminho)
}

/// A pasta Música do Windows (pode ter sido movida, para o OneDrive etc.).
#[cfg(target_os = "windows")]
fn pasta_de_musica() -> Option<PathBuf> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{FOLDERID_Music, KNOWN_FOLDER_FLAG, SHGetKnownFolderPath};
    // SAFETY: o PWSTR devolvido é lido uma vez e liberado com CoTaskMemFree
    unsafe {
        let caminho = SHGetKnownFolderPath(&FOLDERID_Music, KNOWN_FOLDER_FLAG(0), None).ok()?;
        let texto = caminho.to_string().ok();
        CoTaskMemFree(Some(caminho.0 as *const _));
        texto.map(PathBuf::from)
    }
}

/// Pasta dos arquivos do Furinar no Windows. Uma configuração ao lado do
/// executável (como as versões antigas, sem instalador, salvavam) mantém o
/// modo portátil; senão, `%APPDATA%\Furinar` — a pasta de instalação não é
/// lugar de dados.
#[cfg(any(target_os = "windows", test))]
fn pasta_windows(
    exe: Option<&std::path::Path>,
    appdata: Option<&std::path::Path>,
    existe: impl Fn(&std::path::Path) -> bool,
) -> PathBuf {
    if let Some(exe) = exe
        && existe(&exe.join("furinar_config.json"))
    {
        return exe.to_path_buf();
    }
    match appdata {
        Some(appdata) => appdata.join("Furinar"),
        None => exe.map(std::path::Path::to_path_buf).unwrap_or_default(),
    }
}

impl AppConfig {
    pub fn carregar() -> Self {
        let config_path = config_path();
        if let Some(parent) = config_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let mut config = match fs::read_to_string(&config_path) {
            Ok(conteudo) => Self::desde_json(&conteudo),
            Err(_) if !config_path.exists() => Self::primeira_execucao(),
            Err(_) => Self::default(),
        };
        config.migrar();
        config
    }

    /// Sem configuração ainda: já abre a pasta de música do sistema, com as
    /// subpastas (é onde costumam ficar artista/álbum).
    fn primeira_execucao() -> Self {
        let mut config = Self::default();
        if let Some(musica) = pasta_de_musica().filter(|p| p.is_dir()) {
            config.pastas.push(musica.to_string_lossy().into_owned());
            config.escanear_subpastas = true;
        }
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

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn pasta_de_musica_do_user_dirs() {
        use std::path::Path;
        let home = Path::new("/home/a");
        let conteudo =
            "# comentário\nXDG_DESKTOP_DIR=\"$HOME/Desktop\"\nXDG_MUSIC_DIR=\"$HOME/Músicas\"\n";
        assert_eq!(musica_xdg(conteudo, home), Some(home.join("Músicas")));
        assert_eq!(
            musica_xdg("XDG_MUSIC_DIR=\"/mnt/musica\"", home),
            Some(PathBuf::from("/mnt/musica"))
        );
        // Desligada (aponta para a home) ou ausente
        assert_eq!(musica_xdg("XDG_MUSIC_DIR=\"$HOME/\"", home), None);
        assert_eq!(musica_xdg("XDG_VIDEOS_DIR=\"$HOME/V\"", home), None);
    }

    #[test]
    fn windows_usa_appdata_ou_fica_portatil() {
        use std::path::Path;
        let exe = Path::new(r"C:\Users\a\AppData\Local\Programs\Furinar");
        let appdata = Path::new(r"C:\Users\a\AppData\Roaming");
        // Instalado (ou .exe solto novo): dados no %APPDATA%
        assert_eq!(
            pasta_windows(Some(exe), Some(appdata), |_| false),
            appdata.join("Furinar")
        );
        // Config ao lado do .exe (versões antigas, portátil): continua lá
        assert_eq!(pasta_windows(Some(exe), Some(appdata), |_| true), exe);
        // Sem %APPDATA%: pasta do executável
        assert_eq!(pasta_windows(Some(exe), None, |_| false), exe);
    }

    #[test]
    fn json_sem_volume_carrega_padrao_e_preserva_o_restante() {
        let config = AppConfig::desde_json(r#"{"pastas":["/musica"],"tema_claro":true}"#);

        assert_eq!(config.volume, 0.8);
        assert_eq!(config.modo_loop, 0);
        assert_eq!(config.pastas, vec!["/musica".to_string()]);
        assert!(config.tema_claro);
        // ReplayGain ligado para quem vem de uma versão antiga; o fundo
        // reativo (mais CPU) só se a pessoa ligar
        assert!(config.replaygain && !config.fundo_reativo);
        assert_eq!(config.crossfade, 0);
    }

    #[test]
    fn json_irrecuperavel_cai_no_padrao_com_volume_audivel() {
        let config = AppConfig::desde_json("{isso nao e json");

        assert_eq!(config.volume, 0.8, "config corrompida não pode deixar mudo");
        assert!(config.pastas.is_empty());
        assert_eq!(config.idioma, "en");
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
