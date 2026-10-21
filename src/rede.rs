//! Cliente HTTP da busca na web (letras, metadados, capas).
//!
//! Bloqueante: só roda na thread do buscador. Respeita o limite de cada
//! serviço (o MusicBrainz bloqueia quem passa de 1 pedido por segundo) e tenta
//! de novo, com espera, quando o servidor responde 503/429.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;

/// O MusicBrainz exige um User-Agent que identifique o app e um contato.
const USER_AGENT: &str = concat!(
    "Furinar/",
    env!("CARGO_PKG_VERSION"),
    " ( https://github.com/LokKol17/furinar )"
);
const TEMPO_LIMITE: Duration = Duration::from_secs(15);
const TENTATIVAS: u32 = 4;

#[derive(Debug)]
pub enum Erro {
    /// Sem conexão, DNS, tempo esgotado...
    Rede(String),
    /// Resposta HTTP fora de 2xx (depois das novas tentativas).
    Status(u16),
    /// Corpo que não é o JSON esperado.
    Formato(String),
}

impl std::fmt::Display for Erro {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Erro::Rede(e) => write!(f, "rede: {e}"),
            Erro::Status(s) => write!(f, "HTTP {s}"),
            Erro::Formato(e) => write!(f, "resposta inesperada: {e}"),
        }
    }
}

pub struct Cliente {
    http: reqwest::blocking::Client,
    /// Último pedido feito a cada host, para o intervalo mínimo.
    ultimo: HashMap<&'static str, Instant>,
}

/// Intervalo mínimo entre pedidos ao mesmo host.
fn intervalo(host: &str) -> Duration {
    match host {
        // Regra do MusicBrainz: 1/s (com folga)
        "musicbrainz.org" => Duration::from_millis(1100),
        // AcoustID: até 3/s
        "api.acoustid.org" => Duration::from_millis(350),
        // Last.fm pede no máximo ~5/s
        "ws.audioscrobbler.com" => Duration::from_millis(250),
        _ => Duration::from_millis(200),
    }
}

impl Cliente {
    pub fn novo() -> Result<Self, Erro> {
        let http = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(TEMPO_LIMITE)
            .build()
            .map_err(|e| Erro::Rede(e.to_string()))?;
        Ok(Self {
            http,
            ultimo: HashMap::new(),
        })
    }

    fn esperar_vez(&mut self, host: &'static str) {
        if let Some(antes) = self.ultimo.get(host) {
            let falta = intervalo(host).saturating_sub(antes.elapsed());
            if !falta.is_zero() {
                std::thread::sleep(falta);
            }
        }
        self.ultimo.insert(host, Instant::now());
    }

    /// Faz o pedido com novas tentativas. `Ok(None)` = 404 (não existe).
    fn pedir(
        &mut self,
        host: &'static str,
        montar: impl Fn(&reqwest::blocking::Client) -> reqwest::blocking::RequestBuilder,
    ) -> Result<Option<reqwest::blocking::Response>, Erro> {
        let mut espera = Duration::from_secs(2);
        for tentativa in 1..=TENTATIVAS {
            self.esperar_vez(host);
            let resposta = montar(&self.http).send();
            match resposta {
                Ok(r) if r.status().is_success() => return Ok(Some(r)),
                Ok(r) if r.status().as_u16() == 404 => return Ok(None),
                Ok(r) => {
                    let status = r.status().as_u16();
                    // Sobrecarga ou limite: espera e tenta de novo
                    if !matches!(status, 429 | 500 | 502 | 503 | 504) || tentativa == TENTATIVAS {
                        return Err(Erro::Status(status));
                    }
                }
                Err(e) if tentativa == TENTATIVAS || !e.is_timeout() => {
                    return Err(Erro::Rede(e.to_string()));
                }
                Err(_) => {}
            }
            std::thread::sleep(espera);
            espera *= 2;
        }
        Err(Erro::Rede("sem resposta".into()))
    }

    /// GET com query; `Ok(None)` quando o recurso não existe (404).
    pub fn json<T: DeserializeOwned>(
        &mut self,
        host: &'static str,
        url: &str,
        query: &[(&str, String)],
    ) -> Result<Option<T>, Erro> {
        let resposta = self.pedir(host, |c| {
            c.get(url)
                .query(query)
                .header(reqwest::header::ACCEPT, "application/json")
        })?;
        resposta
            .map(|r| r.json::<T>().map_err(|e| Erro::Formato(e.to_string())))
            .transpose()
    }

    /// POST de formulário com resposta em JSON (a impressão digital não cabe
    /// numa URL).
    pub fn json_post<T: DeserializeOwned>(
        &mut self,
        host: &'static str,
        url: &str,
        formulario: &[(&str, String)],
    ) -> Result<Option<T>, Erro> {
        let resposta = self.pedir(host, |c| c.post(url).form(formulario))?;
        resposta
            .map(|r| r.json::<T>().map_err(|e| Erro::Formato(e.to_string())))
            .transpose()
    }

    /// POST com corpo JSON e cabeçalhos extras (token do ListenBrainz).
    pub fn post_json(
        &mut self,
        host: &'static str,
        url: &str,
        cabecalhos: &[(&str, String)],
        corpo: &serde_json::Value,
    ) -> Result<Option<serde_json::Value>, Erro> {
        let resposta = self.pedir(host, |c| {
            let mut r = c.post(url).json(corpo);
            for (k, v) in cabecalhos {
                r = r.header(*k, v);
            }
            r
        })?;
        resposta
            .map(|r| r.json().map_err(|e| Erro::Formato(e.to_string())))
            .transpose()
    }

    /// Bytes de um arquivo (capa). `Ok(None)` = não existe.
    pub fn bytes(&mut self, host: &'static str, url: &str) -> Result<Option<Vec<u8>>, Erro> {
        let resposta = self.pedir(host, |c| c.get(url))?;
        resposta
            .map(|r| {
                r.bytes()
                    .map(|b| b.to_vec())
                    .map_err(|e| Erro::Rede(e.to_string()))
            })
            .transpose()
    }
}
