//! Devolve ao sistema páginas que o processo não precisa manter no RSS.
//!
//! O kernel mapeia código em blocos grandes (fault-around, large folios),
//! então boa parte do executável e das bibliotecas fica contada no RSS sem
//! estar em uso. Essas páginas são cópias limpas do page cache:
//! `MADV_DONTNEED` só as desmapeia, e um acesso futuro as traz de volta com
//! um page fault menor.
//!
//! As faixas vêm do próprio carregador (`dl_iterate_phdr`): só os segmentos
//! `PT_LOAD` sem permissão de escrita (código e dados só-leitura). A versão
//! anterior lia o `/proc/self/smaps` e aparava tudo que fosse só-leitura e
//! limpo, mas o smaps é um retrato de um instante: uma biblioteca sendo
//! carregada por outra thread nesse meio tempo (módulos de DNS do
//! `getaddrinfo`, plugins do PipeWire) aparecia como uma reserva "limpa" que,
//! quando o `madvise` chegava, já tinha a tabela de relocação preenchida — e
//! ela voltava zerada, derrubando o app com salto para o endereço 0. Pelo
//! carregador isso não acontece: a relocação mora sempre num segmento
//! gravável, que fica de fora, e durante a iteração nenhuma biblioteca é
//! descarregada.
//!
//! Fontes e o cache do fontconfig vêm do smaps, mas só os mapeamentos
//! compartilhados (`r--s`): o carregador nunca mapeia biblioteca assim, e
//! descartar uma página compartilhada não perde nada (ela é o próprio page
//! cache). Logo antes do `madvise`, `/proc/self/map_files` confere que a
//! faixa ainda é exatamente daquele arquivo.

/// Faixa (início, tamanho) de um segmento, encolhida para páginas inteiras:
/// a primeira e a última página podem ser divididas com um segmento vizinho.
#[cfg(any(target_os = "linux", test))]
fn segmento(base: usize, vaddr: usize, tamanho: usize, pagina: usize) -> Option<(usize, usize)> {
    let inicio = (base + vaddr).div_ceil(pagina) * pagina;
    let fim = (base + vaddr + tamanho) / pagina * pagina;
    (fim > inicio).then(|| (inicio, fim - inicio))
}

/// O que o smaps diz das regiões: as que têm páginas anônimas (cópias
/// privadas modificadas — um segmento só-leitura assim teve o código
/// relocado e não pode levar `DONTNEED`) e os arquivos de dados mapeados
/// compartilhados e só-leitura (fontes, caches), com o caminho.
#[cfg(any(target_os = "linux", test))]
#[derive(Default)]
struct Regioes {
    anonimas: Vec<(usize, usize)>,
    compartilhadas: Vec<(usize, usize, String)>,
}

#[cfg(any(target_os = "linux", test))]
fn ler_smaps(smaps: &str) -> Regioes {
    let mut r = Regioes::default();
    // (início, fim, caminho se for arquivo de dados compartilhado)
    let mut atual: Option<(usize, usize, Option<String>)> = None;
    for linha in smaps.lines() {
        let mut campos = linha.split_whitespace();
        let Some(primeiro) = campos.next() else {
            continue;
        };
        if let Some((inicio, fim)) = primeiro.split_once('-')
            && let (Ok(inicio), Ok(fim)) = (
                usize::from_str_radix(inicio, 16),
                usize::from_str_radix(fim, 16),
            )
        {
            let perms = campos.next().unwrap_or("");
            let caminho = campos.skip(3).collect::<Vec<_>>().join(" ");
            let dados = perms == "r--s"
                && caminho.starts_with('/')
                && !caminho.contains(".so")
                && !caminho.ends_with(" (deleted)")
                && fim > inicio;
            atual = Some((inicio, fim, dados.then_some(caminho)));
            continue;
        }
        // "Anonymous:" encerra o que importa de cada região
        if primeiro == "Anonymous:"
            && let Some((inicio, fim, caminho)) = atual.take()
        {
            if campos.next().is_some_and(|kb| kb != "0") {
                r.anonimas.push((inicio, fim));
            } else if let Some(caminho) = caminho {
                r.compartilhadas.push((inicio, fim, caminho));
            }
        }
    }
    r
}

#[cfg(any(target_os = "linux", test))]
fn cruza(faixa: (usize, usize), regioes: &[(usize, usize)]) -> bool {
    let (inicio, fim) = (faixa.0, faixa.0 + faixa.1);
    regioes.iter().any(|&(a, b)| inicio < b && a < fim)
}

/// Chamado pelo `dl_iterate_phdr` para cada objeto carregado (o executável
/// e cada biblioteca).
#[cfg(target_os = "linux")]
unsafe extern "C" fn aparar_objeto(
    info: *mut libc::dl_phdr_info,
    _tamanho: libc::size_t,
    dados: *mut libc::c_void,
) -> libc::c_int {
    // SAFETY: o carregador passa um dl_phdr_info válido durante a chamada, e
    // `dados` é o &Vec que `aparar` passou, vivo até o fim da iteração.
    let (info, anonimas) = unsafe { (&*info, &*(dados as *const Vec<(usize, usize)>)) };
    if info.dlpi_phdr.is_null() {
        return 0;
    }
    // SAFETY: dlpi_phdr aponta para dlpi_phnum cabeçalhos do objeto
    let cabecalhos =
        unsafe { std::slice::from_raw_parts(info.dlpi_phdr, info.dlpi_phnum as usize) };
    // [vdso] é um mapeamento especial do kernel: não é cópia de arquivo
    let nome = if info.dlpi_name.is_null() {
        ""
    } else {
        // SAFETY: nome em C terminado em zero, válido durante a chamada
        unsafe { std::ffi::CStr::from_ptr(info.dlpi_name) }
            .to_str()
            .unwrap_or("")
    };
    if nome.contains("vdso") {
        return 0;
    }
    // SAFETY: sysconf não tem pré-condição
    let pagina = unsafe { libc::sysconf(libc::_SC_PAGESIZE) }.max(4096) as usize;
    for h in cabecalhos {
        if h.p_type != libc::PT_LOAD || h.p_flags & libc::PF_W != 0 {
            continue;
        }
        let Some(faixa) = segmento(
            info.dlpi_addr as usize,
            h.p_vaddr as usize,
            h.p_memsz as usize,
            pagina,
        ) else {
            continue;
        };
        if cruza(faixa, anonimas) {
            continue;
        }
        // SAFETY: segmento só-leitura de um objeto carregado, sem páginas
        // anônimas: o conteúdo volta idêntico do arquivo. Enquanto o
        // dl_iterate_phdr roda, nenhum objeto é descarregado.
        unsafe {
            libc::madvise(faixa.0 as *mut libc::c_void, faixa.1, libc::MADV_DONTNEED);
        }
    }
    0
}

/// Desmapeia o código limpo do executável e das bibliotecas e devolve o heap
/// livre ao SO.
#[cfg(target_os = "linux")]
pub fn aparar() {
    let regioes = std::fs::read_to_string("/proc/self/smaps")
        .map(|s| ler_smaps(&s))
        .unwrap_or_default();
    // SAFETY: o callback só lê o que o carregador e `anonimas` fornecem
    unsafe {
        libc::dl_iterate_phdr(
            Some(aparar_objeto),
            &regioes.anonimas as *const Vec<(usize, usize)> as *mut libc::c_void,
        );
    }
    for (inicio, fim, caminho) in regioes.compartilhadas {
        let ainda = std::fs::read_link(format!("/proc/self/map_files/{inicio:x}-{fim:x}"));
        if !ainda.is_ok_and(|c| c.as_os_str() == caminho.as_str()) {
            continue;
        }
        // SAFETY: mapeamento compartilhado só-leitura de um arquivo de dados,
        // conferido agora há pouco: as páginas são o page cache, descartar
        // não perde conteúdo.
        unsafe {
            libc::madvise(
                inicio as *mut libc::c_void,
                fim - inicio,
                libc::MADV_DONTNEED,
            );
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

    #[test]
    fn segmento_fica_so_com_paginas_inteiras() {
        // Já alinhado
        assert_eq!(
            segmento(0x1000, 0x2000, 0x3000, 0x1000),
            Some((0x3000, 0x3000))
        );
        // Pontas divididas com vizinhos: ficam de fora
        assert_eq!(
            segmento(0x1000, 0x2800, 0x3000, 0x1000),
            Some((0x4000, 0x2000))
        );
        // Menor que uma página inteira
        assert_eq!(segmento(0, 0x1200, 0x800, 0x1000), None);
    }

    #[test]
    fn separa_anonimas_e_compartilhadas() {
        let smaps = "\
5555aaaa0000-5555aaab0000 r-xp 00000000 00:2a 123 /usr/bin/furinar
Private_Dirty:         8 kB
Anonymous:             0 kB
5555aaac0000-5555aaad0000 r--p 00020000 00:2a 123 /usr/bin/furinar
Anonymous:            48 kB
5555aaad0000-5555aaae0000 rw-p 00030000 00:2a 123 /usr/bin/furinar
Anonymous:             0 kB
7f0000000000-7f0000100000 r--s 00000000 00:2a 456 /var/cache/fontconfig/abc-le64.cache-12
Anonymous:             0 kB
7f0000100000-7f0000200000 rw-s 00000000 00:01 789 /memfd:softbuffer (deleted)
Anonymous:             0 kB
7f0000200000-7f0000300000 r--s 00000000 00:2a 457 /usr/share/fonts/Minha Fonte.ttf
Anonymous:             0 kB
7f0000300000-7f0000400000 r--s 00000000 00:2a 458 /usr/lib/libalgo.so.1
Anonymous:             0 kB
7fff00000000-7fff00002000 r-xp 00000000 00:00 0 [vdso]
Anonymous:             0 kB
";
        let r = ler_smaps(smaps);
        // Página suja no page cache (binário recém-gravado) não é anônima
        assert_eq!(r.anonimas, vec![(0x5555aaac0000, 0x5555aaad0000)]);
        // Privado, gravável, memfd e vdso ficam de fora
        assert_eq!(
            r.compartilhadas,
            vec![
                (
                    0x7f0000000000,
                    0x7f0000100000,
                    "/var/cache/fontconfig/abc-le64.cache-12".to_string()
                ),
                (
                    0x7f0000200000,
                    0x7f0000300000,
                    "/usr/share/fonts/Minha Fonte.ttf".to_string()
                )
            ]
        );
        assert!(ler_smaps("").anonimas.is_empty());
    }

    #[test]
    fn cruzamento_de_faixas() {
        let anon = [(0x5000, 0x6000)];
        assert!(cruza((0x4000, 0x2000), &anon));
        assert!(!cruza((0x6000, 0x1000), &anon));
        assert!(!cruza((0x3000, 0x2000), &anon));
    }

    /// O de verdade, no próprio processo de teste (que tem outras threads
    /// rodando): não pode derrubar nada.
    #[cfg(target_os = "linux")]
    #[test]
    fn aparar_no_processo_de_verdade() {
        let antes = std::fs::read_to_string("/proc/self/status").unwrap();
        aparar();
        aparar();
        // Código continua funcionando depois (inclusive o de bibliotecas)
        let depois = std::fs::read_to_string("/proc/self/status").unwrap();
        assert!(antes.contains("VmRSS") && depois.contains("VmRSS"));
        assert_eq!(format!("{:.2}", std::f64::consts::PI.sin().abs()), "0.00");
    }

    /// Estresse da corrida que derrubava o app: uma thread carrega e
    /// descarrega bibliotecas (e chama funções delas) sem parar, enquanto a
    /// outra apara. Manual: `cargo test estresse_aparar -- --ignored`.
    /// `FURINAR_ESTRESSE=antigo` usa o algoritmo antigo (pelo smaps), que
    /// trava com salto para o endereço 0.
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "estresse de vários segundos"]
    fn estresse_aparar() {
        use std::ffi::CString;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};

        let parar = Arc::new(AtomicBool::new(false));
        let carregador = {
            let parar = parar.clone();
            std::thread::spawn(move || {
                let libs = [
                    ("libsqlite3.so.0", "sqlite3_libversion_number"),
                    ("libjson-c.so.5", "json_c_version_num"),
                    ("libyaml-0.so.2", "yaml_get_version_string"),
                    ("liblzo2.so.2", "lzo_version"),
                ];
                let mut voltas = 0u64;
                while !parar.load(Ordering::Relaxed) {
                    for (lib, simbolo) in libs {
                        let lib = CString::new(lib).unwrap();
                        let simbolo = CString::new(simbolo).unwrap();
                        // SAFETY: bibliotecas do sistema; a função é chamada
                        // só pelo endereço, sem argumentos usados
                        unsafe {
                            let h = libc::dlopen(lib.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
                            if h.is_null() {
                                continue;
                            }
                            let f = libc::dlsym(h, simbolo.as_ptr());
                            if !f.is_null() {
                                let f: extern "C" fn() -> usize = std::mem::transmute(f);
                                std::hint::black_box(f());
                            }
                            libc::dlclose(h);
                        }
                    }
                    voltas += 1;
                }
                voltas
            })
        };

        let antigo = std::env::var("FURINAR_ESTRESSE").is_ok_and(|v| v == "antigo");
        let fim = std::time::Instant::now() + std::time::Duration::from_secs(8);
        let mut aparos = 0u64;
        while std::time::Instant::now() < fim {
            if antigo {
                aparar_antigo();
            } else {
                aparar();
            }
            aparos += 1;
        }
        parar.store(true, Ordering::Relaxed);
        let voltas = carregador.join().unwrap();
        println!("{aparos} aparos, {voltas} voltas de dlopen");
    }

    /// O algoritmo antigo, só para o estresse mostrar a diferença.
    #[cfg(target_os = "linux")]
    fn aparar_antigo() {
        let smaps = std::fs::read_to_string("/proc/self/smaps").unwrap_or_default();
        let mut candidata: Option<(usize, usize, bool)> = None;
        let mut regioes = Vec::new();
        for linha in smaps.lines() {
            let mut c = linha.split_whitespace();
            let Some(p) = c.next() else { continue };
            if let Some((a, b)) = p.split_once('-')
                && let (Ok(a), Ok(b)) = (usize::from_str_radix(a, 16), usize::from_str_radix(b, 16))
            {
                if let Some((a, b, true)) = candidata.take() {
                    regioes.push((a, b));
                }
                let perms = c.next().unwrap_or("");
                let caminho = c.nth(3).unwrap_or("");
                candidata = Some((a, b, caminho.starts_with('/') && !perms.contains('w')));
                continue;
            }
            if matches!(p, "Private_Dirty:" | "Anonymous:")
                && c.next().is_some_and(|kb| kb != "0")
                && let Some(r) = candidata.as_mut()
            {
                r.2 = false;
            }
        }
        if let Some((a, b, true)) = candidata {
            regioes.push((a, b));
        }
        for (a, b) in regioes {
            // SAFETY: (não é seguro — é a reprodução do bug)
            unsafe {
                libc::madvise(a as *mut libc::c_void, b - a, libc::MADV_DONTNEED);
            }
        }
    }
}
