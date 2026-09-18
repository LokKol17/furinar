# Furinar

Player de áudio leve e rápido para desktop.

## Visão geral

Furinar é um player de áudio minimalista que prioriza **consumo mínimo de recursos** — tipicamente entre **2–7 MB de RAM** em uso. Abra uma pasta com arquivos de áudio, selecione uma faixa e ouça sem distrações.

### Formatos suportados

MP3, WAV, FLAC, OGG/Vorbis, M4A

### Funcionalidades

- **Playlist por pasta** — escaneia os arquivos de áudio de um diretório e ordena alfabeticamente
- **Controles básicos** — Play/Pause, Stop, Anterior, Próxima
- **Loop** — desligado, repetir faixa ou repetir toda a playlist
- **Shuffle** — reprodução aleatória
- **Seek** — barra de progresso com busca temporal
- **Volume** — controle deslizante com persistência
- **Controles multimídia do sistema** — play/pause, anterior/próxima, stop e seek pelos botões de mídia do teclado (SMTC no Windows, ex.: centro de ações e teclas de mídia)
- **Configuração persistente** — pasta, volume, modo de loop, shuffle, faixa atual e posição são salvos automaticamente

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
  "pasta": "E:\\Music",
  "volume": 0.11666667,
  "modo_loop": 2,
  "shuffle": true,
  "indice_atual": 5,
  "tempo_atual": 42
}
```

Edite o arquivo manualmente se necessário — o player lê as configurações ao iniciar.

Ao reabrir, o player restaura automaticamente a última faixa tocada e a posição de reprodução.
