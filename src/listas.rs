//! Abas e listas: scan, ordem de exibição, listas especiais (favoritas,
//! mais tocadas, álbum) e o que a lista da UI mostra.

use crate::*;

/// Marca na UI a posição da faixa atual na lista visível.
///
/// Só destaca quando a aba visível é a mesma de onde a faixa vem: marcar
/// "tocando" numa lista que não é a de origem confunde.
pub(crate) fn atualizar_selecao_visivel(estado: &EstadoAudio, ui: &MainWindow) {
    let pos = if estado.pasta_reproducao == Some(estado.aba_visivel) {
        estado
            .indice_atual
            .and_then(|idx| estado.indices_visiveis.iter().position(|&i| i == idx))
    } else {
        None
    };
    ui.set_indice_selecionado(pos.map_or(-1, |p| p as i32));
}

/// Reconstrói a lista da aba visível aplicando o filtro e atualiza a seleção.
pub(crate) fn atualizar_lista(estado: &mut EstadoAudio, ui: &MainWindow) {
    let indices = ordem_lista(estado, estado.aba_visivel, &estado.filtro);
    let modelo: Vec<LinhaLista> = match estado.pastas.get(estado.aba_visivel) {
        Some(pasta) => indices
            .iter()
            .map(|&i| LinhaLista {
                texto: texto_exibicao(&pasta.tracks[i]).into(),
                favorita: pasta
                    .chave_faixa(i)
                    .is_some_and(|k| estado.biblioteca.e_favorita(&k)),
            })
            .collect(),
        None => Vec::new(),
    };

    estado.indices_visiveis = indices;
    ui.set_musicas(ModelRc::new(VecModel::from(modelo)));
    ui.set_ordem_atual(
        estado
            .pastas
            .get(estado.aba_visivel)
            .map_or(0, |p| p.ordem.indice() as i32),
    );
    atualizar_favorita_atual(estado, ui);

    atualizar_selecao_visivel(estado, ui);
    atualizar_painel(estado, ui);
    // O filtro da aba que toca muda a ordem de navegação
    atualizar_fila(estado);
}

/// Espelha a lista de pastas e os índices das abas na UI.
pub(crate) fn atualizar_abas(estado: &EstadoAudio, ui: &MainWindow) {
    let lang = translations::get_translations(&estado.idioma);
    let nomes: Vec<SharedString> = estado
        .pastas
        .iter()
        .map(|p| match p.tipo {
            Tipo::Favoritas => lang.get("tab_favorites").copied().unwrap_or("").into(),
            Tipo::MaisTocadas => lang.get("tab_most_played").copied().unwrap_or("").into(),
            _ => p.nome.as_str().into(),
        })
        .collect();
    ui.set_abas(ModelRc::new(VecModel::from(nomes)));
    ui.set_aba_ativa(estado.aba_visivel as i32);
    ui.set_aba_tocando(estado.pasta_reproducao.map_or(-1, |i| i as i32));
}

/// Escaneia a pasta na primeira vez que ela é necessária. Idempotente.
pub(crate) fn garantir_pasta_carregada(estado: &mut EstadoAudio, indice: usize) {
    let escanear_sub = estado.escanear_subpastas;
    let Some(pasta) = estado.pastas.get(indice) else {
        return;
    };
    if pasta.carregada {
        return;
    }
    let chaves: Vec<String> = match pasta.tipo {
        Tipo::Favoritas => estado.biblioteca.favoritas().to_vec(),
        Tipo::MaisTocadas => estado
            .biblioteca
            .mais_tocadas(biblioteca::TAMANHO_MAIS_TOCADAS)
            .into_iter()
            .map(String::from)
            .collect(),
        Tipo::Pasta => Vec::new(),
        // A aba do álbum já nasce com as faixas
        Tipo::Album { .. } => return,
    };
    if pasta.tipo != Tipo::Pasta {
        let tracks = faixas_por_chave(&estado.pastas, &chaves, &estado.metadados);
        let pasta = &mut estado.pastas[indice];
        pasta.tracks = tracks;
        pasta.carregada = true;
        return;
    }

    let tracks = escanear(
        &estado.pastas[indice].caminho,
        escanear_sub,
        &estado.metadados,
    );
    let pasta = &mut estado.pastas[indice];
    pasta.tracks = tracks;
    pasta.carregada = true;
}

/// Faixas de uma pasta, em ordem de nome, com as correções da web por cima.
pub(crate) fn escanear(
    caminho: &Path,
    recursivo: bool,
    banco: &metadados::Banco,
) -> Vec<TrackInfo> {
    let mut tracks = Vec::new();
    coletar_arquivos_audio(caminho, caminho, recursivo, &mut tracks);
    tracks.sort_by(|a, b| a.path.cmp(&b.path));
    banco.aplicar(&mut tracks, |t| chave_faixa(caminho, &t.path));
    tracks
}

/// Faixas das listas especiais, a partir dos caminhos. Quem já está numa pasta
/// carregada vem de lá (sem ler o arquivo de novo); o resto é lido agora.
/// Arquivo que sumiu fica de fora.
pub(crate) fn faixas_por_chave(
    pastas: &[PastaMusical],
    chaves: &[String],
    banco: &metadados::Banco,
) -> Vec<TrackInfo> {
    let mut conhecidas: HashMap<String, &TrackInfo> = HashMap::new();
    for pasta in pastas
        .iter()
        .filter(|p| p.tipo == Tipo::Pasta && p.carregada)
    {
        for t in &pasta.tracks {
            conhecidas.insert(chave_faixa(&pasta.caminho, &t.path), t);
        }
    }
    chaves
        .iter()
        .filter_map(|chave| {
            let mut t = match conhecidas.get(chave) {
                Some(t) => (*t).clone(),
                None => {
                    let caminho = Path::new(chave);
                    if !caminho.is_file() {
                        return None;
                    }
                    let mut t = track::ler_faixa(caminho, chave.clone());
                    if let Some(c) = banco.correcao(chave) {
                        c.aplicar(&mut t);
                    }
                    t
                }
            };
            t.path = chave.clone();
            Some(t)
        })
        .collect()
}

/// Ordem de exibição e navegação de uma lista: filtro da busca e depois a
/// ordem escolhida para a aba.
pub(crate) fn ordem_lista(estado: &EstadoAudio, indice: usize, filtro: &str) -> Vec<usize> {
    let Some(pasta) = estado.pastas.get(indice) else {
        return Vec::new();
    };
    let mut ordem = ordem_filtrada(&pasta.tracks, filtro);
    track::ordenar(&mut ordem, &pasta.tracks, pasta.ordem, |i| {
        pasta
            .chave_faixa(i)
            .map_or(0, |k| estado.biblioteca.reproducoes(&k))
    });
    ordem
}

/// Acha de novo a faixa atual na lista que toca, pelo nome: depois de um
/// reescaneio ou de uma lista especial remontada, as posições mudam.
/// `false` quando ela não está mais lá.
pub(crate) fn reposicionar_atual(estado: &mut EstadoAudio) -> bool {
    let (Some(p), Some(nome)) = (estado.pasta_reproducao, estado.arquivo_atual.as_deref()) else {
        return true;
    };
    let novo = estado
        .pastas
        .get(p)
        .and_then(|pasta| pasta.tracks.iter().position(|t| t.path == nome));
    estado.indice_atual = novo;
    novo.is_some()
}

/// Remonta uma lista especial depois que as favoritas ou as contagens
/// mudaram. Se ela está tocando e a faixa atual sairia dela, espera: a lista
/// seria puxada debaixo do que toca. Volta a ser montada na próxima visita.
pub(crate) fn renovar_especial(estado: &mut EstadoAudio, ui: &MainWindow, indice: usize) {
    let Some(pasta) = estado.pastas.get_mut(indice) else {
        return;
    };
    pasta.carregada = false;
    let visivel = estado.aba_visivel == indice;
    let tocando = estado.pasta_reproducao == Some(indice);
    if !visivel && !tocando {
        return;
    }
    let antes = std::mem::take(&mut estado.pastas[indice].tracks);
    garantir_pasta_carregada(estado, indice);
    if tocando {
        if !reposicionar_atual(estado) {
            // Fica como estava até a faixa acabar
            let pasta = &mut estado.pastas[indice];
            pasta.tracks = antes;
            pasta.carregada = false;
            reposicionar_atual(estado);
            if visivel {
                atualizar_lista(estado, ui);
            }
            return;
        }
        estado.sacola_shuffle.clear();
        estado.sorteio.clear();
    }
    if visivel {
        atualizar_lista(estado, ui);
    }
}

/// Abre uma pasta: entra como nova aba, ou só troca para a aba já existente.
/// Não interfere na reprodução em andamento.
pub(crate) fn abrir_pasta(estado: &mut EstadoAudio, ui: &MainWindow, caminho: PathBuf) {
    if let Some(idx) = estado
        .pastas
        .iter()
        .position(|p| p.tipo == Tipo::Pasta && p.caminho == caminho)
    {
        estado.aba_visivel = idx;
        // Pode ser uma pasta que ainda não foi visitada nesta sessão
        garantir_pasta_carregada(estado, idx);
        atualizar_abas(estado, ui);
        atualizar_lista(estado, ui);
        return;
    }

    let tracks = escanear(&caminho, estado.escanear_subpastas, &estado.metadados);
    let mut pasta = PastaMusical::pasta(caminho);
    pasta.tracks = tracks;
    // Já escaneada acima: não passa pelo caminho preguiçoso
    pasta.carregada = true;
    // Antes da aba do álbum, que fica sempre por último
    let pos = estado
        .pastas
        .iter()
        .position(|p| matches!(p.tipo, Tipo::Album { .. }))
        .unwrap_or(estado.pastas.len());
    estado.pastas.insert(pos, pasta);
    if let Some(p) = estado.pasta_reproducao.as_mut()
        && *p >= pos
    {
        *p += 1;
    }
    estado.aba_visivel = pos;

    atualizar_abas(estado, ui);
    atualizar_lista(estado, ui);
}

/// Fecha uma aba. Se era a pasta que estava tocando, a reprodução para.
pub(crate) fn fechar_pasta(estado: &mut EstadoAudio, ui: &MainWindow, idx: usize) {
    // As listas especiais não fecham
    if idx < ESPECIAIS || idx >= estado.pastas.len() {
        return;
    }

    if estado.pasta_reproducao == Some(idx) {
        stop_music(estado, ui);
        estado.pasta_reproducao = None;
        estado.indice_atual = None;
        estado.arquivo_atual = None;
    } else if let Some(p) = estado.pasta_reproducao
        && p > idx
    {
        estado.pasta_reproducao = Some(p - 1);
    }

    estado.pastas.remove(idx);

    if estado.aba_visivel > idx {
        estado.aba_visivel -= 1;
    } else if estado.aba_visivel >= estado.pastas.len() {
        // Cai para a última aba restante, que pode nunca ter sido visitada
        estado.aba_visivel = estado.pastas.len().saturating_sub(1);
    }

    let aba = estado.aba_visivel;
    garantir_pasta_carregada(estado, aba);

    atualizar_abas(estado, ui);
    atualizar_lista(estado, ui);
    atualizar_imersivo(estado, ui);
}

/// Abre (ou troca) a aba do álbum com as faixas do grupo, na ordem do disco,
/// e toca a primeira.
pub(crate) fn abrir_album(estado: &mut EstadoAudio, ui: &MainWindow, grupo: &track::GrupoAlbum) {
    let Some(origem) = estado.pastas.get(estado.aba_visivel) else {
        return;
    };
    let mut tracks: Vec<TrackInfo> = grupo
        .faixas
        .iter()
        .filter_map(|&i| {
            let mut t = origem.tracks.get(i)?.clone();
            t.path = chave_faixa(&origem.caminho, &t.path);
            Some(t)
        })
        .collect();
    tracks.sort_by(|a, b| a.numero.cmp(&b.numero).then_with(|| a.path.cmp(&b.path)));
    let chave_origem = match &origem.tipo {
        Tipo::Album { origem } => origem.clone(),
        _ => origem.chave(),
    };
    let mut pasta = PastaMusical::especial(
        Tipo::Album {
            origem: chave_origem,
        },
        Ordem::Padrao,
    );
    pasta.nome = grupo.nome.clone();
    pasta.tracks = tracks;
    pasta.carregada = true;

    // Uma aba de álbum só, sempre a última
    let pos = match estado
        .pastas
        .iter()
        .position(|p| matches!(p.tipo, Tipo::Album { .. }))
    {
        Some(pos) => {
            estado.pastas[pos] = pasta;
            if estado.pasta_reproducao == Some(pos) {
                // Mesma aba, faixas novas: a sacola era do álbum anterior
                estado.sacola_shuffle.clear();
                estado.sorteio.clear();
            }
            pos
        }
        None => {
            estado.pastas.push(pasta);
            estado.pastas.len() - 1
        }
    };
    estado.aba_visivel = pos;
    // A busca da aba anterior esconderia faixas do álbum
    estado.filtro.clear();
    ui.set_texto_busca(SharedString::new());
    atualizar_abas(estado, ui);
    atualizar_lista(estado, ui);
    tocar_faixa(estado, ui, pos, 0);
    ui.set_imv_guia(0);
}

/// Reescaneia todas as pastas (a opção de subpastas é global) e reposiciona a
/// faixa atual pelo caminho, já que os índices podem ter mudado.
pub(crate) fn reescaneiar_pastas(estado: &mut EstadoAudio, ui: &MainWindow) {
    let recursivo = estado.escanear_subpastas;
    for pasta in &mut estado.pastas {
        // Pastas ainda não carregadas serão escaneadas com a nova opção quando
        // forem abertas; carregá-las agora só gastaria memória à toa.
        if !pasta.carregada || pasta.tipo != Tipo::Pasta {
            continue;
        }
        pasta.tracks = escanear(&pasta.caminho, recursivo, &estado.metadados);
    }

    // Índices podem ter mudado com o reescaneio; a sacola do shuffle
    // inteligente não serve mais.
    estado.sacola_shuffle.clear();
    estado.sorteio.clear();

    if !reposicionar_atual(estado) {
        // A faixa atual já não está na pasta
        estado.pasta_reproducao = None;
        estado.indice_atual = None;
        estado.arquivo_atual = None;
        stop_music(estado, ui);
    }

    atualizar_abas(estado, ui);
    atualizar_lista(estado, ui);
    atualizar_imersivo(estado, ui);
}

/// Mais uma reprodução da faixa atual. A lista "Mais tocadas" é remontada
/// quando estiver à vista.
pub(crate) fn contar_reproducao(estado: &mut EstadoAudio, ui: &MainWindow) {
    estado.contada = true;
    let Some(chave) = estado
        .pasta_reproducao
        .and_then(|p| estado.pastas.get(p))
        .zip(estado.arquivo_atual.as_deref())
        .map(|(p, nome)| chave_faixa(&p.caminho, nome))
    else {
        return;
    };
    estado.biblioteca.contar(&chave);
    estado.biblioteca.salvar();
    renovar_especial(estado, ui, POS_MAIS_TOCADAS);
}

/// Marca ou desmarca uma faixa como favorita e atualiza o que mostra isso.
pub(crate) fn alternar_favorita(estado: &mut EstadoAudio, ui: &MainWindow, chave: &str) {
    estado.biblioteca.alternar_favorita(chave);
    estado.biblioteca.salvar();
    renovar_especial(estado, ui, POS_FAVORITAS);
    // A estrela das linhas (a lista especial já remontou a sua)
    if estado.aba_visivel != POS_FAVORITAS {
        atualizar_lista(estado, ui);
    }
    atualizar_favorita_atual(estado, ui);
    atualizar_fila(estado);
}

/// Estrela da faixa que toca (modo imersivo e atalho F).
pub(crate) fn atualizar_favorita_atual(estado: &EstadoAudio, ui: &MainWindow) {
    let favorita = chave_atual(estado).is_some_and(|k| estado.biblioteca.e_favorita(&k));
    ui.set_favorita_atual(favorita);
}

pub(crate) fn chave_atual(estado: &EstadoAudio) -> Option<String> {
    let pasta = estado.pastas.get(estado.pasta_reproducao?)?;
    Some(chave_faixa(
        &pasta.caminho,
        estado.arquivo_atual.as_deref()?,
    ))
}

/// Liga os callbacks da UI desta área.
pub(crate) fn ligar(ui: &MainWindow, estado: &Rc<RefCell<EstadoAudio>>) {
    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_abrir_pasta(move || {
            if let Some(caminho) = rfd::FileDialog::new().pick_folder()
                && let Some(ui) = ui_fraca.upgrade()
            {
                let mut e = estado.borrow_mut();
                // Pasta nova: limpa a busca
                e.filtro.clear();
                ui.set_texto_busca("".into());
                abrir_pasta(&mut e, &ui, caminho);
                salvar_configuracao(&e);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_aba_selecionada(move |indice| {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                let idx = indice as usize;
                if idx >= e.pastas.len() {
                    return;
                }
                // Navegar não é reproduzir: só muda o que a lista exibe
                e.aba_visivel = idx;
                // Carrega as faixas na primeira visita a esta aba
                garantir_pasta_carregada(&mut e, idx);
                atualizar_abas(&e, &ui);
                atualizar_lista(&mut e, &ui);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_fechar_aba(move |indice| {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                fechar_pasta(&mut e, &ui, indice as usize);
                salvar_configuracao(&e);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_alternar_escanear_subpastas(move |ligado| {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                e.escanear_subpastas = ligado;
                salvar_configuracao(&e);
                reescaneiar_pastas(&mut e, &ui);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_musica_selecionada(move |indice| {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                // `indice` é a posição na lista filtrada da aba visível;
                // converte para o índice real e toca a partir dessa pasta.
                let track_idx = match e.indices_visiveis.get(indice as usize) {
                    Some(&i) => i,
                    None => return,
                };
                let pasta_idx = e.aba_visivel;
                tocar_faixa(&mut e, &ui, pasta_idx, track_idx);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_busca_mudou(move |texto| {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                e.filtro = texto.to_string();
                atualizar_lista(&mut e, &ui);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_ordem_escolhida(move |indice| {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let aba = e.aba_visivel;
            let Some(pasta) = e.pastas.get_mut(aba) else {
                return;
            };
            pasta.ordem = Ordem::de_indice(indice.max(0) as usize);
            // A ordem nova é a da navegação: a fila em ordem muda, a do
            // shuffle não (sorteio não depende de ordem)
            atualizar_lista(&mut e, &ui);
            salvar_configuracao(&e);
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_favorita_alternada(move |posicao| {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let Some(chave) = e
                .indices_visiveis
                .get(posicao.max(0) as usize)
                .and_then(|&i| e.pastas.get(e.aba_visivel)?.chave_faixa(i))
            else {
                return;
            };
            alternar_favorita(&mut e, &ui, &chave);
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_alternar_favorita_atual(move || {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            if let Some(chave) = chave_atual(&e) {
                alternar_favorita(&mut e, &ui, &chave);
            }
        });
    }
}
