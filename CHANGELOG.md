# Changelog

Todas as mudanças notáveis neste projeto serão documentadas neste arquivo.

O formato é baseado no [Keep a Changelog](https://keepachangelog.com/),
e o projeto adere ao [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Alterado

- Tela de informações da faixa mais clara: um texto diz o que fazer, o botão virou "Procurar na internet", a confiança aparece em palavras (Certeza, Provável, Talvez) e o resultado escolhido mostra "o que muda" (título, artista, álbum: atual → novo). Os dois botões "Aplicar" / "Aplicar e gravar nas tags" viraram um só, "Usar estes dados", com a caixinha "Também salvar no arquivo" explicada logo abaixo; a escolha fica lembrada.

### Corrigido

- "Buscar na web" às vezes terminava em "erro HTTP 400": o AcoustID recusa (com 400) impressão de áudio curto demais e duração 0, que o Furinar mandava quando o arquivo não informa a duração (MP3 sem cabeçalho Xing, M4A fragmentado de download). Agora usa a duração do áudio decodificado e não consulta com menos de 10 s. Erros de serviço mostram o motivo que ele deu (ex.: "HTTP 400: invalid fingerprint").
- Sem chave do AcoustID, a busca que não acha nada pelo nome diz que a busca pelo som precisa da chave, em vez de só "Nada encontrado".

## [1.11.0] — 2026-10-30

### Adicionado

- Dicas (tooltips) nos botões, com a tecla de atalho num "chip" (ex.: Informações da faixa `I`), no modo normal, no imersivo e no mini.
- Avisos rápidos no rodapé: favoritar/desfavoritar, letra encontrada, arquivo aberto de fora, loop, shuffle e temporizador de sono.
- Indicador "tocando agora" (barrinhas animadas) na linha da faixa que toca; só existe enquanto toca.

### Alterado

- Botões encolhem no clique e voltam com uma leve mola; o ícone "salta" quando troca (play/pause, estrela, modos).
- Configurações e tela de informações entram com fade e um leve crescimento; no imersivo e no mini o título e a capa trocam com fade.
- Barras de progresso e volume engrossam no hover e o botão cresce.

## [1.10.0] — 2026-10-26

### Adicionado

- Identificação pelo som (AcoustID) sem configurar nada: os builds oficiais já vêm com a chave de aplicativo do Furinar. Uma chave própria nas configurações continua tendo prioridade.
- "Abrir com o Furinar" e duplo clique em arquivos de áudio/vídeo: a faixa toca na aba que já a contém ou numa aba temporária com a pasta dela (vários arquivos: uma aba só com eles). Com o Furinar já aberto, os arquivos vão para ele em vez de abrir outro player. No Windows o instalador registra o Furinar no "Abrir com" (sem trocar o player padrão).
- Primeira execução: a pasta de música do sistema (`XDG_MUSIC_DIR` / Músicas do Windows) já entra como aba, com subpastas.
- Instalador do Windows: opção de baixar o FFmpeg (conferido por SHA-256) para MKV/WebM/AVI e o vídeo no modo imersivo; ele fica na pasta do Furinar e sai na desinstalação.
- Sem FFmpeg, as configurações dizem o que fica desligado e onde conseguir.
- Pacotes: `ffmpeg` como dependência opcional (deb `Recommends`, rpm `Recommends`, Arch `optdepends`); o `install.sh` avisa quando falta.

### Alterado

- Busca de letras no LRCLIB ligada por padrão em instalações novas. Na busca automática, faixa sem artista (nem na tag, nem no nome do arquivo) ou sem duração só vai se o AcoustID identificá-la com confiança, para não salvar a letra de outra música com título genérico ("Intro", "Faixa 1"). O botão "Baixar letra" continua buscando sempre.

### Corrigido

- O `.desktop` declarava os tipos de áudio, mas o Furinar ignorava o arquivo recebido; agora aceita vários (`%F`), inclui vídeos e passa no `desktop-file-validate` (categoria `AudioVideo`).

## [1.9.1] — 2026-10-25

### Corrigido

- Varrer pastas com MKV/WebM não congela mais a janela a cada vídeo: duração e tags lidas pelo `ffprobe` ficam num cache em disco (`sondas.json`, ao lado do cache de capas), refeito só para arquivo novo ou modificado.
- Sem `ffmpeg`, o seek em MP4 com vídeo não trava mais o áudio: o arquivo é reaberto e avançado numa thread à parte, com silêncio enquanto isso, sem desalinhar a posição.
- Identificação pelo som (AcoustID) funciona em vídeos: MKV/WebM (e MP4 com vídeo) têm o áudio lido pelo `ffmpeg` para a impressão digital, com a duração certa.

### Alterado

- Release só é compilada depois que o CI (lint e testes, Linux e Windows) passa no commit da tag; a 1.8.0 e a 1.9.0 saíram com um aviso do clippy novo, sem problema no app.
- Versão do Rust fixada em `rust-toolchain.toml`, a mesma localmente e no CI.

## [1.9.0] — 2026-10-25

### Adicionado

- Modo "só o vídeo" no modo imersivo (tecla `V` ou botão de vídeo, só em faixa de vídeo): esconde título, letra, controles e o véu escuro, deixando o vídeo inteiro em brilho normal. Mexer o mouse mostra só um botão para voltar; `Esc` também sai.
- Faixas de vídeo têm um ícone próprio na lista da janela normal e um selo na capa no painel da biblioteca do modo imersivo.

### Corrigido

- Duração e seek de MP4 com vídeo: o decodificador calculava a duração pela trilha de vídeo (um clipe de 2 s aparecia com 0,46 s) e todo seek parava nesse ponto, o que também dessincronizava o vídeo de fundo. Com `ffmpeg` no sistema o áudio de vídeos passa por ele (seek exato); sem ele, a duração vem do cabeçalho do arquivo e o seek reabre a faixa e avança até a posição.
- `make sync-version` reescrevia a última entrada de release do metainfo AppStream com a versão nova; agora só acrescenta uma entrada nova (com a data do CHANGELOG) e nunca mexe no histórico, e o `make check-version` cobra a descrição dela.

## [1.8.0] — 2026-10-25

### Adicionado

- Vídeos como faixas: MP4/MOV/M4V entram na biblioteca e tocam direto; MKV/WebM/AVI (áudio Opus etc.) tocam com o `ffmpeg` do sistema, e sem ele nem aparecem na lista. Na janela normal e no mini player toca só o áudio, sem decodificar vídeo. No modo imersivo, o próprio vídeo vira o fundo, em sincronia com a música: pausa junto e acompanha o seek.
- Vídeo sem capa embutida ganha como capa um quadro dele mesmo (via `ffmpeg`, guardado no cache de miniaturas). Título, artista e duração de MKV/WebM vêm do `ffprobe`.

### Alterado

- Metadados AppStream (`furinar.metainfo.xml`) passam a ser licenciados em CC0-1.0, como pedem o Flathub e as lojas de apps do Linux; o código continua BSD-3-Clause.

## [1.7.3] — 2026-10-23

### Adicionado

- O status do Discord mostra a capa da música em vez da logo do Furinar: a do lançamento corrigido no MusicBrainz ou, se não houver, a encontrada no Deezer (título e artista, uma vez por faixa, só com a opção ligada). Sem capa encontrada, continua a logo.

## [1.7.2] — 2026-10-23

### Adicionado

- Instalador para Windows (`furinar-windows-x86_64-setup.exe`): instala só para o usuário, sem pedir administrador, com atalho no menu Iniciar, atalho opcional na área de trabalho e desinstalador em Configurações → Aplicativos. O `.exe` portátil continua disponível.
- Aviso de atualização novo, no canto da janela: mostra as novidades da versão, o progresso do download e, no fim, um botão "Reiniciar agora" que reabre o Furinar na mesma música. Se algo der errado, o erro aparece ali, com "Tentar de novo".
- No Windows instalado, a atualização roda o instalador em silêncio (e reabre o app), mantendo a versão em "Aplicativos" correta.

### Alterado

- No Windows, as configurações passam a ficar em `%APPDATA%\Furinar`. Quem já tem um `furinar_config.json` ao lado do executável continua no modo portátil, sem perder nada.
- O instalador do Linux (`install.sh`) instala só para o usuário, em `~/.local`, sem sudo (a não ser que faltem bibliotecas do sistema), sempre a versão mais recente, e remove a instalação antiga em `/usr/local`. Assim o Furinar passa a se atualizar sozinho também no Linux.
- Quando o Furinar não pode se atualizar sozinho (pasta sem permissão de escrita), o aviso oferece abrir a página da release.
- As notas de cada release vêm do CHANGELOG.
- Idioma e tema escolhidos em seletores no topo das configurações ("English | Português", "Escuro | Claro"), com a opção atual destacada. Antes eram um botão que mostrava o outro idioma e uma caixa "Tema: Escuro".

### Corrigido

- O aviso de nova versão nunca aparecia: atualizar dependia de baixar a versão nova à mão.
- Em inglês, o cartão de configurações vazava para fora da janela: um rótulo longo não quebrava linha e alargava todo o conteúdo.
- O arquivo `SHA256SUMS` não era anexado às releases.

### Segurança

- O app e o `install.sh` conferem cada download com o `SHA256SUMS` da release antes de instalar; arquivo corrompido ou adulterado é recusado.

## [1.7.1] — 2026-10-22

### Adicionado

- Novas capturas de tela do modo imersivo, da busca de tags e letras e do mini player no README e no site.

### Corrigido

- Falha de segmentação ocasional no Linux ("segmentation fault" ao tocar, buscar na web ou após minutos de uso). A rotina que devolve memória ao sistema podia descartar a tabela de relocação de uma biblioteca que outra thread estava carregando naquele instante (DNS, plugins do PipeWire).

## [1.7.0] — 2026-10-21

### Adicionado
- **Modo imersivo** em tela cheia (botão de tela cheia, `F11` ou `furinar --imersivo`; `Esc` sai):
  - capa da faixa (embutida ou `cover`/`folder`/`front`.* da pasta) e metadados: artista, álbum, ano, formato, taxa, canais;
  - letra inteira rolando suavemente, linha atual em destaque, clique pula para o trecho;
  - fundos personalizados: `background.*` ou `furinar-bg.*` na pasta da música, ou um fundo global nas configurações. Aceita imagem, GIF/WebP animado e vídeo (mp4/webm/mkv/mov/avi, via `ffmpeg` do sistema, se instalado). Sem fundo, a capa desfocada;
  - cor de destaque tirada da capa, crossfade na troca de faixa, controles que somem após 3 s parado;
  - visualizador de espectro (FFT) que reage à música;
  - painel da biblioteca (`L` ou botão; `/` busca): abas das pastas, busca, lista com miniatura da capa, artista • álbum e duração; a faixa tocando ganha um mini equalizador. As miniaturas ficam em cache em `~/.cache/furinar/capas` (`%LOCALAPPDATA%\furinar\capas` no Windows), limitado a 64 MB — as menos usadas saem primeiro;
  - fila "a seguir" com as próximas 5 faixas, respeitando shuffle e loop (o shuffle sorteia adiantado, então o que aparece é o que toca); clique pula para a faixa;
  - modo só letra (`H`): esconde capa, visualizador e controles;
  - relógio e temporizador de sono (15/30/60 min ou fim da faixa), com o volume baixando nos últimos 30 s;
  - `Esc` em camadas: fecha o painel, depois o modo só letra, depois sai.
  - grade de álbuns no painel: clicar num álbum abre uma aba temporária com ele, na ordem do disco, e toca;
  - aviso "a seguir" nos últimos 10 s da faixa;
  - fundo que pulsa com os graves (opção nas configurações);
  - configurações próprias da tela cheia (ReplayGain, fundo reativo, crossfade, fundo), no visual escuro do modo; `Esc` fecha;
  - sair e configurações no canto superior esquerdo, biblioteca no direito (do lado de onde a gaveta abre); o rodapé fica só com os controles da música;
  - rodízio de fundos: vários globais (o seletor aceita vários arquivos) ou vários `background*`/`furinar-bg*` na pasta trocam a cada faixa, nunca repetindo o anterior.
- Nada do modo imersivo fica alocado fora dele; ao sair, a memória é devolvida.
- Reprodução sem silêncio entre faixas (gapless): a próxima entra na fila do player 2 s antes do fim.
- Crossfade opcional de 3, 6 ou 10 s (configurações). Faixas seguidas do mesmo álbum, em ordem, emendam sem crossfade.
- ReplayGain (ganho da faixa, ou do álbum na falta; limitado pelo pico). Ligado por padrão.
- Favoritas (estrela na linha, no painel, no modo imersivo ou tecla `F`) e Mais tocadas (contada depois de metade da faixa ou 4 min), como abas fixas antes das pastas. Guardadas em `furinar_biblioteca.json`, ao lado da configuração.
- Ordenação por aba (padrão, título, artista, álbum, duração, adicionadas recentemente, mais tocadas), lembrada por pasta.
- Letra embutida nas tags quando não há `.lrc`: SYLT (MP3) e USLT/LYRICS/©lyr; texto em LRC sincroniza, texto puro aparece inteiro, sem destaque.
- **Mini player** (botão na barra de título ou `M`): a janela encolhe numa barra com capa, título e anterior/play/próxima, sempre por cima (no Windows e X11; no Wayland o compositor decide).
- **Discord**: "Ouvindo" com título, artista, barra de progresso e capa (Cover Art Archive, quando conhecida). Opção nas configurações; pausado limpa o status.
- **Scrobble** no ListenBrainz (token) e no Last.fm (chave e segredo da API account, conexão pelo navegador): "tocando agora" e cada faixa ouvida (metade ou 4 min). Envios que falham por rede ficam guardados e vão depois.
- **Letra estilo karaokê** na tela cheia: a linha atual enche no ritmo do canto — exata com LRC estendido (`<mm:ss.xx>` por palavra), estimada nas letras só com tempo por linha.
- Temporizador de sono também no modo normal (botão da lua), com o restante ao lado do tempo.
- **Metadados e letras da web** (botão de informações ou tecla `I`, nos dois modos):
  - identificação pelo MusicBrainz (título, artista, álbum, ano, número da faixa) com nota de confiança; sem tags, tenta o nome do arquivo ("Artista - Título", "Artista 'Título'");
  - com chave do AcoustID nas configurações, identifica também pelo som (impressão digital Chromaprint em Rust puro) quando o texto não basta;
  - capa do Cover Art Archive, usada quando a faixa e a pasta não têm capa;
  - as correções ficam num banco local (`furinar_metadados.json`) e são aplicadas por cima das tags; "Restaurar original" desfaz. Gravar nas tags dos arquivos só com confirmação (a faixa tocando fica de fora);
  - lote da aba inteira: acertos com 90% ou mais entram sozinhos, o resto vai para revisão; baixa também as letras que faltam;
  - letras do LRCLIB: sincronizada vira `.lrc` ao lado da faixa (ou no cache, se a pasta não aceitar escrita); só texto vai para o cache. Busca automática ao tocar é opcional e vem desligada.
  - a thread e o cliente HTTP só existem enquanto há trabalho.

### Alterado
- Instalador (`install.sh`) com menu interativo para instalar ou desinstalar, e mensagens em inglês
- Idioma padrão agora é inglês (`en`); antes era português (`pt-br`). Configs já salvas com `pt-br` continuam em português
- Fundo reativo do modo imersivo vai para a tela a no máximo ~15 fps e só quando o pulso muda o bastante (cada mudança repinta a tela inteira; antes custava ~20% de um núcleo). Passa a vir desligado em instalações novas, com o aviso "usa mais CPU".
- Depois de abrir o dispositivo de áudio, o código das bibliotecas de áudio que só serviu à inicialização volta ao sistema: tocando, ~21 MB em vez de ~27 MB.
- Ícones de loop e shuffle mostram o modo: desligado fica apagado; loop da faixa tem um "1"; shuffle inteligente (cada faixa uma vez) usa as setas e o aleatório (pode repetir) um dado. O ciclo do shuffle passa a ser desligado → inteligente → aleatório.
- Modo imersivo: com os controles escondidos, o visualizador desce até o pé da tela e a capa vem junto.
- Ao reabrir, o app volta **pausado** na última faixa (antes começava a tocar), e o dispositivo de áudio só abre no Play. A faixa é lembrada pelo caminho, não pela posição na lista (uma faixa nova na pasta não troca mais a música retomada), junto com a fila do shuffle.
- O scan lê também álbum, número da faixa, duração e ReplayGain (para ordenar e agrupar sem abrir o arquivo de novo).
- A miniatura em cache não abre mais a faixa: álbum e duração já vêm do scan.
- A posição da música vem do player, não de um contador somado a cada 250 ms.
- Perfil de release com LTO, `codegen-units = 1` e `strip`: ~2 MB a menos de RSS
- A saída de áudio é aberta uma vez e reaproveitada entre seeks e trocas de faixa (antes era reaberta a cada seek)
- A duração do MP3 é calculada uma vez por faixa, não a cada seek (o cálculo varre o arquivo inteiro)
- O resultado da checagem de update chega direto no event loop; sai o timer que acordava a cada 250 ms durante toda a execução
- O volume não é mais gravado no config a cada evento do slider; é salvo ao fechar
- Linux (glibc): no máximo 2 arenas de malloc e `malloc_trim` depois da checagem de update
- Linux: páginas limpas de código e fontes são devolvidas ao sistema (`madvise`) 5 s após abrir, depois da checagem de update e ao minimizar — RSS cai de ~52 MB para ~29 MB

### Corrigido
- No Wayland (GNOME/KDE), o Alt+Tab e a dock mostravam ícone genérico e "Desconhecido": a janela não informava o app id que liga ao `furinar.desktop`
- "Anterior" com shuffle voltava à vizinha na lista; agora volta à faixa que tocou antes (histórico), e "próxima" retorna à de onde se voltou
- Clicar numa opção das configurações fechava o menu (agora é um cartão sobre a janela, com rolagem, fechado com Esc ou clique fora)
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

[Unreleased]: https://github.com/LokKol17/furinar/compare/v1.7.3...HEAD
[1.7.3]: https://github.com/LokKol17/furinar/compare/v1.7.2...v1.7.3
[1.7.2]: https://github.com/LokKol17/furinar/compare/v1.7.1...v1.7.2
[1.7.1]: https://github.com/LokKol17/furinar/compare/v1.7.0...v1.7.1
[1.7.0]: https://github.com/LokKol17/furinar/compare/v1.5.3...v1.7.0
[1.5.3]: https://github.com/LokKol17/furinar/compare/v1.5.2...v1.5.3
[1.4.0]: https://github.com/LokKol17/furinar/compare/v0.1.0...v1.4.0
[0.1.0]: https://github.com/LokKol17/furinar/releases/tag/v0.1.0
