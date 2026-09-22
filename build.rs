use std::path::Path;

/// Ícone escolhido pelo usuário (qualquer formato que a crate `image` leia).
const ICONE_FONTE: &str = "ui/assets/furinar_icon.png";
/// Quadrado normalizado, consumido pelo .slint e pela geração do .ico.
const ICONE_QUADRADO: &str = "ui/assets/furinar_icon_quadrado.png";
/// Lado, em pixels, do quadrado normalizado.
const LADO: u32 = 256;

fn main() {
    // O .slint referencia o PNG normalizado, então ele precisa existir antes
    // da interface ser compilada.
    let quadrado = normalizar_icone();

    slint_build::compile("ui/main.slint").expect("falha ao compilar a interface Slint");

    embutir_icone(quadrado);
}

/// Lê o PNG de origem, recorta as bordas transparentes e centraliza o conteúdo
/// num quadrado com transparência — preservando a proporção, sem esticar.
///
/// Serve para aceitar imagens que não são quadradas (a maioria dos ícones de
/// janela e o recurso do .exe exigem quadrado).
fn normalizar_icone() -> image::RgbaImage {
    println!("cargo:rerun-if-changed={ICONE_FONTE}");

    let quadrado = match image::open(ICONE_FONTE) {
        Ok(imagem) => normalizar_quadrado(&imagem.into_rgba8(), LADO),
        Err(erro) => {
            println!("cargo:warning={ICONE_FONTE} não pôde ser lido ({erro}); usando ícone vazio");
            image::RgbaImage::new(LADO, LADO)
        }
    };

    if let Err(erro) = quadrado.save(ICONE_QUADRADO) {
        println!("cargo:warning=falha ao escrever {ICONE_QUADRADO}: {erro}");
    }

    quadrado
}

fn normalizar_quadrado(origem: &image::RgbaImage, lado: u32) -> image::RgbaImage {
    let (largura, altura) = origem.dimensions();

    // Caixa do conteúdo, ignorando o que é praticamente transparente
    let (mut x0, mut y0, mut x1, mut y1) = (largura, altura, 0u32, 0u32);
    let mut achou = false;
    for (x, y, pixel) in origem.enumerate_pixels() {
        if pixel.0[3] > 16 {
            achou = true;
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }

    if !achou {
        return image::RgbaImage::new(lado, lado);
    }

    let conteudo = image::imageops::crop_imm(origem, x0, y0, x1 - x0 + 1, y1 - y0 + 1).to_image();

    // Escala preservando a proporção até caber no quadrado
    let (c_largura, c_altura) = conteudo.dimensions();
    let escala = lado as f32 / c_largura.max(c_altura) as f32;
    let n_largura = ((c_largura as f32 * escala).round() as u32).clamp(1, lado);
    let n_altura = ((c_altura as f32 * escala).round() as u32).clamp(1, lado);
    let redimensionado = image::imageops::resize(
        &conteudo,
        n_largura,
        n_altura,
        image::imageops::FilterType::Lanczos3,
    );

    let mut quadrado = image::RgbaImage::new(lado, lado);
    image::imageops::overlay(
        &mut quadrado,
        &redimensionado,
        ((lado - n_largura) / 2) as i64,
        ((lado - n_altura) / 2) as i64,
    );
    quadrado
}

/// Embute o ícone do executável (o que aparece no Explorer).
fn embutir_icone(quadrado: image::RgbaImage) {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let destino =
        Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR ausente")).join("furinar.ico");
    if let Err(erro) = gerar_ico(&quadrado, &destino) {
        println!("cargo:warning=falha ao gerar o .ico: {erro}");
        return;
    }

    #[cfg(target_os = "windows")]
    {
        let caminho = destino.to_string_lossy().to_string();
        if let Err(erro) = tauri_winres::WindowsResource::new()
            .set_icon(&caminho)
            .compile()
        {
            println!("cargo:warning=falha ao embutir o ícone no executável: {erro}");
        }
    }
}

fn gerar_ico(
    quadrado: &image::RgbaImage,
    destino: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    use image::ExtendedColorType;
    use image::codecs::ico::{IcoEncoder, IcoFrame};

    let quadros = [16u32, 32, 48, 256]
        .into_iter()
        .map(|lado| {
            let redimensionado = image::imageops::resize(
                quadrado,
                lado,
                lado,
                image::imageops::FilterType::Lanczos3,
            );
            IcoFrame::as_png(
                redimensionado.as_raw(),
                lado,
                lado,
                ExtendedColorType::Rgba8,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let arquivo = std::fs::File::create(destino)?;
    IcoEncoder::new(std::io::BufWriter::new(arquivo)).encode_images(&quadros)?;
    Ok(())
}
