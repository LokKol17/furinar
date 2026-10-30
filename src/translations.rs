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
    t.insert("tip_immersive", "Modo imersivo");
    t.insert("tip_exit_immersive", "Sair do modo imersivo");
    t.insert("tip_library", "Biblioteca");
    t.insert("library_title", "Biblioteca");
    t.insert("library_empty", "Nenhuma faixa aqui.");
    t.insert("up_next", "A seguir");
    t.insert("up_next_empty", "Nada na fila.");
    t.insert("tip_lyrics_only", "Só a letra");
    t.insert("tip_video_only", "Só o vídeo");
    t.insert("tip_sleep", "Temporizador de sono");
    t.insert("sleep_end", "Fim da faixa");
    t.insert("tip_info", "Informações da faixa");
    t.insert("info_tab_track", "Faixa");
    t.insert("info_tab_batch", "Aba inteira");
    t.insert("info_corrected", "Corrigida pela web");
    t.insert("info_restore", "Restaurar original");
    t.insert("info_lyrics_label", "Letra");
    t.insert("info_get_lyrics", "Baixar letra");
    t.insert("info_search_web", "Procurar na internet");
    t.insert(
        "info_hint",
        "Nome ou dados errados? Procure a faixa na internet e escolha a versão certa.",
    );
    t.insert("info_apply", "Usar estes dados");
    t.insert("info_apply_tags", "Também salvar no arquivo");
    t.insert(
        "info_apply_tags_desc",
        "Outros players e o celular também passam a ver a correção. Sem isso, só o Furinar muda, e dá para desfazer.",
    );
    t.insert("info_changes", "O que muda");
    t.insert("info_no_changes", "Nada muda: os dados já estão assim.");
    t.insert("info_field_title", "Título");
    t.insert("info_field_artist", "Artista");
    t.insert("info_field_album", "Álbum");
    t.insert("info_empty", "(vazio)");
    t.insert("info_conf_high", "Certeza");
    t.insert("info_conf_mid", "Provável");
    t.insert("info_conf_low", "Talvez");
    t.insert(
        "info_confirm_tags",
        "Isso altera os arquivos de música. Clique de novo para confirmar.",
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
        "Identifica pelo som faixas sem tag. A versão oficial já vem com uma chave; a sua (grátis em acoustid.org/new-application) tem prioridade.",
    );
    t.insert("info_lyrics_ok", "tem");
    t.insert("info_lyrics_missing_web", "a web não tem");
    t.insert("info_lyrics_none", "nenhuma");
    t.insert("batch_running", "Indexando…");
    t.insert("batch_summary", "{auto} corrigidas sozinhas · {revisar} para revisar · {nada} sem resultado · {letras} letras baixadas");
    t.insert("info_error", "Erro");
    t.insert(
        "discord_status",
        "Mostrar no Discord o que está tocando (capa buscada no Deezer)",
    );
    t.insert("tip_mini", "Mini player");
    t.insert("tip_mini_exit", "Voltar ao tamanho normal");
    t.insert("scrobble_title", "Scrobble");
    t.insert("listenbrainz_token", "Token do ListenBrainz");
    t.insert("lastfm_keys", "Last.fm: chave e segredo da API");
    t.insert("lastfm_key", "Chave da API");
    t.insert("lastfm_secret", "Segredo");
    t.insert("lastfm_connect", "Conectar Last.fm");
    t.insert("lastfm_finish", "Concluir conexão");
    t.insert("lastfm_disconnect", "Desconectar Last.fm");
    t.insert("scrobble_desc", "Token em listenbrainz.org/settings. Chave do Last.fm em last.fm/api/account/create. Cada faixa conta depois de metade ou 4 min.");
    t.insert(
        "lastfm_waiting",
        "Autorize no navegador e clique em Concluir.",
    );
    t.insert("lastfm_connected", "Conectado como {u}");
    t.insert("info_by_sound", "identificada pelo som");
    t.insert("info_nothing", "Nada encontrado.");
    t.insert(
        "tags_fail_permission",
        "“{nome}” não pôde ser alterado: sem permissão (arquivo só leitura ou pasta protegida).",
    );
    t.insert(
        "tags_fail_busy",
        "“{nome}” está aberto em outro programa: feche-o e tente de novo.",
    );
    t.insert("tags_fail_other", "“{nome}” não pôde ser alterado: {erro}");
    t.insert("tags_fail_more", "(e mais {n} com problema)");
    t.insert(
        "update_err_net",
        "não deu para baixar a versão nova. Confira a conexão e tente de novo.",
    );
    t.insert(
        "update_err_corrupt",
        "o download veio corrompido e foi descartado (nada foi trocado). Tente de novo.",
    );
    t.insert("update_err_permission", "sem permissão para trocar o Furinar em {pasta}. Baixe a versão nova pela página da release.");
    t.insert("media_label", "Mostrar");
    t.insert("media_all", "Tudo");
    t.insert("media_audio", "Só áudio");
    t.insert("media_video", "Só vídeo");
    t.insert("playlist_add_to", "Adicionar à playlist");
    t.insert("playlist_new", "Nova playlist…");
    t.insert("playlist_remove", "Remover desta playlist");
    t.insert("playlist_move_up", "Mover para cima");
    t.insert("playlist_move_down", "Mover para baixo");
    t.insert("playlist_rename", "Renomear…");
    t.insert("playlist_delete", "Apagar…");
    t.insert("playlist_import", "Importar playlist (.m3u)…");
    t.insert("playlist_open_folder", "Abrir pasta…");
    t.insert("playlist_title_new", "Nova playlist");
    t.insert("playlist_title_rename", "Renomear playlist");
    t.insert("playlist_title_delete", "Apagar playlist");
    t.insert(
        "playlist_delete_msg",
        "Apagar “{nome}”? As músicas continuam no disco; só a lista some.",
    );
    t.insert("playlist_name", "Nome da playlist");
    t.insert("playlist_create", "Criar");
    t.insert("playlist_confirm_rename", "Renomear");
    t.insert("playlist_confirm_delete", "Apagar");
    t.insert("playlist_cancel", "Cancelar");
    t.insert("playlist_empty", "Playlist vazia. Clique com o botão direito numa música (ou no + ao passar o mouse) e escolha \"Adicionar à playlist\".");
    t.insert("sort_playlist", "Ordem da playlist");
    t.insert("playlist_added", "Adicionada a “{nome}”");
    t.insert("playlist_already", "Já está em “{nome}”");
    t.insert("playlist_removed", "Removida de “{nome}”");
    t.insert("playlist_created", "Playlist “{nome}” criada");
    t.insert(
        "playlist_created_with",
        "Playlist “{nome}” criada com a música",
    );
    t.insert("playlist_deleted", "Playlist “{nome}” apagada");
    t.insert(
        "playlist_save_failed",
        "Não deu para salvar a playlist “{nome}”",
    );
    t.insert("info_result_one", "1 resultado");
    t.insert(
        "net_timeout",
        "{servico} demorou demais para responder. Tente de novo daqui a pouco.",
    );
    t.insert(
        "net_offline",
        "Não deu para falar com {servico}. Confira a conexão com a internet.",
    );
    t.insert(
        "net_busy",
        "{servico} está sobrecarregado agora. Tente de novo em alguns minutos.",
    );
    t.insert(
        "net_server",
        "{servico} está com problemas (erro {n}). Tente de novo mais tarde.",
    );
    t.insert(
        "net_denied",
        "{servico} recusou o acesso. Confira a chave ou o login nas Configurações.",
    );
    t.insert("net_refused", "{servico} recusou o pedido: {motivo}.");
    t.insert("net_refused_code", "{servico} recusou o pedido (erro {n}).");
    t.insert(
        "net_check_key",
        "Confira a chave ou o login nas Configurações.",
    );
    t.insert("net_unexpected", "{servico} respondeu algo inesperado. Tente de novo; se continuar, avise no GitHub do Furinar.");
    t.insert(
        "info_nothing_no_key",
        "Nada encontrado pelo nome. A busca pelo som precisa de uma chave do AcoustID: preencha em Configurações.",
    );
    t.insert("info_results", "{n} resultados — escolha um");
    t.insert("info_searching", "Buscando…");
    t.insert("tags_written", "{n} arquivos atualizados");
    t.insert("tags_none", "Nenhuma correção nesta aba.");
    t.insert("tags_writing", "Gravando…");
    t.insert("tab_favorites", "Favoritas");
    t.insert("tab_most_played", "Mais tocadas");
    t.insert("tab_opened", "Abertos");
    t.insert(
        "favorites_empty",
        "Nenhuma favorita ainda. Marque faixas com a estrela (ou F).",
    );
    t.insert(
        "most_played_empty",
        "As faixas que você mais ouve aparecem aqui.",
    );
    t.insert("tip_sort", "Ordenar");
    t.insert("key_space", "Espaço");
    t.insert("toast_fav_on", "Adicionada às favoritas");
    t.insert("toast_fav_off", "Removida das favoritas");
    t.insert("toast_lyrics_saved", "Letra encontrada");
    t.insert("toast_playing", "Tocando:");
    t.insert(
        "play_missing",
        "Não deu para tocar “{nome}”: o arquivo não existe mais (foi movido ou apagado).",
    );
    t.insert(
        "play_needs_ffmpeg",
        "Não deu para tocar “{nome}”: este formato precisa do FFmpeg.",
    );
    t.insert(
        "play_unsupported",
        "Não deu para tocar “{nome}”: formato não suportado ou arquivo danificado.",
    );
    t.insert(
        "play_no_output",
        "Não deu para tocar: nenhuma saída de áudio disponível. Confira o fone ou a caixa de som.",
    );
    t.insert("toast_sleep_off", "Temporizador de sono desligado");
    t.insert("tip_favorite", "Favoritar");
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
    t.insert(
        "reactive_bg",
        "Fundo da tela cheia pulsa com a música (usa mais CPU)",
    );
    t.insert("stereo", "Estéreo");
    t.insert("mono", "Mono");
    t.insert("decimal_sep", ",");
    t.insert("bg_label", "Fundo da tela cheia");
    t.insert("bg_choose", "Adicionar…");
    t.insert("bg_count", "{n} fundos — trocam a cada faixa");
    t.insert("bg_clear", "Limpar");
    t.insert("bg_none", "Capa desfocada (padrão)");
    t.insert(
        "ffmpeg_missing",
        "FFmpeg não encontrado: MKV/WebM/AVI e o vídeo no modo imersivo ficam desligados. Instale o pacote ffmpeg da sua distribuição e reabra o Furinar.",
    );
    t.insert(
        "ffmpeg_missing_windows",
        "FFmpeg não encontrado: MKV/WebM/AVI e o vídeo no modo imersivo ficam desligados. Rode o instalador de novo marcando \"Baixar o FFmpeg\".",
    );
    t.insert("ffmpeg_get", "Baixar o FFmpeg");
    t.insert("search_placeholder", "Buscar por título ou artista...");
    t.insert("no_lyrics", "Sem letra disponível para esta faixa.");
    t.insert("config_title", "Configurações");
    t.insert("theme", "Tema");
    t.insert("theme_dark", "Escuro");
    t.insert("theme_light", "Claro");
    t.insert("scan_subfolders", "Escaneia subpastas");
    t.insert(
        "scan_subfolders_desc",
        "Inclui arquivos de áudio de subpastas ao abrir uma pasta.",
    );
    t.insert("update_title", "Nova versão disponível");
    t.insert("update_downloading", "Baixando…");
    t.insert("update_button", "Atualizar");
    t.insert("update_later", "Depois");
    t.insert("language", "Idioma");
    t.insert("update_ready_title", "Atualização pronta");
    t.insert(
        "update_ready",
        "Reinicie o Furinar para usar a versão nova. Ele volta na mesma música.",
    );
    t.insert("update_restart", "Reiniciar agora");
    t.insert("update_failed", "Não deu para atualizar:");
    t.insert("update_retry", "Tentar de novo");
    t.insert("update_manual", "O Furinar não consegue se atualizar nesta pasta. Baixe a versão nova na página da release.");
    t.insert("update_open_page", "Página da release");
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
    t.insert("tip_immersive", "Immersive mode");
    t.insert("tip_exit_immersive", "Exit immersive mode");
    t.insert("tip_library", "Library");
    t.insert("library_title", "Library");
    t.insert("library_empty", "No tracks here.");
    t.insert("up_next", "Up next");
    t.insert("up_next_empty", "Nothing queued.");
    t.insert("tip_lyrics_only", "Lyrics only");
    t.insert("tip_video_only", "Video only");
    t.insert("tip_sleep", "Sleep timer");
    t.insert("sleep_end", "End of track");
    t.insert("tip_info", "Track info");
    t.insert("info_tab_track", "Track");
    t.insert("info_tab_batch", "Whole tab");
    t.insert("info_corrected", "Fixed from the web");
    t.insert("info_restore", "Restore original");
    t.insert("info_lyrics_label", "Lyrics");
    t.insert("info_get_lyrics", "Get lyrics");
    t.insert("info_search_web", "Search online");
    t.insert(
        "info_hint",
        "Wrong name or details? Search for the track online and pick the right one.",
    );
    t.insert("info_apply", "Use this info");
    t.insert("info_apply_tags", "Also save it to the file");
    t.insert(
        "info_apply_tags_desc",
        "Other players and your phone will see the fix too. Without it, only Furinar changes, and you can undo it.",
    );
    t.insert("info_changes", "What changes");
    t.insert(
        "info_no_changes",
        "Nothing changes: the details already match.",
    );
    t.insert("info_field_title", "Title");
    t.insert("info_field_artist", "Artist");
    t.insert("info_field_album", "Album");
    t.insert("info_empty", "(empty)");
    t.insert("info_conf_high", "Sure");
    t.insert("info_conf_mid", "Likely");
    t.insert("info_conf_low", "Maybe");
    t.insert(
        "info_confirm_tags",
        "This changes the music files. Click again to confirm.",
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
        "Identifies untagged tracks by their sound. Official builds ship with a key; yours (free at acoustid.org/new-application) takes precedence.",
    );
    t.insert("info_lyrics_ok", "available");
    t.insert("info_lyrics_missing_web", "not on the web");
    t.insert("info_lyrics_none", "none");
    t.insert("batch_running", "Indexing…");
    t.insert("batch_summary", "{auto} fixed automatically · {revisar} to review · {nada} not found · {letras} lyrics downloaded");
    t.insert("info_error", "Error");
    t.insert(
        "discord_status",
        "Show what's playing on Discord (cover looked up on Deezer)",
    );
    t.insert("tip_mini", "Mini player");
    t.insert("tip_mini_exit", "Back to full size");
    t.insert("scrobble_title", "Scrobble");
    t.insert("listenbrainz_token", "ListenBrainz token");
    t.insert("lastfm_keys", "Last.fm: API key and secret");
    t.insert("lastfm_key", "API key");
    t.insert("lastfm_secret", "Secret");
    t.insert("lastfm_connect", "Connect Last.fm");
    t.insert("lastfm_finish", "Finish connecting");
    t.insert("lastfm_disconnect", "Disconnect Last.fm");
    t.insert("scrobble_desc", "Token at listenbrainz.org/settings. Last.fm key at last.fm/api/account/create. A track counts after half of it or 4 min.");
    t.insert(
        "lastfm_waiting",
        "Authorize in the browser, then click Finish.",
    );
    t.insert("lastfm_connected", "Connected as {u}");
    t.insert("info_by_sound", "matched by sound");
    t.insert("info_nothing", "Nothing found.");
    t.insert(
        "tags_fail_permission",
        "“{nome}” couldn't be changed: no permission (read-only file or protected folder).",
    );
    t.insert(
        "tags_fail_busy",
        "“{nome}” is open in another program: close it and try again.",
    );
    t.insert("tags_fail_other", "“{nome}” couldn't be changed: {erro}");
    t.insert("tags_fail_more", "(and {n} more with problems)");
    t.insert(
        "update_err_net",
        "couldn't download the new version. Check your connection and try again.",
    );
    t.insert(
        "update_err_corrupt",
        "the download was corrupted and thrown away (nothing was replaced). Try again.",
    );
    t.insert("update_err_permission", "no permission to replace Furinar in {pasta}. Download the new version from the release page.");
    t.insert("media_label", "Show");
    t.insert("media_all", "Everything");
    t.insert("media_audio", "Audio only");
    t.insert("media_video", "Video only");
    t.insert("playlist_add_to", "Add to playlist");
    t.insert("playlist_new", "New playlist…");
    t.insert("playlist_remove", "Remove from this playlist");
    t.insert("playlist_move_up", "Move up");
    t.insert("playlist_move_down", "Move down");
    t.insert("playlist_rename", "Rename…");
    t.insert("playlist_delete", "Delete…");
    t.insert("playlist_import", "Import playlist (.m3u)…");
    t.insert("playlist_open_folder", "Open folder…");
    t.insert("playlist_title_new", "New playlist");
    t.insert("playlist_title_rename", "Rename playlist");
    t.insert("playlist_title_delete", "Delete playlist");
    t.insert(
        "playlist_delete_msg",
        "Delete “{nome}”? The songs stay on disk; only the list goes away.",
    );
    t.insert("playlist_name", "Playlist name");
    t.insert("playlist_create", "Create");
    t.insert("playlist_confirm_rename", "Rename");
    t.insert("playlist_confirm_delete", "Delete");
    t.insert("playlist_cancel", "Cancel");
    t.insert("playlist_empty", "Empty playlist. Right-click a song (or use the + that shows on hover) and pick \"Add to playlist\".");
    t.insert("sort_playlist", "Playlist order");
    t.insert("playlist_added", "Added to “{nome}”");
    t.insert("playlist_already", "Already in “{nome}”");
    t.insert("playlist_removed", "Removed from “{nome}”");
    t.insert("playlist_created", "Playlist “{nome}” created");
    t.insert(
        "playlist_created_with",
        "Playlist “{nome}” created with the song",
    );
    t.insert("playlist_deleted", "Playlist “{nome}” deleted");
    t.insert(
        "playlist_save_failed",
        "Couldn't save the playlist “{nome}”",
    );
    t.insert("info_result_one", "1 result");
    t.insert(
        "net_timeout",
        "{servico} took too long to answer. Try again in a moment.",
    );
    t.insert(
        "net_offline",
        "Couldn't reach {servico}. Check your internet connection.",
    );
    t.insert(
        "net_busy",
        "{servico} is overloaded right now. Try again in a few minutes.",
    );
    t.insert(
        "net_server",
        "{servico} is having problems (error {n}). Try again later.",
    );
    t.insert(
        "net_denied",
        "{servico} denied access. Check the key or login in Settings.",
    );
    t.insert("net_refused", "{servico} refused the request: {motivo}.");
    t.insert(
        "net_refused_code",
        "{servico} refused the request (error {n}).",
    );
    t.insert("net_check_key", "Check the key or login in Settings.");
    t.insert("net_unexpected", "{servico} sent an unexpected answer. Try again; if it keeps happening, report it on Furinar's GitHub.");
    t.insert(
        "info_nothing_no_key",
        "Nothing found by name. Matching by sound needs an AcoustID key: add one in Settings.",
    );
    t.insert("info_results", "{n} results — pick one");
    t.insert("info_searching", "Searching…");
    t.insert("tags_written", "{n} files updated");
    t.insert("tags_none", "No fixes in this tab.");
    t.insert("tags_writing", "Writing…");
    t.insert("tab_favorites", "Favorites");
    t.insert("tab_most_played", "Most played");
    t.insert("tab_opened", "Opened");
    t.insert(
        "favorites_empty",
        "No favorites yet. Star tracks (or press F).",
    );
    t.insert(
        "most_played_empty",
        "The tracks you play the most show up here.",
    );
    t.insert("tip_sort", "Sort");
    t.insert("key_space", "Space");
    t.insert("toast_fav_on", "Added to favorites");
    t.insert("toast_fav_off", "Removed from favorites");
    t.insert("toast_lyrics_saved", "Lyrics found");
    t.insert("toast_playing", "Playing:");
    t.insert(
        "play_missing",
        "Couldn't play “{nome}”: the file no longer exists (moved or deleted).",
    );
    t.insert(
        "play_needs_ffmpeg",
        "Couldn't play “{nome}”: this format needs FFmpeg.",
    );
    t.insert(
        "play_unsupported",
        "Couldn't play “{nome}”: unsupported format or damaged file.",
    );
    t.insert(
        "play_no_output",
        "Couldn't play: no audio output available. Check your headphones or speakers.",
    );
    t.insert("toast_sleep_off", "Sleep timer off");
    t.insert("tip_favorite", "Favorite");
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
    t.insert(
        "reactive_bg",
        "Fullscreen background pulses with the music (uses more CPU)",
    );
    t.insert("stereo", "Stereo");
    t.insert("mono", "Mono");
    t.insert("decimal_sep", ".");
    t.insert("bg_label", "Fullscreen background");
    t.insert("bg_choose", "Add…");
    t.insert("bg_count", "{n} backgrounds — one per track");
    t.insert("bg_clear", "Clear");
    t.insert("bg_none", "Blurred cover (default)");
    t.insert(
        "ffmpeg_missing",
        "FFmpeg not found: MKV/WebM/AVI and the video in the immersive mode are off. Install your distribution's ffmpeg package and reopen Furinar.",
    );
    t.insert(
        "ffmpeg_missing_windows",
        "FFmpeg not found: MKV/WebM/AVI and the video in the immersive mode are off. Run the installer again with \"Download FFmpeg\" checked.",
    );
    t.insert("ffmpeg_get", "Get FFmpeg");
    t.insert("search_placeholder", "Search by title or artist...");
    t.insert("no_lyrics", "No lyrics available for this track.");
    t.insert("config_title", "Settings");
    t.insert("theme", "Theme");
    t.insert("theme_dark", "Dark");
    t.insert("theme_light", "Light");
    t.insert("scan_subfolders", "Scan subfolders");
    t.insert(
        "scan_subfolders_desc",
        "Include audio files from subfolders when opening a folder.",
    );
    t.insert("update_title", "New version available");
    t.insert("update_downloading", "Downloading…");
    t.insert("update_button", "Update");
    t.insert("update_later", "Later");
    t.insert("language", "Language");
    t.insert("update_ready_title", "Update ready");
    t.insert(
        "update_ready",
        "Restart Furinar to use the new version. It reopens on the same track.",
    );
    t.insert("update_restart", "Restart now");
    t.insert("update_failed", "Couldn't update:");
    t.insert("update_retry", "Try again");
    t.insert("update_manual", "Furinar can't update itself in this folder. Download the new version from the release page.");
    t.insert("update_open_page", "Release page");
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
        "pt-br" | "pt" => pt_br(),
        _ => en(),
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
    fn idioma_desconhecido_cai_no_ingles() {
        assert_eq!(get_translations("en").get("tip_stop"), Some(&"Stop"));
        assert_eq!(get_translations("pt-br").get("tip_stop"), Some(&"Parar"));
        assert_eq!(get_translations("pt").get("tip_stop"), Some(&"Parar"));
        assert_eq!(get_translations("fr").get("tip_stop"), Some(&"Stop"));
        assert_eq!(get_translations("").get("tip_stop"), Some(&"Stop"));
    }
}
