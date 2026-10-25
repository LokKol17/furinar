//! Áudio decodificado pelo `ffmpeg` do sistema, para o que o symphonia não
//! abre (MKV/WebM com Opus, AVI...). O `ffmpeg` manda PCM f32 por pipe; uma
//! thread lê em blocos e entrega por um canal limitado, para a thread de
//! áudio nunca esperar por IO.

use std::io::Read;
use std::num::NonZero;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Sample, SampleRate, Source};

use crate::ffmpeg;

const TAXA: u32 = 48_000;
const CANAIS: u16 = 2;
/// Amostras por bloco (~43 ms em estéreo a 48 kHz).
const BLOCO: usize = 4096;
/// Blocos no canal: ~0,3 s decodificados à frente, no máximo.
const BLOCOS_NO_CANAL: usize = 8;
/// Quanto esperar pelo primeiro bloco ao abrir (o `ffmpeg` sobe em ~100 ms).
const ESPERA_INICIAL: Duration = Duration::from_secs(5);

pub struct FonteFfmpeg {
    caminho: PathBuf,
    duracao: Option<Duration>,
    filho: Child,
    rx: Receiver<Vec<f32>>,
    bloco: Vec<f32>,
    pos: usize,
}

impl FonteFfmpeg {
    /// `None` sem `ffmpeg` ou se o arquivo não tem áudio que ele decodifique.
    pub fn abrir(caminho: &Path) -> Option<Self> {
        if !ffmpeg::disponivel() {
            return None;
        }
        // Lofty primeiro (MP4/MOV, sem subir processo); ffprobe para o resto
        let duracao = crate::track::duracao_container(caminho).or_else(|| {
            ffmpeg::sondar(caminho)
                .map(|s| s.duracao)
                .filter(|d| !d.is_zero())
        });
        let (filho, rx) = iniciar(caminho, Duration::ZERO)?;
        let mut fonte = Self {
            caminho: caminho.to_path_buf(),
            duracao,
            filho,
            rx,
            bloco: Vec::new(),
            pos: 0,
        };
        // Primeiro bloco já aqui (fora da thread de áudio): confirma que há
        // áudio e evita começar com silêncio
        fonte.bloco = fonte.rx.recv_timeout(ESPERA_INICIAL).ok()?;
        Some(fonte)
    }

    fn encerrar(&mut self) {
        let _ = self.filho.kill();
        let _ = self.filho.wait();
    }
}

/// Sobe o `ffmpeg` a partir de `inicio` e a thread que lê o pipe.
fn iniciar(caminho: &Path, inicio: Duration) -> Option<(Child, Receiver<Vec<f32>>)> {
    let mut filho = ffmpeg::comando("ffmpeg")
        .args(["-v", "error", "-nostdin", "-ss"])
        .arg(format!("{:.3}", inicio.as_secs_f64()))
        .arg("-i")
        .arg(caminho)
        .args(["-vn", "-sn", "-f", "f32le", "-ac"])
        .arg(CANAIS.to_string())
        .arg("-ar")
        .arg(TAXA.to_string())
        .arg("-")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let saida = filho.stdout.take()?;
    let (tx, rx) = mpsc::sync_channel(BLOCOS_NO_CANAL);
    let lancou = std::thread::Builder::new()
        .name("furinar-audio-ffmpeg".into())
        .spawn(move || ler_pipe(saida, &tx));
    if lancou.is_err() {
        let _ = filho.kill();
        let _ = filho.wait();
        return None;
    }
    Some((filho, rx))
}

/// Lê f32 little-endian do pipe até o fim (ou até a fonte ser solta).
fn ler_pipe(mut saida: impl Read, tx: &mpsc::SyncSender<Vec<f32>>) {
    let mut bruto = vec![0u8; BLOCO * 4];
    // Bytes de uma amostra cortada entre duas leituras
    let mut sobra = 0usize;
    loop {
        let lidos = match saida.read(&mut bruto[sobra..]) {
            Ok(0) | Err(_) => return,
            Ok(n) => n,
        };
        let total = sobra + lidos;
        let inteiros = total / 4 * 4;
        let bloco: Vec<f32> = bruto[..inteiros]
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        bruto.copy_within(inteiros..total, 0);
        sobra = total - inteiros;
        if !bloco.is_empty() && tx.send(bloco).is_err() {
            return;
        }
    }
}

impl Drop for FonteFfmpeg {
    fn drop(&mut self) {
        self.encerrar();
    }
}

impl Iterator for FonteFfmpeg {
    type Item = Sample;

    fn next(&mut self) -> Option<Sample> {
        if self.pos >= self.bloco.len() {
            match self.rx.try_recv() {
                Ok(b) => {
                    self.bloco = b;
                    self.pos = 0;
                }
                // Decodificação atrasou (ou acabou de vir um seek): silêncio
                // em vez de travar a thread de áudio inteira
                Err(TryRecvError::Empty) => return Some(0.0),
                Err(TryRecvError::Disconnected) => return None,
            }
        }
        let amostra = self.bloco[self.pos];
        self.pos += 1;
        Some(amostra as Sample)
    }
}

impl Source for FonteFfmpeg {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZero::new(CANAIS).expect("CANAIS > 0")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZero::new(TAXA).expect("TAXA > 0")
    }

    fn total_duration(&self) -> Option<Duration> {
        self.duracao
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.encerrar();
        let (filho, rx) = iniciar(&self.caminho, pos).ok_or_else(|| {
            SeekError::Other(std::sync::Arc::new(std::io::Error::other(
                "ffmpeg não abriu",
            )))
        })?;
        self.filho = filho;
        self.rx = rx;
        self.bloco.clear();
        self.pos = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WebM com Opus (o caso que o symphonia não abre), gerado pelo ffmpeg.
    fn webm_de_teste(nome: &str, segundos: u32) -> Option<PathBuf> {
        if !ffmpeg::disponivel() {
            eprintln!("sem ffmpeg: teste pulado");
            return None;
        }
        let caminho =
            std::env::temp_dir().join(format!("furinar-teste-{nome}-{}.webm", std::process::id()));
        let ok = ffmpeg::comando("ffmpeg")
            .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
            .arg(format!("sine=frequency=440:duration={segundos}"))
            .args(["-c:a", "libopus"])
            .arg(&caminho)
            .status()
            .is_ok_and(|s| s.success());
        ok.then_some(caminho)
    }

    /// Amostras até o fim, esperando os blocos (o `next` devolve silêncio
    /// quando o canal está momentaneamente vazio).
    fn contar_ate_o_fim(f: &mut FonteFfmpeg) -> usize {
        let mut n = 0;
        while f.next().is_some() {
            n += 1;
            if f.pos >= f.bloco.len() {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        n
    }

    #[test]
    fn webm_opus_toca_inteiro_e_faz_seek() {
        let Some(caminho) = webm_de_teste("opus", 2) else {
            return;
        };
        let mut f = FonteFfmpeg::abrir(&caminho).expect("webm deve abrir");
        assert_eq!(f.channels().get(), 2);
        let duracao = f.total_duration().unwrap().as_secs_f64();
        assert!((duracao - 2.0).abs() < 0.1, "duração {duracao}");

        // Tolerância: silêncio de espera entre blocos entra na conta
        let amostras = contar_ate_o_fim(&mut f) as f64 / (TAXA * CANAIS as u32) as f64;
        assert!(amostras >= 1.9, "tocou só {amostras} s");

        f.try_seek(Duration::from_millis(1500)).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        let resto = contar_ate_o_fim(&mut f) as f64 / (TAXA * CANAIS as u32) as f64;
        assert!(resto < 1.0, "depois do seek tocou {resto} s");
        let _ = std::fs::remove_file(caminho);
    }

    #[test]
    fn arquivo_sem_audio_nao_abre() {
        if !ffmpeg::disponivel() {
            return;
        }
        let caminho =
            std::env::temp_dir().join(format!("furinar-teste-vazio-{}.webm", std::process::id()));
        std::fs::write(&caminho, b"nada aqui").unwrap();
        assert!(FonteFfmpeg::abrir(&caminho).is_none());
        let _ = std::fs::remove_file(caminho);
    }
}
