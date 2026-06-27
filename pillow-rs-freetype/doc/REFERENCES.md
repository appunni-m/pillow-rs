# How to Regenerate References

Two independent reference matrices, both pinned to FreeType 2.14.3:

| Matrix file | Generator | Output |
|---|---|---|
| `tests/fixtures/coverage_matrix.json` | PIL 12.2.0 | 1910 rows (PIL-padded mask + bbox) |
| `tests/fixtures/coverage_matrix_ft.json` | FreeType C | 1910 rows (raw FT bitmap + bbox) |

## Prerequisites

FreeType 2.14.3 must be built from the vendored source and installed to `~/.local`.  
This is a one-time setup:

```bash
cd pillow-rs-freetype
bash scripts/build_ft.sh
```

(Requires cmake, gcc, make. No other dependencies.)

## Regenerate PIL Matrix

```bash
python pillow-rs-freetype/scripts/generate_font_refs.py
```

Requires: `pip install Pillow>=12.2.0` (PIL bundles FreeType 2.14.3).  
Input: `tests/fixtures/input/fonts_autohint/*.ttf`  
Output: `tests/fixtures/coverage_matrix.json`

## Regenerate FreeType Matrix

```bash
# Build the reference generator binary
gcc -o /tmp/gen_ft_refs pillow-rs-freetype/scripts/gen_ft_refs.c \
  -I$HOME/.local/include/freetype2 -L$HOME/.local/lib -lfreetype \
  -Wl,-rpath,$HOME/.local/lib

# Generate the matrix
python pillow-rs-freetype/scripts/gen_ft_matrix.py
```

Input: `tests/fixtures/input/fonts_autohint/*.ttf`  
Output: `tests/fixtures/coverage_matrix_ft.json`

## Run Tests

```bash
cargo test -p pillow-rs-freetype test_font_coverage_matrix \
  -- --nocapture
```

Two test functions run:
- `test_font_coverage_matrix_pil` — `BitmapBackend::PIL` against `coverage_matrix.json`
- `test_font_coverage_matrix_freetype` — `BitmapBackend::FreeType` against `coverage_matrix_ft.json`

## Trace a Failing Glyph

```bash
# C reference (FreeType 2.14.3)
gcc -o /tmp/trace_edges pillow-rs-freetype/scripts/trace_edges.c \
  -I pillow-rs-freetype/freetype/include \
  -I pillow-rs-freetype/freetype/src/autofit \
  -L $HOME/.local/lib -lfreetype -Wl,-rpath,$HOME/.local/lib
/tmp/trace_edges <font.ttf> <size_pt> <char>

# Rust output (both backends)
cargo run --example dump_all_masks -- <font.ttf> <size> pil
cargo run --example dump_all_masks -- <font.ttf> <size> ft
cargo run --example trace_raster -- <font.ttf> <size> <char>
cargo run --example dump_outline -- <font.ttf> <size> <char>
```

## Version Audit

```bash
# PIL
python3 -c 'from PIL import _imagingft; print(_imagingft.freetype2_version)'

# Locally built FreeType
LD_LIBRARY_PATH=$HOME/.local/lib python3 -c '
import ctypes; l=ctypes.CDLL("libfreetype.so"); lib=ctypes.c_void_p()
l.FT_Init_FreeType(ctypes.byref(lib))
m,mi,p=ctypes.c_int(),ctypes.c_int(),ctypes.c_int()
l.FT_Library_Version(lib,ctypes.byref(m),ctypes.byref(mi),ctypes.byref(p))
print(f"{m.value}.{mi.value}.{p.value}")
l.FT_Done_FreeType(lib)'

# Vendored C source
head -3 pillow-rs-freetype/freetype/README
```
