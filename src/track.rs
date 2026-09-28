use std::fs;

use lofty::config::ParseOptions;
use lofty::prelude::*;
use lofty::probe::Probe;
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

// ---------------------------------------------------------------------
// Informações de faixa (tags ID3/Vorbis)
// ---------------------------------------------------------------------

#[derive(Clone)]
pub struct TrackInfo {
    /// Relativo à pasta raiz.
    pub path: String,
    /// Tag title ou fallback do nome do arquivo.
    pub titulo: String,
    /// Tag artist (se houver).
    pub artista: Option<String>,
}

/// Normaliza texto para busca: remove acentos e passa para minúsculas, de modo
/// que "cancao" encontre "canção".
pub fn normalizar_busca(texto: &str) -> String {
    texto
        .nfd()
        .filter(|c| !is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase()
}

/// Lê título e artista das tags. O título cai para o nome do arquivo quando
/// não há tag.
pub fn ler_tags(caminho: &std::path::Path) -> (String, Option<String>) {
    // Lê apenas as tags (sem propriedades nem capa) para manter o scan rápido
    let opcoes = ParseOptions::new()
        .read_properties(false)
        .read_cover_art(false);

    let mut titulo = None;
    let mut artista = None;

    if let Ok(tagged) = Probe::open(caminho).and_then(|p| p.options(opcoes).read()) {
        if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
            titulo = tag
                .title()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            artista = tag
                .artist()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
        }
    }

    // Fallback: nome do arquivo sem extensão
    let titulo = titulo.unwrap_or_else(|| {
        caminho
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .replace('_', " ")
    });

    (titulo, artista)
}

/// Nome de exibição de uma pasta.
pub fn nome_da_pasta(caminho: &std::path::Path) -> String {
    caminho
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.to_string())
        .unwrap_or_else(|| caminho.to_string_lossy().to_string())
}

pub fn e_arquivo_audio(caminho: &std::path::Path) -> bool {
    if let Some(ext) = caminho.extension().and_then(|e| e.to_str()) {
        matches!(
            ext.to_lowercase().as_str(),
            "mp3" | "wav" | "flac" | "ogg" | "m4a"
        )
    } else {
        false
    }
}

pub fn coletar_arquivos_audio(
    diretorio: &std::path::Path,
    raiz: &std::path::Path,
    recursivo: bool,
    saida: &mut Vec<TrackInfo>,
) {
    if let Ok(entradas) = fs::read_dir(diretorio) {
        for entrada in entradas.flatten() {
            let path = entrada.path();
            if path.is_file() && e_arquivo_audio(&path) {
                if let Ok(rel) = path.strip_prefix(raiz) {
                    if let Some(nome) = rel.to_str() {
                        let (titulo, artista) = ler_tags(&path);
                        saida.push(TrackInfo {
                            path: nome.replace('\\', "/"),
                            titulo,
                            artista,
                        });
                    }
                }
            } else if recursivo && path.is_dir() {
                coletar_arquivos_audio(&path, raiz, recursivo, saida);
            }
        }
    }
}

/// Texto exibido na lista: "Artista - Título" quando há artista.
pub fn texto_exibicao(track: &TrackInfo) -> String {
    match &track.artista {
        Some(artista) => format!("{} - {}", artista, track.titulo),
        None => track.titulo.clone(),
    }
}

/// Chave de busca de uma faixa: título + artista normalizados (sem acento,
/// minúsculo). É calculada sob demanda, só durante o filtro, para não manter
/// uma cópia do texto por faixa na memória.
pub fn chave_busca(track: &TrackInfo) -> String {
    let mut chave = track.titulo.clone();
    if let Some(artista) = &track.artista {
        chave.push(' ');
        chave.push_str(artista);
    }
    normalizar_busca(&chave)
}

/// Ordem de navegação de uma pasta, aplicando o filtro atual.
pub fn ordem_filtrada(tracks: &[TrackInfo], filtro: &str) -> Vec<usize> {
    let filtro = normalizar_busca(filtro);
    if filtro.is_empty() {
        // Sem filtro não há por que normalizar faixa nenhuma
        return (0..tracks.len()).collect();
    }

    tracks
        .iter()
        .enumerate()
        .filter(|(_, t)| chave_busca(t).contains(filtro.as_str()))
        .map(|(i, _)| i)
        .collect()
}
