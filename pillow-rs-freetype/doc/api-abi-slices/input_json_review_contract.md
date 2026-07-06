# Public API Input JSON Review Contract

`tests/manifest.yaml` is the source of truth for public C FreeType subjects.
This phase creates one fixture input JSON file per manifest subject under:

```text
tests/fixtures/inputs/public-api/
```

The files are intentionally numerous. Consolidation can happen after the input
model is complete and reviewed.

## File Ownership

Each reviewed subject owns exactly one JSON file:

```text
tests/fixtures/inputs/public-api/<subject-id>.json
```

The filename is the manifest subject id with non `[A-Za-z0-9_.-]` characters
converted to `_`. Do not put multiple subjects in one file during this phase.

Workers own a contiguous 20-subject slice and must edit only those subject
files. They must not edit `tests/manifest.yaml`, existing legacy input files,
or unrelated review artifacts.

## Required Shape

Every file is a normal unified fixture input file:

```json
{
  "version": 1,
  "subject": "ftmm.FT_Get_MM_Var",
  "manifest_cases": ["success_mm_var", "invalid_face_error"],
  "cases": []
}
```

Each `cases[]` entry must include the fields the current runner already knows:

```json
{
  "case_id": "ftmm.FT_Get_MM_Var.success_mm_var",
  "subject": "ftmm.FT_Get_MM_Var",
  "case": "success_mm_var",
  "operation": "ftmm.get_mm_var",
  "schema": "api_result",
  "expect_error": false,
  "inputs": {
    "assets": {},
    "params": {}
  },
  "classifiers": ["kind:function", "header:ftmm"],
  "expectation": {
    "status": "ok",
    "output_shape": {},
    "compare": {
      "mode": "exact",
      "paths": []
    }
  }
}
```

The current Rust test parser ignores unknown fields such as `expectation`, so
this can model future comparison behavior before the runner supports it.

## Inputs

Do not store large blobs inline. Use references to shared assets:

```json
"inputs": {
  "assets": {
    "font": {
      "kind": "ref",
      "id": "fonts/variable/inter.ttf"
    }
  },
  "params": {
    "face_index": 0,
    "axis_coords": [0, 400, 1000]
  }
}
```

Use small inline scalar params freely. Binary/font/table inputs must reference
future shared fixture assets and explain the required asset properties in
`expectation.fixture_requirements` if the asset does not exist yet.

## Output And Comparison

Every case must state what runtime comparison should eventually do:

- `status`: `ok`, `error`, or `build_dependent`.
- `output_shape`: structural output, not generated C values.
- `compare.mode`: `exact`, `exact_error`, `layout`, `value`, `bytes`,
  `hash`, `nullness`, `identity_class`, or `unsupported_until_runner_added`.
- `compare.paths`: JSON paths or field names to compare exactly.
- `error`: expected public error classification for error cases.

Never commit generated C oracle output. Runtime must generate C FreeType output
and compare C vs Rust FFI vs C FFI vs Wasm FFI.

## Coverage Rules

Every manifest case must have at least one input case in the subject file.
It is valid for one input case to cover multiple closely coupled manifest cases
only if `covers_manifest_cases` names them explicitly.

For functions, include success and error cases where the public API can fail.
For records/types, include size, alignment, field order, pointer/nullability,
and ABI import cases. For constants/macros, include exact value/import cases and
runtime cases only where the value is observable through a public API.

Use representative runtime fixture cases for large constant families such as
`ttnameid.*`; do not invent per-language behavior when the public surface is
only a scalar value.

## Worker Validation

For each owned slice, workers must report:

- files created,
- first and last subject,
- manifest cases covered for every file,
- header/source areas checked,
- any missing shared fixture assets,
- JSON parse/count sanity result.

Broad tests are intentionally not required in this phase.
