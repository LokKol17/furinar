//! Reprodução: fonte e player, seek, próxima/anterior (shuffle, histórico),
//! gapless e crossfade, letra sincronizada, volume e temporizador de sono.

use crate::*;

/// Opções do crossfade, em segundos.
pub(crate) const OPCOES_CROSSFADE: [u8; 4] = [0, 3, 6, 10];

pub(crate) fn proximo_crossfade(atual: u8) -> u8 {
    let pos = OPCOES_CROSSFADE
        .iter()
        .position(|&o| o == atual)
        .unwrap_or(0);
    OPCOES_CROSSFADE[(pos + 1) % OPCOES_CROSSFADE.len()]
}

pub(crate) fn publicar_crossfade(estado: &EstadoAudio, ui: &MainWindow) {
    let lang = translations::get_translations(&estado.idioma);
    let rotulo = lang.get("crossfade").copied().unwrap_or("crossfade");
    let valor = if estado.crossfade == 0 {
        lang.get("crossfade_off").copied().unwrap_or("").to_string()
    } else {
        format!("{} s", estado.crossfade)
    };
    ui.set_crossfade_texto(format!("{rotulo}: {valor}").into());
}

/// Carrega a letra da faixa e já publica o primeiro recorte: o `.lrc` ao
/// lado da faixa ou, na falta dele, a letra embutida nas tags. Não ter letra
/// é o caso normal ("sem letra disponível"), não uma falha.
pub(crate) fn carregar_letra(
    estado: &mut EstadoAudio,
    ui: &MainWindow,
    caminho_faixa: &std::path::Path,
    tempo: f64,
) {
    let chave = biblioteca::chave_caminho(caminho_faixa);
    // O texto do LRC (ao lado da faixa ou no cache) traz também os tempos
    // das palavras, quando é LRC estendido
    let texto_lrc = fs::read_to_string(caminho_lrc(caminho_faixa))
        .ok()
        .or_else(|| letras_web::lrc_do_cache(&chave));
    let (letra, palavras) = match texto_lrc {
        Some(conteudo) => (
            lrc::Letra::Sincronizada(parse_lrc(&conteudo)),
            lrc::palavras_lrc(&conteudo),
        ),
        None => (
            letras_web::do_cache(&chave).unwrap_or_else(|| lrc::letra_embutida(caminho_faixa)),
            Vec::new(),
        ),
    };
    estado.letra_palavras = palavras;

    estado.letra_livre = matches!(&letra, lrc::Letra::Livre(l) if !l.is_empty());
    estado.letra_atual = letra.linhas();
    ui.set_letra_livre(estado.letra_livre);
    estado.letra_indice = if estado.letra_livre {
        -1
    } else {
        indice_letra(&estado.letra_atual, tempo)
    };
    // O modo imersivo rola a letra inteira; o painel normal usa só o recorte
    let completa: Vec<SharedString> = estado
        .letra_atual
        .iter()
        .map(|(_, t)| t.as_str().into())
        .collect();
    ui.set_letra_completa(ModelRc::new(VecModel::from(completa)));
    publicar_letra(estado, ui);
}

/// Recalcula a linha atual e só mexe na UI quando ela muda.
pub(crate) fn sincronizar_letra(estado: &mut EstadoAudio, ui: &MainWindow) {
    if estado.letra_livre {
        return;
    }
    let indice = indice_letra(&estado.letra_atual, estado.tempo_decorrido);
    if indice == estado.letra_indice {
        return;
    }
    estado.letra_indice = indice;
    publicar_letra(estado, ui);
}

/// Manda pra UI o recorte de linhas ao redor da linha atual.
pub(crate) fn publicar_letra(estado: &mut EstadoAudio, ui: &MainWindow) {
    let (inicio, janela, destaque) = janela_letra(&estado.letra_atual, estado.letra_indice);
    estado.letra_janela_inicio = inicio;
    ui.set_letra(ModelRc::new(VecModel::from(janela)));
    ui.set_letra_destaque(destaque);
    ui.set_letra_indice(estado.letra_indice);
}

/// Abre o decodificador certo para a extensão do arquivo.
///
/// `byte_len` (tamanho do arquivo) é obrigatório: sem ele o symphonia trata a
/// fonte como não-seekable e o reader isomp4 do M4A falha logo em `try_new` —
/// no rodio 0.19 esse erro virava `unreachable!()` e derrubava o app. O hint de
/// extensão escolhe o reader do container direto, sem depender do probe.
pub(crate) fn criar_decodificador<R>(
    leitor: R,
    extensao: &str,
    byte_len: u64,
) -> Result<Decoder<R>, DecoderError>
where
    R: std::io::Read + std::io::Seek + Send + Sync + 'static,
{
    Decoder::builder()
        .with_data(leitor)
        .with_byte_len(byte_len)
        .with_seekable(true)
        .with_hint(extensao)
        .build()
}

/// Fonte pronta para o player: decodificador com ganho/transição e espião
/// do visualizador.
pub(crate) type Fonte = visualizador::Espiao<transicao::Transicao<Decodificado>>;

/// Decodificador da faixa: o symphonia, ou o `ffmpeg` do sistema para o que
/// ele não abre (vídeos MKV/WebM/AVI, Opus...).
pub(crate) enum Decodificado {
    /// `video`: MP4/MOV com vídeo tocado sem ffmpeg (ver `VideoNativo`)
    Nativo {
        d: Decoder<BufReader<File>>,
        video: Option<VideoNativo>,
    },
    Ffmpeg(audio_ffmpeg::FonteFfmpeg),
}

/// MP4 com trilha de vídeo pelo symphonia: o rodio pega a trilha padrão do
/// container (a de vídeo) para calcular a duração e limita o seek a ela —
/// num clipe de 2 s dava 0,46 s, e todo seek parava ali. Aqui a duração vem
/// do cabeçalho (lofty) e o seek reabre o arquivo e descarta até a posição.
pub(crate) struct VideoNativo {
    caminho: PathBuf,
    duracao: Option<Duration>,
    /// Seek em andamento: o decodificador já na posição chega por aqui. Até
    /// lá a fonte toca silêncio, em vez de travar a thread de áudio.
    seek: Option<std::sync::mpsc::Receiver<Decoder<BufReader<File>>>>,
    /// Amostras de silêncio já entregues nesse seek: descartadas do novo
    /// decodificador, para a música não ficar atrás da posição do player.
    silencio: u64,
}

impl VideoNativo {
    fn novo(caminho: &Path) -> Self {
        Self {
            caminho: caminho.to_path_buf(),
            duracao: track::duracao_container(caminho),
            seek: None,
            silencio: 0,
        }
    }
}

/// Avança `amostras` (arredondado aos canais, para não trocar esquerda e
/// direita). Decodificar AAC é centenas de vezes mais rápido que tocar.
fn pular_amostras(d: &mut Decoder<BufReader<File>>, amostras: u64) {
    let canais = d.channels().get() as u64;
    for _ in 0..amostras - amostras % canais {
        if d.next().is_none() {
            break;
        }
    }
}

fn abrir_nativo(caminho: &Path) -> Option<Decoder<BufReader<File>>> {
    let extensao = caminho
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let arquivo = File::open(caminho).ok()?;
    let byte_len = arquivo.metadata().ok()?.len();
    criar_decodificador(BufReader::new(arquivo), &extensao, byte_len).ok()
}

impl Decodificado {
    /// Abre a faixa. Vídeo vai pelo `ffmpeg` quando há um no PATH (seek e
    /// duração exatos); o resto pelo symphonia, com o `ffmpeg` de reserva.
    pub(crate) fn abrir(caminho: &Path) -> Option<Self> {
        let video = ffmpeg::e_video(caminho);
        if video
            && ffmpeg::disponivel()
            && let Some(f) = audio_ffmpeg::FonteFfmpeg::abrir(caminho)
        {
            return Some(Self::Ffmpeg(f));
        }
        if !ffmpeg::so_com_ffmpeg(caminho)
            && let Some(d) = abrir_nativo(caminho)
        {
            let video = video.then(|| VideoNativo::novo(caminho));
            return Some(Self::Nativo { d, video });
        }
        audio_ffmpeg::FonteFfmpeg::abrir(caminho).map(Self::Ffmpeg)
    }
}

impl Iterator for Decodificado {
    type Item = rodio::Sample;

    fn next(&mut self) -> Option<rodio::Sample> {
        match self {
            Self::Nativo { d, video: Some(v) } if v.seek.is_some() => {
                let rx = v.seek.as_ref()?;
                match rx.try_recv() {
                    Ok(mut novo) => {
                        pular_amostras(&mut novo, v.silencio);
                        *d = novo;
                        v.seek = None;
                        d.next()
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        v.silencio += 1;
                        Some(0.0)
                    }
                    // Não reabriu: segue de onde estava
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        v.seek = None;
                        d.next()
                    }
                }
            }
            Self::Nativo { d, .. } => d.next(),
            Self::Ffmpeg(f) => f.next(),
        }
    }
}

impl rodio::Source for Decodificado {
    fn current_span_len(&self) -> Option<usize> {
        match self {
            Self::Nativo { d, .. } => d.current_span_len(),
            Self::Ffmpeg(f) => f.current_span_len(),
        }
    }

    fn channels(&self) -> rodio::ChannelCount {
        match self {
            Self::Nativo { d, .. } => d.channels(),
            Self::Ffmpeg(f) => f.channels(),
        }
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        match self {
            Self::Nativo { d, .. } => d.sample_rate(),
            Self::Ffmpeg(f) => f.sample_rate(),
        }
    }

    fn total_duration(&self) -> Option<Duration> {
        match self {
            Self::Nativo { video: Some(v), .. } => v.duracao,
            Self::Nativo { d, .. } => d.total_duration(),
            Self::Ffmpeg(f) => f.total_duration(),
        }
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
        match self {
            Self::Nativo { d, video: Some(v) } => {
                // Reabre do começo e decodifica até a posição numa thread
                // (o try_seek roda na thread de áudio)
                let por_segundo = d.sample_rate().get() as u128 * d.channels().get() as u128;
                let pular = (pos.as_nanos() * por_segundo / 1_000_000_000) as u64;
                let caminho = v.caminho.clone();
                let (tx, rx) = std::sync::mpsc::sync_channel(1);
                std::thread::Builder::new()
                    .name("furinar-seek".into())
                    .spawn(move || {
                        if let Some(mut novo) = abrir_nativo(&caminho) {
                            pular_amostras(&mut novo, pular);
                            let _ = tx.send(novo);
                        }
                    })
                    .map_err(|e| rodio::source::SeekError::Other(std::sync::Arc::new(e)))?;
                v.seek = Some(rx);
                v.silencio = 0;
                Ok(())
            }
            Self::Nativo { d, .. } => d.try_seek(pos),
            Self::Ffmpeg(f) => f.try_seek(pos),
        }
    }
}

/// Caminho completo da faixa atual, sempre na pasta de onde ela vem (nunca
/// na aba visível).
pub(crate) fn caminho_atual(estado: &EstadoAudio) -> Option<PathBuf> {
    let pasta = estado.pastas.get(estado.pasta_reproducao?)?;
    Some(pasta.caminho.join(estado.arquivo_atual.as_deref()?))
}

/// Fator do ReplayGain de uma faixa (1 com a opção desligada ou sem tag).
pub(crate) fn ganho_de(estado: &EstadoAudio, pasta: usize, indice: Option<usize>) -> f32 {
    if !estado.replaygain {
        return 1.0;
    }
    indice
        .and_then(|i| estado.pastas.get(pasta)?.tracks.get(i)?.ganho)
        .unwrap_or(1.0)
}

/// Abre a faixa e devolve a fonte, a duração (s) e o controle do fade-out.
/// `entrada` é o fade-in (crossfade); zero entra direto.
pub(crate) fn abrir_fonte(
    estado: &EstadoAudio,
    caminho: &Path,
    ganho: f32,
    entrada: Duration,
) -> Option<(Fonte, u64, transicao::Controle)> {
    let extensao = caminho
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let decodificador = Decodificado::abrir(caminho)?;

    // Seek na mesma faixa: a duração já é conhecida
    let total_secs = if estado.duracao_de.as_deref() == Some(caminho) {
        estado.duracao_total_secs
    } else if extensao == "mp3" {
        mp3_duration::from_path(caminho)
            .map(|d| d.as_secs())
            .unwrap_or_else(|_| {
                decodificador
                    .total_duration()
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
            })
    } else {
        decodificador
            .total_duration()
            .map(|d| d.as_secs())
            .unwrap_or(0)
    };

    let controle = transicao::Controle::default();
    let fonte = visualizador::Espiao::new(
        transicao::Transicao::new(decodificador, ganho, entrada, controle.clone()),
        estado.captura.clone(),
    );
    Some((fonte, total_secs, controle))
}

/// Mixer da saída de áudio, abrindo o dispositivo se preciso.
pub(crate) fn mixer(estado: &mut EstadoAudio) -> Option<&rodio::mixer::Mixer> {
    if estado.saida.is_none() {
        estado.saida = DeviceSinkBuilder::open_default_sink().ok();
        // Abrir o dispositivo carrega o código das bibliotecas de áudio
        // (ALSA/PipeWire); passada a inicialização, boa parte não é mais
        // tocada e pode voltar ao sistema (~6 MB)
        Timer::single_shot(Duration::from_secs(4), memoria::aparar);
    }
    estado.saida.as_ref().map(|s| s.mixer())
}

/// Recria o player da faixa atual a partir de `alvo_secs`, tocando ou já
/// pausado (pausa antes do primeiro som: sem estalo no seek pausado).
pub(crate) fn executar_seek(
    estado: &mut EstadoAudio,
    ui: &MainWindow,
    alvo_secs: u64,
    pausado: bool,
) {
    let Some(caminho_completo) = caminho_atual(estado) else {
        return;
    };

    if let Some(antigo) = estado.audio_player.take() {
        antigo.stop();
    }
    if let Some(saindo) = estado.saindo.take() {
        saindo.stop();
    }
    // A próxima que estava na fila do player volta para a fila do shuffle
    if let Some(e) = estado.engatilhada.take() {
        devolver_a_fila(estado, e.indice);
    }
    estado.transicao_feita = false;

    let ganho = ganho_de(
        estado,
        estado.pasta_reproducao.unwrap_or(0),
        estado.indice_atual,
    );
    let Some((fonte, total_secs, controle)) =
        abrir_fonte(estado, &caminho_completo, ganho, Duration::ZERO)
    else {
        return;
    };
    let volume = estado.volume_atual;
    let Some(mixer) = mixer(estado) else {
        return;
    };
    let sink = Player::connect_new(mixer);
    sink.set_volume(volume);
    if pausado {
        sink.pause();
    }
    sink.append(fonte);
    if alvo_secs > 0 {
        let _ = sink.try_seek(Duration::from_secs(alvo_secs));
    }

    estado.duracao_total_secs = total_secs;
    estado.duracao_de = Some(caminho_completo);
    estado.controle = controle;
    estado.tempo_decorrido = alvo_secs as f64;
    estado.ultimo_segundo = alvo_secs;
    estado.audio_player = Some(sink);

    publicar_tempo(estado, ui);
    publicar_play_pause(estado, ui, !pausado);
}

/// Barra e texto de tempo da faixa atual.
pub(crate) fn publicar_tempo(estado: &EstadoAudio, ui: &MainWindow) {
    let total = estado.duracao_total_secs;
    let atual = estado.tempo_decorrido as u64;
    ui.set_progresso(if total > 0 {
        atual as f32 / total as f32
    } else {
        0.0
    });
    let com_horas = total >= 3600;
    ui.set_texto_tempo(
        format!(
            "{} / {}",
            formatar_tempo(atual, com_horas),
            formatar_tempo(total, com_horas)
        )
        .into(),
    );
}

pub(crate) fn publicar_play_pause(estado: &EstadoAudio, ui: &MainWindow, tocando: bool) {
    let t = translations::get_translations(&estado.idioma);
    let texto = if tocando {
        texto_pause(&t)
    } else {
        texto_play(&t)
    };
    ui.set_texto_play_pause(texto.into());
    ui.set_tocando(tocando);
}

/// Passa o estado do app para a faixa `track_idx` da pasta `pasta_idx`: letra,
/// seleção, origem do som. Não mexe no player.
pub(crate) fn marcar_faixa(
    estado: &mut EstadoAudio,
    ui: &MainWindow,
    pasta_idx: usize,
    track_idx: usize,
    alvo_secs: u64,
) -> bool {
    garantir_pasta_carregada(estado, pasta_idx);

    let Some(pasta) = estado.pastas.get(pasta_idx) else {
        return false;
    };
    let Some(caminho) = pasta.tracks.get(track_idx).map(|t| t.path.clone()) else {
        return false;
    };

    // Troca de faixa: descarta a letra anterior e tenta carregar a nova
    let completo = pasta.caminho.join(&caminho);
    carregar_letra(estado, ui, &completo, alvo_secs as f64);

    if estado.pasta_reproducao != Some(pasta_idx) {
        // Pasta de reprodução mudou: a sacola do shuffle inteligente era
        // da pasta antiga, não serve mais aqui.
        estado.sacola_shuffle.clear();
        estado.sorteio.clear();
    }

    let anterior = estado.pasta_reproducao.zip(estado.indice_atual);
    if !estado.voltando
        && anterior != Some((pasta_idx, track_idx))
        && let Some((p, i)) = anterior
        && let Some(chave) = estado.pastas.get(p).and_then(|pasta| pasta.chave_faixa(i))
    {
        estado.historico.push((p, chave));
        if estado.historico.len() > TAMANHO_HISTORICO {
            estado.historico.remove(0);
        }
    }

    estado.pasta_reproducao = Some(pasta_idx);
    estado.indice_atual = Some(track_idx);
    estado.arquivo_atual = Some(caminho);
    estado.contada = false;

    atualizar_selecao_visivel(estado, ui);
    atualizar_favorita_atual(estado, ui);
    // Sem letra nenhuma: pede à web (se a opção estiver ligada)
    if estado.letra_atual.is_empty() && estado.letras_web {
        pedir_letra(estado, false);
    }
    true
}

/// Toca uma faixa de uma pasta específica, começando em `alvo_secs`.
///
/// É este caminho que fixa de onde vem o som: `pasta_reproducao` passa a ser
/// `pasta_idx`, independente da aba que está sendo exibida.
pub(crate) fn iniciar_faixa(
    estado: &mut EstadoAudio,
    ui: &MainWindow,
    pasta_idx: usize,
    track_idx: usize,
    alvo_secs: u64,
) {
    if !marcar_faixa(estado, ui, pasta_idx, track_idx, alvo_secs) {
        return;
    }
    executar_seek(estado, ui, alvo_secs, false);
    atualizar_imersivo(estado, ui);
}

pub(crate) fn tocar_faixa(
    estado: &mut EstadoAudio,
    ui: &MainWindow,
    pasta_idx: usize,
    track_idx: usize,
) {
    iniciar_faixa(estado, ui, pasta_idx, track_idx, 0);
}

/// A próxima faixa já começou no player (gapless, ou o crossfade entrou): o
/// estado passa a ser dela sem recriar nada.
pub(crate) fn assumir_faixa(
    estado: &mut EstadoAudio,
    ui: &MainWindow,
    pasta_idx: usize,
    track_idx: usize,
    duracao: u64,
) {
    if !marcar_faixa(estado, ui, pasta_idx, track_idx, 0) {
        return;
    }
    estado.duracao_total_secs = duracao;
    estado.duracao_de = caminho_atual(estado);
    estado.tempo_decorrido = 0.0;
    estado.ultimo_segundo = 0;
    estado.transicao_feita = false;
    publicar_tempo(estado, ui);
    atualizar_imersivo(estado, ui);
    atualizar_fila(estado);
}

/// O que um seek vindo da UI deve fazer com a faixa atual.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum EfeitoSeek {
    /// Player vivo: recria o player preservando tocar/pausado.
    ManterEstado,
    /// Parado com faixa selecionada (Stop): recarrega e toca do ponto pedido.
    Recarregar,
    /// Nada carregado: não há o que reposicionar.
    Nada,
}

pub(crate) fn efeito_seek(tem_player: bool, tem_faixa: bool) -> EfeitoSeek {
    match (tem_player, tem_faixa) {
        (true, _) => EfeitoSeek::ManterEstado,
        (false, true) => EfeitoSeek::Recarregar,
        (false, false) => EfeitoSeek::Nada,
    }
}

/// Reposiciona a faixa mantendo o estado de pausa.
pub(crate) fn seek_mantendo_pausa(estado: &mut EstadoAudio, ui: &MainWindow, alvo_secs: u64) {
    let Some(estava_pausado) = estado.audio_player.as_ref().map(|sink| sink.is_paused()) else {
        // Sem faixa carregada não há o que reposicionar
        return;
    };
    executar_seek(estado, ui, alvo_secs, estava_pausado);
}

/// Seek vindo de fora (barra, MPRIS): com player, preserva a pausa; parado
/// com faixa (Stop ou recém-aberto), carrega e toca dali.
pub(crate) fn seek_pedido(estado: &mut EstadoAudio, ui: &MainWindow, alvo_secs: u64) {
    match efeito_seek(
        estado.audio_player.is_some(),
        estado.arquivo_atual.is_some(),
    ) {
        EfeitoSeek::ManterEstado => seek_mantendo_pausa(estado, ui, alvo_secs),
        EfeitoSeek::Recarregar => executar_seek(estado, ui, alvo_secs, false),
        EfeitoSeek::Nada => {}
    }
}

/// A próxima já estava na fila do player e a fila mudou (loop, shuffle,
/// sono): tira ela de lá recriando o player no mesmo ponto.
pub(crate) fn desengatilhar(estado: &mut EstadoAudio, ui: &MainWindow) {
    if estado.engatilhada.is_none() {
        return;
    }
    let pos = estado
        .audio_player
        .as_ref()
        .map_or(0, |p| p.get_pos().as_secs());
    seek_mantendo_pausa(estado, ui, pos);
    atualizar_fila(estado);
}

/// Faixa tirada da fila do shuffle que acabou não tocando volta para a frente.
pub(crate) fn devolver_a_fila(estado: &mut EstadoAudio, indice: usize) {
    if estado.indice_atual == Some(indice) {
        return;
    }
    match estado.modo_shuffle {
        2 => estado.sacola_shuffle.push(indice),
        1 => estado.sorteio.insert(0, indice),
        _ => {}
    }
}

/// Quantas faixas o histórico do "anterior" guarda.
pub(crate) const TAMANHO_HISTORICO: usize = 200;

/// Shuffle: a faixa que tocou antes da atual, tirada do histórico. A atual
/// volta para a frente da fila, para "próxima" voltar a ela. `None` com o
/// histórico vazio (ou só com faixas que sumiram).
pub(crate) fn voltar_no_historico(estado: &mut EstadoAudio) -> Option<(usize, usize)> {
    while let Some((p, chave)) = estado.historico.pop() {
        let Some(pasta) = estado.pastas.get(p) else {
            continue;
        };
        let Some(i) = (0..pasta.tracks.len())
            .find(|&i| pasta.chave_faixa(i).as_deref() == Some(chave.as_str()))
        else {
            continue;
        };
        // A engatilhada volta primeiro, para a atual ficar na frente dela
        if let Some(e) = estado.engatilhada.take() {
            devolver_a_fila(estado, e.indice);
        }
        if estado.pasta_reproducao == Some(p)
            && let Some(atual) = estado.indice_atual
        {
            match estado.modo_shuffle {
                2 => estado.sacola_shuffle.push(atual),
                1 => estado.sorteio.insert(0, atual),
                _ => {}
            }
        }
        return Some((p, i));
    }
    None
}

pub(crate) fn tocar_anterior(estado: &mut EstadoAudio, ui: &MainWindow) {
    // Shuffle: volta ao que tocou de verdade
    if estado.modo_shuffle != 0
        && let Some((p, i)) = voltar_no_historico(estado)
    {
        estado.voltando = true;
        tocar_faixa(estado, ui, p, i);
        estado.voltando = false;
        return;
    }
    // Em ordem: a vizinha na pasta de onde vem a faixa atual, nunca na aba
    // visível
    let Some((pasta_idx, ordem)) = ordem_reproducao(estado) else {
        return;
    };
    let pos = estado
        .indice_atual
        .and_then(|idx| ordem.iter().position(|&i| i == idx));
    let track_idx = match pos {
        Some(p) if p > 0 => ordem[p - 1],
        // Na primeira, ou fora do filtro: vai para a última
        _ => ordem[ordem.len() - 1],
    };
    tocar_faixa(estado, ui, pasta_idx, track_idx);
}

pub(crate) fn alternar_play_pause(estado: &mut EstadoAudio, ui: &MainWindow) {
    match &estado.audio_player {
        Some(sink) if sink.is_paused() => {
            sink.play();
            publicar_play_pause(estado, ui, true);
        }
        Some(_) => pausar(estado, ui),
        None => retomar(estado, ui),
    }
}

pub(crate) fn pausar(estado: &mut EstadoAudio, ui: &MainWindow) {
    if let Some(ref sink) = estado.audio_player
        && !sink.is_paused()
    {
        sink.pause();
        publicar_play_pause(estado, ui, false);
    }
    // A anterior do crossfade não fica tocando sozinha
    if let Some(saindo) = estado.saindo.take() {
        saindo.stop();
    }
}

/// Play: continua o player pausado, ou carrega a faixa que ficou selecionada
/// (Stop, ou o app recém-aberto) a partir de onde ela estava.
pub(crate) fn retomar(estado: &mut EstadoAudio, ui: &MainWindow) {
    match &estado.audio_player {
        Some(sink) if sink.is_paused() => {
            sink.play();
            publicar_play_pause(estado, ui, true);
        }
        Some(_) => {}
        None if estado.arquivo_atual.is_some() => {
            let alvo = estado.tempo_decorrido as u64;
            executar_seek(estado, ui, alvo, false);
            atualizar_imersivo(estado, ui);
        }
        None => {}
    }
}

pub(crate) fn stop_music(estado: &mut EstadoAudio, ui: &MainWindow) {
    estado.audio_player = None;
    estado.saindo = None;
    if let Some(e) = estado.engatilhada.take() {
        devolver_a_fila(estado, e.indice);
    }
    // Parado não segura o dispositivo: sem stream aberto, sem thread de áudio
    estado.saida = None;
    estado.tempo_decorrido = 0.0;
    estado.ultimo_segundo = 0;
    ui.set_progresso(0.0);
    publicar_play_pause(estado, ui, false);
    ui.set_texto_tempo("00:00 / 00:00".into());

    // Nada tocando: some com a letra da faixa anterior
    estado.letra_atual.clear();
    estado.letra_livre = false;
    ui.set_letra_livre(false);
    estado.letra_indice = -1;
    publicar_letra(estado, ui);
}

/// Pasta que está tocando e sua ordem de navegação (com o filtro, quando ele
/// é dessa pasta). Navegar é sempre na pasta da faixa atual, nunca na aba
/// visível.
pub(crate) fn ordem_reproducao(estado: &EstadoAudio) -> Option<(usize, Vec<usize>)> {
    let pasta_idx = estado.pasta_reproducao?;
    let filtro = filtro_efetivo(estado.aba_visivel, estado.pasta_reproducao, &estado.filtro);
    let ordem = ordem_lista(estado, pasta_idx, filtro);
    (!ordem.is_empty()).then_some((pasta_idx, ordem))
}

/// Shuffle inteligente: uma "sacola" embaralhada é consumida até esvaziar e
/// só então outra é feita. Garante que todas as faixas tocam uma vez antes de
/// qualquer repetição.
pub(crate) fn encher_sacola(estado: &mut EstadoAudio, ordem: &[usize]) {
    let mut rng = rand::thread_rng();
    let mut nova_sacola = ordem.to_vec();
    nova_sacola.shuffle(&mut rng);

    // Evita repetição colada na emenda entre um ciclo e o outro: a próxima a
    // sair é `nova_sacola.last()` (o pop consome do fim) — se for igual à
    // última tocada, troca de lugar com outra posição.
    if nova_sacola.len() > 1 && nova_sacola.last().copied() == estado.indice_atual {
        let ultimo = nova_sacola.len() - 1;
        nova_sacola.swap(0, ultimo);
    }

    estado.sacola_shuffle = nova_sacola;
}

/// As próximas `n` faixas (índices na pasta que toca), sem tocar nenhuma. O
/// sorteio do shuffle acontece aqui, adiantado, e `escolher_proxima` consome
/// a mesma fila: o que aparece em "a seguir" é o que toca.
pub(crate) fn proximas(estado: &mut EstadoAudio, n: usize) -> Vec<usize> {
    let Some((_, ordem)) = ordem_reproducao(estado) else {
        return Vec::new();
    };
    match estado.modo_shuffle {
        2 => {
            if estado.sacola_shuffle.is_empty() {
                encher_sacola(estado, &ordem);
            }
            estado
                .sacola_shuffle
                .iter()
                .rev()
                .take(n)
                .copied()
                .collect()
        }
        1 => {
            let mut rng = rand::thread_rng();
            while estado.sorteio.len() < n {
                match ordem.choose(&mut rng) {
                    Some(&i) => estado.sorteio.push(i),
                    None => break,
                }
            }
            estado.sorteio.iter().take(n).copied().collect()
        }
        _ => {
            let mut pos = estado
                .indice_atual
                .and_then(|idx| ordem.iter().position(|&i| i == idx));
            let mut saida = Vec::new();
            for _ in 0..n.min(ordem.len()) {
                let proxima = match pos {
                    Some(p) if p + 1 < ordem.len() => p + 1,
                    Some(_) if estado.modo_loop == 2 => 0,
                    Some(_) => break,
                    None => 0,
                };
                saida.push(ordem[proxima]);
                pos = Some(proxima);
            }
            saida
        }
    }
}

/// Tira da fila a próxima a tocar: (pasta, faixa). `None` no fim da pasta
/// (em ordem, sem loop) ou sem lista.
pub(crate) fn escolher_proxima(estado: &mut EstadoAudio) -> Option<(usize, usize)> {
    let (pasta_idx, ordem) = ordem_reproducao(estado)?;
    let alvo = match estado.modo_shuffle {
        2 => {
            if estado.sacola_shuffle.is_empty() {
                encher_sacola(estado, &ordem);
            }
            estado.sacola_shuffle.pop()
        }
        // Aleatório de verdade: sorteia entre todas as faixas, sem excluir
        // nada (pode repetir, inclusive de seguida — é assim que aleatório de
        // verdade se comporta). Quem quiser evitar repetição é o Shuffle
        // Inteligente. Se a fila "a seguir" já sorteou, vale o sorteio dela.
        1 if !estado.sorteio.is_empty() => Some(estado.sorteio.remove(0)),
        1 => ordem.choose(&mut rand::thread_rng()).copied(),
        // Em ordem: `proximas` já sabe parar no fim da pasta
        _ => proximas(estado, 1).first().copied(),
    };
    alvo.map(|i| (pasta_idx, i))
}

pub(crate) fn tocar_proxima_manual(estado: &mut EstadoAudio, ui: &MainWindow) {
    // Já na fila do player: é ela a próxima
    if let Some(e) = estado.engatilhada.take() {
        tocar_faixa(estado, ui, e.pasta, e.indice);
        return;
    }
    if ordem_reproducao(estado).is_none() {
        return;
    }
    match escolher_proxima(estado) {
        Some((pasta_idx, track_idx)) => tocar_faixa(estado, ui, pasta_idx, track_idx),
        None => stop_music(estado, ui), // fim da pasta: para
    }
}

pub(crate) fn proxima_musica_auto(estado: &mut EstadoAudio, ui: &MainWindow) {
    if estado.sono == Some(sono::Sono::FimDaFaixa) {
        // Hora de dormir: já deixa a próxima engatilhada, mas pausada
        estado.sono = None;
        tocar_proxima_manual(estado, ui);
        pausar(estado, ui);
        return;
    }
    if estado.modo_loop == 1 {
        executar_seek(estado, ui, 0, false);
    } else {
        tocar_proxima_manual(estado, ui);
    }
}

/// Antecedência com que a próxima entra na fila do player (gapless). Folga
/// para o timer de 250 ms e para abrir o arquivo.
pub(crate) const ANTECEDENCIA_GAPLESS: f64 = 2.0;

/// Perto do fim da faixa: põe a próxima na fila do player (gapless) ou começa
/// o crossfade. Faixas seguidas do mesmo álbum, em ordem, emendam sem
/// crossfade: disco ao vivo e álbum conceitual não podem ter a emenda mexida.
pub(crate) fn preparar_transicao(estado: &mut EstadoAudio, ui: &MainWindow) {
    if estado.transicao_feita
        || estado.engatilhada.is_some()
        || estado.sono == Some(sono::Sono::FimDaFaixa)
    {
        return;
    }
    let Some(sink) = &estado.audio_player else {
        return;
    };
    let total = estado.duracao_total_secs as f64;
    if sink.is_paused() || total <= 0.0 {
        return;
    }
    let restante = total - sink.get_pos().as_secs_f64();
    let crossfade = estado.crossfade as f64;
    if restante > crossfade.max(ANTECEDENCIA_GAPLESS) {
        return;
    }
    estado.transicao_feita = true;

    let proxima = if estado.modo_loop == 1 {
        estado.pasta_reproducao.zip(estado.indice_atual)
    } else {
        escolher_proxima(estado)
    };
    let Some((pasta_idx, indice)) = proxima else {
        return;
    };
    let Some(caminho) = estado
        .pastas
        .get(pasta_idx)
        .and_then(|p| p.caminho_faixa(indice))
    else {
        return;
    };

    let mesmo_album = {
        let album = |i: Option<usize>| {
            let p = estado.pastas.get(pasta_idx)?;
            p.tracks.get(i?)?.album.clone()
        };
        estado.modo_shuffle == 0
            && album(estado.indice_atual).is_some()
            && album(estado.indice_atual) == album(Some(indice))
    };
    let cruzar = crossfade > 0.0 && estado.modo_loop != 1 && !mesmo_album;
    let entrada = if cruzar {
        Duration::from_secs_f64(restante.max(0.5))
    } else {
        Duration::ZERO
    };
    let ganho = ganho_de(estado, pasta_idx, Some(indice));
    let Some((fonte, duracao, controle)) = abrir_fonte(estado, &caminho, ganho, entrada) else {
        devolver_a_fila(estado, indice);
        return;
    };

    if !cruzar {
        if let Some(sink) = &estado.audio_player {
            sink.append(fonte);
        }
        estado.engatilhada = Some(Engatilhada {
            pasta: pasta_idx,
            indice,
            duracao,
            controle,
        });
        atualizar_fila(estado);
        return;
    }

    // Crossfade: a atual some no que falta dela, a próxima entra num player
    // novo no mesmo tempo
    estado.controle.esmaecer(entrada);
    let volume = estado.volume_atual;
    let Some(mixer) = mixer(estado) else {
        return;
    };
    let novo = Player::connect_new(mixer);
    novo.set_volume(volume);
    novo.append(fonte);
    estado.saindo = estado.audio_player.replace(novo);
    estado.controle = controle;
    assumir_faixa(estado, ui, pasta_idx, indice, duracao);
}

/// Restaura a faixa e a posição salvas, pausada e sem abrir o dispositivo de
/// áudio: o Play é que carrega (`retomar`).
pub(crate) fn restaurar_posicao(estado: &mut EstadoAudio, ui: &MainWindow, config: &AppConfig) {
    let Some(pasta_idx) = estado.pasta_reproducao else {
        return;
    };
    let Some(pasta) = estado.pastas.get(pasta_idx) else {
        return;
    };
    let indice = match &config.faixa_salva {
        Some(chave) => {
            (0..pasta.tracks.len()).find(|&i| pasta.chave_faixa(i).as_ref() == Some(chave))
        }
        // Config antiga: posição na lista
        None => config.indice_atual.filter(|&i| i < pasta.tracks.len()),
    };
    let Some(indice) = indice else {
        estado.pasta_reproducao = None;
        return;
    };
    let tempo = config.tempo_atual.unwrap_or(0);
    let duracao = pasta.tracks[indice].duracao as u64;

    // A fila do shuffle que ia tocar
    let posicoes: HashMap<String, usize> = (0..pasta.tracks.len())
        .filter_map(|i| Some((pasta.chave_faixa(i)?, i)))
        .collect();
    let fila: Vec<usize> = config
        .fila_shuffle
        .iter()
        .filter_map(|k| posicoes.get(k).copied())
        .collect();

    if !marcar_faixa(estado, ui, pasta_idx, indice, tempo) {
        return;
    }
    match estado.modo_shuffle {
        2 => estado.sacola_shuffle = fila.into_iter().rev().collect(),
        1 => estado.sorteio = fila,
        _ => {}
    }
    estado.tempo_decorrido = tempo as f64;
    estado.duracao_total_secs = duracao;
    publicar_tempo(estado, ui);
    publicar_play_pause(estado, ui, false);
}

pub(crate) fn atualizar_progresso(estado: &mut EstadoAudio, ui: &MainWindow) {
    // A anterior do crossfade terminou de sumir
    if estado.saindo.as_ref().is_some_and(|s| s.empty()) {
        estado.saindo = None;
    }

    // Gapless: a engatilhada começou (a anterior saiu da fila do player)
    if estado.engatilhada.is_some()
        && estado.audio_player.as_ref().is_some_and(|s| s.len() <= 1)
        && let Some(e) = estado.engatilhada.take()
    {
        estado.controle = e.controle;
        assumir_faixa(estado, ui, e.pasta, e.indice, e.duracao);
    }

    let total_secs = estado.duracao_total_secs;
    let terminou = match estado.audio_player {
        Some(ref sink) => sink.empty() && total_secs > 0,
        None => false,
    };
    if terminou {
        proxima_musica_auto(estado, ui);
        return;
    }

    let Some(sink) = &estado.audio_player else {
        return;
    };
    let pausado = sink.is_paused();
    if !pausado {
        estado.tempo_decorrido = sink.get_pos().as_secs_f64();
    }

    // Letra: recalcula a linha atual (também quando pausado, após um seek)
    sincronizar_letra(estado, ui);

    if pausado {
        return;
    }

    if !estado.contada && biblioteca::conta_como_ouvida(estado.tempo_decorrido, total_secs) {
        contar_reproducao(estado, ui);
    }
    preparar_transicao(estado, ui);

    let atual_secs = estado.tempo_decorrido as u64;
    if atual_secs != estado.ultimo_segundo {
        estado.ultimo_segundo = atual_secs;
        publicar_tempo(estado, ui);
    }
}

/// Temporizador de sono: esmaece o volume no fim e pausa. Roda no timer de
/// 250 ms, com ou sem o modo imersivo.
pub(crate) fn verificar_sono(estado: &mut EstadoAudio, ui: &MainWindow) {
    let agora = Instant::now();
    if let Some(sono::Sono::Ate { fim, .. }) = estado.sono {
        let fator = sono::fator_volume(fim, agora);
        if fator <= 0.0 {
            estado.sono = None;
            pausar(estado, ui);
            aplicar_volume(estado, estado.volume_atual);
        } else if fator < 1.0 {
            aplicar_volume(estado, estado.volume_atual * fator);
        }
    }

    let lang = translations::get_translations(&estado.idioma);
    let fim_da_faixa = lang.get("sleep_end").copied().unwrap_or("sleep_end");
    let texto = sono::texto(estado.sono, agora, fim_da_faixa);
    if ui.get_sono_texto() != texto.as_str() {
        ui.set_sono_texto(texto.into());
    }
}

/// Volume nos players (o do crossfade incluído). `volume_atual` é o do slider;
/// o sono passa um valor menor no esmaecimento.
pub(crate) fn aplicar_volume(estado: &EstadoAudio, volume: f32) {
    for p in estado.audio_player.iter().chain(&estado.saindo) {
        p.set_volume(volume);
    }
}

/// Liga os callbacks da UI desta área.
pub(crate) fn ligar(ui: &MainWindow, estado: &Rc<RefCell<EstadoAudio>>) {
    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_anterior(move || {
            if let Some(ui) = ui_fraca.upgrade() {
                tocar_anterior(&mut estado.borrow_mut(), &ui);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_play_pause(move || {
            if let Some(ui) = ui_fraca.upgrade() {
                alternar_play_pause(&mut estado.borrow_mut(), &ui);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_parar(move || {
            if let Some(ui) = ui_fraca.upgrade() {
                stop_music(&mut estado.borrow_mut(), &ui);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_proxima(move || {
            if let Some(ui) = ui_fraca.upgrade() {
                tocar_proxima_manual(&mut estado.borrow_mut(), &ui);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_alternar_loop(move || {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                e.modo_loop = (e.modo_loop + 1) % 3;
                let lang = translations::get_translations(&e.idioma);
                ui.set_texto_loop(texto_loop(e.modo_loop, &lang).into());
                ui.set_loop_modo(e.modo_loop as i32);
                desengatilhar(&mut e, &ui);
                atualizar_fila(&mut e);
                salvar_configuracao(&e);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_alternar_shuffle(move || {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                // Desligado → inteligente → aleatório → desligado
                e.modo_shuffle = match e.modo_shuffle {
                    0 => 2,
                    2 => 1,
                    _ => 0,
                };
                // A engatilhada foi escolhida no modo antigo
                desengatilhar(&mut e, &ui);
                // Sacola da pasta antiga não serve pro modo novo.
                e.sacola_shuffle.clear();
                e.sorteio.clear();
                let lang = translations::get_translations(&e.idioma);
                ui.set_texto_shuffle(texto_shuffle(e.modo_shuffle, &lang).into());
                ui.set_shuffle_modo(e.modo_shuffle as i32);
                atualizar_fila(&mut e);
                salvar_configuracao(&e);
            }
        });
    }

    {
        let estado = estado.clone();
        ui.on_volume_mudou(move |v| {
            let mut e = estado.borrow_mut();
            e.volume_atual = v;
            aplicar_volume(&e, v);
            // Sem salvar aqui: o slider dispara a cada pixel arrastado, e o
            // save ao fechar a janela já persiste o volume.
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_progresso_mudou(move |v| {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                let alvo = (v as f64 * e.duracao_total_secs as f64) as u64;
                // Arrastar a barra com a faixa pausada não pode dar play
                seek_pedido(&mut e, &ui, alvo);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_seek_relativo(move |delta| {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                let alvo = (e.tempo_decorrido as i64 + delta as i64)
                    .clamp(0, e.duracao_total_secs as i64) as u64;
                seek_mantendo_pausa(&mut e, &ui, alvo);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_letra_linha_clicada(move |posicao| {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                // `posicao` é relativa ao recorte que está na UI
                let Some(absoluto) = e.letra_janela_inicio.checked_add(posicao as usize) else {
                    return;
                };
                if e.letra_livre {
                    return;
                }
                let Some((tempo, _)) = e.letra_atual.get(absoluto) else {
                    return;
                };
                let alvo = tempo.max(0.0) as u64;
                seek_mantendo_pausa(&mut e, &ui, alvo);
                // Reflete o novo ponto na hora, sem esperar o próximo tick
                sincronizar_letra(&mut e, &ui);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_letra_completa_clicada(move |indice| {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                if e.letra_livre {
                    return;
                }
                let Some((tempo, _)) = usize::try_from(indice)
                    .ok()
                    .and_then(|i| e.letra_atual.get(i))
                else {
                    return;
                };
                let alvo = tempo.max(0.0) as u64;
                seek_mantendo_pausa(&mut e, &ui, alvo);
                sincronizar_letra(&mut e, &ui);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_alternar_sono(move || {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            e.sono = sono::proximo(e.sono, Instant::now());
            // Trocar ou desligar no meio do esmaecimento devolve o volume
            aplicar_volume(&e, e.volume_atual);
            // "Fim da faixa" pausa na próxima: ela não pode já estar na fila
            if e.sono == Some(sono::Sono::FimDaFaixa) {
                desengatilhar(&mut e, &ui);
            }
            verificar_sono(&mut e, &ui);
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_alternar_crossfade(move || {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            e.crossfade = proximo_crossfade(e.crossfade);
            publicar_crossfade(&e, &ui);
            salvar_configuracao(&e);
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_alternar_replaygain(move |ligado| {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            e.replaygain = ligado;
            // O ganho mora na fonte: refaz a faixa atual no mesmo ponto
            let pos = e.audio_player.as_ref().map(|p| p.get_pos().as_secs());
            if let Some(pos) = pos {
                seek_mantendo_pausa(&mut e, &ui, pos);
            }
            salvar_configuracao(&e);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::Sample;

    fn fixture(nome: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(nome)
    }

    /// Vídeo curto gerado pelo ffmpeg (`None` sem ele, ou sem o codificador).
    fn video_de_teste(ext: &str, codecs: &[&str]) -> Option<PathBuf> {
        if !ffmpeg::disponivel() {
            return None;
        }
        let caminho =
            std::env::temp_dir().join(format!("furinar-teste-video-{}.{ext}", std::process::id()));
        ffmpeg::comando("ffmpeg")
            .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
            .arg("testsrc=size=160x90:rate=10:duration=2")
            .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=2"])
            .args(["-metadata", "title=Clipe", "-shortest"])
            .args(codecs)
            .arg(&caminho)
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
            .then_some(caminho)
    }

    #[test]
    fn videos_tocam_com_duracao_e_seek_certos() {
        let casos = [
            ("mp4", &["-c:v", "mpeg4", "-c:a", "aac"][..]),
            ("webm", &["-c:v", "libvpx", "-c:a", "libopus"][..]),
        ];
        for (ext, codecs) in casos {
            let Some(caminho) = video_de_teste(ext, codecs) else {
                eprintln!("sem ffmpeg/codificador para {ext}: pulado");
                continue;
            };
            // Com ffmpeg, vídeo sempre vai por ele
            let fonte = Decodificado::abrir(&caminho).expect("vídeo deve abrir");
            assert!(matches!(fonte, Decodificado::Ffmpeg(_)), "{ext}");
            let total = fonte.total_duration().unwrap().as_secs_f64();
            assert!((total - 2.0).abs() < 0.1, "{ext}: duração {total} s");

            let tags = track::ler_tags(&caminho);
            assert_eq!(tags.titulo, "Clipe", "{ext}");
            assert_eq!(tags.duracao, 2, "{ext}");

            // Sem ffmpeg, o MP4 vai pelo symphonia: duração do container e
            // seek reabrindo (o do rodio parava em 0,46 s)
            if ext == "mp4" {
                let mut nativo = Decodificado::Nativo {
                    d: abrir_nativo(&caminho).unwrap(),
                    video: Some(VideoNativo::novo(&caminho)),
                };
                let total = nativo.total_duration().unwrap().as_secs_f64();
                assert!((total - 2.0).abs() < 0.1, "nativo: duração {total} s");
                let taxa = nativo.sample_rate().get() as f64 * nativo.channels().get() as f64;
                // O seek decodifica numa thread; a fonte toca silêncio até
                // ele chegar e depois desconta esse silêncio. 4800 amostras
                // de silêncio + o resto = os mesmos 0,5 s
                nativo.try_seek(Duration::from_millis(1500)).unwrap();
                for _ in 0..4800 {
                    assert_eq!(nativo.next(), Some(0.0));
                }
                std::thread::sleep(Duration::from_millis(300));
                let resto = nativo.count() as f64 / taxa;
                assert!(
                    (resto - 0.5).abs() < 0.1,
                    "nativo: depois do seek {resto} s"
                );
            }
            let _ = std::fs::remove_file(caminho);
        }
    }

    /// Abre a fixture do mesmo jeito que `executar_seek` abre a faixa real:
    /// lendo o tamanho do arquivo antes de construir o decodificador.
    fn decodificar(nome: &str, extensao: &str) -> Decoder<BufReader<File>> {
        let arquivo = File::open(fixture(nome)).unwrap_or_else(|e| panic!("fixture {nome}: {e}"));
        let byte_len = arquivo
            .metadata()
            .unwrap_or_else(|e| panic!("metadata de {nome}: {e}"))
            .len();
        criar_decodificador(BufReader::new(arquivo), extensao, byte_len)
            .unwrap_or_else(|e| panic!("{nome} deve decodificar: {e:?}"))
    }

    fn amostras_nao_silenciosas(decodificador: Decoder<BufReader<File>>) {
        let duracao = decodificador
            .total_duration()
            .expect("duração deve ser conhecida");
        let amostras: Vec<Sample> = decodificador.take(4096).collect();
        assert!(!amostras.is_empty(), "{:?} não devolveu amostras", duracao);
        assert!(
            amostras.iter().any(|s| *s != 0.0),
            "{:?} devolveu só silêncio",
            duracao
        );
    }

    #[test]
    fn m4a_decodifica_com_hint_de_extensao() {
        let decodificador = decodificar("tone.m4a", "m4a");
        let duracao = decodificador
            .total_duration()
            .expect("duração do m4a deve ser conhecida");
        assert!(duracao.as_millis() >= 150, "durou {duracao:?}");

        amostras_nao_silenciosas(decodificador);
    }

    #[test]
    fn m4a_com_moov_no_inicio_tambem_decodifica() {
        let decodificador = decodificar("tone_faststart.m4a", "m4a");
        let duracao = decodificador
            .total_duration()
            .expect("duração do m4a deve ser conhecida");
        assert!(duracao.as_secs() >= 1, "durou {duracao:?}");

        amostras_nao_silenciosas(decodificador);
    }

    #[test]
    fn m4a_permite_seek() {
        let mut decodificador = decodificar("tone.m4a", "m4a");
        decodificador
            .try_seek(Duration::from_millis(100))
            .expect("seek em m4a deve funcionar");
    }

    #[test]
    fn mp3_decodifica_com_hint_de_extensao() {
        let decodificador = decodificar("tone.mp3", "mp3");
        amostras_nao_silenciosas(decodificador);
    }

    /// Lista em memória com `n` faixas ("/m/0.mp3"...).
    fn estado_com_faixas(n: usize) -> EstadoAudio {
        let mut e = EstadoAudio::default();
        let mut pasta = PastaMusical::pasta(PathBuf::from("/m"));
        pasta.tracks = (0..n)
            .map(|i| TrackInfo {
                path: format!("{i}.mp3"),
                titulo: i.to_string(),
                ..Default::default()
            })
            .collect();
        pasta.carregada = true;
        e.pastas.push(pasta);
        e.pasta_reproducao = Some(0);
        e
    }

    #[test]
    fn anterior_no_shuffle_volta_pelo_historico() {
        let mut e = estado_com_faixas(10);
        e.modo_shuffle = 2;
        // Tocou 7, depois 2, e agora toca 5
        e.historico = vec![(0, "/m/7.mp3".into()), (0, "/m/2.mp3".into())];
        e.indice_atual = Some(5);
        e.sacola_shuffle = vec![9, 4];

        assert_eq!(voltar_no_historico(&mut e), Some((0, 2)));
        // A atual (5) é a próxima a sair da sacola (pop pelo fim)
        assert_eq!(e.sacola_shuffle.last(), Some(&5));
        e.indice_atual = Some(2);
        assert_eq!(voltar_no_historico(&mut e), Some((0, 7)));
        assert_eq!(voltar_no_historico(&mut e), None);
    }

    #[test]
    fn anterior_no_aleatorio_poe_a_atual_na_frente_do_sorteio() {
        let mut e = estado_com_faixas(10);
        e.modo_shuffle = 1;
        e.historico = vec![(0, "/m/3.mp3".into()), (0, "/m/sumiu.mp3".into())];
        e.indice_atual = Some(8);
        e.sorteio = vec![1, 6];
        // A que sumiu da pasta é pulada
        assert_eq!(voltar_no_historico(&mut e), Some((0, 3)));
        assert_eq!(e.sorteio, [8, 1, 6]);
    }

    #[test]
    fn seek_com_player_preserva_tocando_ou_pausado() {
        assert_eq!(efeito_seek(true, true), EfeitoSeek::ManterEstado);
        assert_eq!(efeito_seek(true, false), EfeitoSeek::ManterEstado);
    }

    #[test]
    fn seek_sem_player_só_recarrega_com_faixa_selecionada() {
        assert_eq!(efeito_seek(false, true), EfeitoSeek::Recarregar);
        assert_eq!(efeito_seek(false, false), EfeitoSeek::Nada);
    }
}
