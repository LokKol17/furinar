//! Integração com o sistema: controles de mídia (MPRIS/SMTC), botões na
//! miniatura da barra de tarefas e cantos arredondados no Windows.

use crate::*;

/// Contorna um bug conhecido do renderer de software do Slint no Windows: o
/// rastreamento de "região suja" às vezes não marca a janela inteira como
/// suja quando o fundo muda (troca de tema) ou em certos eventos de
/// composição do DWM (Aero Snap, redimensionar, trocar de monitor) — o
/// sintoma é a janela ficando parcialmente transparente até algo mais
/// forçar o redesenho daquele pedaço. `RedrawWindow` com essas flags força
/// um repaint completo e imediato, sem esperar o rastreamento de região
/// suja decidir sozinho.
#[cfg(target_os = "windows")]
pub(crate) fn forcar_repaint_completo(hwnd: HWND) {
    unsafe {
        let _ = RedrawWindow(
            Some(hwnd),
            None,
            None,
            RDW_INVALIDATE | RDW_UPDATENOW | RDW_ERASE | RDW_ALLCHILDREN,
        );
    }
}

pub(crate) fn obter_hwnd(ui: &MainWindow) -> Option<*mut std::ffi::c_void> {
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
pub(crate) fn configurar_controles_multimidia(
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
pub(crate) fn processar_eventos_multimidia(
    rx: &Receiver<MediaControlEvent>,
    estado: &mut EstadoAudio,
    ui: &MainWindow,
) {
    for evento in rx.try_iter() {
        match evento {
            MediaControlEvent::Play => retomar(estado, ui),
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
                seek_pedido(estado, ui, alvo);
            }
            MediaControlEvent::SeekBy(direcao, duracao) => {
                let delta = match direcao {
                    SeekDirection::Forward => duracao.as_secs() as i64,
                    SeekDirection::Backward => -(duracao.as_secs() as i64),
                };
                let alvo = (estado.tempo_decorrido as i64 + delta)
                    .clamp(0, estado.duracao_total_secs as i64) as u64;
                seek_pedido(estado, ui, alvo);
            }
            MediaControlEvent::SetPosition(pos) => {
                seek_pedido(estado, ui, pos.0.as_secs());
            }
            MediaControlEvent::SetVolume(v) => {
                let vol = v.clamp(0.0, 1.0) as f32;
                estado.volume_atual = vol;
                ui.set_volume(vol);
                aplicar_volume(estado, vol);
            }
            _ => {}
        }
    }
}

#[cfg(feature = "mpris")]
pub(crate) fn sincronizar_controles(
    controles: &Rc<RefCell<Option<MediaControls>>>,
    estado: &mut EstadoAudio,
) {
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
            .map(|t| (t.titulo.clone(), t.artista.clone(), t.album.clone()));

        if let Some((titulo, artista, album)) = info {
            let _ = c.set_metadata(MediaMetadata {
                title: Some(titulo.as_str()),
                artist: artista.as_deref(),
                album: album.as_deref(),
                cover_url: None,
                duration: Some(Duration::from_secs(estado.duracao_total_secs)),
            });
        }
        estado.faixa_controles = estado.arquivo_atual.clone();
    }

    // Status: atualiza quando muda (a cada segundo ou em play/pause/stop)
    let progresso = MediaPosition(Duration::from_secs(estado.ultimo_segundo));
    let playback = match &estado.audio_player {
        Some(sink) if !sink.is_paused() => MediaPlayback::Playing {
            progress: Some(progresso),
        },
        Some(_) => MediaPlayback::Paused {
            progress: Some(progresso),
        },
        None => MediaPlayback::Stopped,
    };
    if estado.status_controles.as_ref() != Some(&playback) {
        let _ = c.set_playback(playback.clone());
        estado.status_controles = Some(playback);
    }
}

// IDs dos botões (viram LOWORD(wParam) no WM_COMMAND).
#[cfg(target_os = "windows")]
pub(crate) const BTN_ANTERIOR: u32 = 0x100;

#[cfg(target_os = "windows")]
pub(crate) const BTN_PLAY_PAUSE: u32 = 0x101;

#[cfg(target_os = "windows")]
pub(crate) const BTN_PROXIMA: u32 = 0x102;

// Índices dos ícones na HIMAGELIST, na ordem em que são inseridos.
#[cfg(target_os = "windows")]
pub(crate) const IDX_ICONE_ANTERIOR: u32 = 0;

#[cfg(target_os = "windows")]
pub(crate) const IDX_ICONE_PLAY: u32 = 1;

#[cfg(target_os = "windows")]
pub(crate) const IDX_ICONE_PAUSE: u32 = 2;

#[cfg(target_os = "windows")]
pub(crate) const IDX_ICONE_PROXIMA: u32 = 3;

/// Recursos dos botões da taskbar. Precisam continuar vivos enquanto o app
/// roda: a shell referencia a HIMAGELIST e os HICONs são nossos.
#[cfg(target_os = "windows")]
pub(crate) struct BotoesTaskbar {
    pub(crate) taskbar: ITaskbarList3,
    pub(crate) hwnd: HWND,
    pub(crate) himl: HIMAGELIST,
    pub(crate) icones: Vec<HICON>,
    pub(crate) pausado: bool,
}

#[cfg(target_os = "windows")]
impl BotoesTaskbar {
    /// Troca o ícone do botão central entre play e pause.
    pub(crate) fn atualizar_play_pause(&mut self, pausado: bool) {
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
pub(crate) fn forma_triangulo(x: i32, y: i32, apex_x: i32, base_x: i32) -> bool {
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
pub(crate) fn forma_play(x: i32, y: i32) -> bool {
    forma_triangulo(x, y, 12, 4)
}

#[cfg(target_os = "windows")]
pub(crate) fn forma_pause(x: i32, y: i32) -> bool {
    ((4..=6).contains(&x) || (9..=11).contains(&x)) && (3..=12).contains(&y)
}

#[cfg(target_os = "windows")]
pub(crate) fn forma_anterior(x: i32, y: i32) -> bool {
    ((2..=3).contains(&x) && (3..=12).contains(&y)) || forma_triangulo(x, y, 5, 13)
}

#[cfg(target_os = "windows")]
pub(crate) fn forma_proxima(x: i32, y: i32) -> bool {
    forma_triangulo(x, y, 10, 2) || ((11..=12).contains(&x) && (3..=12).contains(&y))
}

/// Gera as máscaras AND/XOR de um ícone monocromático 1bpp.
///
/// Fundo: AND=1 e XOR=0 (transparente). Forma: AND=0 e XOR=1 (branco).
#[cfg(target_os = "windows")]
pub(crate) fn mascaras_icone(
    tamanho: i32,
    dentro: impl Fn(i32, i32) -> bool,
) -> (Vec<u8>, Vec<u8>) {
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
pub(crate) fn criar_icone(
    tamanho: i32,
    dentro: impl Fn(i32, i32) -> bool,
) -> windows::core::Result<HICON> {
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
pub(crate) fn criar_botao(id: u32, indice_icone: u32, icone: HICON, dica: &str) -> THUMBBUTTON {
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
pub(crate) fn configurar_botoes_taskbar(ui: &MainWindow) -> Option<BotoesTaskbar> {
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
pub(crate) fn processar_eventos_taskbar(
    rx: &Receiver<u32>,
    estado: &mut EstadoAudio,
    ui: &MainWindow,
) {
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
pub(crate) fn atualizar_icone_taskbar(
    botoes: &Rc<RefCell<Option<BotoesTaskbar>>>,
    estado: &EstadoAudio,
) {
    let pausado = match &estado.audio_player {
        Some(sink) => sink.is_paused(),
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
pub(crate) fn arredondar_cantos(ui: &MainWindow) {
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

#[cfg(not(target_os = "windows"))]
pub(crate) fn arredondar_cantos(_ui: &MainWindow) {}

/// Status do Discord a partir do que está tocando (pausado ou parado limpa).
/// Com a opção desligada a conexão é solta.
pub(crate) fn atualizar_discord(estado: &mut EstadoAudio) {
    if !estado.discord_ativo {
        estado.discord = None;
        return;
    }
    let tocando = estado.audio_player.as_ref().is_some_and(|p| !p.is_paused());
    let mut pedir_capa = None;
    let status = tocando
        .then(|| {
            let pasta = estado.pastas.get(estado.pasta_reproducao?)?;
            let indice = estado.indice_atual?;
            let t = pasta.tracks.get(indice)?;
            let chave = pasta.chave_faixa(indice);
            // Capa: a do lançamento corrigido no MusicBrainz; senão a do
            // Deezer (pedida uma vez por faixa); até chegar, a logo
            let capa = chave
                .as_ref()
                .and_then(|k| estado.metadados.correcao(k)?.lancamento.clone())
                .map(|l| format!("https://coverartarchive.org/release/{l}/front-250"))
                .or_else(|| {
                    let k = chave.as_ref()?;
                    match estado.capas_discord.get(k) {
                        Some(url) => url.clone(),
                        None => {
                            pedir_capa = Some(buscador::Pedido::CapaDiscord {
                                chave: k.clone(),
                                titulo: t.titulo.clone(),
                                artista: t.artista.clone(),
                                duracao: estado.duracao_total_secs,
                            });
                            None
                        }
                    }
                });
            Some(discord::Status {
                titulo: t.titulo.clone(),
                artista: t.artista.clone(),
                album: t.album.clone(),
                posicao: estado.tempo_decorrido as u64,
                duracao: estado.duracao_total_secs,
                capa,
            })
        })
        .flatten();
    if let Some(buscador::Pedido::CapaDiscord { chave, .. }) = &pedir_capa {
        estado.capas_discord.insert(chave.clone(), None);
    }
    if let Some(pedido) = pedir_capa {
        pedir_web(estado, pedido);
    }
    estado
        .discord
        .get_or_insert_with(discord::Discord::novo)
        .mostrar(status);
}

/// Aviso rápido no rodapé, que some sozinho. Um aviso novo substitui o
/// anterior, e o timer do anterior não apaga o novo (`toast_id`).
pub(crate) fn avisar(ui: &MainWindow, texto: &str) {
    let id = ui.get_toast_id().wrapping_add(1);
    ui.set_toast_id(id);
    ui.set_toast_texto(texto.into());
    ui.set_toast_visivel(true);
    let ui_fraca = ui.as_weak();
    Timer::single_shot(Duration::from_millis(2200), move || {
        if let Some(ui) = ui_fraca.upgrade()
            && ui.get_toast_id() == id
        {
            ui.set_toast_visivel(false);
        }
    });
}

/// Abre uma página no navegador padrão.
pub(crate) fn abrir_no_navegador(url: &str) {
    #[cfg(target_os = "windows")]
    let comando = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
    #[cfg(not(target_os = "windows"))]
    let comando = std::process::Command::new("xdg-open").arg(url).spawn();
    let _ = comando;
}

/// Liga os callbacks da UI desta área.
pub(crate) fn ligar(ui: &MainWindow, estado: &Rc<RefCell<EstadoAudio>>) {
    {
        let estado = estado.clone();
        ui.on_alternar_discord(move |ligado| {
            let mut e = estado.borrow_mut();
            e.discord_ativo = ligado;
            // Desligar solta a conexão já (e o Discord limpa o status)
            atualizar_discord(&mut e);
            salvar_configuracao(&e);
        });
    }
}
