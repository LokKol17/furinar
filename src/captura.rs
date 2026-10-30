//! Capturas de tela sem janela, para inspeção visual (testes manuais,
//! `--ignored`). As imagens saem em `target/`.

use crate::*;

fn fixture(nome: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(nome)
}

/// Renderiza o modo normal num PNG (`target/captura-normal.png`), com as
/// faixas da pasta de `FURINAR_CAPTURA_FAIXA`. Manual, como a do imersivo.
#[test]
#[ignore = "gera PNG para inspeção visual"]
fn captura_modo_normal() {
    use slint::platform::software_renderer::{
        MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
    };
    use slint::platform::{Platform, WindowAdapter};

    struct Plataforma(Rc<MinimalSoftwareWindow>);
    impl Platform for Plataforma {
        fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
            Ok(self.0.clone())
        }
    }

    // FURINAR_CAPTURA_MINI=1: o mini player (janela encolhida)
    let mini = std::env::var_os("FURINAR_CAPTURA_MINI").is_some();
    let (largura, altura) = if mini {
        (380u32, 112u32)
    } else {
        // FURINAR_CAPTURA_ALTURA=<px>: janela mais alta, para ver um cartão
        // de configurações inteiro sem rolar
        let altura = std::env::var("FURINAR_CAPTURA_ALTURA")
            .ok()
            .and_then(|a| a.parse().ok())
            .unwrap_or(734u32);
        (660u32, altura)
    };
    let janela = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Plataforma(janela.clone())))
        .expect("plataforma de teste");
    janela.set_size(slint::PhysicalSize::new(largura, altura));
    let ui = MainWindow::new().expect("MainWindow");
    ui.show().expect("show");
    aplicar_idioma(
        &std::env::var("FURINAR_CAPTURA_IDIOMA").unwrap_or_else(|_| "pt-br".into()),
        &ui,
    );

    let caminho = std::env::var_os("FURINAR_CAPTURA_FAIXA")
        .map(PathBuf::from)
        .unwrap_or_else(|| fixture("tone.mp3"));
    let raiz = caminho.parent().map(PathBuf::from).unwrap_or_default();
    let mut e = EstadoAudio::default();
    e.pastas
        .push(PastaMusical::especial(Tipo::Favoritas, Ordem::Padrao));
    e.pastas
        .push(PastaMusical::especial(Tipo::MaisTocadas, Ordem::Padrao));
    e.pastas.push(PastaMusical::pasta(raiz));
    e.aba_visivel = ESPECIAIS;
    garantir_pasta_carregada(&mut e, ESPECIAIS);
    e.pastas[ESPECIAIS].ordem = Ordem::Album;
    for i in [1, 3] {
        if let Some(k) = e.pastas[ESPECIAIS].chave_faixa(i) {
            e.biblioteca.alternar_favorita(&k);
        }
    }
    e.pasta_reproducao = Some(ESPECIAIS);
    e.arquivo_atual = e.pastas[ESPECIAIS].tracks.get(2).map(|t| t.path.clone());
    e.indice_atual = Some(2);
    // Uma playlist (só a aba; o arquivo não precisa existir). A pasta vai
    // para depois dela
    listas::inserir_aba(
        &mut e,
        ESPECIAIS,
        PastaMusical::playlist(PathBuf::from("/tmp/Para correr.m3u8")),
    );
    let pasta = ESPECIAIS + 1;
    atualizar_abas(&e, &ui);
    atualizar_lista(&mut e, &ui);
    ui.set_sono_texto("14:59".into());
    ui.set_texto_tempo("01:14 / 03:31".into());
    ui.set_progresso(0.35);
    publicar_crossfade(&e, &ui);
    // FURINAR_CAPTURA_LOOP / _SHUFFLE: modo dos botões (0, 1, 2)
    let modo = |v: &str| {
        std::env::var(v)
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    };
    ui.set_loop_modo(modo("FURINAR_CAPTURA_LOOP"));
    ui.set_shuffle_modo(modo("FURINAR_CAPTURA_SHUFFLE"));
    // Tocando: o indicador da linha atual aparece
    ui.set_tocando(true);
    // FURINAR_CAPTURA_TOAST=<texto>: aviso no rodapé
    if let Ok(texto) = std::env::var("FURINAR_CAPTURA_TOAST") {
        avisar(&ui, &texto);
    }
    // FURINAR_CAPTURA_DIALOGO=1|2|3: diálogo de playlist (nova, renomear,
    // apagar)
    if let Some(modo) = std::env::var("FURINAR_CAPTURA_DIALOGO")
        .ok()
        .and_then(|m| m.parse::<i32>().ok())
    {
        ui.set_dialogo_mensagem(
            "Apagar “Para correr”? As músicas continuam no disco; só a lista some.".into(),
        );
        ui.set_dialogo_texto(if modo == 1 { "" } else { "Para correr" }.into());
        ui.set_dialogo_modo(modo);
    }
    // FURINAR_CAPTURA_CONFIG=1: configurações abertas
    if std::env::var_os("FURINAR_CAPTURA_CONFIG").is_some() {
        ui.set_config_aberta(true);
    }
    if mini {
        ui.set_modo_mini(true);
        let t = &e.pastas[pasta].tracks[2];
        ui.set_mini_titulo(t.titulo.as_str().into());
        ui.set_mini_artista(t.artista.as_deref().unwrap_or("").into());
        if let Some(c) = e.pastas[pasta].caminho_faixa(2)
            && let Some(buf) = miniaturas::miniatura(&c, None)
        {
            ui.set_mini_capa(slint::Image::from_rgba8(buf));
            ui.set_mini_tem_capa(true);
        }
    }
    // FURINAR_CAPTURA_UPDATE=1..5: aviso de atualização naquele estado
    if let Some(estado) = std::env::var("FURINAR_CAPTURA_UPDATE")
        .ok()
        .and_then(|e| e.parse().ok())
    {
        ui.set_update_versao("1.7.2".into());
        ui.set_update_notas(
            updates::notas_legiveis(
                "### Adicionado\n\n- Instalador para Windows\n\n### Corrigido\n\n- O aviso de atualização não aparecia\n- O `SHA256SUMS` não era anexado às releases\n",
            )
            .into(),
        );
        ui.set_update_progresso(0.42);
        ui.set_update_erro("o arquivo baixado não confere com o SHA256SUMS".into());
        ui.set_update_estado(estado);
    }

    // FURINAR_CAPTURA_INFO=1 (=2 guia do lote, =3 edição à mão): informações
    if let Some(guia) = std::env::var_os("FURINAR_CAPTURA_INFO") {
        let candidato = |titulo: &str, confianca, fonte| metadados::Candidato {
            gravacao: "x".into(),
            titulo: titulo.into(),
            artista: "Queen".into(),
            album: Some("A Night at the Opera".into()),
            lancamento: None,
            ano: Some(1975),
            numero: 11,
            duracao: 355,
            confianca,
            fonte,
        };
        e.info_candidatos = vec![
            candidato("Bohemian Rhapsody", 100, metadados::Fonte::Texto),
            candidato("Bohemian Rhapsody (Live)", 83, metadados::Fonte::Texto),
            candidato("Bohemian Rhapsody", 61, metadados::Fonte::ImpressaoDigital),
        ];
        e.info_chave = e.pastas[pasta].chave_faixa(2);
        publicar_info(&e, &ui);
        ui.set_info_estado("3 resultados — escolha um".into());
        if guia == "2" {
            let consulta = consulta_de(&e, pasta, 2).expect("consulta");
            e.revisao = vec![
                (
                    consulta.clone(),
                    candidato("Bohemian Rhapsody", 76, metadados::Fonte::Texto),
                ),
                (
                    consulta,
                    candidato("Awake Now", 62, metadados::Fonte::Texto),
                ),
            ];
            e.lote = Lote {
                ativo: true,
                feitas: 34,
                total: 148,
                ..Default::default()
            };
            publicar_lote(&e, &ui);
            ui.set_info_guia(1);
        }
        publicar_edicao(&e, &ui);
        if guia == "3" {
            ui.set_info_guia(2);
        }
        ui.set_info_aberta(true);
    }

    // FURINAR_CAPTURA_DICA=x,y: ponteiro parado ali (mostra o tooltip)
    if let Some((x, y)) = std::env::var("FURINAR_CAPTURA_DICA")
        .ok()
        .and_then(|v| {
            v.split_once(',')
                .map(|(x, y)| (x.parse().ok(), y.parse().ok()))
        })
        .and_then(|(x, y)| x.zip(y))
    {
        janela.dispatch_event(slint::platform::WindowEvent::PointerMoved {
            position: slint::LogicalPosition::new(x, y),
        });
    }
    // Tempo real para as animações de entrada (~300 ms) e o atraso do
    // tooltip (500 ms) passarem
    for _ in 0..18 {
        slint::platform::update_timers_and_animations();
        std::thread::sleep(Duration::from_millis(40));
    }
    let mut pixels = vec![PremultipliedRgbaColor::default(); (largura * altura) as usize];
    janela.request_redraw();
    janela.draw_if_needed(|r| {
        r.render(&mut pixels, largura as usize);
    });
    let bytes: Vec<u8> = pixels
        .iter()
        .flat_map(|p| [p.red, p.green, p.blue, 255])
        .collect();
    let saida = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/captura-normal.png");
    image::RgbaImage::from_raw(largura, altura, bytes)
        .expect("buffer")
        .save(&saida)
        .expect("salvar png");
    println!("captura em {}", saida.display());
}

/// Renderiza o modo imersivo num PNG, sem abrir janela, para inspeção
/// visual. Manual:
///
/// `FURINAR_CAPTURA_FAIXA=/musica.mp3 cargo test captura_modo_imersivo -- --ignored`
///
/// A imagem sai em `target/captura-imersivo.png`.
#[test]
#[ignore = "gera PNG para inspeção visual"]
fn captura_modo_imersivo() {
    use slint::platform::software_renderer::{
        MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
    };
    use slint::platform::{Platform, WindowAdapter};

    struct Plataforma(Rc<MinimalSoftwareWindow>);
    impl Platform for Plataforma {
        fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
            Ok(self.0.clone())
        }
    }

    let (largura, altura) = (1600u32, 900u32);
    let janela = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Plataforma(janela.clone())))
        .expect("plataforma de teste");
    janela.set_size(slint::PhysicalSize::new(largura, altura));

    let ui = MainWindow::new().expect("MainWindow");
    ui.show().expect("show");
    aplicar_idioma(
        &std::env::var("FURINAR_CAPTURA_IDIOMA").unwrap_or_else(|_| "pt-br".into()),
        &ui,
    );

    let caminho = std::env::var_os("FURINAR_CAPTURA_FAIXA")
        .map(PathBuf::from)
        .unwrap_or_else(|| fixture("tone.mp3"));
    let mut imv = imersivo::EstadoImersivo::novo(&ui);
    // Espectro de mentira: um acorde, para as barras aparecerem
    let captura = visualizador::Captura::default();
    captura.ligar(true);
    let acorde = rodio::source::SineWave::new(220.0)
        .mix(rodio::source::SineWave::new(1760.0).amplify(0.3))
        .take_duration(Duration::from_millis(100));
    let _: Vec<f32> = visualizador::Espiao::new(acorde, captura.clone()).collect();
    ui.set_modo_imersivo(true);
    imv.trocar_faixa(
        &ui,
        Some(imersivo::Faixa {
            titulo: track::ler_tags(&caminho).titulo,
            artista: track::ler_tags(&caminho).artista,
            pasta_raiz: caminho.parent().map(PathBuf::from).unwrap_or_default(),
            capa_web: None,
            album: None,
            ano: None,
            fundos_globais: std::env::var_os("FURINAR_CAPTURA_FUNDO")
                .map(PathBuf::from)
                .into_iter()
                .collect(),
            caminho: caminho.clone(),
        }),
    );

    let letra = fs::read_to_string(caminho_lrc(&caminho))
        .map(|c| parse_lrc(&c))
        .unwrap_or_default();
    let linhas: Vec<SharedString> = letra.iter().map(|(_, t)| t.as_str().into()).collect();
    ui.set_letra_indice(if linhas.is_empty() {
        -1
    } else {
        4.min(linhas.len() as i32 - 1)
    });
    ui.set_letra_completa(ModelRc::new(VecModel::from(linhas)));
    ui.set_progresso(0.35);
    ui.set_texto_tempo("01:14 / 03:31".into());
    ui.set_imv_relogio("21:47".into());
    ui.set_imv_letra_progresso(0.45);
    ui.set_sono_texto("29:41".into());
    ui.set_favorita_atual(true);
    ui.set_imv_proxima("Próxima faixa — Artista".into());
    // FURINAR_CAPTURA_CONFIG=1: configurações abertas
    if std::env::var_os("FURINAR_CAPTURA_CONFIG").is_some() {
        ui.set_crossfade_texto("Crossfade: 6 s".into());
        ui.set_fundo_global_nome("pixel-city.gif".into());
        ui.set_imv_config_aberta(true);
    }

    // FURINAR_CAPTURA_PAINEL=1: painel aberto com as faixas da pasta
    if std::env::var_os("FURINAR_CAPTURA_PAINEL").is_some() {
        let raiz = caminho.parent().map(PathBuf::from).unwrap_or_default();
        let mut tracks = Vec::new();
        coletar_arquivos_audio(&raiz, &raiz, false, &mut tracks);
        tracks.sort_by(|a, b| a.path.cmp(&b.path));
        let nome = caminho.strip_prefix(&raiz).ok().and_then(|n| n.to_str());
        let atual = tracks.iter().position(|t| Some(t.path.as_str()) == nome);
        let mut pasta = PastaMusical::pasta(raiz);
        pasta.tracks = tracks;
        pasta.carregada = true;
        let mut bib = Biblioteca::default();
        let sem_correcoes = metadados::Banco::default();
        if let Some(k) = atual.and_then(|i| pasta.chave_faixa(i + 1)) {
            bib.alternar_favorita(&k);
        }
        ui.set_abas(ModelRc::new(VecModel::from(vec![
            SharedString::from("Favoritas"),
            "Mais tocadas".into(),
            SharedString::from(pasta.nome.as_str()),
            "Outra pasta".into(),
        ])));
        ui.set_aba_ativa(2);
        ui.set_aba_tocando(2);
        ui.set_indice_selecionado(atual.map_or(-1, |i| i as i32));
        // Um quadro antes: a tela imersiva tem que existir para o
        // `changed painel_aberto` dela disparar e centralizar a lista
        janela.request_redraw();
        janela.draw_if_needed(|r| {
            let mut lixo = vec![PremultipliedRgbaColor::default(); (largura * altura) as usize];
            r.render(&mut lixo, largura as usize);
        });
        ui.set_imv_painel_aberto(true);
        let mut p = painel::Painel::novo(&ui);
        let inicio = atual.unwrap_or(0).saturating_sub(6);
        p.rolou(inicio, 17);
        p.publicar(&ui, pasta.tracks.len(), |pos| {
            item_painel(&pasta, pos, atual, &bib, &sem_correcoes)
        });
        let fila = (1..=5)
            .filter_map(|k| item_painel(&pasta, atual.unwrap_or(0) + k, None, &bib, &sem_correcoes))
            .collect();
        p.publicar_fila(fila);
        // FURINAR_CAPTURA_ALBUNS=1: a guia da grade
        if std::env::var_os("FURINAR_CAPTURA_ALBUNS").is_some() {
            let todas: Vec<usize> = (0..pasta.tracks.len()).collect();
            p.mostrar_albuns(true);
            p.albuns_rolou(0, 12);
            p.publicar_albuns(
                track::agrupar_albuns(&pasta.tracks, &todas),
                &translations::pt_br(),
                |i| Some((pasta.caminho_faixa(i)?, None)),
            );
            ui.set_imv_guia(painel::GUIA_ALBUNS);
        }
        imv.painel = Some(p);
    }

    // Espera a thread da capa e deixa as animações chegarem ao fim.
    // FURINAR_CAPTURA_OCIOSO=1: espera os controles sumirem (3 s)
    if std::env::var_os("FURINAR_CAPTURA_OCIOSO").is_some() {
        ui.set_imv_ocioso(true);
    }
    // FURINAR_CAPTURA_SO_VIDEO=1 (com faixa de vídeo): modo só vídeo
    if std::env::var_os("FURINAR_CAPTURA_SO_VIDEO").is_some() {
        ui.set_imv_so_video(true);
    }
    // Faixa de vídeo: a "música" fica parada aos 5 s, o quadro de lá aparece
    for _ in 0..40 {
        let posicao = Some(Duration::from_secs(5));
        imv.tick(&ui, "pt-br", &captura, true, true, posicao);
        slint::platform::update_timers_and_animations();
        std::thread::sleep(Duration::from_millis(50));
    }

    let mut pixels = vec![PremultipliedRgbaColor::default(); (largura * altura) as usize];
    janela.request_redraw();
    janela.draw_if_needed(|r| {
        r.render(&mut pixels, largura as usize);
    });
    let bytes: Vec<u8> = pixels
        .iter()
        .flat_map(|p| [p.red, p.green, p.blue, 255])
        .collect();
    let saida = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/captura-imersivo.png");
    image::RgbaImage::from_raw(largura, altura, bytes)
        .expect("buffer")
        .save(&saida)
        .expect("salvar png");
    println!("captura em {}", saida.display());
}
