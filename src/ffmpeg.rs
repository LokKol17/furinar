//! `ffmpeg`/`ffprobe` do sistema: vídeos como faixas (áudio de MKV/WebM/AVI,
//! duração e tags que o lofty não lê, quadro para capa) e os fundos de vídeo.
//! Nada aqui é obrigatório: sem `ffmpeg` no PATH, cada chamador cai para o
//! que faz sem ele.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Vídeos cujo áudio (AAC) o symphonia decodifica sozinho.
pub const EXT_VIDEO_NATIVO: [&str; 3] = ["mp4", "m4v", "mov"];
/// Vídeos que só tocam com o `ffmpeg` decodificando o áudio (Opus etc.).
pub const EXT_VIDEO_FFMPEG: [&str; 3] = ["mkv", "webm", "avi"];

fn extensao(caminho: &Path) -> String {
    caminho
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
}

/// A faixa é um arquivo de vídeo (toca o vídeo no fundo do modo imersivo).
pub fn e_video(caminho: &Path) -> bool {
    let ext = extensao(caminho);
    EXT_VIDEO_NATIVO.contains(&ext.as_str()) || EXT_VIDEO_FFMPEG.contains(&ext.as_str())
}

/// O áudio só sai pelo `ffmpeg` (o symphonia não tem o container/codec).
pub fn so_com_ffmpeg(caminho: &Path) -> bool {
    EXT_VIDEO_FFMPEG.contains(&extensao(caminho).as_str())
}

/// `ffmpeg` e `ffprobe` no PATH. Verificado uma vez só: o scan pergunta a
/// cada arquivo.
pub fn disponivel() -> bool {
    static DISPONIVEL: OnceLock<bool> = OnceLock::new();
    *DISPONIVEL.get_or_init(|| {
        ["ffmpeg", "ffprobe"].iter().all(|p| {
            comando(p)
                .arg("-version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|s| s.success())
        })
    })
}

#[cfg(not(target_os = "windows"))]
pub fn comando(programa: &str) -> Command {
    Command::new(programa)
}

/// `Command` sem janela de console piscando.
#[cfg(target_os = "windows")]
pub fn comando(programa: &str) -> Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut cmd = Command::new(programa);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// O que o `ffprobe` diz de um arquivo.
#[derive(Debug, Default, PartialEq)]
pub struct Sonda {
    pub duracao: Duration,
    pub titulo: Option<String>,
    pub artista: Option<String>,
}

/// Duração e tags do container, para o que o lofty não lê (MKV/WebM/AVI).
/// O resultado fica num cache em disco (`sondas.json`, ao lado das capas)
/// pelo tamanho e data de modificação: a varredura roda na thread da UI, e
/// cada `ffprobe` custa dezenas de ms.
pub fn sondar(caminho: &Path) -> Option<Sonda> {
    if !disponivel() {
        return None;
    }
    let assinatura = assinatura(caminho)?;
    let com_cache = |f: &mut dyn FnMut(&mut CacheSondas) -> Option<Sonda>| {
        let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
        f(cache.get_or_insert_with(|| CacheSondas::carregar(arquivo_cache())))
    };
    if let Some(s) = com_cache(&mut |c| c.consultar(caminho, assinatura)) {
        return Some(s);
    }
    // O ffprobe roda sem o lock: a thread das miniaturas também sonda
    let s = sondar_agora(caminho)?;
    com_cache(&mut |c| {
        c.guardar(caminho, assinatura, &s);
        None
    });
    Some(s)
}

/// Grava o cache das sondas, se mudou (chamado ao fim de cada varredura).
pub fn gravar_cache() {
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(c) = cache.as_mut() {
        c.gravar();
    }
}

static CACHE: Mutex<Option<CacheSondas>> = Mutex::new(None);

fn arquivo_cache() -> Option<PathBuf> {
    crate::miniaturas::pasta_cache_furinar().map(|p| p.join("sondas.json"))
}

/// Acima disso, entradas de arquivos que sumiram saem ao gravar.
const MAX_SONDAS: usize = 5000;

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
struct Entrada {
    tamanho: u64,
    modificado: u64,
    duracao_ms: u64,
    titulo: Option<String>,
    artista: Option<String>,
}

#[derive(Default)]
struct CacheSondas {
    arquivo: Option<PathBuf>,
    mapa: HashMap<String, Entrada>,
    sujo: bool,
}

/// Tamanho e data de modificação (s): mudou um dos dois, sonda de novo.
fn assinatura(caminho: &Path) -> Option<(u64, u64)> {
    let m = std::fs::metadata(caminho).ok()?;
    let modificado = m
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some((m.len(), modificado))
}

impl CacheSondas {
    fn carregar(arquivo: Option<PathBuf>) -> Self {
        let mapa = arquivo
            .as_deref()
            .and_then(|a| std::fs::read(a).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self {
            arquivo,
            mapa,
            sujo: false,
        }
    }

    fn consultar(&self, caminho: &Path, (tamanho, modificado): (u64, u64)) -> Option<Sonda> {
        let e = self.mapa.get(caminho.to_string_lossy().as_ref())?;
        (e.tamanho == tamanho && e.modificado == modificado).then(|| Sonda {
            duracao: Duration::from_millis(e.duracao_ms),
            titulo: e.titulo.clone(),
            artista: e.artista.clone(),
        })
    }

    fn guardar(&mut self, caminho: &Path, (tamanho, modificado): (u64, u64), s: &Sonda) {
        self.mapa.insert(
            caminho.to_string_lossy().into_owned(),
            Entrada {
                tamanho,
                modificado,
                duracao_ms: s.duracao.as_millis() as u64,
                titulo: s.titulo.clone(),
                artista: s.artista.clone(),
            },
        );
        self.sujo = true;
    }

    #[cfg(test)]
    fn sondar(&mut self, caminho: &Path, sondar: impl Fn(&Path) -> Option<Sonda>) -> Option<Sonda> {
        let assinatura = assinatura(caminho)?;
        if let Some(s) = self.consultar(caminho, assinatura) {
            return Some(s);
        }
        let s = sondar(caminho)?;
        self.guardar(caminho, assinatura, &s);
        Some(s)
    }

    /// Via arquivo temporário + rename: outra instância nunca lê pela metade.
    fn gravar(&mut self) {
        if !self.sujo {
            return;
        }
        if self.mapa.len() > MAX_SONDAS {
            self.mapa.retain(|k, _| Path::new(k).is_file());
        }
        let Some(arquivo) = &self.arquivo else {
            return;
        };
        let Ok(json) = serde_json::to_vec(&self.mapa) else {
            return;
        };
        if let Some(pasta) = arquivo.parent() {
            let _ = std::fs::create_dir_all(pasta);
        }
        let temp = arquivo.with_extension("json.tmp");
        if std::fs::write(&temp, json).is_ok() && std::fs::rename(&temp, arquivo).is_ok() {
            self.sujo = false;
        }
    }
}

fn sondar_agora(caminho: &Path) -> Option<Sonda> {
    let saida = comando("ffprobe")
        .args(["-v", "error", "-of", "json"])
        .args(["-show_entries", "format=duration:format_tags=title,artist"])
        .arg(caminho)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !saida.status.success() {
        return None;
    }
    ler_sonda(&String::from_utf8_lossy(&saida.stdout))
}

fn ler_sonda(json: &str) -> Option<Sonda> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let formato = v.get("format")?;
    let duracao = formato
        .get("duration")
        .and_then(|d| d.as_str())
        .and_then(|d| d.parse::<f64>().ok())
        .filter(|d| d.is_finite() && *d >= 0.0)
        .map_or(Duration::ZERO, Duration::from_secs_f64);
    // As chaves das tags variam de caixa conforme o container (TITLE no MKV)
    let tag = |nome: &str| {
        formato
            .get("tags")?
            .as_object()?
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(nome))
            .and_then(|(_, v)| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    Some(Sonda {
        duracao,
        titulo: tag("title"),
        artista: tag("artist"),
    })
}

/// Um quadro do vídeo em JPEG, com o lado maior em `lado` px, para servir de
/// capa. Tirado a 10% da duração (máx. 30 s): o primeiro quadro costuma ser
/// preto.
pub fn quadro_jpeg(caminho: &Path, lado: u32) -> Option<Vec<u8>> {
    if !e_video(caminho) || !disponivel() {
        return None;
    }
    let duracao = sondar(caminho).map_or(Duration::ZERO, |s| s.duracao);
    let instante = (duracao / 10).min(Duration::from_secs(30));
    let saida = comando("ffmpeg")
        .args(["-v", "error", "-nostdin", "-ss"])
        .arg(format!("{:.3}", instante.as_secs_f64()))
        .arg("-i")
        .arg(caminho)
        .args(["-an", "-frames:v", "1", "-vf"])
        .arg(format!(
            "scale={lado}:{lado}:force_original_aspect_ratio=decrease"
        ))
        .args(["-f", "image2pipe", "-c:v", "mjpeg", "-"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    (saida.status.success() && !saida.stdout.is_empty()).then_some(saida.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconhece_videos_pela_extensao() {
        assert!(e_video(Path::new("a/clipe.MP4")));
        assert!(e_video(Path::new("clipe.webm")));
        assert!(!e_video(Path::new("musica.m4a")));
        assert!(so_com_ffmpeg(Path::new("x.MKV")));
        assert!(!so_com_ffmpeg(Path::new("x.mov")));
    }

    #[test]
    fn sonda_le_duracao_e_tags_em_qualquer_caixa() {
        let json = r#"{"format":{"duration":"212.480000","tags":{"TITLE":"Mei Mei","artist":" PinocchioP "}}}"#;
        assert_eq!(
            ler_sonda(json),
            Some(Sonda {
                duracao: Duration::from_secs_f64(212.48),
                titulo: Some("Mei Mei".into()),
                artista: Some("PinocchioP".into()),
            })
        );
        let sem_tags = ler_sonda(r#"{"format":{"duration":"N/A"}}"#).unwrap();
        assert_eq!(sem_tags, Sonda::default());
        assert_eq!(ler_sonda("{}"), None);
    }

    #[test]
    fn cache_de_sondas_so_sonda_de_novo_quando_o_arquivo_muda() {
        let pasta =
            std::env::temp_dir().join(format!("furinar-teste-sondas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pasta);
        std::fs::create_dir_all(&pasta).unwrap();
        let video = pasta.join("clipe.webm");
        std::fs::write(&video, b"abc").unwrap();
        let arquivo = pasta.join("sondas.json");

        let chamadas = std::cell::Cell::new(0);
        let falsa = |_: &Path| {
            chamadas.set(chamadas.get() + 1);
            Some(Sonda {
                duracao: Duration::from_millis(212_480),
                titulo: Some("Mei Mei".into()),
                artista: None,
            })
        };

        let mut c = CacheSondas::carregar(Some(arquivo.clone()));
        let primeira = c.sondar(&video, falsa).unwrap();
        assert_eq!(c.sondar(&video, falsa).unwrap(), primeira);
        assert_eq!(chamadas.get(), 1);

        // Sobrevive a reabrir o app
        c.gravar();
        let mut c = CacheSondas::carregar(Some(arquivo));
        assert_eq!(c.sondar(&video, falsa).unwrap(), primeira);
        assert_eq!(chamadas.get(), 1);

        // Arquivo mudou de tamanho: sonda de novo
        std::fs::write(&video, b"abcdef").unwrap();
        c.sondar(&video, falsa);
        assert_eq!(chamadas.get(), 2);
        let _ = std::fs::remove_dir_all(pasta);
    }
}
