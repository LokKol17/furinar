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
    t.insert("tip_immersive", "Modo imersivo (F11)");
    t.insert("tip_exit_immersive", "Sair do modo imersivo (Esc)");
    t.insert("tip_library", "Biblioteca (L)");
    t.insert("library_title", "Biblioteca");
    t.insert("library_empty", "Nenhuma faixa aqui.");
    t.insert("up_next", "A seguir");
    t.insert("up_next_empty", "Nada na fila.");
    t.insert("tip_lyrics_only", "Só a letra (H)");
    t.insert("tip_sleep", "Temporizador de sono");
    t.insert("sleep_end", "Fim da faixa");
    t.insert("tip_info", "Informações da faixa (I)");
    t.insert("info_tab_track", "Faixa");
    t.insert("info_tab_batch", "Aba inteira");
    t.insert("info_corrected", "Corrigida pela web");
    t.insert("info_restore", "Restaurar original");
    t.insert("info_lyrics_label", "Letra");
    t.insert("info_get_lyrics", "Baixar letra");
    t.insert("info_search_web", "Buscar na web");
    t.insert("info_apply", "Aplicar");
    t.insert("info_apply_tags", "Aplicar e gravar nas tags");
    t.insert(
        "info_confirm_tags",
        "Isso altera o arquivo de música. Clique de novo para confirmar.",
    );
    t.insert("info_confirm_button", "Confirmar");
    t.insert("batch_desc", "Identifica todas as faixas desta aba no MusicBrainz e baixa as letras que faltam. Acertos com 90% ou mais entram sozinhos; o resto fica aqui para você revisar. Nada é gravado nos arquivos sem você pedir.");
    t.insert("batch_start", "Indexar esta aba");
    t.insert("batch_cancel", "Cancelar");
    t.insert("batch_review", "Para revisar");
    t.insert("batch_accept_all", "Aceitar todas");
    t.insert("tags_write_tab", "Gravar correções desta aba nas tags");
    t.insert("lyrics_web", "Buscar letras na internet");
    t.insert(
        "lyrics_web_desc",
        "Faixa sem letra: procura no LRCLIB ao tocar (envia título e artista).",
    );
    t.insert("acoustid_key", "Chave do AcoustID (opcional)");
    t.insert(
        "acoustid_desc",
        "Identifica pelo som faixas sem tag. Chave grátis em acoustid.org/new-application.",
    );
    t.insert("info_lyrics_ok", "tem");
    t.insert("info_lyrics_missing_web", "a web não tem");
    t.insert("info_lyrics_none", "nenhuma");
    t.insert("batch_running", "Indexando…");
    t.insert("batch_summary", "{auto} corrigidas sozinhas · {revisar} para revisar · {nada} sem resultado · {letras} letras baixadas");
    t.insert("info_error", "Erro");
    t.insert("info_by_sound", "identificada pelo som");
    t.insert("info_nothing", "Nada encontrado.");
    t.insert("info_results", "{n} resultados — escolha um");
    t.insert("info_searching", "Buscando…");
    t.insert("tags_written", "{n} arquivos atualizados");
    t.insert("tags_none", "Nenhuma correção nesta aba.");
    t.insert("tags_writing", "Gravando…");
    t.insert("tab_favorites", "Favoritas");
    t.insert("tab_most_played", "Mais tocadas");
    t.insert(
        "favorites_empty",
        "Nenhuma favorita ainda. Marque faixas com a estrela (ou F).",
    );
    t.insert(
        "most_played_empty",
        "As faixas que você mais ouve aparecem aqui.",
    );
    t.insert("tip_sort", "Ordenar");
    t.insert("tip_favorite", "Favoritar (F)");
    t.insert("sort_default", "Padrão");
    t.insert("sort_title", "Título");
    t.insert("sort_artist", "Artista");
    t.insert("sort_album", "Álbum");
    t.insert("sort_duration", "Duração");
    t.insert("sort_added", "Adicionadas recentemente");
    t.insert("sort_plays", "Mais tocadas");
    t.insert("albums", "Álbuns");
    t.insert("albums_empty", "Nenhum álbum aqui.");
    t.insert("album_track", "faixa");
    t.insert("album_tracks", "faixas");
    t.insert("various_artists", "Vários artistas");
    t.insert("no_album", "Sem álbum");
    t.insert("up_next_preview", "A seguir");
    t.insert("crossfade", "Crossfade");
    t.insert("crossfade_off", "Desligado");
    t.insert("replaygain", "Normalizar volume (ReplayGain)");
    t.insert(
        "replaygain_desc",
        "Usa o ganho gravado nas tags para as faixas tocarem no mesmo volume.",
    );
    t.insert("reactive_bg", "Fundo da tela cheia pulsa com a música");
    t.insert("stereo", "Estéreo");
    t.insert("mono", "Mono");
    t.insert("decimal_sep", ",");
    t.insert("bg_label", "Fundo da tela cheia");
    t.insert("bg_choose", "Adicionar…");
    t.insert("bg_count", "{n} fundos — trocam a cada faixa");
    t.insert("bg_clear", "Limpar");
    t.insert("bg_none", "Capa desfocada (padrão)");
    t.insert("search_placeholder", "Buscar por título ou artista...");
    t.insert("no_lyrics", "Sem letra disponível para esta faixa.");
    t.insert("config_title", "Configurações");
    t.insert("theme_dark", "Tema: Escuro");
    t.insert("theme_light", "Tema: Claro");
    t.insert("scan_subfolders", "Escaneia subpastas");
    t.insert(
        "scan_subfolders_desc",
        "Inclui arquivos de áudio de subpastas ao abrir uma pasta.",
    );
    t.insert("update_title", "📦 Nova versão disponível!");
    t.insert("update_downloading", "Baixando...");
    t.insert("update_button", "Atualizar");
    t.insert("update_later", "Depois");
    t.insert("play", "Play");
    t.insert("pause", "Pausar");
    t.insert("loop_off", "Loop: Desl");
    t.insert("loop_track", "Loop: Faixa");
    t.insert("loop_list", "Loop: Lista");
    t.insert("shuffle_off", "Shuffle: Desl");
    t.insert("shuffle_on", "Shuffle: Aleatório (pode repetir)");
    t.insert("shuffle_smart", "Shuffle: Inteligente (sem repetir)");
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
    t.insert("tip_immersive", "Immersive mode (F11)");
    t.insert("tip_exit_immersive", "Exit immersive mode (Esc)");
    t.insert("tip_library", "Library (L)");
    t.insert("library_title", "Library");
    t.insert("library_empty", "No tracks here.");
    t.insert("up_next", "Up next");
    t.insert("up_next_empty", "Nothing queued.");
    t.insert("tip_lyrics_only", "Lyrics only (H)");
    t.insert("tip_sleep", "Sleep timer");
    t.insert("sleep_end", "End of track");
    t.insert("tip_info", "Track info (I)");
    t.insert("info_tab_track", "Track");
    t.insert("info_tab_batch", "Whole tab");
    t.insert("info_corrected", "Fixed from the web");
    t.insert("info_restore", "Restore original");
    t.insert("info_lyrics_label", "Lyrics");
    t.insert("info_get_lyrics", "Get lyrics");
    t.insert("info_search_web", "Search the web");
    t.insert("info_apply", "Apply");
    t.insert("info_apply_tags", "Apply and write tags");
    t.insert(
        "info_confirm_tags",
        "This changes the music file. Click again to confirm.",
    );
    t.insert("info_confirm_button", "Confirm");
    t.insert("batch_desc", "Identifies every track in this tab on MusicBrainz and downloads missing lyrics. Matches of 90% or more are applied automatically; the rest waits here for review. Nothing is written to your files unless you ask.");
    t.insert("batch_start", "Index this tab");
    t.insert("batch_cancel", "Cancel");
    t.insert("batch_review", "To review");
    t.insert("batch_accept_all", "Accept all");
    t.insert("tags_write_tab", "Write this tab's fixes to tags");
    t.insert("lyrics_web", "Fetch lyrics from the internet");
    t.insert(
        "lyrics_web_desc",
        "Tracks without lyrics: look them up on LRCLIB when played (sends title and artist).",
    );
    t.insert("acoustid_key", "AcoustID key (optional)");
    t.insert(
        "acoustid_desc",
        "Identifies untagged tracks by their sound. Free key at acoustid.org/new-application.",
    );
    t.insert("info_lyrics_ok", "available");
    t.insert("info_lyrics_missing_web", "not on the web");
    t.insert("info_lyrics_none", "none");
    t.insert("batch_running", "Indexing…");
    t.insert("batch_summary", "{auto} fixed automatically · {revisar} to review · {nada} not found · {letras} lyrics downloaded");
    t.insert("info_error", "Error");
    t.insert("info_by_sound", "matched by sound");
    t.insert("info_nothing", "Nothing found.");
    t.insert("info_results", "{n} results — pick one");
    t.insert("info_searching", "Searching…");
    t.insert("tags_written", "{n} files updated");
    t.insert("tags_none", "No fixes in this tab.");
    t.insert("tags_writing", "Writing…");
    t.insert("tab_favorites", "Favorites");
    t.insert("tab_most_played", "Most played");
    t.insert(
        "favorites_empty",
        "No favorites yet. Star tracks (or press F).",
    );
    t.insert(
        "most_played_empty",
        "The tracks you play the most show up here.",
    );
    t.insert("tip_sort", "Sort");
    t.insert("tip_favorite", "Favorite (F)");
    t.insert("sort_default", "Default");
    t.insert("sort_title", "Title");
    t.insert("sort_artist", "Artist");
    t.insert("sort_album", "Album");
    t.insert("sort_duration", "Duration");
    t.insert("sort_added", "Recently added");
    t.insert("sort_plays", "Most played");
    t.insert("albums", "Albums");
    t.insert("albums_empty", "No albums here.");
    t.insert("album_track", "track");
    t.insert("album_tracks", "tracks");
    t.insert("various_artists", "Various artists");
    t.insert("no_album", "No album");
    t.insert("up_next_preview", "Up next");
    t.insert("crossfade", "Crossfade");
    t.insert("crossfade_off", "Off");
    t.insert("replaygain", "Normalize volume (ReplayGain)");
    t.insert(
        "replaygain_desc",
        "Uses the gain stored in the tags so tracks play at the same loudness.",
    );
    t.insert("reactive_bg", "Fullscreen background pulses with the music");
    t.insert("stereo", "Stereo");
    t.insert("mono", "Mono");
    t.insert("decimal_sep", ".");
    t.insert("bg_label", "Fullscreen background");
    t.insert("bg_choose", "Add…");
    t.insert("bg_count", "{n} backgrounds — one per track");
    t.insert("bg_clear", "Clear");
    t.insert("bg_none", "Blurred cover (default)");
    t.insert("search_placeholder", "Search by title or artist...");
    t.insert("no_lyrics", "No lyrics available for this track.");
    t.insert("config_title", "Settings");
    t.insert("theme_dark", "Theme: Dark");
    t.insert("theme_light", "Theme: Light");
    t.insert("scan_subfolders", "Scan subfolders");
    t.insert(
        "scan_subfolders_desc",
        "Include audio files from subfolders when opening a folder.",
    );
    t.insert("update_title", "📦 Update available!");
    t.insert("update_downloading", "Downloading...");
    t.insert("update_button", "Update");
    t.insert("update_later", "Later");
    t.insert("play", "Play");
    t.insert("pause", "Pause");
    t.insert("loop_off", "Loop: Off");
    t.insert("loop_track", "Loop: Track");
    t.insert("loop_list", "Loop: List");
    t.insert("shuffle_off", "Shuffle: Off");
    t.insert("shuffle_on", "Shuffle: Random (may repeat)");
    t.insert("shuffle_smart", "Shuffle: Smart (no repeats)");
    t
}

pub fn get_translations(lang: &str) -> Translations {
    match lang {
        "en" => en(),
        _ => pt_br(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Uma chave só em um idioma aparece crua na UI do outro — o fallback é a
    /// própria chave, então a paridade é o que garante a tradução.
    #[test]
    fn idiomas_tem_as_mesmas_chaves() {
        let pt: Vec<&str> = pt_br().into_keys().collect();
        let en: Vec<&str> = en().into_keys().collect();

        let mut pt = pt;
        let mut en = en;
        pt.sort_unstable();
        en.sort_unstable();
        assert_eq!(pt, en);
    }

    #[test]
    fn nenhum_texto_esta_vazio() {
        for (idioma, t) in [("pt-br", pt_br()), ("en", en())] {
            for (chave, texto) in t {
                assert!(!texto.is_empty(), "{idioma}: {chave} vazio");
            }
        }
    }

    #[test]
    fn idioma_desconhecido_cai_no_pt_br() {
        assert_eq!(get_translations("en").get("tip_stop"), Some(&"Stop"));
        assert_eq!(get_translations("pt-br").get("tip_stop"), Some(&"Parar"));
        assert_eq!(get_translations("fr").get("tip_stop"), Some(&"Parar"));
        assert_eq!(get_translations("").get("tip_stop"), Some(&"Parar"));
    }
}
