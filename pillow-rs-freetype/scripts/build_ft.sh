#!/bin/bash
# Build FreeType 2.14.3 from fetched oracle source and install to ~/.local
# Used by gen_ft_refs.c and trace_edges.c for reference generation.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
"${ROOT}/scripts/fetch_ft.sh"
cd "${ROOT}/freetype"
mkdir -p build && cd build
if [ ! -f CMakeCache.txt ]; then
  cmake .. \
    -DCMAKE_INSTALL_PREFIX="$HOME/.local" \
    -DCMAKE_BUILD_TYPE=Release \
    -DBUILD_SHARED_LIBS=ON \
    -DFT_DISABLE_ZLIB=ON \
    -DFT_DISABLE_PNG=ON \
    -DFT_DISABLE_BZIP2=ON \
    -DFT_DISABLE_BROTLI=ON \
    -DFT_DISABLE_HARFBUZZ=ON
fi
if [ -n "${FONTDONE_BUILD_JOBS:-}" ]; then
  build_jobs="${FONTDONE_BUILD_JOBS}"
elif command -v nproc >/dev/null 2>&1; then
  build_jobs="$(nproc)"
elif command -v sysctl >/dev/null 2>&1; then
  build_jobs="$(sysctl -n hw.ncpu)"
else
  build_jobs="$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 1)"
fi
cmake --build . -j"${build_jobs}"
echo "FreeType 2.14.3 built at ${ROOT}/freetype/build"
