#!/usr/bin/env bash
set -euo pipefail

# Ensure standard font directories are located
export FONTCONFIG_PATH="${FONTCONFIG_PATH:-/etc/fonts}"

# Locate servo-browse binary
BINARY="${SERVO_BROWSE_BIN:-cargo run --quiet --}"

# Check for display server: if DISPLAY is missing and xvfb-run is available, use it as fallback wrapper
if [ -z "${DISPLAY:-}" ]; then
  if command -v xvfb-run >/dev/null 2>&1; then
    exec xvfb-run --auto-servernum --server-args="-screen 0 1280x800x24" $BINARY "$@"
  else
    # Run directly using software offscreen rendering
    exec $BINARY "$@"
  fi
else
  exec $BINARY "$@"
fi
