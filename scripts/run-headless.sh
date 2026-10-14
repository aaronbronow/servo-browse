#!/usr/bin/env bash
# Cross-platform headless execution wrapper for servo-browse
# Supports:
#   - Linux (aarch64 / x86_64) native headless execution
#   - Windows MSYS2 / Git Bash (Buzz / Goose subshells) via native .exe or WSL bridge
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Prevent MSYS2 / Git Bash from mangling CSS selectors or URL arguments starting with '/'
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*)
    export MSYS2_ARG_CONV_EXCL="*"
    ;;
esac

# Source cargo environment if present
if [ -f "$HOME/.cargo/env" ]; then
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi

# Ensure standard font directories are located on Linux
if [[ "$(uname -s)" == Linux* ]]; then
  export FONTCONFIG_PATH="${FONTCONFIG_PATH:-/etc/fonts}"
fi

# Resolve servo-browse execution method:
# 1. Explicit SERVO_BROWSE_BIN override
# 2. Release binary in repo target/release/servo-browse(.exe)
# 3. Installed binary on PATH
# 4. On MSYS2 without native .exe: delegate to WSL if wsl.exe is available
# 5. Fallback to cargo run --quiet --
if [ -n "${SERVO_BROWSE_BIN:-}" ]; then
  exec ${SERVO_BROWSE_BIN} "$@"
elif [ -x "${REPO_ROOT}/target/release/servo-browse.exe" ]; then
  exec "${REPO_ROOT}/target/release/servo-browse.exe" "$@"
elif [ -x "${REPO_ROOT}/target/release/servo-browse" ]; then
  exec "${REPO_ROOT}/target/release/servo-browse" "$@"
elif command -v servo-browse >/dev/null 2>&1; then
  exec servo-browse "$@"
elif [[ "$(uname -s)" =~ ^(MINGW|MSYS|CYGWIN) ]] && command -v wsl.exe >/dev/null 2>&1; then
  WSL_REPO_PATH="$(wsl.exe wslpath -a "${REPO_ROOT}" 2>/dev/null | tr -d '\r')"
  exec wsl.exe -- bash -lc "cd '${WSL_REPO_PATH}' && ./scripts/run-headless.sh $(printf '%q ' "$@")"
else
  exec cargo run --manifest-path "${REPO_ROOT}/Cargo.toml" --quiet -- "$@"
fi
