// #![windows_subsystem = "windows"]

use native_windows_derive as nwd;
use native_windows_gui as nwg;
use nwd::NwgUi;
use nwg::NativeUi;

use rand::seq::SliceRandom;
use rodio::{Decoder, OutputStream, Sink, Source};
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::BufReader;
use std::mem;
use std::path::{Path, PathBuf};
use std::ptr;
use std::time::Duration;

use winapi::ctypes::c_void;
use winapi::shared::basetsd::LONG_PTR;
use winapi::shared::windef::{HBRUSH, HDC, HWND, RECT};
use winapi::um::dwmapi::DwmSetWindowAttribute;
use winapi::um::uxtheme::SetWindowTheme;
use winapi::um::wingdi::{
    CreatePen, CreateSolidBrush, DeleteObject, PS_SOLID, RoundRect, SelectObject, SetBkMode,
    SetTextColor, TRANSPARENT,
};
use winapi::um::winuser::{
    self, BS_OWNERDRAW, DRAWITEMSTRUCT, DT_CENTER, DT_SINGLELINE, DT_VCENTER, DrawTextW, FillRect,
    GWL_STYLE, GetClientRect, GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW,
    InvalidateRect, ODS_SELECTED, ODT_BUTTON, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos, TME_LEAVE, TRACKMOUSEEVENT,
    TrackMouseEvent, UpdateWindow,
};

// Atributos do DWM (não existem em todas as versões da crate winapi, então definimos manualmente)
const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
const DWMWCP_ROUND: i32 = 2;

/// Cor de um botão "flat" nos estados normal e hover (RGB 0-255)
#[derive(Clone, Copy)]
struct CorBotao {
    normal: (u8, u8, u8),
    hover: (u8, u8, u8),
}

fn cor_rgb(r: u8, g: u8, b: u8) -> u32 {
    r as u32 | ((g as u32) << 8) | ((b as u32) << 16)
}

fn obter_texto_janela(hwnd: HWND) -> String {
    unsafe {
        let comprimento = GetWindowTextLengthW(hwnd);
        if comprimento <= 0 {
            return String::new();
        }
        let mut buffer: Vec<u16> = vec![0u16; (comprimento + 1) as usize];
        GetWindowTextW(hwnd, buffer.as_mut_ptr(), comprimento + 1);
        String::from_utf16_lossy(&buffer[..comprimento as usize])
    }
}

#[derive(Serialize, Deserialize, Default)]
struct AppConfig {
    pasta: Option<String>,
    volume: f32,
    modo_loop: u8, // 0: Desligado, 1: Faixa, 2: Playlist
    shuffle: bool,
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

fn e_arquivo_audio(caminho: &Path) -> bool {
    if let Some(ext) = caminho.extension().and_then(|e| e.to_str()) {
        matches!(
            ext.to_lowercase().as_str(),
            "mp3" | "wav" | "flac" | "ogg" | "m4a"
        )
    } else {
        false
    }
}

#[derive(Default, NwgUi)]
pub struct FurinarApp {
    #[nwg_control(
        size: (580, 640),
        position: (300, 200),
        title: "Furinar Audio Player",
        flags: "WINDOW|VISIBLE"
    )]
    #[nwg_events(
        OnWindowClose: [FurinarApp::ao_fechar],
        OnInit: [FurinarApp::setup_inicial]
    )]
    window: nwg::Window,

    #[nwg_resource(family: "Segoe UI", size: 20)]
    fonte_botoes: nwg::Font,

    #[nwg_resource(family: "Segoe UI", size: 16)]
    fonte_ui: nwg::Font,

    #[nwg_control(text: "📁", size: (48, 44), position: (16, 16), font: Some(&data.fonte_botoes))]
    #[nwg_events( OnButtonClick: [FurinarApp::abrir_pasta_dialogo] )]
    btn_pasta: nwg::Button,

    #[nwg_control(text: "⏮", size: (44, 44), position: (72, 16), font: Some(&data.fonte_botoes))]
    #[nwg_events( OnButtonClick: [FurinarApp::tocar_anterior] )]
    btn_anterior: nwg::Button,

    #[nwg_control(text: "⏸", size: (92, 44), position: (124, 16), font: Some(&data.fonte_botoes))]
    #[nwg_events( OnButtonClick: [FurinarApp::alternar_pausa] )]
    btn_pause: nwg::Button,

    #[nwg_control(text: "⏹", size: (44, 44), position: (224, 16), font: Some(&data.fonte_botoes))]
    #[nwg_events( OnButtonClick: [FurinarApp::stop_music] )]
    stop_button: nwg::Button,

    #[nwg_control(text: "⏭", size: (44, 44), position: (276, 16), font: Some(&data.fonte_botoes))]
    #[nwg_events( OnButtonClick: [FurinarApp::tocar_proxima_manual] )]
    btn_proxima: nwg::Button,

    #[nwg_control(text: "🔁 Desl", size: (84, 44), position: (328, 16), font: Some(&data.fonte_ui))]
    #[nwg_events( OnButtonClick: [FurinarApp::alternar_loop] )]
    btn_loop: nwg::Button,

    #[nwg_control(text: "🔀 Desl", size: (84, 44), position: (420, 16), font: Some(&data.fonte_ui))]
    #[nwg_events( OnButtonClick: [FurinarApp::alternar_shuffle] )]
    btn_shuffle: nwg::Button,

    #[nwg_control(range: Some(0..100), pos: Some(80), size: (96, 44), position: (16, 68))]
    #[nwg_events( OnHorizontalScroll: [FurinarApp::alterar_volume] )]
    slider_volume: nwg::TrackBar,

    #[nwg_control(range: Some(0..100), pos: Some(0), size: (330, 28), position: (128, 76))]
    #[nwg_events( OnHorizontalScroll: [FurinarApp::registrar_busca] )]
    slider_progresso: nwg::TrackBar,

    #[nwg_control(text: "00:00 / 00:00", size: (110, 24), position: (468, 80), font: Some(&data.fonte_ui))]
    label_tempo: nwg::Label,

    #[nwg_control(interval: Duration::from_millis(250))]
    #[nwg_events( OnTimerTick: [FurinarApp::atualizar_progresso] )]
    timer: nwg::AnimationTimer,

    #[nwg_resource(action: nwg::FileDialogAction::OpenDirectory, title: "Selecione a pasta de músicas")]
    dialogo_pasta: nwg::FileDialog,

    #[nwg_control(size: (548, 500), position: (16, 120), font: Some(&data.fonte_ui))]
    #[nwg_events( OnListBoxDoubleClick: [FurinarApp::tocar_selecionada] )]
    playlist: nwg::ListBox<String>,

    #[nwg_resource(source_system: Some(nwg::OemIcon::Information))]
    tray_icon: nwg::Icon,

    #[nwg_control(icon: Some(&data.tray_icon), tip: Some("Furinar Audio Player"))]
    #[nwg_events(OnMousePress: [FurinarApp::alternar_visibilidade_janela])]
    tray: nwg::TrayNotification,

    // Componentes de áudio usando os tipos reais do rodio
    audio_player: RefCell<Option<(OutputStream, Sink)>>,
    tempo_decorrido: RefCell<f64>, // Controlador de tempo manual

    pasta_atual: RefCell<Option<PathBuf>>,
    arquivo_atual: RefCell<Option<String>>,
    indice_atual: RefCell<Option<usize>>,
    volume_atual: RefCell<f32>,
    duracao_total_secs: RefCell<u64>,
    ultimo_segundo: RefCell<u64>,
    bloqueio_evento: RefCell<bool>,
    pending_seek: RefCell<Option<u64>>,
    modo_loop: RefCell<u8>,
    modo_shuffle: RefCell<bool>,

    // --- Estado do visual "flat moderno" ---
    raw_handlers: RefCell<Vec<nwg::RawEventHandler>>,
    cores_botoes: RefCell<HashMap<isize, CorBotao>>,
    hover_state: RefCell<HashMap<isize, bool>>,
    pincel_fundo_janela: Cell<HBRUSH>,
    pincel_fundo_lista: Cell<HBRUSH>,
}

impl FurinarApp {
    fn setup_inicial(&self) {
        let config = AppConfig::carregar();

        self.tray.set_visibility(true);

        self.configurar_tema_janela();
        self.configurar_todos_botoes_flat();

        let vol_pos = (config.volume * 100.0) as usize;
        self.slider_volume.set_pos(vol_pos);
        *self.volume_atual.borrow_mut() = config.volume;
        *self.tempo_decorrido.borrow_mut() = 0.0;

        *self.modo_loop.borrow_mut() = config.modo_loop;
        self.atualizar_texto_loop();

        *self.modo_shuffle.borrow_mut() = config.shuffle;
        self.atualizar_texto_shuffle();

        if let Some(caminho_str) = config.pasta {
            let caminho = PathBuf::from(caminho_str);
            if caminho.exists() {
                self.carregar_pasta(caminho);
            }
        }

        self.timer.start();
    }

    // ---------------------------------------------------------------
    // Visual "flat moderno": tema escuro na janela + botões arredondados
    // ---------------------------------------------------------------

    /// Aplica modo escuro à barra de título/cantos da janela (Win10 1809+/Win11)
    /// e tema escuro aos controles nativos (lista, sliders). Também registra
    /// um handler bruto na janela para pintar o fundo escuro e colorir o
    /// texto dos labels/lista.
    fn configurar_tema_janela(&self) {
        unsafe {
            if let Some(hwnd) = self.window.handle.hwnd() {
                let habilitado: i32 = 1;
                DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_USE_IMMERSIVE_DARK_MODE,
                    &habilitado as *const _ as *const c_void,
                    mem::size_of::<i32>() as u32,
                );

                let preferencia: i32 = DWMWCP_ROUND;
                DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_WINDOW_CORNER_PREFERENCE,
                    &preferencia as *const _ as *const c_void,
                    mem::size_of::<i32>() as u32,
                );

                // Sem isso, o Windows não redesenha a barra de título com o
                // novo atributo — ela ficaria clara até a janela ser movida.
                SetWindowPos(
                    hwnd,
                    ptr::null_mut(),
                    0,
                    0,
                    0,
                    0,
                    SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }

            let nome_tema: Vec<u16> = "DarkMode_Explorer\0".encode_utf16().collect();
            if let Some(h) = self.playlist.handle.hwnd() {
                SetWindowTheme(h, nome_tema.as_ptr(), ptr::null());
            }
            if let Some(h) = self.slider_volume.handle.hwnd() {
                SetWindowTheme(h, nome_tema.as_ptr(), ptr::null());
            }
            if let Some(h) = self.slider_progresso.handle.hwnd() {
                SetWindowTheme(h, nome_tema.as_ptr(), ptr::null());
            }
        }

        unsafe {
            self.pincel_fundo_janela
                .set(CreateSolidBrush(cor_rgb(22, 22, 26)));
            self.pincel_fundo_lista
                .set(CreateSolidBrush(cor_rgb(32, 32, 38)));
        }

        let app_ptr = self as *const FurinarApp as usize;

        let resultado =
            nwg::bind_raw_event_handler(&self.window.handle, 0x1_0010, move |h, msg, w, l| {
                let app = unsafe { &*(app_ptr as *const FurinarApp) };

                match msg {
                    winuser::WM_DRAWITEM => unsafe {
                        let dis = &*(l as *const DRAWITEMSTRUCT);
                        if dis.CtlType == ODT_BUTTON {
                            app.desenhar_botao_flat(dis);
                            return Some(1isize);
                        }
                        None
                    },
                    winuser::WM_ERASEBKGND => unsafe {
                        let hdc = w as HDC;
                        let mut rc: RECT = mem::zeroed();
                        GetClientRect(h, &mut rc);
                        FillRect(hdc, &rc, app.pincel_fundo_janela.get());
                        Some(1isize)
                    },
                    winuser::WM_CTLCOLORSTATIC => unsafe {
                        let hdc = w as HDC;
                        SetTextColor(hdc, cor_rgb(232, 232, 238));
                        SetBkMode(hdc, TRANSPARENT as i32);
                        Some(app.pincel_fundo_janela.get() as isize)
                    },
                    winuser::WM_CTLCOLORLISTBOX => unsafe {
                        let hdc = w as HDC;
                        SetTextColor(hdc, cor_rgb(232, 232, 238));
                        SetBkMode(hdc, TRANSPARENT as i32);
                        Some(app.pincel_fundo_lista.get() as isize)
                    },
                    _ => None,
                }
            });

        if let Ok(handler) = resultado {
            self.raw_handlers.borrow_mut().push(handler);
        }

        // A janela já foi pintada com o fundo padrão antes de chegarmos
        // aqui (ela é criada com a flag VISIBLE). Sem isso, o WM_ERASEBKGND
        // escuro só apareceria na próxima vez que algo invalidasse a janela.
        if let Some(hwnd) = self.window.handle.hwnd() {
            unsafe {
                InvalidateRect(hwnd, ptr::null(), 1);
                UpdateWindow(hwnd);
            }
        }
    }

    /// Converte cada botão em um botão "owner-draw" e registra o rastreio de hover.
    fn configurar_todos_botoes_flat(&self) {
        let neutro = CorBotao {
            normal: (46, 46, 54),
            hover: (64, 64, 76),
        };
        let destaque = CorBotao {
            normal: (92, 98, 235),
            hover: (112, 118, 250),
        };

        self.configurar_botao_flat(&self.btn_pasta.handle, neutro, 0x1_0001);
        self.configurar_botao_flat(&self.btn_anterior.handle, neutro, 0x1_0002);
        self.configurar_botao_flat(&self.btn_pause.handle, destaque, 0x1_0003);
        self.configurar_botao_flat(&self.stop_button.handle, neutro, 0x1_0004);
        self.configurar_botao_flat(&self.btn_proxima.handle, neutro, 0x1_0005);
        self.configurar_botao_flat(&self.btn_loop.handle, neutro, 0x1_0006);
        self.configurar_botao_flat(&self.btn_shuffle.handle, neutro, 0x1_0007);
    }

    fn configurar_botao_flat(&self, handle: &nwg::ControlHandle, cor: CorBotao, handler_id: usize) {
        let hwnd = match handle.hwnd() {
            Some(h) => h,
            None => return,
        };

        unsafe {
            let estilo = GetWindowLongPtrW(hwnd, GWL_STYLE);
            SetWindowLongPtrW(hwnd, GWL_STYLE, estilo | BS_OWNERDRAW as LONG_PTR);

            // Mudar o estilo sozinho não faz o Windows repintar o botão -
            // ele continua com a aparência antiga até isto ser forçado.
            SetWindowPos(
                hwnd,
                ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
            InvalidateRect(hwnd, ptr::null(), 1);
        }

        self.cores_botoes.borrow_mut().insert(hwnd as isize, cor);
        self.hover_state.borrow_mut().insert(hwnd as isize, false);

        let app_ptr = self as *const FurinarApp as usize;

        let resultado = nwg::bind_raw_event_handler(handle, handler_id, move |h, msg, _w, _l| {
            let app = unsafe { &*(app_ptr as *const FurinarApp) };
            let chave = h as isize;

            match msg {
                winuser::WM_MOUSEMOVE => {
                    let ja_em_hover = *app.hover_state.borrow().get(&chave).unwrap_or(&false);
                    if !ja_em_hover {
                        app.hover_state.borrow_mut().insert(chave, true);
                        unsafe {
                            let mut tme = TRACKMOUSEEVENT {
                                cbSize: mem::size_of::<TRACKMOUSEEVENT>() as u32,
                                dwFlags: TME_LEAVE,
                                hwndTrack: h,
                                dwHoverTime: 0,
                            };
                            TrackMouseEvent(&mut tme);
                            InvalidateRect(h, ptr::null(), 1);
                        }
                    }
                    None
                }
                winuser::WM_MOUSELEAVE => {
                    app.hover_state.borrow_mut().insert(chave, false);
                    unsafe {
                        InvalidateRect(h, ptr::null(), 1);
                    }
                    None
                }
                _ => None,
            }
        });

        if let Ok(handler) = resultado {
            self.raw_handlers.borrow_mut().push(handler);
        }
    }

    /// Desenha o botão como um retângulo arredondado, respeitando estado
    /// normal / hover / pressionado. Chamado a partir do WM_DRAWITEM da janela.
    fn desenhar_botao_flat(&self, dis: &DRAWITEMSTRUCT) {
        unsafe {
            let hwnd = dis.hwndItem;
            let hdc = dis.hDC;
            let rc = dis.rcItem;
            let chave = hwnd as isize;

            let cor = self
                .cores_botoes
                .borrow()
                .get(&chave)
                .copied()
                .unwrap_or(CorBotao {
                    normal: (46, 46, 54),
                    hover: (64, 64, 76),
                });

            let em_hover = *self.hover_state.borrow().get(&chave).unwrap_or(&false);
            let pressionado = dis.itemState & ODS_SELECTED != 0;

            let (r, g, b) = if pressionado {
                let (hr, hg, hb) = cor.hover;
                (
                    hr.saturating_sub(18),
                    hg.saturating_sub(18),
                    hb.saturating_sub(18),
                )
            } else if em_hover {
                cor.hover
            } else {
                cor.normal
            };

            let pincel = CreateSolidBrush(cor_rgb(r, g, b));
            let caneta = CreatePen(
                PS_SOLID as i32,
                1,
                cor_rgb(
                    r.saturating_sub(12),
                    g.saturating_sub(12),
                    b.saturating_sub(12),
                ),
            );

            let pincel_antigo = SelectObject(hdc, pincel as _);
            let caneta_antiga = SelectObject(hdc, caneta as _);

            RoundRect(hdc, rc.left, rc.top, rc.right, rc.bottom, 12, 12);

            SelectObject(hdc, pincel_antigo);
            SelectObject(hdc, caneta_antiga);
            DeleteObject(pincel as _);
            DeleteObject(caneta as _);

            SetBkMode(hdc, TRANSPARENT as i32);
            SetTextColor(hdc, cor_rgb(238, 238, 242));

            let texto = obter_texto_janela(hwnd);
            let mut texto_utf16: Vec<u16> = texto.encode_utf16().collect();
            texto_utf16.push(0);

            let mut rc_texto = rc;
            DrawTextW(
                hdc,
                texto_utf16.as_ptr(),
                -1,
                &mut rc_texto,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
        }
    }

    // ---------------------------------------------------------------
    // Lógica original do player (inalterada)
    // ---------------------------------------------------------------

    fn salvar_configuracao(&self) {
        let config = AppConfig {
            pasta: self
                .pasta_atual
                .borrow()
                .as_ref()
                .map(|p| p.to_string_lossy().to_string()),
            volume: *self.volume_atual.borrow(),
            modo_loop: *self.modo_loop.borrow(),
            shuffle: *self.modo_shuffle.borrow(),
        };
        config.salvar();
    }

    fn ao_fechar(&self) {
        self.salvar_configuracao();
        nwg::stop_thread_dispatch();
    }

    fn alternar_visibilidade_janela(&self) {
        let visivel = self.window.visible();
        self.window.set_visible(!visivel);
    }

    fn alternar_loop(&self) {
        let novo_modo = (*self.modo_loop.borrow() + 1) % 3;
        *self.modo_loop.borrow_mut() = novo_modo;
        self.atualizar_texto_loop();
        self.salvar_configuracao();
    }

    fn atualizar_texto_loop(&self) {
        match *self.modo_loop.borrow() {
            1 => self.btn_loop.set_text("🔁 Faixa"),
            2 => self.btn_loop.set_text("🔁 Toda"),
            _ => self.btn_loop.set_text("🔁 Desl"),
        }
        if let Some(h) = self.btn_loop.handle.hwnd() {
            unsafe { InvalidateRect(h, ptr::null(), 1) };
        }
    }

    fn alternar_shuffle(&self) {
        let novo = !*self.modo_shuffle.borrow();
        *self.modo_shuffle.borrow_mut() = novo;
        self.atualizar_texto_shuffle();
        self.salvar_configuracao();
    }

    fn atualizar_texto_shuffle(&self) {
        if *self.modo_shuffle.borrow() {
            self.btn_shuffle.set_text("🔀 Lig");
        } else {
            self.btn_shuffle.set_text("🔀 Desl");
        }
        if let Some(h) = self.btn_shuffle.handle.hwnd() {
            unsafe { InvalidateRect(h, ptr::null(), 1) };
        }
    }

    fn alterar_volume(&self) {
        let novo_volume = self.slider_volume.pos() as f32 / 100.0;
        *self.volume_atual.borrow_mut() = novo_volume;

        if let Some((_, ref sink)) = *self.audio_player.borrow() {
            sink.set_volume(novo_volume);
        }
        self.salvar_configuracao();
    }

    fn registrar_busca(&self) {
        if *self.bloqueio_evento.borrow() {
            return;
        }
        let alvo = self.slider_progresso.pos() as u64;
        *self.pending_seek.borrow_mut() = Some(alvo);
    }

    fn executar_seek(&self, alvo_secs: u64) {
        *self.bloqueio_evento.borrow_mut() = true;

        if let Some(pasta) = self.pasta_atual.borrow().as_ref() {
            if let Some(nome_arquivo) = self.arquivo_atual.borrow().as_ref() {
                let caminho_completo = pasta.join(nome_arquivo);

                // Destruir o reprodutor de áudio antigo
                *self.audio_player.borrow_mut() = None;

                if let Ok(arquivo) = File::open(&caminho_completo) {
                    if let Ok((stream, stream_handle)) = OutputStream::try_default() {
                        if let Ok(sink) = Sink::try_new(&stream_handle) {
                            sink.set_volume(*self.volume_atual.borrow());

                            let leitor = BufReader::new(arquivo);
                            if let Ok(decodificador) = Decoder::new(leitor) {
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

                                *self.duracao_total_secs.borrow_mut() = total_secs;

                                self.slider_progresso.set_range_min(0);
                                self.slider_progresso.set_range_max(total_secs as usize);

                                sink.append(decodificador);

                                if alvo_secs > 0 {
                                    let _ = sink.try_seek(Duration::from_secs(alvo_secs));
                                }

                                *self.tempo_decorrido.borrow_mut() = alvo_secs as f64;
                                *self.ultimo_segundo.borrow_mut() = alvo_secs;
                                *self.audio_player.borrow_mut() = Some((stream, sink));
                                self.btn_pause.set_text("⏸");
                                if let Some(h) = self.btn_pause.handle.hwnd() {
                                    unsafe { InvalidateRect(h, ptr::null(), 1) };
                                }
                            }
                        }
                    }
                }
            }
        }

        *self.bloqueio_evento.borrow_mut() = false;
    }

    fn atualizar_progresso(&self) {
        if let Some(alvo_secs) = self.pending_seek.borrow_mut().take() {
            self.executar_seek(alvo_secs);
            return;
        }

        let total_secs = *self.duracao_total_secs.borrow();

        // Verifica se a música terminou num bloco isolado, soltando o
        // empréstimo de audio_player ANTES de chamar proxima_musica_auto
        // (que por baixo dos panos pode precisar de um borrow_mut do mesmo
        // RefCell). Sem isso, dá panic de "RefCell already borrowed".
        let terminou = {
            match *self.audio_player.borrow() {
                Some((_, ref sink)) => sink.empty() && total_secs > 0,
                None => false,
            }
        };

        if terminou {
            self.proxima_musica_auto();
            return;
        }

        let pausado = match *self.audio_player.borrow() {
            Some((_, ref sink)) => sink.is_paused(),
            None => return,
        };

        // O Timer do app roda a cada 250 milisegundos
        if !pausado {
            let mut tempo = self.tempo_decorrido.borrow_mut();
            *tempo += 0.25;

            let atual_secs = *tempo as u64;

            if atual_secs == *self.ultimo_segundo.borrow() {
                return;
            }
            *self.ultimo_segundo.borrow_mut() = atual_secs;

            let com_horas = total_secs >= 3600 || atual_secs >= 3600;

            *self.bloqueio_evento.borrow_mut() = true;
            self.slider_progresso.set_pos(atual_secs as usize);

            let txt_atual = formatar_tempo(atual_secs, com_horas);
            let txt_total = formatar_tempo(total_secs, com_horas);

            self.label_tempo
                .set_text(&format!("{} / {}", txt_atual, txt_total));
            *self.bloqueio_evento.borrow_mut() = false;
        }
    }

    fn proxima_musica_auto(&self) {
        let modo_loop = *self.modo_loop.borrow();

        if modo_loop == 1 {
            self.executar_seek(0);
        } else {
            self.tocar_proxima_manual();
        }
    }

    fn tocar_proxima_manual(&self) {
        let total = self.playlist.len();
        if total == 0 {
            return;
        }

        let idx_atual = self.indice_atual.borrow().unwrap_or(0);
        let proximo_idx = if *self.modo_shuffle.borrow() {
            let mut rng = rand::thread_rng();
            (0..total)
                .filter(|&x| x != idx_atual)
                .collect::<Vec<_>>()
                .choose(&mut rng)
                .copied()
                .unwrap_or(0)
        } else {
            let prox = idx_atual + 1;
            if prox >= total {
                if *self.modo_loop.borrow() == 2 {
                    0
                } else {
                    self.stop_music();
                    return;
                }
            } else {
                prox
            }
        };

        self.tocar_indice(proximo_idx);
    }

    fn tocar_anterior(&self) {
        let total = self.playlist.len();
        if total == 0 {
            return;
        }

        let idx_atual = self.indice_atual.borrow().unwrap_or(0);
        let anterior_idx = if idx_atual == 0 {
            total - 1
        } else {
            idx_atual - 1
        };
        self.tocar_indice(anterior_idx);
    }

    fn tocar_indice(&self, index: usize) {
        if let Some(nome_arquivo) = self.playlist.collection().get(index) {
            *self.arquivo_atual.borrow_mut() = Some(nome_arquivo.clone());
            *self.indice_atual.borrow_mut() = Some(index);
            self.playlist.set_selection(Some(index));
            self.executar_seek(0);
        }
    }

    fn alternar_pausa(&self) {
        if let Some((_, ref sink)) = *self.audio_player.borrow() {
            if sink.is_paused() {
                sink.play();
                self.btn_pause.set_text("⏸");
            } else {
                sink.pause();
                self.btn_pause.set_text("▶");
            }
        }
        if let Some(h) = self.btn_pause.handle.hwnd() {
            unsafe { InvalidateRect(h, ptr::null(), 1) };
        }
    }

    fn abrir_pasta_dialogo(&self) {
        if self.dialogo_pasta.run(Some(&self.window)) {
            if let Ok(pasta_str) = self.dialogo_pasta.get_selected_item() {
                self.carregar_pasta(PathBuf::from(pasta_str));
            }
        }
    }

    fn carregar_pasta(&self, caminho: PathBuf) {
        self.playlist.clear();
        *self.pasta_atual.borrow_mut() = Some(caminho.clone());

        if let Ok(entradas) = fs::read_dir(&caminho) {
            let mut arquivos = Vec::new();
            for entrada in entradas.flatten() {
                let path = entrada.path();
                if path.is_file() && e_arquivo_audio(&path) {
                    if let Some(nome) = path.file_name().and_then(|n| n.to_str()) {
                        arquivos.push(nome.to_string());
                    }
                }
            }
            arquivos.sort();
            for item in arquivos {
                self.playlist.push(item);
            }
        }
        self.salvar_configuracao();
    }

    fn tocar_selecionada(&self) {
        if let Some(index) = self.playlist.selection() {
            self.tocar_indice(index);
        }
    }

    fn stop_music(&self) {  
        *self.audio_player.borrow_mut() = None;
        *self.tempo_decorrido.borrow_mut() = 0.0;
        self.btn_pause.set_text("⏸");
        if let Some(h) = self.btn_pause.handle.hwnd() {
            unsafe { InvalidateRect(h, ptr::null(), 1) };
        }

        *self.bloqueio_evento.borrow_mut() = true;
        self.slider_progresso.set_pos(0);
        *self.bloqueio_evento.borrow_mut() = false;

        self.label_tempo.set_text("00:00 / 00:00");
    }
}

fn main() {
    nwg::init().expect("Falha ao inicializar a GUI");
    nwg::Font::set_global_family("Segoe UI").expect("Falha ao definir fonte");
    let _app = FurinarApp::build_ui(Default::default()).expect("Falha ao construir UI");
    nwg::dispatch_thread_events();
}
