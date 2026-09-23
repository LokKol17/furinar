# Changelog

Todas as mudanças notáveis neste projeto serão documentadas neste arquivo.

O formato é baseado no [Keep a Changelog](https://keepachangelog.com/),
e o projeto adere ao [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Alterado
- Idioma padrão agora é inglês (`en`); antes era português (`pt-br`). Configs já salvas com `pt-br` continuam em português

## [1.5.3] — 2026-09-23

### Adicionado
- `make sync-version` e `make check-version` para sincronizar a versão do `Cargo.toml` com os arquivos de packaging
- Screenshots no AppStream metadata (GNOME Software / KDE Discover)
- Seção de configuração Linux no README

### Corrigido
- `install.sh` agora instala o ícone do app (antes o `.desktop` referenciava um ícone inexistente)
- Target `release` do Makefile gerava o binário Linux com nome `.exe`
- Versões de packaging dessincronizadas do `Cargo.toml`
- Config agora é salvo em `~/.config/furinar/` (XDG) no Linux, em vez do diretório de trabalho

### Alterado
- URLs de `raw.githubusercontent.com` apontam para o branch `master`
- MIME types adicionais no `.desktop` (`audio/x-wav`, `audio/x-m4a`)
- Convenção de release documentada em `docs/RELEASE_CONVENTION.md`

## [1.5.1] — 2026-09-22

### Adicionado
- **i18n** — suporte a múltiplos idiomas (PT-BR e English)
- Toggle de idioma no popup de configurações

### Corrigido
- Auto-update: Timer de polling estava sendo dropped prematuramente (dentro de bloco `{}`)

## [1.5.0] — 2026-09-22

### Adicionado
- **Auto-update** via GitHub Releases
- Suporte a **Linux**
- Scripts de instalação, Makefile, GitHub Actions CI
- Landing page do projeto
- Changelog e convenção de releases

### Corrigido
- `in property` no popup de update mudado para `in-out property`
- `install.sh` funciona via `curl` (gera arquivos inline)
- Bloco `HWND`/`forcar_repaint_completo` protegido com `#[cfg]` para Linux

## [0.1.0] — 2026-09-21

### Adicionado
- Reprodução: MP3, WAV, FLAC, OGG/Vorbis e M4A
- Play/Pause, Stop, Previous, Next, seek, volume
- Loop (desligado / faixa / playlist) e Shuffle
- Múltiplas pastas como abas — trocar de aba não interrompe a reprodução
- Escaneamento opcional de subpastas
- Leitura de tags ID3/Vorbis (título e artista, fallback para nome do arquivo)
- Busca/filtro por título ou artista, insensível a acentos e caixa
- Letras sincronizadas via arquivos `.lrc`
- Tema claro/escuro, alternável instantaneamente
- Janela sem moldura com barra de título customizada (drag-to-move, Aero Snap, cantos arredondados no Win11)
- Controles multimídia nativos: SMTC (teclas de mídia, Action Center)
- Botões de play/pause/next na miniatura da taskbar (Windows)
- Atalhos de teclado: Space, ←/→, ↑/↓, Enter
- Configuração persistente (pasta, faixa, posição, preferências)
- Retoma última faixa e posição ao reabrir
- Migración automática do campo `pasta` → `pastas` na config

[Unreleased]: https://github.com/LokKol17/furinar/compare/v1.5.3...HEAD
[1.5.3]: https://github.com/LokKol17/furinar/compare/v1.5.2...v1.5.3
[1.4.0]: https://github.com/LokKol17/furinar/compare/v0.1.0...v1.4.0
[0.1.0]: https://github.com/LokKol17/furinar/releases/tag/v0.1.0
