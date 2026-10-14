#!/usr/bin/env bash
# Cross-platform environment bootstrap script for servo-browse
# Supports:
#   - Ubuntu / Debian Linux (x86_64 & aarch64, including WSL2 and Hyper-V VMs)
#   - Windows MSYS2 / Git Bash (for Buzz / Goose subshell execution)
set -euo pipefail

echo "==> Detecting platform and architecture..."
OS_UNAME="$(uname -s)"
ARCH_UNAME="$(uname -m)"
echo "    OS:   ${OS_UNAME}"
echo "    Arch: ${ARCH_UNAME}"

install_rustupUnix() {
  if [ -f "$HOME/.cargo/env" ]; then
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
  fi
  if ! command -v cargo >/dev/null 2>&1; then
    echo "==> Installing Rust toolchain via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
  else
    echo "==> Rust toolchain already installed: $(rustc --version)"
  fi
  rustup component add clippy rustfmt
}

case "${OS_UNAME}" in
  Linux*)
    if command -v apt-get >/dev/null 2>&1; then
      echo "==> Installing Ubuntu/Debian system build & runtime dependencies (requires sudo)..."
      sudo apt-get update
      sudo apt-get install -y \
        build-essential \
        pkg-config \
        cmake \
        ninja-build \
        clang \
        libclang-dev \
        llvm-dev \
        lld \
        m4 \
        autoconf \
        automake \
        libtool \
        curl \
        git \
        jq \
        libssl-dev \
        libglib2.0-dev \
        libfontconfig1-dev \
        libfreetype-dev \
        libharfbuzz-dev \
        libegl1-mesa-dev \
        libgles2-mesa-dev \
        libgbm-dev \
        libdrm-dev \
        libunwind-dev \
        libx11-dev \
        libxcb1-dev \
        libxcb-render0-dev \
        libxcb-shape0-dev \
        libxcb-xfixes0-dev \
        libxkbcommon-dev \
        libwayland-dev \
        libgstreamer1.0-dev \
        libgstreamer-plugins-base1.0-dev \
        xvfb \
        fonts-dejavu-core
    else
      echo "WARN: Non-apt Linux distribution detected. Ensure clang, cmake, ninja, EGL/GLES, GBM, fontconfig, freetype, harfbuzz, and openssl dev headers are installed." >&2
    fi

    install_rustupUnix
    ;;

  MINGW*|MSYS*|CYGWIN*)
    echo "==> Windows MSYS2 / Git Bash environment detected."
    echo "    Note: Upstream Servo targets Windows MSVC (x86_64-pc-windows-msvc or aarch64-pc-windows-msvc)."
    if ! command -v cargo >/dev/null 2>&1; then
      echo "    Please install rustup-init.exe for Windows MSVC from https://rustup.rs"
      echo "    or use WSL bridge mode via scripts/run-headless.sh."
    else
      echo "    Found cargo: $(cargo --version)"
    fi
    ;;

  Darwin*)
    echo "==> macOS detected."
    if command -v brew >/dev/null 2>&1; then
      brew install cmake ninja pkg-config llvm harfbuzz fontconfig freetype openssl@3 jq
    fi
    install_rustupUnix
    ;;

  *)
    echo "ERROR: Unsupported OS: ${OS_UNAME}" >&2
    exit 1
    ;;
esac

echo "==> Environment bootstrap complete!"
