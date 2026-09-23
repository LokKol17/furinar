#!/usr/bin/env bash
# ============================================================================
# Furinar — Instalador universal para Linux
# Suporta: Debian/Ubuntu, Arch/Manjaro, Fedora
# ============================================================================
set -euo pipefail

VERSION="${FURINAR_VERSION:-1.5.3}"
INSTALL_DIR="/usr/local/bin"
ICON_DIR="/usr/share/icons/hicolor/256x256/apps"
DESKTOP_DIR="/usr/share/applications"
METADATA_DIR="/usr/share/metainfo"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

info()  { echo -e "${CYAN}[INFO]${NC}  $*"; }
ok()    { echo -e "${GREEN}[OK]${NC}    $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
err()   { echo -e "${RED}[ERROR]${NC} $*" >&2; }

# ---------------------------------------------------------------------------
# Detectar distro
# ---------------------------------------------------------------------------
detect_distro() {
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        DISTRO_ID="${ID:-unknown}"
        DISTRO_FAMILY=""
        case "$DISTRO_ID" in
            debian|ubuntu|linuxmint|pop|elementary|zorin|kali|raspbian)
                DISTRO_FAMILY="debian" ;;
            arch|manjaro|endeavouros|garuda|artix)
                DISTRO_FAMILY="arch" ;;
            fedora|nobara)
                DISTRO_FAMILY="fedora" ;;
            opensuse*|suse*)
                DISTRO_FAMILY="suse" ;;
            alpine)
                DISTRO_FAMILY="alpine" ;;
            *)
                DISTRO_FAMILY="unknown" ;;
        esac
        info "Distribuição detectada: ${PRETTY_NAME:-$DISTRO_ID}"
        info "Família: $DISTRO_FAMILY"
    else
        err "Não foi possível detectar a distribuição."
        exit 1
    fi
}

# ---------------------------------------------------------------------------
# Instalar dependências do sistema
# ---------------------------------------------------------------------------
install_deps() {
    info "Instalando dependências do sistema..."
    case "$DISTRO_FAMILY" in
        debian)
            sudo apt-get update -qq
            sudo apt-get install -y -qq libasound2 libgtk-3-0 2>/dev/null || \
            sudo apt-get install -y -qq libasound2 libgtk-3-0t64 2>/dev/null || true
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
            warn "Distribuição não reconhecida. Instale manualmente:"
            warn "  - libasound2 (ALSA)"
            warn "  - libgtk-3 (file dialogs)"
            ;;
    esac
    ok "Dependências verificadas."
}

# ---------------------------------------------------------------------------
# Baixar binário do GitHub Release
# ---------------------------------------------------------------------------
download_binary() {
    local url="https://github.com/LokKol17/furinar/releases/download/v${VERSION}/furinar-linux-x86_64"
    local dest="/tmp/furinar"

    info "Baixando furinar v${VERSION}..."
    if command -v curl &>/dev/null; then
        curl -fSL "$url" -o "$dest"
    elif command -v wget &>/dev/null; then
        wget -q "$url" -O "$dest"
    else
        err "curl ou wget necessário para download."
        exit 1
    fi
    chmod +x "$dest"

    # Baixar ícone junto
    local icon_url="https://raw.githubusercontent.com/LokKol17/furinar/master/ui/assets/furinar_icon.png"
    local icon_dest="/tmp/furinar_icon.png"
    if command -v curl &>/dev/null; then
        curl -fSL "$icon_url" -o "$icon_dest" 2>/dev/null || true
    elif command -v wget &>/dev/null; then
        wget -q "$icon_url" -O "$icon_dest" 2>/dev/null || true
    fi

    ok "Binário baixado."
}

# ---------------------------------------------------------------------------
# Gerar desktop entry e metadados inline (quando não estamos no repo)
# ---------------------------------------------------------------------------
generate_desktop_files() {
    # Desktop entry
    if [ ! -f /tmp/furinar.desktop ]; then
        cat > /tmp/furinar.desktop << 'DESKTOP'
[Desktop Entry]
Type=Application
Name=Furinar
GenericName=Music Player
Comment=A light, fast, and beautiful audio player
Exec=furinar %f
Icon=furinar
Terminal=false
Categories=Audio;Music;Player;
MimeType=audio/mpeg;audio/x-flac;audio/ogg;audio/wav;audio/mp4;
Keywords=music;player;audio;mp3;flac;ogg;
StartupWMClass=furinar
DESKTOP
    fi

    # AppStream metadata
    if [ ! -f /tmp/dev.furinar.Furinar.metainfo.xml ]; then
        cat > /tmp/dev.furinar.Furinar.metainfo.xml << 'META'
<?xml version="1.0" encoding="UTF-8"?>
<component type="desktop-application">
  <id>dev.furinar.Furinar</id>
  <name>Furinar</name>
  <summary>A light, fast, and beautiful audio player</summary>
  <metadata_license>BSD-3-Clause</metadata_license>
  <project_license>BSD-3-Clause</project_license>
  <launchable type="desktop-id">furinar.desktop</launchable>
  <provides><binary>furinar</binary></provides>
  <content_rating type="oars-1.1" />
</component>
META
    fi
}

# ---------------------------------------------------------------------------
# Instalar arquivos
# ---------------------------------------------------------------------------
install_files() {
    info "Instalando furinar..."
    sudo install -Dm755 /tmp/furinar "${INSTALL_DIR}/furinar"
    sudo rm -f /tmp/furinar

    # Ícone — tenta do repo, senão usa o baixado
    local icon_src=""
    if [ -f "ui/assets/furinar_icon.png" ]; then
        icon_src="ui/assets/furinar_icon.png"
    elif [ -f /tmp/furinar_icon.png ]; then
        icon_src="/tmp/furinar_icon.png"
    fi
    if [ -n "$icon_src" ]; then
        sudo install -Dm644 "$icon_src" "${ICON_DIR}/furinar.png"
        if command -v gtk-update-icon-cache &>/dev/null; then
            sudo gtk-update-icon-cache -f -t /usr/share/icons/hicolor 2>/dev/null || true
        fi
    fi

    # Desktop entry — tenta do repo, senão usa o gerado
    local desktop_src=""
    if [ -f "pkg/furinar.desktop" ]; then
        desktop_src="pkg/furinar.desktop"
    elif [ -f /tmp/furinar.desktop ]; then
        desktop_src="/tmp/furinar.desktop"
    fi
    if [ -n "$desktop_src" ]; then
        sudo install -Dm644 "$desktop_src" "${DESKTOP_DIR}/furinar.desktop"
        if command -v gtk-update-icon-cache &>/dev/null; then
            sudo gtk-update-icon-cache -f -t /usr/share/icons/hicolor 2>/dev/null || true
        fi
    fi

    # AppStream metadata
    local meta_src=""
    if [ -f "pkg/furinar.metainfo.xml" ]; then
        meta_src="pkg/furinar.metainfo.xml"
    elif [ -f /tmp/dev.furinar.Furinar.metainfo.xml ]; then
        meta_src="/tmp/dev.furinar.Furinar.metainfo.xml"
    fi
    if [ -n "$meta_src" ]; then
        sudo install -Dm644 "$meta_src" "${METADATA_DIR}/dev.furinar.Furinar.metainfo.xml"
    fi

    ok "Furinar instalado em ${INSTALL_DIR}/furinar"
}

# ---------------------------------------------------------------------------
# Desinstalar
# ---------------------------------------------------------------------------
uninstall() {
    info "Desinstalando furinar..."
    sudo rm -f "${INSTALL_DIR}/furinar"
    sudo rm -f "${ICON_DIR}/furinar.png"
    sudo rm -f "${DESKTOP_DIR}/furinar.desktop"
    sudo rm -f "${METADATA_DIR}/dev.furinar.Furinar.metainfo.xml"
    ok "Furinar desinstalado."
}

# ---------------------------------------------------------------------------
# Copiar binário local (para instalação a partir do source)
# ---------------------------------------------------------------------------
install_from_source() {
    local src="target/release/furinar"
    if [ ! -f "$src" ]; then
        err "Binário não encontrado em $src"
        err "Execute 'cargo build --release' primeiro."
        exit 1
    fi
    cp "$src" /tmp/furinar
    chmod +x /tmp/furinar
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
usage() {
    echo "Uso: $0 [OPÇÕES]"
    echo ""
    echo "Opções:"
    echo "  (sem args)     Instala furinar (baixa do GitHub Releases)"
    echo "  --deps         Apenas instala dependências"
    echo "  --from-source  Instala a partir do binário local (target/release/furinar)"
    echo "  --uninstall    Remove furinar"
    echo "  --help         Mostra esta mensagem"
}

main() {
    local mode="install"
    for arg in "$@"; do
        case "$arg" in
            --help|-h)    usage; exit 0 ;;
            --deps)       mode="deps" ;;
            --from-source) mode="source" ;;
            --uninstall)  mode="uninstall" ;;
            *)            err "Opção desconhecida: $arg"; usage; exit 1 ;;
        esac
    done

    detect_distro

    case "$mode" in
        deps)
            install_deps
            ;;
        source)
            install_deps
            generate_desktop_files
            install_from_source
            install_files
            ;;
        uninstall)
            uninstall
            ;;
        install)
            install_deps
            download_binary
            generate_desktop_files
            install_files
            ;;
    esac

    echo ""
    ok "Pronto! Execute 'furinar' para iniciar."
}

main "$@"
