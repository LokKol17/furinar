//! Estado do app: as abas (pastas e listas especiais) e o `EstadoAudio`
//! compartilhado por todos os callbacks, e a gravação da configuração.

use crate::*;

/// Chaves das listas especiais na config (caminho de pasta nunca começa
/// com `:`).
pub(crate) const CHAVE_FAVORITAS: &str = ":favoritas";

pub(crate) const CHAVE_MAIS_TOCADAS: &str = ":mais_tocadas";

/// As listas especiais ocupam as primeiras posições de `pastas`, nesta ordem.
pub(crate) const POS_FAVORITAS: usize = 0;

pub(crate) const POS_MAIS_TOCADAS: usize = 1;

pub(crate) const ESPECIAIS: usize = 2;

#[derive(Clone, PartialEq)]
pub(crate) enum Tipo {
    Pasta,
    Favoritas,
    MaisTocadas,
    /// Aba temporária com um álbum, aberta pela grade do modo imersivo. Não
    /// é salva; `origem` é a lista de onde ele saiu.
    Album {
        origem: String,
    },
}

/// Uma aba: pasta de música aberta pelo usuário ou lista especial. Nas
/// especiais `caminho` é vazio e cada faixa guarda o caminho absoluto.
pub(crate) struct PastaMusical {
    pub(crate) caminho: PathBuf,
    /// Nome de exibição na aba (último componente do caminho).
    pub(crate) nome: String,
    pub(crate) tracks: Vec<TrackInfo>,
    /// Se as faixas já foram escaneadas. O scan é preguiçoso: pastas abertas
    /// mas nunca visitadas ficam sem `tracks` até serem necessárias. Nas
    /// especiais, `false` pede para remontar a lista.
    pub(crate) carregada: bool,
    pub(crate) tipo: Tipo,
    pub(crate) ordem: Ordem,
}

impl PastaMusical {
    pub(crate) fn pasta(caminho: PathBuf) -> Self {
        Self {
            nome: nome_da_pasta(&caminho),
            caminho,
            tracks: Vec::new(),
            carregada: false,
            tipo: Tipo::Pasta,
            ordem: Ordem::Padrao,
        }
    }

    pub(crate) fn especial(tipo: Tipo, ordem: Ordem) -> Self {
        Self {
            caminho: PathBuf::new(),
            nome: String::new(),
            tracks: Vec::new(),
            carregada: false,
            tipo,
            ordem,
        }
    }

    /// Identidade da lista na config.
    pub(crate) fn chave(&self) -> String {
        match &self.tipo {
            Tipo::Pasta => self.caminho.to_string_lossy().to_string(),
            Tipo::Favoritas => CHAVE_FAVORITAS.to_string(),
            Tipo::MaisTocadas => CHAVE_MAIS_TOCADAS.to_string(),
            Tipo::Album { origem } => origem.clone(),
        }
    }

    pub(crate) fn caminho_faixa(&self, indice: usize) -> Option<PathBuf> {
        Some(self.caminho.join(&self.tracks.get(indice)?.path))
    }

    pub(crate) fn chave_faixa(&self, indice: usize) -> Option<String> {
        Some(chave_faixa(&self.caminho, &self.tracks.get(indice)?.path))
    }

    /// Pasta onde procurar fundo do modo imersivo: a da aba, ou a da própria
    /// faixa nas listas especiais.
    pub(crate) fn raiz_da_faixa(&self, indice: usize) -> PathBuf {
        if self.tipo == Tipo::Pasta {
            return self.caminho.clone();
        }
        self.caminho_faixa(indice)
            .and_then(|c| c.parent().map(PathBuf::from))
            .unwrap_or_default()
    }
}

/// Contagem do lote em andamento (ou do último).
#[derive(Default)]
pub(crate) struct Lote {
    pub(crate) ativo: bool,
    pub(crate) feitas: usize,
    pub(crate) total: usize,
    pub(crate) automaticas: usize,
    pub(crate) sem_resultado: usize,
    pub(crate) letras: usize,
}

/// Faixa da próxima música já carregada no player (gapless): começa sozinha
/// quando a atual acabar.
pub(crate) struct Engatilhada {
    pub(crate) pasta: usize,
    pub(crate) indice: usize,
    pub(crate) duracao: u64,
    pub(crate) controle: transicao::Controle,
}

pub(crate) struct EstadoAudio {
    /// Saída de áudio do sistema. Aberta uma vez e reaproveitada entre seeks
    /// e trocas de faixa: reabrir o dispositivo a cada vez recria o stream,
    /// a thread e os buffers do backend. Só é fechada no stop.
    pub(crate) saida: Option<MixerDeviceSink>,
    /// Amostras para o visualizador; só copia com o modo imersivo ligado.
    pub(crate) captura: visualizador::Captura,
    pub(crate) audio_player: Option<Player>,
    /// Faixa (caminho completo) cuja duração está em `duracao_total_secs`.
    /// Evita varrer o MP3 inteiro de novo a cada seek.
    pub(crate) duracao_de: Option<PathBuf>,
    pub(crate) tempo_decorrido: f64,
    /// Pastas abertas (abas), na ordem de exibição.
    pub(crate) pastas: Vec<PastaMusical>,
    /// Índice da pasta sendo exibida na lista. Navegar não é reproduzir.
    pub(crate) aba_visivel: usize,
    /// Índice da pasta de onde vem a faixa atual. Pode diferir de `aba_visivel`.
    pub(crate) pasta_reproducao: Option<usize>,
    pub(crate) arquivo_atual: Option<String>,
    pub(crate) indice_atual: Option<usize>,
    pub(crate) volume_atual: f32,
    pub(crate) duracao_total_secs: u64,
    pub(crate) ultimo_segundo: u64,
    pub(crate) modo_loop: u8,
    /// 0 = desligado, 1 = shuffle, 2 = shuffle inteligente.
    pub(crate) modo_shuffle: u8,
    /// Sacola do shuffle inteligente: índices embaralhados restantes na
    /// pasta de reprodução atual. Esvazia -> reembaralha. Resetada (fica
    /// vazia, força reembaralho) quando a pasta de reprodução muda, a
    /// lista de faixas muda, ou o modo sai/entra no inteligente.
    pub(crate) sacola_shuffle: Vec<usize>,
    /// Shuffle comum: próximas faixas já sorteadas, para a fila "a seguir"
    /// mostrar o que vai tocar de verdade. Zera junto com a sacola.
    pub(crate) sorteio: Vec<usize>,
    /// Temporizador de sono ligado.
    pub(crate) sono: Option<sono::Sono>,
    /// O que tocou antes (lista, chave da faixa), para o "anterior" do
    /// shuffle voltar ao que tocou de verdade e não à vizinha na lista.
    pub(crate) historico: Vec<(usize, String)>,
    /// Voltando pelo histórico: a troca não entra nele de novo.
    pub(crate) voltando: bool,
    /// Favoritas e contagem de reproduções.
    pub(crate) biblioteca: Biblioteca,
    /// A faixa atual já contou uma reprodução.
    pub(crate) contada: bool,
    /// Próxima faixa já na fila do player (gapless).
    pub(crate) engatilhada: Option<Engatilhada>,
    /// A transição para a próxima já foi preparada (ou tentada) nesta faixa.
    pub(crate) transicao_feita: bool,
    /// Faixa anterior sumindo no crossfade, até acabar.
    pub(crate) saindo: Option<Player>,
    /// Pede o fade-out da faixa atual.
    pub(crate) controle: transicao::Controle,
    /// Segundos de crossfade (0 = só gapless).
    pub(crate) crossfade: u8,
    pub(crate) replaygain: bool,
    pub(crate) fundo_reativo: bool,
    /// Correções de metadados vindas da web (banco local).
    pub(crate) metadados: metadados::Banco,
    /// Thread da busca na web; só existe com trabalho.
    pub(crate) buscador: Option<buscador::Buscador>,
    /// Buscar a letra na web quando a faixa tocando não tem.
    pub(crate) letras_web: bool,
    /// Chave de aplicativo do AcoustID ("" = sem impressão digital).
    pub(crate) chave_acoustid: String,
    /// Status no Discord ligado, e a conexão (só existe ligado).
    pub(crate) discord_ativo: bool,
    pub(crate) discord: Option<discord::Discord>,
    /// Scrobble: contas, escutas que ainda não foram, a faixa cujo "tocando
    /// agora" já foi avisado, e a conexão do Last.fm em andamento.
    pub(crate) contas: scrobble::Contas,
    pub(crate) fila_scrobble: scrobble::Fila,
    pub(crate) avisada: Option<String>,
    pub(crate) lastfm_token: Option<String>,
    pub(crate) lastfm_usuario: String,
    /// Tela de informações: faixa (chave) e candidatos da última busca.
    pub(crate) info_chave: Option<String>,
    pub(crate) info_candidatos: Vec<metadados::Candidato>,
    /// Lote da aba: o que ficou para revisão e a contagem.
    pub(crate) revisao: Vec<(metadados::Consulta, metadados::Candidato)>,
    pub(crate) lote: Lote,
    pub(crate) escanear_subpastas: bool,
    pub(crate) tema_claro: bool,
    pub(crate) idioma: String,
    /// Mapeia posição na lista visível -> índice em `pastas[aba_visivel].tracks`.
    pub(crate) indices_visiveis: Vec<usize>,
    /// Texto atual da busca (título/artista).
    pub(crate) filtro: String,
    /// Letra sincronizada da faixa atual (tempo em segundos, texto), ordenada.
    pub(crate) letra_atual: Vec<(f64, String)>,
    /// Tempo e posição de cada palavra, por linha (LRC estendido; vazio
    /// quando a letra só tem tempo por linha).
    pub(crate) letra_palavras: Vec<Vec<(f64, usize)>>,
    /// A letra atual não tem tempos (veio da tag sem sincronia).
    pub(crate) letra_livre: bool,
    /// Índice da linha da letra exibida (-1 = antes da primeira).
    pub(crate) letra_indice: i32,
    /// Índice absoluto da primeira linha do recorte que está na UI. Serve para
    /// traduzir o clique numa linha do painel de volta para o timestamp.
    pub(crate) letra_janela_inicio: usize,
    /// Modo imersivo (tela cheia). `None` = desligado, nada alocado.
    pub(crate) imersivo: Option<imersivo::EstadoImersivo>,
    /// Fundos do modo imersivo escolhidos nas configurações.
    pub(crate) fundos_globais: Vec<PathBuf>,
    #[cfg(feature = "mpris")]
    pub(crate) faixa_controles: Option<String>,
    #[cfg(feature = "mpris")]
    pub(crate) status_controles: Option<MediaPlayback>,
}

impl Default for EstadoAudio {
    fn default() -> Self {
        Self {
            saida: None,
            audio_player: None,
            captura: visualizador::Captura::default(),
            duracao_de: None,
            tempo_decorrido: 0.0,
            pastas: Vec::new(),
            aba_visivel: 0,
            pasta_reproducao: None,
            arquivo_atual: None,
            indice_atual: None,
            volume_atual: 0.8,
            duracao_total_secs: 0,
            ultimo_segundo: 0,
            modo_loop: 0,
            modo_shuffle: 0,
            sacola_shuffle: Vec::new(),
            sorteio: Vec::new(),
            sono: None,
            historico: Vec::new(),
            voltando: false,
            biblioteca: Biblioteca::default(),
            contada: false,
            engatilhada: None,
            transicao_feita: false,
            saindo: None,
            controle: transicao::Controle::default(),
            crossfade: 0,
            replaygain: true,
            fundo_reativo: false,
            metadados: metadados::Banco::default(),
            buscador: None,
            letras_web: false,
            chave_acoustid: String::new(),
            discord_ativo: false,
            discord: None,
            contas: scrobble::Contas::default(),
            fila_scrobble: Vec::new(),
            avisada: None,
            lastfm_token: None,
            lastfm_usuario: String::new(),
            info_chave: None,
            info_candidatos: Vec::new(),
            revisao: Vec::new(),
            lote: Lote::default(),
            escanear_subpastas: false,
            tema_claro: false,
            idioma: "en".to_string(),
            indices_visiveis: Vec::new(),
            filtro: String::new(),
            letra_atual: Vec::new(),
            letra_palavras: Vec::new(),
            letra_livre: false,
            letra_indice: -1,
            letra_janela_inicio: 0,
            imersivo: None,
            fundos_globais: Vec::new(),
            #[cfg(feature = "mpris")]
            faixa_controles: None,
            #[cfg(feature = "mpris")]
            status_controles: None,
        }
    }
}

/// Quantas próximas do shuffle vão para a config (a sacola inteira do
/// inteligente pode ter milhares).
pub(crate) const FILA_SALVA: usize = 50;

pub(crate) fn salvar_configuracao(estado: &EstadoAudio) {
    let lista = |i: usize| estado.pastas.get(i).map(PastaMusical::chave);
    let reproducao = estado.pasta_reproducao.and_then(|i| estado.pastas.get(i));
    let faixa_salva =
        reproducao.and_then(|p| Some(chave_faixa(&p.caminho, estado.arquivo_atual.as_deref()?)));
    let proximas: Vec<usize> = match estado.modo_shuffle {
        2 => estado
            .sacola_shuffle
            .iter()
            .rev()
            .take(FILA_SALVA)
            .copied()
            .collect(),
        1 => estado.sorteio.clone(),
        _ => Vec::new(),
    };
    let fila_shuffle = reproducao
        .map(|p| proximas.iter().filter_map(|&i| p.chave_faixa(i)).collect())
        .unwrap_or_default();
    let config = AppConfig {
        pasta: None,
        pastas: estado
            .pastas
            .iter()
            .filter(|p| p.tipo == Tipo::Pasta)
            .map(|p| p.caminho.to_string_lossy().to_string())
            .collect(),
        aba_visivel_salva: 0,
        pasta_reproducao_salva: None,
        indice_atual: None,
        aba_salva: lista(estado.aba_visivel),
        lista_salva: estado.pasta_reproducao.and_then(lista),
        faixa_salva,
        fila_shuffle,
        ordens: estado
            .pastas
            .iter()
            .filter(|p| p.ordem != Ordem::Padrao && !matches!(p.tipo, Tipo::Album { .. }))
            .map(|p| (p.chave(), p.ordem.indice() as u8))
            .collect(),
        crossfade: estado.crossfade,
        letras_web: estado.letras_web,
        chave_acoustid: estado.chave_acoustid.clone(),
        discord: estado.discord_ativo,
        listenbrainz_token: estado.contas.listenbrainz.clone(),
        lastfm_chave: estado.contas.lastfm_chave.clone(),
        lastfm_segredo: estado.contas.lastfm_segredo.clone(),
        lastfm_sessao: estado.contas.lastfm_sessao.clone(),
        lastfm_usuario: estado.lastfm_usuario.clone(),
        replaygain: estado.replaygain,
        fundo_reativo: estado.fundo_reativo,
        volume: estado.volume_atual,
        modo_loop: estado.modo_loop,
        shuffle: false,
        modo_shuffle: estado.modo_shuffle,
        escanear_subpastas: estado.escanear_subpastas,
        tema_claro: estado.tema_claro,
        idioma: estado.idioma.clone(),
        fundo_tela_cheia: None,
        fundos_tela_cheia: estado
            .fundos_globais
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect(),
        tempo_atual: if estado.tempo_decorrido > 0.0 {
            Some(estado.tempo_decorrido as u64)
        } else {
            None
        },
    };
    config.salvar();
}
