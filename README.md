# Furinar

Player de áudio leve e rápido para desktop.

## Visão geral

Furinar é um player de áudio minimalista que prioriza **consumo mínimo de recursos** — tipicamente entre **2–7 MB de RAM** em uso. Abra uma pasta com arquivos de áudio, selecione uma faixa e ouça sem distrações.

### Formatos suportados

MP3, WAV, FLAC, OGG/Vorbis, M4A

### Funcionalidades

- **Playlist por pasta** — escanea los archivos de audio de un directorio y los ordena alfabéticamente (opcionalmente incluye subcarpetas)
- **Controles básicos** — Play/Pause, Stop, Anterior, Próxima
- **Loop** — desligado, repetir faixa o repetir toda la playlist
- **Shuffle** — reproducción aleatoria
- **Seek** — barra de progreso con búsqueda temporal
- **Volume** — control deslizante con persistencia
- **Menú de configuración** — opción para escanear subcarpetas recursivamente
- **Controles multimídia del sistema** — play/pause, anterior/próxima, stop y seek por los botones de medios del teclado (SMTC en Windows, ej.: centro de acciones y teclas de medios)
- **Tags ID3/Vorbis** — lê título e artista das tags e exibe "Artista - Título" na lista (com fallback para o nome do arquivo quando não há tag)
- **Busca/filtro** — filtra a playlist por título ou artista, ignorando acentos e maiúsculas; Anterior/Próxima navegam dentro do resultado filtrado
- **Múltiplas pastas em abas** — cada pasta aberta vira uma aba; navegar entre abas não interfere na reprodução, que continua na pasta de onde a faixa veio (indicada por um ponto dourado na aba)
- **Letras sincronizadas (.lrc)** — botão na barra de controles abre um painel que rola junto com a faixa; arquivos `.lrc` irmãos do áudio são lidos sob demanda, e sem `.lrc` o painel mostra uma mensagem simples
- **Atalhos de teclado** — Espaço pausa/retoma, ←/→ pulam -5s/+5s, ↑/↓ navegam pela lista (com rolagem automática) e Enter toca a faixa selecionada
- **Botões na miniatura da barra de tarefas** — Anterior, Play/Pause e Próxima aparecem no preview do ícone (ITaskbarList3), com o ícone central acompanhando o estado de reprodução
- **Barra de título própria** — janela sem moldura nativa, com barra no tema do app (arrastar com Aero Snap, minimizar e fechar) e cantos arredondados no Windows 11
- **Configuración persistente** — carpeta, volumen, modo de loop, shuffle, escaneo de subcarpetas, pista actual y posición se guardan automáticamente
- **Configuração persistente** — pasta, volume, modo de loop, shuffle, faixa atual e posição são salvos automaticamente
- **Tema claro/escuro** — switch nas configurações com troca instantânea; a escolha é lembrada entre execuções e aplicada antes da primeira renderização (sem piscar)

### Ícone

O ícone do app fica em `ui/assets/furinar_icon.png`. Não precisa ser quadrado: o `build.rs` recorta as bordas transparentes e centraliza o conteúdo num quadrado de 256px (preservando a proporção, sem esticar), gerando `ui/assets/furinar_icon_quadrado.png`. Esse derivado alimenta tanto o ícone da janela quanto o `.ico` multi-resolução (16/32/48/256) embutido no executável. Substitua o arquivo de origem pelo seu — o build cuida do resto.

## Requisitos

- Rust (edition 2024)
- Windows

## Compilar e rodar

```bash
cargo run
```

Para gerar a versão otimizada:

```bash
cargo build --release
```

O binário gerado fica em `target/release/furinar.exe`.

## Configuração

As preferências são salvas automaticamente no arquivo `furinar_config.json`, na raiz do projeto:

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

O campo antigo `pasta` (singular) ainda é lido — se `pastas` estiver vazio e ele existir, é migrado automaticamente para a lista.

Edite o arquivo manualmente se necessário — o player lê as configurações ao iniciar.

Ao reabrir, o player restaura automaticamente a última faixa tocada e a posição de reprodução.
