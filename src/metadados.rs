//! Metadados vindos da web: o que se pergunta (`Consulta`), o que volta
//! (`Candidato`), a nota de confiança e as correções aceitas.
//!
//! As correções ficam num banco local (`furinar_metadados.json`, ao lado da
//! configuração) e são aplicadas por cima das tags no scan: os arquivos só
//! mudam quando o usuário pede "gravar nas tags" (`gravar_tags`).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::track::{TrackInfo, normalizar_busca};

/// O que se sabe de uma faixa para procurá-la.
#[derive(Clone, Debug, PartialEq)]
pub struct Consulta {
    /// Identidade da faixa (`biblioteca::chave_faixa`).
    pub chave: String,
    pub caminho: PathBuf,
    pub titulo: String,
    pub artista: Option<String>,
    pub album: Option<String>,
    /// Segundos (0 = desconhecida).
    pub duracao: u32,
}

impl Consulta {
    /// Título e artista para a busca: o que as tags têm, ou o que dá para
    /// tirar do nome do arquivo ("01 - Artista - Título [Vídeo Oficial].mp3").
    pub fn termos(&self) -> (String, Option<String>) {
        let titulo = limpar_titulo(&self.titulo);
        if let Some(a) = &self.artista {
            return (titulo, Some(a.clone()));
        }
        // "Artista 'Título' feat. X" / "Artista「Título」": o título é o que
        // está entre aspas
        for (abre, fecha) in [('\'', '\''), ('「', '」'), ('『', '』'), ('"', '"')] {
            if let Some(i) = titulo.find(abre)
                && let Some(f) = titulo[i + abre.len_utf8()..].find(fecha)
            {
                let dentro = &titulo[i + abre.len_utf8()..i + abre.len_utf8() + f];
                let antes = titulo[..i].trim();
                if !dentro.trim().is_empty() && !antes.is_empty() {
                    return (dentro.trim().to_string(), Some(antes.to_string()));
                }
            }
        }
        match titulo.split_once(" - ") {
            Some((a, t)) if !a.trim().is_empty() && !t.trim().is_empty() => {
                (sem_feat(t.trim()), Some(a.trim().to_string()))
            }
            _ => (titulo, None),
        }
    }
}

/// "Música feat. Fulano" → "Música" (a participação atrapalha a busca).
fn sem_feat(titulo: &str) -> String {
    let minusculo = titulo.to_lowercase();
    [" feat.", " feat ", " ft.", " ft "]
        .iter()
        .filter_map(|p| minusculo.find(p))
        .min()
        .map_or(titulo, |i| &titulo[..i])
        .trim()
        .to_string()
}

/// Tira o que atrapalha a busca: número de faixa no começo e trechos entre
/// colchetes/parênteses com cara de enfeite ("Official Video", "【MV】").
pub fn limpar_titulo(titulo: &str) -> String {
    let mut s = titulo.trim().to_string();
    // "01 Vento", "1. Vento", "01 - Vento"; mas não "99 Luftballons" nem "1979"
    let digitos = s.chars().take_while(|c| c.is_ascii_digit()).count();
    if (1..=3).contains(&digitos) {
        let resto = &s[digitos..];
        let separado = resto.starts_with(['.', '-', '_'])
            || resto.trim_start().starts_with('-')
            || (s.starts_with('0') && resto.starts_with(' '));
        let sem_numero = resto.trim_start_matches(['.', ' ', '-', '_']);
        if separado && !sem_numero.is_empty() {
            s = sem_numero.to_string();
        }
    }
    for (abre, fecha) in [('[', ']'), ('【', '】'), ('(', ')')] {
        while let (Some(i), Some(f)) = (s.find(abre), s.rfind(fecha)) {
            if f <= i {
                break;
            }
            let dentro = normalizar_busca(&s[i + abre.len_utf8()..f]);
            // Parênteses podem ser parte do nome ("(Not) A Devil"): só sai o
            // que é enfeite conhecido
            let enfeite = abre != '('
                || [
                    "official", "video", "audio", "lyric", "mv", "hd", "remaster", "oficial",
                ]
                .iter()
                .any(|p| dentro.contains(p));
            if !enfeite {
                break;
            }
            s.replace_range(i..f + fecha.len_utf8(), " ");
        }
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// De onde veio o candidato.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fonte {
    Texto,
    ImpressaoDigital,
}

/// Uma resposta possível da web para a faixa.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidato {
    /// MBID da gravação.
    pub gravacao: String,
    pub titulo: String,
    pub artista: String,
    pub album: Option<String>,
    /// MBID do lançamento (para a capa).
    pub lancamento: Option<String>,
    pub ano: Option<u16>,
    /// Disco × 1000 + faixa (como `TrackInfo::numero`; 0 = sem número).
    pub numero: u32,
    /// Segundos (0 = desconhecida).
    pub duracao: u32,
    /// 0 a 100.
    pub confianca: u8,
    pub fonte: Fonte,
}

/// A partir disso o lote aceita sozinho (o resto vai para revisão).
pub const CONFIANCA_AUTOMATICA: u8 = 90;

/// Semelhança de 0 a 1 entre dois textos (sem acento, sem caixa): igual = 1,
/// um contém o outro = 0,85, senão a fração de palavras em comum.
pub fn semelhanca(a: &str, b: &str) -> f32 {
    let (a, b) = (normalizar_busca(a), normalizar_busca(b));
    let (a, b) = (a.trim(), b.trim());
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    if a == b {
        return 1.0;
    }
    if a.contains(b) || b.contains(a) {
        return 0.85;
    }
    let pa: Vec<&str> = a.split_whitespace().collect();
    let pb: Vec<&str> = b.split_whitespace().collect();
    let comuns = pa.iter().filter(|p| pb.contains(p)).count();
    comuns as f32 / pa.len().max(pb.len()) as f32
}

/// Quão perto as durações estão (0 a 1). Sem uma delas, neutro.
pub fn proximidade_duracao(a: u32, b: u32) -> f32 {
    if a == 0 || b == 0 {
        return 0.6;
    }
    match a.abs_diff(b) {
        0..=2 => 1.0,
        3..=5 => 0.8,
        6..=10 => 0.5,
        _ => 0.1,
    }
}

/// Nota de um candidato da busca por texto.
pub fn confianca_texto(consulta: &Consulta, titulo: &str, artista: &str, duracao: u32) -> u8 {
    let (t, a) = consulta.termos();
    let nota_titulo = semelhanca(&t, titulo);
    let nota_artista = a.map_or(0.5, |a| semelhanca(&a, artista));
    let nota_duracao = proximidade_duracao(consulta.duracao, duracao);
    let nota = 0.45 * nota_titulo + 0.30 * nota_artista + 0.25 * nota_duracao;
    // Versão instrumental/karaokê só quando é o que se procura
    let variante = |s: &str| {
        let s = normalizar_busca(s);
        ["instrumental", "karaoke", "off vocal", "inst."]
            .iter()
            .any(|v| s.contains(v))
    };
    let penalidade = if variante(titulo) && !variante(&t) {
        0.12
    } else {
        0.0
    };
    ((nota - penalidade) * 100.0).round().clamp(0.0, 100.0) as u8
}

/// Uma correção aceita para uma faixa.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Correcao {
    pub titulo: String,
    pub artista: String,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub ano: Option<u16>,
    #[serde(default)]
    pub numero: u32,
    #[serde(default)]
    pub gravacao: Option<String>,
    /// Lançamento cuja capa foi baixada (`capa_web`).
    #[serde(default)]
    pub lancamento: Option<String>,
}

impl From<&Candidato> for Correcao {
    fn from(c: &Candidato) -> Self {
        Self {
            titulo: c.titulo.clone(),
            artista: c.artista.clone(),
            album: c.album.clone(),
            ano: c.ano,
            numero: c.numero,
            gravacao: Some(c.gravacao.clone()),
            lancamento: c.lancamento.clone(),
        }
    }
}

impl Correcao {
    /// Põe a correção por cima do que veio das tags.
    pub fn aplicar(&self, t: &mut TrackInfo) {
        t.titulo = self.titulo.clone();
        t.artista = Some(self.artista.clone()).filter(|a| !a.is_empty());
        if self.album.is_some() {
            t.album = self.album.clone();
        }
        if self.numero > 0 {
            t.numero = self.numero;
        }
    }
}

/// Correções aceitas e faixas cuja letra a web não tem (para não perguntar
/// de novo a cada vez que tocam).
#[derive(Default, Serialize, Deserialize)]
pub struct Banco {
    #[serde(default)]
    correcoes: HashMap<String, Correcao>,
    /// Chave → quando a busca de letra falhou (segundos desde 1970).
    #[serde(default)]
    sem_letra: HashMap<String, u64>,
    #[serde(skip)]
    caminho: Option<PathBuf>,
}

/// Depois disso a letra que faltava é procurada de novo (alguém pode ter
/// enviado ao LRCLIB).
const NOVA_BUSCA_LETRA: u64 = 30 * 24 * 3600;

fn agora() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Banco {
    pub fn carregar() -> Self {
        let caminho = crate::config::config_path().with_file_name("furinar_metadados.json");
        let mut b: Self = fs::read_to_string(&caminho)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default();
        b.caminho = Some(caminho);
        b
    }

    pub fn salvar(&self) {
        let Some(caminho) = &self.caminho else {
            return;
        };
        if let Ok(json) = serde_json::to_string(self) {
            let temporario = caminho.with_extension("json.tmp");
            if fs::write(&temporario, json).is_ok() {
                let _ = fs::rename(&temporario, caminho);
            }
        }
    }

    pub fn correcao(&self, chave: &str) -> Option<&Correcao> {
        self.correcoes.get(chave)
    }

    pub fn corrigir(&mut self, chave: &str, correcao: Correcao) {
        self.correcoes.insert(chave.to_string(), correcao);
    }

    /// Volta ao que as tags dizem.
    pub fn restaurar(&mut self, chave: &str) -> bool {
        self.correcoes.remove(chave).is_some()
    }

    /// Aplica as correções às faixas de uma lista. `chave(t)` dá a chave da
    /// faixa.
    pub fn aplicar(&self, tracks: &mut [TrackInfo], chave: impl Fn(&TrackInfo) -> String) {
        if self.correcoes.is_empty() {
            return;
        }
        for t in tracks {
            if let Some(c) = self.correcoes.get(&chave(t)) {
                c.aplicar(t);
            }
        }
    }

    /// Vale perguntar à web pela letra desta faixa?
    pub fn buscar_letra(&self, chave: &str) -> bool {
        self.sem_letra
            .get(chave)
            .is_none_or(|&quando| agora().saturating_sub(quando) > NOVA_BUSCA_LETRA)
    }

    pub fn marcar_sem_letra(&mut self, chave: &str) {
        self.sem_letra.insert(chave.to_string(), agora());
    }

    pub fn esquecer_sem_letra(&mut self, chave: &str) {
        self.sem_letra.remove(chave);
    }
}

/// Pasta do cache da web (`~/.cache/furinar/<sub>` ou
/// `%LOCALAPPDATA%\furinar\<sub>`).
pub fn pasta_cache(sub: &str) -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(not(target_os = "windows"))]
    let base = std::env::var_os("XDG_CACHE_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")));
    base.map(|b| b.join("furinar").join(sub))
}

/// Capa baixada de um lançamento.
pub fn capa_web(lancamento: &str) -> Option<PathBuf> {
    // MBID é só hex e hífen: seguro como nome de arquivo
    if lancamento.is_empty()
        || !lancamento
            .chars()
            .all(|c| c.is_ascii_hexdigit() || c == '-')
    {
        return None;
    }
    pasta_cache("capas_web").map(|p| p.join(format!("{lancamento}.jpg")))
}

/// Escreve a correção (e a capa, se houver e o arquivo não tiver uma) nas
/// tags do arquivo. Mexe no arquivo de música: só sob pedido explícito.
pub fn gravar_tags(caminho: &Path, c: &Correcao) -> Result<(), String> {
    use lofty::config::WriteOptions;
    use lofty::picture::{MimeType, Picture, PictureType};
    use lofty::prelude::*;
    use lofty::tag::Tag;

    let mut arquivo = lofty::read_from_path(caminho).map_err(|e| e.to_string())?;
    if arquivo.primary_tag().is_none() {
        let tipo = arquivo.primary_tag_type();
        arquivo.insert_tag(Tag::new(tipo));
    }
    let Some(tag) = arquivo.primary_tag_mut() else {
        return Err("sem tag".into());
    };
    tag.set_title(c.titulo.clone());
    if !c.artista.is_empty() {
        tag.set_artist(c.artista.clone());
    }
    if let Some(album) = &c.album {
        tag.set_album(album.clone());
    }
    if c.numero > 0 {
        tag.set_track(c.numero % 1000);
        if c.numero >= 1000 {
            tag.set_disk(c.numero / 1000);
        }
    }
    if let Some(ano) = c.ano {
        tag.insert_text(ItemKey::Year, ano.to_string());
    }
    let sem_capa = tag.pictures().is_empty();
    if sem_capa
        && let Some(capa) = c.lancamento.as_deref().and_then(capa_web)
        && let Ok(bytes) = fs::read(capa)
    {
        tag.push_picture(
            Picture::unchecked(bytes)
                .pic_type(PictureType::CoverFront)
                .mime_type(MimeType::Jpeg)
                .build(),
        );
    }
    tag.save_to_path(caminho, WriteOptions::default())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn consulta(titulo: &str, artista: Option<&str>, duracao: u32) -> Consulta {
        Consulta {
            chave: "/m/a.mp3".into(),
            caminho: "/m/a.mp3".into(),
            titulo: titulo.into(),
            artista: artista.map(Into::into),
            album: None,
            duracao,
        }
    }

    #[test]
    fn limpa_numero_e_enfeites_do_titulo() {
        assert_eq!(limpar_titulo("01 - Vento"), "Vento");
        assert_eq!(limpar_titulo("07. Mar [Official Video]"), "Mar");
        assert_eq!(limpar_titulo("Canção 【MV】"), "Canção");
        assert_eq!(limpar_titulo("Sol (Official Audio)"), "Sol");
        // Parênteses do próprio nome ficam; número que é o título também
        assert_eq!(limpar_titulo("(Not) A Devil"), "(Not) A Devil");
        assert_eq!(limpar_titulo("1979"), "1979");
        assert_eq!(limpar_titulo("99 Luftballons"), "99 Luftballons");
        assert_eq!(limpar_titulo("03 Mar"), "Mar");
    }

    #[test]
    fn sem_artista_tenta_o_nome_do_arquivo() {
        let c = consulta("Banda - Música [HD]", None, 0);
        assert_eq!(c.termos(), ("Música".into(), Some("Banda".into())));
        let c = consulta("Música", Some("Banda"), 0);
        assert_eq!(c.termos(), ("Música".into(), Some("Banda".into())));
        let c = consulta("Chinozo 'ショットガン・ナウル' feat.FloweR", None, 0);
        assert_eq!(
            c.termos(),
            ("ショットガン・ナウル".into(), Some("Chinozo".into()))
        );
        let c = consulta("Banda - Música feat. Fulano", None, 0);
        assert_eq!(c.termos(), ("Música".into(), Some("Banda".into())));
    }

    #[test]
    fn semelhanca_ignora_acento_e_caixa() {
        assert_eq!(semelhanca("Canção", "cancao"), 1.0);
        assert_eq!(
            semelhanca("Bohemian Rhapsody", "Bohemian Rhapsody (Remastered)"),
            0.85
        );
        assert!(semelhanca("um dois tres", "um dois quatro") > 0.6);
        assert_eq!(semelhanca("", "x"), 0.0);
    }

    #[test]
    fn confianca_alta_so_quando_tudo_bate() {
        let c = consulta("Bohemian Rhapsody", Some("Queen"), 355);
        assert!(confianca_texto(&c, "Bohemian Rhapsody", "Queen", 356) >= CONFIANCA_AUTOMATICA);
        // Mesmo nome, outro artista: revisão
        assert!(
            confianca_texto(&c, "Bohemian Rhapsody", "Outra Banda", 356) < CONFIANCA_AUTOMATICA
        );
        // Instrumental não ganha da original
        assert!(
            confianca_texto(&c, "Bohemian Rhapsody (Instrumental)", "Queen", 355)
                < confianca_texto(&c, "Bohemian Rhapsody (Live)", "Queen", 355)
        );
        // Duração muito longe também derruba
        assert!(confianca_texto(&c, "Bohemian Rhapsody", "Queen", 200) < CONFIANCA_AUTOMATICA);
    }

    #[test]
    fn correcao_vai_por_cima_das_tags() {
        let mut t = TrackInfo {
            path: "a.mp3".into(),
            titulo: "faixa01".into(),
            album: Some("Velho".into()),
            ..Default::default()
        };
        let c = Correcao {
            titulo: "Vento".into(),
            artista: "Banda".into(),
            numero: 3,
            ..Default::default()
        };
        c.aplicar(&mut t);
        assert_eq!(t.titulo, "Vento");
        assert_eq!(t.artista.as_deref(), Some("Banda"));
        // Sem álbum na correção: fica o das tags
        assert_eq!(t.album.as_deref(), Some("Velho"));
        assert_eq!(t.numero, 3);
    }

    #[test]
    fn letra_ausente_espera_antes_de_buscar_de_novo() {
        let mut b = Banco::default();
        assert!(b.buscar_letra("/a"));
        b.marcar_sem_letra("/a");
        assert!(!b.buscar_letra("/a"));
        b.esquecer_sem_letra("/a");
        assert!(b.buscar_letra("/a"));
    }

    #[test]
    fn capa_web_so_aceita_mbid() {
        assert!(capa_web("96e5dc55-fa8b-483e-a673-e04e1f4cfd2b").is_some());
        assert!(capa_web("../../etc/passwd").is_none());
        assert!(capa_web("").is_none());
    }

    #[test]
    fn grava_tags_numa_copia_da_fixture() {
        let dir = std::env::temp_dir().join(format!("furinar-teste-tags-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let faixa = dir.join("tone.mp3");
        fs::copy(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tone.mp3"),
            &faixa,
        )
        .unwrap();
        let c = Correcao {
            titulo: "Título Novo".into(),
            artista: "Banda".into(),
            album: Some("Disco".into()),
            ano: Some(1999),
            numero: 1004,
            ..Default::default()
        };
        gravar_tags(&faixa, &c).unwrap();
        let lido = crate::track::ler_tags(&faixa);
        assert_eq!(lido.titulo, "Título Novo");
        assert_eq!(lido.artista.as_deref(), Some("Banda"));
        assert_eq!(lido.album.as_deref(), Some("Disco"));
        assert_eq!(lido.numero, 1004);
        let _ = fs::remove_dir_all(&dir);
    }
}
