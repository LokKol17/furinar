//! Ponte entre o estado do app e o modo imersivo: faixa exibida, painel da
//! biblioteca, fila "a seguir", prévia da próxima e relógio.

use crate::*;

/// Resumo dos fundos globais nas configurações: o nome do arquivo, ou
/// quantos são ("" = nenhum).
pub(crate) fn nome_fundo(fundos: &[PathBuf], idioma: &str) -> String {
    match fundos {
        [] => String::new(),
        [um] => um
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        varios => {
            let lang = translations::get_translations(idioma);
            lang.get("bg_count")
                .copied()
                .unwrap_or("{n}")
                .replace("{n}", &varios.len().to_string())
        }
    }
}

/// Faixa atual no formato que o modo imersivo usa.
pub(crate) fn faixa_imersiva(estado: &EstadoAudio) -> Option<imersivo::Faixa> {
    let pasta = estado.pastas.get(estado.pasta_reproducao?)?;
    let indice = estado.indice_atual?;
    let track = pasta.tracks.get(indice)?;
    let correcao = estado
        .metadados
        .correcao(&chave_faixa(&pasta.caminho, &track.path));
    Some(imersivo::Faixa {
        capa_web: correcao
            .and_then(|c| c.lancamento.as_deref())
            .and_then(metadados::capa_web),
        album: correcao.and_then(|c| c.album.clone()),
        ano: correcao.and_then(|c| c.ano),
        caminho: pasta.caminho.join(&track.path),
        pasta_raiz: pasta.raiz_da_faixa(indice),
        titulo: track.titulo.clone(),
        artista: track.artista.clone(),
        fundos_globais: estado.fundos_globais.clone(),
    })
}

/// Avisa o modo imersivo (se ligado) que a faixa atual pode ter mudado.
pub(crate) fn atualizar_imersivo(estado: &mut EstadoAudio, ui: &MainWindow) {
    if estado.imersivo.is_none() {
        return;
    }
    let faixa = faixa_imersiva(estado);
    if let Some(imv) = estado.imersivo.as_mut() {
        imv.trocar_faixa(ui, faixa);
    }
    atualizar_painel(estado, ui);
    atualizar_fila(estado);
}

/// Republica a janela de linhas do painel da biblioteca (se aberto).
pub(crate) fn atualizar_painel(estado: &mut EstadoAudio, ui: &MainWindow) {
    let Some(painel) = estado.imersivo.as_mut().and_then(|i| i.painel.as_mut()) else {
        return;
    };
    let pasta = estado.pastas.get(estado.aba_visivel);
    let tocando = if estado.pasta_reproducao == Some(estado.aba_visivel) {
        estado.indice_atual
    } else {
        None
    };
    let indices = &estado.indices_visiveis;
    let bib = &estado.biblioteca;
    let banco = &estado.metadados;
    painel.publicar(ui, indices.len(), |pos| {
        item_painel(pasta?, *indices.get(pos)?, tocando, bib, banco)
    });
    if painel.mostrando_albuns()
        && let Some(pasta) = pasta
    {
        let lang = translations::get_translations(&estado.idioma);
        painel.publicar_albuns(track::agrupar_albuns(&pasta.tracks, indices), &lang, |i| {
            Some((pasta.caminho_faixa(i)?, capa_web_de(banco, pasta, i)))
        });
    }
}

/// Capa baixada da web para a faixa (se a correção dela trouxe uma).
pub(crate) fn capa_web_de(
    banco: &metadados::Banco,
    pasta: &PastaMusical,
    indice: usize,
) -> Option<PathBuf> {
    let correcao = banco.correcao(&pasta.chave_faixa(indice)?)?;
    metadados::capa_web(correcao.lancamento.as_deref()?)
}

pub(crate) fn item_painel(
    pasta: &PastaMusical,
    indice: usize,
    tocando: Option<usize>,
    bib: &Biblioteca,
    banco: &metadados::Banco,
) -> Option<painel::Item> {
    let t = pasta.tracks.get(indice)?;
    Some(painel::Item {
        caminho: pasta.caminho.join(&t.path),
        indice,
        titulo: t.titulo.clone(),
        artista: t.artista.clone(),
        album: t.album.clone(),
        duracao: t.duracao,
        tocando: tocando == Some(indice),
        favorita: pasta
            .chave_faixa(indice)
            .is_some_and(|k| bib.e_favorita(&k)),
        capa_web: capa_web_de(banco, pasta, indice),
    })
}

/// Quantas faixas a fila "a seguir" mostra.
pub(crate) const TAMANHO_FILA: usize = 5;

/// Republica a fila "a seguir" do painel (se aberto).
pub(crate) fn atualizar_fila(estado: &mut EstadoAudio) {
    let aberto = estado.imersivo.as_ref().is_some_and(|i| i.painel.is_some());
    if !aberto {
        return;
    }
    let itens = fila_a_seguir(estado, TAMANHO_FILA);
    if let Some(painel) = estado.imersivo.as_mut().and_then(|i| i.painel.as_mut()) {
        painel.publicar_fila(itens);
    }
}

/// As próximas `n` como itens do painel. A engatilhada (gapless) já saiu da
/// fila do shuffle, mas é a primeira a tocar.
pub(crate) fn fila_a_seguir(estado: &mut EstadoAudio, n: usize) -> Vec<painel::Item> {
    // Em ordem nada é consumido: a engatilhada já é a primeira de `proximas`
    let engatilhada = estado.engatilhada.as_ref().map(|e| e.indice);
    let consumida = estado.modo_shuffle != 0 || engatilhada == estado.indice_atual;
    let mut indices: Vec<usize> = engatilhada.filter(|_| consumida).into_iter().collect();
    let resto = proximas(estado, n.saturating_sub(indices.len()));
    indices.extend(resto);
    match estado.pasta_reproducao.and_then(|p| estado.pastas.get(p)) {
        Some(pasta) => indices
            .iter()
            .filter_map(|&i| item_painel(pasta, i, None, &estado.biblioteca, &estado.metadados))
            .collect(),
        None => Vec::new(),
    }
}

/// Prévia "a seguir" do modo imersivo nos últimos segundos da faixa.
pub(crate) const PREVIA_SEGUNDOS: f64 = 10.0;

pub(crate) fn atualizar_previa(estado: &mut EstadoAudio, ui: &MainWindow) {
    let restante = estado.duracao_total_secs as f64 - estado.tempo_decorrido;
    let tocando = estado.audio_player.as_ref().is_some_and(|p| !p.is_paused());
    let mostrar = tocando
        && estado.duracao_total_secs > 0
        && restante <= PREVIA_SEGUNDOS
        && estado.modo_loop != 1
        && estado.sono != Some(sono::Sono::FimDaFaixa);
    let texto = if mostrar {
        fila_a_seguir(estado, 1)
            .first()
            .map(|i| match &i.artista {
                Some(a) => format!("{} — {a}", i.titulo),
                None => i.titulo.clone(),
            })
            .unwrap_or_default()
    } else {
        String::new()
    };
    if ui.get_imv_proxima() != texto.as_str() {
        ui.set_imv_proxima(texto.into());
    }
}

/// Relógio do modo imersivo: só mexe na UI quando o minuto vira.
pub(crate) fn atualizar_relogio(ui: &MainWindow) {
    let agora = chrono::Local::now().format("%H:%M").to_string();
    if ui.get_imv_relogio() != agora.as_str() {
        ui.set_imv_relogio(agora.into());
    }
}

/// Liga os callbacks da UI desta área.
pub(crate) fn ligar(ui: &MainWindow, estado: &Rc<RefCell<EstadoAudio>>) {
    // Modo imersivo: o timer de ~30 fps só roda enquanto o modo está ligado
    let timer_imersivo = Rc::new(Timer::default());
    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        let timer_imersivo = timer_imersivo.clone();
        ui.on_alternar_imersivo(move || {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();

            if e.imersivo.take().is_some() {
                // Sair: o Drop do estado derruba threads e imagens pendentes
                timer_imersivo.stop();
                e.captura.ligar(false);
                imersivo::limpar_ui(&ui);
                ui.set_modo_imersivo(false);
                ui.window().set_fullscreen(false);
                // Depois que a janela repintar sem as imagens
                Timer::single_shot(Duration::from_millis(500), memoria::aparar);
                return;
            }

            e.imersivo = Some(imersivo::EstadoImersivo::novo(&ui));
            e.captura.ligar(true);
            ui.set_modo_imersivo(true);
            ui.window().set_fullscreen(true);
            atualizar_imersivo(&mut e, &ui);
            atualizar_relogio(&ui);

            let estado = estado.clone();
            let ui_fraca = ui.as_weak();
            timer_imersivo.start(TimerMode::Repeated, Duration::from_millis(33), move || {
                let Some(ui) = ui_fraca.upgrade() else {
                    return;
                };
                let mut e = estado.borrow_mut();
                let e = &mut *e;
                let tocando = e.audio_player.as_ref().is_some_and(|p| !p.is_paused());
                if let Some(imv) = e.imersivo.as_mut() {
                    imv.tick(&ui, &e.idioma, &e.captura, tocando, e.fundo_reativo);
                }
            });
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_escolher_fundo(move || {
            let extensoes = fundo::extensoes_aceitas();
            // Vários de uma vez; somam aos que já estavam (rodízio)
            let Some(caminhos) = rfd::FileDialog::new()
                .add_filter("Imagem, GIF/WebP ou vídeo", &extensoes)
                .pick_files()
            else {
                return;
            };
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                for c in caminhos {
                    if !e.fundos_globais.contains(&c) {
                        e.fundos_globais.push(c);
                    }
                }
                ui.set_fundo_global_nome(nome_fundo(&e.fundos_globais, &e.idioma).into());
                salvar_configuracao(&e);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_limpar_fundo(move || {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                e.fundos_globais.clear();
                ui.set_fundo_global_nome(SharedString::new());
                salvar_configuracao(&e);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_imv_painel_alternado(move |aberto| {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let Some(imv) = e.imersivo.as_mut() else {
                return;
            };
            if aberto {
                imv.painel = Some(painel::Painel::novo(&ui));
                atualizar_painel(&mut e, &ui);
                atualizar_fila(&mut e);
            } else {
                // Fechado não segura miniatura nem thread
                imv.painel = None;
                painel::limpar_ui(&ui);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_imv_painel_rolou(move |primeira, quantas| {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let mudou = e
                .imersivo
                .as_mut()
                .and_then(|i| i.painel.as_mut())
                .is_some_and(|p| p.rolou(primeira.max(0) as usize, quantas.max(0) as usize));
            if mudou {
                atualizar_painel(&mut e, &ui);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_imv_fila_clicada(move |posicao| {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let Some(indice) = e
                .imersivo
                .as_ref()
                .and_then(|i| i.painel.as_ref())
                .and_then(|p| p.indice_da_fila(posicao.max(0) as usize))
            else {
                return;
            };
            let Some(pasta_idx) = e.pasta_reproducao else {
                return;
            };
            // Clicou em outra que não a engatilhada: ela volta para a fila
            if let Some(eng) = e.engatilhada.take()
                && eng.indice != indice
            {
                devolver_a_fila(&mut e, eng.indice);
            }
            // Pular para uma faixa da fila consome a fila até ela
            match e.modo_shuffle {
                2 => e.sacola_shuffle.retain(|&i| i != indice),
                1 => {
                    if let Some(pos) = e.sorteio.iter().position(|&i| i == indice) {
                        e.sorteio.drain(..=pos);
                    }
                }
                _ => {}
            }
            tocar_faixa(&mut e, &ui, pasta_idx, indice);
        });
    }

    {
        let estado = estado.clone();
        ui.on_alternar_fundo_reativo(move |ligado| {
            let mut e = estado.borrow_mut();
            e.fundo_reativo = ligado;
            salvar_configuracao(&e);
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_imv_guia_mudou(move |guia| {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            if let Some(p) = e.imersivo.as_mut().and_then(|i| i.painel.as_mut()) {
                p.mostrar_albuns(guia == painel::GUIA_ALBUNS);
            }
            atualizar_painel(&mut e, &ui);
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_imv_albuns_rolou(move |primeira, quantas| {
            if ui_fraca.upgrade().is_none() {
                return;
            }
            let mut e = estado.borrow_mut();
            if let Some(p) = e.imersivo.as_mut().and_then(|i| i.painel.as_mut()) {
                p.albuns_rolou(primeira.max(0) as usize, quantas.max(0) as usize);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_imv_album_clicado(move |posicao| {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let Some(grupo) = e
                .imersivo
                .as_ref()
                .and_then(|i| i.painel.as_ref())
                .and_then(|p| p.album(posicao.max(0) as usize))
                .cloned()
            else {
                return;
            };
            abrir_album(&mut e, &ui, &grupo);
        });
    }
}
