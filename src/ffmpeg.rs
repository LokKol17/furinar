//! `ffmpeg`/`ffprobe` do sistema: vídeos como faixas (áudio de MKV/WebM/AVI,
//! duração e tags que o lofty não lê, quadro para capa) e os fundos de vídeo.
//! Nada aqui é obrigatório: sem `ffmpeg` no PATH, cada chamador cai para o
//! que faz sem ele.

use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::Duration;

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
pub fn sondar(caminho: &Path) -> Option<Sonda> {
    if !disponivel() {
        return None;
    }
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
}
