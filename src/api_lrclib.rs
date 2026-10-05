//! Cliente para a API do LRCLIB (lrclib.net)
//!
//! Busca letras sincronizadas por metadados da faixa.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct LrcLibResult {
    #[serde(default, rename = "trackName")]
    pub track_name: String,
    #[serde(default, rename = "artistName")]
    pub artist_name: String,
    #[serde(default, rename = "albumName")]
    pub album_name: Option<String>,
    #[serde(default)]
    pub duration: f64,
    #[serde(default, rename = "syncedLyrics")]
    pub synced_lyrics: Option<String>,
    #[serde(default, rename = "plainLyrics")]
    pub plain_lyrics: Option<String>,
    #[serde(default)]
    pub instrumental: bool,
}

pub fn get_lyrics(
    track_name: &str,
    artist_name: &str,
    album_name: Option<&str>,
    duration_secs: f64,
) -> Result<Option<LrcLibResult>, Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(format!(
            "Furinar/{} ( https://github.com/LokKol17/furinar )",
            env!("CARGO_PKG_VERSION")
        ))
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    if let Some(album) = album_name {
        let result = do_lookup(&client, track_name, artist_name, Some(album), duration_secs)?;
        if result.is_some() {
            return Ok(result);
        }
    }

    do_lookup(&client, track_name, artist_name, None, duration_secs)
}

fn do_lookup(
    client: &reqwest::blocking::Client,
    track_name: &str,
    artist_name: &str,
    album_name: Option<&str>,
    duration_secs: f64,
) -> Result<Option<LrcLibResult>, Box<dyn std::error::Error>> {
    let mut url = reqwest::Url::parse("https://lrclib.net/api/get")?;

    {
        let mut query = url.query_pairs_mut();
        query.append_pair("track_name", track_name);
        query.append_pair("artist_name", artist_name);
        query.append_pair("duration", &format!("{:.0}", duration_secs));
        if let Some(album) = album_name {
            query.append_pair("album_name", album);
        }
    }

    let response = client.get(url.as_str()).send()?;

    match response.status().as_u16() {
        200 => {
            let result: LrcLibResult = response.json()?;
            Ok(Some(result))
        }
        404 => Ok(None),
        429 => {
            std::thread::sleep(std::time::Duration::from_secs(2));
            let response2 = client.get(url.as_str()).send()?;
            if response2.status().is_success() {
                let result: LrcLibResult = response2.json()?;
                Ok(Some(result))
            } else {
                Ok(None)
            }
        }
        status => Err(format!("LRCLIB retornou status {}", status).into()),
    }
}
