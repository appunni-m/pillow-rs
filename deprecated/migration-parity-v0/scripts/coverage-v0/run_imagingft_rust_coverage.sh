#!/usr/bin/env bash
# Compatibility wrapper for the renamed Font public API parity corpus.

set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
exec "$script_dir/run_font_rust_coverage.sh"
