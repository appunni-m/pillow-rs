# References and Fixtures

Fixture generation is part of the maintained harness. See
`doc/GENERATOR_SYSTEM.md` for the generator contract, standard reproduction
flow, and fixture update checklist.

## FreeType Fixture Matrices

Reference fixtures in this crate are generated from pinned FreeType C.  The
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
make test-parity
```

## Regenerating Fixtures

Standard flow:

```bash
make fixtures
```

`make oracle-fetch` downloads and verifies FreeType 2.14.3 into ignored
`freetype/`. Generated matrices under `tests/fixtures/*.json` and raw bytes
under `tests/fixtures/outputs/` are ignored local artifacts. Keep tracked
fixture inputs limited to fonts under `tests/fixtures/input/`.

## Tracing a Failing Glyph

```bash
make fixture-ref-bin
LD_LIBRARY_PATH=freetype/build /tmp/gen_refs_v4 --json \
  tests/fixtures/input/fonts_autohint/DejaVuSans.ttf 0041 20 force_autohint
```
