// Em release não abrimos console junto do app; no debug mantemos, para ver logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

slint::include_modules!();

mod acoustid;
mod audio_ffmpeg;
mod biblioteca;
mod buscador;
mod capa;
mod config;
mod deezer;
mod discord;
mod estado;
mod ffmpeg;
mod fundo;
mod imersivo;
mod instancia;
mod letras_web;
mod listas;
mod lrc;
mod memoria;
mod metadados;
mod miniaturas;
mod musicbrainz;
mod painel;
mod plataforma;
mod ponte_imersivo;
mod rede;
mod reproducao;
mod scrobble;
mod sono;
mod track;
mod transicao;
mod translations;
mod updates;
mod visualizador;
mod web_ui;

#[cfg(test)]
mod captura;

use estado::*;
use listas::*;
use plataforma::*;
use ponte_imersivo::*;
use reproducao::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::rc::Rc;
#[cfg(feature = "mpris")]
use std::sync::mpsc::Sender;
#[cfg(any(feature = "mpris", target_os = "windows"))]
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use web_ui::*;

use biblioteca::{Biblioteca, chave_caminho, chave_faixa};
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
    Ordem, TrackInfo, coletar_arquivos_audio, filtro_efetivo, nome_da_pasta, ordem_filtrada,
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
    i18n.set_tip_immersive(g("tip_immersive"));
    i18n.set_tip_exit_immersive(g("tip_exit_immersive"));
    i18n.set_tip_library(g("tip_library"));
    i18n.set_library_title(g("library_title"));
    i18n.set_library_empty(g("library_empty"));
    i18n.set_up_next(g("up_next"));
    i18n.set_up_next_empty(g("up_next_empty"));
    i18n.set_tip_lyrics_only(g("tip_lyrics_only"));
    i18n.set_tip_video_only(g("tip_video_only"));
    i18n.set_tip_sleep(g("tip_sleep"));
    i18n.set_tip_info(g("tip_info"));
    i18n.set_info_tab_track(g("info_tab_track"));
    i18n.set_info_tab_batch(g("info_tab_batch"));
    i18n.set_info_corrected(g("info_corrected"));
    i18n.set_info_restore(g("info_restore"));
    i18n.set_info_lyrics_label(g("info_lyrics_label"));
    i18n.set_info_get_lyrics(g("info_get_lyrics"));
    i18n.set_info_search_web(g("info_search_web"));
    i18n.set_info_apply(g("info_apply"));
    i18n.set_info_apply_tags(g("info_apply_tags"));
    i18n.set_info_confirm_tags(g("info_confirm_tags"));
    i18n.set_info_confirm_button(g("info_confirm_button"));
    i18n.set_batch_desc(g("batch_desc"));
    i18n.set_batch_start(g("batch_start"));
    i18n.set_batch_cancel(g("batch_cancel"));
    i18n.set_batch_review(g("batch_review"));
    i18n.set_batch_accept_all(g("batch_accept_all"));
    i18n.set_tags_write_tab(g("tags_write_tab"));
    i18n.set_lyrics_web(g("lyrics_web"));
    i18n.set_lyrics_web_desc(g("lyrics_web_desc"));
    i18n.set_acoustid_key(g("acoustid_key"));
    i18n.set_acoustid_desc(g("acoustid_desc"));
    i18n.set_discord_status(g("discord_status"));
    i18n.set_tip_mini(g("tip_mini"));
    i18n.set_tip_mini_exit(g("tip_mini_exit"));
    i18n.set_scrobble_title(g("scrobble_title"));
    i18n.set_listenbrainz_token(g("listenbrainz_token"));
    i18n.set_lastfm_keys(g("lastfm_keys"));
    i18n.set_lastfm_key(g("lastfm_key"));
    i18n.set_lastfm_secret(g("lastfm_secret"));
    i18n.set_lastfm_connect(g("lastfm_connect"));
    i18n.set_lastfm_finish(g("lastfm_finish"));
    i18n.set_lastfm_disconnect(g("lastfm_disconnect"));
    i18n.set_scrobble_desc(g("scrobble_desc"));
    i18n.set_tip_sort(g("tip_sort"));
    i18n.set_key_space(g("key_space"));
    i18n.set_tip_favorite(g("tip_favorite"));
    i18n.set_sort_default(g("sort_default"));
    i18n.set_sort_title(g("sort_title"));
    i18n.set_sort_artist(g("sort_artist"));
    i18n.set_sort_album(g("sort_album"));
    i18n.set_sort_duration(g("sort_duration"));
    i18n.set_sort_added(g("sort_added"));
    i18n.set_sort_plays(g("sort_plays"));
    i18n.set_albums(g("albums"));
    i18n.set_albums_empty(g("albums_empty"));
    i18n.set_up_next_preview(g("up_next_preview"));
    i18n.set_replaygain(g("replaygain"));
    i18n.set_replaygain_desc(g("replaygain_desc"));
    i18n.set_reactive_bg(g("reactive_bg"));
    i18n.set_favorites_empty(g("favorites_empty"));
    i18n.set_most_played_empty(g("most_played_empty"));
    i18n.set_bg_label(g("bg_label"));
    i18n.set_bg_choose(g("bg_choose"));
    i18n.set_bg_clear(g("bg_clear"));
    i18n.set_bg_none(g("bg_none"));
    #[cfg(target_os = "windows")]
    i18n.set_ffmpeg_missing(g("ffmpeg_missing_windows"));
    #[cfg(not(target_os = "windows"))]
    i18n.set_ffmpeg_missing(g("ffmpeg_missing"));
    i18n.set_ffmpeg_get(g("ffmpeg_get"));
    i18n.set_search_placeholder(g("search_placeholder"));
    i18n.set_no_lyrics(g("no_lyrics"));
    i18n.set_config_title(g("config_title"));
    i18n.set_theme(g("theme"));
    i18n.set_theme_dark(g("theme_dark"));
    i18n.set_theme_light(g("theme_light"));
    i18n.set_scan_subfolders(g("scan_subfolders"));
    i18n.set_scan_subfolders_desc(g("scan_subfolders_desc"));
    i18n.set_update_title(g("update_title"));
    i18n.set_update_downloading(g("update_downloading"));
    i18n.set_update_button(g("update_button"));
    i18n.set_update_later(g("update_later"));
    i18n.set_language(g("language"));
    i18n.set_update_ready_title(g("update_ready_title"));
    i18n.set_update_ready(g("update_ready"));
    i18n.set_update_restart(g("update_restart"));
    i18n.set_update_failed(g("update_failed"));
    i18n.set_update_retry(g("update_retry"));
    i18n.set_update_manual(g("update_manual"));
    i18n.set_update_open_page(g("update_open_page"));
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

// ---------------------------------------------------------------------
// Estado de áudio/playlist
// ---------------------------------------------------------------------

// ---------------------------------------------------------------------
// Letras sincronizadas (.lrc)
// ---------------------------------------------------------------------

// ---------------------------------------------------------------------
// Modo imersivo
// ---------------------------------------------------------------------

// ---------------------------------------------------------------------
// Busca na web (letras, metadados, capas)
// ---------------------------------------------------------------------

// ---------------------------------------------------------------------
// Controles multimídia do sistema (SMTC no Windows)
// ---------------------------------------------------------------------

// ---------------------------------------------------------------------
// Botões na miniatura da barra de tarefas (ITaskbarList3)
// ---------------------------------------------------------------------

// ---------------------------------------------------------------------
// Stubs para Linux/Outros (funções da taskbar)
// ---------------------------------------------------------------------

// ---------------------------------------------------------------------
// main
// ---------------------------------------------------------------------

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // A glibc cria uma arena de malloc por thread (áudio, D-Bus, update...), e
    // a memória livre de cada uma não volta para o SO. Duas bastam aqui.
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    // SAFETY: só ajusta um parâmetro do alocador, antes de qualquer thread
    unsafe {
        libc::mallopt(libc::M_ARENA_MAX, 2);
    }

    // "Abrir com" com o Furinar já aberto: os arquivos vão para ele
    let externos = instancia::caminhos_dos_argumentos();
    if !externos.is_empty() && instancia::enviar(&externos) {
        return Ok(());
    }

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
        // Liga a janela ao furinar.desktop: sem isso o GNOME/KDE no Wayland
        // mostra ícone genérico e "Desconhecido" no Alt+Tab e na dock
        slint::set_xdg_app_id("furinar")?;
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
        e.fundos_globais = config.fundos_tela_cheia.iter().map(PathBuf::from).collect();
        ui.set_fundo_global_nome(nome_fundo(&e.fundos_globais, &config.idioma).into());
        e.biblioteca = Biblioteca::carregar();
        e.metadados = metadados::Banco::carregar();
        e.letras_web = config.letras_web;
        e.chave_acoustid = config.chave_acoustid.clone();
        e.discord_ativo = config.discord;
        e.contas = scrobble::Contas {
            listenbrainz: config.listenbrainz_token.clone(),
            lastfm_chave: config.lastfm_chave.clone(),
            lastfm_segredo: config.lastfm_segredo.clone(),
            lastfm_sessao: config.lastfm_sessao.clone(),
        };
        e.lastfm_usuario = config.lastfm_usuario.clone();
        e.fila_scrobble = scrobble::carregar_fila();
        ui.set_listenbrainz_token(config.listenbrainz_token.as_str().into());
        ui.set_lastfm_chave(config.lastfm_chave.as_str().into());
        ui.set_lastfm_segredo(config.lastfm_segredo.as_str().into());
        publicar_lastfm(&e, &ui);
        // O que ficou de uma sessão sem rede vai agora
        enviar_fila_scrobble(&mut e);
        ui.set_discord(config.discord);
        ui.set_letras_web(config.letras_web);
        ui.set_chave_acoustid(config.chave_acoustid.as_str().into());
        e.crossfade = config.crossfade;
        e.replaygain = config.replaygain;
        e.fundo_reativo = config.fundo_reativo;
        ui.set_replaygain(config.replaygain);
        ui.set_fundo_reativo(config.fundo_reativo);

        ui.set_volume(config.volume);
        let lang = translations::get_translations(&config.idioma);
        ui.set_texto_loop(texto_loop(config.modo_loop, &lang).into());
        ui.set_loop_modo(config.modo_loop as i32);
        ui.set_texto_shuffle(texto_shuffle(config.modo_shuffle, &lang).into());
        ui.set_shuffle_modo(config.modo_shuffle as i32);
        ui.set_escanear_subpastas(config.escanear_subpastas);

        // Aplica o tema salvo antes da primeira renderização, para não piscar
        e.tema_claro = config.tema_claro;
        ui.set_tema_escuro(!config.tema_claro);

        // Aplica todas as traduções antes da primeira renderização
        aplicar_idioma(&config.idioma, &ui);
        publicar_crossfade(&e, &ui);

        // Listas especiais primeiro (POS_FAVORITAS, POS_MAIS_TOCADAS)
        let ordem_salva = |chave: &str, padrao| {
            config
                .ordens
                .get(chave)
                .map_or(padrao, |&o| Ordem::de_indice(o as usize))
        };
        e.pastas.push(PastaMusical::especial(
            Tipo::Favoritas,
            ordem_salva(CHAVE_FAVORITAS, Ordem::Padrao),
        ));
        e.pastas.push(PastaMusical::especial(
            Tipo::MaisTocadas,
            ordem_salva(CHAVE_MAIS_TOCADAS, Ordem::Padrao),
        ));

        // Carrega as pastas salvas. Se alguma não existir mais, é pulada, e o
        // mapeamento mantém os índices salvos (de configs antigas) coerentes
        // com a nova lista.
        let mut mapeamento: Vec<Option<usize>> = Vec::with_capacity(config.pastas.len());
        for caminho_salvo in &config.pastas {
            let caminho = PathBuf::from(caminho_salvo);
            if !caminho.is_dir() {
                mapeamento.push(None);
                continue;
            }
            // Sem escanear: as faixas entram sob demanda
            let mut pasta = PastaMusical::pasta(caminho);
            pasta.ordem = ordem_salva(caminho_salvo, Ordem::Padrao);
            e.pastas.push(pasta);
            mapeamento.push(Some(e.pastas.len() - 1));
        }

        let pela_chave = |chave: &Option<String>| {
            let chave = chave.as_deref()?;
            e.pastas.iter().position(|p| p.chave() == chave)
        };
        let legado = |i: Option<usize>| i.and_then(|i| mapeamento.get(i).copied().flatten());
        let primeira_pasta = if e.pastas.len() > ESPECIAIS {
            ESPECIAIS
        } else {
            0
        };
        let aba = pela_chave(&config.aba_salva)
            .or_else(|| legado(Some(config.aba_visivel_salva)))
            .unwrap_or(primeira_pasta);
        let reproducao =
            pela_chave(&config.lista_salva).or_else(|| legado(config.pasta_reproducao_salva));
        e.aba_visivel = aba;
        e.pasta_reproducao = reproducao;

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

    // Callbacks de cada área
    listas::ligar(&ui, &estado);
    reproducao::ligar(&ui, &estado);
    ponte_imersivo::ligar(&ui, &estado);
    web_ui::ligar(&ui, &estado);
    plataforma::ligar(&ui, &estado);

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
                publicar_crossfade(&e, &ui);
                atualizar_abas(&e, &ui);
                ui.set_fundo_global_nome(nome_fundo(&e.fundos_globais, &e.idioma).into());
                let t = idioma.to_string();
                drop(e);
                aplicar_idioma(&t, &ui);
                let e = estado.borrow_mut();
                salvar_configuracao(&e);
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
                verificar_sono(&mut e, &ui);
                processar_web(&mut e, &ui);
                atualizar_discord(&mut e);
                atualizar_scrobble(&mut e);
                atualizar_mini(&e, &ui);
                if e.imersivo.is_some() {
                    atualizar_relogio(&ui);
                    atualizar_previa(&mut e, &ui);
                }
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
                // Minimizada não pinta nada; o que ela precisar volta sob demanda
                memoria::aparar();
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

    // Atualização: a checagem roda numa thread e entrega o resultado no
    // event loop. `update_estado`: 0 nada, 1 disponível, 2 baixando,
    // 3 pronta para reiniciar, 4 erro, 5 manual (pasta sem escrita).
    {
        let pendente: Arc<Mutex<Option<updates::Atualizacao>>> = Arc::default();
        let pronta: Arc<Mutex<Option<updates::Baixado>>> = Arc::default();

        {
            let ui_fraca = ui.as_weak();
            let pendente = pendente.clone();
            std::thread::spawn(move || {
                let atualizacao = updates::verificar();
                // A checagem desserializa a lista inteira de releases e sobe
                // um runtime HTTP: código e heap que não voltam a ser usados.
                memoria::aparar();
                if let Some(at) = atualizacao {
                    let (versao, notas) = (at.versao.clone(), at.notas.clone());
                    let manual = at.modo == updates::Modo::Manual;
                    *pendente.lock().unwrap() = Some(at);
                    let _ = ui_fraca.upgrade_in_event_loop(move |ui| {
                        ui.set_update_versao(versao.into());
                        ui.set_update_notas(notas.into());
                        ui.set_update_estado(if manual { 5 } else { 1 });
                    });
                }
            });
        }

        {
            let ui_fraca = ui.as_weak();
            let pendente = pendente.clone();
            let pronta = pronta.clone();
            ui.on_baixar_atualizacao(move || {
                let Some(ui) = ui_fraca.upgrade() else { return };
                let Some(at) = pendente.lock().unwrap().clone() else {
                    return;
                };
                ui.set_update_progresso(0.0);
                ui.set_update_estado(2);
                let ui_fraca = ui.as_weak();
                let pronta = pronta.clone();
                std::thread::spawn(move || {
                    let progresso_ui = ui_fraca.clone();
                    let resultado = updates::baixar(&at, &move |p| {
                        let _ = progresso_ui.upgrade_in_event_loop(move |ui| {
                            ui.set_update_progresso(p);
                        });
                    });
                    let resultado = resultado.map(|b| *pronta.lock().unwrap() = Some(b));
                    let _ = ui_fraca.upgrade_in_event_loop(move |ui| match resultado {
                        Ok(()) => ui.set_update_estado(3),
                        Err(e) => {
                            eprintln!("Falha na atualização: {e}");
                            ui.set_update_erro(e.into());
                            ui.set_update_estado(4);
                        }
                    });
                });
            });
        }

        {
            let ui_fraca = ui.as_weak();
            let estado = estado.clone();
            ui.on_reiniciar_atualizacao(move || {
                let Some(ui) = ui_fraca.upgrade() else { return };
                let Some(baixado) = pronta.lock().unwrap().take() else {
                    return;
                };
                // Salva antes de abrir a nova: ela lê a configuração ao iniciar
                salvar_configuracao(&estado.borrow());
                match updates::reiniciar(&baixado) {
                    Ok(()) => {
                        let _ = ui.hide();
                    }
                    Err(e) => {
                        ui.set_update_erro(e.into());
                        ui.set_update_estado(4);
                    }
                }
            });
        }

        ui.on_abrir_pagina_atualizacao(move || {
            if let Some(at) = pendente.lock().unwrap().as_ref() {
                abrir_no_navegador(&at.pagina);
            }
        });
    }

    // Passado o startup (restauração da faixa, primeiro paint), boa parte do
    // código e das fontes mapeados não é mais tocada.
    Timer::single_shot(Duration::from_secs(5), memoria::aparar);

    // Arquivos das próximas execuções ("Abrir com" de novo) chegam por aqui
    {
        let ui_fraca = ui.as_weak();
        instancia::escutar(move |caminhos| {
            let _ = ui_fraca.upgrade_in_event_loop(move |ui| {
                ui.invoke_abrir_externos(juntar_linhas(&caminhos))
            });
        });
    }
    if !externos.is_empty() {
        let ui_fraca = ui.as_weak();
        Timer::single_shot(Duration::ZERO, move || {
            if let Some(ui) = ui_fraca.upgrade() {
                ui.invoke_abrir_externos(juntar_linhas(&externos));
            }
        });
    }

    // `furinar --imersivo` já abre em tela cheia (atalho de desktop, kiosk)
    if std::env::args().any(|a| a == "--imersivo") {
        let ui_fraca = ui.as_weak();
        Timer::single_shot(Duration::ZERO, move || {
            if let Some(ui) = ui_fraca.upgrade() {
                ui.invoke_alternar_imersivo();
            }
        });
    }

    ui.run()?;

    // Salva novamente ao encerrar (fallback)
    salvar_configuracao(&estado.borrow());

    Ok(())
}

/// Caminhos um por linha, para passar pelo callback da UI.
fn juntar_linhas(caminhos: &[PathBuf]) -> SharedString {
    caminhos
        .iter()
        .map(|p| p.to_string_lossy())
        .collect::<Vec<_>>()
        .join("\n")
        .into()
}

// ---------------------------------------------------------------------
// Testes
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// O fallback de `aplicar_idioma` é a própria chave: uma chave que não
    /// existe nas traduções aparece crua na UI. Lê o próprio fonte para pegar
    /// toda chave pedida via `g("...")`.
    #[test]
    fn aplicar_idioma_so_usa_chaves_existentes() {
        // No Windows o checkout do git pode trazer o fonte com CRLF
        let fonte = include_str!("main.rs").replace("\r\n", "\n");
        let inicio = fonte
            .find("fn aplicar_idioma(")
            .expect("aplicar_idioma deve existir");
        let corpo = &fonte[inicio..];
        let corpo = &corpo[..corpo.find("\n}\n").expect("fim de aplicar_idioma")];

        let chaves: Vec<&str> = corpo
            .split("g(\"")
            .skip(1)
            .filter_map(|resto| resto.split('"').next())
            .collect();
        assert!(!chaves.is_empty(), "nenhuma chave encontrada");

        for (idioma, t) in [("pt-br", translations::pt_br()), ("en", translations::en())] {
            for chave in &chaves {
                assert!(t.contains_key(chave), "{idioma}: chave {chave} ausente");
            }
        }
    }
}
