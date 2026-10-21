//! Scrobble: avisa ListenBrainz e Last.fm do que está tocando ("tocando
//! agora") e de cada faixa ouvida (a mesma regra de "Mais tocadas": metade da
//! faixa ou 4 minutos).
//!
//! Roda na thread do buscador (`buscador::Pedido::Scrobble`). Escuta que não
//! foi por falta de rede volta para uma fila (`furinar_scrobbles.json`) e vai
//! junto no próximo envio.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::rede::{Cliente, Erro};

const LISTENBRAINZ: &str = "https://api.listenbrainz.org/1/submit-listens";
const LASTFM: &str = "https://ws.audioscrobbler.com/2.0/";

/// Uma faixa ouvida (ou começando a tocar).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Escuta {
    pub artista: String,
    pub titulo: String,
    #[serde(default)]
    pub album: Option<String>,
    /// Segundos (0 = desconhecida).
    #[serde(default)]
    pub duracao: u64,
    /// Quando começou a tocar (segundos desde 1970).
    pub quando: u64,
    /// MBID da gravação, se a faixa foi identificada pela web.
    #[serde(default)]
    pub gravacao: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Servico {
    ListenBrainz,
    LastFm,
}

/// Credenciais de cada serviço (vazio = desligado).
#[derive(Clone, Debug, Default)]
pub struct Contas {
    pub listenbrainz: String,
    pub lastfm_chave: String,
    pub lastfm_segredo: String,
    pub lastfm_sessao: String,
}

impl Contas {
    pub fn alguma(&self) -> bool {
        !self.listenbrainz.is_empty() || self.lastfm_pronto()
    }

    fn lastfm_pronto(&self) -> bool {
        !self.lastfm_chave.is_empty()
            && !self.lastfm_segredo.is_empty()
            && !self.lastfm_sessao.is_empty()
    }

    pub fn servicos(&self) -> Vec<Servico> {
        let mut v = Vec::new();
        if !self.listenbrainz.is_empty() {
            v.push(Servico::ListenBrainz);
        }
        if self.lastfm_pronto() {
            v.push(Servico::LastFm);
        }
        v
    }
}

/// O que ainda não foi enviado.
pub type Fila = Vec<(Servico, Escuta)>;

fn caminho_fila() -> PathBuf {
    crate::config::config_path().with_file_name("furinar_scrobbles.json")
}

pub fn carregar_fila() -> Fila {
    fs::read_to_string(caminho_fila())
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}

pub fn salvar_fila(fila: &Fila) {
    let caminho = caminho_fila();
    if fila.is_empty() {
        let _ = fs::remove_file(caminho);
    } else if let Ok(json) = serde_json::to_string(fila) {
        let _ = fs::write(caminho, json);
    }
}

/// Faixa curta demais não conta (regra do Last.fm, vale para os dois).
pub fn conta(escuta: &Escuta) -> bool {
    escuta.duracao == 0 || escuta.duracao > 30
}

/// "Tocando agora" em todos os serviços (falha não importa: é passageiro).
pub fn tocando_agora(cliente: &mut Cliente, contas: &Contas, escuta: &Escuta) {
    for servico in contas.servicos() {
        let _ = match servico {
            Servico::ListenBrainz => listenbrainz(cliente, contas, "playing_now", &[escuta]),
            Servico::LastFm => lastfm_faixa(cliente, contas, "track.updateNowPlaying", escuta),
        };
    }
}

/// Envia as escutas (a fila antiga primeiro). Devolve o que não foi por erro
/// de rede (vai para a fila) e a última mensagem de erro de outro tipo
/// (token inválido, por exemplo: essas não adianta repetir).
pub fn enviar(cliente: &mut Cliente, contas: &Contas, fila: Fila) -> (Fila, Option<String>) {
    let mut sobrou = Vec::new();
    let mut erro = None;
    for (servico, escuta) in fila {
        let ativo = contas.servicos().contains(&servico);
        if !ativo {
            continue;
        }
        let resultado = match servico {
            Servico::ListenBrainz => listenbrainz(cliente, contas, "single", &[&escuta]),
            Servico::LastFm => lastfm_faixa(cliente, contas, "track.scrobble", &escuta),
        };
        match resultado {
            Ok(()) => {}
            // Sem rede ou servidor sobrecarregado: tenta de novo depois
            Err(e @ (Erro::Rede(_) | Erro::Status(429 | 500..=599))) => {
                erro = Some(e.to_string());
                sobrou.push((servico, escuta));
            }
            Err(e) => erro = Some(e.to_string()),
        }
    }
    (sobrou, erro)
}

fn listenbrainz(
    cliente: &mut Cliente,
    contas: &Contas,
    tipo: &str,
    escutas: &[&Escuta],
) -> Result<(), Erro> {
    let payload: Vec<serde_json::Value> = escutas
        .iter()
        .map(|e| {
            let mut info = json!({
                "submission_client": "Furinar",
                "submission_client_version": env!("CARGO_PKG_VERSION"),
            });
            if e.duracao > 0 {
                info["duration_ms"] = json!(e.duracao * 1000);
            }
            if let Some(g) = &e.gravacao {
                info["recording_mbid"] = json!(g);
            }
            let mut faixa = json!({
                "artist_name": e.artista,
                "track_name": e.titulo,
                "additional_info": info,
            });
            if let Some(album) = &e.album {
                faixa["release_name"] = json!(album);
            }
            let mut item = json!({ "track_metadata": faixa });
            // "playing_now" não leva horário
            if tipo == "single" {
                item["listened_at"] = json!(e.quando);
            }
            item
        })
        .collect();
    let corpo = json!({ "listen_type": tipo, "payload": payload });
    let autorizacao = format!("Token {}", contas.listenbrainz.trim());
    cliente
        .post_json(
            "api.listenbrainz.org",
            LISTENBRAINZ,
            &[("Authorization", autorizacao)],
            &corpo,
        )
        .map(|_| ())
}

/// Assinatura do Last.fm: parâmetros em ordem alfabética, nome+valor
/// colados, mais o segredo, em MD5.
pub fn assinar(parametros: &BTreeMap<&str, String>, segredo: &str) -> String {
    let mut base = String::new();
    for (k, v) in parametros {
        base.push_str(k);
        base.push_str(v);
    }
    base.push_str(segredo);
    format!("{:x}", md5::compute(base.as_bytes()))
}

/// Chamada assinada ao Last.fm (POST, resposta em JSON).
fn lastfm(
    cliente: &mut Cliente,
    segredo: &str,
    mut parametros: BTreeMap<&str, String>,
) -> Result<serde_json::Value, Erro> {
    let assinatura = assinar(&parametros, segredo);
    parametros.insert("api_sig", assinatura);
    parametros.insert("format", "json".into());
    let formulario: Vec<(&str, String)> = parametros.into_iter().collect();
    let resposta: Option<serde_json::Value> =
        cliente.json_post("ws.audioscrobbler.com", LASTFM, &formulario)?;
    let resposta = resposta.ok_or(Erro::Status(404))?;
    if let Some(msg) = resposta.get("message").and_then(|m| m.as_str())
        && resposta.get("error").is_some()
    {
        return Err(Erro::Formato(format!("Last.fm: {msg}")));
    }
    Ok(resposta)
}

fn lastfm_faixa(
    cliente: &mut Cliente,
    contas: &Contas,
    metodo: &'static str,
    e: &Escuta,
) -> Result<(), Erro> {
    let mut p = BTreeMap::new();
    p.insert("method", metodo.to_string());
    p.insert("api_key", contas.lastfm_chave.trim().to_string());
    p.insert("sk", contas.lastfm_sessao.clone());
    p.insert("artist", e.artista.clone());
    p.insert("track", e.titulo.clone());
    if let Some(album) = &e.album {
        p.insert("album", album.clone());
    }
    if e.duracao > 0 {
        p.insert("duration", e.duracao.to_string());
    }
    if metodo == "track.scrobble" {
        p.insert("timestamp", e.quando.to_string());
    }
    if let Some(g) = &e.gravacao {
        p.insert("mbid", g.clone());
    }
    lastfm(cliente, contas.lastfm_segredo.trim(), p).map(|_| ())
}

/// Primeiro passo da conexão: um token, e a página para o usuário autorizar.
pub fn lastfm_token(
    cliente: &mut Cliente,
    chave: &str,
    segredo: &str,
) -> Result<(String, String), Erro> {
    let mut p = BTreeMap::new();
    p.insert("method", "auth.getToken".to_string());
    p.insert("api_key", chave.trim().to_string());
    let r = lastfm(cliente, segredo.trim(), p)?;
    let token = r
        .get("token")
        .and_then(|t| t.as_str())
        .ok_or_else(|| Erro::Formato("sem token".into()))?
        .to_string();
    let url = format!(
        "https://www.last.fm/api/auth/?api_key={}&token={token}",
        chave.trim()
    );
    Ok((token, url))
}

/// Segundo passo, depois da autorização: a sessão (não expira) e o usuário.
pub fn lastfm_sessao(
    cliente: &mut Cliente,
    chave: &str,
    segredo: &str,
    token: &str,
) -> Result<(String, String), Erro> {
    let mut p = BTreeMap::new();
    p.insert("method", "auth.getSession".to_string());
    p.insert("api_key", chave.trim().to_string());
    p.insert("token", token.to_string());
    let r = lastfm(cliente, segredo.trim(), p)?;
    let sessao = &r["session"];
    match (sessao["key"].as_str(), sessao["name"].as_str()) {
        (Some(k), Some(n)) => Ok((k.to_string(), n.to_string())),
        _ => Err(Erro::Formato("sem sessão".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assinatura_do_lastfm_segue_a_documentacao() {
        // Parâmetros em ordem alfabética + segredo, em MD5
        let mut p = BTreeMap::new();
        p.insert("method", "auth.getToken".to_string());
        p.insert("api_key", "abc".to_string());
        let esperado = format!("{:x}", md5::compute("api_keyabcmethodauth.getTokensegredo"));
        assert_eq!(assinar(&p, "segredo"), esperado);
    }

    #[test]
    fn so_servicos_configurados_recebem() {
        let mut c = Contas::default();
        assert!(!c.alguma());
        c.listenbrainz = "t".into();
        assert_eq!(c.servicos(), [Servico::ListenBrainz]);
        // Last.fm só com chave, segredo e sessão
        c.lastfm_chave = "k".into();
        c.lastfm_segredo = "s".into();
        assert_eq!(c.servicos(), [Servico::ListenBrainz]);
        c.lastfm_sessao = "sk".into();
        assert_eq!(c.servicos(), [Servico::ListenBrainz, Servico::LastFm]);
    }

    #[test]
    fn faixa_curta_nao_conta() {
        let mut e = Escuta {
            artista: "a".into(),
            titulo: "t".into(),
            album: None,
            duracao: 25,
            quando: 0,
            gravacao: None,
        };
        assert!(!conta(&e));
        e.duracao = 31;
        assert!(conta(&e));
    }
}
