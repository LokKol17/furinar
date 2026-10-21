//! MusicBrainz (busca de gravações) e Cover Art Archive (capas).
//!
//! Sem chave, mas com limite de 1 pedido por segundo (o `rede::Cliente`
//! respeita). A busca por texto usa título + artista; sem resultado, tenta só
//! o título dentro de uma janela de duração.

use serde::Deserialize;

use crate::metadados::{Candidato, Consulta, Fonte, capa_web, confianca_texto};
use crate::rede::{Cliente, Erro};

const HOST: &str = "musicbrainz.org";
const URL_BUSCA: &str = "https://musicbrainz.org/ws/2/recording";
/// Quantos candidatos a tela de revisão mostra.
const MAX_CANDIDATOS: usize = 8;

#[derive(Debug, Deserialize)]
struct Busca {
    #[serde(default)]
    recordings: Vec<Gravacao>,
}

#[derive(Debug, Deserialize)]
struct Gravacao {
    id: String,
    #[serde(default)]
    title: String,
    /// Milissegundos.
    #[serde(default)]
    length: Option<u64>,
    /// "2001 5.1 mix", "live", "radio edit"... (vazio na versão principal)
    #[serde(default)]
    disambiguation: String,
    /// Primeiro lançamento da gravação: no empate, a mais antiga é a original.
    #[serde(default, rename = "first-release-date")]
    primeira_data: Option<String>,
    #[serde(default, rename = "artist-credit")]
    creditos: Vec<Credito>,
    #[serde(default)]
    releases: Vec<Lancamento>,
}

#[derive(Debug, Deserialize)]
struct Credito {
    #[serde(default)]
    name: String,
    #[serde(default)]
    joinphrase: String,
}

#[derive(Debug, Deserialize)]
struct Lancamento {
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    date: Option<String>,
    #[serde(default, rename = "release-group")]
    grupo: Option<Grupo>,
    #[serde(default)]
    media: Vec<Midia>,
}

#[derive(Debug, Deserialize)]
struct Grupo {
    #[serde(default, rename = "primary-type")]
    tipo: Option<String>,
    #[serde(default, rename = "secondary-types")]
    secundarios: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Midia {
    #[serde(default)]
    position: u32,
    #[serde(default)]
    track: Vec<FaixaMidia>,
}

#[derive(Debug, Deserialize)]
struct FaixaMidia {
    #[serde(default)]
    number: String,
}

/// Aspas e barras escapadas para a query Lucene do MusicBrainz.
fn escapar(texto: &str) -> String {
    texto.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Candidatos para a faixa pela busca por texto, do mais para o menos
/// confiável.
pub fn buscar_texto(cliente: &mut Cliente, consulta: &Consulta) -> Result<Vec<Candidato>, Erro> {
    let (titulo, artista) = consulta.termos();
    if titulo.trim().is_empty() {
        return Ok(Vec::new());
    }
    let gravacao = format!("recording:\"{}\"", escapar(&titulo));
    let do_artista = artista
        .as_ref()
        .map(|a| format!(" AND artist:\"{}\"", escapar(a)));
    // Perto da duração: sem isso, música gravada centenas de vezes (ao vivo,
    // karaokê, remixagens) empurra a original para fora dos resultados
    let da_duracao = (consulta.duracao > 0).then(|| {
        let ms = consulta.duracao as u64 * 1000;
        format!(" AND dur:[{} TO {}]", ms.saturating_sub(5000), ms + 5000)
    });
    let mut queries = Vec::new();
    if let Some(a) = &do_artista {
        if let Some(d) = &da_duracao {
            queries.push(format!("{gravacao}{a}{d}"));
        }
        queries.push(format!("{gravacao}{a}"));
    }
    // Artista grafado diferente (alias, outro alfabeto): só o título
    queries.push(format!("{gravacao}{}", da_duracao.unwrap_or_default()));

    for query in queries {
        let busca: Option<Busca> = cliente.json(
            HOST,
            URL_BUSCA,
            &[
                ("query", query),
                ("fmt", "json".into()),
                ("limit", "25".into()),
            ],
        )?;
        let candidatos = candidatos(
            consulta,
            busca.map(|b| b.recordings).unwrap_or_default(),
            None,
        );
        if !candidatos.is_empty() {
            return Ok(candidatos);
        }
    }
    Ok(Vec::new())
}

/// Detalhes das gravações pelo MBID (vindas do AcoustID), com a confiança que
/// a impressão digital deu.
pub fn buscar_ids(
    cliente: &mut Cliente,
    consulta: &Consulta,
    ids: &[(String, u8)],
) -> Result<Vec<Candidato>, Erro> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let query = ids
        .iter()
        .map(|(id, _)| format!("rid:{id}"))
        .collect::<Vec<_>>()
        .join(" OR ");
    let busca: Option<Busca> = cliente.json(
        HOST,
        URL_BUSCA,
        &[
            ("query", query),
            ("fmt", "json".into()),
            ("limit", ids.len().to_string()),
        ],
    )?;
    Ok(candidatos(
        consulta,
        busca.map(|b| b.recordings).unwrap_or_default(),
        Some(ids),
    ))
}

/// Converte, pontua e ordena. Com `digitais`, a confiança vem da impressão
/// digital (o texto pode estar todo errado, é por isso que ela foi usada).
fn candidatos(
    consulta: &Consulta,
    gravacoes: Vec<Gravacao>,
    digitais: Option<&[(String, u8)]>,
) -> Vec<Candidato> {
    let mut lista: Vec<(u16, Candidato)> = gravacoes
        .into_iter()
        .map(|g| {
            let primeiro_ano = g.primeira_data.as_deref().and_then(ano);
            let artista: String = g
                .creditos
                .iter()
                .map(|c| format!("{}{}", c.name, c.joinphrase))
                .collect();
            let duracao = g.length.map_or(0, |ms| ((ms + 500) / 1000) as u32);
            let lancamento = escolher_lancamento(&g.releases);
            let (confianca, fonte) = match digitais {
                Some(ids) => {
                    let nota = ids
                        .iter()
                        .find(|(id, _)| *id == g.id)
                        .map_or(0, |(_, n)| *n);
                    (nota, Fonte::ImpressaoDigital)
                }
                None => (
                    confianca_texto(consulta, &g.title, &artista, duracao)
                        .saturating_sub(penalidade(lancamento))
                        .saturating_sub(if g.disambiguation.is_empty() { 0 } else { 4 }),
                    Fonte::Texto,
                ),
            };
            let candidato = Candidato {
                gravacao: g.id,
                titulo: g.title,
                artista,
                album: lancamento.map(|l| l.title.clone()),
                lancamento: lancamento.map(|l| l.id.clone()),
                ano: lancamento
                    .and_then(|l| l.date.as_deref())
                    .and_then(ano)
                    .or(primeiro_ano),
                numero: lancamento.map_or(0, numero),
                duracao,
                confianca,
                fonte,
            };
            (primeiro_ano.unwrap_or(9999), candidato)
        })
        .collect();
    // Empate: a gravação mais antiga (a original, não a remixagem de 2001)
    lista.sort_by_key(|(primeiro, c)| (std::cmp::Reverse(c.confianca), *primeiro));
    let mut lista: Vec<Candidato> = lista.into_iter().map(|(_, c)| c).collect();
    // A mesma gravação/álbum repetida não ajuda a escolher
    lista.dedup_by(|a, b| a.titulo == b.titulo && a.artista == b.artista && a.album == b.album);
    lista.truncate(MAX_CANDIDATOS);
    lista
}

/// O lançamento que melhor representa a gravação: oficial, álbum de estúdio
/// (não coletânea nem ao vivo), o mais antigo.
fn escolher_lancamento(lancamentos: &[Lancamento]) -> Option<&Lancamento> {
    lancamentos.iter().min_by_key(|l| {
        let oficial = l.status.as_deref() == Some("Official");
        let tipo = l.grupo.as_ref().and_then(|g| g.tipo.as_deref());
        let secundario = l.grupo.as_ref().is_some_and(|g| !g.secundarios.is_empty());
        let peso_tipo = match tipo {
            Some("Album") if !secundario => 0,
            Some("EP") => 1,
            Some("Single") => 2,
            _ => 3,
        };
        let data = l
            .date
            .clone()
            .filter(|d| !d.is_empty())
            .unwrap_or_else(|| "9999".into());
        (!oficial, peso_tipo, data)
    })
}

/// Ao vivo, coletânea, remix: o nome bate, mas raramente é o arquivo que a
/// pessoa tem. Perde um pouco para a versão de estúdio.
fn penalidade(lancamento: Option<&Lancamento>) -> u8 {
    let Some(grupo) = lancamento.and_then(|l| l.grupo.as_ref()) else {
        return 3;
    };
    if grupo
        .secundarios
        .iter()
        .any(|s| matches!(s.as_str(), "Live" | "Compilation" | "Remix" | "DJ-mix"))
    {
        5
    } else {
        0
    }
}

fn ano(data: &str) -> Option<u16> {
    data.get(..4)?.parse().ok().filter(|&a| a > 0)
}

/// Disco × 1000 + faixa, como no scan.
fn numero(l: &Lancamento) -> u32 {
    l.media
        .iter()
        .find_map(|m| {
            let faixa: u32 = m.track.first()?.number.trim().parse().ok()?;
            Some(m.position.min(999) * 1000 + faixa.min(999))
        })
        .unwrap_or(0)
}

/// Baixa a capa frontal do lançamento para o cache. `true` se ela existe
/// (agora ou de antes).
pub fn baixar_capa(cliente: &mut Cliente, lancamento: &str) -> Result<bool, Erro> {
    let Some(destino) = capa_web(lancamento) else {
        return Ok(false);
    };
    if destino.is_file() {
        return Ok(true);
    }
    let url = format!("https://coverartarchive.org/release/{lancamento}/front-500");
    let Some(bytes) = cliente.bytes("coverartarchive.org", &url)? else {
        return Ok(false);
    };
    // Guarda sempre em JPEG (é o que o nome promete e o que vai para as tags)
    let Ok(img) = image::load_from_memory(&bytes) else {
        return Ok(false);
    };
    let Some(pasta) = destino.parent() else {
        return Ok(false);
    };
    let _ = std::fs::create_dir_all(pasta);
    let mut saida = Vec::new();
    let codificador = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut saida, 90);
    if img.to_rgb8().write_with_encoder(codificador).is_err() {
        return Ok(false);
    }
    Ok(std::fs::write(&destino, saida).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Trecho de uma resposta real do MusicBrainz.
    const BUSCA: &str = r#"{"recordings":[
        {"id":"df3543df-dd02-4415-a92b-15ee3e3da451","score":100,"title":"Bohemian Rhapsody",
         "length":356000,"artist-credit":[{"name":"Queen","joinphrase":""}],
         "releases":[
           {"id":"c1","title":"Greatest Hits","status":"Official","date":"1981",
            "release-group":{"primary-type":"Album","secondary-types":["Compilation"]},
            "media":[{"position":1,"track":[{"number":"1"}]}]},
           {"id":"96e5dc55-fa8b-483e-a673-e04e1f4cfd2b","title":"A Night at the Opera","status":"Official",
            "date":"1975-11-21","release-group":{"primary-type":"Album"},
            "media":[{"position":1,"track":[{"number":"11"}]}]}]},
        {"id":"x2","title":"Bohemian Rhapsody","length":200000,
         "artist-credit":[{"name":"Cover Band","joinphrase":" feat. "},{"name":"Fulano","joinphrase":""}],
         "releases":[]}]}"#;

    fn consulta() -> Consulta {
        Consulta {
            chave: "/m/a.mp3".into(),
            caminho: "/m/a.mp3".into(),
            titulo: "Bohemian Rhapsody".into(),
            artista: Some("Queen".into()),
            album: None,
            duracao: 355,
        }
    }

    #[test]
    fn le_a_busca_e_escolhe_o_album_original() {
        let busca: Busca = serde_json::from_str(BUSCA).unwrap();
        let lista = candidatos(&consulta(), busca.recordings, None);
        let primeiro = &lista[0];
        assert_eq!(primeiro.artista, "Queen");
        // A coletânea perde para o álbum de estúdio
        assert_eq!(primeiro.album.as_deref(), Some("A Night at the Opera"));
        assert_eq!(primeiro.ano, Some(1975));
        assert_eq!(primeiro.numero, 1011);
        assert_eq!(primeiro.duracao, 356);
        assert!(primeiro.confianca >= crate::metadados::CONFIANCA_AUTOMATICA);
        // Créditos juntam artistas com a frase de ligação
        assert_eq!(lista[1].artista, "Cover Band feat. Fulano");
        assert!(lista[1].confianca < primeiro.confianca);
    }

    #[test]
    fn com_impressao_digital_a_nota_e_dela() {
        let busca: Busca = serde_json::from_str(BUSCA).unwrap();
        let ids = [("x2".to_string(), 97u8)];
        let lista = candidatos(&consulta(), busca.recordings, Some(&ids));
        assert_eq!(lista[0].gravacao, "x2");
        assert_eq!(lista[0].confianca, 97);
        assert_eq!(lista[0].fonte, Fonte::ImpressaoDigital);
    }

    #[test]
    fn escapa_aspas_da_query() {
        assert_eq!(escapar(r#"Diz "oi""#), r#"Diz \"oi\""#);
    }
}
