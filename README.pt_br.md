<div align="center">

<img src="ui/assets/furinar_icon.png" width="128" height="128" alt="Furinar" />

# Furinar

**Um player de áudio leve, rápido e bonito — inspirado na Hydro Archon.**

![Windows](https://img.shields.io/badge/plataforma-Windows-3da9d6?style=flat-square)
![Rust](https://img.shields.io/badge/feito%20com-Rust-c9a050?style=flat-square)
![RAM](https://img.shields.io/badge/RAM-2--10MB-0b1a2b?style=flat-square)
![Licença](https://img.shields.io/badge/licença-BSD--3--Clause-lightgrey?style=flat-square)

[🇺🇸 English](README.md) | 🇧🇷 Português

</div>

---

<!--
  Sugestão: coloque uma screenshot real do app aqui antes de publicar.
  Ex: ![Furinar em ação](docs/screenshot.png)
-->

## Por que Furinar?

A maioria dos players de música pra desktop hoje em dia carrega um navegador inteiro por baixo do capô só pra tocar um MP3. O Furinar faz o oposto: abre em milissegundos e some no fundo usando **2 a 10 MB de RAM** — menos que uma aba do seu navegador gasta parada — e ainda assim tem tudo que um player de verdade precisa: tags, letras sincronizadas, múltiplas bibliotecas, integração nativa com o Windows.

A paleta visual — azul-marinho profundo, ciano e dourado (ou azul-bebê e branco pérola, no tema claro) — é uma homenagem à **Furina**, de *Genshin Impact*. Sem relação oficial com a HoYoverse, só um tributo de fã.

## ✨ Funcionalidades

**Reprodução**
- Suporte a MP3, WAV, FLAC, OGG/Vorbis e M4A
- Play/Pause, Stop, Anterior, Próxima, Seek e controle de volume
- Loop (desligado / faixa / playlist) e Shuffle
- Retoma automaticamente a última faixa e posição ao reabrir o app

**Organização**
- **Múltiplas pastas em abas** — abra quantas bibliotecas quiser; navegar entre abas nunca interrompe o que está tocando
- Escaneamento opcional de subpastas
- Leitura de tags **ID3/Vorbis** (título e artista, com fallback pro nome do arquivo)
- Busca com filtro por título/artista, ignorando acentos e maiúsculas

**Extras**
- **Letras sincronizadas** via arquivos `.lrc`, rolando junto com a música
- **Tema claro/escuro**, trocado na hora, sem reiniciar o app
- Janela sem moldura nativa, com barra de título própria (arraste, Aero Snap, cantos arredondados no Windows 11)

**Integração com o Windows**
- Botões de **Play/Pause, Anterior e Próxima** direto na miniatura da barra de tarefas
- Controles multimídia do sistema (teclas de mídia do teclado, central de ações) via SMTC
- Atalhos de teclado: `Espaço` pausa/retoma, `←`/`→` pulam 5s, `↑`/`↓` navegam a lista, `Enter` toca a faixa selecionada

## 🚀 Como usar

1. Baixe o `furinar.exe` mais recente (ou compile — veja abaixo).
2. Abra o app e clique em **Abrir pasta** pra apontar pra onde estão suas músicas.
3. Pronto — o Furinar lembra tudo sozinho da próxima vez que você abrir.

---

## Para desenvolvedores

<details>
<summary>Compilar e rodar</summary>

**Requisitos:** Rust (edition 2024) e Windows.

```bash
cargo run
```

Build otimizada:

```bash
cargo build --release
```

O binário final fica em `target/release/furinar.exe`.

</details>

<details>
<summary>Ícone do app</summary>

O ícone fica em `ui/assets/furinar_icon.png`. Não precisa ser quadrado: o `build.rs` recorta as bordas transparentes e centraliza o conteúdo num quadrado de 256px (preservando a proporção, sem esticar), gerando `ui/assets/furinar_icon_quadrado.png`. Esse derivado alimenta tanto o ícone da janela quanto o `.ico` multi-resolução (16/32/48/256) embutido no executável.

Pra trocar o ícone, é só substituir o arquivo de origem — o build cuida do resto.

</details>

<details>
<summary>Arquivo de configuração</summary>

As preferências são salvas automaticamente em `furinar_config.json`, na raiz do projeto:

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

O campo antigo `pasta` (singular, de versões anteriores) ainda é lido: se `pastas` estiver vazio e ele existir, é migrado automaticamente pra lista na primeira execução.

Dá pra editar o arquivo manualmente — o player relê as configurações a cada início.

</details>

## 📄 Licença

O código-fonte do Furinar é licenciado sob [BSD 3-Cláusula](LICENSE) — você é livre pra usar, fazer fork e modificar, desde que os créditos sejam mantidos e o aviso de licença permaneça intacto.

Construído com [Slint](https://slint.dev), sob a licença Royalty-free.

## 💙 Créditos

Inspirado na Furina, de *Genshin Impact* (HoYoverse). Furinar é um projeto de fã, sem afiliação oficial.

<a href="https://slint.dev">
    <img src="https://raw.githubusercontent.com/slint-ui/slint/master/logo/MadeWithSlint-logo-whitebg.png" width="180" alt="Made with Slint" />
</a>
