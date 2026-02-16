#!/usr/bin/env bash
set -euo pipefail

# Harbor Development Setup Script (Linux / macOS)
# Installs all dependencies needed to build Harbor from source.
# Supports: Ubuntu/Debian, Fedora, Arch, macOS
#
# Windows users: run setup-dev.bat or setup-dev.ps1 instead.

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info()  { echo -e "${GREEN}[+]${NC} $1"; }
warn()  { echo -e "${YELLOW}[!]${NC} $1"; }
error() { echo -e "${RED}[x]${NC} $1"; }

# Detect OS
detect_os() {
    if [[ "$OSTYPE" == "darwin"* ]]; then
        echo "macos"
    elif [[ -f /etc/os-release ]]; then
        . /etc/os-release
        case "$ID" in
            ubuntu|debian|pop|linuxmint) echo "debian" ;;
            fedora|rhel|centos|rocky|alma) echo "fedora" ;;
            arch|manjaro|endeavouros) echo "arch" ;;
            *) echo "unknown" ;;
        esac
    else
        echo "unknown"
    fi
}

OS=$(detect_os)
info "Detected OS: $OS"

# ── System packages (Tauri + libp2p build deps) ──────────────────────

install_system_deps() {
    case "$OS" in
        debian)
            info "Installing system packages (apt)..."
            sudo apt-get update
            sudo apt-get install -y \
                build-essential \
                curl \
                wget \
                pkg-config \
                libssl-dev \
                libgtk-3-dev \
                libwebkit2gtk-4.1-dev \
                libayatana-appindicator3-dev \
                librsvg2-dev \
                patchelf \
                libsoup-3.0-dev \
                libjavascriptcoregtk-4.1-dev
            ;;
        fedora)
            info "Installing system packages (dnf)..."
            sudo dnf install -y \
                gcc gcc-c++ make \
                curl wget \
                pkg-config \
                openssl-devel \
                gtk3-devel \
                webkit2gtk4.1-devel \
                libayatana-appindicator-gtk3-devel \
                librsvg2-devel \
                patchelf \
                libsoup3-devel \
                javascriptcoregtk4.1-devel
            ;;
        arch)
            info "Installing system packages (pacman)..."
            sudo pacman -Syu --noconfirm \
                base-devel \
                curl wget \
                openssl \
                gtk3 \
                webkit2gtk-4.1 \
                libayatana-appindicator \
                librsvg \
                patchelf
            ;;
        macos)
            info "macOS detected — Tauri uses native WebView, minimal deps needed."
            if ! command -v brew &>/dev/null; then
                warn "Homebrew not found. Install from https://brew.sh"
            fi
            ;;
        *)
            error "Unsupported OS. Install Tauri deps manually:"
            error "  https://v2.tauri.app/start/prerequisites/"
            exit 1
            ;;
    esac
}

# ── Rust ──────────────────────────────────────────────────────────────

install_rust() {
    if command -v rustc &>/dev/null; then
        info "Rust already installed: $(rustc --version)"
    else
        info "Installing Rust..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "${CARGO_HOME:-$HOME/.cargo}/env"
        info "Rust installed: $(rustc --version)"
    fi
}

# ── Node.js ───────────────────────────────────────────────────────────

install_node() {
    if command -v node &>/dev/null; then
        info "Node.js already installed: $(node --version)"
    else
        info "Installing Node.js via nvm..."
        curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh | bash
        export NVM_DIR="${HOME}/.nvm"
        [ -s "$NVM_DIR/nvm.sh" ] && . "$NVM_DIR/nvm.sh"
        nvm install --lts
        info "Node.js installed: $(node --version)"
    fi
}

# ── NPM dependencies ─────────────────────────────────────────────────

install_npm_deps() {
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

    if [[ -f "$PROJECT_DIR/package.json" ]]; then
        info "Installing npm dependencies..."
        cd "$PROJECT_DIR"
        npm install
    else
        warn "package.json not found at $PROJECT_DIR — skipping npm install"
    fi
}

# ── Run it ────────────────────────────────────────────────────────────

echo ""
echo "=================================="
echo "  Harbor Development Setup"
echo "=================================="
echo ""

install_system_deps
install_rust
install_node
install_npm_deps

echo ""
info "Setup complete!"
echo ""
echo "  To build the app:        npm run tauri build"
echo "  To run in dev mode:      npm run tauri dev"
echo "  To build relay server:   cd relay-server && cargo build --release"
echo ""
