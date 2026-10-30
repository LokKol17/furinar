//! Thread da busca na web. Recebe pedidos (letra, identificar faixa, lote da
//! aba, capa, gravar tags) e devolve respostas por canal, lidas no timer de
//! 250 ms da UI.
//!
//! Só existe enquanto há trabalho: o `Buscador` é criado no primeiro pedido e
//! solto depois de um tempo ocioso, levando junto o cliente HTTP e seu
//! runtime (alguns MB).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use crate::letras_web;
use crate::metadados::{CONFIANCA_AUTOMATICA, Candidato, Consulta, Correcao};
use crate::rede::{Cliente, Erro};
use crate::{acoustid, deezer, musicbrainz, scrobble};

pub enum Pedido {
    /// Letra de uma faixa: ao tocar (`automatica`) ou pelo botão.
    Letra {
        consulta: Consulta,
        automatica: bool,
    },
    /// Candidatos para uma faixa (tela de informações).
    Identificar(Consulta),
    /// A aba inteira: identifica cada faixa e, com `letras`, baixa as letras
    /// que faltam.
    Lote {
        consultas: Vec<Consulta>,
        letras: bool,
    },
    /// Capa de um lançamento para o cache.
    Capa { chave: String, lancamento: String },
    /// URL de capa no Deezer para o status do Discord.
    CapaDiscord {
        chave: String,
        titulo: String,
        artista: Option<String>,
        duracao: u64,
    },
    /// Escreve as correções nos arquivos.
    GravarTags(Vec<(PathBuf, Correcao)>),
    /// Avisa os serviços de scrobble que a faixa começou.
    TocandoAgora(scrobble::Contas, scrobble::Escuta),
    /// Envia escutas (a fila pendente junto).
    Scrobble(scrobble::Contas, scrobble::Fila),
    /// Conexão com o Last.fm: token + página de autorização, depois a sessão.
    LastfmToken { chave: String, segredo: String },
    LastfmSessao {
        chave: String,
        segredo: String,
        token: String,
    },
}

pub enum Resposta {
    Letra {
        chave: String,
        resultado: Result<letras_web::Resultado, Erro>,
    },
    Candidatos {
        chave: String,
        lista: Vec<Candidato>,
        erro: Option<Erro>,
    },
    LoteProgresso {
        feitas: usize,
        total: usize,
    },
    LoteItem {
        consulta: Consulta,
        candidatos: Vec<Candidato>,
        letra: Option<letras_web::Resultado>,
    },
    LoteFim {
        cancelado: bool,
        erro: Option<Erro>,
    },
    Capa {
        chave: String,
        ok: bool,
    },
    /// `Err` = falhou (rede, cota): tentar de novo depois.
    CapaDiscord {
        chave: String,
        url: Result<Option<String>, Erro>,
    },
    Gravadas {
        ok: usize,
        falhas: Vec<(PathBuf, crate::metadados::FalhaTags)>,
    },
    TocandoAgora,
    /// O que não foi (volta para a fila) e o último erro.
    Scrobble {
        sobrou: scrobble::Fila,
        erro: Option<Erro>,
    },
    /// (token, URL para autorizar)
    LastfmToken(Result<(String, String), Erro>),
    /// (sessão, usuário)
    LastfmSessao(Result<(String, String), Erro>),
}

impl Resposta {
    /// Encerra um pedido (as outras são parciais do lote).
    fn final_de_pedido(&self) -> bool {
        !matches!(
            self,
            Resposta::LoteProgresso { .. } | Resposta::LoteItem { .. }
        )
    }
}

pub struct Buscador {
    tx: Sender<Pedido>,
    rx: Receiver<Resposta>,
    cancelar: Arc<AtomicBool>,
    /// Pedidos sem resposta final ainda.
    pendentes: usize,
    ocioso_desde: Instant,
}

/// Solto depois de tanto tempo sem trabalho.
pub const OCIOSO: Duration = Duration::from_secs(60);

impl Buscador {
    /// `chave_acoustid`: sem ela, só busca por texto.
    pub fn novo(chave_acoustid: Option<String>) -> Self {
        let (tx, rx_pedidos) = mpsc::channel();
        let (tx_respostas, rx) = mpsc::channel();
        let cancelar = Arc::new(AtomicBool::new(false));
        let cancelar_thread = cancelar.clone();
        let _ = std::thread::Builder::new()
            .name("furinar-web".into())
            .spawn(move || trabalhar(&rx_pedidos, &tx_respostas, &cancelar_thread, chave_acoustid));
        Self {
            tx,
            rx,
            cancelar,
            pendentes: 0,
            ocioso_desde: Instant::now(),
        }
    }

    pub fn pedir(&mut self, pedido: Pedido) {
        if matches!(pedido, Pedido::Lote { .. }) {
            self.cancelar.store(false, Ordering::Relaxed);
        }
        if self.tx.send(pedido).is_ok() {
            self.pendentes += 1;
        }
    }

    /// Para o lote em andamento (a faixa atual termina antes).
    pub fn cancelar_lote(&self) {
        self.cancelar.store(true, Ordering::Relaxed);
    }

    pub fn respostas(&mut self) -> Vec<Resposta> {
        let lista: Vec<Resposta> = self.rx.try_iter().collect();
        for r in &lista {
            if r.final_de_pedido() {
                self.pendentes = self.pendentes.saturating_sub(1);
                self.ocioso_desde = Instant::now();
            }
        }
        lista
    }

    /// Sem nenhum pedido em andamento (pode ser trocado por um novo).
    pub fn dispensavel_ja(&self) -> bool {
        self.pendentes == 0
    }

    /// Sem trabalho há tempo bastante para ser solto.
    pub fn dispensavel(&self) -> bool {
        self.pendentes == 0 && self.ocioso_desde.elapsed() >= OCIOSO
    }
}

fn trabalhar(
    rx: &Receiver<Pedido>,
    tx: &Sender<Resposta>,
    cancelar: &AtomicBool,
    chave_acoustid: Option<String>,
) {
    // O cliente só nasce no primeiro pedido que precisa de rede
    let mut conexao: Option<Cliente> = None;
    let chave = chave_acoustid.as_deref();

    for pedido in rx.iter() {
        let resposta = match pedido {
            Pedido::Letra {
                consulta,
                automatica,
            } => Resposta::Letra {
                chave: consulta.chave.clone(),
                resultado: conectar(&mut conexao)
                    .and_then(|c| buscar_letra(c, &consulta, automatica, chave)),
            },
            Pedido::Identificar(consulta) => {
                let (lista, erro) = match conectar(&mut conexao) {
                    Ok(c) => identificar(c, &consulta, chave),
                    Err(e) => (Vec::new(), Some(e)),
                };
                Resposta::Candidatos {
                    chave: consulta.chave,
                    lista,
                    erro,
                }
            }
            Pedido::Lote { consultas, letras } => match conectar(&mut conexao) {
                Ok(c) => lote(c, consultas, letras, chave, cancelar, tx),
                Err(e) => Resposta::LoteFim {
                    cancelado: false,
                    erro: Some(e),
                },
            },
            Pedido::CapaDiscord {
                chave,
                titulo,
                artista,
                duracao,
            } => Resposta::CapaDiscord {
                url: conectar(&mut conexao)
                    .and_then(|c| deezer::capa(c, &titulo, artista.as_deref(), duracao)),
                chave,
            },
            Pedido::Capa { chave, lancamento } => Resposta::Capa {
                ok: conectar(&mut conexao)
                    .ok()
                    .is_some_and(|c| musicbrainz::baixar_capa(c, &lancamento).unwrap_or(false)),
                chave,
            },
            Pedido::GravarTags(itens) => {
                let mut ok = 0;
                let mut falhas = Vec::new();
                for (caminho, correcao) in itens {
                    match crate::metadados::gravar_tags(&caminho, &correcao) {
                        Ok(()) => ok += 1,
                        Err(e) => falhas.push((caminho, e)),
                    }
                }
                Resposta::Gravadas { ok, falhas }
            }
            Pedido::TocandoAgora(contas, escuta) => {
                if let Ok(c) = conectar(&mut conexao) {
                    scrobble::tocando_agora(c, &contas, &escuta);
                }
                Resposta::TocandoAgora
            }
            Pedido::Scrobble(contas, fila) => match conectar(&mut conexao) {
                Ok(c) => {
                    let (sobrou, erro) = scrobble::enviar(c, &contas, fila);
                    Resposta::Scrobble { sobrou, erro }
                }
                Err(e) => Resposta::Scrobble {
                    sobrou: fila,
                    erro: Some(e),
                },
            },
            Pedido::LastfmToken { chave, segredo } => Resposta::LastfmToken(
                conectar(&mut conexao).and_then(|c| scrobble::lastfm_token(c, &chave, &segredo)),
            ),
            Pedido::LastfmSessao {
                chave,
                segredo,
                token,
            } => Resposta::LastfmSessao(
                conectar(&mut conexao)
                    .and_then(|c| scrobble::lastfm_sessao(c, &chave, &segredo, &token)),
            ),
        };
        if tx.send(resposta).is_err() {
            return;
        }
    }
}

fn conectar(conexao: &mut Option<Cliente>) -> Result<&mut Cliente, Erro> {
    let cliente = match conexao.take() {
        Some(c) => c,
        None => Cliente::novo()?,
    };
    Ok(conexao.insert(cliente))
}

/// Letra no LRCLIB. Na busca automática, faixa sem artista (nem na tag, nem
/// no nome do arquivo) ou sem duração só vai se a impressão digital disser
/// com confiança o que ela é: com título genérico ("Intro", "Faixa 1") a busca
/// aberta acharia a letra de outra música. O botão busca sempre.
fn buscar_letra(
    cliente: &mut Cliente,
    consulta: &Consulta,
    automatica: bool,
    chave_acoustid: Option<&str>,
) -> Result<letras_web::Resultado, Erro> {
    if !automatica || !letra_duvidosa(consulta) {
        return letras_web::buscar(cliente, consulta);
    }
    let Some(chave) = chave_acoustid else {
        return Ok(letras_web::Resultado::NaoEncontrada);
    };
    let ids = acoustid::consultar(cliente, chave, &consulta.caminho)?;
    let identificada = musicbrainz::buscar_ids(cliente, consulta, &ids)?
        .into_iter()
        .next()
        .filter(|c| c.confianca >= CONFIANCA_AUTOMATICA);
    let Some(c) = identificada else {
        return Ok(letras_web::Resultado::NaoEncontrada);
    };
    let mut certa = consulta.clone();
    certa.titulo = c.titulo;
    certa.artista = Some(c.artista);
    certa.album = c.album;
    if certa.duracao == 0 {
        certa.duracao = c.duracao;
    }
    letras_web::buscar(cliente, &certa)
}

/// Pouco para o LRCLIB acertar sozinho: sem artista ou sem duração.
fn letra_duvidosa(consulta: &Consulta) -> bool {
    consulta.termos().1.is_none() || consulta.duracao == 0
}

/// Texto primeiro; se não bastar e houver chave, a impressão digital. As
/// duas listas se juntam, pela confiança.
fn identificar(
    cliente: &mut Cliente,
    consulta: &Consulta,
    chave_acoustid: Option<&str>,
) -> (Vec<Candidato>, Option<Erro>) {
    let mut erro = None;
    let mut lista = musicbrainz::buscar_texto(cliente, consulta).unwrap_or_else(|e| {
        erro = Some(e);
        Vec::new()
    });
    let bastou = lista
        .first()
        .is_some_and(|c| c.confianca >= CONFIANCA_AUTOMATICA);
    if !bastou && let Some(chave) = chave_acoustid {
        let digitais = acoustid::consultar(cliente, chave, &consulta.caminho)
            .and_then(|ids| musicbrainz::buscar_ids(cliente, consulta, &ids));
        match digitais {
            Ok(d) => {
                lista.extend(d);
                lista.sort_by_key(|c| std::cmp::Reverse(c.confianca));
                lista.dedup_by(|a, b| a.gravacao == b.gravacao);
                erro = None;
            }
            Err(e) => erro = erro.or(Some(e)),
        }
    }
    if !lista.is_empty() {
        erro = None;
    }
    (lista, erro)
}

fn lote(
    cliente: &mut Cliente,
    consultas: Vec<Consulta>,
    letras: bool,
    chave_acoustid: Option<&str>,
    cancelar: &AtomicBool,
    tx: &Sender<Resposta>,
) -> Resposta {
    let total = consultas.len();
    let mut erro = None;
    for (i, consulta) in consultas.into_iter().enumerate() {
        if cancelar.load(Ordering::Relaxed) {
            return Resposta::LoteFim {
                cancelado: true,
                erro,
            };
        }
        let _ = tx.send(Resposta::LoteProgresso { feitas: i, total });
        let (candidatos, e) = identificar(cliente, &consulta, chave_acoustid);
        if e.is_some() {
            erro = e;
        }
        let aceito = candidatos
            .first()
            .filter(|c| c.confianca >= CONFIANCA_AUTOMATICA);
        if let Some(lancamento) = aceito.and_then(|c| c.lancamento.as_deref()) {
            let _ = musicbrainz::baixar_capa(cliente, lancamento);
        }
        let letra = (letras && !letras_web::tem_letra_local(&consulta.caminho, &consulta.chave))
            .then(|| {
                // Com a identificação aceita, pergunta pelos nomes certos
                let mut c = consulta.clone();
                if let Some(a) = aceito {
                    c.titulo = a.titulo.clone();
                    c.artista = Some(a.artista.clone());
                    c.album = a.album.clone();
                }
                letras_web::buscar(cliente, &c).ok()
            })
            .flatten();
        if tx
            .send(Resposta::LoteItem {
                consulta,
                candidatos,
                letra,
            })
            .is_err()
        {
            break;
        }
    }
    Resposta::LoteFim {
        cancelado: false,
        erro,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letra_automatica_sem_artista_ou_duracao_e_duvidosa() {
        let c = |titulo: &str, artista: Option<&str>, duracao| Consulta {
            chave: "/m/a.mp3".into(),
            caminho: "/m/a.mp3".into(),
            titulo: titulo.into(),
            artista: artista.map(Into::into),
            album: None,
            duracao,
        };
        assert!(!letra_duvidosa(&c("Vento", Some("Fulano"), 200)));
        // O artista do nome do arquivo conta
        assert!(!letra_duvidosa(&c("Fulano - Vento", None, 200)));
        assert!(letra_duvidosa(&c("Intro", None, 200)));
        assert!(letra_duvidosa(&c("Vento", Some("Fulano"), 0)));
    }

    /// Contra os serviços de verdade (precisa de internet). Manual:
    /// `cargo test web_ao_vivo -- --ignored --nocapture`
    #[test]
    #[ignore = "precisa de internet"]
    fn web_ao_vivo() {
        let pasta = std::env::temp_dir().join(format!("furinar-teste-web-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let consulta = Consulta {
            chave: pasta.join("x.mp3").to_string_lossy().to_string(),
            caminho: pasta.join("x.mp3"),
            titulo: "Bohemian Rhapsody".into(),
            artista: Some("Queen".into()),
            album: None,
            duracao: 355,
        };
        let mut cliente = Cliente::novo().unwrap();
        let (lista, erro) = identificar(&mut cliente, &consulta, None);
        println!("erro: {erro:?}");
        for c in &lista {
            println!(
                "{:3}% {} — {} · {:?} ({:?}) #{}",
                c.confianca, c.titulo, c.artista, c.album, c.ano, c.numero
            );
        }
        assert!(
            lista
                .first()
                .is_some_and(|c| c.confianca >= CONFIANCA_AUTOMATICA)
        );

        let letra = letras_web::buscar(&mut cliente, &consulta).unwrap();
        println!("letra: {letra:?}");
        assert!(matches!(
            letra,
            letras_web::Resultado::Salva {
                sincronizada: true,
                ..
            }
        ));

        if let Some(l) = lista[0].lancamento.as_deref() {
            println!("capa: {:?}", musicbrainz::baixar_capa(&mut cliente, l));
        }
        let _ = std::fs::remove_dir_all(&pasta);

        // FURINAR_WEB_PASTA=<pasta>: identifica as primeiras faixas dela
        // (só lê; não grava letra nem tag)
        let Some(musicas) = std::env::var_os("FURINAR_WEB_PASTA").map(PathBuf::from) else {
            return;
        };
        let mut tracks = Vec::new();
        crate::track::coletar_arquivos_audio(&musicas, &musicas, false, &mut tracks);
        tracks.sort_by(|a, b| a.path.cmp(&b.path));
        for t in tracks.iter().take(8) {
            let c = Consulta {
                chave: t.path.clone(),
                caminho: musicas.join(&t.path),
                titulo: t.titulo.clone(),
                artista: t.artista.clone(),
                album: t.album.clone(),
                duracao: t.duracao,
            };
            let (lista, erro) = identificar(&mut cliente, &c, None);
            let melhor = lista.first().map(|m| {
                format!(
                    "{}% {} — {} · {:?} ({:?})",
                    m.confianca, m.titulo, m.artista, m.album, m.ano
                )
            });
            println!(
                "{:?} / {:?}\n    → {melhor:?} {erro:?}",
                t.artista, t.titulo
            );
        }
    }
}
