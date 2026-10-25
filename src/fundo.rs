//! Fundos do modo imersivo: imagem, GIF/WebP animado ou vídeo.
//!
//! Cada fundo toca numa thread produtora que manda quadros prontos por um
//! canal limitado: nunca há mais que alguns quadros na memória, e soltar o
//! `Reprodutor` encerra a thread (o próximo envio falha) e o `ffmpeg`.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::time::{Duration, Instant};

use image::codecs::gif::GifDecoder;
use image::codecs::webp::WebPDecoder;
use image::{AnimationDecoder, RgbaImage};
use slint::{Rgba8Pixel, SharedPixelBuffer};

use crate::ffmpeg::{self, comando};

/// Nomes procurados na pasta da música (sem extensão).
const NOMES_FUNDO: [&str; 2] = ["background", "furinar-bg"];
const EXT_IMAGEM: [&str; 4] = ["png", "jpg", "jpeg", "bmp"];
const EXT_ANIMADA: [&str; 2] = ["gif", "webp"];
const EXT_VIDEO: [&str; 5] = ["mp4", "webm", "mkv", "mov", "avi"];

/// Largura máxima dos quadros. Pixel art costuma ser menor que isso e vai
/// intacta; o resto é reduzido (a UI amplia de volta).
const LARGURA_MAX: u32 = 1280;
/// Fundo estático pode ser maior: é decodificado uma vez só.
const LARGURA_MAX_ESTATICO: u32 = 2560;
/// Abaixo disso o fundo é tratado como pixel art: ampliado sem suavizar.
pub const LARGURA_PIXEL_ART: u32 = 640;
const FPS_VIDEO: u32 = 24;
/// Atraso mínimo entre quadros: teto de ~30 fps (GIF com atraso 0 viraria
/// 100 fps, e cada quadro repinta a tela inteira no renderer de software).
const ATRASO_MIN: Duration = Duration::from_millis(33);
/// Vídeo da faixa: distância entre a música e o quadro que, passada, é
/// tratada como seek (o `ffmpeg` reabre na posição nova).
const SALTO_SINCRONIA: Duration = Duration::from_millis(1500);

/// Extensões aceitas no seletor de arquivo das configurações.
pub fn extensoes_aceitas() -> Vec<&'static str> {
    EXT_IMAGEM
        .iter()
        .chain(&EXT_ANIMADA)
        .chain(&EXT_VIDEO)
        .copied()
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tipo {
    Imagem,
    Animada,
    Video,
}

fn tipo(caminho: &Path) -> Option<Tipo> {
    let ext = caminho.extension()?.to_str()?.to_lowercase();
    if EXT_IMAGEM.contains(&ext.as_str()) {
        Some(Tipo::Imagem)
    } else if EXT_ANIMADA.contains(&ext.as_str()) {
        Some(Tipo::Animada)
    } else if EXT_VIDEO.contains(&ext.as_str()) {
        Some(Tipo::Video)
    } else {
        None
    }
}

/// `background`, `background2`, `background-chuva`, `furinar-bg_1`...: o
/// nome-base, seguido ou não de um sufixo depois de dígito, `-`, `_` ou espaço.
fn e_nome_de_fundo(stem: &str) -> bool {
    let stem = stem.to_lowercase();
    NOMES_FUNDO.iter().any(|n| match stem.strip_prefix(n) {
        Some("") => true,
        Some(resto) => resto.starts_with(|c: char| c.is_ascii_digit() || "-_ ".contains(c)),
        None => false,
    })
}

/// Fundos possíveis para a faixa: os `background*`/`furinar-bg*` da pasta da
/// faixa; senão os da pasta raiz da aba; senão os globais das configurações.
/// Mais de um = rodízio, um diferente a cada faixa (`escolher`).
pub fn resolver(pasta_faixa: &Path, pasta_raiz: &Path, globais: &[PathBuf]) -> Vec<PathBuf> {
    let na_pasta = |pasta: &Path| -> Vec<PathBuf> {
        let Ok(entradas) = std::fs::read_dir(pasta) else {
            return Vec::new();
        };
        let mut achados: Vec<PathBuf> = entradas
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.is_file()
                    && tipo(p).is_some()
                    && p.file_stem()
                        .and_then(|s| s.to_str())
                        .is_some_and(e_nome_de_fundo)
            })
            .collect();
        // Determinístico: o rodízio anda sempre na mesma ordem
        achados.sort();
        achados
    };

    let mut achados = na_pasta(pasta_faixa);
    if achados.is_empty() && pasta_raiz != pasta_faixa {
        achados = na_pasta(pasta_raiz);
    }
    if achados.is_empty() {
        achados = globais.iter().filter(|g| g.is_file()).cloned().collect();
    }
    achados
}

/// Fundo da próxima faixa entre os `candidatos`. Com um só, é ele; com mais,
/// sorteia (`sorteio`, qualquer número) um que não seja o `atual`, para o
/// fundo trocar de verdade a cada faixa.
pub fn escolher(candidatos: &[PathBuf], atual: Option<&Path>, sorteio: usize) -> Option<PathBuf> {
    let outros: Vec<&PathBuf> = candidatos
        .iter()
        .filter(|c| candidatos.len() == 1 || Some(c.as_path()) != atual)
        .collect();
    outros
        .get(sorteio % outros.len().max(1))
        .map(|c| (*c).clone())
}

pub struct Quadro {
    pub pixels: SharedPixelBuffer<Rgba8Pixel>,
    /// `None`: imagem estática, não vem outro quadro.
    pub atraso: Option<Duration>,
    /// Instante do quadro no vídeo da faixa (só no modo sincronizado).
    pub pts: Option<Duration>,
}

/// Toca um fundo: a thread produz, `proximo` entrega no ritmo certo.
pub struct Reprodutor {
    pub caminho: PathBuf,
    /// `None` só no vídeo da faixa ainda não iniciado (espera a posição).
    rx: Option<Receiver<Quadro>>,
    /// Quando o próximo quadro pode ser mostrado.
    prazo: Instant,
    terminou: bool,
    /// Vídeo da própria faixa: segue a posição da música, não o relógio.
    sincronizado: bool,
    /// Quadro já recebido, mas adiante da música.
    pendente: Option<Quadro>,
    /// `pts` do último quadro entregue.
    ultimo_pts: Option<Duration>,
}

impl Reprodutor {
    /// `None` se o tipo não é suportado (ou vídeo sem `ffmpeg` no PATH).
    pub fn abrir(caminho: &Path) -> Option<Self> {
        let tipo = tipo(caminho)?;
        if tipo == Tipo::Video && !ffmpeg::disponivel() {
            return None;
        }

        // Capacidade 2: um quadro na tela, um no canal, um sendo decodificado
        let (tx, rx) = mpsc::sync_channel(2);
        let dono = caminho.to_path_buf();
        std::thread::Builder::new()
            .name("furinar-fundo".into())
            .spawn(move || match tipo {
                Tipo::Imagem => produzir_imagem(&dono, &tx),
                Tipo::Animada => produzir_animacao(&dono, &tx),
                Tipo::Video => produzir_video(&dono, &tx, None),
            })
            .ok()?;

        Some(Self {
            caminho: caminho.to_path_buf(),
            rx: Some(rx),
            prazo: Instant::now(),
            terminou: false,
            sincronizado: false,
            pendente: None,
            ultimo_pts: None,
        })
    }

    /// O vídeo da própria faixa, em sincronia com a música: o `ffmpeg` só sobe
    /// no primeiro `proximo`, já na posição em que a música está.
    pub fn abrir_faixa(caminho: &Path) -> Option<Self> {
        if !ffmpeg::disponivel() || !ffmpeg::e_video(caminho) {
            return None;
        }
        Some(Self {
            caminho: caminho.to_path_buf(),
            rx: None,
            prazo: Instant::now(),
            terminou: false,
            sincronizado: true,
            pendente: None,
            ultimo_pts: None,
        })
    }

    /// (Re)inicia o vídeo da faixa em `posicao`.
    fn iniciar_em(&mut self, posicao: Duration) {
        let (tx, rx) = mpsc::sync_channel(2);
        let dono = self.caminho.clone();
        let lancou = std::thread::Builder::new()
            .name("furinar-fundo".into())
            .spawn(move || produzir_video(&dono, &tx, Some(posicao)));
        self.rx = lancou.is_ok().then_some(rx);
        self.pendente = None;
        self.ultimo_pts = None;
        self.terminou = self.rx.is_none();
    }

    /// Próximo quadro, se já deu a hora dele. Barato quando não há nada.
    /// `posicao` é onde a música está; só conta no vídeo da faixa.
    pub fn proximo(&mut self, agora: Instant, posicao: Option<Duration>) -> Option<Quadro> {
        if self.sincronizado {
            return self.proximo_sincronizado(posicao?);
        }
        if self.terminou || agora < self.prazo {
            return None;
        }
        match self.rx.as_ref()?.try_recv() {
            Ok(q) => {
                match q.atraso {
                    Some(a) => self.prazo = agora + a.max(ATRASO_MIN),
                    None => self.terminou = true,
                }
                Some(q)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.terminou = true;
                None
            }
        }
    }

    /// O quadro mais recente que não passa da música. Pausada, nada é lido:
    /// o canal enche e o `ffmpeg` fica parado no pipe.
    fn proximo_sincronizado(&mut self, posicao: Duration) -> Option<Quadro> {
        let referencia = self
            .pendente
            .as_ref()
            .and_then(|q| q.pts)
            .or(self.ultimo_pts);
        let saltou = referencia.is_some_and(|r| r.abs_diff(posicao) > SALTO_SINCRONIA);
        if self.rx.is_none() || saltou {
            self.iniciar_em(posicao);
            return None;
        }

        let mut melhor = None;
        loop {
            let q = match self.pendente.take() {
                Some(q) => q,
                None => match self.rx.as_ref()?.try_recv() {
                    Ok(q) => q,
                    Err(TryRecvError::Empty) => break,
                    // Vídeo acabou antes da música: fica o último quadro
                    Err(TryRecvError::Disconnected) => {
                        self.terminou = true;
                        break;
                    }
                },
            };
            if q.pts.unwrap_or_default() > posicao {
                self.pendente = Some(q);
                break;
            }
            melhor = Some(q);
        }
        if let Some(q) = &melhor {
            self.ultimo_pts = q.pts;
        }
        melhor
    }
}

fn para_buffer(img: &RgbaImage) -> SharedPixelBuffer<Rgba8Pixel> {
    SharedPixelBuffer::clone_from_slice(img.as_raw(), img.width(), img.height())
}

fn reduzir(img: RgbaImage, largura_max: u32) -> RgbaImage {
    if img.width() <= largura_max {
        return img;
    }
    let altura = (img.height() as u64 * largura_max as u64 / img.width() as u64).max(1) as u32;
    image::imageops::resize(
        &img,
        largura_max,
        altura,
        image::imageops::FilterType::Triangle,
    )
}

fn produzir_imagem(caminho: &Path, tx: &SyncSender<Quadro>) {
    if let Ok(img) = image::open(caminho) {
        let img = reduzir(img.into_rgba8(), LARGURA_MAX_ESTATICO);
        let _ = tx.send(Quadro {
            pixels: para_buffer(&img),
            atraso: None,
            pts: None,
        });
    }
}

/// GIF/WebP em loop, reabrindo o arquivo a cada volta: só os quadros em
/// trânsito ficam na memória, nunca a animação inteira.
fn produzir_animacao(caminho: &Path, tx: &SyncSender<Quadro>) {
    let ext = caminho
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    loop {
        let Ok(arquivo) = File::open(caminho) else {
            return;
        };
        let leitor = BufReader::new(arquivo);
        let quadros = if ext == "gif" {
            GifDecoder::new(leitor).map(|d| d.into_frames())
        } else {
            match WebPDecoder::new(leitor) {
                Ok(d) if d.has_animation() => Ok(d.into_frames()),
                // WebP estático: cai no caminho de imagem
                _ => return produzir_imagem(caminho, tx),
            }
        };
        let Ok(quadros) = quadros else {
            return;
        };

        let mut enviados = 0usize;
        for quadro in quadros {
            let Ok(quadro) = quadro else {
                break;
            };
            let (num, den) = quadro.delay().numer_denom_ms();
            let atraso = Duration::from_millis(u64::from(num) / u64::from(den.max(1)));
            let img = reduzir(quadro.into_buffer(), LARGURA_MAX);
            let q = Quadro {
                pixels: para_buffer(&img),
                atraso: Some(atraso),
                pts: None,
            };
            if tx.send(q).is_err() {
                return; // Reprodutor solto: modo fechado ou fundo trocado
            }
            enviados += 1;
        }

        // Um quadro só: na prática é uma imagem estática
        if enviados <= 1 {
            return;
        }
    }
}

/// Largura e altura do primeiro stream de vídeo.
fn dimensoes_video(caminho: &Path) -> Option<(u32, u32)> {
    let saida = comando("ffprobe")
        .args(["-v", "error", "-select_streams", "v:0"])
        .args(["-show_entries", "stream=width,height", "-of", "csv=p=0:s=x"])
        .arg(caminho)
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let texto = String::from_utf8_lossy(&saida.stdout);
    let (w, h) = texto.trim().split_once('x')?;
    Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
}

/// Dimensões de saída: no máximo `LARGURA_MAX`, proporção mantida, pares
/// (exigência de vários filtros do ffmpeg).
fn dimensoes_saida(largura: u32, altura: u32) -> (u32, u32) {
    let w = largura.clamp(2, LARGURA_MAX) & !1;
    let h = ((altura as u64 * w as u64 / largura.max(1) as u64) as u32).max(2) & !1;
    (w, h)
}

/// Vídeo via `ffmpeg` do sistema: quadros RGBA crus por pipe. Sem `inicio`,
/// em loop (fundo); com, a partir dali e uma vez só, com o `pts` de cada
/// quadro (vídeo da faixa).
fn produzir_video(caminho: &Path, tx: &SyncSender<Quadro>, inicio: Option<Duration>) {
    let Some((w0, h0)) = dimensoes_video(caminho) else {
        return;
    };
    let (w, h) = dimensoes_saida(w0, h0);

    let mut cmd = comando("ffmpeg");
    cmd.args(["-v", "error", "-nostdin"]);
    match inicio {
        Some(t) => cmd.arg("-ss").arg(format!("{:.3}", t.as_secs_f64())),
        None => cmd.args(["-stream_loop", "-1"]),
    };
    let Ok(mut filho) = cmd
        .arg("-i")
        .arg(caminho)
        // fps antes do scale: quadros descartados nem chegam a ser escalados
        .args([
            "-an",
            "-vf",
            &format!("fps={FPS_VIDEO},scale={w}:{h}:flags=fast_bilinear"),
        ])
        .args(["-f", "rawvideo", "-pix_fmt", "rgba", "-"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return;
    };

    let atraso = Duration::from_secs(1) / FPS_VIDEO;
    if let Some(mut saida) = filho.stdout.take() {
        let mut bruto = vec![0u8; (w * h * 4) as usize];
        for n in 0u32.. {
            if saida.read_exact(&mut bruto).is_err() {
                break;
            }
            let q = Quadro {
                pixels: SharedPixelBuffer::clone_from_slice(&bruto, w, h),
                atraso: Some(atraso),
                pts: inicio.map(|t| t + atraso * n),
            };
            if tx.send(q).is_err() {
                break;
            }
        }
    }
    let _ = filho.kill();
    let _ = filho.wait();
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::codecs::gif::GifEncoder;
    use image::{Delay, Frame, Rgba};

    /// Diretório temporário próprio do teste, apagado no fim.
    struct TempDir(PathBuf);
    impl TempDir {
        fn novo(nome: &str) -> Self {
            let p =
                std::env::temp_dir().join(format!("furinar-teste-{nome}-{}", std::process::id()));
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

    #[test]
    fn resolver_prefere_pasta_da_faixa_depois_raiz_depois_global() {
        let raiz = TempDir::novo("resolver");
        let album = raiz.0.join("album");
        std::fs::create_dir_all(&album).unwrap();
        let global = raiz.0.join("global.png");
        std::fs::write(&global, b"x").unwrap();

        let globais = [global.clone()];

        // Só o global
        assert_eq!(resolver(&album, &raiz.0, &globais), globais);

        // Raiz ganha do global
        let na_raiz = raiz.0.join("furinar-bg.gif");
        std::fs::write(&na_raiz, b"x").unwrap();
        assert_eq!(resolver(&album, &raiz.0, &globais), [na_raiz]);

        // Pasta da faixa ganha de tudo; maiúsculas não importam; vários
        // entram no rodízio, em ordem
        let no_album = album.join("Background.MP4");
        let segundo = album.join("background-2.png");
        std::fs::write(&no_album, b"x").unwrap();
        std::fs::write(&segundo, b"x").unwrap();
        std::fs::write(album.join("backgroundsss.png"), b"x").unwrap();
        assert_eq!(resolver(&album, &raiz.0, &globais), [no_album, segundo]);
    }

    #[test]
    fn rodizio_troca_de_fundo_a_cada_faixa() {
        let a = PathBuf::from("a.png");
        let b = PathBuf::from("b.png");
        let c = PathBuf::from("c.png");
        // Um só: sempre ele, mesmo sendo o atual
        assert_eq!(
            escolher(std::slice::from_ref(&a), Some(&a), 7),
            Some(a.clone())
        );
        // Vários: nunca repete o atual
        let todos = [a.clone(), b.clone(), c.clone()];
        for sorteio in 0..10 {
            assert_ne!(escolher(&todos, Some(&b), sorteio), Some(b.clone()));
        }
        assert_eq!(escolher(&todos, None, 4), Some(b));
        assert_eq!(escolher(&[], None, 0), None);
    }

    #[test]
    fn resolver_ignora_extensao_desconhecida_e_global_inexistente() {
        let raiz = TempDir::novo("ignora");
        std::fs::write(raiz.0.join("background.txt"), b"x").unwrap();
        let fantasma = raiz.0.join("nao-existe.png");
        assert!(resolver(&raiz.0, &raiz.0, &[fantasma]).is_empty());
    }

    #[test]
    fn dimensoes_saida_limita_e_mantem_par() {
        assert_eq!(dimensoes_saida(1920, 1080), (1280, 720));
        assert_eq!(dimensoes_saida(641, 361), (640, 360));
        assert_eq!(dimensoes_saida(320, 180), (320, 180));
    }

    #[test]
    fn gif_animado_toca_em_loop_com_atraso() {
        let dir = TempDir::novo("gif");
        let caminho = dir.0.join("background.gif");
        {
            let arquivo = File::create(&caminho).unwrap();
            let mut enc = GifEncoder::new(arquivo);
            for cor in [[255, 0, 0, 255], [0, 0, 255, 255]] {
                let img = RgbaImage::from_pixel(8, 4, Rgba(cor));
                enc.encode_frame(Frame::from_parts(
                    img,
                    0,
                    0,
                    Delay::from_numer_denom_ms(50, 1),
                ))
                .unwrap();
            }
        }

        let mut r = Reprodutor::abrir(&caminho).expect("gif deve abrir");
        let mut cores = Vec::new();
        let inicio = Instant::now();
        // 2 quadros por volta; 3 quadros provam que o loop reabre o arquivo
        while cores.len() < 3 && inicio.elapsed() < Duration::from_secs(5) {
            if let Some(q) = r.proximo(Instant::now() + Duration::from_secs(60), None) {
                assert_eq!((q.pixels.width(), q.pixels.height()), (8, 4));
                assert!(q.atraso.is_some_and(|a| a >= Duration::from_millis(40)));
                cores.push(q.pixels.as_slice()[0].r);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(cores, vec![255, 0, 255]);
    }

    /// Vídeo da faixa alimentado por um canal falso, sem ffmpeg.
    fn sincronizado(rx: Receiver<Quadro>) -> Reprodutor {
        Reprodutor {
            caminho: PathBuf::from("clipe.mp4"),
            rx: Some(rx),
            prazo: Instant::now(),
            terminou: false,
            sincronizado: true,
            pendente: None,
            ultimo_pts: None,
        }
    }

    fn quadro_em(ms: u64) -> Quadro {
        Quadro {
            pixels: SharedPixelBuffer::new(2, 2),
            atraso: Some(Duration::from_millis(42)),
            pts: Some(Duration::from_millis(ms)),
        }
    }

    #[test]
    fn video_da_faixa_segue_a_posicao_da_musica() {
        let (tx, rx) = mpsc::sync_channel(8);
        let mut r = sincronizado(rx);
        let ms = Duration::from_millis;
        for t in [0, 42, 84, 126, 168] {
            tx.send(quadro_em(t)).unwrap();
        }
        let pts = |q: Option<Quadro>| q.and_then(|q| q.pts);

        // Sem posição (nada tocando): nada sai
        assert!(r.proximo(Instant::now(), None).is_none());
        // Aos 100 ms: os atrasados são pulados, sai o de 84
        assert_eq!(pts(r.proximo(Instant::now(), Some(ms(100)))), Some(ms(84)));
        // Música parada: o mesmo instante não entrega outro
        assert!(r.proximo(Instant::now(), Some(ms(100))).is_none());
        assert_eq!(pts(r.proximo(Instant::now(), Some(ms(130)))), Some(ms(126)));

        // Seek para longe: reabre (aqui, sem ffmpeg de verdade) em vez de
        // drenar o canal até lá
        assert!(r.proximo(Instant::now(), Some(ms(60_000))).is_none());
        assert!(r.pendente.is_none() && r.ultimo_pts.is_none());
    }

    #[test]
    fn imagem_estatica_manda_um_quadro_e_termina() {
        let dir = TempDir::novo("png");
        let caminho = dir.0.join("background.png");
        RgbaImage::from_pixel(4, 4, Rgba([1, 2, 3, 255]))
            .save(&caminho)
            .unwrap();

        let mut r = Reprodutor::abrir(&caminho).expect("png deve abrir");
        let inicio = Instant::now();
        let q = loop {
            if let Some(q) = r.proximo(Instant::now(), None) {
                break q;
            }
            assert!(inicio.elapsed() < Duration::from_secs(5), "sem quadro");
            std::thread::sleep(Duration::from_millis(5));
        };
        assert!(q.atraso.is_none());
        assert!(
            r.proximo(Instant::now() + Duration::from_secs(60), None)
                .is_none()
        );
    }
}
