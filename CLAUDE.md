# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Furinar: lightweight desktop audio player (Rust 2024 + Slint UI, rodio/symphonia audio). Targets Linux and Windows. Code identifiers, comments, commit messages and CHANGELOG are in **Portuguese** — keep it that way. Commits use conventional prefixes (`fix:`, `refactor:`, `test:`, `ci:`, `build:`).

## Commands

Linux build deps: `libasound2-dev libgtk-3-dev pkg-config`.

- Run: `cargo run` · Release build: `cargo build --release`
- Format check: `cargo fmt --all --check`
- Lint (CI runs both with `-D warnings`; the codebase has no `#[allow]` — keep it at zero warnings):
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo clippy --all-targets --no-default-features -- -D warnings`
  - Windows-only code (`cfg(target_os = "windows")`) is checked locally with `--target x86_64-pc-windows-gnu`.
- Tests: `cargo test --all-features` and `cargo test --no-default-features`
- Single test: `cargo test <name>` (e.g. `cargo test m4a_permite_seek`). Tests are inline `#[cfg(test)] mod tests` in each module; audio fixtures live in `tests/fixtures/` (loaded via `CARGO_MANIFEST_DIR`).
- Packaging: `make deb | rpm | tarball | release` (`make help` lists all).

## Architecture

- `src/main.rs` is setup only: module list, `aplicar_idioma`, loading the config into `EstadoAudio`, the 250 ms timer and window/update callbacks. App logic lives in modules that `use crate::*` (root re-imports them with `use <mod>::*`, so items are shared crate-wide as `pub(crate)`); each area has a `ligar(ui, estado)` that registers its Slint callbacks:
  - `estado.rs` — `EstadoAudio`, `PastaMusical`/`Tipo`, tab constants, `salvar_configuracao`.
  - `listas.rs` — tabs, scanning, ordering, special lists, favorites/play counts, album tab.
  - `reproducao.rs` — sources/player, seek, next/previous (shuffle, history), gapless/crossfade, lyrics sync, volume, sleep timer.
  - `ponte_imersivo.rs` — bridge to the immersive mode (track shown, library drawer, up-next, preview, clock).
  - `web_ui.rs` — track info screen and tab batch (talks to `buscador`).
  - `plataforma.rs` — MPRIS/SMTC, Windows taskbar buttons, rounded corners, Discord status (`discord.rs`: IPC thread alive only while enabled; app id is public), opening URLs.
  - Scrobbling: `scrobble.rs` (ListenBrainz token / Last.fm signed calls + browser auth), run through `buscador` (`Pedido::TocandoAgora`/`Scrobble`); failed listens persist in `furinar_scrobbles.json`. Triggered from `contar_reproducao` and `atualizar_scrobble` (web_ui.rs).
  - Mini player: same window shrunk to 380×112 (`modo_mini`, `ui/mini.slint`, `always-on-top`), toggled by `alternar_mini` in ponte_imersivo.rs; cover = cached thumbnail loaded on a thread.
  - Karaoke: `lrc::palavras_lrc` (enhanced LRC word times) + `lrc::progresso_linha`; `atualizar_karaoke` runs in the 33 ms immersive timer.
  - `captura.rs` (tests only) — window-less screenshots.
  Mostly pure helpers:
  - `config.rs` — `AppConfig`, serde JSON `furinar_config.json` (XDG `~/.config/furinar/` on Linux, next to the exe on Windows). Legacy singular `pasta` field is migrated into `pastas`.
  - `track.rs` — folder scanning, tag reading (lofty: title, artist, album, number, duration, ReplayGain factor, date added), accent/case-insensitive search (`normalizar_busca`, `ordem_filtrada`), sorting (`Ordem`, `ordenar`) and album grouping (`agrupar_albuns`).
  - `lrc.rs` — `.lrc` synced-lyrics parsing, embedded lyrics fallback (`letra_embutida`: SYLT, then USLT/LYRICS tried as LRC; plain text = `Letra::Livre`, shown whole without highlight) and the visible window of lines.
  - `translations.rs` — `HashMap<&str, &str>` per language (`pt_br`, `en`); default and fallback language is English (`en`); `pt`/`pt-br` select Portuguese.
  - `memoria.rs` — `aparar()` (Linux): `madvise(MADV_DONTNEED)` on read-only `PT_LOAD` segments from `dl_iterate_phdr` (exe + libs) and on shared `r--s` data files (fonts, fontconfig cache, rechecked via `/proc/self/map_files`) + `malloc_trim`. Never trim private file mappings picked from smaps: a library being `dlopen`ed concurrently loses its relocations → jump to 0x0 (`cargo test estresse_aparar -- --ignored`), to cut RSS. Called 5 s after startup, after the update check, and on minimize.
  - Immersive mode (fullscreen, `F11`/`--imersivo`): `imersivo.rs` holds `EstadoImersivo`, stored as `Option` in `EstadoAudio` (`None` = nothing allocated) and driven by a 33 ms timer that only runs in the mode. `capa.rs` loads cover/metadata/dominant color on a thread; `fundo.rs` resolves and plays backgrounds (image, GIF/WebP, video via system `ffmpeg`) through a 2-frame bounded channel; `visualizador.rs` has the `Espiao` `Source` wrapper (copies samples only while the mode is on) + FFT bars. UI in `ui/imersivo.slint`, shown with `if modo_imersivo` while the normal UI is hidden.
  - Tabs: `pastas` starts with two special lists (`POS_FAVORITAS`, `POS_MAIS_TOCADAS`, `ESPECIAIS = 2`) and may end with a temporary album tab (`Tipo::Album`, opened from the album grid, never saved). Special lists have an empty `caminho` and store absolute paths in `TrackInfo.path` (`raiz.join(abs) == abs`), so playback code is the same for all tabs. Tracks are identified by `biblioteca::chave_faixa` (absolute path with `/`); favorites and play counts live in `biblioteca.rs` → `furinar_biblioteca.json`. Lists are saved/restored by key (`PastaMusical::chave`), tracks by path. `ordem_lista()` = filter + per-tab `Ordem`; navigation uses the same order.
  - Playback: `executar_seek` rebuilds the `Player` (paused or not); sources are `Espiao<Transicao<Decoder>>` (`transicao.rs` = ReplayGain gain, crossfade fade-in, fade-out on request). `preparar_transicao` (250 ms timer, near the end) either appends the next track to the same player (gapless, `Engatilhada`, promoted by `assumir_faixa` when `Player::len()` drops) or starts a crossfade in a second player (`saindo`). Anything that changes what plays next calls `desengatilhar`. `historico` (list, track key) feeds "previous" in shuffle (`voltar_no_historico` re-queues the current track).
  - Web lookup (`ui/info.slint`, `I` key): `rede.rs` (blocking reqwest + per-host rate limit, retries on 503/429), `musicbrainz.rs` (text search with duration window, cover art), `acoustid.rs` (rusty-chromaprint fingerprint + lookup, needs the user's AcoustID key), `letras_web.rs` (LRCLIB → `.lrc` beside the track or cache), `metadados.rs` (`Consulta`/`Candidato`, confidence scoring, local `Banco` of accepted fixes in `furinar_metadados.json` applied on scan via `escanear()`, `gravar_tags`). `buscador.rs` runs them on one thread, created on demand and dropped after 60 s idle; responses are polled by `processar_web` in the 250 ms timer. Live test: `cargo test web_ao_vivo -- --ignored --nocapture` (optionally `FURINAR_WEB_PASTA=<dir>`).
  - Library drawer (`ui/painel.slint`, `painel.rs`, exists only while open): the list is virtualized by hand — the UI reports scroll (`imv_painel_rolou`) and Rust publishes only a window of rows (`imv_painel_primeira` + `imv_painel_linhas`). `miniaturas.rs` loads 96 px cover thumbnails on 2 threads, with a JPEG disk cache in `~/.cache/furinar/capas` (empty file = "no cover"), capped at 64 MB by `aparar_cache` (LRU by mtime, refreshed on each hit) when the panel opens. The album grid (guia 2) gets every card but images only for the visible ones (`imv_albuns_rolou`). The "up next" queue comes from `proximas()` in `main.rs`; shuffle pre-draws into `sorteio` (mode 1) / reads the `sacola_shuffle` (mode 2) so the queue shown is what plays. `sono.rs` = sleep timer, checked in the 250 ms timer.
  - Visual check without a window: `FURINAR_CAPTURA_FAIXA=<track> [FURINAR_CAPTURA_FUNDO=<bg>] [FURINAR_CAPTURA_PAINEL=1 [FURINAR_CAPTURA_ALBUNS=1]] [FURINAR_CAPTURA_CONFIG=1] cargo test captura_modo_imersivo -- --ignored` renders `target/captura-imersivo.png`; `captura_modo_normal` renders `target/captura-normal.png` (`FURINAR_CAPTURA_CONFIG=1`, `FURINAR_CAPTURA_INFO=1|2`, `FURINAR_CAPTURA_MINI=1`, `FURINAR_CAPTURA_LOOP`/`_SHUFFLE=0|1|2`); `FURINAR_CAPTURA_OCIOSO=1` renders the immersive mode with controls hidden (winit runs on Wayland, so X11 screenshot tools don't see the window).
  - `updates.rs` — self-update from GitHub Releases (`self_update` with rustls). Disabled when the binary lives under `/usr/bin`, `/usr/local/bin`, snap or flatpak paths.
- **State model**: a single `EstadoAudio` in `Rc<RefCell<…>>`, cloned into every `ui.on_*` closure alongside `ui.as_weak()`. All UI/audio work runs on the Slint event-loop thread. Free functions take `(&mut EstadoAudio, &MainWindow)`.
- **Tabs vs. playback**: `aba_visivel` (folder shown) is independent of `pasta_reproducao` (folder playing). Folders are scanned lazily (`PastaMusical.carregada`, `garantir_pasta_carregada`). `indices_visiveis` maps visible list rows → track indices after filtering.
- **Main 250 ms `Timer`** drives progress, auto-advance, lyrics sync, and polls `mpsc` channels from system media controls (souvlaki, `mpris` feature) and Windows taskbar thumbnail buttons. Those integrations are created lazily inside the timer because the winit window only exists after the event loop starts.
- The update check runs on a `std::thread`; its result goes through `Arc<Mutex<Option<UpdateInfo>>>` and is polled by a second timer.
- **Platform split**: on Windows, a winit msg hook catches taskbar `WM_COMMAND` and forces full repaints (software-renderer redraw bug); DWM rounded corners via the `windows` crate. Non-Windows sets `WINIT_UNIX_BACKEND=x11`, but winit 0.30 ignores it — the app runs natively on Wayland when available. `mpris` is a default feature — code must compile with and without it, on both OSes.
- **UI**: `ui/main.slint` (window, compiled by `build.rs` via `slint_build`), `ui/theme.slint` (light/dark palette), `ui/i18n.slint` (`I18n` global).
- **Adding a translated string** touches three places: the key in both `pt_br()` and `en()` in `translations.rs` (tests enforce identical key sets and non-empty values), a property in `ui/i18n.slint`, and a setter line in `aplicar_idioma` in `main.rs`.
- `build.rs` normalizes `ui/assets/furinar_icon.png` into the generated (gitignored) `ui/assets/furinar_icon_quadrado.png` used by Slint, and on Windows embeds a multi-size `.ico` into the exe.
- `docs/` is the project website (index.html/script.js/styles.css) plus screenshots and `RELEASE_CONVENTION.md`.

## Versioning / release

`Cargo.toml` version is the single source of truth (the self-updater compares against `CARGO_PKG_VERSION`). After bumping it, run `make sync-version` then `make check-version` (propagates to `pkg/debian/control`, `pkg/arch/PKGBUILD`, `pkg/fedora/furinar.spec`, `pkg/install.sh`, `pkg/furinar.metainfo.xml`) and add a `CHANGELOG.md` entry. Pushing a `vX.Y.Z` tag triggers `.github/workflows/release.yml`. Release asset names (`furinar-windows-x86_64.exe`, `furinar-linux-x86_64.tar.gz`) must not change — the updater matches on them. Details: `docs/RELEASE_CONVENTION.md`.
