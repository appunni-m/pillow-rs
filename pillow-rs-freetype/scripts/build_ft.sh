#!/bin/bash
# Build FreeType 2.14.3 from fetched oracle source and install to ~/.local
# Used by gen_ft_refs.c and trace_edges.c for reference generation.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
"${ROOT}/scripts/fetch_ft.sh"
cd "${ROOT}/freetype"
rm -rf build && mkdir build && cd build
cmake .. \
  -DCMAKE_INSTALL_PREFIX="$HOME/.local" \
  -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_SHARED_LIBS=ON \
  -DFT_DISABLE_ZLIB=ON \
  -DFT_DISABLE_PNG=ON \
  -DFT_DISABLE_BZIP2=ON \
  -DFT_DISABLE_BROTLI=ON \
  -DFT_DISABLE_HARFBUZZ=ON
cmake --build . -j$(nproc)
echo "FreeType 2.14.3 built at ${ROOT}/freetype/build"
