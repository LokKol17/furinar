//! Ganho e esmaecimento por faixa, aplicados amostra a amostra.
//!
//! `Transicao` envolve o decodificador: multiplica pelo ReplayGain, faz o
//! fade-in do crossfade e, quando o `Controle` pede, o fade-out até acabar a
//! fonte. Feito na thread de áudio porque o timer da UI (250 ms) deixaria
//! degraus audíveis no volume.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Sample, SampleRate, Source};

/// Pede o fade-out de uma faixa que já está tocando. Clonar é barato.
#[derive(Clone, Default)]
pub struct Controle(Arc<AtomicU32>);

impl Controle {
    /// Some com o som nos próximos `duracao` e encerra a fonte.
    pub fn esmaecer(&self, duracao: Duration) {
        let ms = duracao.as_millis().clamp(1, u32::MAX as u128) as u32;
        self.0.store(ms, Ordering::Relaxed);
    }
}

pub struct Transicao<S> {
    fonte: S,
    ganho: f32,
    /// Fade-in: duração pedida e amostras (contando canais) já passadas.
    entrada: Duration,
    entrada_total: u64,
    entrada_feita: u64,
    controle: Controle,
    saida_total: u64,
    saida_restante: u64,
}

impl<S: Source> Transicao<S> {
    pub fn new(fonte: S, ganho: f32, entrada: Duration, controle: Controle) -> Self {
        Self {
            fonte,
            ganho,
            entrada,
            entrada_total: 0,
            entrada_feita: 0,
            controle,
            saida_total: 0,
            saida_restante: 0,
        }
    }

    fn amostras(&self, duracao: Duration) -> u64 {
        let por_segundo =
            self.fonte.sample_rate().get() as u64 * self.fonte.channels().get() as u64;
        ((duracao.as_secs_f64() * por_segundo as f64) as u64).max(1)
    }
}

impl<S: Source> Iterator for Transicao<S> {
    type Item = Sample;

    fn next(&mut self) -> Option<Sample> {
        if self.saida_total == 0 {
            let ms = self.controle.0.load(Ordering::Relaxed);
            if ms > 0 {
                self.saida_total = self.amostras(Duration::from_millis(ms as u64));
                self.saida_restante = self.saida_total;
            }
        }
        if self.saida_total > 0 {
            if self.saida_restante == 0 {
                return None;
            }
            self.saida_restante -= 1;
        }

        let amostra = self.fonte.next()?;
        let mut g = self.ganho;
        if !self.entrada.is_zero() {
            if self.entrada_total == 0 {
                self.entrada_total = self.amostras(self.entrada);
            }
            if self.entrada_feita < self.entrada_total {
                g *= self.entrada_feita as f32 / self.entrada_total as f32;
                self.entrada_feita += 1;
            }
        }
        if self.saida_total > 0 {
            g *= self.saida_restante as f32 / self.saida_total as f32;
        }
        Some(amostra * g)
    }
}

impl<S: Source> Source for Transicao<S> {
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
        // Seek no meio do fade-in: entra direto no volume cheio
        self.entrada = Duration::ZERO;
        self.fonte.try_seek(pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::buffer::SamplesBuffer;
    use std::num::NonZero;

    /// 1 s de amostras 1.0 em mono a 1 kHz: a amostra n sai como o ganho em n.
    fn uns() -> SamplesBuffer {
        SamplesBuffer::new(
            NonZero::new(1).unwrap(),
            NonZero::new(1000).unwrap(),
            vec![1.0; 1000],
        )
    }

    #[test]
    fn ganho_multiplica_tudo() {
        let saida: Vec<f32> =
            Transicao::new(uns(), 0.5, Duration::ZERO, Controle::default()).collect();
        assert_eq!(saida.len(), 1000);
        assert!(saida.iter().all(|&s| (s - 0.5).abs() < 1e-6));
    }

    #[test]
    fn fade_in_sobe_do_zero() {
        let saida: Vec<f32> =
            Transicao::new(uns(), 1.0, Duration::from_millis(100), Controle::default()).collect();
        assert_eq!(saida[0], 0.0);
        assert!((saida[50] - 0.5).abs() < 0.02, "{}", saida[50]);
        assert_eq!(saida[200], 1.0);
    }

    #[test]
    fn fade_out_pedido_encerra_a_fonte() {
        let controle = Controle::default();
        let mut t = Transicao::new(uns(), 1.0, Duration::ZERO, controle.clone());
        let antes: Vec<f32> = t.by_ref().take(100).collect();
        assert!(antes.iter().all(|&s| s == 1.0));
        controle.esmaecer(Duration::from_millis(200));
        let depois: Vec<f32> = t.collect();
        // Acaba em 200 amostras, não nas 900 que sobravam
        assert_eq!(depois.len(), 200);
        assert!(depois[0] > 0.99 && depois[199] < 0.01);
        assert!(depois.windows(2).all(|w| w[1] <= w[0]));
    }
}
