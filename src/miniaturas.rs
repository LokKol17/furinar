//! Miniaturas de capa para o painel da biblioteca do modo imersivo.
//!
//! Duas threads atendem uma fila em que o pedido novo substitui o anterior:
//! rolar a lista rápido não deixa trabalho velho na frente. As capas reduzidas
//! ficam em cache no disco, então a segunda visita a uma pasta não decodifica
//! capa nenhuma; ele tem teto (`LIMITE_CACHE`) e perde primeiro o que foi
//! usado há mais tempo. As threads terminam quando o `Carregador` é solto.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use image::{DynamicImage, ImageFormat};
use lofty::config::ParseOptions;
use lofty::prelude::*;
use lofty::probe::Probe;
use slint::{Rgba8Pixel, SharedPixelBuffer};

use crate::capa;

/// Lado da miniatura (o painel mostra 40 px; sobra para telas HiDPI).
pub const LADO: u32 = 96;
const THREADS: usize = 2;
const QUALIDADE_JPEG: u8 = 85;
/// Teto do cache em disco. Cada miniatura gasta uns 4–8 KB de disco, então
/// cabem perto de 10 mil faixas.
const LIMITE_CACHE: u64 = 64 * 1024 * 1024;
/// Menor espaço que um arquivo ocupa no disco: o marcador vazio de "sem
/// capa" também conta.
const BLOCO: u64 = 4096;

/// Miniatura pronta para virar `slint::Image` (`None`: a faixa não tem capa).
pub type Capa = Option<SharedPixelBuffer<Rgba8Pixel>>;

/// Faixa e, se houver, a capa baixada da web (a última opção).
pub type Pedido = (PathBuf, Option<PathBuf>);

struct Fila {
    pendentes: VecDeque<Pedido>,
    fim: bool,
}

type Compartilhada = Arc<(Mutex<Fila>, Condvar)>;

pub struct Carregador {
    fila: Compartilhada,
    rx: Receiver<(PathBuf, Capa)>,
}

impl Carregador {
    pub fn novo() -> Self {
        Self::com_cache(pasta_cache())
    }

    fn com_cache(cache: Option<PathBuf>) -> Self {
        let fila: Compartilhada = Arc::new((
            Mutex::new(Fila {
                pendentes: VecDeque::new(),
                fim: false,
            }),
            Condvar::new(),
        ));
        let (tx, rx) = mpsc::channel();
        if let Some(pasta) = cache.clone() {
            let _ = std::thread::Builder::new()
                .name("furinar-capas-limpeza".into())
                .spawn(move || aparar_cache(&pasta, LIMITE_CACHE));
        }
        for _ in 0..THREADS {
            let fila = fila.clone();
            let tx = tx.clone();
            let cache = cache.clone();
            let _ = std::thread::Builder::new()
                .name("furinar-capas".into())
                .spawn(move || trabalhar(&fila, &tx, cache.as_deref()));
        }
        Self { fila, rx }
    }

    /// Troca a fila pelos caminhos dados, na ordem: o pedido anterior que
    /// ainda não começou é esquecido.
    pub fn pedir(&self, caminhos: Vec<Pedido>) {
        let (fila, sinal) = &*self.fila;
        if let Ok(mut f) = fila.lock() {
            f.pendentes = caminhos.into();
            sinal.notify_all();
        }
    }

    pub fn recebidos(&self) -> impl Iterator<Item = (PathBuf, Capa)> + '_ {
        self.rx.try_iter()
    }
}

impl Drop for Carregador {
    fn drop(&mut self) {
        let (fila, sinal) = &*self.fila;
        if let Ok(mut f) = fila.lock() {
            f.fim = true;
            f.pendentes.clear();
        }
        sinal.notify_all();
    }
}

fn trabalhar(fila: &Compartilhada, tx: &Sender<(PathBuf, Capa)>, cache: Option<&Path>) {
    let (trava, sinal) = &**fila;
    loop {
        let (caminho, web) = {
            let Ok(mut f) = trava.lock() else {
                return;
            };
            loop {
                if f.fim {
                    return;
                }
                if let Some(c) = f.pendentes.pop_front() {
                    break c;
                }
                f = match sinal.wait(f) {
                    Ok(f) => f,
                    Err(_) => return,
                };
            }
        };
        let capa = carregar(&caminho, web.as_deref(), cache);
        if tx.send((caminho, capa)).is_err() {
            return;
        }
    }
}

/// Miniatura de uma faixa só, com o cache em disco (mini player).
pub fn miniatura(caminho: &Path, web: Option<&Path>) -> Capa {
    carregar(caminho, web, pasta_cache().as_deref())
}

/// Acha a miniatura: do cache (sem nem abrir a faixa), ou da capa embutida /
/// `cover.jpg` / capa baixada `web` (e então grava no cache). Arquivo de cache
/// vazio quer dizer "essa faixa não tem capa", para não procurar de novo.
pub fn carregar(caminho: &Path, web: Option<&Path>, cache: Option<&Path>) -> Capa {
    let arquivo_cache = cache.and_then(|pasta| chave(caminho, web).map(|k| pasta.join(k)));
    if let Some(arquivo) = &arquivo_cache
        && let Ok(bytes) = std::fs::read(arquivo)
    {
        tocar(arquivo);
        if bytes.is_empty() {
            return None;
        }
        return image::load_from_memory_with_format(&bytes, ImageFormat::Jpeg)
            .ok()
            .map(|img| capa::para_buffer(&img.to_rgba8()));
    }

    let opcoes = ParseOptions::new().read_properties(false);
    let embutida = Probe::open(caminho)
        .and_then(|p| p.options(opcoes).read())
        .ok()
        .and_then(|arquivo| {
            let tag = arquivo.primary_tag().or_else(|| arquivo.first_tag())?;
            capa::capa_da_tag(tag)
        });
    let reduzida = embutida
        .or_else(|| crate::ffmpeg::quadro_jpeg(caminho, LADO * 2))
        .or_else(|| capa::capa_da_pasta(caminho))
        .or_else(|| std::fs::read(web?).ok())
        .and_then(|b| image::load_from_memory(&b).ok())
        .map(|img| img.resize_to_fill(LADO, LADO, image::imageops::FilterType::Triangle));
    if let Some(arquivo) = &arquivo_cache {
        gravar(arquivo, reduzida.as_ref());
    }
    reduzida.map(|img| capa::para_buffer(&img.to_rgba8()))
}

/// Grava via arquivo temporário + rename: outra thread (ou outra instância)
/// nunca lê um JPEG pela metade.
fn gravar(arquivo: &Path, img: Option<&DynamicImage>) {
    let mut bytes = Vec::new();
    if let Some(img) = img {
        let rgb = img.to_rgb8();
        let codificador =
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, QUALIDADE_JPEG);
        if rgb.write_with_encoder(codificador).is_err() {
            return;
        }
    }
    let Some(pasta) = arquivo.parent() else {
        return;
    };
    if std::fs::create_dir_all(pasta).is_err() {
        return;
    }
    let temporario = arquivo.with_extension(format!("{}.tmp", std::process::id()));
    if std::fs::write(&temporario, &bytes).is_ok() && std::fs::rename(&temporario, arquivo).is_err()
    {
        let _ = std::fs::remove_file(&temporario);
    }
}

/// Marca a miniatura como usada agora: o `aparar_cache` apaga pela data de
/// modificação, e a de acesso nem sempre é gravada (`noatime`).
fn tocar(arquivo: &Path) {
    let _ = std::fs::File::options()
        .write(true)
        .open(arquivo)
        .and_then(|f| f.set_modified(SystemTime::now()));
}

/// Se o cache passou de `limite`, apaga as miniaturas usadas há mais tempo até
/// descer a 3/4 dele (folga para não limpar toda vez que o painel abre).
/// Chaves de arquivos que mudaram nunca mais são lidas, então saem primeiro.
fn aparar_cache(pasta: &Path, limite: u64) {
    let Ok(entradas) = std::fs::read_dir(pasta) else {
        return;
    };
    let mut arquivos: Vec<_> = entradas
        .flatten()
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            meta.is_file().then(|| {
                let usado = meta.modified().unwrap_or(UNIX_EPOCH);
                (usado, meta.len().max(BLOCO), e.path())
            })
        })
        .collect();
    let mut total: u64 = arquivos.iter().map(|(_, tamanho, _)| tamanho).sum();
    if total <= limite {
        return;
    }
    arquivos.sort_unstable_by_key(|(usado, _, _)| *usado);
    let alvo = limite / 4 * 3;
    for (_, tamanho, caminho) in arquivos {
        if total <= alvo {
            break;
        }
        if std::fs::remove_file(&caminho).is_ok() {
            total -= tamanho;
        }
    }
}

/// Hash curto de um texto (nomes de arquivo de cache).
pub fn hash_texto(texto: &str) -> String {
    let mut h = Fnv::default();
    h.escrever(texto.as_bytes());
    format!("{:016x}", h.0)
}

/// Nome do arquivo de cache: hash do caminho + data de modificação +
/// tamanho (+ a capa da web, quando ela existe). Trocou a capa (ou o
/// arquivo), muda a chave.
pub fn chave(caminho: &Path, web: Option<&Path>) -> Option<String> {
    let meta = std::fs::metadata(caminho).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos();
    let mut h = Fnv::default();
    h.escrever(caminho.to_string_lossy().as_bytes());
    h.escrever(&mtime.to_le_bytes());
    h.escrever(&meta.len().to_le_bytes());
    if let Some(web) = web.filter(|w| w.is_file()) {
        h.escrever(web.to_string_lossy().as_bytes());
    }
    Some(format!("{:016x}.jpg", h.0))
}

/// FNV-1a de 64 bits: estável entre versões do Rust, ao contrário do
/// `DefaultHasher`, então o cache sobrevive a uma atualização do furinar.
struct Fnv(u64);

impl Default for Fnv {
    fn default() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
}

impl Fnv {
    fn escrever(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

/// `~/.cache/furinar/capas` (XDG) ou `%LOCALAPPDATA%\furinar\capas`.
fn pasta_cache() -> Option<PathBuf> {
    pasta_cache_furinar().map(|b| b.join("capas"))
}

/// `~/.cache/furinar` (Linux) ou `%LOCALAPPDATA%\furinar` (Windows).
pub(crate) fn pasta_cache_furinar() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(not(target_os = "windows"))]
    let base = std::env::var_os("XDG_CACHE_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")));
    base.map(|b| b.join("furinar"))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);
    impl TempDir {
        fn novo(nome: &str) -> Self {
            let p = std::env::temp_dir().join(format!(
                "furinar-teste-miniaturas-{nome}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn fixture(nome: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(nome)
    }

    #[test]
    fn chave_muda_quando_o_arquivo_muda() {
        let tmp = TempDir::novo("chave");
        let a = tmp.0.join("a.mp3");
        std::fs::write(&a, b"um").unwrap();
        let antes = chave(&a, None).unwrap();
        assert_eq!(chave(&a, None).unwrap(), antes);
        std::fs::write(&a, b"outro conteudo").unwrap();
        assert_ne!(chave(&a, None).unwrap(), antes);
        assert!(chave(&tmp.0.join("nao-existe.mp3"), None).is_none());
    }

    #[test]
    fn capa_da_pasta_vai_para_o_cache_e_volta_de_la() {
        let tmp = TempDir::novo("cache");
        let musicas = tmp.0.join("musicas");
        let cache = tmp.0.join("cache");
        std::fs::create_dir_all(&musicas).unwrap();
        let faixa = musicas.join("tone.mp3");
        std::fs::copy(fixture("tone.mp3"), &faixa).unwrap();
        image::RgbImage::from_pixel(300, 200, image::Rgb([200, 30, 30]))
            .save(musicas.join("cover.png"))
            .unwrap();

        let capa = carregar(&faixa, None, Some(&cache)).expect("capa da pasta");
        assert_eq!((capa.width(), capa.height()), (LADO, LADO));

        // Sem o cover.png, a miniatura sai do cache
        std::fs::remove_file(musicas.join("cover.png")).unwrap();
        let de_novo = carregar(&faixa, None, Some(&cache)).expect("capa do cache");
        let px = de_novo.as_slice()[(LADO * LADO / 2) as usize];
        assert!(px.r > 150 && px.g < 80, "cor do cache: {px:?}");
    }

    #[test]
    fn faixa_sem_capa_fica_marcada_no_cache() {
        let tmp = TempDir::novo("sem-capa");
        let faixa = tmp.0.join("tone.mp3");
        std::fs::copy(fixture("tone.mp3"), &faixa).unwrap();
        let cache = tmp.0.join("cache");

        assert!(carregar(&faixa, None, Some(&cache)).is_none());
        let marcador = cache.join(chave(&faixa, None).unwrap());
        assert_eq!(std::fs::metadata(marcador).unwrap().len(), 0);
    }

    #[test]
    fn ler_do_cache_renova_a_data() {
        let tmp = TempDir::novo("renova");
        let faixa = tmp.0.join("tone.mp3");
        std::fs::copy(fixture("tone.mp3"), &faixa).unwrap();
        let cache = tmp.0.join("cache");
        carregar(&faixa, None, Some(&cache));
        let marcador = cache.join(chave(&faixa, None).unwrap());
        let antiga = UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
        std::fs::File::options()
            .write(true)
            .open(&marcador)
            .unwrap()
            .set_modified(antiga)
            .unwrap();

        carregar(&faixa, None, Some(&cache));
        let agora = std::fs::metadata(&marcador).unwrap().modified().unwrap();
        assert!(agora > antiga);
    }

    #[test]
    fn aparar_apaga_os_mais_antigos() {
        let tmp = TempDir::novo("aparar");
        let datar = |nome: &str, segundos: u64| {
            let p = tmp.0.join(nome);
            std::fs::write(&p, vec![0u8; 10_000]).unwrap();
            std::fs::File::options()
                .write(true)
                .open(&p)
                .unwrap()
                .set_modified(UNIX_EPOCH + std::time::Duration::from_secs(segundos))
                .unwrap();
        };
        // 4 arquivos de 10 KB; limite de 30 KB → desce até 22,5 KB: sobram 2
        datar("velho.jpg", 1_000);
        datar("medio.jpg", 2_000);
        datar("novo.jpg", 3_000);
        datar("mais-novo.jpg", 4_000);
        aparar_cache(&tmp.0, 30_000);
        let existe = |nome: &str| tmp.0.join(nome).exists();
        assert!(!existe("velho.jpg") && !existe("medio.jpg"));
        assert!(existe("novo.jpg") && existe("mais-novo.jpg"));

        // Abaixo do limite: nada some
        aparar_cache(&tmp.0, 30_000);
        assert!(existe("novo.jpg") && existe("mais-novo.jpg"));
    }

    #[test]
    fn carregador_entrega_e_encerra() {
        // Sem cache: o teste não escreve no ~/.cache de quem roda
        let carregador = Carregador::com_cache(None);
        carregador.pedir(vec![(fixture("tone.mp3"), None)]);
        let inicio = std::time::Instant::now();
        loop {
            if let Some((caminho, capa)) = carregador.recebidos().next() {
                assert_eq!(caminho, fixture("tone.mp3"));
                assert!(capa.is_none());
                break;
            }
            assert!(inicio.elapsed().as_secs() < 5, "carregador não respondeu");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}
