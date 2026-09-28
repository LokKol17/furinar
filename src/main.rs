// Em release não abrimos console junto do app; no debug mantemos, para ver logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

slint::include_modules!();

mod config;
mod lrc;
mod track;
mod translations;
mod updates;

use std::cell::RefCell;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::PathBuf;
use std::rc::Rc;
#[cfg(feature = "mpris")]
use std::sync::mpsc::Sender;
#[cfg(any(feature = "mpris", target_os = "windows"))]
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use config::AppConfig;
use lrc::{caminho_lrc, indice_letra, janela_letra, parse_lrc};
use rand::seq::SliceRandom;
use rodio::decoder::DecoderError;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Source};
#[cfg(target_os = "windows")]
use slint::winit_030::winit::platform::windows::EventLoopBuilderExtWindows;
use slint::{ModelRc, SharedString, Timer, TimerMode, VecModel};
#[cfg(feature = "mpris")]
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
    SeekDirection,
};
use track::{
    TrackInfo, coletar_arquivos_audio, filtro_efetivo, nome_da_pasta, ordem_filtrada,
    texto_exibicao,
};
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

fn texto_loop(modo: u8, lang: &translations::Translations) -> String {
    let key = match modo {
        1 => "loop_track",
        2 => "loop_list",
        _ => "loop_off",
    };
    lang.get(key).copied().unwrap_or(key).to_string()
}

fn texto_shuffle(modo: u8, lang: &translations::Translations) -> String {
    let key = match modo {
        1 => "shuffle_on",
        2 => "shuffle_smart",
        _ => "shuffle_off",
    };
    lang.get(key).copied().unwrap_or(key).to_string()
}

fn texto_play(lang: &translations::Translations) -> String {
    lang.get("play").copied().unwrap_or("Play").to_string()
}

fn texto_pause(lang: &translations::Translations) -> String {
    lang.get("pause").copied().unwrap_or("Pause").to_string()
}

/// Aplica todas as traduções ao widget da UI.
fn aplicar_idioma(idioma: &str, ui: &MainWindow) {
    let t = translations::get_translations(idioma);
    let g = |k: &str| -> slint::SharedString { t.get(k).copied().unwrap_or(k).into() };
    let i18n = ui.global::<I18n>();
    i18n.set_app_name(g("app_name"));
    i18n.set_tip_minimize(g("tip_minimize"));
    i18n.set_tip_close(g("tip_close"));
    i18n.set_tip_open_folder(g("tip_open_folder"));
    i18n.set_tip_previous(g("tip_previous"));
    i18n.set_tip_stop(g("tip_stop"));
    i18n.set_tip_next(g("tip_next"));
    i18n.set_tip_lyrics(g("tip_lyrics"));
    i18n.set_tip_settings(g("tip_settings"));
    i18n.set_search_placeholder(g("search_placeholder"));
    i18n.set_no_lyrics(g("no_lyrics"));
    i18n.set_config_title(g("config_title"));
    i18n.set_theme_dark(g("theme_dark"));
    i18n.set_theme_light(g("theme_light"));
    i18n.set_scan_subfolders(g("scan_subfolders"));
    i18n.set_scan_subfolders_desc(g("scan_subfolders_desc"));
    i18n.set_update_title(g("update_title"));
    i18n.set_update_downloading(g("update_downloading"));
    i18n.set_update_button(g("update_button"));
    i18n.set_update_later(g("update_later"));
    i18n.set_play_label(g("play"));
    i18n.set_pause_label(g("pause"));
    i18n.set_loop_off(g("loop_off"));
    i18n.set_loop_track(g("loop_track"));
    i18n.set_loop_list(g("loop_list"));
    i18n.set_shuffle_off(g("shuffle_off"));
    i18n.set_shuffle_on(g("shuffle_on"));
    i18n.set_idioma(idioma.into());
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

// ---------------------------------------------------------------------
// Estado de áudio/playlist
// ---------------------------------------------------------------------

struct EstadoAudio {
    audio_player: Option<(MixerDeviceSink, Player)>,
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
    idioma: String,
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
    #[cfg(feature = "mpris")]
    faixa_controles: Option<String>,
    #[cfg(feature = "mpris")]
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
            idioma: "pt-br".to_string(),
            indices_visiveis: Vec::new(),
            filtro: String::new(),
            letra_atual: Vec::new(),
            letra_indice: -1,
            letra_janela_inicio: 0,
            #[cfg(feature = "mpris")]
            faixa_controles: None,
            #[cfg(feature = "mpris")]
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
        idioma: estado.idioma.clone(),
        indice_atual: estado.indice_atual,
        tempo_atual: if estado.tempo_decorrido > 0.0 {
            Some(estado.tempo_decorrido as u64)
        } else {
            None
        },
    };
    config.salvar();
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
    } else if let Some(p) = estado.pasta_reproducao
        && p > idx
    {
        estado.pasta_reproducao = Some(p - 1);
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

/// Abre o decodificador certo para a extensão do arquivo.
///
/// `byte_len` (tamanho do arquivo) é obrigatório: sem ele o symphonia trata a
/// fonte como não-seekable e o reader isomp4 do M4A falha logo em `try_new` —
/// no rodio 0.19 esse erro virava `unreachable!()` e derrubava o app. O hint de
/// extensão escolhe o reader do container direto, sem depender do probe.
fn criar_decodificador<R>(
    leitor: R,
    extensao: &str,
    byte_len: u64,
) -> Result<Decoder<R>, DecoderError>
where
    R: std::io::Read + std::io::Seek + Send + Sync + 'static,
{
    Decoder::builder()
        .with_data(leitor)
        .with_byte_len(byte_len)
        .with_seekable(true)
        .with_hint(extensao)
        .build()
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
    let byte_len = match arquivo.metadata() {
        Ok(m) => m.len(),
        Err(_) => return,
    };
    let stream = match DeviceSinkBuilder::open_default_sink() {
        Ok(s) => s,
        Err(_) => return,
    };
    let sink = Player::connect_new(stream.mixer());
    sink.set_volume(estado.volume_atual);

    let extensao = caminho_completo
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let leitor = BufReader::new(arquivo);
    let decodificador = match criar_decodificador(leitor, &extensao, byte_len) {
        Ok(d) => d,
        Err(_) => return,
    };

    let total_secs = if extensao == "mp3" {
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
    ui.set_texto_play_pause(texto_pause(&translations::get_translations(&estado.idioma)).into());
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

/// O que um seek vindo da UI deve fazer com a faixa atual.
#[derive(Debug, PartialEq, Eq)]
enum EfeitoSeek {
    /// Player vivo: recria o player preservando tocar/pausado.
    ManterEstado,
    /// Parado com faixa selecionada (Stop): recarrega e toca do ponto pedido.
    Recarregar,
    /// Nada carregado: não há o que reposicionar.
    Nada,
}

fn efeito_seek(tem_player: bool, tem_faixa: bool) -> EfeitoSeek {
    match (tem_player, tem_faixa) {
        (true, _) => EfeitoSeek::ManterEstado,
        (false, true) => EfeitoSeek::Recarregar,
        (false, false) => EfeitoSeek::Nada,
    }
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

    if estava_pausado && let Some((_, ref sink)) = estado.audio_player {
        sink.pause();
        ui.set_texto_play_pause(texto_play(&translations::get_translations(&estado.idioma)).into());
        ui.set_tocando(false);
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

        let filtro = filtro_efetivo(estado.aba_visivel, estado.pasta_reproducao, &estado.filtro);
        let ordem = ordem_filtrada(&pasta.tracks, filtro);
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
        let t = translations::get_translations(&estado.idioma);
        if sink.is_paused() {
            sink.play();
            ui.set_texto_play_pause(texto_pause(&t).into());
            ui.set_tocando(true);
        } else {
            sink.pause();
            ui.set_texto_play_pause(texto_play(&t).into());
            ui.set_tocando(false);
        }
    }
}

#[cfg(feature = "mpris")]
fn pausar(estado: &EstadoAudio, ui: &MainWindow) {
    if let Some((_, ref sink)) = estado.audio_player
        && !sink.is_paused()
    {
        sink.pause();
        ui.set_texto_play_pause(texto_play(&translations::get_translations(&estado.idioma)).into());
        ui.set_tocando(false);
    }
}

#[cfg(feature = "mpris")]
fn reproduzir(estado: &EstadoAudio, ui: &MainWindow) {
    if let Some((_, ref sink)) = estado.audio_player
        && sink.is_paused()
    {
        sink.play();
        ui.set_texto_play_pause(
            texto_pause(&translations::get_translations(&estado.idioma)).into(),
        );
        ui.set_tocando(true);
    }
}

fn stop_music(estado: &mut EstadoAudio, ui: &MainWindow) {
    estado.audio_player = None;
    estado.tempo_decorrido = 0.0;
    estado.ultimo_segundo = 0;
    ui.set_progresso(0.0);
    ui.set_texto_play_pause(texto_play(&translations::get_translations(&estado.idioma)).into());
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

        let filtro = filtro_efetivo(estado.aba_visivel, estado.pasta_reproducao, &estado.filtro);
        let ordem = ordem_filtrada(&pasta.tracks, filtro);
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

#[cfg(feature = "mpris")]
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

#[cfg(feature = "mpris")]
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

#[cfg(feature = "mpris")]
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
    {
        // SAFETY: chamado antes de qualquer thread ser spawned
        unsafe { std::env::set_var("WINIT_UNIX_BACKEND", "x11") };
        slint::BackendSelector::new().select()?;
    }

    let ui = MainWindow::new()?;
    let estado = Rc::new(RefCell::new(EstadoAudio::default()));

    // Canal para os eventos dos controles multimídia do sistema (SMTC)
    #[cfg(feature = "mpris")]
    let (tx, rx) = mpsc::channel::<MediaControlEvent>();
    #[cfg(feature = "mpris")]
    let controles = Rc::new(RefCell::new(None::<MediaControls>));
    #[cfg(target_os = "windows")]
    let botoes_taskbar = Rc::new(RefCell::new(None::<BotoesTaskbar>));

    let config = AppConfig::carregar();

    {
        let mut e = estado.borrow_mut();
        e.volume_atual = config.volume;
        e.modo_loop = config.modo_loop;
        e.modo_shuffle = config.modo_shuffle;
        e.escanear_subpastas = config.escanear_subpastas;
        e.idioma = config.idioma.clone();

        ui.set_volume(config.volume);
        let lang = translations::get_translations(&config.idioma);
        ui.set_texto_loop(texto_loop(config.modo_loop, &lang).into());
        ui.set_loop_ativo(config.modo_loop != 0);
        ui.set_texto_shuffle(texto_shuffle(config.modo_shuffle, &lang).into());
        ui.set_shuffle_modo(config.modo_shuffle as i32);
        ui.set_escanear_subpastas(config.escanear_subpastas);

        // Aplica o tema salvo antes da primeira renderização, para não piscar
        e.tema_claro = config.tema_claro;
        ui.set_tema_escuro(!config.tema_claro);

        // Aplica todas as traduções antes da primeira renderização
        aplicar_idioma(&config.idioma, &ui);

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
            if let Some(caminho) = rfd::FileDialog::new().pick_folder()
                && let Some(ui) = ui_fraca.upgrade()
            {
                let mut e = estado.borrow_mut();
                // Pasta nova: limpa a busca
                e.filtro.clear();
                ui.set_texto_busca("".into());
                abrir_pasta(&mut e, &ui, caminho);
                salvar_configuracao(&e);
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
                let lang = translations::get_translations(&e.idioma);
                ui.set_texto_loop(texto_loop(e.modo_loop, &lang).into());
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
                let lang = translations::get_translations(&e.idioma);
                ui.set_texto_shuffle(texto_shuffle(e.modo_shuffle, &lang).into());
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
        #[cfg(target_os = "windows")]
        let ui_fraca = ui.as_weak();
        ui.on_trocar_tema(move |escuro| {
            // O visual já mudou sozinho pelo binding da UI com `Tema.escuro`;
            // aqui só guardamos a escolha no config.
            let mut e = estado.borrow_mut();
            e.tema_claro = !escuro;
            salvar_configuracao(&e);

            // Troca de fundo é o gatilho mais documentado do bug de redraw
            // parcial do renderer de software (ver `forcar_repaint_completo`).
            #[cfg(target_os = "windows")]
            if let Some(ui) = ui_fraca.upgrade()
                && let Some(hwnd) = obter_hwnd(&ui).map(HWND)
            {
                forcar_repaint_completo(hwnd);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_trocar_idioma(move |idioma| {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                e.idioma = idioma.to_string();
                let lang = translations::get_translations(&e.idioma);
                ui.set_texto_loop(texto_loop(e.modo_loop, &lang).into());
                ui.set_texto_shuffle(texto_shuffle(e.modo_shuffle, &lang).into());
                let t = idioma.to_string();
                drop(e);
                aplicar_idioma(&t, &ui);
                let e = estado.borrow_mut();
                salvar_configuracao(&e);
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
                match efeito_seek(e.audio_player.is_some(), e.arquivo_atual.is_some()) {
                    // Arrastar a barra com a faixa pausada não pode dar play:
                    // executar_seek recria o Sink já tocando.
                    EfeitoSeek::ManterEstado => seek_mantendo_pausa(&mut e, &ui, alvo),
                    EfeitoSeek::Recarregar => executar_seek(&mut e, &ui, alvo),
                    EfeitoSeek::Nada => {}
                }
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
        #[cfg(feature = "mpris")]
        let controles = controles.clone();
        #[cfg(target_os = "windows")]
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
                #[cfg(feature = "mpris")]
                if controles.borrow().is_none()
                    && let Some(c) = configurar_controles_multimidia(&ui, tx.clone())
                {
                    *controles.borrow_mut() = Some(c);
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

                #[cfg(feature = "mpris")]
                processar_eventos_multimidia(&rx, &mut e, &ui);
                #[cfg(target_os = "windows")]
                processar_eventos_taskbar(&rx_taskbar, &mut e, &ui);
                atualizar_progresso(&mut e, &ui);
                #[cfg(feature = "mpris")]
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
    let update_timer = Timer::default();
    {
        let ui_weak = ui.as_weak();
        let update_result = update_result.clone();
        update_timer.start(TimerMode::Repeated, Duration::from_millis(250), move || {
            let mut guard = update_result.lock().unwrap();
            if let Some(info) = guard.take()
                && let Some(ui) = ui_weak.upgrade()
            {
                ui.set_update_versao(info.version.into());
                ui.set_update_descricao(info.body.into());
                ui.set_update_disponivel(true);
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

// ---------------------------------------------------------------------
// Testes
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::Sample;

    fn fixture(nome: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(nome)
    }

    /// Abre a fixture do mesmo jeito que `executar_seek` abre a faixa real:
    /// lendo o tamanho do arquivo antes de construir o decodificador.
    fn decodificar(nome: &str, extensao: &str) -> Decoder<BufReader<File>> {
        let arquivo = File::open(fixture(nome)).unwrap_or_else(|e| panic!("fixture {nome}: {e}"));
        let byte_len = arquivo
            .metadata()
            .unwrap_or_else(|e| panic!("metadata de {nome}: {e}"))
            .len();
        criar_decodificador(BufReader::new(arquivo), extensao, byte_len)
            .unwrap_or_else(|e| panic!("{nome} deve decodificar: {e:?}"))
    }

    fn amostras_nao_silenciosas(mut decodificador: Decoder<BufReader<File>>) {
        let duracao = decodificador
            .total_duration()
            .expect("duração deve ser conhecida");
        let amostras: Vec<Sample> = decodificador.take(4096).collect();
        assert!(!amostras.is_empty(), "{:?} não devolveu amostras", duracao);
        assert!(
            amostras.iter().any(|s| *s != 0.0),
            "{:?} devolveu só silêncio",
            duracao
        );
    }

    #[test]
    fn m4a_decodifica_com_hint_de_extensao() {
        let decodificador = decodificar("tone.m4a", "m4a");
        let duracao = decodificador
            .total_duration()
            .expect("duração do m4a deve ser conhecida");
        assert!(duracao.as_millis() >= 150, "durou {duracao:?}");

        amostras_nao_silenciosas(decodificador);
    }

    #[test]
    fn m4a_com_moov_no_inicio_tambem_decodifica() {
        let decodificador = decodificar("tone_faststart.m4a", "m4a");
        let duracao = decodificador
            .total_duration()
            .expect("duração do m4a deve ser conhecida");
        assert!(duracao.as_secs() >= 1, "durou {duracao:?}");

        amostras_nao_silenciosas(decodificador);
    }

    #[test]
    fn m4a_permite_seek() {
        let mut decodificador = decodificar("tone.m4a", "m4a");
        decodificador
            .try_seek(Duration::from_millis(100))
            .expect("seek em m4a deve funcionar");
    }

    #[test]
    fn mp3_decodifica_com_hint_de_extensao() {
        let decodificador = decodificar("tone.mp3", "mp3");
        amostras_nao_silenciosas(decodificador);
    }

    #[test]
    fn seek_com_player_preserva_tocando_ou_pausado() {
        assert_eq!(efeito_seek(true, true), EfeitoSeek::ManterEstado);
        assert_eq!(efeito_seek(true, false), EfeitoSeek::ManterEstado);
    }

    #[test]
    fn seek_sem_player_só_recarrega_com_faixa_selecionada() {
        assert_eq!(efeito_seek(false, true), EfeitoSeek::Recarregar);
        assert_eq!(efeito_seek(false, false), EfeitoSeek::Nada);
    }
}
