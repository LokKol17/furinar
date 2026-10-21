# Changelog

Todas as mudanças notáveis neste projeto serão documentadas neste arquivo.

O formato é baseado no [Keep a Changelog](https://keepachangelog.com/),
e o projeto adere ao [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Adicionado
- **Modo imersivo** em tela cheia (botão de tela cheia, `F11` ou `furinar --imersivo`; `Esc` sai):
  - capa da faixa (embutida ou `cover`/`folder`/`front`.* da pasta) e metadados: artista, álbum, ano, formato, taxa, canais;
  - letra inteira rolando suavemente, linha atual em destaque, clique pula para o trecho;
  - fundos personalizados: `background.*` ou `furinar-bg.*` na pasta da música, ou um fundo global nas configurações. Aceita imagem, GIF/WebP animado e vídeo (mp4/webm/mkv/mov/avi, via `ffmpeg` do sistema, se instalado). Sem fundo, a capa desfocada;
  - cor de destaque tirada da capa, crossfade na troca de faixa, controles que somem após 3 s parado;
  - visualizador de espectro (FFT) que reage à música;
  - painel da biblioteca (`L` ou botão; `/` busca): abas das pastas, busca, lista com miniatura da capa, artista • álbum e duração; a faixa tocando ganha um mini equalizador. As miniaturas ficam em cache em `~/.cache/furinar/capas` (`%LOCALAPPDATA%\furinar\capas` no Windows);
  - fila "a seguir" com as próximas 5 faixas, respeitando shuffle e loop (o shuffle sorteia adiantado, então o que aparece é o que toca); clique pula para a faixa;
  - modo só letra (`H`): esconde capa, visualizador e controles;
  - relógio e temporizador de sono (15/30/60 min ou fim da faixa), com o volume baixando nos últimos 30 s;
  - `Esc` em camadas: fecha o painel, depois o modo só letra, depois sai.
- Nada do modo imersivo fica alocado fora dele; ao sair, a memória é devolvida.

### Alterado
- Perfil de release com LTO, `codegen-units = 1` e `strip`: ~2 MB a menos de RSS
- A saída de áudio é aberta uma vez e reaproveitada entre seeks e trocas de faixa (antes era reaberta a cada seek)
- A duração do MP3 é calculada uma vez por faixa, não a cada seek (o cálculo varre o arquivo inteiro)
- O resultado da checagem de update chega direto no event loop; sai o timer que acordava a cada 250 ms durante toda a execução
- O volume não é mais gravado no config a cada evento do slider; é salvo ao fechar
- Linux (glibc): no máximo 2 arenas de malloc e `malloc_trim` depois da checagem de update
- Linux: páginas limpas de código e fontes são devolvidas ao sistema (`madvise`) 5 s após abrir, depois da checagem de update e ao minimizar — RSS cai de ~52 MB para ~29 MB

### Corrigido
- O estado "baixando" da atualização nunca era limpo: a thread de download chamava `upgrade()` fora da thread da UI

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
