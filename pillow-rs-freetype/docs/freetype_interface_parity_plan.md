# FreeType Interface Parity Plan

Goal: every in-scope public FreeType endpoint family must have an executable Rust parity gate against FreeType C oracle data.

The crate does not need to expose C ABI names. It does need a truthful mapping from Rust behavior to FreeType public endpoints, and the harness must make it impossible to claim endpoint parity without exact fixture-backed tests.

## Completion Rule

An endpoint path is complete only when all are true:

1. The Rust behavior exists.
2. The FreeType endpoint family is mapped in `tests/fixtures/interface_map.json`.
3. C-oracle fixtures or scalar oracle tests exist for the endpoint behavior.
4. The default test suite executes the parity gate.
5. The gate compares exact values, bytes, pixels, metadata, and errors required by that endpoint.
6. The gate fails on missing fixtures, unsupported operations, missing raw bytes, or mismatches.
7. The interface report truthfully shows `passing == total`.

If any item is missing, the endpoint is `partial` or `planned`, not complete.

## Status Words

- `complete`: Rust API exists and executable exact parity gates pass.
- `partial`: Rust API/path exists, but parity coverage is incomplete, thresholded, or not yet executable.
- `planned`: accepted scope, not implemented yet.
- `out_of_scope`: intentionally excluded with a reason.

## Global Rules

1. FreeType C is the source of reference data, not runtime behavior.
2. Pillow compatibility belongs above this crate, in `pillow-rs`.
3. Fixture families describe FreeType paths and flags.
4. SHA-only render tests are insufficient for exact gates; raw bytes and metadata are required.
5. Threshold baselines are debt.
6. Present-but-unexecuted fixture families are debt.
7. Broad matrices cannot be replaced by smaller smoke tests.

## Interface Paths

| Path | FreeType surface | Required parity |
|---|---|---|
| `library/lifecycle` | library init, shutdown, version, modules | error codes, version, module visibility |
| `face/open` | file/memory/stream face creation | error codes, face count, face index behavior |
| `face/metadata` | family/style names, flags, metrics fields | scalar field parity |
| `size/select` | char size, pixel size, strike selection | ppem, x/y scale, size metrics |
| `charmap` | charmap selection and codepoint iteration | glyph index parity |
| `glyph/load` | load glyph/char with load flags | slot errors, metrics, outline points |
| `glyph/render` | render current slot | bitmap mode, dimensions, pitch, placement, pixels |
| `glyph/metrics` | slot metrics and advance APIs | 26.6 and 16.16 numeric parity |
| `glyph/object` | get/copy/transform glyph objects | bbox, outline, bitmap parity |
| `bitmap` | bitmap copy/convert/embolden/blend | buffer and metadata parity |
| `outline` | decompose, transform, bbox/cbox, embolden | geometry parity |
| `raster/grays` | anti-aliased scan conversion | pixel parity |
| `raster/mono` | monochrome scan conversion | bit parity |
| `raster/lcd` | LCD and LCD_V rendering/filtering | subpixel buffer parity |
| `sfnt/tables` | raw SFNT table access | byte and parsed-field parity |
| `truetype/tables` | TrueType table structs | parsed field parity |
| `truetype/bytecode` | fpgm/prep/glyph VM | hinted outline, CVT, storage, pixels |
| `truetype/variations` | MM/var axes and instance coords | metrics, outlines, named instances |
| `cff` | CFF/CFF2 glyph loading | outline, metrics, bitmap parity |
| `type1/cid` | Type 1/CID APIs | metadata, outline, metrics parity |
| `bitmap/fonts` | BDF, PCF, WinFNT, embedded strikes | bitmap and metrics parity |
| `color/fonts` | COLR/CPAL/SVG/sbix/CBDT | layer, color, bitmap parity |
| `stroker` | stroke construction and export | outline parity |
| `synthesis` | embolden/oblique synthesis | outline and bitmap parity |
| `advances` | fast advance queries | numeric parity |
| `kerning` | kerning and track kerning | numeric parity |
| `properties/modules` | module properties and renderers | behavior parity |
| `cache` | FTC cache APIs | API behavior parity |
| `validation` | GX/OT validation | error and table validation parity |
| `compression` | gzip/lzw/bzip streams | byte stream parity |
| `math` | fixed, trig, matrix/vector helpers | exact numeric parity |
| `error/logging` | error strings and logging controls | string/API parity |

## Fixture Families

| Fixture family | Current state | Required promotion |
|---|---|---|
| `force_autohint` | exact executable gate | preserve breadth and exactness |
| `render_mode_matrix` | exact executable gate for current rows | expand without replacing current rows |
| `native_tt_default` | threshold baseline, `3176/7640` | promote to exact `7640/7640` |
| `fixed_parity` | exact executable scalar gate | extend to more math endpoints |
| `core_face_size_charmap` | exact executable API gate for current surface | add fixture families for remaining scalar paths |
| `metrics_only` | C fixtures exist, unexecuted | implement exact runner support |
| `no_hinting` | C fixtures exist, unexecuted | implement exact runner support |
| `outline_cbox` | C fixtures exist, unexecuted | implement exact runner support |
| `render_mono` | C fixtures exist, unexecuted | implement exact runner support |
| `render_lcd` | C fixtures exist, unexecuted | implement exact runner support |
| `render_lcd_v` | in scope if LCD_V remains exposed | add C fixtures and exact runner support |
| `sfnt_tables` | partially covered by API tests | make fixture family explicit |
| `charmap` | partially covered by API tests | make fixture family explicit |

## Execution Chunks

### Chunk 1: Truthful Interface Inventory

Scope:

- Parse vendored FreeType headers for every `FT_EXPORT` symbol.
- Maintain `tests/fixtures/interface_map.json`.
- Report API coverage by path and status.
- Prevent incomplete fixture families from reporting `passing == total`.

Verification:

```bash
cargo test -p pillow-rs-freetype --test interface_coverage --locked -- --nocapture
```

Done criteria:

- All exported symbols are mapped or explicitly out of scope.
- Out-of-scope symbols have reasons.
- `native_tt_default` reports its real incomplete baseline until exact.
- Complete paths require exact executable parity gates.

### Chunk 2: Fixture Provenance And Harness Contract

Scope:

- Generate references only from FreeType C oracle scripts.
- Preserve row counts for broad matrices.
- Require raw bytes for exact render gates.
- Name unexecuted matrices as debt.

Verification:

```bash
cargo test -p pillow-rs-freetype --test harness_contract --locked
cargo test -p pillow-rs-freetype --test coverage_matrix_tests --locked -- --nocapture
```

Done criteria:

- Exact gates fail on missing matrices and missing raw bytes.
- Unsupported matrix operations fail.
- Supplemental fixture families are either executable exact gates or explicit debt.

### Chunk 3: Core Face, Size, Charmap, And Tables

Scope:

- Face loading, face count/index semantics, names, metrics, flags.
- Size selection, ppem, scale, DPI behavior.
- Charmap select/set/get/index iteration.
- SFNT raw table access.

Verification:

```bash
cargo test -p pillow-rs-freetype --test core_face_size_charmap --locked
cargo test -p pillow-rs-freetype --test interface_coverage --locked -- --nocapture
```

Done criteria:

- Current API tests remain exact.
- Fixture families are added where scalar/API tests are not enough.
- Mapped complete endpoints all have executable gates.

### Chunk 4: Native TrueType Default Pipeline

Scope:

- `FT_LOAD_DEFAULT` and native TrueType bytecode behavior.
- fpgm, prep, glyph programs, CVT, storage, twilight zone, phantom points, IUP.
- Metrics, bbox, placement, and rendered pixels.

Verification:

```bash
cargo test -p pillow-rs-freetype --test coverage_matrix_tests --locked -- --nocapture
```

Done criteria:

- `native_tt_default_matrix.json` becomes an exact gate.
- Result is `7640/7640`, not thresholded.
- The threshold bypass is removed.
- `interface_map.json` reports `7640/7640` only after the exact gate passes.

### Chunk 5: Unexecuted Fixture Promotion

Scope:

- Execute existing C-oracle fixture families in the default runner:
  `metrics_only`, `no_hinting`, `outline_cbox`, `render_mono`, `render_lcd`.

Verification:

```bash
cargo test -p pillow-rs-freetype --test coverage_matrix_tests --locked -- --nocapture
cargo test -p pillow-rs-freetype --test harness_contract --locked
```

Done criteria:

- Each family has operation-specific runner support.
- Each active row compares exact expected data.
- Each family moves from "unexecuted debt" to "exact gate" only after all rows pass.

### Chunk 6: Rasterizers And Render Modes

Scope:

- Grayscale, mono, LCD, LCD_V, bitmap conversion, LCD filtering.

Verification:

```bash
cargo test -p pillow-rs-freetype --test render_mode_matrix --locked
cargo test -p pillow-rs-freetype --test coverage_matrix_tests --locked -- --nocapture
```

Done criteria:

- Mono output matches bit packing and pitch.
- LCD/LCD_V output matches subpixel layout, pitch, filtering, and raw bytes.
- Current exact render-mode rows remain exact while coverage expands.

### Chunk 7: Extended FreeType Surface

Scope:

- Variations, CFF, Type 1/CID, bitmap fonts, color fonts, stroker, synthesis, advances, kerning, properties/modules, cache, validation, compression, math, error/logging.

Verification:

```bash
cargo test -p pillow-rs-freetype --test interface_coverage --locked -- --nocapture
```

Done criteria:

- Every in-scope path has executable exact gates.
- Every excluded path has an explicit reason.
- Project-level 100% claims require all in-scope fixture-backed paths to show `passing == total`.

## Release Rule

The report is the release gate. A 100% parity claim is allowed only when:

- Runtime FFI guard passes.
- Harness contract passes.
- Every exact gate passes.
- No threshold baseline remains.
- No present-but-unexecuted fixture family remains.
- Every in-scope interface path reports complete exact parity.
