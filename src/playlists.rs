//! Playlists do usuário: um `.m3u8` por playlist numa pasta ao lado da
//! configuração. O arquivo é a fonte da verdade (outros players abrem e
//! editam); o Furinar lê, muda a lista de caminhos e grava de novo.
//!
//! As entradas são chaves no formato de `biblioteca::chave_faixa` (caminho
//! absoluto com `/`), as mesmas das listas especiais.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::biblioteca::chave_caminho;
use crate::track::{TrackInfo, normalizar_busca};

pub const EXTENSAO: &str = "m3u8";

/// Pasta das playlists (não é criada aqui).
pub fn pasta() -> PathBuf {
    crate::config::config_path().with_file_name("playlists")
}

/// Arquivos de playlist da pasta, pelo nome (sem acento nem caixa).
pub fn listar(pasta: &Path) -> Vec<PathBuf> {
    let mut arquivos: Vec<PathBuf> = fs::read_dir(pasta)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && e_playlist(p))
        .collect();
    arquivos.sort_by_cached_key(|p| normalizar_busca(&nome(p)));
    arquivos
}

/// `.m3u` ou `.m3u8`.
pub fn e_playlist(caminho: &Path) -> bool {
    caminho
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("m3u8") || e.eq_ignore_ascii_case("m3u"))
}

/// Nome de exibição: o nome do arquivo sem a extensão.
pub fn nome(arquivo: &Path) -> String {
    arquivo
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Chaves das faixas, na ordem do arquivo. Arquivo que não existe = vazia.
pub fn ler(arquivo: &Path) -> Vec<String> {
    let bytes = fs::read(arquivo).unwrap_or_default();
    // .m3u antigo pode vir em Latin-1: o que não for UTF-8 vira "?" e a
    // entrada simplesmente não acha o arquivo
    let texto = String::from_utf8_lossy(&bytes);
    let base = arquivo.parent().unwrap_or(Path::new(""));
    interpretar(&texto, base)
}

fn interpretar(texto: &str, base: &Path) -> Vec<String> {
    texto
        .trim_start_matches('\u{feff}')
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let caminho = match l.strip_prefix("file://") {
                // file:///home/... e file:///C:/...
                Some(resto) => {
                    let resto = decodificar_url(resto);
                    let resto = match resto.strip_prefix('/') {
                        Some(sem) if sem.as_bytes().get(1) == Some(&b':') => sem.to_string(),
                        _ => resto,
                    };
                    PathBuf::from(resto)
                }
                // Streams e outros esquemas não são arquivos
                None if l.contains("://") => return None,
                None => PathBuf::from(l),
            };
            let caminho = if caminho.is_absolute() {
                caminho
            } else {
                base.join(caminho)
            };
            Some(chave_caminho(&sem_pontos(&caminho)))
        })
        .collect()
}

/// Tira `.` e `..` do caminho sem ir ao disco: "../b.mp3" relativo à
/// playlist tem que dar a mesma chave que a faixa tem nas outras listas.
fn sem_pontos(caminho: &Path) -> PathBuf {
    use std::path::Component;
    let mut saida = PathBuf::new();
    for c in caminho.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                saida.pop();
            }
            outro => saida.push(outro),
        }
    }
    saida
}

/// `%20` → espaço etc. Sequência inválida fica como está.
fn decodificar_url(texto: &str) -> String {
    let bytes = texto.as_bytes();
    let mut saida = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(h) = texto.get(i + 1..i + 3)
            && let Ok(b) = u8::from_str_radix(h, 16)
        {
            saida.push(b);
            i += 3;
        } else {
            saida.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&saida).into_owned()
}

/// Grava a playlist. `info` dá título/artista/duração de uma chave, quando
/// conhecida, para a linha `#EXTINF` (outros players mostram sem ler a
/// faixa). Grava num temporário e troca: nunca deixa o arquivo pela metade.
pub fn escrever(
    arquivo: &Path,
    entradas: &[String],
    info: impl Fn(&str) -> Option<TrackInfo>,
) -> io::Result<()> {
    let mut texto = String::from("#EXTM3U\n");
    for chave in entradas {
        if let Some(t) = info(chave) {
            let nome = match t.artista.as_deref().filter(|a| !a.is_empty()) {
                Some(a) => format!("{a} - {}", t.titulo),
                None => t.titulo.clone(),
            };
            texto.push_str(&format!(
                "#EXTINF:{},{}\n",
                t.duracao,
                nome.replace('\n', " ")
            ));
        }
        // Caminho nativo: no Windows os players esperam "\"
        texto.push_str(&PathBuf::from(chave).to_string_lossy());
        texto.push('\n');
    }
    if let Some(pai) = arquivo.parent() {
        fs::create_dir_all(pai)?;
    }
    let temporario = arquivo.with_extension("m3u8.tmp");
    fs::write(&temporario, texto)?;
    fs::rename(&temporario, arquivo)
}

/// Nome que dá para usar como nome de arquivo: sem os caracteres proibidos
/// no Windows nem de controle, sem pontos/espaços nas pontas. `None` se não
/// sobrar nada.
pub fn nome_seguro(nome: &str) -> Option<String> {
    let limpo: String = nome
        .chars()
        .filter(|c| !c.is_control() && !"/\\:*?\"<>|".contains(*c))
        .collect();
    let limpo = limpo.trim().trim_matches('.').trim();
    (!limpo.is_empty()).then(|| limpo.chars().take(120).collect())
}

/// Arquivo para a playlist `nome` em `pasta`, com " (2)", " (3)"... se já
/// existir uma com esse nome (sem diferenciar caixa: o Windows não
/// diferencia). `atual` é o próprio arquivo, ao renomear.
pub fn caminho_livre(pasta: &Path, nome: &str, atual: Option<&Path>) -> PathBuf {
    let existentes: Vec<String> = listar(pasta)
        .iter()
        .filter(|p| Some(p.as_path()) != atual)
        .map(|p| self::nome(p).to_lowercase())
        .collect();
    let mut candidato = nome.to_string();
    let mut n = 2;
    while existentes.contains(&candidato.to_lowercase()) {
        candidato = format!("{nome} ({n})");
        n += 1;
    }
    pasta.join(format!("{candidato}.{EXTENSAO}"))
}

/// Acrescenta no fim. `false` = já estava (sem duplicar).
pub fn adicionar(entradas: &mut Vec<String>, chave: &str) -> bool {
    if entradas.iter().any(|e| e == chave) {
        return false;
    }
    entradas.push(chave.to_string());
    true
}

/// Tira a faixa. `false` = não estava.
pub fn remover(entradas: &mut Vec<String>, chave: &str) -> bool {
    let antes = entradas.len();
    entradas.retain(|e| e != chave);
    entradas.len() != antes
}

/// Troca a faixa de lugar com a vizinha visível (`visiveis` são as
/// entradas que existem, na ordem do arquivo; arquivos sumidos continuam no
/// arquivo e ficam onde estão). `delta` -1 sobe, +1 desce.
pub fn mover(entradas: &mut [String], visiveis: &[String], chave: &str, delta: i32) -> bool {
    let Some(pos) = visiveis.iter().position(|v| v == chave) else {
        return false;
    };
    let Some(vizinha) = pos
        .checked_add_signed(delta as isize)
        .and_then(|p| visiveis.get(p))
    else {
        return false;
    };
    let (Some(a), Some(b)) = (
        entradas.iter().position(|e| e == chave),
        entradas.iter().position(|e| e == vizinha),
    ) else {
        return false;
    };
    entradas.swap(a, b);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pasta_teste(nome: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "furinar-teste-playlists-{nome}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn le_m3u_com_comentario_relativo_url_bom_e_crlf() {
        let base = Path::new("/musica/listas");
        let texto = "\u{feff}#EXTM3U\r\n#EXTINF:200,Banda - Faixa\r\n/musica/a.mp3\r\n\r\n\
                     ../b.flac\r\nfile:///musica/c%20d.ogg\r\nhttp://radio/stream\r\n";
        assert_eq!(
            interpretar(texto, base),
            vec!["/musica/a.mp3", "/musica/b.flac", "/musica/c d.ogg"]
        );
    }

    #[test]
    fn url_de_arquivo_do_windows() {
        assert_eq!(decodificar_url("C:/M%C3%BAsica/x.mp3"), "C:/Música/x.mp3");
        assert_eq!(decodificar_url("100%"), "100%");
    }

    #[test]
    fn grava_e_le_de_volta() {
        let dir = pasta_teste("ida-volta");
        let arquivo = dir.join("Minha.m3u8");
        let a = chave_caminho(&dir.join("a.mp3"));
        let b = chave_caminho(&dir.join("b c.mp3"));
        let entradas = vec![a.clone(), b.clone()];
        escrever(&arquivo, &entradas, |k| {
            (k == a).then(|| TrackInfo {
                titulo: "Título".into(),
                artista: Some("Artista".into()),
                duracao: 61,
                ..Default::default()
            })
        })
        .unwrap();
        let texto = fs::read_to_string(&arquivo).unwrap();
        assert!(
            texto.starts_with("#EXTM3U\n#EXTINF:61,Artista - Título\n"),
            "{texto}"
        );
        assert_eq!(ler(&arquivo), entradas);
        assert_eq!(listar(&dir), vec![arquivo]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn nome_seguro_tira_o_proibido() {
        assert_eq!(
            nome_seguro("  Rock/Pop: 90's?  ").as_deref(),
            Some("RockPop 90's")
        );
        assert_eq!(nome_seguro(" ... "), None);
        assert_eq!(nome_seguro("Café"), Some("Café".into()));
    }

    #[test]
    fn caminho_livre_numera_sem_diferenciar_caixa() {
        let dir = pasta_teste("livre");
        fs::write(dir.join("Rock.m3u8"), "").unwrap();
        assert_eq!(caminho_livre(&dir, "rock", None), dir.join("rock (2).m3u8"));
        assert_eq!(caminho_livre(&dir, "Jazz", None), dir.join("Jazz.m3u8"));
        // Renomear para o próprio nome não numera
        let rock = dir.join("Rock.m3u8");
        assert_eq!(
            caminho_livre(&dir, "ROCK", Some(&rock)),
            dir.join("ROCK.m3u8")
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn adicionar_remover_e_mover_com_entrada_sumida_no_meio() {
        let mut e: Vec<String> = ["a", "sumida", "b", "c"].map(String::from).to_vec();
        assert!(!adicionar(&mut e, "a"), "não duplica");
        assert!(adicionar(&mut e, "d"));
        let visiveis: Vec<String> = ["a", "b", "c", "d"].map(String::from).to_vec();
        // "b" sobe: troca com "a" (a sumida fica no lugar)
        assert!(mover(&mut e, &visiveis, "b", -1));
        assert_eq!(e, ["b", "sumida", "a", "c", "d"]);
        assert!(!mover(&mut e, &visiveis, "d", 1), "última não desce");
        assert!(remover(&mut e, "sumida"));
        assert!(!remover(&mut e, "x"));
        assert_eq!(e, ["b", "a", "c", "d"]);
    }
}
