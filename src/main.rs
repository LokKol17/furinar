slint::include_modules!();

use std::cell::RefCell;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use rand::seq::SliceRandom;
use rodio::{Decoder, OutputStream, Sink, Source};
use serde::{Deserialize, Serialize};
use slint::{ModelRc, SharedString, Timer, TimerMode, VecModel};

// ---------------------------------------------------------------------
// Config (idêntico às versões anteriores)
// ---------------------------------------------------------------------

#[derive(Serialize, Deserialize, Default)]
struct AppConfig {
    pasta: Option<String>,
    volume: f32,
    modo_loop: u8,
    shuffle: bool,
    indice_atual: Option<usize>,
    tempo_atual: Option<u64>,
}

impl AppConfig {
    fn carregar() -> Self {
        if let Ok(conteudo) = fs::read_to_string("furinar_config.json") {
            serde_json::from_str(&conteudo).unwrap_or_default()
        } else {
            Self {
                pasta: None,
                volume: 0.8,
                modo_loop: 0,
                shuffle: false,
                indice_atual: None,
                tempo_atual: None,
            }
        }
    }

    fn salvar(&self) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write("furinar_config.json", json);
        }
    }
}

fn formatar_tempo(segundos: u64, com_horas: bool) -> String {
    let horas = segundos / 3600;
    let mins = (segundos % 3600) / 60;
    let segs = segundos % 60;
    if com_horas {
        format!("{:02}:{:02}:{:02}", horas, mins, segs)
    } else {
        format!("{:02}:{:02}", mins, segs)
    }
}

fn e_arquivo_audio(caminho: &std::path::Path) -> bool {
    if let Some(ext) = caminho.extension().and_then(|e| e.to_str()) {
        matches!(
            ext.to_lowercase().as_str(),
            "mp3" | "wav" | "flac" | "ogg" | "m4a"
        )
    } else {
        false
    }
}

fn texto_loop(modo: u8) -> &'static str {
    match modo {
        1 => "Loop: Faixa",
        2 => "Loop: Toda",
        _ => "Loop: Desl",
    }
}

fn texto_shuffle(ligado: bool) -> &'static str {
    if ligado {
        "Shuffle: Lig"
    } else {
        "Shuffle: Desl"
    }
}

// ---------------------------------------------------------------------
// Estado de áudio/playlist (a mesma lógica de sempre, sem nenhum código de UI)
// ---------------------------------------------------------------------

struct EstadoAudio {
    audio_player: Option<(OutputStream, Sink)>,
    tempo_decorrido: f64,
    pasta_atual: Option<PathBuf>,
    arquivo_atual: Option<String>,
    indice_atual: Option<usize>,
    volume_atual: f32,
    duracao_total_secs: u64,
    ultimo_segundo: u64,
    modo_loop: u8,
    modo_shuffle: bool,
    arquivos: Vec<String>,
}

impl Default for EstadoAudio {
    fn default() -> Self {
        Self {
            audio_player: None,
            tempo_decorrido: 0.0,
            pasta_atual: None,
            arquivo_atual: None,
            indice_atual: None,
            volume_atual: 0.8,
            duracao_total_secs: 0,
            ultimo_segundo: 0,
            modo_loop: 0,
            modo_shuffle: false,
            arquivos: Vec::new(),
        }
    }
}

fn salvar_configuracao(estado: &EstadoAudio) {
    let config = AppConfig {
        pasta: estado
            .pasta_atual
            .as_ref()
            .map(|p| p.to_string_lossy().to_string()),
        volume: estado.volume_atual,
        modo_loop: estado.modo_loop,
        shuffle: estado.modo_shuffle,
        indice_atual: estado.indice_atual,
        tempo_atual: if estado.tempo_decorrido > 0.0 {
            Some(estado.tempo_decorrido as u64)
        } else {
            None
        },
    };
    config.salvar();
}

fn carregar_pasta(estado: &mut EstadoAudio, ui: &MainWindow, caminho: PathBuf) {
    estado.arquivos.clear();
    estado.pasta_atual = Some(caminho.clone());

    if let Ok(entradas) = fs::read_dir(&caminho) {
        let mut arquivos = Vec::new();
        for entrada in entradas.flatten() {
            let path = entrada.path();
            if path.is_file() && e_arquivo_audio(&path) {
                if let Some(nome) = path.file_name().and_then(|n| n.to_str()) {
                    arquivos.push(nome.to_string());
                }
            }
        }
        arquivos.sort();
        estado.arquivos = arquivos;
    }

    let modelo: Vec<SharedString> = estado.arquivos.iter().map(|s| s.as_str().into()).collect();
    ui.set_musicas(ModelRc::new(VecModel::from(modelo)));
}

fn executar_seek(estado: &mut EstadoAudio, ui: &MainWindow, alvo_secs: u64) {
    let pasta = match estado.pasta_atual.clone() {
        Some(p) => p,
        None => return,
    };
    let nome_arquivo = match estado.arquivo_atual.clone() {
        Some(n) => n,
        None => return,
    };
    let caminho_completo = pasta.join(&nome_arquivo);

    estado.audio_player = None;

    let arquivo = match File::open(&caminho_completo) {
        Ok(a) => a,
        Err(_) => return,
    };
    let (stream, stream_handle) = match OutputStream::try_default() {
        Ok(s) => s,
        Err(_) => return,
    };
    let sink = match Sink::try_new(&stream_handle) {
        Ok(s) => s,
        Err(_) => return,
    };
    sink.set_volume(estado.volume_atual);

    let leitor = BufReader::new(arquivo);
    let decodificador = match Decoder::new(leitor) {
        Ok(d) => d,
        Err(_) => return,
    };

    let total_secs = if caminho_completo
        .extension()
        .map_or(false, |e| e.eq_ignore_ascii_case("mp3"))
    {
        mp3_duration::from_path(&caminho_completo)
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
    estado.duracao_total_secs = total_secs;

    sink.append(decodificador);
    if alvo_secs > 0 {
        let _ = sink.try_seek(Duration::from_secs(alvo_secs));
    }

    estado.tempo_decorrido = alvo_secs as f64;
    estado.ultimo_segundo = alvo_secs;
    estado.audio_player = Some((stream, sink));

    ui.set_progresso(if total_secs > 0 {
        alvo_secs as f32 / total_secs as f32
    } else {
        0.0
    });
    ui.set_texto_play_pause("Pause".into());
    let com_horas = total_secs >= 3600;
    ui.set_texto_tempo(
        format!(
            "{} / {}",
            formatar_tempo(alvo_secs, com_horas),
            formatar_tempo(total_secs, com_horas)
        )
        .into(),
    );
}

fn tocar_indice(estado: &mut EstadoAudio, ui: &MainWindow, index: usize) {
    if let Some(nome) = estado.arquivos.get(index).cloned() {
        estado.arquivo_atual = Some(nome);
        estado.indice_atual = Some(index);
        ui.set_indice_selecionado(index as i32);
        executar_seek(estado, ui, 0);
    }
}

fn tocar_anterior(estado: &mut EstadoAudio, ui: &MainWindow) {
    let total = estado.arquivos.len();
    if total == 0 {
        return;
    }
    let atual = estado.indice_atual.unwrap_or(0);
    let anterior = if atual == 0 { total - 1 } else { atual - 1 };
    tocar_indice(estado, ui, anterior);
}

fn stop_music(estado: &mut EstadoAudio, ui: &MainWindow) {
    estado.audio_player = None;
    estado.tempo_decorrido = 0.0;
    estado.ultimo_segundo = 0;
    ui.set_progresso(0.0);
    ui.set_texto_play_pause("Play".into());
    ui.set_texto_tempo("00:00 / 00:00".into());
}

fn tocar_proxima_manual(estado: &mut EstadoAudio, ui: &MainWindow) {
    let total = estado.arquivos.len();
    if total == 0 {
        return;
    }
    let atual = estado.indice_atual.unwrap_or(0);
    let proximo = if estado.modo_shuffle {
        let mut rng = rand::thread_rng();
        (0..total)
            .filter(|&x| x != atual)
            .collect::<Vec<_>>()
            .choose(&mut rng)
            .copied()
            .unwrap_or(0)
    } else {
        let prox = atual + 1;
        if prox >= total {
            if estado.modo_loop == 2 {
                0
            } else {
                stop_music(estado, ui);
                return;
            }
        } else {
            prox
        }
    };
    tocar_indice(estado, ui, proximo);
}

fn proxima_musica_auto(estado: &mut EstadoAudio, ui: &MainWindow) {
    if estado.modo_loop == 1 {
        executar_seek(estado, ui, 0);
    } else {
        tocar_proxima_manual(estado, ui);
    }
}

fn restaurar_posicao(estado: &mut EstadoAudio, ui: &MainWindow) {
    let config = AppConfig::carregar();

    // Só restaura se houver índice e tempo salvos
    let (indice, tempo) = match (config.indice_atual, config.tempo_atual) {
        (Some(i), Some(t)) => (i, t),
        _ => return,
    };

    let pasta_str = match config.pasta {
        Some(p) => p,
        None => return,
    };

    let caminho = PathBuf::from(&pasta_str);
    if !caminho.exists() {
        return;
    }

    // Recarrega a playlist
    carregar_pasta(estado, ui, caminho);

    // Verifica se o arquivo ainda existe na lista
    if indice >= estado.arquivos.len() {
        return;
    }

    // Toca a faixa salva na posição recuperada
    tocar_indice(estado, ui, indice);
    executar_seek(estado, ui, tempo);
}

fn atualizar_progresso(estado: &mut EstadoAudio, ui: &MainWindow) {
    let total_secs = estado.duracao_total_secs;

    let terminou = match estado.audio_player {
        Some((_, ref sink)) => sink.empty() && total_secs > 0,
        None => false,
    };
    if terminou {
        proxima_musica_auto(estado, ui);
        return;
    }

    let pausado = match estado.audio_player {
        Some((_, ref sink)) => sink.is_paused(),
        None => return,
    };

    if !pausado {
        estado.tempo_decorrido += 0.25;
        let atual_secs = estado.tempo_decorrido as u64;
        if atual_secs != estado.ultimo_segundo {
            estado.ultimo_segundo = atual_secs;
            ui.set_progresso(if total_secs > 0 {
                atual_secs as f32 / total_secs as f32
            } else {
                0.0
            });
            let com_horas = total_secs >= 3600;
            ui.set_texto_tempo(
                format!(
                    "{} / {}",
                    formatar_tempo(atual_secs, com_horas),
                    formatar_tempo(total_secs, com_horas)
                )
                .into(),
            );
        }
    }
}

// ---------------------------------------------------------------------
// main: cria a UI, conecta os callbacks, inicia o timer, roda o app
// ---------------------------------------------------------------------

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = MainWindow::new()?;
    let estado = Rc::new(RefCell::new(EstadoAudio::default()));

    {
        let mut e = estado.borrow_mut();
        let config = AppConfig::carregar();
        e.volume_atual = config.volume;
        e.modo_loop = config.modo_loop;
        e.modo_shuffle = config.shuffle;

        ui.set_volume(config.volume);
        ui.set_texto_loop(texto_loop(config.modo_loop).into());
        ui.set_texto_shuffle(texto_shuffle(config.shuffle).into());

        if let Some(pasta_str) = config.pasta {
            let caminho = PathBuf::from(pasta_str);
            if caminho.exists() {
                carregar_pasta(&mut e, &ui, caminho);
            }
        }
    }

    // Tenta restaurar a posição anterior (música + tempo)
    {
        let mut e = estado.borrow_mut();
        restaurar_posicao(&mut e, &ui);
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_abrir_pasta(move || {
            // O diálogo é modal/bloqueante - chamamos ele ANTES de pegar
            // o borrow_mut do estado, pra não correr risco de um borrow
            // duplo se algo repintar a janela nesse meio-tempo.
            if let Some(caminho) = rfd::FileDialog::new().pick_folder() {
                if let Some(ui) = ui_fraca.upgrade() {
                    let mut e = estado.borrow_mut();
                    carregar_pasta(&mut e, &ui, caminho.clone());
                    salvar_configuracao(&e);
                }
            }
        });
    }

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
                let e = estado.borrow();
                if let Some((_, ref sink)) = e.audio_player {
                    if sink.is_paused() {
                        sink.play();
                        ui.set_texto_play_pause("Pause".into());
                    } else {
                        sink.pause();
                        ui.set_texto_play_pause("Play".into());
                    }
                }
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
                ui.set_texto_loop(texto_loop(e.modo_loop).into());
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
                e.modo_shuffle = !e.modo_shuffle;
                ui.set_texto_shuffle(texto_shuffle(e.modo_shuffle).into());
                salvar_configuracao(&e);
            }
        });
    }

    {
        let estado = estado.clone();
        ui.on_volume_mudou(move |v| {
            let mut e = estado.borrow_mut();
            e.volume_atual = v;
            if let Some((_, ref sink)) = e.audio_player {
                sink.set_volume(v);
            }
            salvar_configuracao(&e);
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_progresso_mudou(move |v| {
            if let Some(ui) = ui_fraca.upgrade() {
                let mut e = estado.borrow_mut();
                let alvo = (v as f64 * e.duracao_total_secs as f64) as u64;
                executar_seek(&mut e, &ui, alvo);
            }
        });
    }

    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        ui.on_musica_selecionada(move |indice| {
            if let Some(ui) = ui_fraca.upgrade() {
                tocar_indice(&mut estado.borrow_mut(), &ui, indice as usize);
            }
        });
    }

    // Timer de progresso - substitui o SetTimer/WM_TIMER manual das versões anteriores.
    let timer = Timer::default();
    {
        let estado = estado.clone();
        let ui_fraca = ui.as_weak();
        timer.start(TimerMode::Repeated, Duration::from_millis(250), move || {
            if let Some(ui) = ui_fraca.upgrade() {
                atualizar_progresso(&mut estado.borrow_mut(), &ui);
            }
        });
    }

    // Salva o estado final ao fechar
    {
        let estado = estado.clone();
        ui.on_fecha_janela(move || {
            salvar_configuracao(&estado.borrow());
        });
    }

    ui.run()?;

    // Salva novamente ao encerrar (caso o close request não tenha disparado)
    salvar_configuracao(&estado.borrow());

    Ok(())
}
