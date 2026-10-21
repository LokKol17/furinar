//! Capa e metadados da faixa atual, para o modo imersivo.
//!
//! Só a faixa que está tocando passa por aqui: o scan da biblioteca continua
//! lendo apenas título/artista, sem capa nem propriedades.

use std::path::Path;

use image::RgbaImage;
use lofty::config::ParseOptions;
use lofty::picture::PictureType;
use lofty::prelude::*;
use lofty::probe::Probe;
use slint::{Rgba8Pixel, SharedPixelBuffer};

/// Lado máximo da capa exibida no cartão.
const LADO_CAPA: u32 = 512;
/// Lado da capa usada como fundo desfocado. Pequena (≈150 KB) mas não demais:
/// abaixo disso a ampliação da UI deixa blocos visíveis no degradê.
const LADO_DESFOCADA: u32 = 192;
/// Lado da amostra usada para achar a cor dominante.
const LADO_AMOSTRA_COR: u32 = 32;

/// Nomes de arquivo procurados na pasta quando a faixa não tem capa embutida.
const NOMES_CAPA: [&str; 4] = ["cover", "folder", "front", "album"];
const EXTENSOES_IMAGEM: [&str; 4] = ["jpg", "jpeg", "png", "webp"];

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Detalhes {
    pub album: Option<String>,
    pub ano: Option<u16>,
    /// Extensão em maiúsculas ("MP3", "FLAC"...).
    pub formato: String,
    pub taxa_hz: Option<u32>,
    pub canais: Option<u8>,
}

/// Tudo que o modo imersivo precisa da faixa, pronto para virar `slint::Image`
/// na thread da UI. `SharedPixelBuffer` é `Send`; `slint::Image` não é.
pub struct Carregado {
    pub detalhes: Detalhes,
    pub capa: Option<SharedPixelBuffer<Rgba8Pixel>>,
    pub desfocada: Option<SharedPixelBuffer<Rgba8Pixel>>,
    /// Cor dominante da capa (r, g, b), já ajustada para contraste.
    pub cor: Option<(u8, u8, u8)>,
}

/// Lê metadados e capa da faixa e prepara as imagens. Feito para rodar fora
/// da thread da UI: decodificar uma capa grande leva dezenas de ms.
pub fn carregar(caminho: &Path) -> Carregado {
    let (detalhes, embutida) = ler_tags(caminho);

    let bytes = embutida.or_else(|| capa_da_pasta(caminho));
    let imagem = bytes
        .and_then(|b| image::load_from_memory(&b).ok())
        .map(|img| img.thumbnail(LADO_CAPA, LADO_CAPA).into_rgba8());

    let (capa, desfocada, cor) = match imagem {
        Some(img) => {
            let reduzida = image::imageops::thumbnail(&img, LADO_DESFOCADA, LADO_DESFOCADA);
            let desfocada = image::imageops::fast_blur(&reduzida, 12.0);
            let amostra = image::imageops::thumbnail(&img, LADO_AMOSTRA_COR, LADO_AMOSTRA_COR);
            (
                Some(para_buffer(&img)),
                Some(para_buffer(&desfocada)),
                cor_dominante(&amostra),
            )
        }
        None => (None, None, None),
    };

    Carregado {
        detalhes,
        capa,
        desfocada,
        cor,
    }
}

fn para_buffer(img: &RgbaImage) -> SharedPixelBuffer<Rgba8Pixel> {
    SharedPixelBuffer::clone_from_slice(img.as_raw(), img.width(), img.height())
}

/// Metadados + bytes da capa embutida (preferindo a frontal).
fn ler_tags(caminho: &Path) -> (Detalhes, Option<Vec<u8>>) {
    let mut detalhes = Detalhes {
        formato: caminho
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_uppercase(),
        ..Default::default()
    };

    let opcoes = ParseOptions::new().read_cover_art(true);
    let Ok(arquivo) = Probe::open(caminho).and_then(|p| p.options(opcoes).read()) else {
        return (detalhes, None);
    };

    let propriedades = arquivo.properties();
    detalhes.taxa_hz = propriedades.sample_rate();
    detalhes.canais = propriedades.channels();

    let mut capa = None;
    if let Some(tag) = arquivo.primary_tag().or_else(|| arquivo.first_tag()) {
        detalhes.album = tag
            .album()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        detalhes.ano = tag.date().map(|d| d.year).filter(|&a| a > 0);

        let fotos = tag.pictures();
        capa = fotos
            .iter()
            .find(|p| p.pic_type() == PictureType::CoverFront)
            .or_else(|| fotos.first())
            .map(|p| p.data().to_vec());
    }

    (detalhes, capa)
}

/// `cover.jpg`, `folder.png`... na pasta da faixa.
fn capa_da_pasta(caminho_faixa: &Path) -> Option<Vec<u8>> {
    let pasta = caminho_faixa.parent()?;
    NOMES_CAPA.iter().find_map(|nome| {
        EXTENSOES_IMAGEM.iter().find_map(|ext| {
            [ext.to_string(), ext.to_uppercase()]
                .iter()
                .find_map(|e| std::fs::read(pasta.join(format!("{nome}.{e}"))).ok())
        })
    })
}

/// Linha técnica: "MP3 • 48,0 kHz • Estéreo".
pub fn linha_tecnica(d: &Detalhes, lang: &crate::translations::Translations) -> String {
    let mut partes = Vec::new();
    if !d.formato.is_empty() {
        partes.push(d.formato.clone());
    }
    if let Some(hz) = d.taxa_hz {
        let khz = format!("{:.1} kHz", hz as f32 / 1000.0);
        // pt-br usa vírgula decimal
        partes.push(if lang.get("decimal_sep") == Some(&",") {
            khz.replace('.', ",")
        } else {
            khz
        });
    }
    match d.canais {
        Some(1) => partes.push(lang.get("mono").copied().unwrap_or("Mono").to_string()),
        Some(2) => partes.push(lang.get("stereo").copied().unwrap_or("Stereo").to_string()),
        Some(n) if n > 2 => partes.push(format!("{n} ch")),
        _ => {}
    }
    partes.join(" • ")
}

/// Linha de créditos: "artista • álbum • ano", pulando o que faltar.
pub fn linha_creditos(artista: Option<&str>, d: &Detalhes) -> String {
    let ano = d.ano.map(|a| a.to_string());
    [artista, d.album.as_deref(), ano.as_deref()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" • ")
}

/// Cor dominante: histograma de matiz em 24 baldes, cada pixel pesando
/// saturação × valor (cinzas e pretos quase não contam). Da média do balde
/// vencedor sai uma cor com brilho ajustado para destacar sobre fundo escuro.
pub fn cor_dominante(img: &RgbaImage) -> Option<(u8, u8, u8)> {
    const BALDES: usize = 24;
    let mut peso = [0f32; BALDES];
    let mut soma = [[0f32; 3]; BALDES];

    for p in img.pixels() {
        let [r, g, b, a] = p.0;
        if a < 128 {
            continue;
        }
        let (h, s, v) = rgb_para_hsv(r, g, b);
        let w = s * v;
        if w < 0.04 {
            continue;
        }
        let balde = ((h / 360.0) * BALDES as f32) as usize % BALDES;
        peso[balde] += w;
        soma[balde][0] += r as f32 * w;
        soma[balde][1] += g as f32 * w;
        soma[balde][2] += b as f32 * w;
    }

    let (melhor, &p) = peso.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1))?;
    if p <= 0.0 {
        return None;
    }
    let media = soma[melhor].map(|c| c / p);
    let (h, s, _) = rgb_para_hsv(media[0] as u8, media[1] as u8, media[2] as u8);
    // Saturação mínima e valor fixo alto: a cor vira destaque legível
    Some(hsv_para_rgb(h, s.max(0.45), 0.92))
}

fn rgb_para_hsv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / d).rem_euclid(6.0))
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    let s = if max == 0.0 { 0.0 } else { d / max };
    (h, s, max)
}

fn hsv_para_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as u32 % 6 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let f = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (f(r), f(g), f(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn imagem(f: impl Fn(u32, u32) -> [u8; 4]) -> RgbaImage {
        RgbaImage::from_fn(32, 32, |x, y| Rgba(f(x, y)))
    }

    #[test]
    fn cor_dominante_acha_o_vermelho_entre_cinzas() {
        // Um quarto vermelho, o resto cinza: o cinza não pode vencer
        let img = imagem(|x, y| {
            if x < 16 && y < 16 {
                [220, 30, 40, 255]
            } else {
                [128, 128, 128, 255]
            }
        });
        let (r, g, b) = cor_dominante(&img).expect("deve achar cor");
        assert!(r > 180 && g < 100 && b < 110, "veio ({r},{g},{b})");
    }

    #[test]
    fn cor_dominante_de_imagem_cinza_e_none() {
        let img = imagem(|_, _| [90, 90, 90, 255]);
        assert_eq!(cor_dominante(&img), None);
    }

    #[test]
    fn cor_dominante_sempre_clara_o_bastante() {
        // Azul bem escuro ainda vira um destaque legível
        let img = imagem(|_, _| [10, 20, 90, 255]);
        let (r, g, b) = cor_dominante(&img).expect("deve achar cor");
        assert!(r.max(g).max(b) > 200, "veio ({r},{g},{b})");
    }

    #[test]
    fn linha_creditos_pula_o_que_falta() {
        let d = Detalhes {
            album: Some("KAF+YOU".into()),
            ano: Some(2021),
            ..Default::default()
        };
        assert_eq!(
            linha_creditos(Some("Chinozo"), &d),
            "Chinozo • KAF+YOU • 2021"
        );
        assert_eq!(linha_creditos(None, &Detalhes::default()), "");
    }

    #[test]
    fn linha_tecnica_formata_por_idioma() {
        let d = Detalhes {
            formato: "MP3".into(),
            taxa_hz: Some(48000),
            canais: Some(2),
            ..Default::default()
        };
        let pt = crate::translations::get_translations("pt-br");
        let en = crate::translations::get_translations("en");
        assert_eq!(linha_tecnica(&d, &pt), "MP3 • 48,0 kHz • Estéreo");
        assert_eq!(linha_tecnica(&d, &en), "MP3 • 48.0 kHz • Stereo");
    }

    #[test]
    fn carregar_faixa_sem_capa_nao_falha() {
        let caminho =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tone.mp3");
        let c = carregar(&caminho);
        assert_eq!(c.detalhes.formato, "MP3");
        assert!(c.detalhes.taxa_hz.is_some());
        assert!(c.capa.is_none());
    }
}
