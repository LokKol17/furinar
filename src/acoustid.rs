//! Identificação pelo som: impressão digital Chromaprint (em Rust puro) e
//! consulta ao AcoustID, que devolve os MBIDs das gravações.
//!
//! Precisa de uma chave de aplicativo do AcoustID (grátis, em
//! acoustid.org/new-application): a do usuário, nas configurações, ou a do
//! Furinar, embutida nos builds da release. Calcular a impressão decodifica
//! os primeiros 2 minutos da faixa: ~1-2 s de CPU.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::Duration;

use base64::Engine;
use rodio::Source;
use rusty_chromaprint::{Configuration, FingerprintCompressor, Fingerprinter};
use serde::Deserialize;

use crate::metadados::proximidade_duracao;
use crate::rede::{Cliente, Erro, Tipo};

/// Chave de aplicativo do Furinar: vem do secret `ACOUSTID_CHAVE` no CI da
/// release (nunca no repositório). Builds locais saem sem ela.
const CHAVE_EMBUTIDA: Option<&str> = option_env!("FURINAR_ACOUSTID_CHAVE");

/// A chave do usuário, se preenchida; senão a embutida.
pub fn chave_efetiva(do_usuario: &str) -> Option<String> {
    Some(do_usuario.trim())
        .filter(|c| !c.is_empty())
        .or(CHAVE_EMBUTIDA.map(str::trim).filter(|c| !c.is_empty()))
        .map(str::to_string)
}

const HOST: &str = "api.acoustid.org";
/// O AcoustID usa os 120 primeiros segundos.
const SEGUNDOS: u64 = 120;
/// Menos áudio que isso não dá uma impressão que o AcoustID aceite.
const MINIMO_SEGUNDOS: u32 = 10;

/// Impressão digital comprimida (base64, como o `fpcalc` gera) e a duração
/// da faixa em segundos.
pub fn impressao(caminho: &Path) -> Option<(String, u32)> {
    let (amostras, taxa, canais, duracao) = abrir_pcm(caminho)?;

    let config = Configuration::preset_test2();
    let mut fp = Fingerprinter::new(&config);
    fp.start(taxa, canais).ok()?;
    let limite = (SEGUNDOS * taxa as u64 * canais as u64) as usize;
    let mut lote = Vec::with_capacity(8192);
    let mut lidas = 0usize;
    for amostra in amostras {
        lote.push((amostra.clamp(-1.0, 1.0) * i16::MAX as f32) as i16);
        lidas += 1;
        if lote.len() == lote.capacity() {
            fp.consume(&lote);
            lote.clear();
        }
        if lidas >= limite {
            break;
        }
    }
    fp.consume(&lote);
    fp.finish();
    // Pouco áudio decodificado (arquivo quebrado, decodificador que parou
    // cedo): o AcoustID recusa com 400 ou devolve lixo
    let segundos_lidos = (lidas as u64 / (taxa as u64 * canais as u64).max(1)) as u32;
    if fp.fingerprint().is_empty() || segundos_lidos < MINIMO_SEGUNDOS {
        return None;
    }
    let comprimida = FingerprintCompressor::from(&config).compress(fp.fingerprint());
    let texto = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(comprimida);
    // Duração 0 também é 400 ("invalid duration"): o container pode não
    // dizer (MP3 sem cabeçalho Xing, M4A fragmentado de download)
    let duracao = duracao.filter(|&d| d > 0).unwrap_or(segundos_lidos);
    Some((texto, duracao))
}

/// Amostras, taxa, canais e duração (s). Vídeo vai pelo `ffmpeg` quando há
/// um (o symphonia erra a duração de MP4 com vídeo, e MKV/WebM ele nem
/// abre); o resto pelo symphonia, com o `ffmpeg` de reserva.
type Pcm = (Box<dyn Iterator<Item = f32>>, u32, u32, Option<u32>);

fn abrir_pcm(caminho: &Path) -> Option<Pcm> {
    let video = crate::ffmpeg::e_video(caminho);
    let pelo_ffmpeg = || {
        let pcm =
            crate::audio_ffmpeg::PcmBloqueante::abrir(caminho, Duration::from_secs(SEGUNDOS))?;
        let duracao = crate::track::duracao_container(caminho)
            .or_else(|| crate::ffmpeg::sondar(caminho).map(|s| s.duracao))
            .map(|d| d.as_secs() as u32);
        let (taxa, canais) = (pcm.taxa(), pcm.canais());
        Some((
            Box::new(pcm) as Box<dyn Iterator<Item = f32>>,
            taxa,
            canais,
            duracao,
        ))
    };
    if video && let Some(pcm) = pelo_ffmpeg() {
        return Some(pcm);
    }
    if !crate::ffmpeg::so_com_ffmpeg(caminho) {
        let nativo = File::open(caminho).ok().and_then(|arquivo| {
            let byte_len = arquivo.metadata().ok()?.len();
            let extensao = caminho
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            crate::criar_decodificador(BufReader::new(arquivo), &extensao, byte_len).ok()
        });
        if let Some(d) = nativo {
            let taxa = d.sample_rate().get();
            let canais = d.channels().get() as u32;
            let duracao = if video {
                crate::track::duracao_container(caminho)
            } else {
                d.total_duration()
            };
            let duracao = duracao.map(|d| d.as_secs() as u32);
            return Some((Box::new(d), taxa, canais, duracao));
        }
    }
    pelo_ffmpeg()
}

#[derive(Debug, Deserialize)]
struct Resposta {
    #[serde(default)]
    status: String,
    #[serde(default)]
    results: Vec<Resultado>,
}

#[derive(Debug, Deserialize)]
struct Resultado {
    #[serde(default)]
    score: f32,
    #[serde(default)]
    recordings: Vec<Gravacao>,
}

#[derive(Debug, Deserialize)]
struct Gravacao {
    id: String,
    #[serde(default)]
    duration: Option<f32>,
}

/// MBIDs das gravações com a confiança (0-100), da maior para a menor.
pub fn consultar(
    cliente: &mut Cliente,
    chave_app: &str,
    caminho: &Path,
) -> Result<Vec<(String, u8)>, Erro> {
    let Some((impressao, duracao)) = impressao(caminho) else {
        return Ok(Vec::new());
    };
    let resposta: Option<Resposta> = cliente.json_post(
        HOST,
        "https://api.acoustid.org/v2/lookup",
        &[
            ("client", chave_app.to_string()),
            ("meta", "recordings".into()),
            ("duration", duracao.to_string()),
            ("fingerprint", impressao),
        ],
    )?;
    let Some(resposta) = resposta else {
        return Ok(Vec::new());
    };
    if resposta.status != "ok" {
        // Chave inválida, por exemplo
        return Err(Erro::novo("AcoustID", Tipo::Recusa(resposta.status)));
    }
    Ok(notas(resposta.results, duracao))
}

fn notas(resultados: Vec<Resultado>, duracao: u32) -> Vec<(String, u8)> {
    let mut ids: Vec<(String, u8)> = Vec::new();
    for r in resultados {
        for g in r.recordings {
            let perto = proximidade_duracao(duracao, g.duration.map_or(0, |d| d.round() as u32));
            // A impressão manda; a duração só desempata gravações parecidas
            let nota = (r.score * (0.9 + 0.1 * perto) * 100.0)
                .round()
                .clamp(0.0, 100.0) as u8;
            if !ids.iter().any(|(id, _)| *id == g.id) {
                ids.push((g.id, nota));
            }
        }
    }
    ids.sort_by_key(|i| std::cmp::Reverse(i.1));
    ids.truncate(8);
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chave_do_usuario_tem_prioridade_sobre_a_embutida() {
        assert_eq!(chave_efetiva("  minha ").as_deref(), Some("minha"));
        let embutida = CHAVE_EMBUTIDA.map(str::trim).filter(|c| !c.is_empty());
        assert_eq!(chave_efetiva("   ").as_deref(), embutida);
    }

    /// WebM com Opus (o symphonia não abre): a impressão vem pelo ffmpeg.
    #[test]
    fn impressao_de_video_webm_pelo_ffmpeg() {
        if !crate::ffmpeg::disponivel() {
            eprintln!("sem ffmpeg: teste pulado");
            return;
        }
        let caminho = std::env::temp_dir().join(format!(
            "furinar-teste-acoustid-{}.webm",
            std::process::id()
        ));
        let gerou = crate::ffmpeg::comando("ffmpeg")
            .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
            .arg("anoisesrc=d=12:c=pink:a=0.3,tremolo=f=3")
            .args(["-c:a", "libopus"])
            .arg(&caminho)
            .status()
            .is_ok_and(|s| s.success());
        if !gerou {
            eprintln!("sem libopus: teste pulado");
            return;
        }
        let (texto, duracao) = impressao(&caminho).expect("deve gerar impressão");
        assert!(!texto.is_empty());
        assert_eq!(duracao, 12);
        let _ = std::fs::remove_file(caminho);
    }

    /// WAV mono 16 bits com notas trocando a cada meio segundo (as fixtures
    /// são curtas demais para uma impressão).
    fn wav_de_teste(caminho: &Path) {
        let taxa = 22_050u32;
        let notas = [261.6f32, 329.6, 392.0, 523.3, 440.0, 349.2];
        let amostras: Vec<i16> = (0..taxa * 15)
            .map(|n| {
                let t = n as f32 / taxa as f32;
                let f = notas[(t * 2.0) as usize % notas.len()];
                ((t * f * std::f32::consts::TAU).sin() * 0.5 * i16::MAX as f32) as i16
            })
            .collect();
        let dados = amostras.len() as u32 * 2;
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + dados).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&taxa.to_le_bytes());
        b.extend_from_slice(&(taxa * 2).to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&dados.to_le_bytes());
        for a in amostras {
            b.extend_from_slice(&a.to_le_bytes());
        }
        std::fs::write(caminho, b).unwrap();
    }

    #[test]
    fn calcula_impressao_de_um_wav() {
        let caminho = std::env::temp_dir().join(format!(
            "furinar-teste-impressao-{}.wav",
            std::process::id()
        ));
        wav_de_teste(&caminho);
        let resultado = impressao(&caminho);
        let _ = std::fs::remove_file(&caminho);
        let (texto, duracao) = resultado.expect("impressão");
        assert_eq!(duracao, 15);
        // Cabeçalho do Chromaprint: algoritmo 1 ("AQ" em base64)
        assert!(texto.starts_with("AQ"), "{texto}");
    }

    #[test]
    fn le_a_resposta_do_acoustid() {
        let json = r#"{"status":"ok","results":[
            {"id":"a","score":0.95,"recordings":[{"id":"r1","duration":200},{"id":"r2","duration":260}]},
            {"id":"b","score":0.40,"recordings":[{"id":"r1"}]}]}"#;
        let r: Resposta = serde_json::from_str(json).unwrap();
        let ids = notas(r.results, 200);
        assert_eq!(ids[0], ("r1".to_string(), 95));
        assert!(ids[1].1 < 95, "duração longe pesa um pouco");
        assert_eq!(ids.len(), 2, "r1 repetida entra uma vez");
    }
}
