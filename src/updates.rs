pub struct UpdateInfo {
    pub version: String,
    pub body: String,
}

/// Check if this installation was made via a package manager on Linux.
/// Package managers install to /usr/bin, /usr/local/bin, /snap, etc.
fn is_package_install() -> bool {
    #[cfg(target_os = "windows")]
    {
        false
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(exe) = std::env::current_exe() {
            let path = exe.to_string_lossy();
            path.starts_with("/usr/bin")
                || path.starts_with("/usr/local/bin")
                || path.starts_with("/snap")
                || path.starts_with("/var/lib/flatpak")
        } else {
            false
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        false
    }
}

fn get_target() -> &'static str {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "furinar-windows-x86_64.exe"
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "furinar-linux-x86_64.tar.gz"
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        "furinar-macos-x86_64.tar.gz"
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        "furinar-macos-aarch64.tar.gz"
    }
    #[cfg(not(any(
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
    )))]
    {
        "furinar"
    }
}

/// Strip the leading "v" from a tag like "v0.2.0" → "0.2.0".
fn strip_tag_prefix(tag: &str) -> &str {
    tag.strip_prefix('v').unwrap_or(tag)
}

/// Check for a newer release on GitHub.
///
/// Uses the `ReleaseList` API to fetch releases without downloading anything,
/// then compares the latest semver version against the compiled-in version.
pub fn check_for_update() -> Option<UpdateInfo> {
    if is_package_install() {
        return None;
    }

    let current_version = env!("CARGO_PKG_VERSION");
    let current = semver::Version::parse(current_version).ok()?;
    let target = get_target();

    // Fetch the release list from GitHub (non-blocking in the sense that it
    // only does an HTTP GET for the release metadata).
    let releases = self_update::backends::github::ReleaseList::configure()
        .repo_owner("LokKol17")
        .repo_name("furinar")
        .build()
        .ok()?
        .fetch()
        .ok()?;

    // Find the latest release that has an asset matching our target name.
    let latest = releases
        .iter()
        .find(|r| r.assets.iter().any(|a| a.name == target))
        .or_else(|| releases.first())?;

    let latest_version = semver::Version::parse(strip_tag_prefix(&latest.version)).ok()?;

    if latest_version > current {
        return Some(UpdateInfo {
            version: strip_tag_prefix(&latest.version).to_string(),
            body: latest.body.clone().unwrap_or_default(),
        });
    }

    None
}

/// Download and install the latest release.
///
/// Uses `self_update`'s `Update` builder which handles downloading the
/// correct asset, extracting (if archived), and replacing the current binary.
pub fn perform_update() -> Result<(), Box<dyn std::error::Error>> {
    let status = self_update::backends::github::Update::configure()
        .repo_owner("LokKol17")
        .repo_name("furinar")
        .target(get_target())
        .bin_name("furinar")
        .show_download_progress(true)
        .no_confirm(true)
        .current_version(env!("CARGO_PKG_VERSION"))
        .build()?
        .update()?;

    if status.updated() {
        println!("Updated to version {}", status.version());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_tag_prefix_remove_so_o_v_inicial() {
        assert_eq!(strip_tag_prefix("v1.2.3"), "1.2.3");
        assert_eq!(strip_tag_prefix("1.2.3"), "1.2.3");
        assert_eq!(strip_tag_prefix("vv1.0.0"), "v1.0.0");
        assert_eq!(strip_tag_prefix("1.0.0-rc1"), "1.0.0-rc1");
    }

    #[test]
    fn get_target_monta_o_nome_do_asset_da_plataforma() {
        let alvo = get_target();

        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        assert_eq!(alvo, "furinar-windows-x86_64.exe");
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        assert_eq!(alvo, "furinar-linux-x86_64.tar.gz");
        #[cfg(target_os = "macos")]
        assert!(alvo.starts_with("furinar-macos-"), "veio {alvo}");

        // O nome precisa ser compatível com o alvo usado nas releases
        assert!(!alvo.is_empty());
        assert!(alvo.starts_with("furinar"));
    }

    #[test]
    fn versao_compilada_e_semver_valido() {
        let versao = env!("CARGO_PKG_VERSION");
        assert!(
            semver::Version::parse(versao).is_ok(),
            "{versao} não é semver; check_for_update falharia"
        );
    }

    /// Smoke de rede: a HTTPS precisa funcionar com rustls (troca do
    /// native-tls/openssl). Ignorada por padrão por depender de internet:
    /// `cargo test -- --ignored --nocapture`.
    ///
    /// Usa `ReleaseList::fetch` direto porque `check_for_update` engole o
    /// erro com `.ok()?` e um TLS quebrado viraria só "sem atualização".
    #[test]
    #[ignore = "requer rede"]
    fn smoke_rede_busca_releases_via_rustls() {
        let releases = self_update::backends::github::ReleaseList::configure()
            .repo_owner("LokKol17")
            .repo_name("furinar")
            .build()
            .expect("ReleaseList build falhou")
            .fetch()
            .expect("HTTPS com rustls falhou");

        println!("{} releases encontrados", releases.len());
        let alvo = get_target();
        assert!(
            releases
                .iter()
                .any(|r| r.assets.iter().any(|a| a.name == alvo)),
            "nenhuma release tem o asset {alvo}; conferir release.yml"
        );
    }
}
