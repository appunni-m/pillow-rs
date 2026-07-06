# Unified Fixture Input Format

The unified parity harness has one active source of test definitions:

```text
tests/fixtures/inputs/public-api/*.json
```

`tests/manifest.yaml` is the coverage source of truth. It enumerates the public
C API subjects and the required case IDs for each subject. Input JSON files are
the execution source of truth. Every input case must reference a manifest
`subject` and `case`, and may list extra `covers_manifest_cases` when one
aggregate input intentionally covers more than one manifest case.

The runner expands only `cases[].inputs.variability`. Top-level
`matrix_cases`, `_matrix` operation/schema names, `schema: "scalar"`, and
`load_flags_matrix` are rejected.

## Case Shape

Each case keeps fixed operation arguments under `inputs.params` and shared files
under `inputs.assets`.

```json
{
  "case_id": "freetype.FT_Load_Char.matrix_load",
  "subject": "freetype.FT_Load_Char",
  "case": "matrix_load",
  "operation": "freetype.load_char",
  "schema": "api_result",
  "expect_error": false,
  "inputs": {
    "assets": {
      "font_folder": {
        "kind": "file",
        "role": "font_folder",
        "path": "input/fonts"
      }
    },
    "params": {
      "face_index": 0,
      "sizes": [10, 20],
      "char_codes": [65, 103],
      "load_flag_sets": [
        ["FT_LOAD_DEFAULT"],
        ["FT_LOAD_RENDER"]
      ]
    },
    "variability": {
      "axes": ["fonts", "sizes", "codepoints", "load_flags"]
    }
  }
}
```

## Variability Axes

Only common coverage dimensions are variability axes:

- `fonts`: expands a folder of font files. Use `font_folder` in assets or
  `inputs.variability.fonts_folder`; otherwise the runner uses `input/fonts`.
- `sizes`: expands `sizes` or `pixel_sizes` into `pixel_size`.
- `codepoints`: expands `codepoints` or `char_codes` into `char_code`.
- `glyph_indices`: expands `glyph_indices` into `glyph_index`.
- `load_flags`: expands `load_flag_sets`, numeric `load_flags`, and combines
  optional `target_modes`.
- `render_modes`: expands `render_modes` into `render_mode`.

Do not materialize one JSON case per font/size/codepoint combination. Put the
fixed parameters in one case and let the runner expand the applicable axes.

## Assets

Large blobs, binaries, fonts, and shared models belong under
`tests/fixtures/input` or another fixture asset folder. JSON should reference
them by path or shared asset ID; it should not embed large byte arrays.

Supported direct runtime assets are file assets and hex inline bytes. Missing
future assets may remain as model-only cases, but they will not count as runtime
parity until the file exists.

## Output And Comparison

Expected outputs are not committed. At runtime the runner:

1. Expands input cases in memory.
2. Runs the C FreeType oracle and caches the resulting JSONL blob by a SHA-256
   key derived from all expanded inputs.
3. Runs the Rust FFI, C ABI, and WASM ABI paths.
4. Compares status, error code, output shape, and actual output values.

Use `expect_error: true` only when C FreeType is expected to return an error.
Error cases still compare the error status and error code across all backends.

Schema names should be semantic, such as `constant`, `value`, `glyph_slot`,
`size_metrics`, `record_layout`, `face_open`, `set_status`, or `error`.
Do not use migration names such as `scalar` or `*_matrix`.

## Worker Checklist

For each assigned JSON file:

1. Confirm every `cases[].case` exists in `tests/manifest.yaml`.
2. Add `inputs.variability.axes` when params contain common aggregate fields.
3. Keep fixed non-axis parameters in `inputs.params`.
4. Prefer one aggregate case per manifest case over copied per-font rows.
5. Move references to large blobs/fonts into fixture assets, not inline JSON.
6. Do not add or commit expected output.
7. Keep `case_id`, `subject`, and `case` stable unless merging duplicates.
8. Run a JSON parse check for the files you edited.
