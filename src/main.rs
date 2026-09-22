// Em release não abrimos console junto do app; no debug mantemos, para ver logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

slint::include_modules!();

mod updates;

use std::cell::RefCell;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{
    Arc, Mutex,
    mpsc::{self, Receiver, Sender},
};
use std::time::Duration;

use lofty::config::ParseOptions;
use lofty::prelude::*;
use lofty::probe::Probe;
use rand::seq::SliceRandom;
use rodio::{Decoder, OutputStream, Sink, Source};
use serde::{Deserialize, Serialize};
#[cfg(target_os = "windows")]
use slint::winit_030::winit::platform::windows::EventLoopBuilderExtWindows;
use slint::{ModelRc, SharedString, Timer, TimerMode, VecModel};
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
    SeekDirection,
};
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{HINSTANCE, HWND};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Dwm::{
    DWM_WINDOW_CORNER_PREFERENCE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    DwmSetWindowAttribute,
};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Gdi::{
    RDW_ALLCHILDREN, RDW_ERASE, RDW_INVALIDATE, RDW_UPDATENOW, RedrawWindow,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
#[cfg(target_os = "windows")]
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
#[cfg(target_os = "windows")]
use windows::Win32::UI::Controls::{
    HIMAGELIST, ILC_COLOR32, ILC_MASK, ImageList_Create, ImageList_Destroy, ImageList_ReplaceIcon,
};
#[cfg(target_os = "windows")]
use windows::Win32::UI::Shell::{
    ITaskbarList3, THB_BITMAP, THB_FLAGS, THB_ICON, THB_TOOLTIP, THBF_ENABLED, THBN_CLICKED,
    THUMBBUTTON, TaskbarList,
};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIcon, DestroyIcon, GetSystemMetrics, HICON, MSG, SM_CXSMICON, WM_COMMAND,
    WM_DISPLAYCHANGE, WM_DWMCOMPOSITIONCHANGED, WM_EXITSIZEMOVE,
};

// ---------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------

#[derive(Serialize, Deserialize, Default)]
struct AppConfig {
    /// Campo antigo (uma pasta só). Existe apenas para migrar configs antigas.
    #[serde(default, skip_serializing)]
    pasta: Option<String>,
    /// Pastas abertas, na ordem das abas.
    #[serde(default)]
    pastas: Vec<String>,
    /// Aba que estava sendo exibida.
    #[serde(default)]
    aba_visivel_salva: usize,
    /// Pasta de onde vinha a faixa que estava tocando.
    #[serde(default)]
    pasta_reproducao_salva: Option<usize>,
    volume: f32,
    modo_loop: u8,
    /// Campo antigo (só ligado/desligado). Existe apenas para migrar configs antigas.
    #[serde(default, skip_serializing)]
    shuffle: bool,
    /// 0 = desligado, 1 = shuffle, 2 = shuffle inteligente.
    #[serde(default)]
    modo_shuffle: u8,
    indice_atual: Option<usize>,
    tempo_atual: Option<u64>,
    #[serde(default)]
    escanear_subpastas: bool,
    /// Tema claro ligado. `false` = escuro (padrão, mantém quem já usa o app).
    #[serde(default)]
    tema_claro: bool,
}

impl AppConfig {
    fn carregar() -> Self {
        let mut config = if let Ok(conteudo) = fs::read_to_string("furinar_config.json") {
            serde_json::from_str(&conteudo).unwrap_or_default()
        } else {
            Self {
                volume: 0.8,
                ..Default::default()
            }
        };
        config.migrar();
        config
    }

    /// Migra configs antigas: o campo `pasta` (singular) vira uma entrada em
    /// `pastas`, e essa pasta era necessariamente a que estava tocando. O
    /// campo `shuffle` (bool) vira `modo_shuffle` (0/1/2).
    fn migrar(&mut self) {
        if self.pastas.is_empty() {
            if let Some(antiga) = self.pasta.take() {
                self.pastas.push(antiga);
                self.pasta_reproducao_salva = Some(0);
            }
        }
        if self.modo_shuffle == 0 && self.shuffle {
            self.modo_shuffle = 1;
        }
    }

    fn salvar(&self) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write("furinar_config.json", json);
        }
    }
}

fn formatar_tempo(segundos: u64, com_horas: bool) -> String {
    let horas = segundos / 3600;
    let mins = (segundos % 3600) / 60;
    let segs = segundos % 60;
    if com_horas {
        format!("{:02}:{:02}:{:02}", horas, mins, segs)
    } else {
        format!("{:02}:{:02}", mins, segs)
    }
}

fn e_arquivo_audio(caminho: &std::path::Path) -> bool {
    if let Some(ext) = caminho.extension().and_then(|e| e.to_str()) {
        matches!(
            ext.to_lowercase().as_str(),
            "mp3" | "wav" | "flac" | "ogg" | "m4a"
        )
    } else {
        false
    }
}

fn texto_loop(modo: u8) -> &'static str {
    match modo {
        1 => "Loop: Faixa",
        2 => "Loop: Toda",
        _ => "Loop: Desl",
    }
}

fn texto_shuffle(modo: u8) -> &'static str {
    match modo {
        1 => "Shuffle",
        2 => "Shuffle Inteligente",
        _ => "Shuffle: Desl",
    }
}

// ---------------------------------------------------------------------
// Informações de faixa (tags ID3/Vorbis)
// ---------------------------------------------------------------------

#[derive(Clone)]
struct TrackInfo {
    path: String,            // relativo à pasta raiz
    titulo: String,          // tag title ou fallback do nome do arquivo
    artista: Option<String>, // tag artist (se houver)
}

/// Normaliza texto para busca: remove acentos e passa para minúsculas, de modo
/// que "cancao" encontre "canção".
fn normalizar_busca(texto: &str) -> String {
    texto
        .nfd()
        .filter(|c| !is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase()
}

/// Lê título e artista das tags. O título cai para o nome do arquivo quando
/// não há tag.
fn ler_tags(caminho: &std::path::Path) -> (String, Option<String>) {
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

// ---------------------------------------------------------------------
// Pastas (abas)
// ---------------------------------------------------------------------

/// Uma pasta de música aberta pelo usuário, com as faixas já escaneadas.
struct PastaMusical {
    caminho: PathBuf,
    /// Nome de exibição na aba (último componente do caminho).
    nome: String,
    tracks: Vec<TrackInfo>,
    /// Se as faixas já foram escaneadas. O scan é preguiçoso: pastas abertas
    /// mas nunca visitadas ficam sem `tracks` até serem necessárias.
    carregada: bool,
}

/// Nome de exibição de uma pasta.
fn nome_da_pasta(caminho: &std::path::Path) -> String {
    caminho
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.to_string())
        .unwrap_or_else(|| caminho.to_string_lossy().to_string())
}

// ---------------------------------------------------------------------
// Estado de áudio/playlist
// ---------------------------------------------------------------------

struct EstadoAudio {
    audio_player: Option<(OutputStream, Sink)>,
    tempo_decorrido: f64,
    /// Pastas abertas (abas), na ordem de exibição.
    pastas: Vec<PastaMusical>,
    /// Índice da pasta sendo exibida na lista. Navegar não é reproduzir.
    aba_visivel: usize,
    /// Índice da pasta de onde vem a faixa atual. Pode diferir de `aba_visivel`.
    pasta_reproducao: Option<usize>,
    arquivo_atual: Option<String>,
    indice_atual: Option<usize>,
    volume_atual: f32,
    duracao_total_secs: u64,
    ultimo_segundo: u64,
    modo_loop: u8,
    /// 0 = desligado, 1 = shuffle, 2 = shuffle inteligente.
    modo_shuffle: u8,
    /// Sacola do shuffle inteligente: índices embaralhados restantes na
    /// pasta de reprodução atual. Esvazia -> reembaralha. Resetada (fica
    /// vazia, força reembaralho) quando a pasta de reprodução muda, a
    /// lista de faixas muda, ou o modo sai/entra no inteligente.
    sacola_shuffle: Vec<usize>,
    escanear_subpastas: bool,
    tema_claro: bool,
    /// Mapeia posição na lista visível -> índice em `pastas[aba_visivel].tracks`.
    indices_visiveis: Vec<usize>,
    /// Texto atual da busca (título/artista).
    filtro: String,
    /// Letra sincronizada da faixa atual (tempo em segundos, texto), ordenada.
    letra_atual: Vec<(f64, String)>,
    /// Índice da linha da letra exibida (-1 = antes da primeira).
    letra_indice: i32,
    /// Índice absoluto da primeira linha do recorte que está na UI. Serve para
    /// traduzir o clique numa linha do painel de volta para o timestamp.
    letra_janela_inicio: usize,
    faixa_controles: Option<String>,
    status_controles: Option<MediaPlayback>,
}

impl Default for EstadoAudio {
    fn default() -> Self {
        Self {
            audio_player: None,
            tempo_decorrido: 0.0,
            pastas: Vec::new(),
            aba_visivel: 0,
            pasta_reproducao: None,
            arquivo_atual: None,
            indice_atual: None,
            volume_atual: 0.8,
            duracao_total_secs: 0,
            ultimo_segundo: 0,
            modo_loop: 0,
            modo_shuffle: 0,
            sacola_shuffle: Vec::new(),
            escanear_subpastas: false,
            tema_claro: false,
            indices_visiveis: Vec::new(),
            filtro: String::new(),
            letra_atual: Vec::new(),
            letra_indice: -1,
            letra_janela_inicio: 0,
            faixa_controles: None,
            status_controles: None,
        }
    }
}

fn salvar_configuracao(estado: &EstadoAudio) {
    let config = AppConfig {
        pasta: None,
        pastas: estado
            .pastas
            .iter()
            .map(|p| p.caminho.to_string_lossy().to_string())
            .collect(),
        aba_visivel_salva: estado.aba_visivel,
        pasta_reproducao_salva: estado.pasta_reproducao,
        volume: estado.volume_atual,
        modo_loop: estado.modo_loop,
        shuffle: false,
        modo_shuffle: estado.modo_shuffle,
        escanear_subpastas: estado.escanear_subpastas,
        tema_claro: estado.tema_claro,
        indice_atual: estado.indice_atual,
        tempo_atual: if estado.tempo_decorrido > 0.0 {
            Some(estado.tempo_decorrido as u64)
        } else {
            None
        },
    };
    config.salvar();
}

fn coletar_arquivos_audio(
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
fn texto_exibicao(track: &TrackInfo) -> String {
    match &track.artista {
        Some(artista) => format!("{} - {}", artista, track.titulo),
        None => track.titulo.clone(),
    }
}

/// Ordem de navegação de uma pasta, aplicando o filtro atual.
/// Chave de busca de uma faixa: título + artista normalizados (sem acento,
/// minúsculo). É calculada sob demanda, só durante o filtro, para não manter
/// uma cópia do texto por faixa na memória.
fn chave_busca(track: &TrackInfo) -> String {
    let mut chave = track.titulo.clone();
    if let Some(artista) = &track.artista {
        chave.push(' ');
        chave.push_str(artista);
    }
    normalizar_busca(&chave)
}

fn ordem_filtrada(tracks: &[TrackInfo], filtro: &str) -> Vec<usize> {
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

/// Marca na UI a posição da faixa atual na lista visível.
///
/// Só destaca quando a aba visível é a mesma de onde a faixa vem: marcar
/// "tocando" numa lista que não é a de origem confunde.
fn atualizar_selecao_visivel(estado: &EstadoAudio, ui: &MainWindow) {
    let pos = if estado.pasta_reproducao == Some(estado.aba_visivel) {
        estado
            .indice_atual
            .and_then(|idx| estado.indices_visiveis.iter().position(|&i| i == idx))
    } else {
        None
    };
    ui.set_indice_selecionado(pos.map_or(-1, |p| p as i32));
}

/// Reconstrói a lista da aba visível aplicando o filtro e atualiza a seleção.
fn atualizar_lista(estado: &mut EstadoAudio, ui: &MainWindow) {
    let (indices, modelo) = match estado.pastas.get(estado.aba_visivel) {
        Some(pasta) => {
            let indices = ordem_filtrada(&pasta.tracks, &estado.filtro);
            let modelo: Vec<SharedString> = indices
                .iter()
                .map(|&i| texto_exibicao(&pasta.tracks[i]).into())
                .collect();
            (indices, modelo)
        }
        None => (Vec::new(), Vec::new()),
    };

    estado.indices_visiveis = indices;
    ui.set_musicas(ModelRc::new(VecModel::from(modelo)));

    atualizar_selecao_visivel(estado, ui);
}

/// Espelha a lista de pastas e os índices das abas na UI.
fn atualizar_abas(estado: &EstadoAudio, ui: &MainWindow) {
    let nomes: Vec<SharedString> = estado
        .pastas
        .iter()
        .map(|p| p.nome.as_str().into())
        .collect();
    ui.set_abas(ModelRc::new(VecModel::from(nomes)));
    ui.set_aba_ativa(estado.aba_visivel as i32);
    ui.set_aba_tocando(estado.pasta_reproducao.map_or(-1, |i| i as i32));
}

/// Escaneia a pasta na primeira vez que ela é necessária. Idempotente.
fn garantir_pasta_carregada(estado: &mut EstadoAudio, indice: usize) {
    let escanear = estado.escanear_subpastas;
    let Some(pasta) = estado.pastas.get_mut(indice) else {
        return;
    };
    if pasta.carregada {
        return;
    }

    let mut tracks = Vec::new();
    coletar_arquivos_audio(&pasta.caminho, &pasta.caminho, escanear, &mut tracks);
    tracks.sort_by(|a, b| a.path.cmp(&b.path));
    pasta.tracks = tracks;
    pasta.carregada = true;
}

/// Abre uma pasta: entra como nova aba, ou só troca para a aba já existente.
/// Não interfere na reprodução em andamento.
fn abrir_pasta(estado: &mut EstadoAudio, ui: &MainWindow, caminho: PathBuf) {
    if let Some(idx) = estado.pastas.iter().position(|p| p.caminho == caminho) {
        estado.aba_visivel = idx;
        // Pode ser uma pasta que ainda não foi visitada nesta sessão
        garantir_pasta_carregada(estado, idx);
        atualizar_abas(estado, ui);
        atualizar_lista(estado, ui);
        return;
    }

    let mut tracks = Vec::new();
    coletar_arquivos_audio(&caminho, &caminho, estado.escanear_subpastas, &mut tracks);
    tracks.sort_by(|a, b| a.path.cmp(&b.path));

    let nome = nome_da_pasta(&caminho);
    estado.pastas.push(PastaMusical {
        caminho,
        nome,
        tracks,
        // Já escaneada acima: não passa pelo caminho preguiçoso
        carregada: true,
    });
    estado.aba_visivel = estado.pastas.len() - 1;

    atualizar_abas(estado, ui);
    atualizar_lista(estado, ui);
}

/// Fecha uma aba. Se era a pasta que estava tocando, a reprodução para.
fn fechar_pasta(estado: &mut EstadoAudio, ui: &MainWindow, idx: usize) {
    if idx >= estado.pastas.len() {
        return;
    }

    if estado.pasta_reproducao == Some(idx) {
        stop_music(estado, ui);
        estado.pasta_reproducao = None;
        estado.indice_atual = None;
        estado.arquivo_atual = None;
    } else if let Some(p) = estado.pasta_reproducao {
        if p > idx {
            estado.pasta_reproducao = Some(p - 1);
        }
    }

    estado.pastas.remove(idx);

    if estado.aba_visivel > idx {
        estado.aba_visivel -= 1;
    } else if estado.aba_visivel >= estado.pastas.len() {
        // Cai para a última aba restante, que pode nunca ter sido visitada
        estado.aba_visivel = estado.pastas.len().saturating_sub(1);
    }

    let aba = estado.aba_visivel;
    garantir_pasta_carregada(estado, aba);

    atualizar_abas(estado, ui);
    atualizar_lista(estado, ui);
}

/// Reescaneia todas as pastas (a opção de subpastas é global) e reposiciona a
/// faixa atual pelo caminho, já que os índices podem ter mudado.
fn reescaneiar_pastas(estado: &mut EstadoAudio, ui: &MainWindow) {
    let escanear = estado.escanear_subpastas;
    for pasta in &mut estado.pastas {
        // Pastas ainda não carregadas serão escaneadas com a nova opção quando
        // forem abertas; carregá-las agora só gastaria memória à toa.
        if !pasta.carregada {
            continue;
        }

        let mut tracks = Vec::new();
        coletar_arquivos_audio(&pasta.caminho, &pasta.caminho, escanear, &mut tracks);
        tracks.sort_by(|a, b| a.path.cmp(&b.path));
        pasta.tracks = tracks;
    }

    // Índices podem ter mudado com o reescaneio; a sacola do shuffle
    // inteligente não serve mais.
    estado.sacola_shuffle.clear();

    if let (Some(p), Some(nome)) = (estado.pasta_reproducao, estado.arquivo_atual.clone()) {
        let novo = estado
            .pastas
            .get(p)
            .and_then(|pasta| pasta.tracks.iter().position(|t| t.path == nome));
        match novo {
            Some(pos) => estado.indice_atual = Some(pos),
            None => {
                // A faixa atual já não está na pasta
                estado.pasta_reproducao = None;
                estado.indice_atual = None;
                estado.arquivo_atual = None;
                stop_music(estado, ui);
            }
        }
    }

    atualizar_abas(estado, ui);
    atualizar_lista(estado, ui);
}

// ---------------------------------------------------------------------
// Letras sincronizadas (.lrc)
// ---------------------------------------------------------------------

/// Quantas linhas mandamos pra UI ao redor da linha atual. Ímpar, para a
/// linha atual poder ficar centralizada.
const LETRA_JANELA: usize = 9;

/// Caminho do .lrc irmão da faixa: mesmo nome, extensão trocada.
fn caminho_lrc(caminho_faixa: &std::path::Path) -> PathBuf {
    caminho_faixa.with_extension("lrc")
}

/// Converte "mm:ss", "mm:ss.xx" ou "mm:ss.xxx" em segundos.
fn parse_tempo(dentro: &str) -> Option<f64> {
    let (minutos, resto) = dentro.split_once(':')?;
    let minutos: u64 = minutos.trim().parse().ok()?;

    let (segundos, fracao) = match resto.split_once('.') {
        Some((s, f)) => (s, Some(f)),
        None => (resto, None),
    };
    let segundos: u64 = segundos.trim().parse().ok()?;

    let fracao = match fracao {
        None => 0.0,
        Some(f) => {
            let f = f.trim();
            // 1 a 3 dígitos: ".5", ".50" e ".500" valem 500 ms
            if f.is_empty() || f.len() > 3 || !f.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let valor: u64 = f.parse().ok()?;
            match f.len() {
                1 => valor as f64 / 10.0,
                2 => valor as f64 / 100.0,
                _ => valor as f64 / 1000.0,
            }
        }
    };

    Some(minutos as f64 * 60.0 + segundos as f64 + fracao)
}

/// Parseia um .lrc. Linhas fora do padrão `[tempo]texto` são ignoradas em
/// silêncio, e uma linha com vários timestamps gera uma entrada por timestamp.
fn parse_lrc(conteudo: &str) -> Vec<(f64, String)> {
    let mut linhas: Vec<(f64, String)> = Vec::new();

    for linha in conteudo.lines() {
        let mut resto = linha;
        let mut tempos: Vec<f64> = Vec::new();

        // Consome todos os `[tempo]` no começo da linha
        while let Some(fim) = resto.find(']') {
            if !resto.starts_with('[') {
                break;
            }
            match parse_tempo(&resto[1..fim]) {
                Some(t) => tempos.push(t),
                // Metadado ([ar:...], [ti:...]) ou lixo: a linha não serve
                None => break,
            }
            resto = &resto[fim + 1..];
        }

        if tempos.is_empty() {
            continue;
        }

        let texto = resto.trim().to_string();
        for t in tempos {
            linhas.push((t, texto.clone()));
        }
    }

    linhas.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    linhas
}

/// Índice da última linha cujo tempo é ≤ `tempo` (-1 se ainda não chegou).
fn indice_letra(letra: &[(f64, String)], tempo: f64) -> i32 {
    let n = letra.partition_point(|(t, _)| *t <= tempo);
    if n == 0 { -1 } else { (n - 1) as i32 }
}

/// Recorte de linhas ao redor da linha atual. Devolve (início, linhas, destaque).
/// A janela fica centrada na linha atual, exceto perto das pontas — é isso
/// que dá a sensação de rolagem em vez de um texto trocando no lugar.
fn janela_letra(letra: &[(f64, String)], indice: i32) -> (usize, Vec<SharedString>, i32) {
    if letra.is_empty() {
        return (0, Vec::new(), -1);
    }

    let inicio = if indice < 0 {
        0
    } else {
        let atual = indice as usize;
        let metade = LETRA_JANELA / 2;
        if atual < metade {
            0
        } else if atual + metade + 1 > letra.len() {
            letra.len().saturating_sub(LETRA_JANELA)
        } else {
            atual - metade
        }
    };

    let fim = (inicio + LETRA_JANELA).min(letra.len());
    let linhas = letra[inicio..fim]
        .iter()
        .map(|(_, t)| t.as_str().into())
        .collect();
    let destaque = if indice < 0 {
        -1
    } else {
        indice - inicio as i32
    };

    (inicio, linhas, destaque)
}

/// Carrega a letra da faixa e já publica o primeiro recorte. Não existir .lrc
/// é o caso normal ("sem letra disponível"), não uma falha.
fn carregar_letra(
    estado: &mut EstadoAudio,
    ui: &MainWindow,
    caminho_faixa: &std::path::Path,
    tempo: f64,
) {
    estado.letra_atual = fs::read_to_string(caminho_lrc(caminho_faixa))
        .map(|conteudo| parse_lrc(&conteudo))
        .unwrap_or_default();
    estado.letra_indice = indice_letra(&estado.letra_atual, tempo);
    publicar_letra(estado, ui);
}

/// Recalcula a linha atual e só mexe na UI quando ela muda.
fn sincronizar_letra(estado: &mut EstadoAudio, ui: &MainWindow) {
    let indice = indice_letra(&estado.letra_atual, estado.tempo_decorrido);
    if indice == estado.letra_indice {
        return;
    }
    estado.letra_indice = indice;
    publicar_letra(estado, ui);
}

/// Manda pra UI o recorte de linhas ao redor da linha atual.
fn publicar_letra(estado: &mut EstadoAudio, ui: &MainWindow) {
    let (inicio, janela, destaque) = janela_letra(&estado.letra_atual, estado.letra_indice);
    estado.letra_janela_inicio = inicio;
    ui.set_letra(ModelRc::new(VecModel::from(janela)));
    ui.set_letra_destaque(destaque);
}

fn executar_seek(estado: &mut EstadoAudio, ui: &MainWindow, alvo_secs: u64) {
    // Sempre a pasta de onde vem a faixa atual, nunca a aba visível.
    let pasta = match estado.pasta_reproducao.and_then(|i| estado.pastas.get(i)) {
        Some(p) => p.caminho.clone(),
        None => return,
    };
    let nome_arquivo = match estado.arquivo_atual.clone() {
        Some(n) => n,
        None => return,
    };
    let caminho_completo = pasta.join(&nome_arquivo);

    estado.audio_player = None;

    let arquivo = match File::open(&caminho_completo) {
        Ok(a) => a,
        Err(_) => return,
    };
    let (stream, stream_handle) = match OutputStream::try_default() {
        Ok(s) => s,
        Err(_) => return,
    };
    let sink = match Sink::try_new(&stream_handle) {
        Ok(s) => s,
        Err(_) => return,
    };
    sink.set_volume(estado.volume_atual);

    let leitor = BufReader::new(arquivo);
    let decodificador = match Decoder::new(leitor) {
        Ok(d) => d,
        Err(_) => return,
    };

    let total_secs = if caminho_completo
        .extension()
        .map_or(false, |e| e.eq_ignore_ascii_case("mp3"))
    {
        mp3_duration::from_path(&caminho_completo)
            .map(|d| d.as_secs())
            .unwrap_or_else(|_| {
                decodificador
                    .total_duration()
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
            })
    } else {
        decodificador
            .total_duration()
            .map(|d| d.as_secs())
            .unwrap_or(0)
    };
    estado.duracao_total_secs = total_secs;

    sink.append(decodificador);
    if alvo_secs > 0 {
        let _ = sink.try_seek(Duration::from_secs(alvo_secs));
    }

    estado.tempo_decorrido = alvo_secs as f64;
    estado.ultimo_segundo = alvo_secs;
    estado.audio_player = Some((stream, sink));

    ui.set_progresso(if total_secs > 0 {
        alvo_secs as f32 / total_secs as f32
    } else {
        0.0
    });
    ui.set_texto_play_pause("Pause".into());
    ui.set_tocando(true);
    let com_horas = total_secs >= 3600;
    ui.set_texto_tempo(
        format!(
            "{} / {}",
            formatar_tempo(alvo_secs, com_horas),
            formatar_tempo(total_secs, com_horas)
        )
        .into(),
    );
}

/// Toca uma faixa de uma pasta específica, começando em `alvo_secs`.
///
/// É este caminho que fixa de onde vem o som: `pasta_reproducao` passa a ser
/// `pasta_idx`, independente da aba que está sendo exibida.
fn iniciar_faixa(
    estado: &mut EstadoAudio,
    ui: &MainWindow,
    pasta_idx: usize,
    track_idx: usize,
    alvo_secs: u64,
) {
    garantir_pasta_carregada(estado, pasta_idx);

    let (caminho, pasta_raiz) = match estado.pastas.get(pasta_idx) {
        Some(pasta) => match pasta.tracks.get(track_idx) {
            Some(t) => (t.path.clone(), pasta.caminho.clone()),
            None => return,
        },
        None => return,
    };

    // Troca de faixa: descarta a letra anterior e tenta carregar a nova
    carregar_letra(estado, ui, &pasta_raiz.join(&caminho), alvo_secs as f64);

    if estado.pasta_reproducao != Some(pasta_idx) {
        // Pasta de reprodução mudou: a sacola do shuffle inteligente era
        // da pasta antiga, não serve mais aqui.
        estado.sacola_shuffle.clear();
    }

    estado.pasta_reproducao = Some(pasta_idx);
    estado.indice_atual = Some(track_idx);
    estado.arquivo_atual = Some(caminho);

    atualizar_selecao_visivel(estado, ui);
    executar_seek(estado, ui, alvo_secs);
}

fn tocar_faixa(estado: &mut EstadoAudio, ui: &MainWindow, pasta_idx: usize, track_idx: usize) {
    iniciar_faixa(estado, ui, pasta_idx, track_idx, 0);
}

/// Reposiciona a faixa mantendo o estado de pausa. `executar_seek` recria o
/// player já tocando, então re-pausamos quando for o caso.
fn seek_mantendo_pausa(estado: &mut EstadoAudio, ui: &MainWindow, alvo_secs: u64) {
    let Some(estava_pausado) = estado
        .audio_player
        .as_ref()
        .map(|(_, sink)| sink.is_paused())
    else {
        // Sem faixa carregada não há o que reposicionar
        return;
    };

    executar_seek(estado, ui, alvo_secs);

    if estava_pausado {
        if let Some((_, ref sink)) = estado.audio_player {
            sink.pause();
            ui.set_texto_play_pause("Play".into());
            ui.set_tocando(false);
        }
    }
}

fn tocar_anterior(estado: &mut EstadoAudio, ui: &MainWindow) {
    // Navega na pasta de onde vem a faixa atual, nunca na aba visível.
    let alvo = {
        let Some(pasta_idx) = estado.pasta_reproducao else {
            return;
        };
        let Some(pasta) = estado.pastas.get(pasta_idx) else {
            return;
        };

        let ordem = ordem_filtrada(&pasta.tracks, &estado.filtro);
        if ordem.is_empty() {
            return;
        }

        let pos = estado
            .indice_atual
            .and_then(|idx| ordem.iter().position(|&i| i == idx));
        let track_idx = match pos {
            Some(0) => ordem[ordem.len() - 1],
            Some(p) => ordem[p - 1],
            // Faixa atual fora do filtro: começa pela última
            None => ordem[ordem.len() - 1],
        };
        (pasta_idx, track_idx)
    };
    tocar_faixa(estado, ui, alvo.0, alvo.1);
}

fn alternar_play_pause(estado: &EstadoAudio, ui: &MainWindow) {
    if let Some((_, ref sink)) = estado.audio_player {
        if sink.is_paused() {
            sink.play();
            ui.set_texto_play_pause("Pause".into());
            ui.set_tocando(true);
        } else {
            sink.pause();
            ui.set_texto_play_pause("Play".into());
            ui.set_tocando(false);
        }
    }
}

fn pausar(estado: &EstadoAudio, ui: &MainWindow) {
    if let Some((_, ref sink)) = estado.audio_player {
        if !sink.is_paused() {
            sink.pause();
            ui.set_texto_play_pause("Play".into());
            ui.set_tocando(false);
        }
    }
}

fn reproduzir(estado: &EstadoAudio, ui: &MainWindow) {
    if let Some((_, ref sink)) = estado.audio_player {
        if sink.is_paused() {
            sink.play();
            ui.set_texto_play_pause("Pause".into());
            ui.set_tocando(true);
        }
    }
}

fn stop_music(estado: &mut EstadoAudio, ui: &MainWindow) {
    estado.audio_player = None;
    estado.tempo_decorrido = 0.0;
    estado.ultimo_segundo = 0;
    ui.set_progresso(0.0);
    ui.set_texto_play_pause("Play".into());
    ui.set_tocando(false);
    ui.set_texto_tempo("00:00 / 00:00".into());

    // Nada tocando: some com a letra da faixa anterior
    estado.letra_atual.clear();
    estado.letra_indice = -1;
    publicar_letra(estado, ui);
}

fn tocar_proxima_manual(estado: &mut EstadoAudio, ui: &MainWindow) {
    // Navega na pasta de onde vem a faixa atual, nunca na aba visível.
    let alvo = {
        let Some(pasta_idx) = estado.pasta_reproducao else {
            return;
        };
        let Some(pasta) = estado.pastas.get(pasta_idx) else {
            return;
        };

        let ordem = ordem_filtrada(&pasta.tracks, &estado.filtro);
        if ordem.is_empty() {
            return;
        }

        let pos = estado
            .indice_atual
            .and_then(|idx| ordem.iter().position(|&i| i == idx));

        if estado.modo_shuffle == 2 {
            // Shuffle inteligente: consome de uma "sacola" embaralhada até
            // esvaziar; só então reembaralha uma nova. Garante que todas
            // as faixas tocam uma vez antes de qualquer repetição.
            if estado.sacola_shuffle.is_empty() {
                let mut rng = rand::thread_rng();
                let mut nova_sacola = ordem.clone();
                nova_sacola.shuffle(&mut rng);

                // Evita repetição colada na emenda entre um ciclo e o
                // outro: a próxima a sair é `nova_sacola.last()` (o pop
                // consome do fim) — se for igual à última tocada, troca de
                // lugar com outra posição.
                if nova_sacola.len() > 1 && nova_sacola.last().copied() == estado.indice_atual {
                    let ultimo = nova_sacola.len() - 1;
                    nova_sacola.swap(0, ultimo);
                }

                estado.sacola_shuffle = nova_sacola;
            }
            estado.sacola_shuffle.pop().map(|i| (pasta_idx, i))
        } else if estado.modo_shuffle == 1 {
            // Aleatório de verdade: sorteia entre todas as faixas, sem
            // excluir nada (pode repetir, inclusive de seguida — é assim
            // que aleatório de verdade se comporta). Quem quiser evitar
            // repetição é o Shuffle Inteligente.
            let mut rng = rand::thread_rng();
            ordem.choose(&mut rng).copied().map(|i| (pasta_idx, i))
        } else {
            match pos {
                Some(p) if p + 1 < ordem.len() => Some((pasta_idx, ordem[p + 1])),
                Some(_) if estado.modo_loop == 2 => Some((pasta_idx, ordem[0])),
                Some(_) => None, // fim da pasta: para
                None => Some((pasta_idx, ordem[0])),
            }
        }
    };

    match alvo {
        Some((pasta_idx, track_idx)) => tocar_faixa(estado, ui, pasta_idx, track_idx),
        None => stop_music(estado, ui),
    }
}

fn proxima_musica_auto(estado: &mut EstadoAudio, ui: &MainWindow) {
    if estado.modo_loop == 1 {
        executar_seek(estado, ui, 0);
    } else {
        tocar_proxima_manual(estado, ui);
    }
}

/// Restaura a faixa e a posição salvas. A pasta de reprodução já foi
/// resolvida (e migrada) no carregamento da config.
fn restaurar_posicao(estado: &mut EstadoAudio, ui: &MainWindow, config: &AppConfig) {
    let (Some(pasta_idx), Some(indice), Some(tempo)) = (
        estado.pasta_reproducao,
        config.indice_atual,
        config.tempo_atual,
    ) else {
        return;
    };

    let existe = estado
        .pastas
        .get(pasta_idx)
        .is_some_and(|p| indice < p.tracks.len());
    if !existe {
        estado.pasta_reproducao = None;
        return;
    }

    iniciar_faixa(estado, ui, pasta_idx, indice, tempo);
}

fn atualizar_progresso(estado: &mut EstadoAudio, ui: &MainWindow) {
    let total_secs = estado.duracao_total_secs;

    let terminou = match estado.audio_player {
        Some((_, ref sink)) => sink.empty() && total_secs > 0,
        None => false,
    };
    if terminou {
        proxima_musica_auto(estado, ui);
        return;
    }

    // Letra: recalcula a linha atual (também quando pausado, após um seek)
    sincronizar_letra(estado, ui);

    let pausado = match estado.audio_player {
        Some((_, ref sink)) => sink.is_paused(),
        None => return,
    };

    if !pausado {
        estado.tempo_decorrido += 0.25;
        let atual_secs = estado.tempo_decorrido as u64;
        if atual_secs != estado.ultimo_segundo {
            estado.ultimo_segundo = atual_secs;
            ui.set_progresso(if total_secs > 0 {
                atual_secs as f32 / total_secs as f32
            } else {
                0.0
            });
            let com_horas = total_secs >= 3600;
            ui.set_texto_tempo(
                format!(
                    "{} / {}",
                    formatar_tempo(atual_secs, com_horas),
                    formatar_tempo(total_secs, com_horas)
                )
                .into(),
            );
        }
    }
}

// ---------------------------------------------------------------------
// Controles multimídia do sistema (SMTC no Windows)
// ---------------------------------------------------------------------

/// Contorna um bug conhecido do renderer de software do Slint no Windows: o
/// rastreamento de "região suja" às vezes não marca a janela inteira como
/// suja quando o fundo muda (troca de tema) ou em certos eventos de
/// composição do DWM (Aero Snap, redimensionar, trocar de monitor) — o
/// sintoma é a janela ficando parcialmente transparente até algo mais
/// forçar o redesenho daquele pedaço. `RedrawWindow` com essas flags força
/// um repaint completo e imediato, sem esperar o rastreamento de região
/// suja decidir sozinho.
#[cfg(target_os = "windows")]
fn forcar_repaint_completo(hwnd: HWND) {
    unsafe {
        let _ = RedrawWindow(
            Some(hwnd),
            None,
            None,
            RDW_INVALIDATE | RDW_UPDATENOW | RDW_ERASE | RDW_ALLCHILDREN,
        );
    }
}

fn obter_hwnd(ui: &MainWindow) -> Option<*mut std::ffi::c_void> {
    use raw_window_handle::HasWindowHandle;
    let handle = ui.window().window_handle();
    let raw = handle.window_handle().ok()?.as_raw();
    match raw {
        raw_window_handle::RawWindowHandle::Win32(win32) => {
            Some(win32.hwnd.get() as *mut std::ffi::c_void)
        }
        _ => None,
    }
}

fn configurar_controles_multimidia(
    ui: &MainWindow,
    tx: Sender<MediaControlEvent>,
) -> Option<MediaControls> {
    let hwnd = obter_hwnd(ui);
    let config = PlatformConfig {
        display_name: "Furinar",
        dbus_name: "furinar",
        hwnd,
    };
    let mut controles = MediaControls::new(config).ok()?;
    controles
        .attach(move |event| {
            let _ = tx.send(event);
        })
        .ok()?;
    Some(controles)
}

fn processar_eventos_multimidia(
    rx: &Receiver<MediaControlEvent>,
    estado: &mut EstadoAudio,
    ui: &MainWindow,
) {
    for evento in rx.try_iter() {
        match evento {
            MediaControlEvent::Play => reproduzir(estado, ui),
            MediaControlEvent::Pause => pausar(estado, ui),
            MediaControlEvent::Toggle => alternar_play_pause(estado, ui),
            MediaControlEvent::Next => tocar_proxima_manual(estado, ui),
            MediaControlEvent::Previous => tocar_anterior(estado, ui),
            MediaControlEvent::Stop => stop_music(estado, ui),
            MediaControlEvent::Seek(direcao) => {
                let delta = match direcao {
                    SeekDirection::Forward => 10i64,
                    SeekDirection::Backward => -10i64,
                };
                let alvo = (estado.tempo_decorrido as i64 + delta)
                    .clamp(0, estado.duracao_total_secs as i64) as u64;
                executar_seek(estado, ui, alvo);
            }
            MediaControlEvent::SeekBy(direcao, duracao) => {
                let delta = match direcao {
                    SeekDirection::Forward => duracao.as_secs() as i64,
                    SeekDirection::Backward => -(duracao.as_secs() as i64),
                };
                let alvo = (estado.tempo_decorrido as i64 + delta)
                    .clamp(0, estado.duracao_total_secs as i64) as u64;
                executar_seek(estado, ui, alvo);
            }
            MediaControlEvent::SetPosition(pos) => {
                executar_seek(estado, ui, pos.0.as_secs());
            }
            MediaControlEvent::SetVolume(v) => {
                let vol = v.clamp(0.0, 1.0) as f32;
                estado.volume_atual = vol;
                ui.set_volume(vol);
                if let Some((_, ref sink)) = estado.audio_player {
                    sink.set_volume(vol);
                }
                salvar_configuracao(estado);
            }
            _ => {}
        }
    }
}

fn sincronizar_controles(controles: &Rc<RefCell<Option<MediaControls>>>, estado: &mut EstadoAudio) {
    let mut guard = controles.borrow_mut();
    let Some(c) = guard.as_mut() else { return };

    // Metadados: atualiza apenas quando a faixa muda. A busca é na pasta de
    // reprodução, que é de onde o som realmente vem.
    if estado.arquivo_atual != estado.faixa_controles {
        let info = estado
            .pasta_reproducao
            .and_then(|i| estado.pastas.get(i))
            .and_then(|pasta| {
                let nome = estado.arquivo_atual.as_deref()?;
                pasta.tracks.iter().find(|t| t.path == nome)
            })
            .map(|t| (t.titulo.clone(), t.artista.clone()));

        if let Some((titulo, artista)) = info {
            let _ = c.set_metadata(MediaMetadata {
                title: Some(titulo.as_str()),
                artist: artista.as_deref(),
                album: None,
                cover_url: None,
                duration: Some(Duration::from_secs(estado.duracao_total_secs)),
            });
        }
        estado.faixa_controles = estado.arquivo_atual.clone();
    }

    // Status: atualiza quando muda (a cada segundo ou em play/pause/stop)
    let progresso = MediaPosition(Duration::from_secs(estado.ultimo_segundo));
    let playback = match &estado.audio_player {
        Some((_, sink)) if !sink.is_paused() => MediaPlayback::Playing {
            progress: Some(progresso),
        },
        Some((_, _)) => MediaPlayback::Paused {
            progress: Some(progresso),
        },
        None => MediaPlayback::Stopped,
    };
    if estado.status_controles.as_ref() != Some(&playback) {
        let _ = c.set_playback(playback.clone());
        estado.status_controles = Some(playback);
    }
}

// ---------------------------------------------------------------------
// Botões na miniatura da barra de tarefas (ITaskbarList3)
// ---------------------------------------------------------------------

// IDs dos botões (viram LOWORD(wParam) no WM_COMMAND).
#[cfg(target_os = "windows")]
const BTN_ANTERIOR: u32 = 0x100;
#[cfg(target_os = "windows")]
const BTN_PLAY_PAUSE: u32 = 0x101;
#[cfg(target_os = "windows")]
const BTN_PROXIMA: u32 = 0x102;

// Índices dos ícones na HIMAGELIST, na ordem em que são inseridos.
#[cfg(target_os = "windows")]
const IDX_ICONE_ANTERIOR: u32 = 0;
#[cfg(target_os = "windows")]
const IDX_ICONE_PLAY: u32 = 1;
#[cfg(target_os = "windows")]
const IDX_ICONE_PAUSE: u32 = 2;
#[cfg(target_os = "windows")]
const IDX_ICONE_PROXIMA: u32 = 3;

/// Recursos dos botões da taskbar. Precisam continuar vivos enquanto o app
/// roda: a shell referencia a HIMAGELIST e os HICONs são nossos.
#[cfg(target_os = "windows")]
struct BotoesTaskbar {
    taskbar: ITaskbarList3,
    hwnd: HWND,
    himl: HIMAGELIST,
    icones: Vec<HICON>,
    pausado: bool,
}

#[cfg(target_os = "windows")]
impl BotoesTaskbar {
    /// Troca o ícone do botão central entre play e pause.
    fn atualizar_play_pause(&mut self, pausado: bool) {
        if self.pausado == pausado {
            return;
        }
        self.pausado = pausado;
        let indice = if pausado {
            IDX_ICONE_PAUSE
        } else {
            IDX_ICONE_PLAY
        };
        let botoes = [criar_botao(
            BTN_PLAY_PAUSE,
            indice,
            self.icones[indice as usize],
            "Tocar/Pausar",
        )];
        unsafe {
            let _ = self.taskbar.ThumbBarUpdateButtons(self.hwnd, &botoes);
        }
    }
}

#[cfg(target_os = "windows")]
impl Drop for BotoesTaskbar {
    fn drop(&mut self) {
        unsafe {
            let _ = ImageList_Destroy(Some(self.himl));
            for icone in &self.icones {
                let _ = DestroyIcon(*icone);
            }
        }
    }
}

/// Triângulo com ápice em `apex_x` e base vertical em `base_x`.
#[cfg(target_os = "windows")]
fn forma_triangulo(x: i32, y: i32, apex_x: i32, base_x: i32) -> bool {
    let (esq, dir) = if apex_x < base_x {
        (apex_x, base_x)
    } else {
        (base_x, apex_x)
    };
    if x < esq || x > dir {
        return false;
    }
    let meia_altura = (x - apex_x).abs() * 5 / (base_x - apex_x).abs();
    (y - 8).abs() <= meia_altura
}

#[cfg(target_os = "windows")]
fn forma_play(x: i32, y: i32) -> bool {
    forma_triangulo(x, y, 12, 4)
}

#[cfg(target_os = "windows")]
fn forma_pause(x: i32, y: i32) -> bool {
    ((4..=6).contains(&x) || (9..=11).contains(&x)) && (3..=12).contains(&y)
}

#[cfg(target_os = "windows")]
fn forma_anterior(x: i32, y: i32) -> bool {
    ((2..=3).contains(&x) && (3..=12).contains(&y)) || forma_triangulo(x, y, 5, 13)
}

#[cfg(target_os = "windows")]
fn forma_proxima(x: i32, y: i32) -> bool {
    forma_triangulo(x, y, 10, 2) || ((11..=12).contains(&x) && (3..=12).contains(&y))
}

/// Gera as máscaras AND/XOR de um ícone monocromático 1bpp.
///
/// Fundo: AND=1 e XOR=0 (transparente). Forma: AND=0 e XOR=1 (branco).
#[cfg(target_os = "windows")]
fn mascaras_icone(tamanho: i32, dentro: impl Fn(i32, i32) -> bool) -> (Vec<u8>, Vec<u8>) {
    // Cada linha é preenchida até múltiplo de 2 bytes (WORD)
    let bytes_por_linha = ((tamanho + 15) / 16 * 2) as usize;
    let mut mascara_and = vec![0u8; bytes_por_linha * tamanho as usize];
    let mut mascara_xor = vec![0u8; bytes_por_linha * tamanho as usize];

    for y in 0..tamanho {
        for x in 0..tamanho {
            let offset = y as usize * bytes_por_linha + (x / 8) as usize;
            let bit = 0x80u8 >> (x % 8);
            if dentro(x, y) {
                mascara_xor[offset] |= bit;
            } else {
                mascara_and[offset] |= bit;
            }
        }
    }

    (mascara_and, mascara_xor)
}

#[cfg(target_os = "windows")]
fn criar_icone(tamanho: i32, dentro: impl Fn(i32, i32) -> bool) -> windows::core::Result<HICON> {
    let (mascara_and, mascara_xor) = mascaras_icone(tamanho, dentro);
    let modulo = unsafe { GetModuleHandleW(None)? };
    unsafe {
        CreateIcon(
            Some(HINSTANCE(modulo.0)),
            tamanho,
            tamanho,
            1,
            1,
            mascara_and.as_ptr(),
            mascara_xor.as_ptr(),
        )
    }
}

#[cfg(target_os = "windows")]
fn criar_botao(id: u32, indice_icone: u32, icone: HICON, dica: &str) -> THUMBBUTTON {
    let mut sz_tip = [0u16; 260];
    for (i, c) in dica.encode_utf16().take(259).enumerate() {
        sz_tip[i] = c;
    }
    THUMBBUTTON {
        dwMask: THB_ICON | THB_BITMAP | THB_TOOLTIP | THB_FLAGS,
        iId: id,
        iBitmap: indice_icone,
        hIcon: icone,
        szTip: sz_tip,
        dwFlags: THBF_ENABLED,
    }
}

/// Cria a barra de botões na miniatura da taskbar. Devolve `None` (sem quebrar
/// o app) se qualquer passo falhar.
#[cfg(target_os = "windows")]
fn configurar_botoes_taskbar(ui: &MainWindow) -> Option<BotoesTaskbar> {
    let hwnd = HWND(obter_hwnd(ui)?);

    let taskbar: ITaskbarList3 = unsafe {
        CoCreateInstance(
            &TaskbarList,
            None::<&windows::core::IUnknown>,
            CLSCTX_INPROC_SERVER,
        )
        .ok()?
    };
    unsafe {
        taskbar.HrInit().ok()?;
    }

    let tamanho = unsafe { GetSystemMetrics(SM_CXSMICON) };
    if tamanho <= 0 {
        return None;
    }

    // Cria os ícones; se algum falhar, libera os que já foram criados.
    let formas: [fn(i32, i32) -> bool; 4] =
        [forma_anterior, forma_play, forma_pause, forma_proxima];
    let mut icones: Vec<HICON> = Vec::with_capacity(formas.len());
    for forma in formas {
        match criar_icone(tamanho, forma) {
            Ok(icone) => icones.push(icone),
            Err(_) => {
                for icone in &icones {
                    unsafe {
                        let _ = DestroyIcon(*icone);
                    }
                }
                return None;
            }
        }
    }

    let himl = unsafe { ImageList_Create(tamanho, tamanho, ILC_COLOR32 | ILC_MASK, 4, 1) };
    if himl.is_invalid() {
        for icone in &icones {
            unsafe {
                let _ = DestroyIcon(*icone);
            }
        }
        return None;
    }

    // A partir daqui a struct já cuida de liberar os recursos se algo falhar.
    let botoes = BotoesTaskbar {
        taskbar,
        hwnd,
        himl,
        icones,
        pausado: false,
    };

    unsafe {
        for icone in &botoes.icones {
            ImageList_ReplaceIcon(botoes.himl, -1, *icone);
        }

        botoes
            .taskbar
            .ThumbBarSetImageList(botoes.hwnd, botoes.himl)
            .ok()?;

        let lista = [
            criar_botao(
                BTN_ANTERIOR,
                IDX_ICONE_ANTERIOR,
                botoes.icones[0],
                "Anterior",
            ),
            criar_botao(
                BTN_PLAY_PAUSE,
                IDX_ICONE_PLAY,
                botoes.icones[1],
                "Tocar/Pausar",
            ),
            criar_botao(BTN_PROXIMA, IDX_ICONE_PROXIMA, botoes.icones[3], "Próxima"),
        ];
        botoes
            .taskbar
            .ThumbBarAddButtons(botoes.hwnd, &lista)
            .ok()?;
    }

    Some(botoes)
}

/// Trata os cliques que o message hook mandou pelo canal.
#[cfg(target_os = "windows")]
fn processar_eventos_taskbar(rx: &Receiver<u32>, estado: &mut EstadoAudio, ui: &MainWindow) {
    for id in rx.try_iter() {
        match id {
            BTN_ANTERIOR => tocar_anterior(estado, ui),
            BTN_PLAY_PAUSE => alternar_play_pause(estado, ui),
            BTN_PROXIMA => tocar_proxima_manual(estado, ui),
            _ => {}
        }
    }
}

/// Mantém o ícone do botão central coerente com o estado de reprodução.
#[cfg(target_os = "windows")]
fn atualizar_icone_taskbar(botoes: &Rc<RefCell<Option<BotoesTaskbar>>>, estado: &EstadoAudio) {
    let pausado = match &estado.audio_player {
        Some((_, sink)) => sink.is_paused(),
        // Parado: o botão oferece "tocar"
        None => true,
    };
    if let Some(b) = botoes.borrow_mut().as_mut() {
        b.atualizar_play_pause(pausado);
    }
}

/// Cantos arredondados nativos (Windows 11). Em versões que não conhecem o
/// atributo a chamada falha e é simplesmente ignorada.
#[cfg(target_os = "windows")]
fn arredondar_cantos(ui: &MainWindow) {
    let Some(hwnd) = obter_hwnd(ui).map(HWND) else {
        return;
    };

    let preferencia: DWM_WINDOW_CORNER_PREFERENCE = DWMWCP_ROUND;
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preferencia as *const _ as *const std::ffi::c_void,
            std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        );
    }
}

// ---------------------------------------------------------------------
// Stubs para Linux/Outros (funções da taskbar)
// ---------------------------------------------------------------------

#[cfg(not(target_os = "windows"))]
fn processar_eventos_taskbar(_rx: &Receiver<u32>, _estado: &mut EstadoAudio, _ui: &MainWindow) {}

#[cfg(not(target_os = "windows"))]
fn atualizar_icone_taskbar(_botoes: &Rc<RefCell<Option<()>>>, _estado: &EstadoAudio) {}

#[cfg(not(target_os = "windows"))]
fn configurar_botoes_taskbar(_ui: &MainWindow) -> Option<()> {
    None
}

#[cfg(not(target_os = "windows"))]
fn arredondar_cantos(_ui: &MainWindow) {}

// ---------------------------------------------------------------------
// main
// ---------------------------------------------------------------------

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Canal dos cliques nos botões da miniatura da taskbar
    #[cfg(target_os = "windows")]
    let (tx_taskbar, rx_taskbar) = mpsc::channel::<u32>();

    // Injeta um message hook no event loop do winit (via backend do Slint) para
    // observar o `WM_COMMAND` que a shell manda ao clicar nos botões. Precisa
    // ser antes de qualquer janela existir.
    #[cfg(target_os = "windows")]
    {
        let mut construtor_eventos: slint::winit_030::EventLoopBuilder =
            slint::winit_030::winit::event_loop::EventLoop::with_user_event();
        construtor_eventos.with_msg_hook(move |msg| {
            // Só observa: retorna `false` para o winit despachar a mensagem normal.
            let msg = msg as *const MSG;
            if !msg.is_null() {
                let msg = unsafe { &*msg };
                if msg.message == WM_COMMAND {
                    let id = (msg.wParam.0 & 0xFFFF) as u32;
                    let codigo = ((msg.wParam.0 >> 16) & 0xFFFF) as u32;
                    if codigo == THBN_CLICKED && (BTN_ANTERIOR..=BTN_PROXIMA).contains(&id) {
                        let _ = tx_taskbar.send(id);
                    }
                } else if matches!(
                    msg.message,
                    WM_EXITSIZEMOVE | WM_DWMCOMPOSITIONCHANGED | WM_DISPLAYCHANGE
                ) {
                    // Gatilhos conhecidos do bug de redraw parcial do renderer
                    // de software (ver `forcar_repaint_completo`).
                    forcar_repaint_completo(msg.hwnd);
                }
            }
            false
        });
        slint::BackendSelector::new()
            .with_winit_event_loop_builder(construtor_eventos)
            .select()?;
    }

    #[cfg(not(target_os = "windows"))]
    slint::BackendSelector::new().select()?;

    let ui = MainWindow::new()?;
    let estado = Rc::new(RefCell::new(EstadoAudio::default()));

    // Canal para os eventos dos controles multimídia do sistema (SMTC)
    let (tx, rx) = mpsc::channel::<MediaControlEvent>();
    let controles = Rc::new(RefCell::new(None::<MediaControls>));
    #[cfg(target_os = "windows")]
    let botoes_taskbar = Rc::new(RefCell::new(None::<BotoesTaskbar>));
    #[cfg(not(target_os = "windows"))]
    let botoes_taskbar = Rc::new(RefCell::new(None::<()>));

    let config = AppConfig::carregar();

    {
        let mut e = estado.borrow_mut();
        e.volume_atual = config.volume;
        e.modo_loop = config.modo_loop;
        e.modo_shuffle = config.modo_shuffle;
        e.escanear_subpastas = config.escanear_subpastas;

        ui.set_volume(config.volume);
        ui.set_texto_loop(texto_loop(config.modo_loop).into());
        ui.set_loop_ativo(config.modo_loop != 0);
        ui.set_texto_shuffle(texto_shuffle(config.modo_shuffle).into());
        ui.set_shuffle_modo(config.modo_shuffle as i32);
        ui.set_escanear_subpastas(config.escanear_subpastas);

        // Aplica o tema salvo antes da primeira renderização, para não piscar
        e.tema_claro = config.tema_claro;
        ui.set_tema_escuro(!config.tema_claro);

        // Carrega as pastas salvas. Se alguma não existir mais, é pulada, e o
        // mapeamento mantém os índices salvos coerentes com a nova lista.
        let mut mapeamento: Vec<Option<usize>> = Vec::with_capacity(config.pastas.len());
        for caminho_salvo in &config.pastas {
            let caminho = PathBuf::from(caminho_salvo);
            if !caminho.is_dir() {
                mapeamento.push(None);
                continue;
            }

            let nome = nome_da_pasta(&caminho);
            // Sem escanear: as faixas entram sob demanda
            e.pastas.push(PastaMusical {
                caminho,
                nome,
                tracks: Vec::new(),
                carregada: false,
            });
            mapeamento.push(Some(e.pastas.len() - 1));
        }

        if !e.pastas.is_empty() {
            e.aba_visivel = mapeamento
                .get(config.aba_visivel_salva)
                .copied()
                .flatten()
                .unwrap_or(0);
        }
        e.pasta_reproducao = config
            .pasta_reproducao_salva
            .and_then(|i| mapeamento.get(i).copied().flatten());

        // Só a aba visível e a pasta que estava tocando precisam das faixas agora
        let aba = e.aba_visivel;
        garantir_pasta_carregada(&mut e, aba);
        if let Some(p) = e.pasta_reproducao {
            garantir_pasta_carregada(&mut e, p);
        }

        atualizar_abas(&e, &ui);
        atualizar_lista(&mut e, &ui);
    }

    // Tenta restaurar a faixa e a posição anteriores
    {
        let mut e = estado.borrow_mut();
        restaurar_posicao(&mut e, &ui, &config);
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_abrir_pasta(move || {
            if let Some(caminho) = rfd::FileDialog::new().pick_folder() {
                if let Some(ui) = ui_fraca.upgrade() {
                    let mut e = estado.borrow_mut();
                    // Pasta nova: limpa a busca
                    e.filtro.clear();
                    ui.set_texto_busca("".into());
                    abrir_pasta(&mut e, &ui, caminho);
                    salvar_configuracao(&e);
                }
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_aba_selecionada(move |indice| {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                let idx = indice as usize;
                if idx >= e.pastas.len() {
                    return;
                }
                // Navegar não é reproduzir: só muda o que a lista exibe
                e.aba_visivel = idx;
                // Carrega as faixas na primeira visita a esta aba
                garantir_pasta_carregada(&mut e, idx);
                atualizar_abas(&e, &ui);
                atualizar_lista(&mut e, &ui);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_fechar_aba(move |indice| {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                fechar_pasta(&mut e, &ui, indice as usize);
                salvar_configuracao(&e);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_anterior(move || {
            if let Some(ui) = ui_fraca.upgrade() {
                tocar_anterior(&mut estado.borrow_mut(), &ui);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_play_pause(move || {
            if let Some(ui) = ui_fraca.upgrade() {
                alternar_play_pause(&estado.borrow(), &ui);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_parar(move || {
            if let Some(ui) = ui_fraca.upgrade() {
                stop_music(&mut estado.borrow_mut(), &ui);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_proxima(move || {
            if let Some(ui) = ui_fraca.upgrade() {
                tocar_proxima_manual(&mut estado.borrow_mut(), &ui);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_alternar_loop(move || {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                e.modo_loop = (e.modo_loop + 1) % 3;
                ui.set_texto_loop(texto_loop(e.modo_loop).into());
                ui.set_loop_ativo(e.modo_loop != 0);
                salvar_configuracao(&e);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_alternar_shuffle(move || {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                e.modo_shuffle = (e.modo_shuffle + 1) % 3;
                // Sacola da pasta antiga não serve pro modo novo.
                e.sacola_shuffle.clear();
                ui.set_texto_shuffle(texto_shuffle(e.modo_shuffle).into());
                ui.set_shuffle_modo(e.modo_shuffle as i32);
                salvar_configuracao(&e);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_alternar_escanear_subpastas(move |ligado| {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                e.escanear_subpastas = ligado;
                salvar_configuracao(&e);
                reescaneiar_pastas(&mut e, &ui);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_trocar_tema(move |escuro| {
            // O visual já mudou sozinho pelo binding da UI com `Tema.escuro`;
            // aqui só guardamos a escolha no config.
            let mut e = estado.borrow_mut();
            e.tema_claro = !escuro;
            salvar_configuracao(&e);

            // Troca de fundo é o gatilho mais documentado do bug de redraw
            // parcial do renderer de software (ver `forcar_repaint_completo`).
            if let Some(ui) = ui_fraca.upgrade() {
                if let Some(hwnd) = obter_hwnd(&ui).map(HWND) {
                    forcar_repaint_completo(hwnd);
                }
            }
        });
    }

    {
        let estado = estado.clone();
        ui.on_volume_mudou(move |v| {
            let mut e = estado.borrow_mut();
            e.volume_atual = v;
            if let Some((_, ref sink)) = e.audio_player {
                sink.set_volume(v);
            }
            salvar_configuracao(&e);
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_progresso_mudou(move |v| {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                let alvo = (v as f64 * e.duracao_total_secs as f64) as u64;
                executar_seek(&mut e, &ui, alvo);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_seek_relativo(move |delta| {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                let alvo = (e.tempo_decorrido as i64 + delta as i64)
                    .clamp(0, e.duracao_total_secs as i64) as u64;
                seek_mantendo_pausa(&mut e, &ui, alvo);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_letra_linha_clicada(move |posicao| {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                // `posicao` é relativa ao recorte que está na UI
                let Some(absoluto) = e.letra_janela_inicio.checked_add(posicao as usize) else {
                    return;
                };
                let Some((tempo, _)) = e.letra_atual.get(absoluto) else {
                    return;
                };
                let alvo = tempo.max(0.0) as u64;
                seek_mantendo_pausa(&mut e, &ui, alvo);
                // Reflete o novo ponto na hora, sem esperar o próximo tick
                sincronizar_letra(&mut e, &ui);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_musica_selecionada(move |indice| {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                // `indice` é a posição na lista filtrada da aba visível;
                // converte para o índice real e toca a partir dessa pasta.
                let track_idx = match e.indices_visiveis.get(indice as usize) {
                    Some(&i) => i,
                    None => return,
                };
                let pasta_idx = e.aba_visivel;
                tocar_faixa(&mut e, &ui, pasta_idx, track_idx);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_busca_mudou(move |texto| {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                e.filtro = texto.to_string();
                atualizar_lista(&mut e, &ui);
            }
        });
    }

    // Timer de progresso
    let timer = Timer::default();
    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        let controles = controles.clone();
        let botoes_taskbar = botoes_taskbar.clone();
        #[cfg(target_os = "windows")]
        let mut tentativas_taskbar = 0u32;
        let mut cantos_arredondados = false;
        timer.start(TimerMode::Repeated, Duration::from_millis(250), move || {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();

                // Cantos arredondados nativos: uma vez, assim que a janela existe
                if !cantos_arredondados && obter_hwnd(&ui).is_some() {
                    cantos_arredondados = true;
                    arredondar_cantos(&ui);
                }

                // Configura os controles multimídia na primeira oportunidade:
                // a janela winit só existe depois que o event loop inicia
                if controles.borrow().is_none() {
                    if let Some(c) = configurar_controles_multimidia(&ui, tx.clone()) {
                        *controles.borrow_mut() = Some(c);
                    }
                }

                // Idem para os botões da taskbar: só quando a janela já existe,
                // e com poucas tentativas para não recriar recursos à toa.
                #[cfg(target_os = "windows")]
                if botoes_taskbar.borrow().is_none()
                    && tentativas_taskbar < 3
                    && obter_hwnd(&ui).is_some()
                {
                    tentativas_taskbar += 1;
                    if let Some(b) = configurar_botoes_taskbar(&ui) {
                        *botoes_taskbar.borrow_mut() = Some(b);
                    }
                }

                processar_eventos_multimidia(&rx, &mut e, &ui);
                #[cfg(target_os = "windows")]
                processar_eventos_taskbar(&rx_taskbar, &mut e, &ui);
                atualizar_progresso(&mut e, &ui);
                sincronizar_controles(&controles, &mut e);
                #[cfg(target_os = "windows")]
                atualizar_icone_taskbar(&botoes_taskbar, &e);
            }
        });
    }

    {
        let ui_fraca = ui.as_weak();
        ui.on_minimizar(move || {
            if let Some(ui) = ui_fraca.upgrade() {
                ui.window().set_minimized(true);
            }
        });
    }

    {
        let ui_fraca = ui.as_weak();
        ui.on_alternar_maximizado(move || {
            if let Some(ui) = ui_fraca.upgrade() {
                let janela = ui.window();
                janela.set_maximized(!janela.is_maximized());
            }
        });
    }

    // Salva o estado final ao fechar
    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_fecha_janela(move || {
            if let Some(ui) = ui_fraca.upgrade() {
                // Mesmo comportamento de antes: salva e deixa o event loop sair
                salvar_configuracao(&estado.borrow());
                let _ = ui.hide();
            }
        });
    }

    // Check for updates on startup — spawn a background thread, then use
    // a polling timer on the main thread to push data into the UI.
    let update_result: Arc<Mutex<Option<updates::UpdateInfo>>> = Arc::new(Mutex::new(None));
    {
        let update_result = update_result.clone();
        std::thread::spawn(move || {
            if let Some(info) = updates::check_for_update() {
                *update_result.lock().unwrap() = Some(info);
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let update_result = update_result.clone();
        let update_timer = Timer::default();
        update_timer.start(TimerMode::Repeated, Duration::from_millis(250), move || {
            let mut guard = update_result.lock().unwrap();
            if let Some(info) = guard.take() {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_update_versao(info.version.into());
                    ui.set_update_descricao(info.body.into());
                    ui.set_update_disponivel(true);
                }
            }
        });
    }

    // Wire up the update download callback
    {
        let ui_weak = ui.as_weak();
        let update_result = update_result.clone();
        ui.on_baixar_atualizacao(move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_atualizando(true);
                let ui_weak2 = ui.as_weak();
                let update_result2 = update_result.clone();
                std::thread::spawn(move || {
                    let result = updates::perform_update();
                    if let Some(ui) = ui_weak2.upgrade() {
                        ui.set_atualizando(false);
                        match result {
                            Ok(()) => {
                                ui.set_update_disponivel(false);
                                println!("Update complete! Please restart Furinar.");
                            }
                            Err(e) => {
                                eprintln!("Update failed: {e}");
                            }
                        }
                    }
                    drop(update_result2);
                });
            }
        });
    }

    ui.run()?;

    // Salva novamente ao encerrar (fallback)
    salvar_configuracao(&estado.borrow());

    Ok(())
}
