//! "Ouvindo Furinar" no perfil do Discord (Rich Presence, pela IPC local do
//! app do Discord).
//!
//! Uma thread guarda a conexão e o último estado: o Discord pode abrir ou
//! fechar a qualquer momento, então ela reconecta sozinha. Só existe com a
//! opção ligada; soltar o `Discord` encerra a thread e limpa o status.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use discord_rich_presence::activity::{
    Activity, ActivityType, Assets, StatusDisplayType, Timestamps,
};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};

/// App "Furinar" no portal de desenvolvedores do Discord (o nome dele é o
/// que aparece como "Ouvindo Furinar"). Não é segredo.
const APLICATIVO: &str = "1562611821087563846";
/// Sem capa conhecida: o ícone do Furinar.
const ICONE: &str =
    "https://raw.githubusercontent.com/LokKol17/furinar/master/ui/assets/furinar_icon.png";
/// Com o Discord fechado, tenta reconectar nesse intervalo.
const RECONECTAR: Duration = Duration::from_secs(20);

/// O que mostrar. `None` (em `Discord::mostrar`) limpa o status.
#[derive(Clone, Debug, PartialEq)]
pub struct Status {
    pub titulo: String,
    pub artista: Option<String>,
    pub album: Option<String>,
    /// Segundos já tocados e duração total (0 = desconhecida): o Discord
    /// desenha a barra de progresso a partir disso.
    pub posicao: u64,
    pub duracao: u64,
    /// URL da capa (Cover Art Archive), se a faixa tem uma conhecida.
    pub capa: Option<String>,
}

/// Status com o instante em que a faixa começou (segundos desde 1970).
type Envio = (Status, u64);

pub struct Discord {
    tx: Sender<Option<Envio>>,
    /// Último status mandado, para não repetir a cada tick.
    ultimo: Option<Envio>,
}

impl Discord {
    pub fn novo() -> Self {
        let (tx, rx) = mpsc::channel();
        let _ = std::thread::Builder::new()
            .name("furinar-discord".into())
            .spawn(move || trabalhar(&rx));
        Self { tx, ultimo: None }
    }

    /// Manda o status se ele mudou de verdade (a posição andando no ritmo do
    /// relógio não conta: o Discord já anima a barra sozinho).
    pub fn mostrar(&mut self, status: Option<Status>) {
        let envio = status.map(|s| {
            let inicio = agora().saturating_sub(s.posicao);
            (s, inicio)
        });
        if mudou(self.ultimo.as_ref(), envio.as_ref()) {
            self.ultimo = envio.clone();
            let _ = self.tx.send(envio);
        }
    }
}

/// Outra faixa, ou a mesma começando em outro instante (seek). A folga cobre
/// o atraso entre os ticks.
fn mudou(antes: Option<&Envio>, agora: Option<&Envio>) -> bool {
    match (antes, agora) {
        (Some((a, ia)), Some((b, ib))) => {
            a.titulo != b.titulo
                || a.artista != b.artista
                || a.album != b.album
                || a.capa != b.capa
                || a.duracao != b.duracao
                || ia.abs_diff(*ib) > 2
        }
        (None, None) => false,
        _ => true,
    }
}

fn agora() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn trabalhar(rx: &Receiver<Option<Envio>>) {
    let mut cliente = DiscordIpcClient::new(APLICATIVO);
    let mut conectado = false;
    let mut desejado: Option<Envio> = None;
    loop {
        // Espera mudança; sem conexão, acorda de tempos em tempos para
        // tentar de novo
        let espera = if conectado {
            Duration::from_secs(3600)
        } else {
            RECONECTAR
        };
        match rx.recv_timeout(espera) {
            Ok(s) => desejado = s,
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                if conectado {
                    let _ = cliente.clear_activity();
                    let _ = cliente.close();
                }
                return;
            }
        }
        // Só o mais recente importa
        while let Ok(s) = rx.try_recv() {
            desejado = s;
        }
        if !conectado {
            conectado = cliente.connect().is_ok();
            if !conectado {
                continue;
            }
        }
        let ok = match &desejado {
            Some((s, inicio)) => cliente.set_activity(atividade(s, *inicio)).is_ok(),
            None => cliente.clear_activity().is_ok(),
        };
        if !ok {
            // O Discord fechou: reconecta na próxima volta
            let _ = cliente.close();
            conectado = false;
        }
    }
}

fn atividade(s: &Status, inicio: u64) -> Activity<'_> {
    let inicio = (inicio * 1000) as i64;
    let mut tempos = Timestamps::new().start(inicio);
    if s.duracao > 0 {
        tempos = tempos.end(inicio + (s.duracao * 1000) as i64);
    }
    let mut assets = Assets::new().large_image(s.capa.as_deref().unwrap_or(ICONE));
    if let Some(album) = &s.album {
        assets = assets.large_text(album.as_str());
    }
    let mut a = Activity::new()
        .activity_type(ActivityType::Listening)
        // "Ouvindo <título>" na lista de membros
        .status_display_type(StatusDisplayType::Details)
        .details(s.titulo.as_str())
        .timestamps(tempos)
        .assets(assets);
    if let Some(artista) = &s.artista {
        a = a.state(artista.as_str());
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(posicao: u64) -> Status {
        Status {
            titulo: "Faixa".into(),
            artista: Some("Banda".into()),
            album: None,
            posicao,
            duracao: 200,
            capa: None,
        }
    }

    #[test]
    fn posicao_andando_nao_conta_como_mudanca() {
        // Tocando normal: posição e relógio andam juntos, o início é o mesmo
        let antes = (status(10), 1000);
        assert!(!mudou(Some(&antes), Some(&(status(11), 1001))));
        // Seek: começou em outro instante
        assert!(mudou(Some(&antes), Some(&(status(90), 921))));
        // Outra faixa, pausa (None) e volta
        let mut outra = status(10);
        outra.titulo = "Outra".into();
        assert!(mudou(Some(&antes), Some(&(outra, 1000))));
        assert!(mudou(Some(&antes), None));
        assert!(!mudou(None, None));
    }
}
