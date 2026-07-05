# Dynamic API/ABI Harness Slice

Baseline: `d76593a2`

The compatibility target is FreeType C. Servo `rust-freetype` is only a
checklist reference. The harness must prove that `fontdone` can become a
drop-in FreeType replacement without allowing runtime FFI shortcuts in the
implementation.

## Goal

Build one generator-driven harness family that compares C FreeType and Rust
`fontdone` for every public surface by output type instead of writing one-off
tests for each function. Similar functions should feed the same runner and
comparator.

## Harness Layers

| Layer | Purpose | Runtime FFI status |
| --- | --- | --- |
| C oracle runner | Compile tiny C programs against pinned FreeType headers/source and emit structured JSON. | Allowed only in scripts/generators/tests. |
| Rust semantic runner | Call safe Rust APIs and emit the same JSON schema. | Pure Rust only. |
| Future C ABI runner | Compile tiny C programs against `fontdone` exported `FT_*` headers/library. | Test-only boundary for replacement validation. |
| Comparator | Compare normalized JSON by output kind. | Pure Rust or script, no production FFI. |
| Coverage gate | Fail when a public symbol lacks a mapped runner, explicit planned status, or accepted exclusion. | Pure Rust test. |

## Shared Output Schemas

| Schema | Applies to | Required exactness |
| --- | --- | --- |
| `error` | All fallible functions | FreeType error code or mapped public error class. |
| `scalar` | Version, counts, indices, flags, names | Exact integer/string bytes. |
| `vector` | Advances, kerning, transforms | Exact `FT_Pos`/`FT_Fixed` values. |
| `metrics` | `FT_Glyph_Metrics`, `FT_Size_Metrics` | Exact field values and units. |
| `bbox` | cbox/bbox APIs | Exact `xMin`, `yMin`, `xMax`, `yMax`. |
| `outline` | outline copy/transform/decompose | Exact points, tags, contours, flags. |
| `bitmap` | render and bitmap APIs | Exact rows, width, pitch, pixel mode, num grays, byte buffer. |
| `table_bytes` | SFNT/name/table APIs | Exact byte slice and length semantics. |
| `record_layout` | C ABI layer records | Exact size, alignment, field offsets, field C names. |
| `constant` | macros/enums/load flags | Exact numeric value. |

## Input Row Model

Every row should be deterministic and reusable across C and Rust:

```json
{
  "case_id": "font:dejavu_sans:face0:glyph36:ppem16:load_default",
  "font": "tests/data/DejaVuSans.ttf",
  "face_index": 0,
  "char_code": 65,
  "glyph_index": 36,
  "size": {"kind": "pixel", "width": 0, "height": 16},
  "load_flags": ["FT_LOAD_DEFAULT"],
  "render_mode": "FT_RENDER_MODE_NORMAL",
  "operation": "FT_Load_Glyph"
}
```

Rows live in generated fixture manifests. The generator must record:

- pinned FreeType version and commit/source directory;
- font file checksum;
- generator command line;
- platform architecture and pointer width for ABI layout tests;
- schema version;
- output hash for each oracle result.

## Generator Contract

The generator is part of the system, not an ad-hoc script.

Required commands:

```bash
make -C pillow-rs-freetype api-abi-audit
make -C pillow-rs-freetype generate-api-abi-fixtures
make -C pillow-rs-freetype test-api-abi
```

Planned files:

| Path | Role |
| --- | --- |
| `scripts/audit_api_abi.py` | Existing public symbol/record/status audit. |
| `scripts/generate_api_abi_fixtures.py` | Builds row manifests and C oracle JSON. |
| `scripts/c_api_oracle/*.c` | Small C runners grouped by output schema. |
| `tests/api_abi_parity.rs` | Rust comparator over generated rows. |
| `tests/api_abi_layout.rs` | C ABI constants/layout checks for exported C layer. |
| `tests/fixtures/api_abi/*.json` | Checked-in small manifests or regenerated target artifacts, depending on size policy. |
| `target/api-abi-fixtures/` | Large generated oracle outputs. |

Large output JSON should be regenerated under `target/` when absent. Small
manifests and font inputs should stay versioned so future contributors run the
same row space.

## C Oracle Runner Shape

Each C runner should accept a row JSON file or compact argv fields and print one
JSON object per row. The runner must not contain handwritten expected values.

Example groups:

- `metrics_runner.c`: size, load, glyph metrics, advances.
- `bitmap_runner.c`: render modes, bitmap conversion/copy/embolden.
- `outline_runner.c`: cbox/bbox/decompose/transform/copy.
- `charmap_runner.c`: char index, first/next char, charmap metadata.
- `sfnt_runner.c`: table info, raw table slices, name records.
- `layout_runner.c`: `sizeof`, `_Alignof`, and `offsetof` for C records.
- `constants_runner.c`: numeric values for macros/enums.

## Rust Runner Shape

Rust tests should use the same row files and emit the same normalized structs.
Avoid per-symbol assertion code where the output type is identical. The
comparator should report:

- `case_id`;
- operation;
- first differing field path;
- C value;
- Rust value;
- output schema;
- related public symbol.

This keeps failures bucketable by public endpoint while avoiding thousands of
bespoke assertions.

## Future C ABI Replacement Checks

When `fontdone` exposes a C library, add a second C compile/link path:

1. Compile the same C runners against FreeType headers and FreeType library.
2. Compile them against `fontdone` generated headers and `fontdone` library.
3. Run both against the same input rows.
4. Compare JSON exactly.

This validates function interface, import/link interface, usage lifecycle, and
output parity separately from the safe Rust facade.

## Coverage Gates

The coverage test should fail when:

- a FreeType public function is absent from the status map;
- a complete/partial symbol has no runner assignment;
- a C record mapped to the C ABI layer lacks size/offset coverage;
- a constant enum/macro used by public APIs lacks numeric coverage;
- a generated fixture family exists but is not executed by `make test-coverage`
  or `make test-api-abi`;
- a row count shrinks without an intentional manifest update.

## No-Cheat Guards

- The production crate must pass `no_runtime_ffi`.
- C code can live only under scripts/tests/oracle directories.
- Oracle JSON generation must include the FreeType version and generator hash.
- Comparator code must not special-case failing case IDs.
- Thresholds are not acceptable for integer, byte, pixel, metric, bbox, outline,
  constant, or layout parity.

## Open Implementation Risks

- `FT_FaceRec` and `FT_GlyphSlotRec` contain pointer graphs and mutable
  lifetimes that should not leak into the safe Rust API. They need a separate C
  ABI layer with stable ownership wrappers.
- Some public FreeType APIs expose callbacks (`FT_Outline_Decompose`, cache
  requester, list iterator). These need runner adapters that compare callback
  event streams.
- Layout tests are platform-specific. They must record pointer width and target
  triple and only compare C FreeType to `fontdone` C ABI for the same target.
- Color, cache, stroker, validation, and compressed stream APIs may require
  separate feature gates, but they should remain visible in the audit until a
  migration-safe decision is recorded.
