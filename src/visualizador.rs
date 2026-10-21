//! Visualizador do modo imersivo: espectro em barras.
//!
//! `Espiao` envolve o decodificador e, só com a captura ligada, copia as
//! amostras (em mono) para um anel compartilhado. A UI tira desse anel a
//! janela mais recente e calcula as barras com uma FFT. Desligado, o espião
//! só repassa as amostras.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Sample, SampleRate, Source};
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;

/// Tamanho da FFT (≈ 43 ms a 48 kHz).
const TAMANHO: usize = 2048;
pub const BARRAS: usize = 48;
const FREQ_MIN: f32 = 40.0;
const FREQ_MAX: f32 = 16_000.0;
/// Faixa dinâmica mostrada: abaixo disso a barra é zero.
const DB_MIN: f32 = -60.0;
/// Quanto a barra cai por tick quando o som baixa (ataque é imediato).
const QUEDA: f32 = 0.86;
/// Amostras acumuladas no espião antes de travar o anel.
const LOTE: usize = 256;

struct Anel {
    dados: Vec<f32>,
    pos: usize,
    taxa: u32,
}

/// Ponte entre a thread de áudio e a UI. Clonar é barato.
#[derive(Clone)]
pub struct Captura {
    ativo: Arc<AtomicBool>,
    anel: Arc<Mutex<Anel>>,
}

impl Default for Captura {
    fn default() -> Self {
        Self {
            ativo: Arc::new(AtomicBool::new(false)),
            anel: Arc::new(Mutex::new(Anel {
                dados: vec![0.0; TAMANHO],
                pos: 0,
                taxa: 44_100,
            })),
        }
    }
}

impl Captura {
    pub fn ligar(&self, ligado: bool) {
        self.ativo.store(ligado, Ordering::Relaxed);
        if !ligado && let Ok(mut anel) = self.anel.lock() {
            anel.dados.fill(0.0);
        }
    }

    /// As `TAMANHO` amostras mais recentes, da mais antiga para a mais nova.
    fn janela(&self) -> Option<(Vec<f32>, u32)> {
        let anel = self.anel.lock().ok()?;
        let mut v = Vec::with_capacity(TAMANHO);
        v.extend_from_slice(&anel.dados[anel.pos..]);
        v.extend_from_slice(&anel.dados[..anel.pos]);
        Some((v, anel.taxa))
    }

    fn empurrar(&self, lote: &[f32], taxa: u32) {
        // try_lock: a thread de áudio nunca espera pela UI; perder um lote
        // só deixa o espectro um tico menos atualizado.
        let Ok(mut anel) = self.anel.try_lock() else {
            return;
        };
        anel.taxa = taxa;
        for &s in lote {
            let pos = anel.pos;
            anel.dados[pos] = s;
            anel.pos = (pos + 1) % TAMANHO;
        }
    }
}

/// `Source` que repassa tudo e, com a captura ligada, copia em mono.
pub struct Espiao<S> {
    fonte: S,
    captura: Captura,
    canal: u16,
    soma: f32,
    lote: Vec<f32>,
}

impl<S: Source> Espiao<S> {
    pub fn new(fonte: S, captura: Captura) -> Self {
        Self {
            fonte,
            captura,
            canal: 0,
            soma: 0.0,
            lote: Vec::with_capacity(LOTE),
        }
    }
}

impl<S: Source> Iterator for Espiao<S> {
    type Item = Sample;

    fn next(&mut self) -> Option<Sample> {
        let amostra = self.fonte.next()?;
        if self.captura.ativo.load(Ordering::Relaxed) {
            let canais = self.fonte.channels().get();
            self.soma += amostra;
            self.canal += 1;
            if self.canal >= canais {
                self.lote.push(self.soma / canais as f32);
                self.soma = 0.0;
                self.canal = 0;
                if self.lote.len() >= LOTE {
                    self.captura
                        .empurrar(&self.lote, self.fonte.sample_rate().get());
                    self.lote.clear();
                }
            }
        }
        Some(amostra)
    }
}

impl<S: Source> Source for Espiao<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.fonte.current_span_len()
    }
    fn channels(&self) -> ChannelCount {
        self.fonte.channels()
    }
    fn sample_rate(&self) -> SampleRate {
        self.fonte.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.fonte.total_duration()
    }
    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        // Trecho novo: o meio-quadro acumulado não vale mais
        self.canal = 0;
        self.soma = 0.0;
        self.fonte.try_seek(pos)
    }
}

/// Calcula as barras e guarda o estado da suavização entre ticks.
pub struct Analisador {
    fft: Arc<dyn rustfft::Fft<f32>>,
    hann: Vec<f32>,
    buffer: Vec<Complex<f32>>,
    barras: [f32; BARRAS],
}

impl Default for Analisador {
    fn default() -> Self {
        let hann = (0..TAMANHO)
            .map(|i| {
                let x = std::f32::consts::TAU * i as f32 / (TAMANHO - 1) as f32;
                0.5 - 0.5 * x.cos()
            })
            .collect();
        Self {
            fft: FftPlanner::new().plan_fft_forward(TAMANHO),
            hann,
            buffer: vec![Complex::default(); TAMANHO],
            barras: [0.0; BARRAS],
        }
    }
}

impl Analisador {
    /// Próximo quadro das barras (0..1). Com `tocando` falso as barras só
    /// caem; devolve `None` quando já estão todas no chão, para a UI não
    /// repintar à toa.
    pub fn quadro(&mut self, captura: &Captura, tocando: bool) -> Option<[f32; BARRAS]> {
        let alvo = if tocando {
            captura
                .janela()
                .map(|(amostras, taxa)| self.espectro(&amostras, taxa))
                .unwrap_or([0.0; BARRAS])
        } else {
            [0.0; BARRAS]
        };

        let antes = self.barras;
        for (b, a) in self.barras.iter_mut().zip(alvo) {
            *b = a.max(*b * QUEDA);
            if *b < 0.01 {
                *b = 0.0;
            }
        }
        (self.barras != antes || tocando).then_some(self.barras)
    }

    fn espectro(&mut self, amostras: &[f32], taxa: u32) -> [f32; BARRAS] {
        for ((c, &s), &w) in self.buffer.iter_mut().zip(amostras).zip(&self.hann) {
            *c = Complex::new(s * w, 0.0);
        }
        self.fft.process(&mut self.buffer);

        // Normaliza para que um seno de amplitude 1 fique em ~0 dB
        let escala = 4.0 / TAMANHO as f32;
        let hz_por_bin = taxa as f32 / TAMANHO as f32;
        let metade = TAMANHO / 2;
        let razao = FREQ_MAX / FREQ_MIN;

        let mut saida = [0.0; BARRAS];
        for (i, barra) in saida.iter_mut().enumerate() {
            let f0 = FREQ_MIN * razao.powf(i as f32 / BARRAS as f32);
            let f1 = FREQ_MIN * razao.powf((i + 1) as f32 / BARRAS as f32);
            let b0 = ((f0 / hz_por_bin) as usize).clamp(1, metade - 1);
            // Bandas graves são mais estreitas que um bin: pega ao menos um
            let b1 = ((f1 / hz_por_bin).ceil() as usize).clamp(b0 + 1, metade);
            let pico = self.buffer[b0..b1]
                .iter()
                .map(|c| c.norm() * escala)
                .fold(0.0f32, f32::max);
            let db = 20.0 * pico.max(1e-9).log10();
            *barra = ((db - DB_MIN) / -DB_MIN).clamp(0.0, 1.0);
        }
        saida
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn captura_com(f: impl Fn(usize) -> f32, taxa: u32) -> Captura {
        let c = Captura::default();
        c.ligar(true);
        let amostras: Vec<f32> = (0..TAMANHO).map(f).collect();
        c.empurrar(&amostras, taxa);
        c
    }

    #[test]
    fn seno_de_1khz_acende_a_banda_certa() {
        let taxa = 48_000;
        let c = captura_com(
            |i| (std::f32::consts::TAU * 1000.0 * i as f32 / taxa as f32).sin(),
            taxa,
        );
        let barras = Analisador::default().quadro(&c, true).unwrap();
        let maior = (0..BARRAS)
            .max_by(|&a, &b| barras[a].total_cmp(&barras[b]))
            .unwrap();
        let razao = FREQ_MAX / FREQ_MIN;
        let f0 = FREQ_MIN * razao.powf(maior as f32 / BARRAS as f32);
        let f1 = FREQ_MIN * razao.powf((maior + 1) as f32 / BARRAS as f32);
        assert!(
            (f0 * 0.8..=f1 * 1.25).contains(&1000.0),
            "banda {maior} = {f0:.0}..{f1:.0} Hz"
        );
        assert!(barras[maior] > 0.8, "pico baixo: {}", barras[maior]);
        // Longe de 1 kHz quase nada
        assert!(barras[0] < 0.2 && barras[BARRAS - 1] < 0.2);
    }

    #[test]
    fn silencio_da_zero_e_para_de_repintar() {
        let c = captura_com(|_| 0.0, 44_100);
        let mut a = Analisador::default();
        assert_eq!(a.quadro(&c, true), Some([0.0; BARRAS]));
        // Pausado e tudo no chão: nada a repintar
        assert_eq!(a.quadro(&c, false), None);
    }

    #[test]
    fn barras_caem_aos_poucos_ao_pausar() {
        let c = captura_com(
            |i| 0.9 * (std::f32::consts::TAU * 500.0 * i as f32 / 44_100.0).sin(),
            44_100,
        );
        let mut a = Analisador::default();
        let tocando = a.quadro(&c, true).unwrap();
        let topo = tocando.iter().copied().fold(0.0f32, f32::max);
        let pausado = a.quadro(&c, false).unwrap();
        let depois = pausado.iter().copied().fold(0.0f32, f32::max);
        assert!(depois > 0.0 && depois < topo, "{topo} -> {depois}");
    }

    #[test]
    fn espiao_desligado_so_repassa() {
        let fonte = rodio::source::SineWave::new(440.0).take_duration(Duration::from_millis(50));
        let captura = Captura::default();
        let antes: Vec<f32> = rodio::source::SineWave::new(440.0)
            .take_duration(Duration::from_millis(50))
            .collect();
        let depois: Vec<f32> = Espiao::new(fonte, captura.clone()).collect();
        assert_eq!(antes, depois);
        let (janela, _) = captura.janela().unwrap();
        assert!(janela.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn espiao_ligado_captura_em_mono() {
        let fonte = rodio::source::SineWave::new(440.0).take_duration(Duration::from_millis(200));
        let captura = Captura::default();
        captura.ligar(true);
        let _: Vec<f32> = Espiao::new(fonte, captura.clone()).collect();
        let (janela, taxa) = captura.janela().unwrap();
        assert_eq!(taxa, 48_000);
        assert!(janela.iter().any(|&s| s.abs() > 0.5));
    }
}
