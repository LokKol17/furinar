# Changelog

Todas as mudanças notáveis neste projeto serão documentadas neste arquivo.

O formato é baseado no [Keep a Changelog](https://keepachangelog.com/),
e o projeto adere ao [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Adicionado
- **Auto-update** via GitHub Releases — checa novas versões no startup e mostra popup para atualizar
- Suporte a **Linux** — compila e roda em Debian/Ubuntu, Arch/Manjaro e Fedora
- Scripts de instalação: `pkg/install.sh` (universal), `.deb`, `.rpm`, `PKGBUILD`
- `Makefile` com targets `deb`, `rpm`, `tarball`, `release`, `install`
- Desktop entry e metadados AppStream para integração com launchers Linux
- Documentação de instalação em README (EN e PT-BR)

### Corrigido
- `in property` no popup de update mudado para `in-out property` (botão "Depois" não fechava)

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

[Unreleased]: https://github.com/LokKol17/furinar/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/LokKol17/furinar/releases/tag/v0.1.0
