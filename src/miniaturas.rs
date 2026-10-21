//! Miniaturas de capa e metadados extras (álbum, duração) para o painel da
//! biblioteca do modo imersivo.
//!
//! Duas threads atendem uma fila em que o pedido novo substitui o anterior:
//! rolar a lista rápido não deixa trabalho velho na frente. As capas reduzidas
//! ficam em cache no disco, então a segunda visita a uma pasta não decodifica
//! capa nenhuma. As threads terminam quando o `Carregador` é solto.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};

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

pub struct Meta {
    pub album: Option<String>,
    pub duracao: Option<u64>,
    pub capa: Option<SharedPixelBuffer<Rgba8Pixel>>,
}

struct Fila {
    pendentes: VecDeque<PathBuf>,
    fim: bool,
}

type Compartilhada = Arc<(Mutex<Fila>, Condvar)>;

pub struct Carregador {
    fila: Compartilhada,
    rx: Receiver<(PathBuf, Meta)>,
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
    pub fn pedir(&self, caminhos: Vec<PathBuf>) {
        let (fila, sinal) = &*self.fila;
        if let Ok(mut f) = fila.lock() {
            f.pendentes = caminhos.into();
            sinal.notify_all();
        }
    }

    pub fn recebidos(&self) -> impl Iterator<Item = (PathBuf, Meta)> + '_ {
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

fn trabalhar(fila: &Compartilhada, tx: &Sender<(PathBuf, Meta)>, cache: Option<&Path>) {
    let (trava, sinal) = &**fila;
    loop {
        let caminho = {
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
        let meta = carregar(&caminho, cache);
        if tx.send((caminho, meta)).is_err() {
            return;
        }
    }
}

/// Lê álbum e duração e acha a miniatura: do cache, ou da capa embutida /
/// `cover.jpg` (e então grava no cache). Arquivo de cache vazio quer dizer
/// "essa faixa não tem capa", para não procurar de novo.
pub fn carregar(caminho: &Path, cache: Option<&Path>) -> Meta {
    let arquivo_cache = cache.and_then(|pasta| chave(caminho).map(|k| pasta.join(k)));
    let em_cache = arquivo_cache.as_ref().and_then(|a| std::fs::read(a).ok());

    let opcoes = ParseOptions::new().read_cover_art(em_cache.is_none());
    let lido = Probe::open(caminho).and_then(|p| p.options(opcoes).read());
    let (mut album, mut duracao, mut embutida) = (None, None, None);
    if let Ok(arquivo) = &lido {
        duracao = Some(arquivo.properties().duration().as_secs()).filter(|&d| d > 0);
        if let Some(tag) = arquivo.primary_tag().or_else(|| arquivo.first_tag()) {
            album = tag
                .album()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            embutida = capa::capa_da_tag(tag);
        }
    }

    let capa = match em_cache {
        Some(bytes) if bytes.is_empty() => None,
        Some(bytes) => image::load_from_memory_with_format(&bytes, ImageFormat::Jpeg)
            .ok()
            .map(|img| capa::para_buffer(&img.to_rgba8())),
        None => {
            let reduzida = embutida
                .or_else(|| capa::capa_da_pasta(caminho))
                .and_then(|b| image::load_from_memory(&b).ok())
                .map(|img| img.resize_to_fill(LADO, LADO, image::imageops::FilterType::Triangle));
            if let Some(arquivo) = &arquivo_cache {
                gravar(arquivo, reduzida.as_ref());
            }
            reduzida.map(|img| capa::para_buffer(&img.to_rgba8()))
        }
    };

    Meta {
        album,
        duracao,
        capa,
    }
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

/// Nome do arquivo de cache: hash do caminho + data de modificação +
/// tamanho. Trocou a capa (ou o arquivo), muda a chave.
pub fn chave(caminho: &Path) -> Option<String> {
    let meta = std::fs::metadata(caminho).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    let mut h = Fnv::default();
    h.escrever(caminho.to_string_lossy().as_bytes());
    h.escrever(&mtime.to_le_bytes());
    h.escrever(&meta.len().to_le_bytes());
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
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(not(target_os = "windows"))]
    let base = std::env::var_os("XDG_CACHE_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")));
    base.map(|b| b.join("furinar").join("capas"))
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
        let antes = chave(&a).unwrap();
        assert_eq!(chave(&a).unwrap(), antes);
        std::fs::write(&a, b"outro conteudo").unwrap();
        assert_ne!(chave(&a).unwrap(), antes);
        assert!(chave(&tmp.0.join("nao-existe.mp3")).is_none());
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

        let meta = carregar(&faixa, Some(&cache));
        let capa = meta.capa.expect("capa da pasta");
        assert_eq!((capa.width(), capa.height()), (LADO, LADO));

        // Sem o cover.png, a miniatura sai do cache
        std::fs::remove_file(musicas.join("cover.png")).unwrap();
        let de_novo = carregar(&faixa, Some(&cache)).capa.expect("capa do cache");
        let px = de_novo.as_slice()[(LADO * LADO / 2) as usize];
        assert!(px.r > 150 && px.g < 80, "cor do cache: {px:?}");
    }

    #[test]
    fn faixa_sem_capa_fica_marcada_no_cache() {
        let tmp = TempDir::novo("sem-capa");
        let faixa = tmp.0.join("tone.mp3");
        std::fs::copy(fixture("tone.mp3"), &faixa).unwrap();
        let cache = tmp.0.join("cache");

        assert!(carregar(&faixa, Some(&cache)).capa.is_none());
        let marcador = cache.join(chave(&faixa).unwrap());
        assert_eq!(std::fs::metadata(marcador).unwrap().len(), 0);
    }

    #[test]
    fn carregador_entrega_e_encerra() {
        // Sem cache: o teste não escreve no ~/.cache de quem roda
        let carregador = Carregador::com_cache(None);
        carregador.pedir(vec![fixture("tone.mp3")]);
        let inicio = std::time::Instant::now();
        loop {
            if let Some((caminho, meta)) = carregador.recebidos().next() {
                assert_eq!(caminho, fixture("tone.mp3"));
                assert!(meta.capa.is_none());
                break;
            }
            assert!(inicio.elapsed().as_secs() < 5, "carregador não respondeu");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}
