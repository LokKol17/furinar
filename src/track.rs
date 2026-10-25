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

#[derive(Clone, Default)]
pub struct TrackInfo {
    /// Relativo à pasta raiz. Nas listas especiais (favoritas, mais tocadas,
    /// álbum) é o caminho absoluto: `raiz.join(abs)` dá o próprio `abs`.
    pub path: String,
    /// Tag title ou fallback do nome do arquivo.
    pub titulo: String,
    /// Tag artist (se houver).
    pub artista: Option<String>,
    pub album: Option<String>,
    /// Disco × 1000 + faixa, para a ordem do álbum (0 = sem número).
    pub numero: u32,
    /// Duração em segundos (0 = desconhecida).
    pub duracao: u32,
    /// Quando o arquivo entrou no disco (segundos desde 1970), para "data de
    /// adição".
    pub adicionado: u64,
    /// Fator linear do ReplayGain (faixa, ou álbum na falta), já limitado
    /// pelo pico para não estourar.
    pub ganho: Option<f32>,
}

/// O que as tags dizem de uma faixa.
pub struct Tags {
    pub titulo: String,
    pub artista: Option<String>,
    pub album: Option<String>,
    pub numero: u32,
    pub duracao: u32,
    pub ganho: Option<f32>,
}

/// Ordem da lista, escolhida por aba.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Ordem {
    /// Ordem natural da lista: nome do arquivo nas pastas, ordem em que
    /// entraram nas favoritas, número de reproduções nas mais tocadas.
    #[default]
    Padrao,
    Titulo,
    Artista,
    Album,
    Duracao,
    /// Mais recentes primeiro.
    Adicao,
    /// Mais tocadas primeiro.
    Reproducoes,
}

impl Ordem {
    pub const TODAS: [Ordem; 7] = [
        Ordem::Padrao,
        Ordem::Titulo,
        Ordem::Artista,
        Ordem::Album,
        Ordem::Duracao,
        Ordem::Adicao,
        Ordem::Reproducoes,
    ];

    pub fn de_indice(i: usize) -> Self {
        Self::TODAS.get(i).copied().unwrap_or_default()
    }

    pub fn indice(self) -> usize {
        Self::TODAS.iter().position(|&o| o == self).unwrap_or(0)
    }
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

fn texto(valor: Option<std::borrow::Cow<'_, str>>) -> Option<String> {
    valor
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Lê as tags. O título cai para o nome do arquivo quando não há tag.
pub fn ler_tags(caminho: &std::path::Path) -> Tags {
    // Sem capa: o scan precisa ser rápido. As propriedades vêm pela duração
    // (lida do cabeçalho, sem decodificar).
    let opcoes = ParseOptions::new().read_cover_art(false);

    let mut titulo = None;
    let mut artista = None;
    let mut album = None;
    let mut numero = 0;
    let mut duracao = 0;
    let mut ganho = None;

    if let Ok(tagged) = Probe::open(caminho).and_then(|p| p.options(opcoes).read()) {
        duracao = tagged.properties().duration().as_secs() as u32;
        if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
            titulo = texto(tag.title());
            artista = texto(tag.artist());
            album = texto(tag.album());
            numero = tag.disk().unwrap_or(0).min(999) * 1000 + tag.track().unwrap_or(0).min(999);
            let valor = |chave| tag.get_string(chave).and_then(parse_decibeis);
            ganho = fator_replaygain(
                valor(ItemKey::ReplayGainTrackGain).or(valor(ItemKey::ReplayGainAlbumGain)),
                valor(ItemKey::ReplayGainTrackPeak).or(valor(ItemKey::ReplayGainAlbumPeak)),
            );
        }
    }

    // Vídeo que o lofty não lê (MKV/WebM/AVI): duração e tags pelo ffprobe
    if duracao == 0
        && crate::ffmpeg::e_video(caminho)
        && let Some(sonda) = crate::ffmpeg::sondar(caminho)
    {
        duracao = sonda.duracao.as_secs() as u32;
        titulo = titulo.or(sonda.titulo);
        artista = artista.or(sonda.artista);
    }

    // Fallback: nome do arquivo sem extensão
    let titulo = titulo.unwrap_or_else(|| {
        caminho
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .replace('_', " ")
    });

    Tags {
        titulo,
        artista,
        album,
        numero,
        duracao,
        ganho,
    }
}

/// "-7.31 dB", "+1.2 dB" ou "0.988" → número. Os picos não têm unidade.
fn parse_decibeis(texto: &str) -> Option<f32> {
    let limpo = texto.trim();
    let limpo = limpo
        .strip_suffix("dB")
        .or_else(|| limpo.strip_suffix("db"))
        .or_else(|| limpo.strip_suffix("DB"))
        .unwrap_or(limpo);
    limpo
        .trim()
        .trim_start_matches('+')
        .parse()
        .ok()
        .filter(|v: &f32| v.is_finite())
}

/// Ganho em dB → fator linear, sem passar do que o pico permite (fator × pico
/// ≤ 1, ou a faixa estoura).
pub fn fator_replaygain(ganho_db: Option<f32>, pico: Option<f32>) -> Option<f32> {
    let fator = 10f32.powf(ganho_db? / 20.0);
    let teto = pico.filter(|&p| p > 0.0).map_or(f32::INFINITY, |p| 1.0 / p);
    Some(fator.min(teto))
}

/// Quando o arquivo chegou: criação, ou modificação onde o sistema de arquivos
/// não guarda a criação.
pub fn data_de_adicao(caminho: &std::path::Path) -> u64 {
    fs::metadata(caminho)
        .ok()
        .and_then(|m| m.created().or_else(|_| m.modified()).ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}

/// Monta a faixa a partir do arquivo. `nome` é o que vai em `path`.
pub fn ler_faixa(caminho: &std::path::Path, nome: String) -> TrackInfo {
    let tags = ler_tags(caminho);
    TrackInfo {
        path: nome,
        titulo: tags.titulo,
        artista: tags.artista,
        album: tags.album,
        numero: tags.numero,
        duracao: tags.duracao,
        adicionado: data_de_adicao(caminho),
        ganho: tags.ganho,
    }
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
        let ext = ext.to_lowercase();
        matches!(ext.as_str(), "mp3" | "wav" | "flac" | "ogg" | "m4a")
            || crate::ffmpeg::EXT_VIDEO_NATIVO.contains(&ext.as_str())
            // Sem ffmpeg o áudio desses não sai: nem entram na lista
            || (crate::ffmpeg::EXT_VIDEO_FFMPEG.contains(&ext.as_str())
                && crate::ffmpeg::disponivel())
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
                if let Ok(rel) = path.strip_prefix(raiz)
                    && let Some(nome) = rel.to_str()
                {
                    saida.push(ler_faixa(&path, nome.replace('\\', "/")));
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

/// Reordena `indices` (posições em `tracks`) conforme `ordem`. Estável: o
/// empate fica na ordem natural. `reproducoes(i)` dá quantas vezes a faixa
/// `i` tocou.
pub fn ordenar(
    indices: &mut [usize],
    tracks: &[TrackInfo],
    ordem: Ordem,
    reproducoes: impl Fn(usize) -> u32,
) {
    let titulo = |i: usize| normalizar_busca(&tracks[i].titulo);
    let artista = |i: usize| normalizar_busca(tracks[i].artista.as_deref().unwrap_or(""));
    let album = |i: usize| normalizar_busca(tracks[i].album.as_deref().unwrap_or(""));
    // Sem a tag, vai para o fim (e não para o começo, como "" iria)
    let vazio_no_fim = |s: String| (s.is_empty(), s);
    match ordem {
        Ordem::Padrao => {}
        Ordem::Titulo => indices.sort_by_cached_key(|&i| titulo(i)),
        Ordem::Artista => indices.sort_by_cached_key(|&i| {
            (
                vazio_no_fim(artista(i)),
                vazio_no_fim(album(i)),
                tracks[i].numero,
                titulo(i),
            )
        }),
        Ordem::Album => {
            indices.sort_by_cached_key(|&i| (vazio_no_fim(album(i)), tracks[i].numero, titulo(i)))
        }
        Ordem::Duracao => indices.sort_by_key(|&i| tracks[i].duracao),
        Ordem::Adicao => indices.sort_by_key(|&i| std::cmp::Reverse(tracks[i].adicionado)),
        Ordem::Reproducoes => indices.sort_by_key(|&i| std::cmp::Reverse(reproducoes(i))),
    }
}

/// Um álbum da lista, para a grade do modo imersivo.
#[derive(Clone, Debug, PartialEq)]
pub struct GrupoAlbum {
    /// Tag de álbum, ou o nome da pasta das faixas sem tag ("" na raiz).
    pub nome: String,
    /// Artista em comum (`None` se não tem ou se são vários).
    pub artista: Option<String>,
    /// Mais de um artista no álbum.
    pub varios: bool,
    /// Posições em `tracks`, na ordem do disco. A capa vem da primeira.
    pub faixas: Vec<usize>,
}

/// Agrupa as faixas `indices` por álbum. Mesmo nome em pastas diferentes são
/// álbuns diferentes ("Greatest Hits" de duas bandas não se misturam).
pub fn agrupar_albuns(tracks: &[TrackInfo], indices: &[usize]) -> Vec<GrupoAlbum> {
    let mut grupos: Vec<GrupoAlbum> = Vec::new();
    let mut posicao: std::collections::HashMap<(String, &str), usize> =
        std::collections::HashMap::new();
    for &i in indices {
        let Some(t) = tracks.get(i) else {
            continue;
        };
        let pasta = t.path.rsplit_once('/').map_or("", |(p, _)| p);
        let chave = (normalizar_busca(t.album.as_deref().unwrap_or("")), pasta);
        let g = *posicao.entry(chave).or_insert_with(|| {
            let nome = t
                .album
                .clone()
                .unwrap_or_else(|| pasta.rsplit('/').next().unwrap_or("").to_string());
            grupos.push(GrupoAlbum {
                nome,
                artista: t.artista.clone(),
                varios: false,
                faixas: Vec::new(),
            });
            grupos.len() - 1
        });
        let grupo = &mut grupos[g];
        if grupo.artista != t.artista {
            grupo.varios = true;
        }
        grupo.faixas.push(i);
    }
    for g in &mut grupos {
        if g.varios {
            g.artista = None;
        }
        g.faixas.sort_by_key(|&i| (tracks[i].numero, i));
    }
    grupos.sort_by_cached_key(|g| {
        let nome = normalizar_busca(&g.nome);
        (nome.is_empty(), nome)
    });
    grupos
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
    use std::path::Path;
    #[cfg(unix)]
    use std::path::PathBuf;
    #[cfg(unix)]
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[cfg(unix)]
    static SEQUENCIA: AtomicUsize = AtomicUsize::new(0);

    /// Só nos testes unix: os testes de symlink não existem em Windows.
    #[cfg(unix)]
    struct PastaTemporaria(PathBuf);

    #[cfg(unix)]
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

    #[cfg(unix)]
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
                ..Default::default()
            },
            TrackInfo {
                path: "b.mp3".into(),
                titulo: "Vento".into(),
                ..Default::default()
            },
        ];

        assert_eq!(ordem_filtrada(&tracks, ""), vec![0, 1]);
        assert_eq!(ordem_filtrada(&tracks, "cancao"), vec![0]);
        assert_eq!(ordem_filtrada(&tracks, "banda"), vec![0]);
        assert_eq!(ordem_filtrada(&tracks, "vento"), vec![1]);
        assert!(ordem_filtrada(&tracks, "inexistente").is_empty());
    }

    fn faixa(titulo: &str, artista: Option<&str>, album: Option<&str>, numero: u32) -> TrackInfo {
        TrackInfo {
            path: format!("{titulo}.mp3"),
            titulo: titulo.into(),
            artista: artista.map(Into::into),
            album: album.map(Into::into),
            numero,
            ..Default::default()
        }
    }

    #[test]
    fn ordenar_por_cada_criterio() {
        let mut tracks = vec![
            faixa("Zebra", Some("Banda B"), Some("Disco 1"), 2),
            faixa("Água", None, None, 0),
            faixa("Mar", Some("Banda A"), Some("Disco 1"), 1),
        ];
        tracks[0].duracao = 300;
        tracks[1].duracao = 100;
        tracks[2].duracao = 200;
        tracks[0].adicionado = 10;
        tracks[1].adicionado = 30;
        tracks[2].adicionado = 20;
        let ordem = |o| {
            let mut v = vec![0, 1, 2];
            ordenar(&mut v, &tracks, o, |i| [5, 0, 9][i]);
            v
        };
        assert_eq!(ordem(Ordem::Padrao), [0, 1, 2]);
        // Sem acento: "Água" entra como "agua"
        assert_eq!(ordem(Ordem::Titulo), [1, 2, 0]);
        // Sem artista vai para o fim
        assert_eq!(ordem(Ordem::Artista), [2, 0, 1]);
        // Mesmo álbum: pelo número da faixa
        assert_eq!(ordem(Ordem::Album), [2, 0, 1]);
        assert_eq!(ordem(Ordem::Duracao), [1, 2, 0]);
        assert_eq!(ordem(Ordem::Adicao), [1, 2, 0]);
        assert_eq!(ordem(Ordem::Reproducoes), [2, 0, 1]);
    }

    #[test]
    fn agrupa_por_album_e_pasta() {
        let mut tracks = vec![
            faixa("B2", Some("X"), Some("Beta"), 2),
            faixa("A1", Some("X"), Some("Alfa"), 1),
            faixa("B1", Some("Y"), Some("Beta"), 1),
            faixa("Solta", None, None, 0),
            faixa("Outro Beta", Some("Z"), Some("Beta"), 1),
        ];
        tracks[0].path = "beta/b2.mp3".into();
        tracks[1].path = "alfa/a1.mp3".into();
        tracks[2].path = "beta/b1.mp3".into();
        tracks[3].path = "soltas/solta.mp3".into();
        tracks[4].path = "outra/beta.mp3".into();
        let grupos = agrupar_albuns(&tracks, &[0, 1, 2, 3, 4]);
        let nomes: Vec<&str> = grupos.iter().map(|g| g.nome.as_str()).collect();
        // Sem tag: o nome da pasta; "Beta" de outra pasta é outro álbum
        assert_eq!(nomes, ["Alfa", "Beta", "Beta", "soltas"]);
        let beta = &grupos[1];
        assert_eq!(beta.faixas, [2, 0], "ordem do disco");
        assert!(beta.varios && beta.artista.is_none());
        assert_eq!(grupos[0].artista.as_deref(), Some("X"));
        // Só o que passou no filtro entra
        assert_eq!(agrupar_albuns(&tracks, &[1]).len(), 1);
    }

    #[test]
    fn ordem_ida_e_volta_pelo_indice() {
        for o in Ordem::TODAS {
            assert_eq!(Ordem::de_indice(o.indice()), o);
        }
        assert_eq!(Ordem::de_indice(99), Ordem::Padrao);
    }

    #[test]
    fn replaygain_le_decibeis_e_respeita_o_pico() {
        assert_eq!(parse_decibeis("-6.02 dB"), Some(-6.02));
        assert_eq!(parse_decibeis("+1.5 dB"), Some(1.5));
        assert_eq!(parse_decibeis(" 0.98 "), Some(0.98));
        assert_eq!(parse_decibeis("lixo"), None);

        let metade = fator_replaygain(Some(-6.0206), None).unwrap();
        assert!((metade - 0.5).abs() < 0.001, "{metade}");
        // +6 dB dobraria, mas o pico em 0,8 só deixa subir até 1,25
        let limitado = fator_replaygain(Some(6.0206), Some(0.8)).unwrap();
        assert!((limitado - 1.25).abs() < 0.001, "{limitado}");
        assert_eq!(fator_replaygain(None, Some(0.5)), None);
    }

    #[test]
    fn e_arquivo_audio_reconhece_as_extensoes_tocaveis() {
        for nome in [
            "a.mp3", "a.wav", "a.flac", "a.ogg", "a.m4a", "A.M4A", "v.mp4", "v.MOV", "v.m4v",
        ] {
            assert!(e_arquivo_audio(Path::new(nome)), "{nome} deveria ser áudio");
        }
        // MKV/WebM só entram com o ffmpeg, que decodifica o áudio deles
        assert_eq!(
            e_arquivo_audio(Path::new("v.webm")),
            crate::ffmpeg::disponivel()
        );
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
