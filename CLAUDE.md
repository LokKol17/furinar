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

- `src/main.rs` holds almost everything: app state, playback logic, Slint callback wiring, OS integration. The other modules are small, mostly pure helpers:
  - `config.rs` — `AppConfig`, serde JSON `furinar_config.json` (XDG `~/.config/furinar/` on Linux, next to the exe on Windows). Legacy singular `pasta` field is migrated into `pastas`.
  - `track.rs` — folder scanning, tag reading (lofty), accent/case-insensitive search (`normalizar_busca`, `ordem_filtrada`).
  - `lrc.rs` — `.lrc` synced-lyrics parsing and the visible window of lines.
  - `translations.rs` — `HashMap<&str, &str>` per language (`pt_br`, `en`); unknown language falls back to pt-br.
  - `memoria.rs` — `aparar()` (Linux): `madvise(MADV_DONTNEED)` on clean read-only file mappings from `/proc/self/smaps` + `malloc_trim`, to cut RSS. Called 5 s after startup, after the update check, and on minimize.
  - `updates.rs` — self-update from GitHub Releases (`self_update` with rustls). Disabled when the binary lives under `/usr/bin`, `/usr/local/bin`, snap or flatpak paths.
- **State model**: a single `EstadoAudio` in `Rc<RefCell<…>>`, cloned into every `ui.on_*` closure alongside `ui.as_weak()`. All UI/audio work runs on the Slint event-loop thread. Free functions take `(&mut EstadoAudio, &MainWindow)`.
- **Tabs vs. playback**: `aba_visivel` (folder shown) is independent of `pasta_reproducao` (folder playing). Folders are scanned lazily (`PastaMusical.carregada`, `garantir_pasta_carregada`). `indices_visiveis` maps visible list rows → track indices after filtering.
- **Main 250 ms `Timer`** drives progress, auto-advance, lyrics sync, and polls `mpsc` channels from system media controls (souvlaki, `mpris` feature) and Windows taskbar thumbnail buttons. Those integrations are created lazily inside the timer because the winit window only exists after the event loop starts.
- The update check runs on a `std::thread`; its result goes through `Arc<Mutex<Option<UpdateInfo>>>` and is polled by a second timer.
- **Platform split**: on Windows, a winit msg hook catches taskbar `WM_COMMAND` and forces full repaints (software-renderer redraw bug); DWM rounded corners via the `windows` crate. Non-Windows forces `WINIT_UNIX_BACKEND=x11`. `mpris` is a default feature — code must compile with and without it, on both OSes.
- **UI**: `ui/main.slint` (window, compiled by `build.rs` via `slint_build`), `ui/theme.slint` (light/dark palette), `ui/i18n.slint` (`I18n` global).
- **Adding a translated string** touches three places: the key in both `pt_br()` and `en()` in `translations.rs` (tests enforce identical key sets and non-empty values), a property in `ui/i18n.slint`, and a setter line in `aplicar_idioma` in `main.rs`.
- `build.rs` normalizes `ui/assets/furinar_icon.png` into the generated (gitignored) `ui/assets/furinar_icon_quadrado.png` used by Slint, and on Windows embeds a multi-size `.ico` into the exe.
- `docs/` is the project website (index.html/script.js/styles.css) plus screenshots and `RELEASE_CONVENTION.md`.

## Versioning / release

`Cargo.toml` version is the single source of truth (the self-updater compares against `CARGO_PKG_VERSION`). After bumping it, run `make sync-version` then `make check-version` (propagates to `pkg/debian/control`, `pkg/arch/PKGBUILD`, `pkg/fedora/furinar.spec`, `pkg/install.sh`, `pkg/furinar.metainfo.xml`) and add a `CHANGELOG.md` entry. Pushing a `vX.Y.Z` tag triggers `.github/workflows/release.yml`. Release asset names (`furinar-windows-x86_64.exe`, `furinar-linux-x86_64.tar.gz`) must not change — the updater matches on them. Details: `docs/RELEASE_CONVENTION.md`.
