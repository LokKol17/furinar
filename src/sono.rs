//! Temporizador de sono: pausa depois de N minutos (baixando o volume aos
//! poucos no fim) ou quando a faixa atual acabar.

use std::time::{Duration, Instant};

/// Últimos segundos em que o volume desce até zero antes de pausar.
pub const ESMAECER: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sono {
    Ate { fim: Instant, minutos: u32 },
    FimDaFaixa,
}

/// Próxima opção do botão: desligado → 15 → 30 → 60 min → fim da faixa →
/// desligado.
pub fn proximo(atual: Option<Sono>, agora: Instant) -> Option<Sono> {
    let ate = |minutos: u32| Sono::Ate {
        fim: agora + Duration::from_secs(minutos as u64 * 60),
        minutos,
    };
    match atual {
        None => Some(ate(15)),
        Some(Sono::Ate { minutos: 15, .. }) => Some(ate(30)),
        Some(Sono::Ate { minutos: 30, .. }) => Some(ate(60)),
        Some(Sono::Ate { .. }) => Some(Sono::FimDaFaixa),
        Some(Sono::FimDaFaixa) => None,
    }
}

/// Fração do volume a aplicar agora: 1 até faltar `ESMAECER`, depois desce
/// linearmente até 0.
pub fn fator_volume(fim: Instant, agora: Instant) -> f32 {
    let restante = fim.saturating_duration_since(agora);
    (restante.as_secs_f32() / ESMAECER.as_secs_f32()).min(1.0)
}

/// Texto curto do indicador: tempo restante, ou o rótulo de fim da faixa.
pub fn texto(sono: Option<Sono>, agora: Instant, fim_da_faixa: &str) -> String {
    match sono {
        None => String::new(),
        Some(Sono::FimDaFaixa) => fim_da_faixa.to_string(),
        Some(Sono::Ate { fim, .. }) => {
            // Arredonda para cima: "15:00" logo ao ligar, nunca "14:59"
            let s = fim
                .saturating_duration_since(agora)
                .as_millis()
                .div_ceil(1000) as u64;
            crate::formatar_tempo(s, s >= 3600)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn botao_percorre_as_opcoes_e_volta_ao_desligado() {
        let agora = Instant::now();
        let mut atual = None;
        let mut vistos = Vec::new();
        for _ in 0..5 {
            atual = proximo(atual, agora);
            vistos.push(atual.map(|s| match s {
                Sono::Ate { minutos, .. } => minutos,
                Sono::FimDaFaixa => 0,
            }));
        }
        assert_eq!(vistos, [Some(15), Some(30), Some(60), Some(0), None]);
    }

    #[test]
    fn volume_so_desce_nos_ultimos_segundos() {
        let agora = Instant::now();
        assert_eq!(fator_volume(agora + Duration::from_secs(600), agora), 1.0);
        let meio = fator_volume(agora + ESMAECER / 2, agora);
        assert!((meio - 0.5).abs() < 0.01, "{meio}");
        assert_eq!(fator_volume(agora, agora + Duration::from_secs(1)), 0.0);
    }

    #[test]
    fn texto_mostra_o_restante() {
        let agora = Instant::now();
        let quinze = proximo(None, agora);
        assert_eq!(texto(quinze, agora, "x"), "15:00");
        assert_eq!(
            texto(Some(Sono::FimDaFaixa), agora, "Fim da faixa"),
            "Fim da faixa"
        );
        assert_eq!(texto(None, agora, "x"), "");
    }
}
