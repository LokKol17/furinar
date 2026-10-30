//! Playlists na UI: menus das faixas e das abas, o botão "+ playlist" da
//! faixa atual e o diálogo de nome/confirmação. Os dados e o formato ficam
//! em `playlists.rs`.

use crate::*;

/// Diálogo aberto (`dialogo_modo` na UI).
const DIALOGO_NOVA: i32 = 1;
const DIALOGO_RENOMEAR: i32 = 2;
const DIALOGO_APAGAR: i32 = 3;

fn t(estado: &EstadoAudio, chave: &str) -> String {
    translations::get_translations(&estado.idioma)
        .get(chave)
        .copied()
        .unwrap_or("")
        .to_string()
}

/// As abas de playlist existentes no começo da sessão, depois das especiais.
pub(crate) fn carregar_abas(estado: &mut EstadoAudio) {
    for arquivo in playlists::listar(&playlists::pasta()) {
        let pos = listas::fim_playlists(estado);
        listas::inserir_aba(estado, pos, PastaMusical::playlist(arquivo));
    }
}

/// Aba da `n`-ésima playlist (a ordem das abas).
fn aba_da_playlist(estado: &EstadoAudio, n: i32) -> Option<usize> {
    let aba = ESPECIAIS + usize::try_from(n).ok()?;
    (aba < listas::fim_playlists(estado)).then_some(aba)
}

fn arquivo(estado: &EstadoAudio, aba: usize) -> Option<PathBuf> {
    match &estado.pastas.get(aba)?.tipo {
        Tipo::Playlist { arquivo } => Some(arquivo.clone()),
        _ => None,
    }
}

/// Chave da faixa na linha `linha` da lista visível.
fn chave_da_linha(estado: &EstadoAudio, linha: i32) -> Option<String> {
    let i = *estado.indices_visiveis.get(usize::try_from(linha).ok()?)?;
    estado.pastas.get(estado.aba_visivel)?.chave_faixa(i)
}

/// Grava as entradas, com título/duração das faixas que alguma aba
/// carregada conhece (para o `#EXTINF`).
fn gravar(estado: &EstadoAudio, arquivo: &Path, entradas: &[String]) -> bool {
    let mut conhecidas: HashMap<String, &TrackInfo> = HashMap::new();
    for pasta in estado.pastas.iter().filter(|p| p.carregada) {
        for (i, t) in pasta.tracks.iter().enumerate() {
            if let Some(chave) = pasta.chave_faixa(i) {
                conhecidas.entry(chave).or_insert(t);
            }
        }
    }
    match playlists::escrever(arquivo, entradas, |k| {
        conhecidas.get(k).map(|t| (*t).clone())
    }) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("playlist {}: {e}", arquivo.display());
            false
        }
    }
}

/// Depois de mudar o arquivo: remonta a aba (e a fila, se ela toca).
fn renovar(estado: &mut EstadoAudio, ui: &MainWindow, aba: usize) {
    if estado.pasta_reproducao == Some(aba) {
        desengatilhar(estado, ui);
    }
    renovar_especial(estado, ui, aba);
    atualizar_painel(estado, ui);
}

/// Põe `chave` na playlist da aba `aba`, com aviso.
fn adicionar(estado: &mut EstadoAudio, ui: &MainWindow, aba: usize, chave: &str) {
    let Some(arquivo) = arquivo(estado, aba) else {
        return;
    };
    let nome = estado.pastas[aba].nome.clone();
    let mut entradas = playlists::ler(&arquivo);
    if !playlists::adicionar(&mut entradas, chave) {
        avisar(ui, &t(estado, "playlist_already").replace("{nome}", &nome));
        return;
    }
    if !gravar(estado, &arquivo, &entradas) {
        avisar(
            ui,
            &t(estado, "playlist_save_failed").replace("{nome}", &nome),
        );
        return;
    }
    renovar(estado, ui, aba);
    avisar(ui, &t(estado, "playlist_added").replace("{nome}", &nome));
}

/// Cria a playlist `nome` (já com a faixa pendente, se houver). `Some(aba)`
/// se deu certo.
fn criar(estado: &mut EstadoAudio, ui: &MainWindow, nome: &str) -> Option<usize> {
    let nome = playlists::nome_seguro(nome)?;
    let pasta = playlists::pasta();
    let arquivo = playlists::caminho_livre(&pasta, &nome, None);
    let entradas: Vec<String> = estado.playlist_pendente.take().into_iter().collect();
    if !gravar(estado, &arquivo, &entradas) {
        avisar(
            ui,
            &t(estado, "playlist_save_failed").replace("{nome}", &nome),
        );
        return None;
    }
    // Na ordem do nome, entre as outras playlists
    let chave_nome = track::normalizar_busca(&playlists::nome(&arquivo));
    let fim = listas::fim_playlists(estado);
    let pos = (ESPECIAIS..fim)
        .find(|&i| track::normalizar_busca(&estado.pastas[i].nome) > chave_nome)
        .unwrap_or(fim);
    listas::inserir_aba(estado, pos, PastaMusical::playlist(arquivo));
    let texto = if entradas.is_empty() {
        "playlist_created"
    } else {
        "playlist_created_with"
    };
    avisar(
        ui,
        &t(estado, texto).replace("{nome}", &estado.pastas[pos].nome),
    );
    Some(pos)
}

/// Mostra a aba `aba` (carregando, se preciso).
fn mostrar(estado: &mut EstadoAudio, ui: &MainWindow, aba: usize) {
    estado.aba_visivel = aba;
    estado.filtro.clear();
    ui.set_texto_busca(SharedString::new());
    garantir_pasta_carregada(estado, aba);
    atualizar_abas(estado, ui);
    atualizar_lista(estado, ui);
}

fn abrir_dialogo(ui: &MainWindow, modo: i32, texto: &str) {
    ui.set_dialogo_texto(texto.into());
    ui.set_dialogo_modo(modo);
}

/// Liga os callbacks da UI desta área.
pub(crate) fn ligar(ui: &MainWindow, estado: &Rc<RefCell<EstadoAudio>>) {
    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_playlist_adicionar(move |linha, playlist| {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let Some(chave) = chave_da_linha(&e, linha) else {
                return;
            };
            match aba_da_playlist(&e, playlist) {
                Some(aba) => adicionar(&mut e, &ui, aba, &chave),
                None => {
                    e.playlist_pendente = Some(chave);
                    abrir_dialogo(&ui, DIALOGO_NOVA, "");
                }
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_playlist_adicionar_atual(move |playlist| {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let Some(chave) = chave_atual(&e) else {
                return;
            };
            match aba_da_playlist(&e, playlist) {
                Some(aba) => adicionar(&mut e, &ui, aba, &chave),
                None => {
                    e.playlist_pendente = Some(chave);
                    abrir_dialogo(&ui, DIALOGO_NOVA, "");
                }
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_playlist_remover(move |linha| {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let aba = e.aba_visivel;
            let (Some(arquivo), Some(chave)) = (arquivo(&e, aba), chave_da_linha(&e, linha)) else {
                return;
            };
            let mut entradas = playlists::ler(&arquivo);
            if playlists::remover(&mut entradas, &chave) && gravar(&e, &arquivo, &entradas) {
                renovar(&mut e, &ui, aba);
                let nome = e.pastas[aba].nome.clone();
                avisar(&ui, &t(&e, "playlist_removed").replace("{nome}", &nome));
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_playlist_mover(move |linha, delta| {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let aba = e.aba_visivel;
            let (Some(arquivo), Some(chave)) = (arquivo(&e, aba), chave_da_linha(&e, linha)) else {
                return;
            };
            // A ordem visível só é a do arquivo sem busca e na ordem padrão
            if !e.filtro.is_empty() || e.pastas[aba].ordem != Ordem::Padrao {
                return;
            }
            let visiveis: Vec<String> = e
                .indices_visiveis
                .iter()
                .filter_map(|&i| e.pastas[aba].chave_faixa(i))
                .collect();
            let mut entradas = playlists::ler(&arquivo);
            if playlists::mover(&mut entradas, &visiveis, &chave, delta)
                && gravar(&e, &arquivo, &entradas)
            {
                renovar(&mut e, &ui, aba);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_playlist_nova(move || {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            estado.borrow_mut().playlist_pendente = None;
            abrir_dialogo(&ui, DIALOGO_NOVA, "");
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_playlist_renomear(move |aba| {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let aba = aba.max(0) as usize;
            if arquivo(&e, aba).is_some() {
                e.playlist_alvo = Some(aba);
                let nome = e.pastas[aba].nome.clone();
                abrir_dialogo(&ui, DIALOGO_RENOMEAR, &nome);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_playlist_apagar(move |aba| {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let aba = aba.max(0) as usize;
            if arquivo(&e, aba).is_some() {
                e.playlist_alvo = Some(aba);
                let nome = e.pastas[aba].nome.clone();
                ui.set_dialogo_mensagem(
                    t(&e, "playlist_delete_msg").replace("{nome}", &nome).into(),
                );
                abrir_dialogo(&ui, DIALOGO_APAGAR, &nome);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_playlist_importar(move || {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let Some(origem) = rfd::FileDialog::new()
                .add_filter("M3U", &["m3u8", "m3u"])
                .pick_file()
            else {
                return;
            };
            let mut e = estado.borrow_mut();
            // Lido aqui (caminhos relativos resolvem contra a pasta de
            // origem) e gravado absoluto: a cópia não depende de onde estava
            e.playlist_pendente = None;
            let entradas = playlists::ler(&origem);
            let Some(aba) = criar(&mut e, &ui, &playlists::nome(&origem)) else {
                return;
            };
            if let Some(destino) = arquivo(&e, aba)
                && gravar(&e, &destino, &entradas)
            {
                mostrar(&mut e, &ui, aba);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_dialogo_confirmar(move |texto| {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            let modo = ui.get_dialogo_modo();
            match modo {
                DIALOGO_NOVA => {
                    if playlists::nome_seguro(&texto).is_none() {
                        return;
                    }
                    // Criada pelo "+" das abas: vai para ela. Pelo menu de
                    // uma faixa: fica onde está (o aviso diz para onde foi)
                    let de_uma_faixa = e.playlist_pendente.is_some();
                    if let Some(aba) = criar(&mut e, &ui, &texto) {
                        if de_uma_faixa {
                            atualizar_abas(&e, &ui);
                        } else {
                            mostrar(&mut e, &ui, aba);
                        }
                    }
                }
                DIALOGO_RENOMEAR => {
                    let (Some(aba), Some(nome)) = (e.playlist_alvo, playlists::nome_seguro(&texto))
                    else {
                        return;
                    };
                    let Some(atual) = arquivo(&e, aba) else {
                        return;
                    };
                    let novo = playlists::caminho_livre(&playlists::pasta(), &nome, Some(&atual));
                    if let Err(erro) = fs::rename(&atual, &novo) {
                        eprintln!("renomear playlist: {erro}");
                        avisar(&ui, &t(&e, "playlist_save_failed").replace("{nome}", &nome));
                        return;
                    }
                    let pasta = &mut e.pastas[aba];
                    pasta.nome = playlists::nome(&novo);
                    pasta.tipo = Tipo::Playlist { arquivo: novo };
                    atualizar_abas(&e, &ui);
                    salvar_configuracao(&e);
                }
                DIALOGO_APAGAR => {
                    let Some(aba) = e.playlist_alvo else {
                        return;
                    };
                    let Some(atual) = arquivo(&e, aba) else {
                        return;
                    };
                    let nome = e.pastas[aba].nome.clone();
                    if let Err(erro) = fs::remove_file(&atual) {
                        eprintln!("apagar playlist: {erro}");
                        avisar(&ui, &t(&e, "playlist_save_failed").replace("{nome}", &nome));
                        return;
                    }
                    listas::remover_aba(&mut e, &ui, aba);
                    salvar_configuracao(&e);
                    avisar(&ui, &t(&e, "playlist_deleted").replace("{nome}", &nome));
                }
                _ => {}
            }
            e.playlist_alvo = None;
            ui.set_dialogo_modo(0);
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_dialogo_cancelar(move || {
            let Some(ui) = ui_fraca.upgrade() else {
                return;
            };
            let mut e = estado.borrow_mut();
            e.playlist_pendente = None;
            e.playlist_alvo = None;
            ui.set_dialogo_modo(0);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Inserir aba no meio (playlist nova) desloca a que toca e a visível.
    #[test]
    fn inserir_aba_desloca_os_indices() {
        let mut e = EstadoAudio::default();
        e.pastas
            .push(PastaMusical::especial(Tipo::Favoritas, Ordem::Padrao));
        e.pastas
            .push(PastaMusical::especial(Tipo::MaisTocadas, Ordem::Padrao));
        e.pastas.push(PastaMusical::pasta(PathBuf::from("/m")));
        e.pasta_reproducao = Some(2);
        e.aba_visivel = 2;
        listas::inserir_aba(
            &mut e,
            2,
            PastaMusical::playlist(PathBuf::from("/p/A.m3u8")),
        );
        assert_eq!(e.pasta_reproducao, Some(3));
        assert_eq!(e.aba_visivel, 3);
        assert_eq!(listas::fim_playlists(&e), 3);
        assert_eq!(aba_da_playlist(&e, 0), Some(2));
        assert_eq!(aba_da_playlist(&e, 1), None);
        assert_eq!(e.pastas[2].nome, "A");
        assert_eq!(e.pastas[2].chave(), "/p/A.m3u8");
    }
}
