# References and Fixtures

## FT Fixture Matrix

File: `tests/fixtures/coverage_matrix_ft.json`  
Generator: `scripts/gen_ft_refs.c` → `scripts/gen_ft_matrix.py`  
Reference: FreeType 2.14.3 C library, built from vendored source with `FT_LOAD_RENDER | FT_LOAD_FORCE_AUTOHINT`  
Fonts: 29 fonts under `tests/fixtures/input/fonts_autohint/`  
Current pass rate: 27,686/27,695 (99.97%)

## PIL Fixture Matrix

File: `tests/fixtures/coverage_matrix.json`  
Generator: `scripts/generate_font_refs.py`  
Reference: PIL 12.2.0 `ImageFont.getmask()`  

## Running Tests

```bash
# FT fixtures (autohint comparison)
cargo test -p pillow-rs-freetype test_font_coverage_matrix_freetype

# PIL fixtures
cargo test -p pillow-rs-freetype test_font_coverage_matrix_pil
```

## Regenerating Fixtures

Build FreeType from vendored source (one-time):
```bash
cd pillow-rs-freetype && bash scripts/build_ft.sh
```

Regenerate FT matrix:
```bash
gcc -o /tmp/gen_ft_refs pillow-rs-freetype/scripts/gen_ft_refs.c \
  -I$HOME/.local/include/freetype2 -L$HOME/.local/lib -lfreetype
python pillow-rs-freetype/scripts/gen_ft_matrix.py
```

Regenerate PIL matrix:
```bash
python pillow-rs-freetype/scripts/generate_font_refs.py
```

## Tracing a Failing Glyph

```bash
# C reference
gcc -o /tmp/trace pillow-rs-freetype/scripts/trace_one_glyph.c \
  -I$HOME/.local/include/freetype2 -L$HOME/.local/lib -lfreetype
LD_LIBRARY_PATH=$HOME/.local/lib /tmp/trace <font.ttf> <size_pt> <codepoint>

# Rust
cargo run --example <name> --manifest-path pillow-rs-freetype/Cargo.toml
```
