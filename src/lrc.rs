use std::path::PathBuf;

use slint::SharedString;

/// Quantas linhas mandamos pra UI ao redor da linha atual. Ímpar, para a
/// linha atual poder ficar centralizada.
pub const LETRA_JANELA: usize = 9;

/// Caminho do .lrc irmão da faixa: mesmo nome, extensão trocada.
pub fn caminho_lrc(caminho_faixa: &std::path::Path) -> PathBuf {
    caminho_faixa.with_extension("lrc")
}

/// Converte "mm:ss", "mm:ss.xx" ou "mm:ss.xxx" em segundos.
pub fn parse_tempo(dentro: &str) -> Option<f64> {
    let (minutos, resto) = dentro.split_once(':')?;
    let minutos: u64 = minutos.trim().parse().ok()?;

    let (segundos, fracao) = match resto.split_once('.') {
        Some((s, f)) => (s, Some(f)),
        None => (resto, None),
    };
    let segundos: u64 = segundos.trim().parse().ok()?;

    let fracao = match fracao {
        None => 0.0,
        Some(f) => {
            let f = f.trim();
            // 1 a 3 dígitos: ".5", ".50" e ".500" valem 500 ms
            if f.is_empty() || f.len() > 3 || !f.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let valor: u64 = f.parse().ok()?;
            match f.len() {
                1 => valor as f64 / 10.0,
                2 => valor as f64 / 100.0,
                _ => valor as f64 / 1000.0,
            }
        }
    };

    Some(minutos as f64 * 60.0 + segundos as f64 + fracao)
}

/// Parseia um .lrc. Linhas fora do padrão `[tempo]texto` são ignoradas em
/// silêncio, e uma linha com vários timestamps gera uma entrada por timestamp.
pub fn parse_lrc(conteudo: &str) -> Vec<(f64, String)> {
    let mut linhas: Vec<(f64, String)> = Vec::new();

    for linha in conteudo.lines() {
        let mut resto = linha;
        let mut tempos: Vec<f64> = Vec::new();

        // Consome todos os `[tempo]` no começo da linha
        while let Some(fim) = resto.find(']') {
            if !resto.starts_with('[') {
                break;
            }
            match parse_tempo(&resto[1..fim]) {
                Some(t) => tempos.push(t),
                // Metadado ([ar:...], [ti:...]) ou lixo: a linha não serve
                None => break,
            }
            resto = &resto[fim + 1..];
        }

        if tempos.is_empty() {
            continue;
        }

        let texto = resto.trim().to_string();
        for t in tempos {
            linhas.push((t, texto.clone()));
        }
    }

    linhas.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    linhas
}

/// Índice da última linha cujo tempo é ≤ `tempo` (-1 se ainda não chegou).
pub fn indice_letra(letra: &[(f64, String)], tempo: f64) -> i32 {
    let n = letra.partition_point(|(t, _)| *t <= tempo);
    if n == 0 { -1 } else { (n - 1) as i32 }
}

/// Recorte de linhas ao redor da linha atual. Devolve (início, linhas, destaque).
/// A janela fica centrada na linha atual, exceto perto das pontas — é isso
/// que dá a sensação de rolagem em vez de um texto trocando no lugar.
pub fn janela_letra(letra: &[(f64, String)], indice: i32) -> (usize, Vec<SharedString>, i32) {
    if letra.is_empty() {
        return (0, Vec::new(), -1);
    }

    let inicio = if indice < 0 {
        0
    } else {
        let atual = indice as usize;
        let metade = LETRA_JANELA / 2;
        if atual < metade {
            0
        } else if atual + metade + 1 > letra.len() {
            letra.len().saturating_sub(LETRA_JANELA)
        } else {
            atual - metade
        }
    };

    let fim = (inicio + LETRA_JANELA).min(letra.len());
    let linhas = letra[inicio..fim]
        .iter()
        .map(|(_, t)| t.as_str().into())
        .collect();
    let destaque = if indice < 0 {
        -1
    } else {
        indice - inicio as i32
    };

    (inicio, linhas, destaque)
}
