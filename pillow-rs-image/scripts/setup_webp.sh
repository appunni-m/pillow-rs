#!/usr/bin/env bash
# Setup libwebp vendor files for native development.
# Requires: libwebp-dev (apt package) or equivalent on your system.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
VENDOR_DIR="$SCRIPT_DIR/../vendor"

mkdir -p "$VENDOR_DIR/lib" "$VENDOR_DIR/include/webp"

# Copy headers from libwebp-dev (or from system include dir)
for h in decode.h types.h; do
    cp "/usr/include/webp/$h" "$VENDOR_DIR/include/webp/" 2>/dev/null ||
        echo "WARNING: header $h not found. Install libwebp-dev."
done

# Copy .so symlink from libwebp-dev
cp -a "/usr/lib/x86_64-linux-gnu/libwebp.so" "$VENDOR_DIR/lib/" 2>/dev/null ||
    echo "WARNING: libwebp.so not found in /usr/lib/x86_64-linux-gnu."

# Ensure .so.7 is present (from runtime package)
cp -a "/usr/lib/x86_64-linux-gnu/libwebp.so.7" "$VENDOR_DIR/lib/" 2>/dev/null ||
    echo "WARNING: libwebp.so.7 not found. Install libwebp7."

echo "WebP vendor files set up in $VENDOR_DIR"
