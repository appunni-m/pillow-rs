# fontdone::ffi Unified Parity Contract

Baseline: `9f86de87`

This contract defines the unified FreeType parity interface for `fontdone`.
Every public FreeType endpoint that is in scope for replacement must have a
1:1 catalog entry under `fontdone::ffi`, even when the implementation behind
that entry delegates to existing pure-Rust safe APIs. The catalog is the parity
surface. Idiomatic public APIs are tested for ergonomic delegation, but they do
not count as FreeType parity unless the matching `fontdone::ffi` endpoint passes
its fixture family and comparator.

The implementation remains pure Rust. C FreeType is an oracle for fixture
generation, audit, and diagnosis only. Runtime FFI, native FreeType calls,
`extern "C"` shortcuts in production code, `freetype-sys`, `bindgen`, `cc`
build hooks, `dlopen`, or linked native FreeType are not allowed in the runtime
crate.

## Contract Goal

The unified harness must prove FreeType replacement by public endpoint, not by
nearby semantic behavior. For each in-scope C symbol, record, constant, and
macro:

1. `fontdone::ffi` exposes one named catalog entry that corresponds to the
   FreeType public endpoint.
2. The entry declares fixture families that exercise the endpoint.
3. The entry declares one normalized output schema.
4. The entry declares one comparator for that schema.
5. The entry reports a parity state: `complete`, `partial`, `planned`,
   `unsupported`, or `out_of_scope`.

A semantic wrapper such as `Face::load_glyph`, `Font::render_char_mode`, or a
Pillow-style convenience API may be covered by delegation tests. Those tests
check that the ergonomic API reaches the same pure-Rust implementation, but the
FreeType parity count is credited only to the associated `fontdone::ffi`
catalog entry after its FreeType fixture comparator passes.

## Catalog Entry Shape

Each catalog row should be data, not test prose. The row belongs in the maintained
interface map or its generated successor and should be auditable by
`make -C pillow-rs-freetype api-abi-audit`.

```text
ffi_id: freetype.FT_Load_Glyph
c_symbol: FT_Load_Glyph
c_header: freetype.h
rust_entry: fontdone::ffi::ft_load_glyph
safe_delegate: Face::load_glyph
status: partial
fixture_families:
  - native_tt_default
  - force_autohint
  - no_hinting
  - metrics_only
  - outline_cbox
  - render_mono
  - render_lcd
output_schema: glyph_slot
comparator: glyph_slot_exact
parity_counts_as: ffi_only
```

Required fields:

| Field | Meaning |
| --- | --- |
| `ffi_id` | Stable catalog key. Use the FreeType family and exact public name. |
| `c_symbol` | C function, record, constant, enum, or macro name from pinned FreeType. |
| `c_header` | Header area used by the C oracle or audit. |
| `rust_entry` | Exact `fontdone::ffi` endpoint, record, or constant. |
| `safe_delegate` | Optional ergonomic Rust API that the entry delegates to. |
| `status` | `complete`, `partial`, `planned`, `unsupported`, or `out_of_scope`. |
| `fixture_families` | Fixture families required before the entry can count as parity. |
| `output_schema` | Normalized JSON/schema emitted by both oracle and Rust runner. |
| `comparator` | Exact comparator used for the schema. |
| `parity_counts_as` | Always `ffi_only` for FreeType parity accounting. |

Coverage gates must fail when an in-scope public FreeType endpoint lacks a
catalog row, when a non-`out_of_scope` row lacks fixtures or a comparator, or
when an idiomatic API is counted directly as FreeType parity.

## Fixture Families

Fixture families are shared across endpoints by input and output type. Existing
families continue to provide the first harness substrate:

| Family | Primary endpoints | Required proof |
| --- | --- | --- |
| `native_tt_default` | Default TrueType load, render, metrics, bbox | Exact slot metrics, outline geometry, placement, and bitmap bytes. |
| `force_autohint` | Autohinted load and render paths | Exact metrics, placement, and gray bitmap bytes. |
| `no_hinting` | Unhinted load and render paths | Exact unhinted metrics, outline/cbox, placement, and bytes. |
| `metrics_only` | Metrics endpoints and advance-like surfaces | Exact 26.6 metrics and public error results. |
| `outline_cbox` | Outline bbox/cbox and geometry surfaces | Exact point, tag, contour, bbox, and cbox values. |
| `render_mono` | Mono render mode and target behavior | Exact packed mono bytes, pitch, placement, and pixel mode. |
| `render_lcd` | LCD and LCD_V render modes and targets | Exact subpixel bytes, pitch, placement, and pixel mode. |
| `render_mode` | Small static render-mode smoke matrix | Exact mode dispatch and bytes for the existing fixed rows; not full ABI parity by itself. |

Additional API/ABI fixture families should be added as the catalog expands:

| Planned family | Endpoints | Required proof |
| --- | --- | --- |
| `ffi_constants` | `FT_LOAD_*`, `FT_RENDER_MODE_*`, glyph formats, bbox modes, error codes | Exact numeric values. |
| `ffi_records` | `FT_Bitmap`, `FT_Outline`, `FT_Glyph_Metrics`, public face/slot/size records | Exact size, alignment, field offsets, field names, and pointer-width metadata. |
| `ffi_lifecycle` | `FT_Init_FreeType`, `FT_Done_FreeType`, face/size/glyph creation and destruction | Exact error codes, ownership transitions, and observable state. |
| `ffi_charmap_sfnt` | Charmap selection/enumeration, SFNT table/name APIs | Exact scalar values, record fields, byte slices, and error behavior. |
| `ffi_glyph_objects` | `FT_Get_Glyph`, copy, transform, cbox, bitmap conversion, done | Exact owned glyph state, bbox/cbox, bytes, and destroy semantics. |
| `ffi_advances_kerning` | `FT_Get_Advance`, `FT_Get_Advances`, `FT_Get_Kerning`, track kerning | Exact fixed-point vectors, arrays, and error/write behavior. |
| `ffi_mm_variations` | Multiple master and variation APIs | Exact axis records, coordinates, named instance behavior, and metric effects. |

Fixture generation must remain reproducible from maintained scripts. Fixture
rows must include a stable case id, font identity and checksum, face index,
size, transform when relevant, load flags, render mode, operation, pinned
FreeType version, generator command, schema version, and oracle output hash.

## Output Schemas And Comparators

Every `fontdone::ffi` entry emits one schema. Similar FreeType endpoints should
share schemas and comparators instead of adding bespoke assertions.

| Schema | Applies to | Comparator |
| --- | --- | --- |
| `error` | All fallible endpoints | Exact FreeType error code, or exact mapped public error class for explicitly non-ABI semantic checks. |
| `scalar` | Versions, counts, indices, flags, names, booleans | Exact integer value, string bytes, and null/empty distinction. |
| `fixed_vector` | Kerning, advances, transforms, vector helpers | Exact `FT_Pos`, `FT_Fixed`, and vector component values. |
| `metrics` | `FT_Glyph_Metrics`, `FT_Size_Metrics`, linear advances | Exact field values in FreeType units. |
| `glyph_slot` | `FT_Load_Glyph`, `FT_Load_Char`, `FT_Render_Glyph` | Exact format, metrics, advance, deltas, bitmap fields, outline fields, and error code. |
| `bbox` | Glyph and outline bbox/cbox APIs | Exact `xMin`, `yMin`, `xMax`, `yMax`, including rounding mode behavior. |
| `outline` | Outline copy, transform, embolden, decompose | Exact points, tags, contours, flags, and callback event stream. |
| `bitmap` | Rendered and standalone bitmap APIs | Exact rows, width, pitch, pixel mode, num grays, palette metadata, and byte buffer. |
| `table_bytes` | SFNT/name/table APIs | Exact length, slice contents, truncation/write semantics, and error code. |
| `record_layout` | ABI records | Exact size, alignment, field offsets, field C names, and target triple/pointer width. |
| `constant` | Enums, macros, load flags, render modes | Exact numeric value and alias behavior. |
| `lifecycle` | Library, face, size, glyph, and module ownership | Exact return code, handle validity, mutated public state, and cleanup behavior. |

Comparator failures must report `ffi_id`, `case_id`, fixture family, operation,
schema, first differing field path, C oracle value, Rust value, and enough input
metadata to reproduce one row. No comparator may use tolerance thresholds for
integer, fixed-point, byte, pixel, metric, bbox, outline, constant, or layout
parity. No comparator may special-case a failing case id.

## Idiomatic API Delegation Tests

Safe and ergonomic APIs remain valuable, but their role is separate:

- They verify that public Rust callers receive stable, idiomatic types.
- They verify delegation to the same core implementation used by
  `fontdone::ffi`.
- They may compare safe output to an already-passing `fontdone::ffi` result.
- They do not create FreeType parity credit by themselves.

For example, a `Font::render_char_mode` test can prove the convenience method
uses the correct renderer. It does not prove `FT_Load_Glyph` plus
`FT_Render_Glyph` parity until `fontdone::ffi::ft_load_glyph` and
`fontdone::ffi::ft_render_glyph` pass their cataloged fixture families.

## First Implementation Milestones

1. Create the `fontdone::ffi` catalog data model and seed it from the existing
   API/ABI audit. Every in-scope FreeType endpoint must have a row with status,
   schema, comparator, and planned fixture family.
2. Add coverage tests that fail when an in-scope row lacks a `fontdone::ffi`
   endpoint, fixture family, output schema, or comparator. These tests must also
   reject direct parity credit for idiomatic APIs.
3. Promote constants and record layouts first. They are deterministic, small,
   and unblock later function rows that depend on exact FreeType numeric values
   and ABI-facing records.
4. Add `glyph_slot` runners for `FT_Load_Glyph`, `FT_Load_Char`, and
   `FT_Render_Glyph`, reusing the existing matrix families where possible and
   preserving separate load and render operations.
5. Add lifecycle rows for library, face, size, and glyph ownership so later
   runners can share one replacement-style setup sequence.
6. Add callback and byte-slice families for outline decomposition and SFNT/name
   APIs once scalar, record, and glyph-slot gates are established.
7. Fold the unified API/ABI gates into the crate-local CI path only after the
   generated fixtures and row-count guards are reproducible from maintained
   Make targets.

## Make Targets

Current maintained targets that this contract builds on:

```bash
make -C pillow-rs-freetype api-abi-audit
make -C pillow-rs-freetype test-interface
make -C pillow-rs-freetype test-harness
make -C pillow-rs-freetype test-generator
make -C pillow-rs-freetype test-coverage-matrix
make -C pillow-rs-freetype test-render-mode
make -C pillow-rs-freetype test-coverage
make -C pillow-rs-freetype test-parity
make -C pillow-rs-freetype test-ffi
```

First unified-contract targets to add when implementation begins:

```bash
make -C pillow-rs-freetype generate-api-abi-fixtures
make -C pillow-rs-freetype test-api-abi-catalog
make -C pillow-rs-freetype test-api-abi-layout
make -C pillow-rs-freetype test-api-abi-parity
make -C pillow-rs-freetype test-api-abi
```

Target responsibilities:

| Target | Responsibility |
| --- | --- |
| `generate-api-abi-fixtures` | Generate catalog-driven C oracle fixtures and row manifests under the maintained fixture policy. |
| `test-api-abi-catalog` | Validate every in-scope public FreeType endpoint has a `fontdone::ffi` row with schema, comparator, fixture family, and status. |
| `test-api-abi-layout` | Check constants and C ABI record layout rows against pinned FreeType oracle data for the current target. |
| `test-api-abi-parity` | Run the schema comparators for all generated `fontdone::ffi` function rows. |
| `test-api-abi` | Run catalog, layout, parity, no-runtime-FFI, and row-count guards for the unified contract. |

Until these targets exist, use the current targets above for guard coverage and
keep the planned target names stable in docs and dispatch instructions.

## Reporting Rules

Parity reports should group results by `ffi_id` and fixture family. A row counts
as FreeType parity only when the `fontdone::ffi` entry passes all required
comparators for its declared complete status. Partial status must show passing
and failing row counts without narrowing the goal to the passing subset.

Reports must preserve:

- catalog entry id and C symbol;
- fixture family and row count;
- output schema and comparator;
- complete, partial, planned, unsupported, or out-of-scope status;
- first failing field path for failures;
- current no-runtime-FFI result.

Do not relabel existing semantic coverage as complete ABI or FreeType parity.
Existing safe API tests remain regression guards, while the unified
`fontdone::ffi` catalog is the source of truth for parity accounting.
