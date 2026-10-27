#!/usr/bin/env bash
# ============================================================================
# Furinar — installer for Linux
# Supports: Debian/Ubuntu, Arch/Manjaro, Fedora, openSUSE (and any x86_64 distro)
#
# Installs for the current user only (~/.local), without sudo, so Furinar can
# update itself from inside the app. sudo is only used to install the system
# libraries it needs and to remove an old system-wide install (/usr/local).
#
#   curl -fsSL https://raw.githubusercontent.com/LokKol17/furinar/master/pkg/install.sh | bash
#
# FURINAR_VERSION=X.Y.Z pins a version; by default the latest release is used.
# ============================================================================
set -euo pipefail

FURINAR_VERSION="${FURINAR_VERSION:-}"
REPO="LokKol17/furinar"
TARBALL="furinar-linux-x86_64.tar.gz"

BIN_DIR="${XDG_BIN_HOME:-$HOME/.local/bin}"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}"
ICON_DIR="$DATA_DIR/icons/hicolor/256x256/apps"
DESKTOP_DIR="$DATA_DIR/applications"
METADATA_DIR="$DATA_DIR/metainfo"

# Where versions up to 1.7.1 installed (system-wide, with sudo)
OLD_FILES=(
    /usr/local/bin/furinar
    /usr/share/icons/hicolor/256x256/apps/furinar.png
    /usr/share/applications/furinar.desktop
    /usr/share/metainfo/dev.furinar.Furinar.metainfo.xml
)

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

info()  { echo -e "${CYAN}[INFO]${NC}  $*"; }
ok()    { echo -e "${GREEN}[OK]${NC}    $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
err()   { echo -e "${RED}[ERROR]${NC} $*" >&2; }

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# ---------------------------------------------------------------------------
# Download helper (curl or wget)
# ---------------------------------------------------------------------------
fetch() {
    local url="$1" out="$2"
    if command -v curl &>/dev/null; then
        curl -fsSL "$url" -o "$out"
    elif command -v wget &>/dev/null; then
        wget -q "$url" -O "$out"
    else
        err "curl or wget is required for download."
        exit 1
    fi
}

release_url() {
    if [ -n "$FURINAR_VERSION" ]; then
        echo "https://github.com/${REPO}/releases/download/v${FURINAR_VERSION#v}/$1"
    else
        echo "https://github.com/${REPO}/releases/latest/download/$1"
    fi
}

# ---------------------------------------------------------------------------
# Detect distro
# ---------------------------------------------------------------------------
detect_distro() {
    DISTRO_FAMILY="unknown"
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        case "${ID:-unknown}" in
            debian|ubuntu|linuxmint|pop|elementary|zorin|kali|raspbian)
                DISTRO_FAMILY="debian" ;;
            arch|manjaro|endeavouros|garuda|artix)
                DISTRO_FAMILY="arch" ;;
            fedora|nobara)
                DISTRO_FAMILY="fedora" ;;
            opensuse*|suse*)
                DISTRO_FAMILY="suse" ;;
        esac
        info "Detected distribution: ${PRETTY_NAME:-${ID:-unknown}}"
    fi
}

# ---------------------------------------------------------------------------
# Install system dependencies (the only step that needs sudo)
# ---------------------------------------------------------------------------
install_deps() {
    info "Checking system libraries (ALSA, GTK 3)..."
    if command -v ldconfig &>/dev/null \
        && ldconfig -p 2>/dev/null | grep -q 'libasound\.so\.2' \
        && ldconfig -p 2>/dev/null | grep -q 'libgtk-3\.so\.0'; then
        ok "Already installed."
        return 0
    fi
    case "$DISTRO_FAMILY" in
        debian)
            sudo apt-get update -qq
            sudo apt-get install -y -qq libasound2 libgtk-3-0 2>/dev/null || \
            sudo apt-get install -y -qq libasound2t64 libgtk-3-0t64 2>/dev/null || true
            ;;
        arch)
            sudo pacman -S --noconfirm --needed alsa-lib gtk3 2>/dev/null || true
            ;;
        fedora)
            sudo dnf install -y alsa-lib gtk3 2>/dev/null || true
            ;;
        suse)
            sudo zypper install -y libasound2 gtk3 2>/dev/null || true
            ;;
        *)
            warn "Unrecognized distribution. Make sure these are installed:"
            warn "  - libasound2 (ALSA)"
            warn "  - libgtk-3 (file dialogs)"
            ;;
    esac
}

# ---------------------------------------------------------------------------
# Download and verify the binary
# ---------------------------------------------------------------------------
download_binary() {
    info "Downloading Furinar ${FURINAR_VERSION:-(latest)}..."
    fetch "$(release_url "$TARBALL")" "$WORK/$TARBALL"

    # Releases from 1.7.2 on ship SHA256SUMS; refuse a tarball that doesn't match
    if fetch "$(release_url SHA256SUMS)" "$WORK/SHA256SUMS" 2>/dev/null; then
        local expected actual
        expected="$(awk -v f="$TARBALL" '{ n = $2; sub(/^\*/, "", n); sub(/.*\//, "", n) } n == f { print tolower($1) }' "$WORK/SHA256SUMS")"
        actual="$(sha256sum "$WORK/$TARBALL" | cut -d' ' -f1)"
        if [ -z "$expected" ] || [ "$expected" != "$actual" ]; then
            err "Checksum mismatch for $TARBALL (expected ${expected:-?}, got $actual)."
            err "The download is corrupted or was tampered with. Nothing was installed."
            exit 1
        fi
        ok "Checksum verified."
    else
        warn "This release has no SHA256SUMS (older than 1.7.2); skipping the checksum."
    fi

    tar -xzf "$WORK/$TARBALL" -C "$WORK" furinar
    chmod +x "$WORK/furinar"
}

copy_local_binary() {
    local src="target/release/furinar"
    if [ ! -f "$src" ]; then
        err "Binary not found at $src. Run 'cargo build --release' first."
        exit 1
    fi
    install -m755 "$src" "$WORK/furinar"
}

# ---------------------------------------------------------------------------
# Old system-wide install (/usr/local): it would shadow the new one in PATH
# and in the app menu, and it can't update itself
# ---------------------------------------------------------------------------
remove_old_system_install() {
    local found=()
    for f in "${OLD_FILES[@]}"; do
        [ -e "$f" ] && found+=("$f")
    done
    [ "${#found[@]}" -eq 0 ] && return 0

    warn "Found an old system-wide install (it needs sudo to be removed):"
    for f in "${found[@]}"; do warn "  $f"; done
    if sudo rm -f "${found[@]}"; then
        ok "Old system-wide install removed."
    else
        warn "Could not remove it. Delete those files by hand, or the old version may still open."
    fi
}

# ---------------------------------------------------------------------------
# Install files under ~/.local
# ---------------------------------------------------------------------------
install_files() {
    info "Installing to ${BIN_DIR}..."
    install -Dm755 "$WORK/furinar" "$BIN_DIR/furinar"

    # Icon: from the repo when running inside it, otherwise downloaded
    if [ -f "ui/assets/furinar_icon.png" ]; then
        install -Dm644 "ui/assets/furinar_icon.png" "$ICON_DIR/furinar.png"
    elif fetch "https://raw.githubusercontent.com/${REPO}/master/ui/assets/furinar_icon.png" "$WORK/furinar.png" 2>/dev/null; then
        install -Dm644 "$WORK/furinar.png" "$ICON_DIR/furinar.png"
    fi

    # Desktop entry with the absolute path: ~/.local/bin isn't always in the
    # PATH that the desktop session uses to launch apps
    mkdir -p "$DESKTOP_DIR"
    cat > "$DESKTOP_DIR/furinar.desktop" << DESKTOP
[Desktop Entry]
Type=Application
Name=Furinar
GenericName=Music Player
Comment=A light, fast, and beautiful audio player
Exec="$BIN_DIR/furinar" %F
Icon=furinar
Terminal=false
Categories=AudioVideo;Audio;Music;Player;
MimeType=audio/mpeg;audio/flac;audio/x-flac;audio/ogg;audio/x-vorbis+ogg;audio/wav;audio/x-wav;audio/mp4;audio/x-m4a;audio/webm;video/mp4;video/quicktime;video/x-m4v;video/x-matroska;video/webm;video/x-msvideo;
Keywords=music;player;audio;mp3;flac;ogg;
StartupWMClass=furinar
DESKTOP

    if [ -f "pkg/furinar.metainfo.xml" ]; then
        install -Dm644 "pkg/furinar.metainfo.xml" "$METADATA_DIR/dev.furinar.Furinar.metainfo.xml"
    fi

    command -v update-desktop-database &>/dev/null && update-desktop-database -q "$DESKTOP_DIR" 2>/dev/null || true
    command -v gtk-update-icon-cache &>/dev/null && gtk-update-icon-cache -q -t "$DATA_DIR/icons/hicolor" 2>/dev/null || true

    ok "Furinar installed at $BIN_DIR/furinar"
    case ":$PATH:" in
        *":$BIN_DIR:"*) ;;
        *) warn "$BIN_DIR is not in your PATH. The app menu entry works; to run 'furinar' from a terminal, add it to PATH." ;;
    esac
    info "Furinar now updates itself: when a new version is out, it offers the update inside the app."
    ffmpeg_hint
}

# MKV/WebM/AVI and the track video in the immersive mode need ffmpeg. Only a
# hint: through "curl | bash" there is no terminal to ask
ffmpeg_hint() {
    command -v ffmpeg &>/dev/null && command -v ffprobe &>/dev/null && return 0
    local cmd=""
    case "$DISTRO_FAMILY" in
        debian) cmd="sudo apt install ffmpeg" ;;
        arch) cmd="sudo pacman -S ffmpeg" ;;
        fedora) cmd="sudo dnf install ffmpeg-free" ;;
        suse) cmd="sudo zypper install ffmpeg" ;;
    esac
    warn "Optional: install ffmpeg for MKV/WebM/AVI and the video in the immersive mode${cmd:+: $cmd}"
}

# ---------------------------------------------------------------------------
# Uninstall
# ---------------------------------------------------------------------------
uninstall() {
    info "Uninstalling Furinar..."
    rm -f "$BIN_DIR/furinar" "$ICON_DIR/furinar.png" \
        "$DESKTOP_DIR/furinar.desktop" "$METADATA_DIR/dev.furinar.Furinar.metainfo.xml"
    command -v update-desktop-database &>/dev/null && update-desktop-database -q "$DESKTOP_DIR" 2>/dev/null || true
    remove_old_system_install
    ok "Furinar uninstalled. Your settings are kept in ~/.config/furinar."
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Without arguments, an interactive menu is shown (Install / Uninstall)."
    echo ""
    echo "Options:"
    echo "  --install      Install or update (latest release, or FURINAR_VERSION=X.Y.Z)"
    echo "  --deps         Only install the system libraries"
    echo "  --from-source  Install the local build (target/release/furinar)"
    echo "  --uninstall    Uninstall Furinar"
    echo "  --help         Show this message"
}

choose_mode_interactive() {
    local choice
    while true; do
        echo "" >&2
        echo "Choose an option:" >&2
        echo "1) Install / update" >&2
        echo "2) Uninstall" >&2
        echo "" >&2
        if ! IFS= read -r -p "> " choice < /dev/tty; then
            # No terminal (e.g. piped from curl without a TTY): just install
            echo "install"
            return 0
        fi
        case "$choice" in
            1|"") echo "install"; return 0 ;;
            2) echo "uninstall"; return 0 ;;
            *) err "Invalid option. Please choose 1 or 2." ;;
        esac
    done
}

main() {
    local mode=""
    if [ "$#" -eq 0 ]; then
        mode="$(choose_mode_interactive)"
    else
        for arg in "$@"; do
            case "$arg" in
                --help|-h) usage; exit 0 ;;
                --install) mode="install" ;;
                --deps) mode="deps" ;;
                --from-source) mode="source" ;;
                --uninstall) mode="uninstall" ;;
                *) err "Unknown option: $arg"; usage; exit 1 ;;
            esac
        done
    fi

    if [ "$(id -u)" -eq 0 ]; then
        warn "Running as root installs Furinar for root only. Run it as your normal user."
    fi

    detect_distro

    case "$mode" in
        deps) install_deps ;;
        source)
            install_deps
            copy_local_binary
            remove_old_system_install
            install_files
            ;;
        install)
            install_deps
            download_binary
            remove_old_system_install
            install_files
            ;;
        uninstall) uninstall ;;
        *) err "Invalid mode: $mode"; exit 1 ;;
    esac
    echo ""
}

main "$@"
