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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caminho_lrc_troca_a_extensao_da_faixa() {
        assert_eq!(
            caminho_lrc(std::path::Path::new("/musica/a.mp3")),
            PathBuf::from("/musica/a.lrc")
        );
        assert_eq!(
            caminho_lrc(std::path::Path::new("sem_extensao")),
            PathBuf::from("sem_extensao.lrc")
        );
    }

    #[test]
    fn parse_tempo_aceita_os_formatos_de_lrc() {
        assert_eq!(parse_tempo("01:30"), Some(90.0));
        assert_eq!(parse_tempo("0:07"), Some(7.0));
        assert_eq!(parse_tempo("01:30.5"), Some(90.5));
        assert_eq!(parse_tempo("01:30.50"), Some(90.5));
        assert_eq!(parse_tempo("01:30.500"), Some(90.5));
        assert_eq!(parse_tempo("12:34.007"), Some(754.007));
    }

    #[test]
    fn parse_tempo_rejeita_lixo() {
        assert_eq!(parse_tempo(""), None);
        assert_eq!(parse_tempo("1:2.1234"), None, "mais de 3 dígitos de fração");
        assert_eq!(parse_tempo("1:2."), None, "fração vazia");
        assert_eq!(parse_tempo("1:2.1a"), None, "fração não numérica");
        assert_eq!(parse_tempo("abc"), None, "sem ':'");
        assert_eq!(parse_tempo("aa:10"), None);
        assert_eq!(parse_tempo("1:bb"), None);
    }

    #[test]
    fn parse_lrc_ignora_metadados_e_ordena_por_tempo() {
        let conteudo = "\
[ar:Artista]
[ti:Título]
[00:50.00]Segunda
[00:10.00]Primeira
[xx]lixo
sem colchete
";
        let letra = parse_lrc(conteudo);

        assert_eq!(
            letra,
            vec![
                (10.0, "Primeira".to_string()),
                (50.0, "Segunda".to_string())
            ]
        );
    }

    #[test]
    fn parse_lrc_gera_uma_entrada_por_timestamp() {
        let letra = parse_lrc("[00:10.00][00:50.00]Refrão\n[01:00]Fim");

        assert_eq!(letra.len(), 3);
        assert_eq!(letra[0], (10.0, "Refrão".to_string()));
        assert_eq!(letra[1], (50.0, "Refrão".to_string()));
        assert_eq!(letra[2], (60.0, "Fim".to_string()));
    }

    #[test]
    fn indice_letra_aponta_para_a_ultima_linha_ja_passada() {
        let letra = vec![
            (10.0, "a".to_string()),
            (20.0, "b".to_string()),
            (30.0, "c".to_string()),
        ];

        assert_eq!(indice_letra(&letra, 0.0), -1, "antes da primeira linha");
        assert_eq!(indice_letra(&letra, 10.0), 0, "no tempo exato da linha");
        assert_eq!(indice_letra(&letra, 25.0), 1);
        assert_eq!(indice_letra(&letra, 99.0), 2, "depois da última");
        assert_eq!(indice_letra(&[], 10.0), -1);
    }

    #[test]
    fn janela_letra_centraliza_e_rola_nas_pontas() {
        let letra: Vec<(f64, String)> = (0..20).map(|i| (i as f64, i.to_string())).collect();

        // Começo: não há como centralizar, a lista começa em 0
        let (inicio, linhas, destaque) = janela_letra(&letra, 0);
        assert_eq!((inicio, destaque), (0, 0));
        assert_eq!(linhas.len(), LETRA_JANELA);
        assert_eq!(linhas.first().map(|s| s.as_str()), Some("0"));

        // Meio: linha atual no centro da janela
        let (inicio, _, destaque) = janela_letra(&letra, 10);
        assert_eq!(inicio, 10 - LETRA_JANELA / 2);
        assert_eq!(destaque, (LETRA_JANELA / 2) as i32);

        // Fim: a última linha fica visível e destacada
        let (inicio, linhas, destaque) = janela_letra(&letra, 19);
        assert_eq!(inicio, 20 - LETRA_JANELA);
        assert_eq!(destaque, (LETRA_JANELA - 1) as i32);
        assert_eq!(linhas.last().map(|s| s.as_str()), Some("19"));

        // Sem linha atual: só rola, sem destaque
        let (inicio, _, destaque) = janela_letra(&letra, -1);
        assert_eq!((inicio, destaque), (0, -1));

        let (inicio, linhas, destaque) = janela_letra(&[], 3);
        assert_eq!((inicio, destaque), (0, -1));
        assert!(linhas.is_empty());
    }
}
