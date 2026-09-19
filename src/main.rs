slint::include_modules!();

use std::cell::RefCell;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use lofty::config::ParseOptions;
use lofty::prelude::*;
use lofty::probe::Probe;
use rand::seq::SliceRandom;
use rodio::{Decoder, OutputStream, Sink, Source};
use serde::{Deserialize, Serialize};
use slint::{ModelRc, SharedString, Timer, TimerMode, VecModel};
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
    SeekDirection,
};
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

// ---------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------

#[derive(Serialize, Deserialize, Default)]
struct AppConfig {
    pasta: Option<String>,
    volume: f32,
    modo_loop: u8,
    shuffle: bool,
    indice_atual: Option<usize>,
    tempo_atual: Option<u64>,
    #[serde(default)]
    escanear_subpastas: bool,
}

impl AppConfig {
    fn carregar() -> Self {
        if let Ok(conteudo) = fs::read_to_string("furinar_config.json") {
            serde_json::from_str(&conteudo).unwrap_or_default()
        } else {
            Self {
                pasta: None,
                volume: 0.8,
                modo_loop: 0,
                shuffle: false,
                indice_atual: None,
                tempo_atual: None,
                escanear_subpastas: false,
            }
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

fn texto_shuffle(ligado: bool) -> &'static str {
    if ligado {
        "Shuffle: Lig"
    } else {
        "Shuffle: Desl"
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
    chave_busca: String,     // título + artista normalizados (sem acento, minúsculo)
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
// Estado de áudio/playlist
// ---------------------------------------------------------------------

struct EstadoAudio {
    audio_player: Option<(OutputStream, Sink)>,
    tempo_decorrido: f64,
    pasta_atual: Option<PathBuf>,
    arquivo_atual: Option<String>,
    indice_atual: Option<usize>,
    volume_atual: f32,
    duracao_total_secs: u64,
    ultimo_segundo: u64,
    modo_loop: u8,
    modo_shuffle: bool,
    escanear_subpastas: bool,
    tracks: Vec<TrackInfo>,
    /// Mapeia posição na lista visível -> índice em `tracks`.
    indices_visiveis: Vec<usize>,
    /// Texto atual da busca (título/artista).
    filtro: String,
    faixa_controles: Option<String>,
    status_controles: Option<MediaPlayback>,
}

impl Default for EstadoAudio {
    fn default() -> Self {
        Self {
            audio_player: None,
            tempo_decorrido: 0.0,
            pasta_atual: None,
            arquivo_atual: None,
            indice_atual: None,
            volume_atual: 0.8,
            duracao_total_secs: 0,
            ultimo_segundo: 0,
            modo_loop: 0,
            modo_shuffle: false,
            escanear_subpastas: false,
            tracks: Vec::new(),
            indices_visiveis: Vec::new(),
            filtro: String::new(),
            faixa_controles: None,
            status_controles: None,
        }
    }
}

fn salvar_configuracao(estado: &EstadoAudio) {
    let config = AppConfig {
        pasta: estado
            .pasta_atual
            .as_ref()
            .map(|p| p.to_string_lossy().to_string()),
        volume: estado.volume_atual,
        modo_loop: estado.modo_loop,
        shuffle: estado.modo_shuffle,
        escanear_subpastas: estado.escanear_subpastas,
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
                        let mut chave = titulo.clone();
                        if let Some(a) = &artista {
                            chave.push(' ');
                            chave.push_str(a);
                        }
                        saida.push(TrackInfo {
                            path: nome.replace('\\', "/"),
                            titulo,
                            artista,
                            chave_busca: normalizar_busca(&chave),
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

/// Marca na UI a posição visível da faixa atual (-1 se ela estiver filtrada).
fn atualizar_selecao_visivel(estado: &EstadoAudio, ui: &MainWindow) {
    let pos = estado
        .indice_atual
        .and_then(|idx| estado.indices_visiveis.iter().position(|&i| i == idx));
    ui.set_indice_selecionado(pos.map_or(-1, |p| p as i32));
}

/// Reconstrói a lista visível aplicando o filtro atual e atualiza a seleção.
fn atualizar_lista(estado: &mut EstadoAudio, ui: &MainWindow) {
    let filtro = normalizar_busca(&estado.filtro);

    estado.indices_visiveis = estado
        .tracks
        .iter()
        .enumerate()
        .filter(|(_, t)| filtro.is_empty() || t.chave_busca.contains(filtro.as_str()))
        .map(|(i, _)| i)
        .collect();

    let modelo: Vec<SharedString> = estado
        .indices_visiveis
        .iter()
        .map(|&i| texto_exibicao(&estado.tracks[i]).into())
        .collect();
    ui.set_musicas(ModelRc::new(VecModel::from(modelo)));

    atualizar_selecao_visivel(estado, ui);
}

fn carregar_pasta(estado: &mut EstadoAudio, ui: &MainWindow, caminho: PathBuf) {
    estado.tracks.clear();
    estado.pasta_atual = Some(caminho.clone());

    let mut tracks = Vec::new();
    coletar_arquivos_audio(&caminho, &caminho, estado.escanear_subpastas, &mut tracks);
    tracks.sort_by(|a, b| a.path.cmp(&b.path));
    estado.tracks = tracks;

    atualizar_lista(estado, ui);
}

fn reescaneiar_pasta(estado: &mut EstadoAudio, ui: &MainWindow) {
    if let Some(pasta) = estado.pasta_atual.clone() {
        let atual = estado.arquivo_atual.clone();
        carregar_pasta(estado, ui, pasta);

        // Mantém a faixa atual seleccionada na nova lista
        if let Some(nome) = atual {
            if let Some(pos) = estado.tracks.iter().position(|t| t.path == nome) {
                estado.indice_atual = Some(pos);
            } else {
                // A faixa atual já não está na playlist
                estado.arquivo_atual = None;
                estado.indice_atual = None;
                stop_music(estado, ui);
            }
        }

        atualizar_lista(estado, ui);
    }
}

fn executar_seek(estado: &mut EstadoAudio, ui: &MainWindow, alvo_secs: u64) {
    let pasta = match estado.pasta_atual.clone() {
        Some(p) => p,
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

fn tocar_indice(estado: &mut EstadoAudio, ui: &MainWindow, index: usize) {
    if let Some(track) = estado.tracks.get(index).cloned() {
        estado.arquivo_atual = Some(track.path.clone());
        estado.indice_atual = Some(index);
        atualizar_selecao_visivel(estado, ui);
        executar_seek(estado, ui, 0);
    }
}

fn tocar_anterior(estado: &mut EstadoAudio, ui: &MainWindow) {
    // Navega dentro da lista visível, respeitando o filtro atual.
    let alvo = {
        let visiveis = &estado.indices_visiveis;
        if visiveis.is_empty() {
            return;
        }
        let pos = estado
            .indice_atual
            .and_then(|idx| visiveis.iter().position(|&i| i == idx));
        match pos {
            Some(0) => visiveis[visiveis.len() - 1],
            Some(p) => visiveis[p - 1],
            // Faixa atual fora do filtro: começa pela última visível
            None => visiveis[visiveis.len() - 1],
        }
    };
    tocar_indice(estado, ui, alvo);
}

fn alternar_play_pause(estado: &EstadoAudio, ui: &MainWindow) {
    if let Some((_, ref sink)) = estado.audio_player {
        if sink.is_paused() {
            sink.play();
            ui.set_texto_play_pause("Pause".into());
        } else {
            sink.pause();
            ui.set_texto_play_pause("Play".into());
        }
    }
}

fn pausar(estado: &EstadoAudio, ui: &MainWindow) {
    if let Some((_, ref sink)) = estado.audio_player {
        if !sink.is_paused() {
            sink.pause();
            ui.set_texto_play_pause("Play".into());
        }
    }
}

fn reproduzir(estado: &EstadoAudio, ui: &MainWindow) {
    if let Some((_, ref sink)) = estado.audio_player {
        if sink.is_paused() {
            sink.play();
            ui.set_texto_play_pause("Pause".into());
        }
    }
}

fn stop_music(estado: &mut EstadoAudio, ui: &MainWindow) {
    estado.audio_player = None;
    estado.tempo_decorrido = 0.0;
    estado.ultimo_segundo = 0;
    ui.set_progresso(0.0);
    ui.set_texto_play_pause("Play".into());
    ui.set_texto_tempo("00:00 / 00:00".into());
}

fn tocar_proxima_manual(estado: &mut EstadoAudio, ui: &MainWindow) {
    // Navega dentro da lista visível, respeitando o filtro atual.
    let alvo = {
        let visiveis = &estado.indices_visiveis;
        if visiveis.is_empty() {
            return;
        }
        let pos = estado
            .indice_atual
            .and_then(|idx| visiveis.iter().position(|&i| i == idx));

        if estado.modo_shuffle {
            let mut rng = rand::thread_rng();
            let candidatos: Vec<usize> = visiveis
                .iter()
                .copied()
                .filter(|&i| Some(i) != estado.indice_atual)
                .collect();
            candidatos
                .choose(&mut rng)
                .copied()
                .or_else(|| visiveis.first().copied())
        } else {
            match pos {
                Some(p) if p + 1 < visiveis.len() => Some(visiveis[p + 1]),
                Some(_) if estado.modo_loop == 2 => Some(visiveis[0]),
                Some(_) => None, // fim da lista visível: para
                None => Some(visiveis[0]),
            }
        }
    };

    match alvo {
        Some(i) => tocar_indice(estado, ui, i),
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

fn restaurar_posicao(estado: &mut EstadoAudio, ui: &MainWindow) {
    let config = AppConfig::carregar();

    let (indice, tempo) = match (config.indice_atual, config.tempo_atual) {
        (Some(i), Some(t)) => (i, t),
        _ => return,
    };

    let pasta_str = match config.pasta {
        Some(p) => p,
        None => return,
    };

    let caminho = PathBuf::from(&pasta_str);
    if !caminho.exists() {
        return;
    }

    carregar_pasta(estado, ui, caminho);

    if indice >= estado.tracks.len() {
        return;
    }

    tocar_indice(estado, ui, indice);
    executar_seek(estado, ui, tempo);
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
    let hwnd = obter_hwnd(ui)?;
    let config = PlatformConfig {
        display_name: "Furinar",
        dbus_name: "furinar",
        hwnd: Some(hwnd),
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

    // Metadados: atualiza apenas quando a faixa muda
    if estado.arquivo_atual != estado.faixa_controles {
        if let Some(track) = estado
            .tracks
            .iter()
            .find(|t| t.path == estado.arquivo_atual.as_deref().unwrap_or(""))
        {
            let _ = c.set_metadata(MediaMetadata {
                title: Some(track.titulo.as_str()),
                artist: track.artista.as_deref(),
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
// main
// ---------------------------------------------------------------------

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = MainWindow::new()?;
    let estado = Rc::new(RefCell::new(EstadoAudio::default()));

    // Canal para os eventos dos controles multimídia do sistema (SMTC)
    let (tx, rx) = mpsc::channel::<MediaControlEvent>();
    let controles = Rc::new(RefCell::new(None::<MediaControls>));

    {
        let mut e = estado.borrow_mut();
        let config = AppConfig::carregar();
        e.volume_atual = config.volume;
        e.modo_loop = config.modo_loop;
        e.modo_shuffle = config.shuffle;
        e.escanear_subpastas = config.escanear_subpastas;

        ui.set_volume(config.volume);
        ui.set_texto_loop(texto_loop(config.modo_loop).into());
        ui.set_texto_shuffle(texto_shuffle(config.shuffle).into());
        ui.set_escanear_subpastas(config.escanear_subpastas);

        if let Some(pasta_str) = config.pasta {
            let caminho = PathBuf::from(pasta_str);
            if caminho.exists() {
                carregar_pasta(&mut e, &ui, caminho);
            }
        }
    }

    // Tenta restaurar a posição anterior (música + tempo)
    {
        let mut e = estado.borrow_mut();
        restaurar_posicao(&mut e, &ui);
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
                    carregar_pasta(&mut e, &ui, caminho);
                    salvar_configuracao(&e);
                }
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
                e.modo_shuffle = !e.modo_shuffle;
                ui.set_texto_shuffle(texto_shuffle(e.modo_shuffle).into());
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
                reescaneiar_pasta(&mut e, &ui);
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
        ui.on_musica_selecionada(move |indice| {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                // `indice` é a posição na lista filtrada; converte para o
                // índice real em `tracks`.
                let canonico = match e.indices_visiveis.get(indice as usize) {
                    Some(&i) => i,
                    None => return,
                };
                tocar_indice(&mut e, &ui, canonico);
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
        timer.start(TimerMode::Repeated, Duration::from_millis(250), move || {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();

                // Configura os controles multimídia na primeira oportunidade:
                // a janela winit só existe depois que o event loop inicia
                if controles.borrow().is_none() {
                    if let Some(c) = configurar_controles_multimidia(&ui, tx.clone()) {
                        *controles.borrow_mut() = Some(c);
                    }
                }

                processar_eventos_multimidia(&rx, &mut e, &ui);
                atualizar_progresso(&mut e, &ui);
                sincronizar_controles(&controles, &mut e);
            }
        });
    }

    // Salva o estado final ao fechar
    {
        let estado = estado.clone();
        ui.on_fecha_janela(move || {
            salvar_configuracao(&estado.borrow());
        });
    }

    ui.run()?;

    // Salva novamente ao encerrar (fallback)
    salvar_configuracao(&estado.borrow());

    Ok(())
}
