# Unified Fixture Input Format

The unified parity harness has one active source of executable definitions:

```text
tests/fixtures/inputs/public-api/*.json
```

`tests/manifest.yaml` is the public coverage contract. Each JSON case names one
manifest `subject` and `case`; `covers_manifest_cases` may name additional cases
when one public operation intentionally proves more than one contract entry.

The harness executes only inputs written in JSON. It does not discover fonts,
enumerate glyphs, expand axes, or construct Cartesian products.

## Two Valid Case Shapes

A logical case uses either one direct input or a list of explicit grouped input
variants. It must not mix the two shapes.

### Direct Input

Use a direct input when one concrete combination is sufficient:

```json
{
  "case_id": "freetype.FT_Get_Char_Index.latin_capital_a",
  "subject": "freetype.FT_Get_Char_Index",
  "case": "latin_capital_a",
  "operation": "get_char_index",
  "schema": "api_result",
  "expect_error": false,
  "inputs": {
    "assets": {
      "font": { "kind": "ref", "id": "fonts/autohint/basic-latin.ttf" }
    },
    "params": {
      "face_index": 0,
      "char_code": 65
    }
  }
}
```

### Explicit Grouped Variants

Use `inputs.variants` when one logical manifest case needs several deliberate
combinations:

```json
{
  "case_id": "freetype.FT_LOAD_COMPUTE_METRICS.compute_metrics_load_behavior",
  "subject": "freetype.FT_LOAD_COMPUTE_METRICS",
  "case": "compute_metrics_load_behavior",
  "operation": "load_glyph",
  "schema": "api_result",
  "expect_error": false,
  "inputs": {
    "variants": [
      {
        "id": "mono-device-width",
        "assets": {
          "font": { "kind": "ref", "id": "fonts/metrics/hdmx_observable.ttf" }
        },
        "params": {
          "face_index": 0,
          "pixel_size": 20,
          "load_flags": ["FT_LOAD_TARGET_MONO"],
          "glyph_index": { "from_char_code": 65 }
        },
        "coverage": ["tt/hdmx:mono-device-width-lookup"]
      },
      {
        "id": "mono-compute-metrics",
        "assets": {
          "font": { "kind": "ref", "id": "fonts/metrics/hdmx_observable.ttf" }
        },
        "params": {
          "face_index": 0,
          "pixel_size": 20,
          "load_flags": [
            "FT_LOAD_TARGET_MONO",
            "FT_LOAD_COMPUTE_METRICS"
          ],
          "glyph_index": { "from_char_code": 65 }
        },
        "coverage": ["tt/hdmx:mono-compute-metrics-suppression"]
      }
    ]
  }
}
```

Every grouped variant requires:

- A non-empty `id` unique within its logical case.
- Its complete `assets` and `params`.
- At least one non-empty `coverage` intent explaining why the combination
  exists.
- Optional `expect_error` only when that variant differs from the case default.

The runtime ID is `<case_id>@<variant-id>`. Variants are concrete rows, not axes.
Two fonts and three sizes therefore require only the combinations explicitly
listed, not six generated combinations.

## Coverage Intent

A coverage intent names the behavior, branch, condition, table, glyph topology,
or error path that justifies the concrete input. Prefer stable identifiers such
as:

```text
tt/hdmx:mono-device-width-lookup
tt/glyf:recursive-composite-no-hinting
autohint/latin:blue-zone-overshoot
render/lcd:negative-pitch-copy
font/cmap:format-12-supplementary-hit
```

Coverage intent is reviewed against llvm-cov output and the font inventory. It
does not change execution or comparison behavior.

## Assets

Use tracked fixture assets for fonts and binary data:

```json
{ "kind": "ref", "id": "fonts/autohint/basic-latin.ttf" }
```

Direct file assets and inline bytes remain available for cases that own those
forms:

```json
{ "kind": "file", "path": "fonts/autohint/basic-latin.ttf" }
```

```json
{ "kind": "inline_bytes", "encoding": "hex", "value": "00010000" }
```

Do not add `font_folder`. Runtime folder discovery is forbidden. Do not add new
references to `tests/fixtures/deprecated/`; replace the required property with a
focused active fixture.

## Parameters

Parameters are operation data interpreted by the existing runner and public
backend adapters. Arrays are values unless the operation explicitly defines an
array-valued parameter. They never become expansion axes.

Some operations already use a parameter named `variants` for operation-specific
data, such as charmap lifecycle rows. That field is `inputs.params.variants` and
is unrelated to grouped `inputs.variants`. Preserve existing parser semantics;
do not generalize operation parameters into harness expansion.

## Output And Comparison

Expected backend outputs are not committed. For every concrete input, the
runner:

1. Resolves and hashes fixture bytes.
2. Executes pinned C FreeType to obtain the oracle output.
3. Executes the pure Rust FFI path.
4. Executes the C ABI wrapper around the Rust core.
5. Executes the WASM ABI wrapper around the Rust core.
6. Compares status, error code, output shape, scalar fields, geometry, metrics,
   and bytes according to the operation's existing exact comparison.

Changing a font is valid only when the new C output and all three Rust-backed
outputs are interchangeable for that input. Do not edit expectations or
comparison rules to accept a mismatch.

## Forbidden Legacy Shapes

The Rust and Python validators reject:

- Top-level `matrix_cases`.
- `inputs.variability`.
- `font_folder` assets.
- A logical case that mixes `inputs.variants` with direct `inputs.assets` or
  `inputs.params`.
- Empty or duplicate variant IDs.
- Variants without coverage intent.

Environment limits and operation filters are diagnostics only. They must not
change the authoritative concrete input set.

## Maintained Commands

Run from `pillow-rs-freetype/`:

```bash
make api-abi-check
make test-unified-fixtures
make test-unified-coverage
make test-unified-condition-coverage
```

`make test-unified-coverage` records stable function, line, and region coverage.
`make test-unified-condition-coverage` uses nightly Rust condition
instrumentation, which also instruments branch outcomes. Completion totals are
calculated from `pillow-rs-freetype/src/**`; C and WASM wrappers still execute in
the parity run.

## Worker Checklist

1. Confirm `subject` and `case` exist in `tests/manifest.yaml`.
2. Choose one direct input or explicit grouped variants.
3. Name the exact font property and glyph behavior needed for each variant.
4. Use the minimum deliberate sizes, flags, modes, transforms, and coordinates.
5. Add controls only when they prove a distinct branch or condition outcome.
6. Keep parser-specific operation parameters consistent with existing cases.
7. Run the API/ABI validator and exact parity before measuring coverage.
8. Keep a variant only when it fulfills its named obligation without reducing
   existing structural coverage.

See `doc/FONT_FIXTURE_COVERAGE_PLAN.md` for corpus ownership, structural coverage
requirements, phase gates, and the progress ledger.
