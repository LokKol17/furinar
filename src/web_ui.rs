//! Tela de informações da faixa e lote da aba: pedidos ao buscador da web
//! e o que volta dele.

use crate::*;

/// Como mostrar um candidato na tela de informações.
pub(crate) fn linha_candidato(c: &metadados::Candidato, pelo_som: &str) -> CandidatoInfo {
    // Mesmo formato do álbum atual na tela, para a comparação "o que muda"
    let album = match (&c.album, c.ano) {
        (Some(album), Some(ano)) => format!("{album} ({ano})"),
        (Some(album), None) => album.clone(),
        (None, _) => String::new(),
    };
    let mut detalhe = c.artista.clone();
    if !album.is_empty() {
        detalhe.push_str(" · ");
        detalhe.push_str(&album);
    }
    if c.duracao > 0 {
        detalhe.push_str(" · ");
        detalhe.push_str(&formatar_tempo(c.duracao as u64, c.duracao >= 3600));
    }
    if c.fonte == metadados::Fonte::ImpressaoDigital {
        detalhe.push_str(" · ");
        detalhe.push_str(pelo_som);
    }
    CandidatoInfo {
        titulo: c.titulo.as_str().into(),
        artista: c.artista.as_str().into(),
        album: album.into(),
        detalhe: detalhe.into(),
        confianca: c.confianca as i32,
        digital: c.fonte == metadados::Fonte::ImpressaoDigital,
    }
}

/// Manda um pedido ao buscador, criando-o se for o primeiro.
pub(crate) fn pedir_web(estado: &mut EstadoAudio, pedido: buscador::Pedido) {
    let chave = acoustid::chave_efetiva(&estado.chave_acoustid);
    estado
        .buscador
        .get_or_insert_with(|| buscador::Buscador::novo(chave))
        .pedir(pedido);
}

pub(crate) fn consulta_de(
    estado: &EstadoAudio,
    pasta_idx: usize,
    indice: usize,
) -> Option<metadados::Consulta> {
    let pasta = estado.pastas.get(pasta_idx)?;
    let t = pasta.tracks.get(indice)?;
    Some(metadados::Consulta {
        chave: chave_faixa(&pasta.caminho, &t.path),
        caminho: pasta.caminho.join(&t.path),
        titulo: t.titulo.clone(),
        artista: t.artista.clone(),
        album: t.album.clone(),
        duracao: t.duracao,
    })
}

/// Faixa (lista, posição) com esta chave, de preferência a que toca.
pub(crate) fn achar_faixa(estado: &EstadoAudio, chave: &str) -> Option<(usize, usize)> {
    if chave_atual(estado).as_deref() == Some(chave) {
        return estado.pasta_reproducao.zip(estado.indice_atual);
    }
    estado.pastas.iter().enumerate().find_map(|(p, pasta)| {
        let i =
            (0..pasta.tracks.len()).find(|&i| pasta.chave_faixa(i).as_deref() == Some(chave))?;
        Some((p, i))
    })
}

/// Letra da faixa atual na web. `forcar`: mesmo que ela já tenha falhado
/// antes (botão da tela de informações).
pub(crate) fn pedir_letra(estado: &mut EstadoAudio, forcar: bool) {
    let Some(consulta) = estado
        .pasta_reproducao
        .zip(estado.indice_atual)
        .and_then(|(p, i)| consulta_de(estado, p, i))
    else {
        return;
    };
    if forcar {
        estado.metadados.esquecer_sem_letra(&consulta.chave);
    } else if !estado.metadados.buscar_letra(&consulta.chave) {
        return;
    }
    pedir_web(
        estado,
        buscador::Pedido::Letra {
            consulta,
            automatica: !forcar,
        },
    );
}

/// Põe a correção no banco e nas listas carregadas, e pede a capa.
pub(crate) fn aplicar_correcao(
    estado: &mut EstadoAudio,
    chave: &str,
    correcao: metadados::Correcao,
) {
    for pasta in &mut estado.pastas {
        let raiz = pasta.caminho.clone();
        for t in &mut pasta.tracks {
            if chave_faixa(&raiz, &t.path) == chave {
                correcao.aplicar(t);
            }
        }
    }
    if let Some(lancamento) = correcao.lancamento.clone() {
        pedir_web(
            estado,
            buscador::Pedido::Capa {
                chave: chave.to_string(),
                lancamento,
            },
        );
    }
    estado.metadados.corrigir(chave, correcao);
}

/// Depois de mexer nos metadados: lista, painel e tela cheia de novo.
pub(crate) fn renovar_vistas(estado: &mut EstadoAudio, ui: &MainWindow) {
    atualizar_lista(estado, ui);
    let faixa = faixa_imersiva(estado);
    if let Some(imv) = estado.imersivo.as_mut() {
        imv.atualizar_dados(ui, faixa);
    }
    atualizar_fila(estado);
    publicar_info(estado, ui);
}

/// Volta a faixa ao que as tags dizem.
pub(crate) fn restaurar_original(estado: &mut EstadoAudio, chave: &str) {
    if !estado.metadados.restaurar(chave) {
        return;
    }
    for pasta in &mut estado.pastas {
        let raiz = pasta.caminho.clone();
        for t in &mut pasta.tracks {
            if chave_faixa(&raiz, &t.path) == chave {
                *t = track::ler_faixa(&raiz.join(&t.path), t.path.clone());
            }
        }
    }
}

/// Preenche a tela de informações com a faixa `info_chave`.
pub(crate) fn publicar_info(estado: &EstadoAudio, ui: &MainWindow) {
    let lang = translations::get_translations(&estado.idioma);
    let t = |k: &str| lang.get(k).copied().unwrap_or("").to_string();
    let Some(chave) = estado.info_chave.as_deref() else {
        return;
    };
    let Some((p, i)) = achar_faixa(estado, chave) else {
        return;
    };
    let pasta = &estado.pastas[p];
    let faixa = &pasta.tracks[i];
    ui.set_info_titulo(faixa.titulo.as_str().into());
    ui.set_info_artista(faixa.artista.as_deref().unwrap_or("").into());
    let correcao = estado.metadados.correcao(chave);
    let mut album = faixa.album.clone().unwrap_or_default();
    if let Some(ano) = correcao.and_then(|c| c.ano) {
        album.push_str(&format!(" ({ano})"));
    }
    ui.set_info_album(album.into());
    ui.set_info_corrigida(correcao.is_some());
    let caminho = pasta.caminho.join(&faixa.path);
    let letra = if letras_web::tem_letra_local(&caminho, chave) {
        t("info_lyrics_ok")
    } else if !estado.metadados.buscar_letra(chave) {
        t("info_lyrics_missing_web")
    } else {
        t("info_lyrics_none")
    };
    ui.set_info_letra(letra.into());
    let pelo_som = t("info_by_sound");
    let linhas: Vec<CandidatoInfo> = estado
        .info_candidatos
        .iter()
        .map(|c| linha_candidato(c, &pelo_som))
        .collect();
    ui.set_info_candidatos(ModelRc::new(VecModel::from(linhas)));
}

/// Campos da edição à mão: a lista (já corrigida) e, do que ela não
/// guarda, a correção à mão ou as tags do arquivo.
pub(crate) fn publicar_edicao(estado: &EstadoAudio, ui: &MainWindow) {
    let Some(chave) = estado.info_chave.as_deref() else {
        return;
    };
    let Some((p, i)) = achar_faixa(estado, chave) else {
        return;
    };
    let faixa = &estado.pastas[p].tracks[i];
    let caminho = estado.pastas[p].caminho.join(&faixa.path);
    let extras = match estado.metadados.correcao(chave) {
        Some(c) if c.manual => metadados::Extras {
            artista_album: c.artista_album.clone(),
            genero: c.genero.clone(),
            ano: c.ano,
        },
        c => {
            let mut lido = metadados::ler_extras(&caminho);
            lido.ano = c.and_then(|c| c.ano).or(lido.ano);
            lido
        }
    };
    let numero = |n: u32| {
        if n > 0 { n.to_string() } else { String::new() }
    };
    ui.set_info_edicao(EdicaoInfo {
        titulo: faixa.titulo.as_str().into(),
        artista: faixa.artista.as_deref().unwrap_or("").into(),
        album: faixa.album.as_deref().unwrap_or("").into(),
        artista_album: extras.artista_album.unwrap_or_default().into(),
        genero: extras.genero.unwrap_or_default().into(),
        ano: extras.ano.map(|a| a.to_string()).unwrap_or_default().into(),
        faixa: numero(faixa.numero % 1000).into(),
        disco: numero(faixa.numero / 1000).into(),
        arquivo: caminho.display().to_string().into(),
    });
}

/// Campo numérico da edição: vazio = 0, `None` = inválido.
fn numero_campo(texto: &str, maximo: u32) -> Option<u32> {
    let texto = texto.trim();
    if texto.is_empty() {
        return Some(0);
    }
    texto.parse::<u32>().ok().filter(|&n| n <= maximo)
}

/// A edição à mão vira uma correção (`None` = número inválido). Mantém a
/// gravação e a capa de uma correção da web que já havia.
fn correcao_da_edicao(
    dados: &EdicaoInfo,
    anterior: Option<&metadados::Correcao>,
) -> Option<metadados::Correcao> {
    let texto = |s: &SharedString| {
        let s = s.trim();
        (!s.is_empty()).then(|| s.to_string())
    };
    let ano = numero_campo(&dados.ano, 9999)?;
    let faixa = numero_campo(&dados.faixa, 999)?;
    let disco = numero_campo(&dados.disco, 999)?;
    Some(metadados::Correcao {
        titulo: dados.titulo.trim().to_string(),
        artista: dados.artista.trim().to_string(),
        album: Some(dados.album.trim().to_string()),
        ano: u16::try_from(ano).ok().filter(|&a| a > 0),
        numero: disco * 1000 + faixa,
        gravacao: anterior.and_then(|c| c.gravacao.clone()),
        lancamento: anterior.and_then(|c| c.lancamento.clone()),
        artista_album: texto(&dados.artista_album),
        genero: texto(&dados.genero),
        manual: true,
    })
}

/// Revisão do lote e resumo na tela.
pub(crate) fn publicar_lote(estado: &EstadoAudio, ui: &MainWindow) {
    let lang = translations::get_translations(&estado.idioma);
    let t = |k: &str| lang.get(k).copied().unwrap_or("").to_string();
    let l = &estado.lote;
    ui.set_lote_ativo(l.ativo);
    ui.set_lote_progresso(if l.total > 0 {
        l.feitas as f32 / l.total as f32
    } else {
        0.0
    });
    let texto = if l.ativo {
        format!("{} {}/{}", t("batch_running"), l.feitas, l.total)
    } else if l.total > 0 {
        t("batch_summary")
            .replace("{auto}", &l.automaticas.to_string())
            .replace("{revisar}", &estado.revisao.len().to_string())
            .replace("{nada}", &l.sem_resultado.to_string())
            .replace("{letras}", &l.letras.to_string())
    } else {
        String::new()
    };
    ui.set_lote_texto(texto.into());
    let itens: Vec<ItemRevisao> = estado
        .revisao
        .iter()
        .map(|(consulta, c)| {
            let original = match &consulta.artista {
                Some(a) => format!("{a} - {}", consulta.titulo),
                None => consulta.titulo.clone(),
            };
            let mut sugestao = format!("{} - {}", c.artista, c.titulo);
            if let Some(album) = &c.album {
                sugestao.push_str(&format!(" · {album}"));
            }
            ItemRevisao {
                original: original.into(),
                sugestao: sugestao.into(),
                confianca: c.confianca as i32,
            }
        })
        .collect();
    ui.set_revisao(ModelRc::new(VecModel::from(itens)));
}

/// Respostas do buscador (timer de 250 ms).
pub(crate) fn processar_web(estado: &mut EstadoAudio, ui: &MainWindow) {
    let Some(b) = estado.buscador.as_mut() else {
        return;
    };
    let respostas = b.respostas();
    let lang = translations::get_translations(&estado.idioma);
    let t = |k: &str| lang.get(k).copied().unwrap_or("").to_string();
    let mut renovar = false;
    for resposta in respostas {
        match resposta {
            buscador::Resposta::Letra { chave, resultado } => {
                match resultado {
                    Ok(letras_web::Resultado::Salva { .. }) => {
                        estado.metadados.esquecer_sem_letra(&chave);
                        if chave_atual(estado).as_deref() == Some(chave.as_str())
                            && let Some(caminho) = caminho_atual(estado)
                        {
                            let tempo = estado.tempo_decorrido;
                            carregar_letra(estado, ui, &caminho, tempo);
                            avisar(ui, &t("toast_lyrics_saved"));
                        }
                    }
                    Ok(_) => estado.metadados.marcar_sem_letra(&chave),
                    Err(e) => {
                        if estado.info_chave.as_deref() == Some(chave.as_str()) {
                            ui.set_info_estado(e.descrever(&t).into());
                        }
                    }
                }
                estado.metadados.salvar();
                if estado.info_chave.as_deref() == Some(chave.as_str()) {
                    ui.set_info_ocupado(false);
                    publicar_info(estado, ui);
                }
            }
            buscador::Resposta::Candidatos { chave, lista, erro } => {
                if estado.info_chave.as_deref() != Some(chave.as_str()) {
                    continue;
                }
                let texto = match (&erro, lista.len()) {
                    (Some(e), 0) => e.descrever(&t),
                    // Sem chave a busca pelo som nem roda: dizer, em vez de
                    // só "nada encontrado" (builds locais saem sem a embutida)
                    (_, 0) if acoustid::chave_efetiva(&estado.chave_acoustid).is_none() => {
                        t("info_nothing_no_key")
                    }
                    (_, 0) => t("info_nothing"),
                    (_, 1) => t("info_result_one"),
                    (_, n) => t("info_results").replace("{n}", &n.to_string()),
                };
                estado.info_candidatos = lista;
                ui.set_info_estado(texto.into());
                ui.set_info_ocupado(false);
                publicar_info(estado, ui);
            }
            buscador::Resposta::LoteProgresso { feitas, total } => {
                estado.lote.feitas = feitas;
                estado.lote.total = total;
                publicar_lote(estado, ui);
            }
            buscador::Resposta::LoteItem {
                consulta,
                candidatos,
                letra,
            } => {
                match candidatos.first() {
                    Some(c) if c.confianca >= metadados::CONFIANCA_AUTOMATICA => {
                        aplicar_correcao(estado, &consulta.chave, metadados::Correcao::from(c));
                        estado.lote.automaticas += 1;
                        renovar = true;
                    }
                    Some(c) => estado.revisao.push((consulta.clone(), c.clone())),
                    None => estado.lote.sem_resultado += 1,
                }
                match letra {
                    Some(letras_web::Resultado::Salva { .. }) => estado.lote.letras += 1,
                    Some(_) => estado.metadados.marcar_sem_letra(&consulta.chave),
                    None => {}
                }
                estado.lote.feitas += 1;
            }
            buscador::Resposta::LoteFim { cancelado, erro } => {
                estado.lote.ativo = false;
                estado.metadados.salvar();
                renovar = true;
                if let Some(e) = erro.filter(|_| !cancelado && estado.lote.automaticas == 0) {
                    ui.set_info_estado(e.descrever(&t).into());
                }
                // A letra da faixa atual pode ter chegado no lote
                if let Some(caminho) = caminho_atual(estado)
                    && estado.letra_atual.is_empty()
                {
                    let tempo = estado.tempo_decorrido;
                    carregar_letra(estado, ui, &caminho, tempo);
                }
            }
            buscador::Resposta::Capa { chave, ok } => {
                if ok {
                    // A miniatura "sem capa" dessa faixa não vale mais
                    let caminho = achar_faixa(estado, &chave)
                        .and_then(|(l, i)| estado.pastas[l].caminho_faixa(i));
                    if let Some(p) = estado.imersivo.as_mut().and_then(|i| i.painel.as_mut())
                        && let Some(c) = caminho
                    {
                        p.esquecer(&c);
                    }
                    renovar = true;
                }
            }
            buscador::Resposta::TocandoAgora => {}
            buscador::Resposta::CapaDiscord { chave, url } => match url {
                Ok(url) => {
                    estado.capas_discord.insert(chave, url);
                }
                // Sai do cache: a próxima vez que tocar, pede de novo
                Err(_) => {
                    estado.capas_discord.remove(&chave);
                }
            },
            buscador::Resposta::Scrobble { sobrou, erro } => {
                estado.fila_scrobble.extend(sobrou);
                scrobble::salvar_fila(&estado.fila_scrobble);
                let texto = erro.map(|e| e.descrever(&t)).unwrap_or_default();
                ui.set_scrobble_estado(texto.into());
            }
            buscador::Resposta::LastfmToken(resultado) => match resultado {
                Ok((token, url)) => {
                    estado.lastfm_token = Some(token);
                    abrir_no_navegador(&url);
                    publicar_lastfm(estado, ui);
                }
                Err(e) => ui.set_lastfm_estado(e.descrever(&t).into()),
            },
            buscador::Resposta::LastfmSessao(resultado) => {
                estado.lastfm_token = None;
                match resultado {
                    Ok((sessao, usuario)) => {
                        estado.contas.lastfm_sessao = sessao;
                        estado.lastfm_usuario = usuario;
                        salvar_configuracao(estado);
                        publicar_lastfm(estado, ui);
                    }
                    Err(e) => {
                        publicar_lastfm(estado, ui);
                        ui.set_lastfm_estado(e.descrever(&t).into());
                    }
                }
            }
            buscador::Resposta::Gravadas { ok, falhas } => {
                let mut texto = t("tags_written").replace("{n}", &ok.to_string());
                if let Some((caminho, falha)) = falhas.first() {
                    eprintln!("tags não gravadas em {}: {falha:?}", caminho.display());
                    let nome = caminho
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let motivo = match falha {
                        metadados::FalhaTags::SemPermissao => t("tags_fail_permission"),
                        metadados::FalhaTags::EmUso => t("tags_fail_busy"),
                        metadados::FalhaTags::Outra(e) => t("tags_fail_other").replace("{erro}", e),
                    };
                    texto.push_str(" · ");
                    texto.push_str(&motivo.replace("{nome}", &nome));
                    if falhas.len() > 1 {
                        texto.push(' ');
                        texto.push_str(
                            &t("tags_fail_more").replace("{n}", &(falhas.len() - 1).to_string()),
                        );
                    }
                }
                ui.set_tags_texto(texto.into());
            }
        }
    }
    if renovar {
        renovar_vistas(estado, ui);
        atualizar_painel(estado, ui);
    }
    publicar_lote(estado, ui);
    if estado.buscador.as_ref().is_some_and(|b| b.dispensavel()) {
        // Sem trabalho há um minuto: solta o cliente HTTP e a thread
        estado.buscador = None;
        Timer::single_shot(Duration::from_millis(500), memoria::aparar);
    }
}

/// A faixa atual como escuta: começou a tocar há `tempo_decorrido`.
fn escuta_atual(estado: &EstadoAudio) -> Option<scrobble::Escuta> {
    let pasta = estado.pastas.get(estado.pasta_reproducao?)?;
    let indice = estado.indice_atual?;
    let t = pasta.tracks.get(indice)?;
    let agora = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let gravacao = pasta
        .chave_faixa(indice)
        .and_then(|k| estado.metadados.correcao(&k)?.gravacao.clone());
    Some(scrobble::Escuta {
        // Sem artista os serviços recusam: vai o título como está
        artista: t.artista.clone()?,
        titulo: t.titulo.clone(),
        album: t.album.clone(),
        duracao: estado.duracao_total_secs,
        quando: agora.saturating_sub(estado.tempo_decorrido as u64),
        gravacao,
    })
}

/// "Tocando agora": uma vez por faixa, quando ela começa a tocar.
pub(crate) fn atualizar_scrobble(estado: &mut EstadoAudio) {
    if !estado.contas.alguma() {
        return;
    }
    let tocando = estado.audio_player.as_ref().is_some_and(|p| !p.is_paused());
    let chave = chave_atual(estado);
    if !tocando || chave.is_none() || chave == estado.avisada {
        return;
    }
    estado.avisada = chave;
    if let Some(escuta) = escuta_atual(estado) {
        let contas = estado.contas.clone();
        pedir_web(estado, buscador::Pedido::TocandoAgora(contas, escuta));
    }
}

/// A faixa atual contou como ouvida: vai para os serviços (com o que estava
/// na fila).
pub(crate) fn scrobble_ouvida(estado: &mut EstadoAudio) {
    if !estado.contas.alguma() {
        return;
    }
    if let Some(escuta) = escuta_atual(estado).filter(scrobble::conta) {
        for servico in estado.contas.servicos() {
            estado.fila_scrobble.push((servico, escuta.clone()));
        }
    }
    enviar_fila_scrobble(estado);
}

pub(crate) fn enviar_fila_scrobble(estado: &mut EstadoAudio) {
    if estado.fila_scrobble.is_empty() || !estado.contas.alguma() {
        return;
    }
    let fila = std::mem::take(&mut estado.fila_scrobble);
    // Se o app fechar no meio, a fila não se perde
    scrobble::salvar_fila(&fila);
    let contas = estado.contas.clone();
    pedir_web(estado, buscador::Pedido::Scrobble(contas, fila));
}

/// Estado da conta do Last.fm na tela.
pub(crate) fn publicar_lastfm(estado: &EstadoAudio, ui: &MainWindow) {
    let lang = translations::get_translations(&estado.idioma);
    let t = |k: &str| lang.get(k).copied().unwrap_or("").to_string();
    let conectado = !estado.contas.lastfm_sessao.is_empty();
    ui.set_lastfm_conectado(conectado);
    ui.set_lastfm_aguardando(estado.lastfm_token.is_some());
    let texto = if conectado {
        t("lastfm_connected").replace("{u}", &estado.lastfm_usuario)
    } else if estado.lastfm_token.is_some() {
        t("lastfm_waiting")
    } else {
        String::new()
    };
    ui.set_lastfm_estado(texto.into());
}

/// Liga os callbacks da UI desta área.
pub(crate) fn ligar(ui: &MainWindow, estado: &Rc<RefCell<EstadoAudio>>) {
    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_abrir_info(move || {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let Some(chave) = chave_atual(&e) else {
                return;
            };
            // Outra faixa (ou o buscador já foi embora): a busca que estava
            // em andamento era de outra tela e a resposta dela será
            // descartada, então nada mais devolveria os botões
            if e.info_chave.as_deref() != Some(chave.as_str()) || e.buscador.is_none() {
                ui.set_info_ocupado(false);
            }
            if e.info_chave.as_deref() != Some(chave.as_str()) {
                e.info_candidatos.clear();
                ui.set_info_estado(SharedString::new());
            }
            e.info_chave = Some(chave);
            ui.set_tags_texto(SharedString::new());
            publicar_info(&e, &ui);
            publicar_edicao(&e, &ui);
            publicar_lote(&e, &ui);
            ui.set_info_aberta(true);
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_info_buscar(move || {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let Some(consulta) = e
                .info_chave
                .clone()
                .and_then(|k| achar_faixa(&e, &k))
                .and_then(|(p, i)| consulta_de(&e, p, i))
            else {
                return;
            };
            let lang = translations::get_translations(&e.idioma);
            ui.set_info_estado(lang.get("info_searching").copied().unwrap_or("").into());
            ui.set_info_ocupado(true);
            e.info_candidatos.clear();
            publicar_info(&e, &ui);
            pedir_web(&mut e, buscador::Pedido::Identificar(consulta));
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_info_aplicar(move |posicao, gravar| {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let (Some(chave), Some(c)) = (
                e.info_chave.clone(),
                e.info_candidatos.get(posicao.max(0) as usize).cloned(),
            ) else {
                return;
            };
            if e.gravar_tags_info != gravar {
                e.gravar_tags_info = gravar;
                salvar_configuracao(&e);
            }
            let correcao = metadados::Correcao::from(&c);
            aplicar_correcao(&mut e, &chave, correcao.clone());
            e.metadados.salvar();
            if gravar
                && let Some((p, i)) = achar_faixa(&e, &chave)
                && let Some(caminho) = e.pastas[p].caminho_faixa(i)
            {
                pedir_web(
                    &mut e,
                    buscador::Pedido::GravarTags(vec![(caminho, correcao)]),
                );
            }
            e.info_candidatos.clear();
            ui.set_info_estado(SharedString::new());
            renovar_vistas(&mut e, &ui);
            atualizar_painel(&mut e, &ui);
            publicar_edicao(&e, &ui);
            // Com os nomes certos, a letra pode aparecer
            if e.letra_atual.is_empty() && chave_atual(&e).as_deref() == Some(chave.as_str()) {
                pedir_letra(&mut e, true);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_info_salvar_edicao(move |dados, gravar| {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let Some(chave) = e.info_chave.clone() else {
                return;
            };
            let lang = translations::get_translations(&e.idioma);
            let t = |k: &str| lang.get(k).copied().unwrap_or("").to_string();
            if dados.titulo.trim().is_empty() {
                return;
            }
            let Some(correcao) = correcao_da_edicao(&dados, e.metadados.correcao(&chave)) else {
                avisar(&ui, &t("edit_bad_number"));
                return;
            };
            if e.gravar_tags_info != gravar {
                e.gravar_tags_info = gravar;
                salvar_configuracao(&e);
            }
            let nome = correcao.titulo.clone();
            aplicar_correcao(&mut e, &chave, correcao.clone());
            e.metadados.salvar();
            ui.set_tags_texto(SharedString::new());
            if gravar
                && let Some((p, i)) = achar_faixa(&e, &chave)
                && let Some(caminho) = e.pastas[p].caminho_faixa(i)
            {
                ui.set_tags_texto(t("tags_writing").into());
                pedir_web(
                    &mut e,
                    buscador::Pedido::GravarTags(vec![(caminho, correcao)]),
                );
            }
            renovar_vistas(&mut e, &ui);
            atualizar_painel(&mut e, &ui);
            publicar_edicao(&e, &ui);
            avisar(&ui, &t("edit_saved").replace("{nome}", &nome));
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_info_restaurar(move || {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            if let Some(chave) = e.info_chave.clone() {
                restaurar_original(&mut e, &chave);
                e.metadados.salvar();
                renovar_vistas(&mut e, &ui);
                atualizar_painel(&mut e, &ui);
                publicar_edicao(&e, &ui);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_info_baixar_letra(move || {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            // A tela é sempre da faixa atual
            if e.info_chave.is_some() && e.info_chave == chave_atual(&e) {
                let lang = translations::get_translations(&e.idioma);
                ui.set_info_estado(lang.get("info_searching").copied().unwrap_or("").into());
                ui.set_info_ocupado(true);
                pedir_letra(&mut e, true);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_lote_iniciar(move || {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let aba = e.aba_visivel;
            let n = e.pastas.get(aba).map_or(0, |p| p.tracks.len());
            // Quem já tem correção fica de fora
            let consultas: Vec<metadados::Consulta> = (0..n)
                .filter_map(|i| consulta_de(&e, aba, i))
                .filter(|c| e.metadados.correcao(&c.chave).is_none())
                .collect();
            e.revisao.clear();
            e.lote = Lote {
                ativo: !consultas.is_empty(),
                total: consultas.len(),
                ..Default::default()
            };
            if !consultas.is_empty() {
                pedir_web(
                    &mut e,
                    buscador::Pedido::Lote {
                        consultas,
                        letras: true,
                    },
                );
            }
            publicar_lote(&e, &ui);
        });
    }

    {
        let estado = estado.clone();
        ui.on_lote_cancelar(move || {
            if let Some(b) = estado.borrow().buscador.as_ref() {
                b.cancelar_lote();
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_revisao_decidir(move |posicao, aceitar| {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let todas = posicao < 0;
            let escolhidas: Vec<(metadados::Consulta, metadados::Candidato)> = if todas {
                std::mem::take(&mut e.revisao)
            } else if (posicao as usize) < e.revisao.len() {
                vec![e.revisao.remove(posicao as usize)]
            } else {
                return;
            };
            if aceitar {
                for (consulta, c) in escolhidas {
                    aplicar_correcao(&mut e, &consulta.chave, metadados::Correcao::from(&c));
                }
                e.metadados.salvar();
                renovar_vistas(&mut e, &ui);
                atualizar_painel(&mut e, &ui);
            }
            publicar_lote(&e, &ui);
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_gravar_tags_aba(move || {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let aba = e.aba_visivel;
            let atual = caminho_atual(&e);
            let Some(pasta) = e.pastas.get(aba) else {
                return;
            };
            // A faixa tocando fica de fora: o decodificador está lendo o
            // arquivo, e reescrever as tags no começo dele estragaria o som
            let itens: Vec<(PathBuf, metadados::Correcao)> = (0..pasta.tracks.len())
                .filter_map(|i| {
                    let caminho = pasta.caminho_faixa(i)?;
                    let correcao = e.metadados.correcao(&pasta.chave_faixa(i)?)?;
                    (Some(&caminho) != atual.as_ref()).then(|| (caminho, correcao.clone()))
                })
                .collect();
            let lang = translations::get_translations(&e.idioma);
            if itens.is_empty() {
                ui.set_tags_texto(lang.get("tags_none").copied().unwrap_or("").into());
                return;
            }
            ui.set_tags_texto(lang.get("tags_writing").copied().unwrap_or("").into());
            pedir_web(&mut e, buscador::Pedido::GravarTags(itens));
        });
    }

    {
        let estado = estado.clone();
        ui.on_alternar_letras_web(move |ligado| {
            let mut e = estado.borrow_mut();
            e.letras_web = ligado;
            salvar_configuracao(&e);
        });
    }

    {
        let estado = estado.clone();
        ui.on_chave_acoustid_mudou(move |chave| {
            let mut e = estado.borrow_mut();
            e.chave_acoustid = chave.trim().to_string();
            // O buscador leva a chave de quando nasceu
            if e.buscador.as_ref().is_some_and(|b| b.dispensavel_ja()) {
                e.buscador = None;
            }
            salvar_configuracao(&e);
        });
    }

    {
        let estado = estado.clone();
        ui.on_listenbrainz_mudou(move |token| {
            let mut e = estado.borrow_mut();
            e.contas.listenbrainz = token.trim().to_string();
            salvar_configuracao(&e);
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_lastfm_credenciais_mudou(move |chave, segredo| {
            let mut e = estado.borrow_mut();
            let mudou =
                e.contas.lastfm_chave != chave.trim() || e.contas.lastfm_segredo != segredo.trim();
            e.contas.lastfm_chave = chave.trim().to_string();
            e.contas.lastfm_segredo = segredo.trim().to_string();
            // Outra chave: a sessão era da antiga
            if mudou && !e.contas.lastfm_sessao.is_empty() {
                e.contas.lastfm_sessao.clear();
                e.lastfm_usuario.clear();
            }
            salvar_configuracao(&e);
            if let Some(ui) = ui_fraca.upgrade() {
                publicar_lastfm(&e, &ui);
            }
        });
    }

    {
        let estado = estado.clone();
        ui.on_lastfm_conectar(move || {
            let mut e = estado.borrow_mut();
            let (chave, segredo) = (
                e.contas.lastfm_chave.clone(),
                e.contas.lastfm_segredo.clone(),
            );
            if chave.is_empty() || segredo.is_empty() {
                return;
            }
            pedir_web(&mut e, buscador::Pedido::LastfmToken { chave, segredo });
        });
    }

    {
        let estado = estado.clone();
        ui.on_lastfm_concluir(move || {
            let mut e = estado.borrow_mut();
            let Some(token) = e.lastfm_token.clone() else {
                return;
            };
            let (chave, segredo) = (
                e.contas.lastfm_chave.clone(),
                e.contas.lastfm_segredo.clone(),
            );
            pedir_web(
                &mut e,
                buscador::Pedido::LastfmSessao {
                    chave,
                    segredo,
                    token,
                },
            );
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_lastfm_desconectar(move || {
            let mut e = estado.borrow_mut();
            e.contas.lastfm_sessao.clear();
            e.lastfm_usuario.clear();
            e.lastfm_token = None;
            salvar_configuracao(&e);
            if let Some(ui) = ui_fraca.upgrade() {
                publicar_lastfm(&e, &ui);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edicao(ano: &str, faixa: &str, disco: &str) -> EdicaoInfo {
        EdicaoInfo {
            titulo: " Vento ".into(),
            artista: "Banda".into(),
            genero: "  ".into(),
            ano: ano.into(),
            faixa: faixa.into(),
            disco: disco.into(),
            ..Default::default()
        }
    }

    #[test]
    fn edicao_vira_correcao_manual() {
        let anterior = metadados::Correcao {
            lancamento: Some("mbid".into()),
            ..Default::default()
        };
        let c = correcao_da_edicao(&edicao("2004", "7", "2"), Some(&anterior)).unwrap();
        assert!(c.manual);
        assert_eq!(c.titulo, "Vento");
        assert_eq!(c.album.as_deref(), Some(""));
        assert_eq!(c.genero, None);
        assert_eq!(c.ano, Some(2004));
        assert_eq!(c.numero, 2007);
        // A capa baixada antes continua
        assert_eq!(c.lancamento.as_deref(), Some("mbid"));

        let vazia = correcao_da_edicao(&edicao("", "", ""), None).unwrap();
        assert_eq!((vazia.ano, vazia.numero), (None, 0));
    }

    #[test]
    fn numero_invalido_recusa_a_edicao() {
        assert!(correcao_da_edicao(&edicao("dois mil", "", ""), None).is_none());
        assert!(correcao_da_edicao(&edicao("", "1000", ""), None).is_none());
        assert!(correcao_da_edicao(&edicao("", "", "-1"), None).is_none());
    }
}
