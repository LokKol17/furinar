use std::collections::HashSet;
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
    let mut visitados = HashSet::new();
    coletar_recursivo(diretorio, raiz, recursivo, saida, &mut visitados);
}

/// Varredura com controle das pastas já visitadas: um symlink para uma pasta
/// ancestral (ou para outra pasta já lida) entraria em recursão infinita ou
/// duplicaria faixas, já que `is_dir()` segue o link.
fn coletar_recursivo(
    diretorio: &std::path::Path,
    raiz: &std::path::Path,
    recursivo: bool,
    saida: &mut Vec<TrackInfo>,
    visitados: &mut HashSet<std::path::PathBuf>,
) {
    // Sem canonicalize o mesmo diretório entra por caminhos diferentes
    // (`./a` e `a`); se falhar (link quebrado), seguimos pelo caminho dado.
    let chave = diretorio
        .canonicalize()
        .unwrap_or_else(|_| diretorio.to_path_buf());
    if !visitados.insert(chave) {
        return;
    }

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
                coletar_recursivo(&path, raiz, recursivo, saida, visitados);
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

/// Filtro que a navegação (próxima/anterior) deve usar.
///
/// A caixa de busca filtra a lista da aba **visível**, mas a navegação anda
/// pela pasta de onde veio a faixa **tocando**. Quando as duas são diferentes
/// o texto pertence a outra lista: aplicá-lo aqui filtraria a pasta errada,
/// então nesse caso a navegação percorre a pasta inteira.
pub fn filtro_efetivo(aba_visivel: usize, pasta_reproducao: Option<usize>, filtro: &str) -> &str {
    if pasta_reproducao == Some(aba_visivel) {
        filtro
    } else {
        ""
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static SEQUENCIA: AtomicUsize = AtomicUsize::new(0);

    struct PastaTemporaria(PathBuf);

    impl PastaTemporaria {
        fn nova(nome: &str) -> Self {
            let caminho = std::env::temp_dir().join(format!(
                "furinar_test_{nome}_{}_{}",
                std::process::id(),
                SEQUENCIA.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&caminho);
            fs::create_dir_all(&caminho).expect("cria pasta temporária");
            Self(caminho)
        }
    }

    impl Drop for PastaTemporaria {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn filtro_efetivo_vale_quando_a_busca_e_da_pasta_que_toca() {
        assert_eq!(filtro_efetivo(2, Some(2), "xpto"), "xpto");
    }

    #[test]
    fn filtro_efetivo_e_vazio_para_a_outra_pasta() {
        // Busca digitada na aba 1 não pode filtrar a navegação da aba 2
        assert_eq!(filtro_efetivo(1, Some(2), "xpto"), "");
        assert_eq!(filtro_efetivo(0, None, "xpto"), "");
    }

    #[test]
    fn normalizar_busca_tira_acento_e_caixa() {
        assert_eq!(normalizar_busca("Canção Àlbum"), "cancao album");
        assert_eq!(normalizar_busca("MP3"), "mp3");
    }

    #[test]
    fn ordem_filtrada_filtra_por_titulo_e_artista() {
        let tracks = vec![
            TrackInfo {
                path: "a.mp3".into(),
                titulo: "Canção Nova".into(),
                artista: Some("Banda".into()),
            },
            TrackInfo {
                path: "b.mp3".into(),
                titulo: "Vento".into(),
                artista: None,
            },
        ];

        assert_eq!(ordem_filtrada(&tracks, ""), vec![0, 1]);
        assert_eq!(ordem_filtrada(&tracks, "cancao"), vec![0]);
        assert_eq!(ordem_filtrada(&tracks, "banda"), vec![0]);
        assert_eq!(ordem_filtrada(&tracks, "vento"), vec![1]);
        assert!(ordem_filtrada(&tracks, "inexistente").is_empty());
    }

    #[test]
    fn e_arquivo_audio_reconhece_as_extensoes_tocaveis() {
        for nome in ["a.mp3", "a.wav", "a.flac", "a.ogg", "a.m4a", "A.M4A"] {
            assert!(e_arquivo_audio(Path::new(nome)), "{nome} deveria ser áudio");
        }
        for nome in ["a.txt", "a.mp3.bak", "sem_extensao"] {
            assert!(
                !e_arquivo_audio(Path::new(nome)),
                "{nome} não deveria ser áudio"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn symlink_para_ancestral_nao_entra_em_loop_nem_duplica() {
        let tmp = PastaTemporaria::nova("symlink_loop");
        let raiz = tmp.0.clone();
        fs::create_dir_all(raiz.join("sub")).unwrap();
        fs::write(raiz.join("a.mp3"), b"x").unwrap();
        fs::write(raiz.join("sub").join("b.mp3"), b"x").unwrap();
        // Link apontando de volta para a raiz: sem controle de visita isto
        // recursa para sempre.
        std::os::unix::fs::symlink(&raiz, raiz.join("sub").join("loop")).unwrap();

        let mut saida = Vec::new();
        coletar_arquivos_audio(&raiz, &raiz, true, &mut saida);

        let mut caminhos: Vec<String> = saida.iter().map(|t| t.path.clone()).collect();
        caminhos.sort();
        assert_eq!(caminhos, vec!["a.mp3", "sub/b.mp3"]);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_para_pasta_externa_continua_sendo_seguido() {
        let externa = PastaTemporaria::nova("symlink_externa");
        let raiz = PastaTemporaria::nova("symlink_raiz");
        fs::write(externa.0.join("remota.mp3"), b"x").unwrap();
        std::os::unix::fs::symlink(&externa.0, raiz.0.join("link")).unwrap();

        let mut saida = Vec::new();
        coletar_arquivos_audio(&raiz.0, &raiz.0, true, &mut saida);

        let caminhos: Vec<String> = saida.iter().map(|t| t.path.clone()).collect();
        assert_eq!(caminhos, vec!["link/remota.mp3"]);
    }
}
