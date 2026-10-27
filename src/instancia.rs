//! "Abrir com o Furinar" com o Furinar já aberto: a nova execução entrega os
//! caminhos para a que está rodando e sai, em vez de subir um segundo player.
//!
//! Linux: socket Unix em `$XDG_RUNTIME_DIR`. Windows: TCP no loopback, com a
//! porta num arquivo ao lado da configuração. O dono responde `furinar`: um
//! socket órfão (o dono anterior caiu) ou uma porta reaproveitada por outro
//! programa não enganam o cliente.

use std::io::{Read, Write};
use std::net::Shutdown;
use std::path::PathBuf;
use std::time::Duration;

const RESPOSTA: &[u8] = b"furinar\n";
/// Quanto o cliente espera a resposta do dono.
const ESPERA: Duration = Duration::from_secs(2);
/// Uma segunda instância (aberta sem arquivos, ou o app novo logo depois de
/// uma atualização) tenta assumir o posto de dono neste intervalo.
const NOVA_TENTATIVA: Duration = Duration::from_secs(5);
/// Teto do que um cliente manda (caminhos, um por linha).
const MAXIMO: u64 = 1 << 20;

/// Arquivos e pastas da linha de comando (`furinar %F`), absolutos. Opções
/// (`--imersivo`) e o que não existe ficam de fora.
pub fn caminhos_dos_argumentos() -> Vec<PathBuf> {
    std::env::args_os()
        .skip(1)
        .filter(|a| !a.to_string_lossy().starts_with("--"))
        .filter_map(|a| std::path::absolute(PathBuf::from(a)).ok())
        .filter(|p| p.is_dir() || (p.is_file() && crate::track::e_arquivo_audio(p)))
        .collect()
}

/// Entrega `caminhos` à instância aberta. `false` = não há outra (esta segue
/// e abre os caminhos ela mesma).
pub fn enviar(caminhos: &[PathBuf]) -> bool {
    let texto: String = caminhos
        .iter()
        .map(|p| format!("{}\n", p.to_string_lossy()))
        .collect();
    conversar(texto.as_bytes())
}

/// Thread que recebe os caminhos das execuções seguintes. Se outra instância
/// já é a dona, espera ela fechar e assume.
pub fn escutar(entregar: impl Fn(Vec<PathBuf>) + Send + 'static) {
    let _ = std::thread::Builder::new()
        .name("furinar-instancia".into())
        .spawn(move || {
            loop {
                if !conversar(b"")
                    && let Some(ouvinte) = canal::ouvir()
                {
                    servir(&ouvinte, &entregar);
                    return;
                }
                std::thread::sleep(NOVA_TENTATIVA);
            }
        });
}

/// Manda `dados` ao dono e confere a resposta. Vazio = só "tem alguém aí?".
fn conversar(dados: &[u8]) -> bool {
    let Some(mut s) = canal::conectar() else {
        return false;
    };
    let _ = s.set_read_timeout(Some(ESPERA));
    if s.write_all(dados).is_err() || s.shutdown(Shutdown::Write).is_err() {
        return false;
    }
    let mut resposta = Vec::new();
    let _ = s.take(RESPOSTA.len() as u64).read_to_end(&mut resposta);
    resposta == RESPOSTA
}

fn servir(ouvinte: &canal::Ouvinte, entregar: &impl Fn(Vec<PathBuf>)) {
    for conexao in ouvinte.incoming() {
        let Ok(mut s) = conexao else { continue };
        let _ = s.set_read_timeout(Some(ESPERA));
        let mut texto = String::new();
        if (&mut s).take(MAXIMO).read_to_string(&mut texto).is_err() {
            continue;
        }
        let _ = s.write_all(RESPOSTA);
        let caminhos = ler_caminhos(&texto);
        if !caminhos.is_empty() {
            entregar(caminhos);
        }
    }
}

fn ler_caminhos(texto: &str) -> Vec<PathBuf> {
    texto
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .collect()
}

#[cfg(not(windows))]
mod canal {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::{UnixListener, UnixStream};

    pub type Ouvinte = UnixListener;

    fn endereco() -> PathBuf {
        match std::env::var_os("XDG_RUNTIME_DIR").filter(|d| std::path::Path::new(d).is_dir()) {
            Some(d) => PathBuf::from(d).join("furinar.sock"),
            // Sem a pasta de runtime: junto da configuração, que é só do usuário
            None => crate::config::config_path().with_file_name("furinar.sock"),
        }
    }

    pub fn conectar() -> Option<UnixStream> {
        UnixStream::connect(endereco()).ok()
    }

    /// Só depois de `conversar` falhar: o arquivo que sobrar é de um dono
    /// que já fechou.
    pub fn ouvir() -> Option<UnixListener> {
        let endereco = endereco();
        let _ = std::fs::remove_file(&endereco);
        let ouvinte = UnixListener::bind(&endereco).ok()?;
        let _ = std::fs::set_permissions(&endereco, std::fs::Permissions::from_mode(0o600));
        Some(ouvinte)
    }
}

#[cfg(windows)]
mod canal {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};

    pub type Ouvinte = TcpListener;

    fn arquivo_porta() -> PathBuf {
        crate::config::config_path().with_file_name("furinar_instancia.porta")
    }

    pub fn conectar() -> Option<TcpStream> {
        let porta: u16 = std::fs::read_to_string(arquivo_porta())
            .ok()?
            .trim()
            .parse()
            .ok()?;
        let endereco = SocketAddr::from((Ipv4Addr::LOCALHOST, porta));
        TcpStream::connect_timeout(&endereco, Duration::from_millis(300)).ok()
    }

    pub fn ouvir() -> Option<TcpListener> {
        let ouvinte = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).ok()?;
        let porta = ouvinte.local_addr().ok()?.port();
        std::fs::write(arquivo_porta(), porta.to_string()).ok()?;
        Some(ouvinte)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caminhos_recebidos_so_absolutos_e_sem_linhas_vazias() {
        let raiz = std::env::temp_dir();
        let a = raiz.join("a.mp3");
        let texto = format!("{}\n\nrelativo.mp3\n  {}  \n", a.display(), a.display());
        assert_eq!(ler_caminhos(&texto), vec![a.clone(), a]);
    }
}
