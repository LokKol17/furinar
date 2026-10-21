// Em release não abrimos console junto do app; no debug mantemos, para ver logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

slint::include_modules!();

mod acoustid;
mod biblioteca;
mod buscador;
mod capa;
mod config;
mod fundo;
mod imersivo;
mod letras_web;
mod lrc;
mod memoria;
mod metadados;
mod miniaturas;
mod musicbrainz;
mod painel;
mod rede;
mod sono;
mod track;
mod transicao;
mod translations;
mod updates;
mod visualizador;

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
use std::time::{Duration, Instant};

use biblioteca::{Biblioteca, chave_faixa};
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
    i18n.set_tip_sort(g("tip_sort"));
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

/// Chaves das listas especiais na config (caminho de pasta nunca começa
/// com `:`).
const CHAVE_FAVORITAS: &str = ":favoritas";
const CHAVE_MAIS_TOCADAS: &str = ":mais_tocadas";
/// As listas especiais ocupam as primeiras posições de `pastas`, nesta ordem.
const POS_FAVORITAS: usize = 0;
const POS_MAIS_TOCADAS: usize = 1;
const ESPECIAIS: usize = 2;

#[derive(Clone, PartialEq)]
enum Tipo {
    Pasta,
    Favoritas,
    MaisTocadas,
    /// Aba temporária com um álbum, aberta pela grade do modo imersivo. Não
    /// é salva; `origem` é a lista de onde ele saiu.
    Album {
        origem: String,
    },
}

/// Uma aba: pasta de música aberta pelo usuário ou lista especial. Nas
/// especiais `caminho` é vazio e cada faixa guarda o caminho absoluto.
struct PastaMusical {
    caminho: PathBuf,
    /// Nome de exibição na aba (último componente do caminho).
    nome: String,
    tracks: Vec<TrackInfo>,
    /// Se as faixas já foram escaneadas. O scan é preguiçoso: pastas abertas
    /// mas nunca visitadas ficam sem `tracks` até serem necessárias. Nas
    /// especiais, `false` pede para remontar a lista.
    carregada: bool,
    tipo: Tipo,
    ordem: Ordem,
}

impl PastaMusical {
    fn pasta(caminho: PathBuf) -> Self {
        Self {
            nome: nome_da_pasta(&caminho),
            caminho,
            tracks: Vec::new(),
            carregada: false,
            tipo: Tipo::Pasta,
            ordem: Ordem::Padrao,
        }
    }

    fn especial(tipo: Tipo, ordem: Ordem) -> Self {
        Self {
            caminho: PathBuf::new(),
            nome: String::new(),
            tracks: Vec::new(),
            carregada: false,
            tipo,
            ordem,
        }
    }

    /// Identidade da lista na config.
    fn chave(&self) -> String {
        match &self.tipo {
            Tipo::Pasta => self.caminho.to_string_lossy().to_string(),
            Tipo::Favoritas => CHAVE_FAVORITAS.to_string(),
            Tipo::MaisTocadas => CHAVE_MAIS_TOCADAS.to_string(),
            Tipo::Album { origem } => origem.clone(),
        }
    }

    fn caminho_faixa(&self, indice: usize) -> Option<PathBuf> {
        Some(self.caminho.join(&self.tracks.get(indice)?.path))
    }

    fn chave_faixa(&self, indice: usize) -> Option<String> {
        Some(chave_faixa(&self.caminho, &self.tracks.get(indice)?.path))
    }

    /// Pasta onde procurar fundo do modo imersivo: a da aba, ou a da própria
    /// faixa nas listas especiais.
    fn raiz_da_faixa(&self, indice: usize) -> PathBuf {
        if self.tipo == Tipo::Pasta {
            return self.caminho.clone();
        }
        self.caminho_faixa(indice)
            .and_then(|c| c.parent().map(PathBuf::from))
            .unwrap_or_default()
    }
}

/// Contagem do lote em andamento (ou do último).
#[derive(Default)]
struct Lote {
    ativo: bool,
    feitas: usize,
    total: usize,
    automaticas: usize,
    sem_resultado: usize,
    letras: usize,
}

/// Como mostrar um candidato na tela de informações.
fn linha_candidato(c: &metadados::Candidato, pelo_som: &str) -> CandidatoInfo {
    let mut detalhe = c.artista.clone();
    if let Some(album) = &c.album {
        detalhe.push_str(" · ");
        detalhe.push_str(album);
        if let Some(ano) = c.ano {
            detalhe.push_str(&format!(" ({ano})"));
        }
    }
    if c.duracao > 0 {
        detalhe.push_str(" · ");
        detalhe.push_str(&formatar_tempo(c.duracao as u64, c.duracao >= 3600));
    }
    if c.fonte == metadados::Fonte::ImpressaoDigital {
        detalhe.push_str(" · ");
        detalhe.push_str(pelo_som);
    }
    CandidatoInfo {
        titulo: c.titulo.as_str().into(),
        detalhe: detalhe.into(),
        confianca: c.confianca as i32,
        digital: c.fonte == metadados::Fonte::ImpressaoDigital,
    }
}

/// Faixa da próxima música já carregada no player (gapless): começa sozinha
/// quando a atual acabar.
struct Engatilhada {
    pasta: usize,
    indice: usize,
    duracao: u64,
    controle: transicao::Controle,
}

// ---------------------------------------------------------------------
// Estado de áudio/playlist
// ---------------------------------------------------------------------

struct EstadoAudio {
    /// Saída de áudio do sistema. Aberta uma vez e reaproveitada entre seeks
    /// e trocas de faixa: reabrir o dispositivo a cada vez recria o stream,
    /// a thread e os buffers do backend. Só é fechada no stop.
    saida: Option<MixerDeviceSink>,
    /// Amostras para o visualizador; só copia com o modo imersivo ligado.
    captura: visualizador::Captura,
    audio_player: Option<Player>,
    /// Faixa (caminho completo) cuja duração está em `duracao_total_secs`.
    /// Evita varrer o MP3 inteiro de novo a cada seek.
    duracao_de: Option<PathBuf>,
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
    /// Shuffle comum: próximas faixas já sorteadas, para a fila "a seguir"
    /// mostrar o que vai tocar de verdade. Zera junto com a sacola.
    sorteio: Vec<usize>,
    /// Temporizador de sono ligado.
    sono: Option<sono::Sono>,
    /// O que tocou antes (lista, chave da faixa), para o "anterior" do
    /// shuffle voltar ao que tocou de verdade e não à vizinha na lista.
    historico: Vec<(usize, String)>,
    /// Voltando pelo histórico: a troca não entra nele de novo.
    voltando: bool,
    /// Favoritas e contagem de reproduções.
    biblioteca: Biblioteca,
    /// A faixa atual já contou uma reprodução.
    contada: bool,
    /// Próxima faixa já na fila do player (gapless).
    engatilhada: Option<Engatilhada>,
    /// A transição para a próxima já foi preparada (ou tentada) nesta faixa.
    transicao_feita: bool,
    /// Faixa anterior sumindo no crossfade, até acabar.
    saindo: Option<Player>,
    /// Pede o fade-out da faixa atual.
    controle: transicao::Controle,
    /// Segundos de crossfade (0 = só gapless).
    crossfade: u8,
    replaygain: bool,
    fundo_reativo: bool,
    /// Correções de metadados vindas da web (banco local).
    metadados: metadados::Banco,
    /// Thread da busca na web; só existe com trabalho.
    buscador: Option<buscador::Buscador>,
    /// Buscar a letra na web quando a faixa tocando não tem.
    letras_web: bool,
    /// Chave de aplicativo do AcoustID ("" = sem impressão digital).
    chave_acoustid: String,
    /// Tela de informações: faixa (chave) e candidatos da última busca.
    info_chave: Option<String>,
    info_candidatos: Vec<metadados::Candidato>,
    /// Lote da aba: o que ficou para revisão e a contagem.
    revisao: Vec<(metadados::Consulta, metadados::Candidato)>,
    lote: Lote,
    escanear_subpastas: bool,
    tema_claro: bool,
    idioma: String,
    /// Mapeia posição na lista visível -> índice em `pastas[aba_visivel].tracks`.
    indices_visiveis: Vec<usize>,
    /// Texto atual da busca (título/artista).
    filtro: String,
    /// Letra sincronizada da faixa atual (tempo em segundos, texto), ordenada.
    letra_atual: Vec<(f64, String)>,
    /// A letra atual não tem tempos (veio da tag sem sincronia).
    letra_livre: bool,
    /// Índice da linha da letra exibida (-1 = antes da primeira).
    letra_indice: i32,
    /// Índice absoluto da primeira linha do recorte que está na UI. Serve para
    /// traduzir o clique numa linha do painel de volta para o timestamp.
    letra_janela_inicio: usize,
    /// Modo imersivo (tela cheia). `None` = desligado, nada alocado.
    imersivo: Option<imersivo::EstadoImersivo>,
    /// Fundos do modo imersivo escolhidos nas configurações.
    fundos_globais: Vec<PathBuf>,
    #[cfg(feature = "mpris")]
    faixa_controles: Option<String>,
    #[cfg(feature = "mpris")]
    status_controles: Option<MediaPlayback>,
}

impl Default for EstadoAudio {
    fn default() -> Self {
        Self {
            saida: None,
            audio_player: None,
            captura: visualizador::Captura::default(),
            duracao_de: None,
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
            sorteio: Vec::new(),
            sono: None,
            historico: Vec::new(),
            voltando: false,
            biblioteca: Biblioteca::default(),
            contada: false,
            engatilhada: None,
            transicao_feita: false,
            saindo: None,
            controle: transicao::Controle::default(),
            crossfade: 0,
            replaygain: true,
            fundo_reativo: true,
            metadados: metadados::Banco::default(),
            buscador: None,
            letras_web: false,
            chave_acoustid: String::new(),
            info_chave: None,
            info_candidatos: Vec::new(),
            revisao: Vec::new(),
            lote: Lote::default(),
            escanear_subpastas: false,
            tema_claro: false,
            idioma: "pt-br".to_string(),
            indices_visiveis: Vec::new(),
            filtro: String::new(),
            letra_atual: Vec::new(),
            letra_livre: false,
            letra_indice: -1,
            letra_janela_inicio: 0,
            imersivo: None,
            fundos_globais: Vec::new(),
            #[cfg(feature = "mpris")]
            faixa_controles: None,
            #[cfg(feature = "mpris")]
            status_controles: None,
        }
    }
}

/// Quantas próximas do shuffle vão para a config (a sacola inteira do
/// inteligente pode ter milhares).
const FILA_SALVA: usize = 50;

fn salvar_configuracao(estado: &EstadoAudio) {
    let lista = |i: usize| estado.pastas.get(i).map(PastaMusical::chave);
    let reproducao = estado.pasta_reproducao.and_then(|i| estado.pastas.get(i));
    let faixa_salva =
        reproducao.and_then(|p| Some(chave_faixa(&p.caminho, estado.arquivo_atual.as_deref()?)));
    let proximas: Vec<usize> = match estado.modo_shuffle {
        2 => estado
            .sacola_shuffle
            .iter()
            .rev()
            .take(FILA_SALVA)
            .copied()
            .collect(),
        1 => estado.sorteio.clone(),
        _ => Vec::new(),
    };
    let fila_shuffle = reproducao
        .map(|p| proximas.iter().filter_map(|&i| p.chave_faixa(i)).collect())
        .unwrap_or_default();
    let config = AppConfig {
        pasta: None,
        pastas: estado
            .pastas
            .iter()
            .filter(|p| p.tipo == Tipo::Pasta)
            .map(|p| p.caminho.to_string_lossy().to_string())
            .collect(),
        aba_visivel_salva: 0,
        pasta_reproducao_salva: None,
        indice_atual: None,
        aba_salva: lista(estado.aba_visivel),
        lista_salva: estado.pasta_reproducao.and_then(lista),
        faixa_salva,
        fila_shuffle,
        ordens: estado
            .pastas
            .iter()
            .filter(|p| p.ordem != Ordem::Padrao && !matches!(p.tipo, Tipo::Album { .. }))
            .map(|p| (p.chave(), p.ordem.indice() as u8))
            .collect(),
        crossfade: estado.crossfade,
        letras_web: estado.letras_web,
        chave_acoustid: estado.chave_acoustid.clone(),
        replaygain: estado.replaygain,
        fundo_reativo: estado.fundo_reativo,
        volume: estado.volume_atual,
        modo_loop: estado.modo_loop,
        shuffle: false,
        modo_shuffle: estado.modo_shuffle,
        escanear_subpastas: estado.escanear_subpastas,
        tema_claro: estado.tema_claro,
        idioma: estado.idioma.clone(),
        fundo_tela_cheia: None,
        fundos_tela_cheia: estado
            .fundos_globais
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect(),
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
    let indices = ordem_lista(estado, estado.aba_visivel, &estado.filtro);
    let modelo: Vec<LinhaLista> = match estado.pastas.get(estado.aba_visivel) {
        Some(pasta) => indices
            .iter()
            .map(|&i| LinhaLista {
                texto: texto_exibicao(&pasta.tracks[i]).into(),
                favorita: pasta
                    .chave_faixa(i)
                    .is_some_and(|k| estado.biblioteca.e_favorita(&k)),
            })
            .collect(),
        None => Vec::new(),
    };

    estado.indices_visiveis = indices;
    ui.set_musicas(ModelRc::new(VecModel::from(modelo)));
    ui.set_ordem_atual(
        estado
            .pastas
            .get(estado.aba_visivel)
            .map_or(0, |p| p.ordem.indice() as i32),
    );
    atualizar_favorita_atual(estado, ui);

    atualizar_selecao_visivel(estado, ui);
    atualizar_painel(estado, ui);
    // O filtro da aba que toca muda a ordem de navegação
    atualizar_fila(estado);
}

/// Espelha a lista de pastas e os índices das abas na UI.
fn atualizar_abas(estado: &EstadoAudio, ui: &MainWindow) {
    let lang = translations::get_translations(&estado.idioma);
    let nomes: Vec<SharedString> = estado
        .pastas
        .iter()
        .map(|p| match p.tipo {
            Tipo::Favoritas => lang.get("tab_favorites").copied().unwrap_or("").into(),
            Tipo::MaisTocadas => lang.get("tab_most_played").copied().unwrap_or("").into(),
            _ => p.nome.as_str().into(),
        })
        .collect();
    ui.set_abas(ModelRc::new(VecModel::from(nomes)));
    ui.set_aba_ativa(estado.aba_visivel as i32);
    ui.set_aba_tocando(estado.pasta_reproducao.map_or(-1, |i| i as i32));
}

/// Escaneia a pasta na primeira vez que ela é necessária. Idempotente.
fn garantir_pasta_carregada(estado: &mut EstadoAudio, indice: usize) {
    let escanear_sub = estado.escanear_subpastas;
    let Some(pasta) = estado.pastas.get(indice) else {
        return;
    };
    if pasta.carregada {
        return;
    }
    let chaves: Vec<String> = match pasta.tipo {
        Tipo::Favoritas => estado.biblioteca.favoritas().to_vec(),
        Tipo::MaisTocadas => estado
            .biblioteca
            .mais_tocadas(biblioteca::TAMANHO_MAIS_TOCADAS)
            .into_iter()
            .map(String::from)
            .collect(),
        Tipo::Pasta => Vec::new(),
        // A aba do álbum já nasce com as faixas
        Tipo::Album { .. } => return,
    };
    if pasta.tipo != Tipo::Pasta {
        let tracks = faixas_por_chave(&estado.pastas, &chaves, &estado.metadados);
        let pasta = &mut estado.pastas[indice];
        pasta.tracks = tracks;
        pasta.carregada = true;
        return;
    }

    let tracks = escanear(
        &estado.pastas[indice].caminho,
        escanear_sub,
        &estado.metadados,
    );
    let pasta = &mut estado.pastas[indice];
    pasta.tracks = tracks;
    pasta.carregada = true;
}

/// Faixas de uma pasta, em ordem de nome, com as correções da web por cima.
fn escanear(caminho: &Path, recursivo: bool, banco: &metadados::Banco) -> Vec<TrackInfo> {
    let mut tracks = Vec::new();
    coletar_arquivos_audio(caminho, caminho, recursivo, &mut tracks);
    tracks.sort_by(|a, b| a.path.cmp(&b.path));
    banco.aplicar(&mut tracks, |t| chave_faixa(caminho, &t.path));
    tracks
}

/// Faixas das listas especiais, a partir dos caminhos. Quem já está numa pasta
/// carregada vem de lá (sem ler o arquivo de novo); o resto é lido agora.
/// Arquivo que sumiu fica de fora.
fn faixas_por_chave(
    pastas: &[PastaMusical],
    chaves: &[String],
    banco: &metadados::Banco,
) -> Vec<TrackInfo> {
    let mut conhecidas: HashMap<String, &TrackInfo> = HashMap::new();
    for pasta in pastas
        .iter()
        .filter(|p| p.tipo == Tipo::Pasta && p.carregada)
    {
        for t in &pasta.tracks {
            conhecidas.insert(chave_faixa(&pasta.caminho, &t.path), t);
        }
    }
    chaves
        .iter()
        .filter_map(|chave| {
            let mut t = match conhecidas.get(chave) {
                Some(t) => (*t).clone(),
                None => {
                    let caminho = Path::new(chave);
                    if !caminho.is_file() {
                        return None;
                    }
                    let mut t = track::ler_faixa(caminho, chave.clone());
                    if let Some(c) = banco.correcao(chave) {
                        c.aplicar(&mut t);
                    }
                    t
                }
            };
            t.path = chave.clone();
            Some(t)
        })
        .collect()
}

/// Ordem de exibição e navegação de uma lista: filtro da busca e depois a
/// ordem escolhida para a aba.
fn ordem_lista(estado: &EstadoAudio, indice: usize, filtro: &str) -> Vec<usize> {
    let Some(pasta) = estado.pastas.get(indice) else {
        return Vec::new();
    };
    let mut ordem = ordem_filtrada(&pasta.tracks, filtro);
    track::ordenar(&mut ordem, &pasta.tracks, pasta.ordem, |i| {
        pasta
            .chave_faixa(i)
            .map_or(0, |k| estado.biblioteca.reproducoes(&k))
    });
    ordem
}

/// Acha de novo a faixa atual na lista que toca, pelo nome: depois de um
/// reescaneio ou de uma lista especial remontada, as posições mudam.
/// `false` quando ela não está mais lá.
fn reposicionar_atual(estado: &mut EstadoAudio) -> bool {
    let (Some(p), Some(nome)) = (estado.pasta_reproducao, estado.arquivo_atual.as_deref()) else {
        return true;
    };
    let novo = estado
        .pastas
        .get(p)
        .and_then(|pasta| pasta.tracks.iter().position(|t| t.path == nome));
    estado.indice_atual = novo;
    novo.is_some()
}

/// Remonta uma lista especial depois que as favoritas ou as contagens
/// mudaram. Se ela está tocando e a faixa atual sairia dela, espera: a lista
/// seria puxada debaixo do que toca. Volta a ser montada na próxima visita.
fn renovar_especial(estado: &mut EstadoAudio, ui: &MainWindow, indice: usize) {
    let Some(pasta) = estado.pastas.get_mut(indice) else {
        return;
    };
    pasta.carregada = false;
    let visivel = estado.aba_visivel == indice;
    let tocando = estado.pasta_reproducao == Some(indice);
    if !visivel && !tocando {
        return;
    }
    let antes = std::mem::take(&mut estado.pastas[indice].tracks);
    garantir_pasta_carregada(estado, indice);
    if tocando {
        if !reposicionar_atual(estado) {
            // Fica como estava até a faixa acabar
            let pasta = &mut estado.pastas[indice];
            pasta.tracks = antes;
            pasta.carregada = false;
            reposicionar_atual(estado);
            if visivel {
                atualizar_lista(estado, ui);
            }
            return;
        }
        estado.sacola_shuffle.clear();
        estado.sorteio.clear();
    }
    if visivel {
        atualizar_lista(estado, ui);
    }
}

/// Abre uma pasta: entra como nova aba, ou só troca para a aba já existente.
/// Não interfere na reprodução em andamento.
fn abrir_pasta(estado: &mut EstadoAudio, ui: &MainWindow, caminho: PathBuf) {
    if let Some(idx) = estado
        .pastas
        .iter()
        .position(|p| p.tipo == Tipo::Pasta && p.caminho == caminho)
    {
        estado.aba_visivel = idx;
        // Pode ser uma pasta que ainda não foi visitada nesta sessão
        garantir_pasta_carregada(estado, idx);
        atualizar_abas(estado, ui);
        atualizar_lista(estado, ui);
        return;
    }

    let tracks = escanear(&caminho, estado.escanear_subpastas, &estado.metadados);
    let mut pasta = PastaMusical::pasta(caminho);
    pasta.tracks = tracks;
    // Já escaneada acima: não passa pelo caminho preguiçoso
    pasta.carregada = true;
    // Antes da aba do álbum, que fica sempre por último
    let pos = estado
        .pastas
        .iter()
        .position(|p| matches!(p.tipo, Tipo::Album { .. }))
        .unwrap_or(estado.pastas.len());
    estado.pastas.insert(pos, pasta);
    if let Some(p) = estado.pasta_reproducao.as_mut()
        && *p >= pos
    {
        *p += 1;
    }
    estado.aba_visivel = pos;

    atualizar_abas(estado, ui);
    atualizar_lista(estado, ui);
}

/// Fecha uma aba. Se era a pasta que estava tocando, a reprodução para.
fn fechar_pasta(estado: &mut EstadoAudio, ui: &MainWindow, idx: usize) {
    // As listas especiais não fecham
    if idx < ESPECIAIS || idx >= estado.pastas.len() {
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
    atualizar_imersivo(estado, ui);
}

/// Abre (ou troca) a aba do álbum com as faixas do grupo, na ordem do disco,
/// e toca a primeira.
fn abrir_album(estado: &mut EstadoAudio, ui: &MainWindow, grupo: &track::GrupoAlbum) {
    let Some(origem) = estado.pastas.get(estado.aba_visivel) else {
        return;
    };
    let mut tracks: Vec<TrackInfo> = grupo
        .faixas
        .iter()
        .filter_map(|&i| {
            let mut t = origem.tracks.get(i)?.clone();
            t.path = chave_faixa(&origem.caminho, &t.path);
            Some(t)
        })
        .collect();
    tracks.sort_by(|a, b| a.numero.cmp(&b.numero).then_with(|| a.path.cmp(&b.path)));
    let chave_origem = match &origem.tipo {
        Tipo::Album { origem } => origem.clone(),
        _ => origem.chave(),
    };
    let mut pasta = PastaMusical::especial(
        Tipo::Album {
            origem: chave_origem,
        },
        Ordem::Padrao,
    );
    pasta.nome = grupo.nome.clone();
    pasta.tracks = tracks;
    pasta.carregada = true;

    // Uma aba de álbum só, sempre a última
    let pos = match estado
        .pastas
        .iter()
        .position(|p| matches!(p.tipo, Tipo::Album { .. }))
    {
        Some(pos) => {
            estado.pastas[pos] = pasta;
            if estado.pasta_reproducao == Some(pos) {
                // Mesma aba, faixas novas: a sacola era do álbum anterior
                estado.sacola_shuffle.clear();
                estado.sorteio.clear();
            }
            pos
        }
        None => {
            estado.pastas.push(pasta);
            estado.pastas.len() - 1
        }
    };
    estado.aba_visivel = pos;
    // A busca da aba anterior esconderia faixas do álbum
    estado.filtro.clear();
    ui.set_texto_busca(SharedString::new());
    atualizar_abas(estado, ui);
    atualizar_lista(estado, ui);
    tocar_faixa(estado, ui, pos, 0);
    ui.set_imv_guia(0);
}

/// Opções do crossfade, em segundos.
const OPCOES_CROSSFADE: [u8; 4] = [0, 3, 6, 10];

fn proximo_crossfade(atual: u8) -> u8 {
    let pos = OPCOES_CROSSFADE
        .iter()
        .position(|&o| o == atual)
        .unwrap_or(0);
    OPCOES_CROSSFADE[(pos + 1) % OPCOES_CROSSFADE.len()]
}

fn publicar_crossfade(estado: &EstadoAudio, ui: &MainWindow) {
    let lang = translations::get_translations(&estado.idioma);
    let rotulo = lang.get("crossfade").copied().unwrap_or("crossfade");
    let valor = if estado.crossfade == 0 {
        lang.get("crossfade_off").copied().unwrap_or("").to_string()
    } else {
        format!("{} s", estado.crossfade)
    };
    ui.set_crossfade_texto(format!("{rotulo}: {valor}").into());
}

/// Reescaneia todas as pastas (a opção de subpastas é global) e reposiciona a
/// faixa atual pelo caminho, já que os índices podem ter mudado.
fn reescaneiar_pastas(estado: &mut EstadoAudio, ui: &MainWindow) {
    let recursivo = estado.escanear_subpastas;
    for pasta in &mut estado.pastas {
        // Pastas ainda não carregadas serão escaneadas com a nova opção quando
        // forem abertas; carregá-las agora só gastaria memória à toa.
        if !pasta.carregada || pasta.tipo != Tipo::Pasta {
            continue;
        }
        pasta.tracks = escanear(&pasta.caminho, recursivo, &estado.metadados);
    }

    // Índices podem ter mudado com o reescaneio; a sacola do shuffle
    // inteligente não serve mais.
    estado.sacola_shuffle.clear();
    estado.sorteio.clear();

    if !reposicionar_atual(estado) {
        // A faixa atual já não está na pasta
        estado.pasta_reproducao = None;
        estado.indice_atual = None;
        estado.arquivo_atual = None;
        stop_music(estado, ui);
    }

    atualizar_abas(estado, ui);
    atualizar_lista(estado, ui);
    atualizar_imersivo(estado, ui);
}

// ---------------------------------------------------------------------
// Letras sincronizadas (.lrc)
// ---------------------------------------------------------------------

/// Carrega a letra da faixa e já publica o primeiro recorte: o `.lrc` ao
/// lado da faixa ou, na falta dele, a letra embutida nas tags. Não ter letra
/// é o caso normal ("sem letra disponível"), não uma falha.
fn carregar_letra(
    estado: &mut EstadoAudio,
    ui: &MainWindow,
    caminho_faixa: &std::path::Path,
    tempo: f64,
) {
    let chave = biblioteca::chave_caminho(caminho_faixa);
    let letra = match fs::read_to_string(caminho_lrc(caminho_faixa)) {
        Ok(conteudo) => lrc::Letra::Sincronizada(parse_lrc(&conteudo)),
        Err(_) => {
            letras_web::do_cache(&chave).unwrap_or_else(|| lrc::letra_embutida(caminho_faixa))
        }
    };

    estado.letra_livre = matches!(&letra, lrc::Letra::Livre(l) if !l.is_empty());
    estado.letra_atual = letra.linhas();
    ui.set_letra_livre(estado.letra_livre);
    estado.letra_indice = if estado.letra_livre {
        -1
    } else {
        indice_letra(&estado.letra_atual, tempo)
    };
    // O modo imersivo rola a letra inteira; o painel normal usa só o recorte
    let completa: Vec<SharedString> = estado
        .letra_atual
        .iter()
        .map(|(_, t)| t.as_str().into())
        .collect();
    ui.set_letra_completa(ModelRc::new(VecModel::from(completa)));
    publicar_letra(estado, ui);
}

/// Recalcula a linha atual e só mexe na UI quando ela muda.
fn sincronizar_letra(estado: &mut EstadoAudio, ui: &MainWindow) {
    if estado.letra_livre {
        return;
    }
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
    ui.set_letra_indice(estado.letra_indice);
}

// ---------------------------------------------------------------------
// Modo imersivo
// ---------------------------------------------------------------------

/// Resumo dos fundos globais nas configurações: o nome do arquivo, ou
/// quantos são ("" = nenhum).
fn nome_fundo(fundos: &[PathBuf], idioma: &str) -> String {
    match fundos {
        [] => String::new(),
        [um] => um
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        varios => {
            let lang = translations::get_translations(idioma);
            lang.get("bg_count")
                .copied()
                .unwrap_or("{n}")
                .replace("{n}", &varios.len().to_string())
        }
    }
}

/// Faixa atual no formato que o modo imersivo usa.
fn faixa_imersiva(estado: &EstadoAudio) -> Option<imersivo::Faixa> {
    let pasta = estado.pastas.get(estado.pasta_reproducao?)?;
    let indice = estado.indice_atual?;
    let track = pasta.tracks.get(indice)?;
    let correcao = estado
        .metadados
        .correcao(&chave_faixa(&pasta.caminho, &track.path));
    Some(imersivo::Faixa {
        capa_web: correcao
            .and_then(|c| c.lancamento.as_deref())
            .and_then(metadados::capa_web),
        album: correcao.and_then(|c| c.album.clone()),
        ano: correcao.and_then(|c| c.ano),
        caminho: pasta.caminho.join(&track.path),
        pasta_raiz: pasta.raiz_da_faixa(indice),
        titulo: track.titulo.clone(),
        artista: track.artista.clone(),
        fundos_globais: estado.fundos_globais.clone(),
    })
}

/// Avisa o modo imersivo (se ligado) que a faixa atual pode ter mudado.
fn atualizar_imersivo(estado: &mut EstadoAudio, ui: &MainWindow) {
    if estado.imersivo.is_none() {
        return;
    }
    let faixa = faixa_imersiva(estado);
    if let Some(imv) = estado.imersivo.as_mut() {
        imv.trocar_faixa(ui, faixa);
    }
    atualizar_painel(estado, ui);
    atualizar_fila(estado);
}

/// Republica a janela de linhas do painel da biblioteca (se aberto).
fn atualizar_painel(estado: &mut EstadoAudio, ui: &MainWindow) {
    let Some(painel) = estado.imersivo.as_mut().and_then(|i| i.painel.as_mut()) else {
        return;
    };
    let pasta = estado.pastas.get(estado.aba_visivel);
    let tocando = if estado.pasta_reproducao == Some(estado.aba_visivel) {
        estado.indice_atual
    } else {
        None
    };
    let indices = &estado.indices_visiveis;
    let bib = &estado.biblioteca;
    let banco = &estado.metadados;
    painel.publicar(ui, indices.len(), |pos| {
        item_painel(pasta?, *indices.get(pos)?, tocando, bib, banco)
    });
    if painel.mostrando_albuns()
        && let Some(pasta) = pasta
    {
        let lang = translations::get_translations(&estado.idioma);
        painel.publicar_albuns(track::agrupar_albuns(&pasta.tracks, indices), &lang, |i| {
            Some((pasta.caminho_faixa(i)?, capa_web_de(banco, pasta, i)))
        });
    }
}

/// Capa baixada da web para a faixa (se a correção dela trouxe uma).
fn capa_web_de(banco: &metadados::Banco, pasta: &PastaMusical, indice: usize) -> Option<PathBuf> {
    let correcao = banco.correcao(&pasta.chave_faixa(indice)?)?;
    metadados::capa_web(correcao.lancamento.as_deref()?)
}

fn item_painel(
    pasta: &PastaMusical,
    indice: usize,
    tocando: Option<usize>,
    bib: &Biblioteca,
    banco: &metadados::Banco,
) -> Option<painel::Item> {
    let t = pasta.tracks.get(indice)?;
    Some(painel::Item {
        caminho: pasta.caminho.join(&t.path),
        indice,
        titulo: t.titulo.clone(),
        artista: t.artista.clone(),
        album: t.album.clone(),
        duracao: t.duracao,
        tocando: tocando == Some(indice),
        favorita: pasta
            .chave_faixa(indice)
            .is_some_and(|k| bib.e_favorita(&k)),
        capa_web: capa_web_de(banco, pasta, indice),
    })
}

/// Quantas faixas a fila "a seguir" mostra.
const TAMANHO_FILA: usize = 5;

/// Republica a fila "a seguir" do painel (se aberto).
fn atualizar_fila(estado: &mut EstadoAudio) {
    let aberto = estado.imersivo.as_ref().is_some_and(|i| i.painel.is_some());
    if !aberto {
        return;
    }
    let itens = fila_a_seguir(estado, TAMANHO_FILA);
    if let Some(painel) = estado.imersivo.as_mut().and_then(|i| i.painel.as_mut()) {
        painel.publicar_fila(itens);
    }
}

/// As próximas `n` como itens do painel. A engatilhada (gapless) já saiu da
/// fila do shuffle, mas é a primeira a tocar.
fn fila_a_seguir(estado: &mut EstadoAudio, n: usize) -> Vec<painel::Item> {
    // Em ordem nada é consumido: a engatilhada já é a primeira de `proximas`
    let engatilhada = estado.engatilhada.as_ref().map(|e| e.indice);
    let consumida = estado.modo_shuffle != 0 || engatilhada == estado.indice_atual;
    let mut indices: Vec<usize> = engatilhada.filter(|_| consumida).into_iter().collect();
    let resto = proximas(estado, n.saturating_sub(indices.len()));
    indices.extend(resto);
    match estado.pasta_reproducao.and_then(|p| estado.pastas.get(p)) {
        Some(pasta) => indices
            .iter()
            .filter_map(|&i| item_painel(pasta, i, None, &estado.biblioteca, &estado.metadados))
            .collect(),
        None => Vec::new(),
    }
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

/// Fonte pronta para o player: decodificador com ganho/transição e espião
/// do visualizador.
type Fonte = visualizador::Espiao<transicao::Transicao<Decoder<BufReader<File>>>>;

/// Caminho completo da faixa atual, sempre na pasta de onde ela vem (nunca
/// na aba visível).
fn caminho_atual(estado: &EstadoAudio) -> Option<PathBuf> {
    let pasta = estado.pastas.get(estado.pasta_reproducao?)?;
    Some(pasta.caminho.join(estado.arquivo_atual.as_deref()?))
}

/// Fator do ReplayGain de uma faixa (1 com a opção desligada ou sem tag).
fn ganho_de(estado: &EstadoAudio, pasta: usize, indice: Option<usize>) -> f32 {
    if !estado.replaygain {
        return 1.0;
    }
    indice
        .and_then(|i| estado.pastas.get(pasta)?.tracks.get(i)?.ganho)
        .unwrap_or(1.0)
}

/// Abre a faixa e devolve a fonte, a duração (s) e o controle do fade-out.
/// `entrada` é o fade-in (crossfade); zero entra direto.
fn abrir_fonte(
    estado: &EstadoAudio,
    caminho: &Path,
    ganho: f32,
    entrada: Duration,
) -> Option<(Fonte, u64, transicao::Controle)> {
    let arquivo = File::open(caminho).ok()?;
    let byte_len = arquivo.metadata().ok()?.len();
    let extensao = caminho
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let decodificador = criar_decodificador(BufReader::new(arquivo), &extensao, byte_len).ok()?;

    // Seek na mesma faixa: a duração já é conhecida
    let total_secs = if estado.duracao_de.as_deref() == Some(caminho) {
        estado.duracao_total_secs
    } else if extensao == "mp3" {
        mp3_duration::from_path(caminho)
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

    let controle = transicao::Controle::default();
    let fonte = visualizador::Espiao::new(
        transicao::Transicao::new(decodificador, ganho, entrada, controle.clone()),
        estado.captura.clone(),
    );
    Some((fonte, total_secs, controle))
}

/// Mixer da saída de áudio, abrindo o dispositivo se preciso.
fn mixer(estado: &mut EstadoAudio) -> Option<&rodio::mixer::Mixer> {
    if estado.saida.is_none() {
        estado.saida = DeviceSinkBuilder::open_default_sink().ok();
    }
    estado.saida.as_ref().map(|s| s.mixer())
}

/// Recria o player da faixa atual a partir de `alvo_secs`, tocando ou já
/// pausado (pausa antes do primeiro som: sem estalo no seek pausado).
fn executar_seek(estado: &mut EstadoAudio, ui: &MainWindow, alvo_secs: u64, pausado: bool) {
    let Some(caminho_completo) = caminho_atual(estado) else {
        return;
    };

    if let Some(antigo) = estado.audio_player.take() {
        antigo.stop();
    }
    if let Some(saindo) = estado.saindo.take() {
        saindo.stop();
    }
    // A próxima que estava na fila do player volta para a fila do shuffle
    if let Some(e) = estado.engatilhada.take() {
        devolver_a_fila(estado, e.indice);
    }
    estado.transicao_feita = false;

    let ganho = ganho_de(
        estado,
        estado.pasta_reproducao.unwrap_or(0),
        estado.indice_atual,
    );
    let Some((fonte, total_secs, controle)) =
        abrir_fonte(estado, &caminho_completo, ganho, Duration::ZERO)
    else {
        return;
    };
    let volume = estado.volume_atual;
    let Some(mixer) = mixer(estado) else {
        return;
    };
    let sink = Player::connect_new(mixer);
    sink.set_volume(volume);
    if pausado {
        sink.pause();
    }
    sink.append(fonte);
    if alvo_secs > 0 {
        let _ = sink.try_seek(Duration::from_secs(alvo_secs));
    }

    estado.duracao_total_secs = total_secs;
    estado.duracao_de = Some(caminho_completo);
    estado.controle = controle;
    estado.tempo_decorrido = alvo_secs as f64;
    estado.ultimo_segundo = alvo_secs;
    estado.audio_player = Some(sink);

    publicar_tempo(estado, ui);
    publicar_play_pause(estado, ui, !pausado);
}

/// Barra e texto de tempo da faixa atual.
fn publicar_tempo(estado: &EstadoAudio, ui: &MainWindow) {
    let total = estado.duracao_total_secs;
    let atual = estado.tempo_decorrido as u64;
    ui.set_progresso(if total > 0 {
        atual as f32 / total as f32
    } else {
        0.0
    });
    let com_horas = total >= 3600;
    ui.set_texto_tempo(
        format!(
            "{} / {}",
            formatar_tempo(atual, com_horas),
            formatar_tempo(total, com_horas)
        )
        .into(),
    );
}

fn publicar_play_pause(estado: &EstadoAudio, ui: &MainWindow, tocando: bool) {
    let t = translations::get_translations(&estado.idioma);
    let texto = if tocando {
        texto_pause(&t)
    } else {
        texto_play(&t)
    };
    ui.set_texto_play_pause(texto.into());
    ui.set_tocando(tocando);
}

/// Passa o estado do app para a faixa `track_idx` da pasta `pasta_idx`: letra,
/// seleção, origem do som. Não mexe no player.
fn marcar_faixa(
    estado: &mut EstadoAudio,
    ui: &MainWindow,
    pasta_idx: usize,
    track_idx: usize,
    alvo_secs: u64,
) -> bool {
    garantir_pasta_carregada(estado, pasta_idx);

    let Some(pasta) = estado.pastas.get(pasta_idx) else {
        return false;
    };
    let Some(caminho) = pasta.tracks.get(track_idx).map(|t| t.path.clone()) else {
        return false;
    };

    // Troca de faixa: descarta a letra anterior e tenta carregar a nova
    let completo = pasta.caminho.join(&caminho);
    carregar_letra(estado, ui, &completo, alvo_secs as f64);

    if estado.pasta_reproducao != Some(pasta_idx) {
        // Pasta de reprodução mudou: a sacola do shuffle inteligente era
        // da pasta antiga, não serve mais aqui.
        estado.sacola_shuffle.clear();
        estado.sorteio.clear();
    }

    let anterior = estado.pasta_reproducao.zip(estado.indice_atual);
    if !estado.voltando
        && anterior != Some((pasta_idx, track_idx))
        && let Some((p, i)) = anterior
        && let Some(chave) = estado.pastas.get(p).and_then(|pasta| pasta.chave_faixa(i))
    {
        estado.historico.push((p, chave));
        if estado.historico.len() > TAMANHO_HISTORICO {
            estado.historico.remove(0);
        }
    }

    estado.pasta_reproducao = Some(pasta_idx);
    estado.indice_atual = Some(track_idx);
    estado.arquivo_atual = Some(caminho);
    estado.contada = false;

    atualizar_selecao_visivel(estado, ui);
    atualizar_favorita_atual(estado, ui);
    // Sem letra nenhuma: pede à web (se a opção estiver ligada)
    if estado.letra_atual.is_empty() && estado.letras_web {
        pedir_letra(estado, false);
    }
    true
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
    if !marcar_faixa(estado, ui, pasta_idx, track_idx, alvo_secs) {
        return;
    }
    executar_seek(estado, ui, alvo_secs, false);
    atualizar_imersivo(estado, ui);
}

fn tocar_faixa(estado: &mut EstadoAudio, ui: &MainWindow, pasta_idx: usize, track_idx: usize) {
    iniciar_faixa(estado, ui, pasta_idx, track_idx, 0);
}

/// A próxima faixa já começou no player (gapless, ou o crossfade entrou): o
/// estado passa a ser dela sem recriar nada.
fn assumir_faixa(
    estado: &mut EstadoAudio,
    ui: &MainWindow,
    pasta_idx: usize,
    track_idx: usize,
    duracao: u64,
) {
    if !marcar_faixa(estado, ui, pasta_idx, track_idx, 0) {
        return;
    }
    estado.duracao_total_secs = duracao;
    estado.duracao_de = caminho_atual(estado);
    estado.tempo_decorrido = 0.0;
    estado.ultimo_segundo = 0;
    estado.transicao_feita = false;
    publicar_tempo(estado, ui);
    atualizar_imersivo(estado, ui);
    atualizar_fila(estado);
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

/// Reposiciona a faixa mantendo o estado de pausa.
fn seek_mantendo_pausa(estado: &mut EstadoAudio, ui: &MainWindow, alvo_secs: u64) {
    let Some(estava_pausado) = estado.audio_player.as_ref().map(|sink| sink.is_paused()) else {
        // Sem faixa carregada não há o que reposicionar
        return;
    };
    executar_seek(estado, ui, alvo_secs, estava_pausado);
}

/// Seek vindo de fora (barra, MPRIS): com player, preserva a pausa; parado
/// com faixa (Stop ou recém-aberto), carrega e toca dali.
fn seek_pedido(estado: &mut EstadoAudio, ui: &MainWindow, alvo_secs: u64) {
    match efeito_seek(
        estado.audio_player.is_some(),
        estado.arquivo_atual.is_some(),
    ) {
        EfeitoSeek::ManterEstado => seek_mantendo_pausa(estado, ui, alvo_secs),
        EfeitoSeek::Recarregar => executar_seek(estado, ui, alvo_secs, false),
        EfeitoSeek::Nada => {}
    }
}

/// A próxima já estava na fila do player e a fila mudou (loop, shuffle,
/// sono): tira ela de lá recriando o player no mesmo ponto.
fn desengatilhar(estado: &mut EstadoAudio, ui: &MainWindow) {
    if estado.engatilhada.is_none() {
        return;
    }
    let pos = estado
        .audio_player
        .as_ref()
        .map_or(0, |p| p.get_pos().as_secs());
    seek_mantendo_pausa(estado, ui, pos);
    atualizar_fila(estado);
}

/// Faixa tirada da fila do shuffle que acabou não tocando volta para a frente.
fn devolver_a_fila(estado: &mut EstadoAudio, indice: usize) {
    if estado.indice_atual == Some(indice) {
        return;
    }
    match estado.modo_shuffle {
        2 => estado.sacola_shuffle.push(indice),
        1 => estado.sorteio.insert(0, indice),
        _ => {}
    }
}

/// Quantas faixas o histórico do "anterior" guarda.
const TAMANHO_HISTORICO: usize = 200;

/// Shuffle: a faixa que tocou antes da atual, tirada do histórico. A atual
/// volta para a frente da fila, para "próxima" voltar a ela. `None` com o
/// histórico vazio (ou só com faixas que sumiram).
fn voltar_no_historico(estado: &mut EstadoAudio) -> Option<(usize, usize)> {
    while let Some((p, chave)) = estado.historico.pop() {
        let Some(pasta) = estado.pastas.get(p) else {
            continue;
        };
        let Some(i) = (0..pasta.tracks.len())
            .find(|&i| pasta.chave_faixa(i).as_deref() == Some(chave.as_str()))
        else {
            continue;
        };
        // A engatilhada volta primeiro, para a atual ficar na frente dela
        if let Some(e) = estado.engatilhada.take() {
            devolver_a_fila(estado, e.indice);
        }
        if estado.pasta_reproducao == Some(p)
            && let Some(atual) = estado.indice_atual
        {
            match estado.modo_shuffle {
                2 => estado.sacola_shuffle.push(atual),
                1 => estado.sorteio.insert(0, atual),
                _ => {}
            }
        }
        return Some((p, i));
    }
    None
}

fn tocar_anterior(estado: &mut EstadoAudio, ui: &MainWindow) {
    // Shuffle: volta ao que tocou de verdade
    if estado.modo_shuffle != 0
        && let Some((p, i)) = voltar_no_historico(estado)
    {
        estado.voltando = true;
        tocar_faixa(estado, ui, p, i);
        estado.voltando = false;
        return;
    }
    // Em ordem: a vizinha na pasta de onde vem a faixa atual, nunca na aba
    // visível
    let Some((pasta_idx, ordem)) = ordem_reproducao(estado) else {
        return;
    };
    let pos = estado
        .indice_atual
        .and_then(|idx| ordem.iter().position(|&i| i == idx));
    let track_idx = match pos {
        Some(p) if p > 0 => ordem[p - 1],
        // Na primeira, ou fora do filtro: vai para a última
        _ => ordem[ordem.len() - 1],
    };
    tocar_faixa(estado, ui, pasta_idx, track_idx);
}

fn alternar_play_pause(estado: &mut EstadoAudio, ui: &MainWindow) {
    match &estado.audio_player {
        Some(sink) if sink.is_paused() => {
            sink.play();
            publicar_play_pause(estado, ui, true);
        }
        Some(_) => pausar(estado, ui),
        None => retomar(estado, ui),
    }
}

fn pausar(estado: &mut EstadoAudio, ui: &MainWindow) {
    if let Some(ref sink) = estado.audio_player
        && !sink.is_paused()
    {
        sink.pause();
        publicar_play_pause(estado, ui, false);
    }
    // A anterior do crossfade não fica tocando sozinha
    if let Some(saindo) = estado.saindo.take() {
        saindo.stop();
    }
}

/// Play: continua o player pausado, ou carrega a faixa que ficou selecionada
/// (Stop, ou o app recém-aberto) a partir de onde ela estava.
fn retomar(estado: &mut EstadoAudio, ui: &MainWindow) {
    match &estado.audio_player {
        Some(sink) if sink.is_paused() => {
            sink.play();
            publicar_play_pause(estado, ui, true);
        }
        Some(_) => {}
        None if estado.arquivo_atual.is_some() => {
            let alvo = estado.tempo_decorrido as u64;
            executar_seek(estado, ui, alvo, false);
            atualizar_imersivo(estado, ui);
        }
        None => {}
    }
}

fn stop_music(estado: &mut EstadoAudio, ui: &MainWindow) {
    estado.audio_player = None;
    estado.saindo = None;
    if let Some(e) = estado.engatilhada.take() {
        devolver_a_fila(estado, e.indice);
    }
    // Parado não segura o dispositivo: sem stream aberto, sem thread de áudio
    estado.saida = None;
    estado.tempo_decorrido = 0.0;
    estado.ultimo_segundo = 0;
    ui.set_progresso(0.0);
    publicar_play_pause(estado, ui, false);
    ui.set_texto_tempo("00:00 / 00:00".into());

    // Nada tocando: some com a letra da faixa anterior
    estado.letra_atual.clear();
    estado.letra_livre = false;
    ui.set_letra_livre(false);
    estado.letra_indice = -1;
    publicar_letra(estado, ui);
}

/// Pasta que está tocando e sua ordem de navegação (com o filtro, quando ele
/// é dessa pasta). Navegar é sempre na pasta da faixa atual, nunca na aba
/// visível.
fn ordem_reproducao(estado: &EstadoAudio) -> Option<(usize, Vec<usize>)> {
    let pasta_idx = estado.pasta_reproducao?;
    let filtro = filtro_efetivo(estado.aba_visivel, estado.pasta_reproducao, &estado.filtro);
    let ordem = ordem_lista(estado, pasta_idx, filtro);
    (!ordem.is_empty()).then_some((pasta_idx, ordem))
}

/// Shuffle inteligente: uma "sacola" embaralhada é consumida até esvaziar e
/// só então outra é feita. Garante que todas as faixas tocam uma vez antes de
/// qualquer repetição.
fn encher_sacola(estado: &mut EstadoAudio, ordem: &[usize]) {
    let mut rng = rand::thread_rng();
    let mut nova_sacola = ordem.to_vec();
    nova_sacola.shuffle(&mut rng);

    // Evita repetição colada na emenda entre um ciclo e o outro: a próxima a
    // sair é `nova_sacola.last()` (o pop consome do fim) — se for igual à
    // última tocada, troca de lugar com outra posição.
    if nova_sacola.len() > 1 && nova_sacola.last().copied() == estado.indice_atual {
        let ultimo = nova_sacola.len() - 1;
        nova_sacola.swap(0, ultimo);
    }

    estado.sacola_shuffle = nova_sacola;
}

/// As próximas `n` faixas (índices na pasta que toca), sem tocar nenhuma. O
/// sorteio do shuffle acontece aqui, adiantado, e `escolher_proxima` consome
/// a mesma fila: o que aparece em "a seguir" é o que toca.
fn proximas(estado: &mut EstadoAudio, n: usize) -> Vec<usize> {
    let Some((_, ordem)) = ordem_reproducao(estado) else {
        return Vec::new();
    };
    match estado.modo_shuffle {
        2 => {
            if estado.sacola_shuffle.is_empty() {
                encher_sacola(estado, &ordem);
            }
            estado
                .sacola_shuffle
                .iter()
                .rev()
                .take(n)
                .copied()
                .collect()
        }
        1 => {
            let mut rng = rand::thread_rng();
            while estado.sorteio.len() < n {
                match ordem.choose(&mut rng) {
                    Some(&i) => estado.sorteio.push(i),
                    None => break,
                }
            }
            estado.sorteio.iter().take(n).copied().collect()
        }
        _ => {
            let mut pos = estado
                .indice_atual
                .and_then(|idx| ordem.iter().position(|&i| i == idx));
            let mut saida = Vec::new();
            for _ in 0..n.min(ordem.len()) {
                let proxima = match pos {
                    Some(p) if p + 1 < ordem.len() => p + 1,
                    Some(_) if estado.modo_loop == 2 => 0,
                    Some(_) => break,
                    None => 0,
                };
                saida.push(ordem[proxima]);
                pos = Some(proxima);
            }
            saida
        }
    }
}

/// Tira da fila a próxima a tocar: (pasta, faixa). `None` no fim da pasta
/// (em ordem, sem loop) ou sem lista.
fn escolher_proxima(estado: &mut EstadoAudio) -> Option<(usize, usize)> {
    let (pasta_idx, ordem) = ordem_reproducao(estado)?;
    let alvo = match estado.modo_shuffle {
        2 => {
            if estado.sacola_shuffle.is_empty() {
                encher_sacola(estado, &ordem);
            }
            estado.sacola_shuffle.pop()
        }
        // Aleatório de verdade: sorteia entre todas as faixas, sem excluir
        // nada (pode repetir, inclusive de seguida — é assim que aleatório de
        // verdade se comporta). Quem quiser evitar repetição é o Shuffle
        // Inteligente. Se a fila "a seguir" já sorteou, vale o sorteio dela.
        1 if !estado.sorteio.is_empty() => Some(estado.sorteio.remove(0)),
        1 => ordem.choose(&mut rand::thread_rng()).copied(),
        // Em ordem: `proximas` já sabe parar no fim da pasta
        _ => proximas(estado, 1).first().copied(),
    };
    alvo.map(|i| (pasta_idx, i))
}

fn tocar_proxima_manual(estado: &mut EstadoAudio, ui: &MainWindow) {
    // Já na fila do player: é ela a próxima
    if let Some(e) = estado.engatilhada.take() {
        tocar_faixa(estado, ui, e.pasta, e.indice);
        return;
    }
    if ordem_reproducao(estado).is_none() {
        return;
    }
    match escolher_proxima(estado) {
        Some((pasta_idx, track_idx)) => tocar_faixa(estado, ui, pasta_idx, track_idx),
        None => stop_music(estado, ui), // fim da pasta: para
    }
}

fn proxima_musica_auto(estado: &mut EstadoAudio, ui: &MainWindow) {
    if estado.sono == Some(sono::Sono::FimDaFaixa) {
        // Hora de dormir: já deixa a próxima engatilhada, mas pausada
        estado.sono = None;
        tocar_proxima_manual(estado, ui);
        pausar(estado, ui);
        return;
    }
    if estado.modo_loop == 1 {
        executar_seek(estado, ui, 0, false);
    } else {
        tocar_proxima_manual(estado, ui);
    }
}

/// Antecedência com que a próxima entra na fila do player (gapless). Folga
/// para o timer de 250 ms e para abrir o arquivo.
const ANTECEDENCIA_GAPLESS: f64 = 2.0;

/// Perto do fim da faixa: põe a próxima na fila do player (gapless) ou começa
/// o crossfade. Faixas seguidas do mesmo álbum, em ordem, emendam sem
/// crossfade: disco ao vivo e álbum conceitual não podem ter a emenda mexida.
fn preparar_transicao(estado: &mut EstadoAudio, ui: &MainWindow) {
    if estado.transicao_feita
        || estado.engatilhada.is_some()
        || estado.sono == Some(sono::Sono::FimDaFaixa)
    {
        return;
    }
    let Some(sink) = &estado.audio_player else {
        return;
    };
    let total = estado.duracao_total_secs as f64;
    if sink.is_paused() || total <= 0.0 {
        return;
    }
    let restante = total - sink.get_pos().as_secs_f64();
    let crossfade = estado.crossfade as f64;
    if restante > crossfade.max(ANTECEDENCIA_GAPLESS) {
        return;
    }
    estado.transicao_feita = true;

    let proxima = if estado.modo_loop == 1 {
        estado.pasta_reproducao.zip(estado.indice_atual)
    } else {
        escolher_proxima(estado)
    };
    let Some((pasta_idx, indice)) = proxima else {
        return;
    };
    let Some(caminho) = estado
        .pastas
        .get(pasta_idx)
        .and_then(|p| p.caminho_faixa(indice))
    else {
        return;
    };

    let mesmo_album = {
        let album = |i: Option<usize>| {
            let p = estado.pastas.get(pasta_idx)?;
            p.tracks.get(i?)?.album.clone()
        };
        estado.modo_shuffle == 0
            && album(estado.indice_atual).is_some()
            && album(estado.indice_atual) == album(Some(indice))
    };
    let cruzar = crossfade > 0.0 && estado.modo_loop != 1 && !mesmo_album;
    let entrada = if cruzar {
        Duration::from_secs_f64(restante.max(0.5))
    } else {
        Duration::ZERO
    };
    let ganho = ganho_de(estado, pasta_idx, Some(indice));
    let Some((fonte, duracao, controle)) = abrir_fonte(estado, &caminho, ganho, entrada) else {
        devolver_a_fila(estado, indice);
        return;
    };

    if !cruzar {
        if let Some(sink) = &estado.audio_player {
            sink.append(fonte);
        }
        estado.engatilhada = Some(Engatilhada {
            pasta: pasta_idx,
            indice,
            duracao,
            controle,
        });
        atualizar_fila(estado);
        return;
    }

    // Crossfade: a atual some no que falta dela, a próxima entra num player
    // novo no mesmo tempo
    estado.controle.esmaecer(entrada);
    let volume = estado.volume_atual;
    let Some(mixer) = mixer(estado) else {
        return;
    };
    let novo = Player::connect_new(mixer);
    novo.set_volume(volume);
    novo.append(fonte);
    estado.saindo = estado.audio_player.replace(novo);
    estado.controle = controle;
    assumir_faixa(estado, ui, pasta_idx, indice, duracao);
}

/// Restaura a faixa e a posição salvas, pausada e sem abrir o dispositivo de
/// áudio: o Play é que carrega (`retomar`).
fn restaurar_posicao(estado: &mut EstadoAudio, ui: &MainWindow, config: &AppConfig) {
    let Some(pasta_idx) = estado.pasta_reproducao else {
        return;
    };
    let Some(pasta) = estado.pastas.get(pasta_idx) else {
        return;
    };
    let indice = match &config.faixa_salva {
        Some(chave) => {
            (0..pasta.tracks.len()).find(|&i| pasta.chave_faixa(i).as_ref() == Some(chave))
        }
        // Config antiga: posição na lista
        None => config.indice_atual.filter(|&i| i < pasta.tracks.len()),
    };
    let Some(indice) = indice else {
        estado.pasta_reproducao = None;
        return;
    };
    let tempo = config.tempo_atual.unwrap_or(0);
    let duracao = pasta.tracks[indice].duracao as u64;

    // A fila do shuffle que ia tocar
    let posicoes: HashMap<String, usize> = (0..pasta.tracks.len())
        .filter_map(|i| Some((pasta.chave_faixa(i)?, i)))
        .collect();
    let fila: Vec<usize> = config
        .fila_shuffle
        .iter()
        .filter_map(|k| posicoes.get(k).copied())
        .collect();

    if !marcar_faixa(estado, ui, pasta_idx, indice, tempo) {
        return;
    }
    match estado.modo_shuffle {
        2 => estado.sacola_shuffle = fila.into_iter().rev().collect(),
        1 => estado.sorteio = fila,
        _ => {}
    }
    estado.tempo_decorrido = tempo as f64;
    estado.duracao_total_secs = duracao;
    publicar_tempo(estado, ui);
    publicar_play_pause(estado, ui, false);
}

fn atualizar_progresso(estado: &mut EstadoAudio, ui: &MainWindow) {
    // A anterior do crossfade terminou de sumir
    if estado.saindo.as_ref().is_some_and(|s| s.empty()) {
        estado.saindo = None;
    }

    // Gapless: a engatilhada começou (a anterior saiu da fila do player)
    if estado.engatilhada.is_some()
        && estado.audio_player.as_ref().is_some_and(|s| s.len() <= 1)
        && let Some(e) = estado.engatilhada.take()
    {
        estado.controle = e.controle;
        assumir_faixa(estado, ui, e.pasta, e.indice, e.duracao);
    }

    let total_secs = estado.duracao_total_secs;
    let terminou = match estado.audio_player {
        Some(ref sink) => sink.empty() && total_secs > 0,
        None => false,
    };
    if terminou {
        proxima_musica_auto(estado, ui);
        return;
    }

    let Some(sink) = &estado.audio_player else {
        return;
    };
    let pausado = sink.is_paused();
    if !pausado {
        estado.tempo_decorrido = sink.get_pos().as_secs_f64();
    }

    // Letra: recalcula a linha atual (também quando pausado, após um seek)
    sincronizar_letra(estado, ui);

    if pausado {
        return;
    }

    if !estado.contada && biblioteca::conta_como_ouvida(estado.tempo_decorrido, total_secs) {
        contar_reproducao(estado, ui);
    }
    preparar_transicao(estado, ui);

    let atual_secs = estado.tempo_decorrido as u64;
    if atual_secs != estado.ultimo_segundo {
        estado.ultimo_segundo = atual_secs;
        publicar_tempo(estado, ui);
    }
}

/// Mais uma reprodução da faixa atual. A lista "Mais tocadas" é remontada
/// quando estiver à vista.
fn contar_reproducao(estado: &mut EstadoAudio, ui: &MainWindow) {
    estado.contada = true;
    let Some(chave) = estado
        .pasta_reproducao
        .and_then(|p| estado.pastas.get(p))
        .zip(estado.arquivo_atual.as_deref())
        .map(|(p, nome)| chave_faixa(&p.caminho, nome))
    else {
        return;
    };
    estado.biblioteca.contar(&chave);
    estado.biblioteca.salvar();
    renovar_especial(estado, ui, POS_MAIS_TOCADAS);
}

/// Marca ou desmarca uma faixa como favorita e atualiza o que mostra isso.
fn alternar_favorita(estado: &mut EstadoAudio, ui: &MainWindow, chave: &str) {
    estado.biblioteca.alternar_favorita(chave);
    estado.biblioteca.salvar();
    renovar_especial(estado, ui, POS_FAVORITAS);
    // A estrela das linhas (a lista especial já remontou a sua)
    if estado.aba_visivel != POS_FAVORITAS {
        atualizar_lista(estado, ui);
    }
    atualizar_favorita_atual(estado, ui);
    atualizar_fila(estado);
}

/// Estrela da faixa que toca (modo imersivo e atalho F).
fn atualizar_favorita_atual(estado: &EstadoAudio, ui: &MainWindow) {
    let favorita = chave_atual(estado).is_some_and(|k| estado.biblioteca.e_favorita(&k));
    ui.set_favorita_atual(favorita);
}

// ---------------------------------------------------------------------
// Busca na web (letras, metadados, capas)
// ---------------------------------------------------------------------

/// Manda um pedido ao buscador, criando-o se for o primeiro.
fn pedir_web(estado: &mut EstadoAudio, pedido: buscador::Pedido) {
    let chave = Some(estado.chave_acoustid.trim().to_string()).filter(|c| !c.is_empty());
    estado
        .buscador
        .get_or_insert_with(|| buscador::Buscador::novo(chave))
        .pedir(pedido);
}

fn consulta_de(
    estado: &EstadoAudio,
    pasta_idx: usize,
    indice: usize,
) -> Option<metadados::Consulta> {
    let pasta = estado.pastas.get(pasta_idx)?;
    let t = pasta.tracks.get(indice)?;
    Some(metadados::Consulta {
        chave: chave_faixa(&pasta.caminho, &t.path),
        caminho: pasta.caminho.join(&t.path),
        titulo: t.titulo.clone(),
        artista: t.artista.clone(),
        album: t.album.clone(),
        duracao: t.duracao,
    })
}

/// Faixa (lista, posição) com esta chave, de preferência a que toca.
fn achar_faixa(estado: &EstadoAudio, chave: &str) -> Option<(usize, usize)> {
    if chave_atual(estado).as_deref() == Some(chave) {
        return estado.pasta_reproducao.zip(estado.indice_atual);
    }
    estado.pastas.iter().enumerate().find_map(|(p, pasta)| {
        let i =
            (0..pasta.tracks.len()).find(|&i| pasta.chave_faixa(i).as_deref() == Some(chave))?;
        Some((p, i))
    })
}

/// Letra da faixa atual na web. `forcar`: mesmo que ela já tenha falhado
/// antes (botão da tela de informações).
fn pedir_letra(estado: &mut EstadoAudio, forcar: bool) {
    let Some(consulta) = estado
        .pasta_reproducao
        .zip(estado.indice_atual)
        .and_then(|(p, i)| consulta_de(estado, p, i))
    else {
        return;
    };
    if forcar {
        estado.metadados.esquecer_sem_letra(&consulta.chave);
    } else if !estado.metadados.buscar_letra(&consulta.chave) {
        return;
    }
    pedir_web(estado, buscador::Pedido::Letra(consulta));
}

/// Põe a correção no banco e nas listas carregadas, e pede a capa.
fn aplicar_correcao(estado: &mut EstadoAudio, chave: &str, correcao: metadados::Correcao) {
    for pasta in &mut estado.pastas {
        let raiz = pasta.caminho.clone();
        for t in &mut pasta.tracks {
            if chave_faixa(&raiz, &t.path) == chave {
                correcao.aplicar(t);
            }
        }
    }
    if let Some(lancamento) = correcao.lancamento.clone() {
        pedir_web(
            estado,
            buscador::Pedido::Capa {
                chave: chave.to_string(),
                lancamento,
            },
        );
    }
    estado.metadados.corrigir(chave, correcao);
}

/// Depois de mexer nos metadados: lista, painel e tela cheia de novo.
fn renovar_vistas(estado: &mut EstadoAudio, ui: &MainWindow) {
    atualizar_lista(estado, ui);
    let faixa = faixa_imersiva(estado);
    if let Some(imv) = estado.imersivo.as_mut() {
        imv.atualizar_dados(ui, faixa);
    }
    atualizar_fila(estado);
    publicar_info(estado, ui);
}

/// Volta a faixa ao que as tags dizem.
fn restaurar_original(estado: &mut EstadoAudio, chave: &str) {
    if !estado.metadados.restaurar(chave) {
        return;
    }
    for pasta in &mut estado.pastas {
        let raiz = pasta.caminho.clone();
        for t in &mut pasta.tracks {
            if chave_faixa(&raiz, &t.path) == chave {
                *t = track::ler_faixa(&raiz.join(&t.path), t.path.clone());
            }
        }
    }
}

/// Preenche a tela de informações com a faixa `info_chave`.
fn publicar_info(estado: &EstadoAudio, ui: &MainWindow) {
    let lang = translations::get_translations(&estado.idioma);
    let t = |k: &str| lang.get(k).copied().unwrap_or("").to_string();
    let Some(chave) = estado.info_chave.as_deref() else {
        return;
    };
    let Some((p, i)) = achar_faixa(estado, chave) else {
        return;
    };
    let pasta = &estado.pastas[p];
    let faixa = &pasta.tracks[i];
    ui.set_info_titulo(faixa.titulo.as_str().into());
    ui.set_info_artista(faixa.artista.as_deref().unwrap_or("").into());
    let correcao = estado.metadados.correcao(chave);
    let mut album = faixa.album.clone().unwrap_or_default();
    if let Some(ano) = correcao.and_then(|c| c.ano) {
        album.push_str(&format!(" ({ano})"));
    }
    ui.set_info_album(album.into());
    ui.set_info_corrigida(correcao.is_some());
    let caminho = pasta.caminho.join(&faixa.path);
    let letra = if letras_web::tem_letra_local(&caminho, chave) {
        t("info_lyrics_ok")
    } else if !estado.metadados.buscar_letra(chave) {
        t("info_lyrics_missing_web")
    } else {
        t("info_lyrics_none")
    };
    ui.set_info_letra(letra.into());
    let pelo_som = t("info_by_sound");
    let linhas: Vec<CandidatoInfo> = estado
        .info_candidatos
        .iter()
        .map(|c| linha_candidato(c, &pelo_som))
        .collect();
    ui.set_info_candidatos(ModelRc::new(VecModel::from(linhas)));
}

/// Revisão do lote e resumo na tela.
fn publicar_lote(estado: &EstadoAudio, ui: &MainWindow) {
    let lang = translations::get_translations(&estado.idioma);
    let t = |k: &str| lang.get(k).copied().unwrap_or("").to_string();
    let l = &estado.lote;
    ui.set_lote_ativo(l.ativo);
    ui.set_lote_progresso(if l.total > 0 {
        l.feitas as f32 / l.total as f32
    } else {
        0.0
    });
    let texto = if l.ativo {
        format!("{} {}/{}", t("batch_running"), l.feitas, l.total)
    } else if l.total > 0 {
        t("batch_summary")
            .replace("{auto}", &l.automaticas.to_string())
            .replace("{revisar}", &estado.revisao.len().to_string())
            .replace("{nada}", &l.sem_resultado.to_string())
            .replace("{letras}", &l.letras.to_string())
    } else {
        String::new()
    };
    ui.set_lote_texto(texto.into());
    let itens: Vec<ItemRevisao> = estado
        .revisao
        .iter()
        .map(|(consulta, c)| {
            let original = match &consulta.artista {
                Some(a) => format!("{a} - {}", consulta.titulo),
                None => consulta.titulo.clone(),
            };
            let mut sugestao = format!("{} - {}", c.artista, c.titulo);
            if let Some(album) = &c.album {
                sugestao.push_str(&format!(" · {album}"));
            }
            ItemRevisao {
                original: original.into(),
                sugestao: sugestao.into(),
                confianca: c.confianca as i32,
            }
        })
        .collect();
    ui.set_revisao(ModelRc::new(VecModel::from(itens)));
}

/// Respostas do buscador (timer de 250 ms).
fn processar_web(estado: &mut EstadoAudio, ui: &MainWindow) {
    let Some(b) = estado.buscador.as_mut() else {
        return;
    };
    let respostas = b.respostas();
    let lang = translations::get_translations(&estado.idioma);
    let t = |k: &str| lang.get(k).copied().unwrap_or("").to_string();
    let mut renovar = false;
    for resposta in respostas {
        match resposta {
            buscador::Resposta::Letra { chave, resultado } => {
                match resultado {
                    Ok(letras_web::Resultado::Salva { .. }) => {
                        estado.metadados.esquecer_sem_letra(&chave);
                        if chave_atual(estado).as_deref() == Some(chave.as_str())
                            && let Some(caminho) = caminho_atual(estado)
                        {
                            let tempo = estado.tempo_decorrido;
                            carregar_letra(estado, ui, &caminho, tempo);
                        }
                    }
                    Ok(_) => estado.metadados.marcar_sem_letra(&chave),
                    Err(e) => {
                        if estado.info_chave.as_deref() == Some(chave.as_str()) {
                            ui.set_info_estado(format!("{}: {e}", t("info_error")).into());
                        }
                    }
                }
                estado.metadados.salvar();
                if estado.info_chave.as_deref() == Some(chave.as_str()) {
                    ui.set_info_ocupado(false);
                    publicar_info(estado, ui);
                }
            }
            buscador::Resposta::Candidatos { chave, lista, erro } => {
                if estado.info_chave.as_deref() != Some(chave.as_str()) {
                    continue;
                }
                let texto = match (&erro, lista.len()) {
                    (Some(e), 0) => format!("{}: {e}", t("info_error")),
                    (_, 0) => t("info_nothing"),
                    (_, n) => t("info_results").replace("{n}", &n.to_string()),
                };
                estado.info_candidatos = lista;
                ui.set_info_estado(texto.into());
                ui.set_info_ocupado(false);
                publicar_info(estado, ui);
            }
            buscador::Resposta::LoteProgresso { feitas, total } => {
                estado.lote.feitas = feitas;
                estado.lote.total = total;
                publicar_lote(estado, ui);
            }
            buscador::Resposta::LoteItem {
                consulta,
                candidatos,
                letra,
            } => {
                match candidatos.first() {
                    Some(c) if c.confianca >= metadados::CONFIANCA_AUTOMATICA => {
                        aplicar_correcao(estado, &consulta.chave, metadados::Correcao::from(c));
                        estado.lote.automaticas += 1;
                        renovar = true;
                    }
                    Some(c) => estado.revisao.push((consulta.clone(), c.clone())),
                    None => estado.lote.sem_resultado += 1,
                }
                match letra {
                    Some(letras_web::Resultado::Salva { .. }) => estado.lote.letras += 1,
                    Some(_) => estado.metadados.marcar_sem_letra(&consulta.chave),
                    None => {}
                }
                estado.lote.feitas += 1;
            }
            buscador::Resposta::LoteFim { cancelado, erro } => {
                estado.lote.ativo = false;
                estado.metadados.salvar();
                renovar = true;
                if let Some(e) = erro.filter(|_| !cancelado && estado.lote.automaticas == 0) {
                    ui.set_info_estado(format!("{}: {e}", t("info_error")).into());
                }
                // A letra da faixa atual pode ter chegado no lote
                if let Some(caminho) = caminho_atual(estado)
                    && estado.letra_atual.is_empty()
                {
                    let tempo = estado.tempo_decorrido;
                    carregar_letra(estado, ui, &caminho, tempo);
                }
            }
            buscador::Resposta::Capa { chave, ok } => {
                if ok {
                    // A miniatura "sem capa" dessa faixa não vale mais
                    let caminho = achar_faixa(estado, &chave)
                        .and_then(|(l, i)| estado.pastas[l].caminho_faixa(i));
                    if let Some(p) = estado.imersivo.as_mut().and_then(|i| i.painel.as_mut())
                        && let Some(c) = caminho
                    {
                        p.esquecer(&c);
                    }
                    renovar = true;
                }
            }
            buscador::Resposta::Gravadas { ok, falhas } => {
                let mut texto = t("tags_written").replace("{n}", &ok.to_string());
                if let Some(f) = falhas.first() {
                    texto.push_str(&format!(" · {}: {f}", t("info_error")));
                }
                ui.set_tags_texto(texto.into());
            }
        }
    }
    if renovar {
        renovar_vistas(estado, ui);
        atualizar_painel(estado, ui);
    }
    publicar_lote(estado, ui);
    if estado.buscador.as_ref().is_some_and(|b| b.dispensavel()) {
        // Sem trabalho há um minuto: solta o cliente HTTP e a thread
        estado.buscador = None;
        Timer::single_shot(Duration::from_millis(500), memoria::aparar);
    }
}

fn chave_atual(estado: &EstadoAudio) -> Option<String> {
    let pasta = estado.pastas.get(estado.pasta_reproducao?)?;
    Some(chave_faixa(
        &pasta.caminho,
        estado.arquivo_atual.as_deref()?,
    ))
}

/// Temporizador de sono: esmaece o volume no fim e pausa. Roda no timer de
/// 250 ms, com ou sem o modo imersivo.
fn verificar_sono(estado: &mut EstadoAudio, ui: &MainWindow) {
    let agora = Instant::now();
    if let Some(sono::Sono::Ate { fim, .. }) = estado.sono {
        let fator = sono::fator_volume(fim, agora);
        if fator <= 0.0 {
            estado.sono = None;
            pausar(estado, ui);
            aplicar_volume(estado, estado.volume_atual);
        } else if fator < 1.0 {
            aplicar_volume(estado, estado.volume_atual * fator);
        }
    }

    let lang = translations::get_translations(&estado.idioma);
    let fim_da_faixa = lang.get("sleep_end").copied().unwrap_or("sleep_end");
    let texto = sono::texto(estado.sono, agora, fim_da_faixa);
    if ui.get_sono_texto() != texto.as_str() {
        ui.set_sono_texto(texto.into());
    }
}

/// Volume nos players (o do crossfade incluído). `volume_atual` é o do slider;
/// o sono passa um valor menor no esmaecimento.
fn aplicar_volume(estado: &EstadoAudio, volume: f32) {
    for p in estado.audio_player.iter().chain(&estado.saindo) {
        p.set_volume(volume);
    }
}

/// Prévia "a seguir" do modo imersivo nos últimos segundos da faixa.
const PREVIA_SEGUNDOS: f64 = 10.0;

fn atualizar_previa(estado: &mut EstadoAudio, ui: &MainWindow) {
    let restante = estado.duracao_total_secs as f64 - estado.tempo_decorrido;
    let tocando = estado.audio_player.as_ref().is_some_and(|p| !p.is_paused());
    let mostrar = tocando
        && estado.duracao_total_secs > 0
        && restante <= PREVIA_SEGUNDOS
        && estado.modo_loop != 1
        && estado.sono != Some(sono::Sono::FimDaFaixa);
    let texto = if mostrar {
        fila_a_seguir(estado, 1)
            .first()
            .map(|i| match &i.artista {
                Some(a) => format!("{} — {a}", i.titulo),
                None => i.titulo.clone(),
            })
            .unwrap_or_default()
    } else {
        String::new()
    };
    if ui.get_imv_proxima() != texto.as_str() {
        ui.set_imv_proxima(texto.into());
    }
}

/// Relógio do modo imersivo: só mexe na UI quando o minuto vira.
fn atualizar_relogio(ui: &MainWindow) {
    let agora = chrono::Local::now().format("%H:%M").to_string();
    if ui.get_imv_relogio() != agora.as_str() {
        ui.set_imv_relogio(agora.into());
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
    // A glibc cria uma arena de malloc por thread (áudio, D-Bus, update...), e
    // a memória livre de cada uma não volta para o SO. Duas bastam aqui.
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    // SAFETY: só ajusta um parâmetro do alocador, antes de qualquer thread
    unsafe {
        libc::mallopt(libc::M_ARENA_MAX, 2);
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
                alternar_play_pause(&mut estado.borrow_mut(), &ui);
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
                ui.set_loop_modo(e.modo_loop as i32);
                desengatilhar(&mut e, &ui);
                atualizar_fila(&mut e);
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
                // Desligado → inteligente → aleatório → desligado
                e.modo_shuffle = match e.modo_shuffle {
                    0 => 2,
                    2 => 1,
                    _ => 0,
                };
                // A engatilhada foi escolhida no modo antigo
                desengatilhar(&mut e, &ui);
                // Sacola da pasta antiga não serve pro modo novo.
                e.sacola_shuffle.clear();
                e.sorteio.clear();
                let lang = translations::get_translations(&e.idioma);
                ui.set_texto_shuffle(texto_shuffle(e.modo_shuffle, &lang).into());
                ui.set_shuffle_modo(e.modo_shuffle as i32);
                atualizar_fila(&mut e);
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

    {
        let estado = estado.clone();
        ui.on_volume_mudou(move |v| {
            let mut e = estado.borrow_mut();
            e.volume_atual = v;
            aplicar_volume(&e, v);
            // Sem salvar aqui: o slider dispara a cada pixel arrastado, e o
            // save ao fechar a janela já persiste o volume.
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_progresso_mudou(move |v| {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                let alvo = (v as f64 * e.duracao_total_secs as f64) as u64;
                // Arrastar a barra com a faixa pausada não pode dar play
                seek_pedido(&mut e, &ui, alvo);
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
                if e.letra_livre {
                    return;
                }
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

    // Modo imersivo: o timer de ~30 fps só roda enquanto o modo está ligado
    let timer_imersivo = Rc::new(Timer::default());
    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        let timer_imersivo = timer_imersivo.clone();
        ui.on_alternar_imersivo(move || {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();

            if e.imersivo.take().is_some() {
                // Sair: o Drop do estado derruba threads e imagens pendentes
                timer_imersivo.stop();
                e.captura.ligar(false);
                imersivo::limpar_ui(&ui);
                ui.set_modo_imersivo(false);
                ui.window().set_fullscreen(false);
                // Depois que a janela repintar sem as imagens
                Timer::single_shot(Duration::from_millis(500), memoria::aparar);
                return;
            }

            e.imersivo = Some(imersivo::EstadoImersivo::novo(&ui));
            e.captura.ligar(true);
            ui.set_modo_imersivo(true);
            ui.window().set_fullscreen(true);
            atualizar_imersivo(&mut e, &ui);
            atualizar_relogio(&ui);

            let estado = estado.clone();
            let ui_fraca = ui.as_weak();
            timer_imersivo.start(TimerMode::Repeated, Duration::from_millis(33), move || {
                let Some(ui) = ui_fraca.upgrade() else {
                    return;
                };
                let mut e = estado.borrow_mut();
                let e = &mut *e;
                let tocando = e.audio_player.as_ref().is_some_and(|p| !p.is_paused());
                if let Some(imv) = e.imersivo.as_mut() {
                    imv.tick(&ui, &e.idioma, &e.captura, tocando, e.fundo_reativo);
                }
            });
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_escolher_fundo(move || {
            let extensoes = fundo::extensoes_aceitas();
            // Vários de uma vez; somam aos que já estavam (rodízio)
            let Some(caminhos) = rfd::FileDialog::new()
                .add_filter("Imagem, GIF/WebP ou vídeo", &extensoes)
                .pick_files()
            else {
                return;
            };
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                for c in caminhos {
                    if !e.fundos_globais.contains(&c) {
                        e.fundos_globais.push(c);
                    }
                }
                ui.set_fundo_global_nome(nome_fundo(&e.fundos_globais, &e.idioma).into());
                salvar_configuracao(&e);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_limpar_fundo(move || {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                e.fundos_globais.clear();
                ui.set_fundo_global_nome(SharedString::new());
                salvar_configuracao(&e);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_letra_completa_clicada(move |indice| {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                if e.letra_livre {
                    return;
                }
                let Some((tempo, _)) = usize::try_from(indice)
                    .ok()
                    .and_then(|i| e.letra_atual.get(i))
                else {
                    return;
                };
                let alvo = tempo.max(0.0) as u64;
                seek_mantendo_pausa(&mut e, &ui, alvo);
                sincronizar_letra(&mut e, &ui);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_imv_painel_alternado(move |aberto| {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let Some(imv) = e.imersivo.as_mut() else {
                return;
            };
            if aberto {
                imv.painel = Some(painel::Painel::novo(&ui));
                atualizar_painel(&mut e, &ui);
                atualizar_fila(&mut e);
            } else {
                // Fechado não segura miniatura nem thread
                imv.painel = None;
                painel::limpar_ui(&ui);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_imv_painel_rolou(move |primeira, quantas| {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let mudou = e
                .imersivo
                .as_mut()
                .and_then(|i| i.painel.as_mut())
                .is_some_and(|p| p.rolou(primeira.max(0) as usize, quantas.max(0) as usize));
            if mudou {
                atualizar_painel(&mut e, &ui);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_imv_fila_clicada(move |posicao| {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let Some(indice) = e
                .imersivo
                .as_ref()
                .and_then(|i| i.painel.as_ref())
                .and_then(|p| p.indice_da_fila(posicao.max(0) as usize))
            else {
                return;
            };
            let Some(pasta_idx) = e.pasta_reproducao else {
                return;
            };
            // Clicou em outra que não a engatilhada: ela volta para a fila
            if let Some(eng) = e.engatilhada.take()
                && eng.indice != indice
            {
                devolver_a_fila(&mut e, eng.indice);
            }
            // Pular para uma faixa da fila consome a fila até ela
            match e.modo_shuffle {
                2 => e.sacola_shuffle.retain(|&i| i != indice),
                1 => {
                    if let Some(pos) = e.sorteio.iter().position(|&i| i == indice) {
                        e.sorteio.drain(..=pos);
                    }
                }
                _ => {}
            }
            tocar_faixa(&mut e, &ui, pasta_idx, indice);
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_alternar_sono(move || {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            e.sono = sono::proximo(e.sono, Instant::now());
            // Trocar ou desligar no meio do esmaecimento devolve o volume
            aplicar_volume(&e, e.volume_atual);
            // "Fim da faixa" pausa na próxima: ela não pode já estar na fila
            if e.sono == Some(sono::Sono::FimDaFaixa) {
                desengatilhar(&mut e, &ui);
            }
            verificar_sono(&mut e, &ui);
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_ordem_escolhida(move |indice| {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let aba = e.aba_visivel;
            let Some(pasta) = e.pastas.get_mut(aba) else {
                return;
            };
            pasta.ordem = Ordem::de_indice(indice.max(0) as usize);
            // A ordem nova é a da navegação: a fila em ordem muda, a do
            // shuffle não (sorteio não depende de ordem)
            atualizar_lista(&mut e, &ui);
            salvar_configuracao(&e);
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_favorita_alternada(move |posicao| {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let Some(chave) = e
                .indices_visiveis
                .get(posicao.max(0) as usize)
                .and_then(|&i| e.pastas.get(e.aba_visivel)?.chave_faixa(i))
            else {
                return;
            };
            alternar_favorita(&mut e, &ui, &chave);
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_alternar_favorita_atual(move || {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            if let Some(chave) = chave_atual(&e) {
                alternar_favorita(&mut e, &ui, &chave);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_alternar_crossfade(move || {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            e.crossfade = proximo_crossfade(e.crossfade);
            publicar_crossfade(&e, &ui);
            salvar_configuracao(&e);
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_alternar_replaygain(move |ligado| {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            e.replaygain = ligado;
            // O ganho mora na fonte: refaz a faixa atual no mesmo ponto
            let pos = e.audio_player.as_ref().map(|p| p.get_pos().as_secs());
            if let Some(pos) = pos {
                seek_mantendo_pausa(&mut e, &ui, pos);
            }
            salvar_configuracao(&e);
        });
    }

    {
        let estado = estado.clone();
        ui.on_alternar_fundo_reativo(move |ligado| {
            let mut e = estado.borrow_mut();
            e.fundo_reativo = ligado;
            salvar_configuracao(&e);
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_imv_guia_mudou(move |guia| {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            if let Some(p) = e.imersivo.as_mut().and_then(|i| i.painel.as_mut()) {
                p.mostrar_albuns(guia == painel::GUIA_ALBUNS);
            }
            atualizar_painel(&mut e, &ui);
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_imv_albuns_rolou(move |primeira, quantas| {
            if ui_fraca.upgrade().is_none() {
                return;
            }
            let mut e = estado.borrow_mut();
            if let Some(p) = e.imersivo.as_mut().and_then(|i| i.painel.as_mut()) {
                p.albuns_rolou(primeira.max(0) as usize, quantas.max(0) as usize);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_imv_album_clicado(move |posicao| {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let Some(grupo) = e
                .imersivo
                .as_ref()
                .and_then(|i| i.painel.as_ref())
                .and_then(|p| p.album(posicao.max(0) as usize))
                .cloned()
            else {
                return;
            };
            abrir_album(&mut e, &ui, &grupo);
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_abrir_info(move || {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let Some(chave) = chave_atual(&e) else {
                return;
            };
            if e.info_chave.as_deref() != Some(chave.as_str()) {
                e.info_candidatos.clear();
                ui.set_info_estado(SharedString::new());
            }
            e.info_chave = Some(chave);
            ui.set_tags_texto(SharedString::new());
            publicar_info(&e, &ui);
            publicar_lote(&e, &ui);
            ui.set_info_aberta(true);
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_info_buscar(move || {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let Some(consulta) = e
                .info_chave
                .clone()
                .and_then(|k| achar_faixa(&e, &k))
                .and_then(|(p, i)| consulta_de(&e, p, i))
            else {
                return;
            };
            let lang = translations::get_translations(&e.idioma);
            ui.set_info_estado(lang.get("info_searching").copied().unwrap_or("").into());
            ui.set_info_ocupado(true);
            e.info_candidatos.clear();
            publicar_info(&e, &ui);
            pedir_web(&mut e, buscador::Pedido::Identificar(consulta));
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_info_aplicar(move |posicao, gravar| {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let (Some(chave), Some(c)) = (
                e.info_chave.clone(),
                e.info_candidatos.get(posicao.max(0) as usize).cloned(),
            ) else {
                return;
            };
            let correcao = metadados::Correcao::from(&c);
            aplicar_correcao(&mut e, &chave, correcao.clone());
            e.metadados.salvar();
            if gravar
                && let Some((p, i)) = achar_faixa(&e, &chave)
                && let Some(caminho) = e.pastas[p].caminho_faixa(i)
            {
                pedir_web(
                    &mut e,
                    buscador::Pedido::GravarTags(vec![(caminho, correcao)]),
                );
            }
            e.info_candidatos.clear();
            ui.set_info_estado(SharedString::new());
            renovar_vistas(&mut e, &ui);
            atualizar_painel(&mut e, &ui);
            // Com os nomes certos, a letra pode aparecer
            if e.letra_atual.is_empty() && chave_atual(&e).as_deref() == Some(chave.as_str()) {
                pedir_letra(&mut e, true);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_info_restaurar(move || {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            if let Some(chave) = e.info_chave.clone() {
                restaurar_original(&mut e, &chave);
                e.metadados.salvar();
                renovar_vistas(&mut e, &ui);
                atualizar_painel(&mut e, &ui);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_info_baixar_letra(move || {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            // A tela é sempre da faixa atual
            if e.info_chave.is_some() && e.info_chave == chave_atual(&e) {
                let lang = translations::get_translations(&e.idioma);
                ui.set_info_estado(lang.get("info_searching").copied().unwrap_or("").into());
                ui.set_info_ocupado(true);
                pedir_letra(&mut e, true);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_lote_iniciar(move || {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let aba = e.aba_visivel;
            let n = e.pastas.get(aba).map_or(0, |p| p.tracks.len());
            // Quem já tem correção fica de fora
            let consultas: Vec<metadados::Consulta> = (0..n)
                .filter_map(|i| consulta_de(&e, aba, i))
                .filter(|c| e.metadados.correcao(&c.chave).is_none())
                .collect();
            e.revisao.clear();
            e.lote = Lote {
                ativo: !consultas.is_empty(),
                total: consultas.len(),
                ..Default::default()
            };
            if !consultas.is_empty() {
                pedir_web(
                    &mut e,
                    buscador::Pedido::Lote {
                        consultas,
                        letras: true,
                    },
                );
            }
            publicar_lote(&e, &ui);
        });
    }

    {
        let estado = estado.clone();
        ui.on_lote_cancelar(move || {
            if let Some(b) = estado.borrow().buscador.as_ref() {
                b.cancelar_lote();
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_revisao_decidir(move |posicao, aceitar| {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let todas = posicao < 0;
            let escolhidas: Vec<(metadados::Consulta, metadados::Candidato)> = if todas {
                std::mem::take(&mut e.revisao)
            } else if (posicao as usize) < e.revisao.len() {
                vec![e.revisao.remove(posicao as usize)]
            } else {
                return;
            };
            if aceitar {
                for (consulta, c) in escolhidas {
                    aplicar_correcao(&mut e, &consulta.chave, metadados::Correcao::from(&c));
                }
                e.metadados.salvar();
                renovar_vistas(&mut e, &ui);
                atualizar_painel(&mut e, &ui);
            }
            publicar_lote(&e, &ui);
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_gravar_tags_aba(move || {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let aba = e.aba_visivel;
            let atual = caminho_atual(&e);
            let Some(pasta) = e.pastas.get(aba) else {
                return;
            };
            // A faixa tocando fica de fora: o decodificador está lendo o
            // arquivo, e reescrever as tags no começo dele estragaria o som
            let itens: Vec<(PathBuf, metadados::Correcao)> = (0..pasta.tracks.len())
                .filter_map(|i| {
                    let caminho = pasta.caminho_faixa(i)?;
                    let correcao = e.metadados.correcao(&pasta.chave_faixa(i)?)?;
                    (Some(&caminho) != atual.as_ref()).then(|| (caminho, correcao.clone()))
                })
                .collect();
            let lang = translations::get_translations(&e.idioma);
            if itens.is_empty() {
                ui.set_tags_texto(lang.get("tags_none").copied().unwrap_or("").into());
                return;
            }
            ui.set_tags_texto(lang.get("tags_writing").copied().unwrap_or("").into());
            pedir_web(&mut e, buscador::Pedido::GravarTags(itens));
        });
    }

    {
        let estado = estado.clone();
        ui.on_alternar_letras_web(move |ligado| {
            let mut e = estado.borrow_mut();
            e.letras_web = ligado;
            salvar_configuracao(&e);
        });
    }

    {
        let estado = estado.clone();
        ui.on_chave_acoustid_mudou(move |chave| {
            let mut e = estado.borrow_mut();
            e.chave_acoustid = chave.trim().to_string();
            // O buscador leva a chave de quando nasceu
            if e.buscador.as_ref().is_some_and(|b| b.dispensavel_ja()) {
                e.buscador = None;
            }
            salvar_configuracao(&e);
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

    // Checagem de update em segundo plano. A thread entrega o resultado
    // direto no event loop (`upgrade_in_event_loop`), sem timer de polling.
    {
        let ui_fraca = ui.as_weak();
        std::thread::spawn(move || {
            let atualizacao = updates::check_for_update();
            // A checagem desserializa a lista inteira de releases e sobe um
            // runtime HTTP: código e heap que não voltam a ser usados.
            memoria::aparar();
            if let Some(info) = atualizacao {
                let _ = ui_fraca.upgrade_in_event_loop(move |ui| {
                    ui.set_update_versao(info.version.into());
                    ui.set_update_descricao(info.body.into());
                    ui.set_update_disponivel(true);
                });
            }
        });
    }

    {
        let ui_fraca = ui.as_weak();
        ui.on_baixar_atualizacao(move || {
            if let Some(ui) = ui_fraca.upgrade() {
                ui.set_atualizando(true);
                let ui_fraca = ui.as_weak();
                std::thread::spawn(move || {
                    // O erro vira texto: `Box<dyn Error>` não é `Send`
                    let resultado = updates::perform_update().map_err(|e| e.to_string());
                    // `upgrade()` só funciona na thread da UI: fora dela
                    // devolve sempre `None` e o "baixando" nunca sumia.
                    let _ = ui_fraca.upgrade_in_event_loop(move |ui| {
                        ui.set_atualizando(false);
                        match resultado {
                            Ok(()) => {
                                ui.set_update_disponivel(false);
                                println!("Update complete! Please restart Furinar.");
                            }
                            Err(e) => eprintln!("Update failed: {e}"),
                        }
                    });
                });
            }
        });
    }

    // Passado o startup (restauração da faixa, primeiro paint), boa parte do
    // código e das fontes mapeados não é mais tocada.
    Timer::single_shot(Duration::from_secs(5), memoria::aparar);

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

    fn amostras_nao_silenciosas(decodificador: Decoder<BufReader<File>>) {
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

    /// O fallback de `aplicar_idioma` é a própria chave: uma chave que não
    /// existe nas traduções aparece crua na UI. Lê o próprio fonte para pegar
    /// toda chave pedida via `g("...")`.
    #[test]
    fn aplicar_idioma_so_usa_chaves_existentes() {
        let fonte = include_str!("main.rs");
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

    /// Lista em memória com `n` faixas ("/m/0.mp3"...).
    fn estado_com_faixas(n: usize) -> EstadoAudio {
        let mut e = EstadoAudio::default();
        let mut pasta = PastaMusical::pasta(PathBuf::from("/m"));
        pasta.tracks = (0..n)
            .map(|i| TrackInfo {
                path: format!("{i}.mp3"),
                titulo: i.to_string(),
                ..Default::default()
            })
            .collect();
        pasta.carregada = true;
        e.pastas.push(pasta);
        e.pasta_reproducao = Some(0);
        e
    }

    #[test]
    fn anterior_no_shuffle_volta_pelo_historico() {
        let mut e = estado_com_faixas(10);
        e.modo_shuffle = 2;
        // Tocou 7, depois 2, e agora toca 5
        e.historico = vec![(0, "/m/7.mp3".into()), (0, "/m/2.mp3".into())];
        e.indice_atual = Some(5);
        e.sacola_shuffle = vec![9, 4];

        assert_eq!(voltar_no_historico(&mut e), Some((0, 2)));
        // A atual (5) é a próxima a sair da sacola (pop pelo fim)
        assert_eq!(e.sacola_shuffle.last(), Some(&5));
        e.indice_atual = Some(2);
        assert_eq!(voltar_no_historico(&mut e), Some((0, 7)));
        assert_eq!(voltar_no_historico(&mut e), None);
    }

    #[test]
    fn anterior_no_aleatorio_poe_a_atual_na_frente_do_sorteio() {
        let mut e = estado_com_faixas(10);
        e.modo_shuffle = 1;
        e.historico = vec![(0, "/m/3.mp3".into()), (0, "/m/sumiu.mp3".into())];
        e.indice_atual = Some(8);
        e.sorteio = vec![1, 6];
        // A que sumiu da pasta é pulada
        assert_eq!(voltar_no_historico(&mut e), Some((0, 3)));
        assert_eq!(e.sorteio, [8, 1, 6]);
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

    /// Renderiza o modo normal num PNG (`target/captura-normal.png`), com as
    /// faixas da pasta de `FURINAR_CAPTURA_FAIXA`. Manual, como a do imersivo.
    #[test]
    #[ignore = "gera PNG para inspeção visual"]
    fn captura_modo_normal() {
        use slint::platform::software_renderer::{
            MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
        };
        use slint::platform::{Platform, WindowAdapter};

        struct Plataforma(Rc<MinimalSoftwareWindow>);
        impl Platform for Plataforma {
            fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
                Ok(self.0.clone())
            }
        }

        let (largura, altura) = (660u32, 734u32);
        let janela = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        slint::platform::set_platform(Box::new(Plataforma(janela.clone())))
            .expect("plataforma de teste");
        janela.set_size(slint::PhysicalSize::new(largura, altura));
        let ui = MainWindow::new().expect("MainWindow");
        ui.show().expect("show");
        aplicar_idioma("pt-br", &ui);

        let caminho = std::env::var_os("FURINAR_CAPTURA_FAIXA")
            .map(PathBuf::from)
            .unwrap_or_else(|| fixture("tone.mp3"));
        let raiz = caminho.parent().map(PathBuf::from).unwrap_or_default();
        let mut e = EstadoAudio::default();
        e.pastas
            .push(PastaMusical::especial(Tipo::Favoritas, Ordem::Padrao));
        e.pastas
            .push(PastaMusical::especial(Tipo::MaisTocadas, Ordem::Padrao));
        e.pastas.push(PastaMusical::pasta(raiz));
        e.aba_visivel = ESPECIAIS;
        garantir_pasta_carregada(&mut e, ESPECIAIS);
        e.pastas[ESPECIAIS].ordem = Ordem::Album;
        for i in [1, 3] {
            if let Some(k) = e.pastas[ESPECIAIS].chave_faixa(i) {
                e.biblioteca.alternar_favorita(&k);
            }
        }
        e.pasta_reproducao = Some(ESPECIAIS);
        e.arquivo_atual = e.pastas[ESPECIAIS].tracks.get(2).map(|t| t.path.clone());
        e.indice_atual = Some(2);
        atualizar_abas(&e, &ui);
        atualizar_lista(&mut e, &ui);
        ui.set_sono_texto("14:59".into());
        ui.set_texto_tempo("01:14 / 03:31".into());
        ui.set_progresso(0.35);
        publicar_crossfade(&e, &ui);
        // FURINAR_CAPTURA_LOOP / _SHUFFLE: modo dos botões (0, 1, 2)
        let modo = |v: &str| {
            std::env::var(v)
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0)
        };
        ui.set_loop_modo(modo("FURINAR_CAPTURA_LOOP"));
        ui.set_shuffle_modo(modo("FURINAR_CAPTURA_SHUFFLE"));
        // FURINAR_CAPTURA_CONFIG=1: configurações abertas
        if std::env::var_os("FURINAR_CAPTURA_CONFIG").is_some() {
            ui.set_config_aberta(true);
        }
        // FURINAR_CAPTURA_INFO=1 (ou =2 para a guia do lote): informações
        if let Some(guia) = std::env::var_os("FURINAR_CAPTURA_INFO") {
            let candidato = |titulo: &str, confianca, fonte| metadados::Candidato {
                gravacao: "x".into(),
                titulo: titulo.into(),
                artista: "Queen".into(),
                album: Some("A Night at the Opera".into()),
                lancamento: None,
                ano: Some(1975),
                numero: 11,
                duracao: 355,
                confianca,
                fonte,
            };
            e.info_candidatos = vec![
                candidato("Bohemian Rhapsody", 100, metadados::Fonte::Texto),
                candidato("Bohemian Rhapsody (Live)", 83, metadados::Fonte::Texto),
                candidato("Bohemian Rhapsody", 61, metadados::Fonte::ImpressaoDigital),
            ];
            e.info_chave = e.pastas[ESPECIAIS].chave_faixa(2);
            publicar_info(&e, &ui);
            ui.set_info_estado("3 resultados — escolha um".into());
            if guia == "2" {
                let consulta = consulta_de(&e, ESPECIAIS, 2).expect("consulta");
                e.revisao = vec![
                    (
                        consulta.clone(),
                        candidato("Bohemian Rhapsody", 76, metadados::Fonte::Texto),
                    ),
                    (
                        consulta,
                        candidato("Awake Now", 62, metadados::Fonte::Texto),
                    ),
                ];
                e.lote = Lote {
                    ativo: true,
                    feitas: 34,
                    total: 148,
                    ..Default::default()
                };
                publicar_lote(&e, &ui);
                ui.set_info_guia(1);
            }
            ui.set_info_aberta(true);
        }

        for _ in 0..10 {
            slint::platform::update_timers_and_animations();
        }
        let mut pixels = vec![PremultipliedRgbaColor::default(); (largura * altura) as usize];
        janela.request_redraw();
        janela.draw_if_needed(|r| {
            r.render(&mut pixels, largura as usize);
        });
        let bytes: Vec<u8> = pixels
            .iter()
            .flat_map(|p| [p.red, p.green, p.blue, 255])
            .collect();
        let saida = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/captura-normal.png");
        image::RgbaImage::from_raw(largura, altura, bytes)
            .expect("buffer")
            .save(&saida)
            .expect("salvar png");
        println!("captura em {}", saida.display());
    }

    /// Renderiza o modo imersivo num PNG, sem abrir janela, para inspeção
    /// visual. Manual:
    ///
    /// `FURINAR_CAPTURA_FAIXA=/musica.mp3 cargo test captura_modo_imersivo -- --ignored`
    ///
    /// A imagem sai em `target/captura-imersivo.png`.
    #[test]
    #[ignore = "gera PNG para inspeção visual"]
    fn captura_modo_imersivo() {
        use slint::platform::software_renderer::{
            MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
        };
        use slint::platform::{Platform, WindowAdapter};

        struct Plataforma(Rc<MinimalSoftwareWindow>);
        impl Platform for Plataforma {
            fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
                Ok(self.0.clone())
            }
        }

        let (largura, altura) = (1600u32, 900u32);
        let janela = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        slint::platform::set_platform(Box::new(Plataforma(janela.clone())))
            .expect("plataforma de teste");
        janela.set_size(slint::PhysicalSize::new(largura, altura));

        let ui = MainWindow::new().expect("MainWindow");
        ui.show().expect("show");
        aplicar_idioma("pt-br", &ui);

        let caminho = std::env::var_os("FURINAR_CAPTURA_FAIXA")
            .map(PathBuf::from)
            .unwrap_or_else(|| fixture("tone.mp3"));
        let mut imv = imersivo::EstadoImersivo::novo(&ui);
        // Espectro de mentira: um acorde, para as barras aparecerem
        let captura = visualizador::Captura::default();
        captura.ligar(true);
        let acorde = rodio::source::SineWave::new(220.0)
            .mix(rodio::source::SineWave::new(1760.0).amplify(0.3))
            .take_duration(Duration::from_millis(100));
        let _: Vec<f32> = visualizador::Espiao::new(acorde, captura.clone()).collect();
        ui.set_modo_imersivo(true);
        imv.trocar_faixa(
            &ui,
            Some(imersivo::Faixa {
                titulo: track::ler_tags(&caminho).titulo,
                artista: track::ler_tags(&caminho).artista,
                pasta_raiz: caminho.parent().map(PathBuf::from).unwrap_or_default(),
                capa_web: None,
                album: None,
                ano: None,
                fundos_globais: std::env::var_os("FURINAR_CAPTURA_FUNDO")
                    .map(PathBuf::from)
                    .into_iter()
                    .collect(),
                caminho: caminho.clone(),
            }),
        );

        let letra = fs::read_to_string(caminho_lrc(&caminho))
            .map(|c| parse_lrc(&c))
            .unwrap_or_default();
        let linhas: Vec<SharedString> = letra.iter().map(|(_, t)| t.as_str().into()).collect();
        ui.set_letra_indice(if linhas.is_empty() {
            -1
        } else {
            4.min(linhas.len() as i32 - 1)
        });
        ui.set_letra_completa(ModelRc::new(VecModel::from(linhas)));
        ui.set_progresso(0.35);
        ui.set_texto_tempo("01:14 / 03:31".into());
        ui.set_imv_relogio("21:47".into());
        ui.set_sono_texto("29:41".into());
        ui.set_favorita_atual(true);
        ui.set_imv_proxima("Próxima faixa — Artista".into());
        // FURINAR_CAPTURA_CONFIG=1: configurações abertas
        if std::env::var_os("FURINAR_CAPTURA_CONFIG").is_some() {
            ui.set_crossfade_texto("Crossfade: 6 s".into());
            ui.set_fundo_global_nome("pixel-city.gif".into());
            ui.set_imv_config_aberta(true);
        }

        // FURINAR_CAPTURA_PAINEL=1: painel aberto com as faixas da pasta
        if std::env::var_os("FURINAR_CAPTURA_PAINEL").is_some() {
            let raiz = caminho.parent().map(PathBuf::from).unwrap_or_default();
            let mut tracks = Vec::new();
            coletar_arquivos_audio(&raiz, &raiz, false, &mut tracks);
            tracks.sort_by(|a, b| a.path.cmp(&b.path));
            let nome = caminho.strip_prefix(&raiz).ok().and_then(|n| n.to_str());
            let atual = tracks.iter().position(|t| Some(t.path.as_str()) == nome);
            let mut pasta = PastaMusical::pasta(raiz);
            pasta.tracks = tracks;
            pasta.carregada = true;
            let mut bib = Biblioteca::default();
            let sem_correcoes = metadados::Banco::default();
            if let Some(k) = atual.and_then(|i| pasta.chave_faixa(i + 1)) {
                bib.alternar_favorita(&k);
            }
            ui.set_abas(ModelRc::new(VecModel::from(vec![
                SharedString::from("Favoritas"),
                "Mais tocadas".into(),
                SharedString::from(pasta.nome.as_str()),
                "Outra pasta".into(),
            ])));
            ui.set_aba_ativa(2);
            ui.set_aba_tocando(2);
            ui.set_indice_selecionado(atual.map_or(-1, |i| i as i32));
            // Um quadro antes: a tela imersiva tem que existir para o
            // `changed painel_aberto` dela disparar e centralizar a lista
            janela.request_redraw();
            janela.draw_if_needed(|r| {
                let mut lixo = vec![PremultipliedRgbaColor::default(); (largura * altura) as usize];
                r.render(&mut lixo, largura as usize);
            });
            ui.set_imv_painel_aberto(true);
            let mut p = painel::Painel::novo(&ui);
            let inicio = atual.unwrap_or(0).saturating_sub(6);
            p.rolou(inicio, 17);
            p.publicar(&ui, pasta.tracks.len(), |pos| {
                item_painel(&pasta, pos, atual, &bib, &sem_correcoes)
            });
            let fila = (1..=5)
                .filter_map(|k| {
                    item_painel(&pasta, atual.unwrap_or(0) + k, None, &bib, &sem_correcoes)
                })
                .collect();
            p.publicar_fila(fila);
            // FURINAR_CAPTURA_ALBUNS=1: a guia da grade
            if std::env::var_os("FURINAR_CAPTURA_ALBUNS").is_some() {
                let todas: Vec<usize> = (0..pasta.tracks.len()).collect();
                p.mostrar_albuns(true);
                p.albuns_rolou(0, 12);
                p.publicar_albuns(
                    track::agrupar_albuns(&pasta.tracks, &todas),
                    &translations::pt_br(),
                    |i| Some((pasta.caminho_faixa(i)?, None)),
                );
                ui.set_imv_guia(painel::GUIA_ALBUNS);
            }
            imv.painel = Some(p);
        }

        // Espera a thread da capa e deixa as animações chegarem ao fim.
        // FURINAR_CAPTURA_OCIOSO=1: espera os controles sumirem (3 s)
        if std::env::var_os("FURINAR_CAPTURA_OCIOSO").is_some() {
            ui.set_imv_ocioso(true);
        }
        for _ in 0..40 {
            imv.tick(&ui, "pt-br", &captura, true, true);
            slint::platform::update_timers_and_animations();
            std::thread::sleep(Duration::from_millis(50));
        }

        let mut pixels = vec![PremultipliedRgbaColor::default(); (largura * altura) as usize];
        janela.request_redraw();
        janela.draw_if_needed(|r| {
            r.render(&mut pixels, largura as usize);
        });
        let bytes: Vec<u8> = pixels
            .iter()
            .flat_map(|p| [p.red, p.green, p.blue, 255])
            .collect();
        let saida = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/captura-imersivo.png");
        image::RgbaImage::from_raw(largura, altura, bytes)
            .expect("buffer")
            .save(&saida)
            .expect("salvar png");
        println!("captura em {}", saida.display());
    }
}
