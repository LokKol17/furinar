# Changelog

Todas as mudanças notáveis neste projeto serão documentadas neste arquivo.

O formato é baseado no [Keep a Changelog](https://keepachangelog.com/),
e o projeto adere ao [Semantic Versioning](https://semver.org/).

## [Unreleased]

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

[Unreleased]: https://github.com/LokKol17/furinar/compare/v1.4.0...HEAD
[1.4.0]: https://github.com/LokKol17/furinar/compare/v0.1.0...v1.4.0
[0.1.0]: https://github.com/LokKol17/furinar/releases/tag/v0.1.0
