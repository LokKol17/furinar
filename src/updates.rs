pub struct UpdateInfo {
    pub version: String,
}

/// Check if this installation was made via a package manager on Linux.
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

fn strip_tag_prefix(tag: &str) -> &str {
    tag.strip_prefix('v').unwrap_or(tag)
}

/// Check for a newer release on GitHub using a single API call.
pub fn check_for_update() -> Option<UpdateInfo> {
    eprintln!("[update] Starting update check...");

    if is_package_install() {
        eprintln!("[update] Package install detected, skipping.");
        return None;
    }

    let current_version = env!("CARGO_PKG_VERSION");
    eprintln!("[update] Current version: {current_version}");
    let current = semver::Version::parse(current_version).ok()?;
    let target = get_target();
    eprintln!("[update] Target asset: {target}");

    let url = "https://api.github.com/repos/LokKol17/furinar/releases/latest";
    eprintln!("[update] Fetching {url}...");

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent(format!("furinar/{current_version}"))
        .build()
        .ok()?;

    let resp = match client.get(url).send() {
        Ok(r) => {
            eprintln!("[update] HTTP status: {}", r.status());
            r
        }
        Err(e) => {
            eprintln!("[update] HTTP error: {e}");
            return None;
        }
    };

    let json: serde_json::Value = match resp.json() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[update] JSON parse error: {e}");
            return None;
        }
    };

    let tag = match json["tag_name"].as_str() {
        Some(t) => {
            eprintln!("[update] Latest tag: {t}");
            t
        }
        None => {
            eprintln!("[update] No tag_name in response");
            return None;
        }
    };

    let latest_version = match semver::Version::parse(strip_tag_prefix(tag)) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[update] Version parse error: {e}");
            return None;
        }
    };

    eprintln!("[update] Latest version: {latest_version}, current: {current}");

    if latest_version <= current {
        eprintln!("[update] Already up to date.");
        return None;
    }

    let has_asset = json["assets"]
        .as_array()
        .map(|arr| arr.iter().any(|a| a["name"].as_str() == Some(target)))
        .unwrap_or(false);

    eprintln!("[update] Has target asset: {has_asset}");

    if !has_asset {
        return None;
    }

    eprintln!("[update] Update available: v{latest_version}");
    Some(UpdateInfo {
        version: strip_tag_prefix(tag).to_string(),
    })
}

#[allow(dead_code)]
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
