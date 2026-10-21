//! Letras do LRCLIB (lrclib.net): grátis, sem chave, e já em LRC.
//!
//! Letra sincronizada vira um `.lrc` ao lado da faixa (outros players também
//! leem); se a pasta não aceitar escrita, vai para o cache do Furinar. Letra só
//! com texto vai sempre para o cache (`.txt`): um `.lrc` sem tempos confundiria
//! os outros players.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::metadados::{Consulta, pasta_cache, proximidade_duracao, semelhanca};
use crate::rede::{Cliente, Erro};

const HOST: &str = "lrclib.net";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Registro {
    #[serde(default)]
    track_name: String,
    #[serde(default)]
    artist_name: String,
    #[serde(default)]
    duration: f64,
    #[serde(default)]
    instrumental: bool,
    #[serde(default)]
    plain_lyrics: Option<String>,
    #[serde(default)]
    synced_lyrics: Option<String>,
}

/// O que a busca de letra conseguiu.
#[derive(Debug, PartialEq, Eq)]
pub enum Resultado {
    /// Gravada neste arquivo.
    Salva {
        caminho: PathBuf,
        sincronizada: bool,
    },
    /// A faixa é instrumental (não adianta procurar de novo).
    Instrumental,
    NaoEncontrada,
}

/// Procura a letra e grava. Erro só quando a rede falhou (vale tentar
/// depois); "não tem" é `NaoEncontrada`.
pub fn buscar(cliente: &mut Cliente, consulta: &Consulta) -> Result<Resultado, Erro> {
    let (titulo, artista) = consulta.termos();
    let Some(artista) = artista else {
        // Sem artista o LRCLIB acerta pouco: só a busca aberta
        return pela_busca(cliente, consulta, &titulo, None);
    };

    let mut query = vec![
        ("track_name", titulo.clone()),
        ("artist_name", artista.clone()),
    ];
    if let Some(album) = &consulta.album {
        query.push(("album_name", album.clone()));
    }
    if consulta.duracao > 0 {
        query.push(("duration", consulta.duracao.to_string()));
    }
    let exato: Option<Registro> = cliente.json(HOST, "https://lrclib.net/api/get", &query)?;
    match exato {
        Some(r) => Ok(gravar(consulta, &r)),
        None => pela_busca(cliente, consulta, &titulo, Some(&artista)),
    }
}

fn pela_busca(
    cliente: &mut Cliente,
    consulta: &Consulta,
    titulo: &str,
    artista: Option<&str>,
) -> Result<Resultado, Erro> {
    let mut query = vec![("track_name", titulo.to_string())];
    if let Some(a) = artista {
        query.push(("artist_name", a.to_string()));
    }
    let lista: Vec<Registro> = cliente
        .json(HOST, "https://lrclib.net/api/search", &query)?
        .unwrap_or_default();
    Ok(match melhor(consulta, &lista) {
        Some(r) => gravar(consulta, r),
        None => Resultado::NaoEncontrada,
    })
}

/// O registro que mais parece a faixa, se algum parece o bastante. Com letra
/// sincronizada ganha um bônus: é o que o Furinar sabe mostrar melhor.
fn melhor<'a>(consulta: &Consulta, lista: &'a [Registro]) -> Option<&'a Registro> {
    let (titulo, artista) = consulta.termos();
    lista
        .iter()
        .map(|r| {
            let mut nota = 0.5 * semelhanca(&titulo, &r.track_name)
                + 0.5 * proximidade_duracao(consulta.duracao, r.duration.round() as u32);
            if let Some(a) = &artista {
                nota = 0.6 * nota + 0.4 * semelhanca(a, &r.artist_name);
            }
            if r.synced_lyrics.is_some() {
                nota += 0.05;
            }
            (nota, r)
        })
        .filter(|(nota, _)| *nota >= 0.7)
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, r)| r)
}

fn gravar(consulta: &Consulta, r: &Registro) -> Resultado {
    if r.instrumental {
        return Resultado::Instrumental;
    }
    if let Some(lrc) = r.synced_lyrics.as_deref().filter(|s| !s.trim().is_empty()) {
        let ao_lado = crate::lrc::caminho_lrc(&consulta.caminho);
        if std::fs::write(&ao_lado, lrc).is_ok() {
            return Resultado::Salva {
                caminho: ao_lado,
                sincronizada: true,
            };
        }
        if let Some(c) = no_cache(&consulta.chave, "lrc", lrc) {
            return Resultado::Salva {
                caminho: c,
                sincronizada: true,
            };
        }
    }
    if let Some(texto) = r.plain_lyrics.as_deref().filter(|s| !s.trim().is_empty())
        && let Some(c) = no_cache(&consulta.chave, "txt", texto)
    {
        return Resultado::Salva {
            caminho: c,
            sincronizada: false,
        };
    }
    Resultado::NaoEncontrada
}

/// Arquivo da letra no cache: nome pelo hash da chave da faixa.
pub fn caminho_cache(chave: &str, extensao: &str) -> Option<PathBuf> {
    let nome = crate::miniaturas::hash_texto(chave);
    pasta_cache("letras").map(|p| p.join(format!("{nome}.{extensao}")))
}

fn no_cache(chave: &str, extensao: &str, conteudo: &str) -> Option<PathBuf> {
    let caminho = caminho_cache(chave, extensao)?;
    std::fs::create_dir_all(caminho.parent()?).ok()?;
    std::fs::write(&caminho, conteudo).ok()?;
    Some(caminho)
}

/// Letra baixada antes e guardada no cache (para faixas cuja pasta não aceita
/// escrita, ou só com texto).
pub fn do_cache(chave: &str) -> Option<crate::lrc::Letra> {
    let ler = |ext| std::fs::read_to_string(caminho_cache(chave, ext)?).ok();
    if let Some(lrc) = ler("lrc") {
        return Some(crate::lrc::Letra::Sincronizada(crate::lrc::parse_lrc(&lrc)));
    }
    ler("txt").map(|t| crate::lrc::letra_do_texto(&t))
}

/// Existe letra local para a faixa (.lrc, embutida ou do cache)?
pub fn tem_letra_local(caminho: &Path, chave: &str) -> bool {
    crate::lrc::caminho_lrc(caminho).is_file()
        || caminho_cache(chave, "lrc").is_some_and(|c| c.is_file())
        || caminho_cache(chave, "txt").is_some_and(|c| c.is_file())
        || !crate::lrc::letra_embutida(caminho).vazia()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn consulta(duracao: u32) -> Consulta {
        Consulta {
            chave: "/m/a.mp3".into(),
            caminho: "/m/a.mp3".into(),
            titulo: "Apocalypse Now".into(),
            artista: Some("PinocchioP".into()),
            album: None,
            duracao,
        }
    }

    #[test]
    fn le_o_json_do_lrclib() {
        let json = r#"[{"id":9082038,"trackName":"Apocalypse Now","artistName":"PinocchioP",
            "albumName":"Apocalypse Now","duration":177.954,"instrumental":false,
            "plainLyrics":"a\nb","syncedLyrics":"[00:05.83] a\n[00:07.47] b"}]"#;
        let lista: Vec<Registro> = serde_json::from_str(json).unwrap();
        assert_eq!(lista[0].duration, 177.954);
        assert!(lista[0].synced_lyrics.is_some());
        assert!(melhor(&consulta(178), &lista).is_some());
    }

    #[test]
    fn escolhe_pela_duracao_e_recusa_o_que_nao_parece() {
        let r = |nome: &str, artista: &str, duracao: f64| Registro {
            track_name: nome.into(),
            artist_name: artista.into(),
            duration: duracao,
            instrumental: false,
            plain_lyrics: Some("x".into()),
            synced_lyrics: None,
        };
        let lista = [
            r("Apocalypse Now", "PinocchioP", 300.0),
            r("Apocalypse Now", "PinocchioP", 178.0),
        ];
        assert_eq!(
            melhor(&consulta(178), &lista).map(|r| r.duration),
            Some(178.0)
        );
        let outra = [r("Outra Música", "Outra Banda", 178.0)];
        assert!(melhor(&consulta(178), &outra).is_none());
    }
}
