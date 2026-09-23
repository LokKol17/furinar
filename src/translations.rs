use std::collections::HashMap;

pub type Translations = HashMap<&'static str, &'static str>;

pub fn pt_br() -> Translations {
    let mut t = Translations::new();
    t.insert("app_name", "Furinar");
    t.insert("tip_minimize", "Minimizar");
    t.insert("tip_close", "Fechar");
    t.insert("tip_open_folder", "Abrir pasta");
    t.insert("tip_previous", "Anterior");
    t.insert("tip_stop", "Parar");
    t.insert("tip_next", "Próxima");
    t.insert("tip_lyrics", "Letra");
    t.insert("tip_settings", "Configurações");
    t.insert("search_placeholder", "Buscar por título ou artista...");
    t.insert("no_lyrics", "Sem letra disponível para esta faixa.");
    t.insert("config_title", "Configurações");
    t.insert("theme_label_dark", "Tema: Escuro");
    t.insert("theme_label_light", "Tema: Claro");
    t.insert("scan_subfolders", "Escaneia subpastas");
    t.insert(
        "scan_subfolders_desc",
        "Inclui arquivos de áudio de subpastas ao abrir uma pasta.",
    );
    t.insert("update_title", "📦 Nova versão disponível!");
    t.insert("update_downloading", "Baixando...");
    t.insert("update_button", "Atualizar");
    t.insert("update_later", "Depois");
    t.insert("update_check", "Verificar atualizações");
    t.insert("update_checking", "Verificando...");
    t.insert("play", "Play");
    t.insert("pause", "Pausar");
    t.insert("loop_off", "Loop: Desl");
    t.insert("loop_track", "Loop: Faixa");
    t.insert("loop_list", "Loop: Lista");
    t.insert("shuffle_off", "Shuffle: Desl");
    t.insert("shuffle_on", "Shuffle: On");
    t.insert("shuffle_smart", "Shuffle: Inteligente");
    t
}

pub fn en() -> Translations {
    let mut t = Translations::new();
    t.insert("app_name", "Furinar");
    t.insert("tip_minimize", "Minimize");
    t.insert("tip_close", "Close");
    t.insert("tip_open_folder", "Open folder");
    t.insert("tip_previous", "Previous");
    t.insert("tip_stop", "Stop");
    t.insert("tip_next", "Next");
    t.insert("tip_lyrics", "Lyrics");
    t.insert("tip_settings", "Settings");
    t.insert("search_placeholder", "Search by title or artist...");
    t.insert("no_lyrics", "No lyrics available for this track.");
    t.insert("config_title", "Settings");
    t.insert("theme_label_dark", "Theme: Dark");
    t.insert("theme_label_light", "Theme: Light");
    t.insert("scan_subfolders", "Scan subfolders");
    t.insert(
        "scan_subfolders_desc",
        "Include audio files from subfolders when opening a folder.",
    );
    t.insert("update_title", "📦 Update available!");
    t.insert("update_downloading", "Downloading...");
    t.insert("update_button", "Update");
    t.insert("update_later", "Later");
    t.insert("update_check", "Check for updates");
    t.insert("update_checking", "Checking...");
    t.insert("play", "Play");
    t.insert("pause", "Pause");
    t.insert("loop_off", "Loop: Off");
    t.insert("loop_track", "Loop: Track");
    t.insert("loop_list", "Loop: List");
    t.insert("shuffle_off", "Shuffle: Off");
    t.insert("shuffle_on", "Shuffle: On");
    t.insert("shuffle_smart", "Shuffle: Smart");
    t
}

pub fn get_translations(lang: &str) -> Translations {
    match lang {
        "en" => en(),
        _ => pt_br(),
    }
}
