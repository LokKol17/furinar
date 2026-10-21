//! Devolve ao sistema páginas que o processo não precisa manter no RSS.
//!
//! O kernel mapeia código e fontes em blocos grandes (fault-around, large
//! folios), então boa parte do binário, das libs e dos arquivos de fonte fica
//! contada no RSS sem estar em uso. Essas páginas são cópias limpas do page
//! cache: `MADV_DONTNEED` só as desmapeia, e um acesso futuro as traz de volta
//! com um page fault menor, sem I/O e sem perder nada.

/// Uma região é aparável quando é cópia limpa de um arquivo: backed por
/// arquivo, sem permissão de escrita e sem nenhuma página privada suja ou
/// anônima. Isso deixa de fora `.data.rel.ro` (relocada), o memfd do
/// framebuffer, heap, pilhas e `[vdso]`.
#[cfg(any(target_os = "linux", test))]
fn regioes_aparaveis(smaps: &str) -> Vec<(usize, usize)> {
    struct Regiao {
        inicio: usize,
        fim: usize,
        candidata: bool,
        suja: bool,
    }

    fn fechar(regiao: Option<Regiao>, saida: &mut Vec<(usize, usize)>) {
        if let Some(r) = regiao
            && r.candidata
            && !r.suja
        {
            saida.push((r.inicio, r.fim - r.inicio));
        }
    }

    let mut saida = Vec::new();
    let mut atual: Option<Regiao> = None;

    for linha in smaps.lines() {
        let mut campos = linha.split_whitespace();
        let Some(primeiro) = campos.next() else {
            continue;
        };

        // Cabeçalho: "inicio-fim perms offset dev inode [caminho]"
        if let Some((inicio, fim)) = primeiro.split_once('-')
            && let (Ok(inicio), Ok(fim)) = (
                usize::from_str_radix(inicio, 16),
                usize::from_str_radix(fim, 16),
            )
        {
            fechar(atual.take(), &mut saida);
            let perms = campos.next().unwrap_or("");
            let caminho = campos.nth(3).unwrap_or("");
            atual = Some(Regiao {
                inicio,
                fim,
                candidata: caminho.starts_with('/') && !perms.contains('w') && fim > inicio,
                suja: false,
            });
            continue;
        }

        if matches!(primeiro, "Private_Dirty:" | "Anonymous:")
            && let Some(r) = atual.as_mut()
            && campos.next().is_some_and(|kb| kb != "0")
        {
            r.suja = true;
        }
    }
    fechar(atual, &mut saida);

    saida
}

/// Desmapeia as páginas limpas de arquivo e devolve o heap livre ao SO.
#[cfg(target_os = "linux")]
pub fn aparar() {
    let Ok(smaps) = std::fs::read_to_string("/proc/self/smaps") else {
        return;
    };
    for (inicio, tamanho) in regioes_aparaveis(&smaps) {
        // SAFETY: só regiões só-leitura, backed por arquivo e sem páginas
        // privadas sujas/anônimas — o conteúdo volta idêntico do page cache.
        unsafe {
            libc::madvise(inicio as *mut libc::c_void, tamanho, libc::MADV_DONTNEED);
        }
    }

    #[cfg(target_env = "gnu")]
    // SAFETY: malloc_trim só libera páginas livres do alocador
    unsafe {
        libc::malloc_trim(0);
    }
}

#[cfg(not(target_os = "linux"))]
pub fn aparar() {}

#[cfg(test)]
mod tests {
    use super::*;

    const SMAPS: &str = "\
5555aaaa0000-5555aaab0000 r--p 00000000 00:2a 123 /usr/bin/furinar
Rss:                  64 kB
Private_Dirty:         0 kB
Anonymous:             0 kB
5555aaab0000-5555aaac0000 r-xp 00010000 00:2a 123 /usr/bin/furinar
Rss:                  64 kB
Private_Dirty:         0 kB
Anonymous:             0 kB
VmFlags: rd ex mr mw me
5555aaac0000-5555aaad0000 r--p 00020000 00:2a 123 /usr/bin/furinar
Private_Dirty:        48 kB
Anonymous:            48 kB
5555aaad0000-5555aaae0000 rw-p 00030000 00:2a 123 /usr/bin/furinar
Private_Dirty:         8 kB
Anonymous:             8 kB
5555aaae0000-5555aaaf0000 rw-p 00000000 00:00 0 [heap]
Private_Dirty:        64 kB
Anonymous:            64 kB
7f0000000000-7f0000100000 r--s 00000000 00:2a 456 /var/cache/fontconfig/abc-le64.cache-12
Private_Dirty:         0 kB
Anonymous:             0 kB
7f0000100000-7f0000200000 rw-s 00000000 00:01 789 /memfd:softbuffer (deleted)
Private_Dirty:         0 kB
Anonymous:             0 kB
7fff00000000-7fff00002000 r-xp 00000000 00:00 0 [vdso]
Private_Dirty:         0 kB
";

    #[test]
    fn aceita_so_copias_limpas_de_arquivo() {
        assert_eq!(
            regioes_aparaveis(SMAPS),
            vec![
                (0x5555aaaa0000, 0x10000),
                (0x5555aaab0000, 0x10000),
                (0x7f0000000000, 0x100000),
            ]
        );
    }

    #[test]
    fn rejeita_relro_sujo_heap_memfd_e_vdso() {
        let regioes = regioes_aparaveis(SMAPS);
        for inicio in [
            0x5555aaac0000usize, // relro relocada
            0x5555aaad0000,      // .data
            0x5555aaae0000,      // heap
            0x7f0000100000,      // framebuffer
            0x7fff00000000,      // vdso
        ] {
            assert!(
                regioes.iter().all(|(i, _)| *i != inicio),
                "{inicio:#x} não pode ser aparada"
            );
        }
    }

    #[test]
    fn smaps_vazio_ou_lixo_nao_quebra() {
        assert!(regioes_aparaveis("").is_empty());
        assert!(regioes_aparaveis("nada aqui\nRss: 4 kB\n").is_empty());
    }
}
