# References and Fixtures

Fixture generation is part of the maintained harness. See
`doc/GENERATOR_SYSTEM.md` for the generator contract, standard reproduction
flow, and fixture update checklist.

## FreeType Fixture Matrices

Reference fixtures in this crate are generated from vendored FreeType C.  The
active families are named by FreeType path and flags:

- `native_tt_default_matrix.json`: `FT_LOAD_RENDER`
- `force_autohint_matrix.json`: `FT_LOAD_RENDER | FT_LOAD_FORCE_AUTOHINT`
- `no_hinting_matrix.json`: `FT_LOAD_RENDER | FT_LOAD_NO_HINTING`
- `metrics_only_matrix.json`: `FT_Load_Glyph` without render
- `outline_cbox_matrix.json`: outline cbox/bbox after load
- `render_mono_matrix.json`: `FT_RENDER_MODE_MONO`
- `render_lcd_matrix.json`: `FT_RENDER_MODE_LCD`

## Running Tests

```bash
cargo test -p pillow-rs-freetype --test coverage_matrix_tests -- --nocapture
```

## Regenerating Fixtures

Standard flow:

```bash
cd pillow-rs-freetype
bash scripts/build_ft.sh
python3 scripts/build_ft_fixture.py --family force_autohint --build-ref-bin
python3 scripts/build_ft_fixture.py --family native_tt_default
python3 scripts/build_ft_fixture.py --family no_hinting --small
python3 scripts/build_ft_fixture.py --family metrics_only --small
python3 scripts/build_ft_fixture.py --family outline_cbox --small
python3 scripts/build_ft_fixture.py --family render_mono --small
python3 scripts/build_ft_fixture.py --family render_lcd --small
python3 scripts/build_render_mode_fixture.py
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
