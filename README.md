<div align="center">

<img src="ui/assets/furinar_icon.png" width="128" height="128" alt="Furinar" />

# Furinar

**A light, fast, and beautiful audio player — inspired by the Hydro Archon.**

![Windows](https://img.shields.io/badge/platform-Windows-3da9d6?style=flat-square)
![Rust](https://img.shields.io/badge/built%20with-Rust-c9a050?style=flat-square)
![RAM](https://img.shields.io/badge/RAM-2--10MB-0b1a2b?style=flat-square)
![License](https://img.shields.io/badge/license-BSD--3--Clause-lightgrey?style=flat-square)

🇺🇸 English | [🇧🇷 Português](README.pt-BR.md)

</div>

---

<div align="center">

<!-- Screenshot 1: main player view (folder/tabs/playlist visible) -->
![Furinar - main player](docs/screenshot-player.png)

</div>

## Why Furinar?

Most desktop music players these days ship an entire browser engine just to play an MP3. Furinar does the opposite: it opens in milliseconds and sits in the background using **2 to 10 MB of RAM** — less than a single browser tab — while still covering everything a real player needs: tags, synced lyrics, multiple libraries, and native Windows integration.

The visual palette — deep navy, cyan, and gold (or baby blue and pearl white in light mode) — is a tribute to **Furina**, from *Genshin Impact*. No official affiliation, just a fan project.

## ✨ Features

**Playback**
- MP3, WAV, FLAC, OGG/Vorbis, and M4A support
- Play/Pause, Stop, Previous, Next, seeking, and volume control
- Loop (off / track / playlist) and Shuffle
- Automatically resumes the last track and position on reopen

**Organization**
- **Multiple folders as tabs** — open as many libraries as you want; switching tabs never interrupts what's currently playing
- Optional recursive subfolder scanning
- **ID3/Vorbis tag reading** (title and artist, falling back to the filename)
- Search/filter by title or artist, accent- and case-insensitive

**Extras**
- **Synced lyrics** via `.lrc` files, scrolling along with the track
- **Light/dark theme**, switched instantly, no restart needed
- Frameless window with a custom title bar (drag-to-move, Aero Snap, rounded corners on Windows 11)

<div align="center">

<!-- Screenshot 2: synced lyrics panel -->
![Furinar - synced lyrics](docs/screenshot-lyrics.png)

</div>

**Windows integration**
- **Play/Pause, Previous, and Next** buttons right on the taskbar thumbnail preview
- System media controls (keyboard media keys, action center) via SMTC
- Keyboard shortcuts: `Space` play/pause, `←`/`→` seek ±5s, `↑`/`↓` navigate the list, `Enter` plays the selected track

<div align="center">
  <table>
    <tr>
      <td align="center"><b>Bright Theme</b></td>
      <td align="center"><b>Dark Theme</b></td>
    </tr>
    <tr>
      <td><img src="docs/screenshot-player.png" alt="Furinar - tema claro"></td>
      <td><img src="docs/screenshot-player-dark.png" alt="Furinar - tema escuro"></td>
    </tr>
  </table>
</div>

## 🚀 Getting started

1. Download the latest `furinar.exe` (or build it — see below).
2. Open the app and click **Open folder** to point it at your music.
3. That's it — Furinar remembers everything on its own from then on.

---

## For developers

<details>
<summary>Build and run</summary>

**Requirements:** Rust (edition 2024) and Windows.

```bash
cargo run
```

Optimized build:

```bash
cargo build --release
```

The final binary lands at `target/release/furinar.exe`.

</details>

<details>
<summary>App icon</summary>

The icon lives at `ui/assets/furinar_icon.png`. It doesn't need to be square: `build.rs` trims transparent edges and centers the content in a 256px square (preserving aspect ratio, no stretching), producing `ui/assets/furinar_icon_quadrado.png`. That derivative feeds both the window icon and the multi-resolution `.ico` (16/32/48/256) embedded in the executable.

To change the icon, just replace the source file — the build handles the rest.

</details>

<details>
<summary>Config file</summary>

Preferences are saved automatically to `furinar_config.json`, in the project root:

```json
{
  "pastas": ["E:\\Music"],
  "aba_visivel_salva": 0,
  "pasta_reproducao_salva": 0,
  "volume": 0.11666667,
  "modo_loop": 2,
  "shuffle": true,
  "indice_atual": 5,
  "tempo_atual": 42,
  "escanear_subpastas": true,
  "tema_claro": false
}
```

The old singular `pasta` field (from earlier versions) is still read: if `pastas` is empty and it exists, it's automatically migrated into the list on first run.

You can edit the file by hand — the player re-reads the config on every launch.

</details>

## 📄 License

Furinar's own source code is licensed under [BSD 3-Clause](LICENSE) — you're free to use, fork, and modify it, as long as credit is given and the license notice is kept.

Built with [Slint](https://slint.dev) under its Royalty-free License.

## 💙 Credits

Inspired by Furina, from *Genshin Impact* (HoYoverse). Furinar is a fan project with no official affiliation.

<a href="https://slint.dev"><img src="https://raw.githubusercontent.com/slint-ui/slint/master/logo/MadeWithSlint-logo-whitebg.png" width="180" alt="Made with Slint" /></a>
