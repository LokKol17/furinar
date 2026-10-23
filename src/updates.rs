//! Atualização a partir das releases do GitHub.
//!
//! `verificar` acha a release mais nova e decide como instalá-la:
//! - **Substituir**: troca o próprio executável (portátil no Windows, e no
//!   Linux quando a pasta do binário é gravável — `install.sh` instala em
//!   `~/.local/bin`);
//! - **Instalador**: Windows instalado pelo setup (há `unins000.exe` ao
//!   lado) — baixa o setup e roda em silêncio, o que mantém o registro de
//!   "Aplicativos" em dia;
//! - **Manual**: a pasta não é gravável (ex.: `/usr/local/bin` de uma
//!   instalação antiga com sudo) — só avisa e abre a página da release.
//!
//! Instalações por gerenciador de pacotes (`/usr/bin`, snap, flatpak) não
//! são verificadas: quem atualiza é o gerenciador.
//!
//! Todo download é conferido com o `SHA256SUMS` da mesma release antes de
//! tocar no executável.

use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const DONO: &str = "LokKol17";
const REPO: &str = "furinar";
const SOMAS: &str = "SHA256SUMS";
const SETUP_WINDOWS: &str = "furinar-windows-x86_64-setup.exe";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modo {
    Substituir,
    Instalador,
    Manual,
}

#[derive(Clone)]
pub struct Atualizacao {
    pub versao: String,
    pub notas: String,
    pub modo: Modo,
    /// Página da release (botão do modo manual)
    pub pagina: String,
    asset: Asset,
    somas: Option<Asset>,
}

#[derive(Clone)]
struct Asset {
    nome: String,
    url: String,
}

/// O que ficou pronto depois do download, para `reiniciar`.
pub enum Baixado {
    /// Executável já trocado; reabrir este caminho
    Substituido(PathBuf),
    /// Setup baixado e conferido; rodar em silêncio
    Instalador(PathBuf),
}

/// Instalado por gerenciador de pacotes: ele é quem atualiza.
fn instalacao_de_pacote(exe: &Path) -> bool {
    let caminho = exe.to_string_lossy();
    cfg!(target_os = "linux")
        && (caminho.starts_with("/usr/bin")
            || caminho.starts_with("/snap")
            || caminho.starts_with("/var/lib/flatpak")
            || caminho.starts_with("/app/"))
}

/// Nome do asset da release para esta plataforma (o executável solto).
fn asset_da_plataforma() -> &'static str {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "furinar-windows-x86_64.exe"
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "furinar-linux-x86_64.tar.gz"
    }
    #[cfg(not(any(
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "x86_64"),
    )))]
    {
        "furinar"
    }
}

/// Dá para criar um arquivo na pasta? (é o que a troca do executável faz)
fn gravavel(pasta: &Path) -> bool {
    let teste = pasta.join(".furinar-teste-escrita");
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&teste)
    {
        Ok(_) => {
            let _ = std::fs::remove_file(&teste);
            true
        }
        Err(_) => false,
    }
}

fn modo_para(pasta: &Path) -> Modo {
    if cfg!(target_os = "windows") && pasta.join("unins000.exe").exists() {
        Modo::Instalador
    } else if gravavel(pasta) {
        Modo::Substituir
    } else {
        Modo::Manual
    }
}

/// "v1.2.3" → "1.2.3".
fn sem_v(tag: &str) -> &str {
    tag.strip_prefix('v').unwrap_or(tag)
}

/// Corpo da release (markdown do CHANGELOG) como texto corrido para o
/// aviso: sem títulos `#`, listas com "•", links só com o texto, e sem a
/// parte de downloads que o CI acrescenta no fim.
pub fn notas_legiveis(corpo: &str) -> String {
    let mut saida: Vec<String> = Vec::new();
    // Logo depois de um título, linha em branco não separa nada
    let mut apos_titulo = false;
    for linha in corpo.lines() {
        let linha = linha.trim_end();
        if linha.starts_with("## Downloads") || linha.starts_with("<!-- downloads -->") {
            break;
        }
        if linha.is_empty() && apos_titulo {
            continue;
        }
        apos_titulo = linha.starts_with('#');
        let linha = linha.trim_start_matches('#').trim_start();
        let linha = match linha.strip_prefix("- ") {
            Some(item) => format!("• {item}"),
            None => linha.to_string(),
        };
        let linha = sem_links(&linha).replace("**", "").replace('`', "");
        if linha.is_empty() && saida.last().is_none_or(|l| l.is_empty()) {
            continue;
        }
        saida.push(linha);
    }
    while saida.last().is_some_and(|l| l.is_empty()) {
        saida.pop();
    }
    saida.join("\n")
}

/// `[texto](url)` → `texto`.
fn sem_links(linha: &str) -> String {
    let mut saida = String::with_capacity(linha.len());
    let mut resto = linha;
    while let Some(abre) = resto.find('[') {
        let Some(fecha) = resto[abre..].find("](").map(|i| abre + i) else {
            break;
        };
        let Some(fim) = resto[fecha..].find(')').map(|i| fecha + i) else {
            break;
        };
        saida.push_str(&resto[..abre]);
        saida.push_str(&resto[abre + 1..fecha]);
        resto = &resto[fim + 1..];
    }
    saida.push_str(resto);
    saida
}

/// Procura uma versão mais nova. `None` sem rede, sem novidade ou em
/// instalação de pacote.
pub fn verificar() -> Option<Atualizacao> {
    let exe = std::env::current_exe().ok()?;
    if instalacao_de_pacote(&exe) {
        return None;
    }
    let modo = modo_para(exe.parent()?);
    let atual = semver::Version::parse(env!("CARGO_PKG_VERSION")).ok()?;

    let releases = self_update::backends::github::ReleaseList::configure()
        .repo_owner(DONO)
        .repo_name(REPO)
        .build()
        .ok()?
        .fetch()
        .ok()?;

    let nome_asset = if modo == Modo::Instalador {
        SETUP_WINDOWS
    } else {
        asset_da_plataforma()
    };
    escolher(&releases, &atual, nome_asset, modo)
}

/// A release mais nova que `atual` com o asset desta instalação.
fn escolher(
    releases: &[self_update::update::Release],
    atual: &semver::Version,
    nome_asset: &str,
    modo: Modo,
) -> Option<Atualizacao> {
    let (versao, release, asset) = releases
        .iter()
        .filter_map(|r| {
            let versao = semver::Version::parse(sem_v(&r.version)).ok()?;
            let asset = r.assets.iter().find(|a| a.name == nome_asset)?;
            Some((versao, r, asset))
        })
        .max_by(|a, b| a.0.cmp(&b.0))?;
    if &versao <= atual {
        return None;
    }
    let como_asset = |a: &self_update::update::ReleaseAsset| Asset {
        nome: a.name.clone(),
        url: a.download_url.clone(),
    };
    Some(Atualizacao {
        versao: versao.to_string(),
        notas: notas_legiveis(release.body.as_deref().unwrap_or("")),
        modo,
        pagina: format!("https://github.com/{DONO}/{REPO}/releases/tag/v{versao}"),
        asset: como_asset(asset),
        somas: release
            .assets
            .iter()
            .find(|a| a.name == SOMAS)
            .map(como_asset),
    })
}

fn cliente() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent(concat!("Furinar/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())
}

fn pedir(
    cliente: &reqwest::blocking::Client,
    url: &str,
) -> Result<reqwest::blocking::Response, String> {
    // A URL da API devolve o binário com este Accept (e redireciona)
    cliente
        .get(url)
        .header(reqwest::header::ACCEPT, "application/octet-stream")
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())
}

/// Hash esperado de `nome` num `SHA256SUMS` ("<hex>  <nome>" por linha).
fn hash_esperado(somas: &str, nome: &str) -> Option<String> {
    somas.lines().find_map(|linha| {
        let (hash, arquivo) = linha.split_once(char::is_whitespace)?;
        let arquivo = arquivo.trim().trim_start_matches('*');
        let arquivo = arquivo.rsplit('/').next().unwrap_or(arquivo);
        (arquivo == nome && hash.len() == 64).then(|| hash.to_ascii_lowercase())
    })
}

/// Copia `leitor` para `saida` calculando o SHA-256 e avisando o progresso
/// (0.0..=1.0) quando o tamanho é conhecido.
fn copiar_com_hash(
    mut leitor: impl Read,
    mut saida: impl Write,
    total: Option<u64>,
    progresso: &dyn Fn(f32),
) -> Result<String, String> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    let mut lidos = 0u64;
    let mut ultimo = -1i32;
    loop {
        let n = leitor.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
        saida.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
        lidos += n as u64;
        if let Some(total) = total.filter(|t| *t > 0) {
            // Avisa a cada 1%, não a cada bloco
            let pct = (lidos * 100 / total).min(100) as i32;
            if pct != ultimo {
                ultimo = pct;
                progresso(pct as f32 / 100.0);
            }
        }
    }
    saida.flush().map_err(|e| e.to_string())?;
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// Baixa, confere e deixa pronto. No modo `Substituir` o executável já
/// sai trocado daqui.
pub fn baixar(at: &Atualizacao, progresso: &dyn Fn(f32)) -> Result<Baixado, String> {
    let somas = at
        .somas
        .as_ref()
        .ok_or("a release não tem SHA256SUMS; atualização recusada")?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let cliente = cliente()?;

    let somas = pedir(&cliente, &somas.url)?
        .text()
        .map_err(|e| e.to_string())?;
    let esperado = hash_esperado(&somas, &at.asset.nome)
        .ok_or_else(|| format!("{} não está no SHA256SUMS", at.asset.nome))?;

    // Ao lado do executável quando possível: a troca vira um rename na
    // mesma partição. O setup vai para a pasta temporária.
    let pasta = match at.modo {
        Modo::Substituir => exe.parent().map(Path::to_path_buf),
        _ => None,
    }
    .unwrap_or_else(std::env::temp_dir);
    let destino = pasta.join(format!(".furinar-{}-{}", at.versao, at.asset.nome));

    let resposta = pedir(&cliente, &at.asset.url)?;
    let total = resposta.content_length();
    let arquivo = std::fs::File::create(&destino).map_err(|e| e.to_string())?;
    let obtido = copiar_com_hash(resposta, std::io::BufWriter::new(arquivo), total, progresso);
    let obtido = match obtido {
        Ok(h) => h,
        Err(e) => {
            let _ = std::fs::remove_file(&destino);
            return Err(e);
        }
    };
    if obtido != esperado {
        let _ = std::fs::remove_file(&destino);
        return Err(format!(
            "o arquivo baixado não confere com o SHA256SUMS (esperado {esperado}, veio {obtido})"
        ));
    }

    match at.modo {
        Modo::Instalador => {
            let setup = std::env::temp_dir().join(SETUP_WINDOWS);
            std::fs::rename(&destino, &setup).map_err(|e| e.to_string())?;
            Ok(Baixado::Instalador(setup))
        }
        Modo::Substituir => {
            let resultado = substituir(&destino, &pasta);
            let _ = std::fs::remove_file(&destino);
            resultado?;
            Ok(Baixado::Substituido(exe))
        }
        Modo::Manual => {
            let _ = std::fs::remove_file(&destino);
            Err("esta instalação não se atualiza sozinha".into())
        }
    }
}

/// Troca o executável em uso pelo baixado (extraindo do .tar.gz no Linux).
fn substituir(baixado: &Path, pasta: &Path) -> Result<(), String> {
    let erro = |e: &dyn std::fmt::Display| e.to_string();
    if !baixado.to_string_lossy().ends_with(".tar.gz") {
        return self_update::self_replace::self_replace(baixado).map_err(|e| erro(&e));
    }
    let temp = self_update::TempDir::new_in(pasta)
        .or_else(|_| self_update::TempDir::new())
        .map_err(|e| erro(&e))?;
    self_update::Extract::from_source(baixado)
        .archive(self_update::ArchiveKind::Tar(Some(
            self_update::Compression::Gz,
        )))
        .extract_file(temp.path(), "furinar")
        .map_err(|e| erro(&e))?;
    self_update::self_replace::self_replace(temp.path().join("furinar")).map_err(|e| erro(&e))
}

/// Abre a versão nova. Quem chama salva o estado antes e fecha o app logo
/// depois. No Windows instalado, o setup roda em silêncio, fecha o que
/// sobrar aberto e reabre o Furinar no fim (`/reabrir=1`, ver furinar.iss).
pub fn reiniciar(baixado: &Baixado) -> Result<(), String> {
    let mut comando = match baixado {
        Baixado::Substituido(exe) => std::process::Command::new(exe),
        Baixado::Instalador(setup) => {
            let mut c = std::process::Command::new(setup);
            c.args([
                "/VERYSILENT",
                "/SUPPRESSMSGBOXES",
                "/NORESTART",
                "/CLOSEAPPLICATIONS",
                "/reabrir=1",
            ]);
            c
        }
    };
    comando.spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use self_update::update::{Release, ReleaseAsset};

    fn release(versao: &str, assets: &[&str], corpo: &str) -> Release {
        Release {
            name: versao.into(),
            version: versao.into(),
            date: String::new(),
            body: Some(corpo.into()),
            assets: assets
                .iter()
                .map(|n| ReleaseAsset {
                    name: n.to_string(),
                    download_url: format!("https://x/{n}"),
                })
                .collect(),
        }
    }

    #[test]
    fn escolhe_a_mais_nova_que_tem_o_asset() {
        let atual = semver::Version::parse("1.7.1").unwrap();
        let releases = [
            // Mais nova, mas sem o setup (ex.: build do Windows falhou)
            release("v1.8.0", &["furinar-windows-x86_64.exe", "SHA256SUMS"], ""),
            release(
                "v1.7.2",
                &[SETUP_WINDOWS, "furinar-windows-x86_64.exe", "SHA256SUMS"],
                "notas",
            ),
            release("v1.7.0", &[SETUP_WINDOWS], ""),
        ];
        let at = escolher(&releases, &atual, SETUP_WINDOWS, Modo::Instalador).unwrap();
        assert_eq!(at.versao, "1.7.2");
        assert_eq!(at.asset.nome, SETUP_WINDOWS);
        assert_eq!(at.somas.unwrap().url, "https://x/SHA256SUMS");
        assert_eq!(
            at.pagina,
            "https://github.com/LokKol17/furinar/releases/tag/v1.7.2"
        );

        let portatil = escolher(
            &releases,
            &atual,
            "furinar-windows-x86_64.exe",
            Modo::Substituir,
        );
        assert_eq!(portatil.unwrap().versao, "1.8.0");
    }

    #[test]
    fn sem_versao_nova_nao_avisa() {
        let atual = semver::Version::parse("1.7.2").unwrap();
        let releases = [release("v1.7.2", &["a"], ""), release("lixo", &["a"], "")];
        assert!(escolher(&releases, &atual, "a", Modo::Substituir).is_none());
    }

    #[test]
    fn le_o_hash_do_sha256sums() {
        let h = "ab".repeat(32);
        let somas = format!(
            "{h}  furinar-windows-x86_64.exe\n{}  windows/furinar-linux-x86_64.tar.gz\n",
            "CD".repeat(32)
        );
        assert_eq!(hash_esperado(&somas, "furinar-windows-x86_64.exe"), Some(h));
        // Caminho antes do nome e hash em maiúsculas
        assert_eq!(
            hash_esperado(&somas, "furinar-linux-x86_64.tar.gz"),
            Some("cd".repeat(32))
        );
        assert_eq!(hash_esperado(&somas, "outro"), None);
        assert_eq!(hash_esperado("curto  outro\n", "outro"), None);
    }

    #[test]
    fn copia_calcula_sha256_e_avisa_progresso() {
        let dados = b"abc";
        let mut saida = Vec::new();
        let avisos = std::cell::RefCell::new(Vec::new());
        let hash = copiar_com_hash(&dados[..], &mut saida, Some(3), &|p| {
            avisos.borrow_mut().push(p)
        })
        .unwrap();
        assert_eq!(saida, dados);
        // SHA-256 de "abc" (FIPS 180-2)
        assert_eq!(
            hash,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(avisos.borrow().last(), Some(&1.0));
    }

    #[test]
    fn notas_viram_texto_corrido() {
        let corpo = "### Corrigido\n\n- Falha ao **tocar** `mp3`\n- Ver [CHANGELOG](https://x)\n\n\n### Adicionado\n- Coisa\n\n## Downloads\n- **Windows:** setup\n";
        assert_eq!(
            notas_legiveis(corpo),
            "Corrigido\n• Falha ao tocar mp3\n• Ver CHANGELOG\n\nAdicionado\n• Coisa"
        );
    }

    #[test]
    fn pacote_nao_se_atualiza_e_pasta_sem_escrita_vira_manual() {
        if cfg!(target_os = "linux") {
            assert!(instalacao_de_pacote(Path::new("/usr/bin/furinar")));
            assert!(!instalacao_de_pacote(Path::new("/usr/local/bin/furinar")));
            assert!(!instalacao_de_pacote(Path::new(
                "/home/a/.local/bin/furinar"
            )));
        }
        let temp = std::env::temp_dir();
        assert_eq!(modo_para(&temp), Modo::Substituir);
        assert_eq!(modo_para(Path::new("/pasta/que/nao/existe")), Modo::Manual);
    }

    /// Servidor HTTP de um tiro por pedido, só para os testes de download:
    /// responde cada caminho com o conteúdo dado.
    fn servidor(arquivos: Vec<(&'static str, Vec<u8>)>) -> String {
        let escuta = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endereco = format!("http://{}", escuta.local_addr().unwrap());
        std::thread::spawn(move || {
            for conexao in escuta.incoming().flatten() {
                let mut conexao = conexao;
                let mut pedido = [0u8; 2048];
                let n = conexao.read(&mut pedido).unwrap_or(0);
                let linha = String::from_utf8_lossy(&pedido[..n]);
                let caminho = linha.split_whitespace().nth(1).unwrap_or("");
                let corpo = arquivos
                    .iter()
                    .find(|(c, _)| *c == caminho)
                    .map(|(_, d)| d.clone());
                let resposta = match corpo {
                    Some(d) => {
                        let mut r = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            d.len()
                        )
                        .into_bytes();
                        r.extend(d);
                        r
                    }
                    None => {
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            .to_vec()
                    }
                };
                let _ = conexao.write_all(&resposta);
            }
        });
        endereco
    }

    fn atualizacao(base: &str, nome: &str) -> Atualizacao {
        Atualizacao {
            versao: "9.9.9".into(),
            notas: String::new(),
            modo: Modo::Instalador,
            pagina: String::new(),
            asset: Asset {
                nome: nome.into(),
                url: format!("{base}/asset"),
            },
            somas: Some(Asset {
                nome: SOMAS.into(),
                url: format!("{base}/somas"),
            }),
        }
    }

    #[test]
    fn baixa_confere_o_hash_e_recusa_arquivo_adulterado() {
        let dados = vec![7u8; 300_000];
        let hash: String = Sha256::digest(&dados)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let nome = "furinar-teste-asset.bin";
        let somas = format!("{hash}  {nome}\n").into_bytes();

        let base = servidor(vec![("/asset", dados.clone()), ("/somas", somas.clone())]);
        let ultimo = std::cell::Cell::new(0.0f32);
        let baixado = baixar(&atualizacao(&base, nome), &|p| ultimo.set(p)).unwrap();
        let Baixado::Instalador(caminho) = baixado else {
            panic!("modo instalador devolve o setup");
        };
        assert_eq!(std::fs::read(&caminho).unwrap(), dados);
        assert_eq!(ultimo.get(), 1.0);
        let _ = std::fs::remove_file(caminho);

        // Mesmo SHA256SUMS, arquivo diferente: recusa e não deixa nada
        let base = servidor(vec![("/asset", vec![8u8; 1000]), ("/somas", somas)]);
        let erro = baixar(&atualizacao(&base, nome), &|_| {}).err().unwrap();
        assert!(erro.contains("não confere"), "{erro}");

        // Sem SHA256SUMS na release: recusa antes de baixar
        let mut sem_somas = atualizacao(&base, nome);
        sem_somas.somas = None;
        assert!(baixar(&sem_somas, &|_| {}).is_err());
    }

    #[test]
    fn versao_compilada_e_semver_valido() {
        let versao = env!("CARGO_PKG_VERSION");
        assert!(
            semver::Version::parse(versao).is_ok(),
            "{versao} não é semver"
        );
    }

    /// Smoke de rede: lista as releases com rustls e confere que a mais nova
    /// tem o asset desta plataforma e o SHA256SUMS.
    /// `cargo test smoke_rede -- --ignored --nocapture`.
    #[test]
    #[ignore = "requer rede"]
    fn smoke_rede_busca_releases_via_rustls() {
        let releases = self_update::backends::github::ReleaseList::configure()
            .repo_owner(DONO)
            .repo_name(REPO)
            .build()
            .expect("ReleaseList build falhou")
            .fetch()
            .expect("HTTPS com rustls falhou");
        println!("{} releases encontrados", releases.len());
        let alvo = asset_da_plataforma();
        assert!(
            releases
                .iter()
                .any(|r| r.assets.iter().any(|a| a.name == alvo)),
            "nenhuma release tem o asset {alvo}; conferir release.yml"
        );
    }
}
