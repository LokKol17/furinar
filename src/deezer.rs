//! Capa de uma faixa no Deezer, para o status do Discord (que só aceita
//! imagem por URL pública). API aberta, sem chave; só é chamada com o status
//! do Discord ligado.

use crate::rede::{Cliente, Erro, Tipo};
use crate::track::normalizar_busca;
use serde_json::Value;

const HOST: &str = "api.deezer.com";
const BUSCA: &str = "https://api.deezer.com/search";

/// URL da capa (500×500) da faixa, se o Deezer tiver uma que confira.
pub fn capa(
    cliente: &mut Cliente,
    titulo: &str,
    artista: Option<&str>,
    duracao: u64,
) -> Result<Option<String>, Erro> {
    let limpo = |s: &str| s.replace('"', " ");
    let mut buscas = Vec::new();
    match artista {
        Some(a) => {
            buscas.push(format!(
                "artist:\"{}\" track:\"{}\"",
                limpo(a),
                limpo(titulo)
            ));
            buscas.push(format!("{} {}", limpo(a), limpo(titulo)));
        }
        None => buscas.push(limpo(titulo)),
    }
    for q in buscas {
        let resposta: Option<Value> =
            cliente.json(HOST, BUSCA, &[("q", q), ("limit", "10".into())])?;
        let Some(resposta) = resposta else { continue };
        if let Some(erro) = resposta.get("error") {
            // Cota estourada etc.: erro, para não guardar "sem capa"
            return Err(Erro::novo("Deezer", Tipo::Recusa(erro.to_string())));
        }
        if let Some(url) = escolher(&resposta, titulo, artista, duracao) {
            return Ok(Some(url));
        }
    }
    Ok(None)
}

/// O resultado que mais confere com a faixa. Título tem que bater (igual ou
/// um contendo o outro: "Loveit" × "Loveit feat. Hatsune Miku"), e o artista
/// também — sem tag de artista, ele precisa aparecer no título (nome de
/// arquivo "Artista - Título"). Duração muito diferente descarta.
fn escolher(resposta: &Value, titulo: &str, artista: Option<&str>, duracao: u64) -> Option<String> {
    let titulo = normalizar_busca(titulo);
    let artista = artista.map(normalizar_busca);
    let contem =
        |a: &str, b: &str| !a.is_empty() && !b.is_empty() && (a.contains(b) || b.contains(a));

    resposta
        .get("data")?
        .as_array()?
        .iter()
        .filter_map(|item| {
            let t = normalizar_busca(item.get("title")?.as_str()?);
            let a = normalizar_busca(item.get("artist")?.get("name")?.as_str()?);
            let capa = item.get("album")?.get("cover_big")?.as_str()?;
            if capa.is_empty() || !contem(&t, &titulo) {
                return None;
            }
            let artista_ok = match &artista {
                Some(artista) => contem(&a, artista),
                None => titulo.contains(&a),
            };
            if !artista_ok {
                return None;
            }
            let mut nota = if t == titulo { 3 } else { 2 };
            if let Some(d) = item.get("duration").and_then(Value::as_u64)
                && duracao > 0
            {
                match d.abs_diff(duracao) {
                    0..=5 => nota += 2,
                    6..=15 => {}
                    _ => return None,
                }
            }
            Some((nota, capa.to_string()))
        })
        // Empate: o primeiro (o Deezer já ordena por relevância)
        .fold(None::<(i32, String)>, |melhor, atual| match melhor {
            Some(m) if m.0 >= atual.0 => Some(m),
            _ => Some(atual),
        })
        .map(|(_, capa)| capa)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(titulo: &str, artista: &str, duracao: u64, capa: &str) -> Value {
        serde_json::json!({
            "title": titulo,
            "duration": duracao,
            "artist": { "name": artista },
            "album": { "title": "x", "cover_big": capa }
        })
    }

    fn resposta(itens: Vec<Value>) -> Value {
        serde_json::json!({ "data": itens, "total": 1 })
    }

    #[test]
    fn prefere_titulo_exato_e_duracao_proxima() {
        let r = resposta(vec![
            item("Bohemian Rhapsody (Live)", "Queen", 360, "ao-vivo"),
            item("Bohemian Rhapsody", "Queen", 354, "estudio"),
        ]);
        assert_eq!(
            escolher(&r, "Bohemian Rhapsody", Some("Queen"), 355).as_deref(),
            Some("estudio")
        );
    }

    #[test]
    fn aceita_feat_e_acento_mas_nao_outro_artista() {
        let r = resposta(vec![
            item("Loveit feat. Hatsune Miku", "PinocchioP", 218, "loveit"),
            item("Loveit", "Outra Pessoa", 218, "errada"),
        ]);
        assert_eq!(
            escolher(&r, "Loveit", Some("pinocchiop"), 218).as_deref(),
            Some("loveit")
        );
        let r = resposta(vec![item("Canção", "João", 100, "c")]);
        assert_eq!(
            escolher(&r, "cancao", Some("JOAO"), 0).as_deref(),
            Some("c")
        );
    }

    #[test]
    fn sem_artista_ele_precisa_estar_no_titulo() {
        let r = resposta(vec![item("Mei Mei", "PinocchioP", 234, "mei")]);
        assert_eq!(
            escolher(&r, "PinocchioP - Mei Mei", None, 0).as_deref(),
            Some("mei")
        );
        assert_eq!(escolher(&r, "Mei Mei", None, 0), None);
    }

    #[test]
    fn duracao_muito_diferente_ou_sem_capa_descarta() {
        let r = resposta(vec![item("Faixa", "Banda", 100, "x")]);
        assert_eq!(escolher(&r, "Faixa", Some("Banda"), 300), None);
        let r = resposta(vec![item("Faixa", "Banda", 100, "")]);
        assert_eq!(escolher(&r, "Faixa", Some("Banda"), 100), None);
        assert_eq!(
            escolher(&resposta(vec![]), "Faixa", Some("Banda"), 100),
            None
        );
        assert_eq!(
            escolher(&serde_json::json!({"error": {}}), "Faixa", None, 0),
            None
        );
    }

    /// Ao vivo: `cargo test deezer_ao_vivo -- --ignored --nocapture`.
    #[test]
    #[ignore = "requer rede"]
    fn deezer_ao_vivo() {
        let mut c = Cliente::novo().unwrap();
        for (t, a, d) in [
            ("Bohemian Rhapsody", Some("Queen"), 354),
            ("ぼくらはみんな意味不明", Some("ピノキオピー"), 230),
            ("PinocchioP - Mei Mei", None, 0),
        ] {
            let capa = capa(&mut c, t, a, d);
            println!("{t}: {capa:?}");
            assert!(capa.is_ok());
        }
    }
}
