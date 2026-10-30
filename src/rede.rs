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

/// Falha de um pedido, com o serviço que falhou: a tela diz quem e o que
/// fazer (`descrever`), não só o erro técnico (`Display`, para o log).
#[derive(Debug)]
pub struct Erro {
    /// Nome do serviço para o usuário ("MusicBrainz", "Last.fm"...).
    pub servico: &'static str,
    pub tipo: Tipo,
}

#[derive(Debug)]
pub enum Tipo {
    /// Sem conexão, DNS, TLS...
    SemConexao(String),
    /// Nenhuma resposta dentro do tempo limite (depois das novas tentativas).
    TempoEsgotado,
    /// Resposta HTTP fora de 2xx (depois das novas tentativas), com o motivo
    /// que o serviço mandou no corpo, se mandou ("" se não).
    Status(u16, String),
    /// O serviço respondeu, mas recusou dentro da resposta (Last.fm e
    /// AcoustID mandam o erro no JSON).
    Recusa(String),
    /// Corpo que não é o JSON esperado.
    Formato(String),
}

impl Erro {
    pub fn novo(servico: &'static str, tipo: Tipo) -> Self {
        Self { servico, tipo }
    }

    /// Vale tentar de novo mais tarde (sem rede, servidor sobrecarregado).
    pub fn temporario(&self) -> bool {
        matches!(
            self.tipo,
            Tipo::SemConexao(_) | Tipo::TempoEsgotado | Tipo::Status(429 | 500..=599, _)
        )
    }

    /// Frase para a tela, no idioma de `t` (chaves `net_*`).
    pub fn descrever(&self, t: &dyn Fn(&str) -> String) -> String {
        let motivo = match &self.tipo {
            Tipo::Status(_, m) | Tipo::Recusa(m) => m.as_str(),
            _ => "",
        };
        let mut texto = match &self.tipo {
            Tipo::TempoEsgotado => t("net_timeout"),
            Tipo::SemConexao(_) => t("net_offline"),
            Tipo::Status(429 | 503, _) => t("net_busy"),
            Tipo::Status(401 | 403, _) => t("net_denied"),
            Tipo::Status(n @ 500..=599, _) => t("net_server").replace("{n}", &n.to_string()),
            Tipo::Status(n, m) if m.is_empty() => {
                t("net_refused_code").replace("{n}", &n.to_string())
            }
            Tipo::Status(..) | Tipo::Recusa(_) => t("net_refused").replace("{motivo}", motivo),
            Tipo::Formato(_) => t("net_unexpected"),
        };
        // "invalid API key", "Invalid session key"...: o usuário resolve
        let minusculo = motivo.to_lowercase();
        let ja_diz = matches!(self.tipo, Tipo::Status(401 | 403, _));
        if !ja_diz
            && ["key", "token", "session", "auth"]
                .iter()
                .any(|p| minusculo.contains(p))
        {
            texto.push(' ');
            texto.push_str(&t("net_check_key"));
        }
        texto.replace("{servico}", self.servico)
    }
}

impl std::fmt::Display for Erro {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = self.servico;
        match &self.tipo {
            Tipo::SemConexao(e) => write!(f, "{s}: sem conexão: {e}"),
            Tipo::TempoEsgotado => write!(f, "{s}: tempo esgotado"),
            Tipo::Status(n, m) if m.is_empty() => write!(f, "{s}: HTTP {n}"),
            Tipo::Status(n, m) => write!(f, "{s}: HTTP {n}: {m}"),
            Tipo::Recusa(m) => write!(f, "{s}: recusou: {m}"),
            Tipo::Formato(e) => write!(f, "{s}: resposta inesperada: {e}"),
        }
    }
}

/// Nome do serviço de cada host, para as mensagens.
fn servico(host: &'static str) -> &'static str {
    match host {
        "musicbrainz.org" => "MusicBrainz",
        "coverartarchive.org" => "Cover Art Archive",
        "api.acoustid.org" => "AcoustID",
        "lrclib.net" => "LRCLIB",
        "ws.audioscrobbler.com" => "Last.fm",
        "api.listenbrainz.org" => "ListenBrainz",
        "api.deezer.com" => "Deezer",
        outro => outro,
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

/// O motivo de um erro no corpo da resposta: `{"error":{"message":..}}`
/// (AcoustID), `{"error":".."}` (MusicBrainz, ListenBrainz) ou
/// `{"message":".."}` (Last.fm). Página HTML ou outra coisa: "".
fn motivo(corpo: &str) -> String {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(corpo) else {
        return String::new();
    };
    [&v["error"]["message"], &v["error"], &v["message"]]
        .into_iter()
        .find_map(|m| m.as_str())
        .map(|m| m.chars().take(160).collect())
        .unwrap_or_default()
}

impl Cliente {
    pub fn novo() -> Result<Self, Erro> {
        let http = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(TEMPO_LIMITE)
            .build()
            .map_err(|e| Erro::novo("HTTPS", Tipo::SemConexao(e.to_string())))?;
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
                        // O AcoustID responde 400 a qualquer pedido que recusa;
                        // o porquê vem no corpo
                        let motivo = motivo(&r.text().unwrap_or_default());
                        return Err(Erro::novo(servico(host), Tipo::Status(status, motivo)));
                    }
                }
                Err(e) if e.is_timeout() && tentativa == TENTATIVAS => {
                    return Err(Erro::novo(servico(host), Tipo::TempoEsgotado));
                }
                Err(e) if !e.is_timeout() => {
                    return Err(Erro::novo(servico(host), Tipo::SemConexao(e.to_string())));
                }
                Err(_) => {}
            }
            std::thread::sleep(espera);
            espera *= 2;
        }
        Err(Erro::novo(servico(host), Tipo::TempoEsgotado))
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
            .map(|r| {
                r.json::<T>()
                    .map_err(|e| Erro::novo(servico(host), Tipo::Formato(e.to_string())))
            })
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
            .map(|r| {
                r.json::<T>()
                    .map_err(|e| Erro::novo(servico(host), Tipo::Formato(e.to_string())))
            })
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
            .map(|r| {
                r.json()
                    .map_err(|e| Erro::novo(servico(host), Tipo::Formato(e.to_string())))
            })
            .transpose()
    }

    /// Bytes de um arquivo (capa). `Ok(None)` = não existe.
    pub fn bytes(&mut self, host: &'static str, url: &str) -> Result<Option<Vec<u8>>, Erro> {
        let resposta = self.pedir(host, |c| c.get(url))?;
        resposta
            .map(|r| {
                r.bytes()
                    .map(|b| b.to_vec())
                    .map_err(|e| Erro::novo(servico(host), Tipo::SemConexao(e.to_string())))
            })
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motivo_do_erro_vem_do_corpo_de_cada_servico() {
        assert_eq!(
            motivo(
                r#"{"error": {"code": 3, "message": "invalid fingerprint"}, "status": "error"}"#
            ),
            "invalid fingerprint"
        );
        assert_eq!(motivo(r#"{"error": "Invalid mbid."}"#), "Invalid mbid.");
        assert_eq!(
            motivo(r#"{"error": 10, "message": "Invalid API key"}"#),
            "Invalid API key"
        );
        assert_eq!(motivo("<html>Bad Request</html>"), "");
    }

    /// A frase da tela diz o serviço, o problema e, quando é a chave, onde
    /// mexer.
    #[test]
    fn erro_descrito_com_servico_e_o_que_fazer() {
        let lang = crate::translations::get_translations("pt-br");
        let t = |k: &str| lang.get(k).copied().unwrap_or(k).to_string();
        let chave = Erro::novo("AcoustID", Tipo::Status(400, "invalid API key".into()));
        let texto = chave.descrever(&t);
        assert!(texto.starts_with("AcoustID recusou"), "{texto}");
        assert!(
            texto.contains("invalid API key") && texto.contains("Configurações"),
            "{texto}"
        );
        assert!(!texto.contains('{'), "{texto}");
        let ocupado = Erro::novo("MusicBrainz", Tipo::Status(503, String::new()));
        assert!(ocupado.temporario());
        assert!(ocupado.descrever(&t).contains("MusicBrainz"));
        let sem_rede = Erro::novo("LRCLIB", Tipo::SemConexao("dns".into()));
        assert!(sem_rede.descrever(&t).contains("internet"));
        assert!(!Erro::novo("Last.fm", Tipo::Recusa("x".into())).temporario());
        assert_eq!(chave.to_string(), "AcoustID: HTTP 400: invalid API key");
    }
}
