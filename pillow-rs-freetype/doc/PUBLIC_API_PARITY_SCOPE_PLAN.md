# Public API Parity Scope Plan

Snapshot source:

- `target/api-abi-audit/route_audit.md`
- `target/coverage/unified-condition-missing-lines.txt`
- `doc/FONT_FIXTURE_COVERAGE_PLAN.md`
- `doc/api-abi-slices/mm_variations.md`

Current verified baseline:

- Concrete cases: 6,862
- Runtime parity: 6,859 / 6,859
- Runtime pending: 3
- Full route/core pending: 5
- Real parity routes: 3,581
- Real null-validation routes: 8
- Compile/static ABI contracts: 2,229
- Green placeholder-style rows: 1,039
- Condition coverage: 17,911 / 19,909 lines, 4,220 / 4,917 branches, 1,146 / 1,325 functions

This plan is about replacing false-green coverage with real public FreeType
behavior. Do not add broad fixture dimensions until the route audit explains
which public endpoint each new row proves.

## Scope Rules

1. Public FreeType-compatible endpoints are the goal surface.
2. Pillow adapter and high-level `Font` convenience helpers are supplementary
   only when routed through a public FreeType row.
3. Every implemented public endpoint must execute the pinned C oracle, Rust
   FFI, C ABI, and WASM ABI where that ABI surface exists.
4. Modeled fallback rows are not proof. They must become real parity,
   explicit unsupported, or explicit pending.
5. External data must be reproducible. Prefer generated compact fixtures under
   `scripts/`; if a fixture cannot be distributed or generated, store the
   observed C behavior as a reviewed sample-output contract only when it is
   version-pinned and source-traceable. Otherwise mark the row untestable or
   pending, not passing.
6. Pure Rust implementation follows FreeType behavior. C source is the oracle
   for behavior and debugging, never a runtime dependency.

## R0: Finish Route Disposition

Goal: reduce the 1,039 green placeholder-style rows to a per-operation ledger
with no unclassified generic fallback.

Current placeholder-style rows:

| Category | Rows | Disposition |
|---|---:|---|
| generic-fallback | 880 | Highest priority. Convert implemented surfaces to real execution; classify optional/out-of-scope modules. |
| generic-error-fallback | 139 | Convert error paths to direct public calls where implemented. |
| null-error-fallback | 7 | Keep only exact null-handle probes; otherwise route directly. |
| explicit-unsupported | 6 | Keep visibly unsupported or implement. Do not count as parity. |
| raw-slot-null-validation | 4 | Keep only where a real slot-state row establishes the setup. |
| void-fallback | 2 | Convert to real noop/null ABI calls or classify as void contract. |
| wrapper-null-validation | 1 | Keep only with independent native-slot proof. |

Largest placeholder subjects:

| Subject | Rows | Notes |
|---|---:|---|
| `ftcolor` | 130 | COLR/CPAL graph, paint traversal, palette, gradients. Needs parser/runtime work before rows are real. |
| `ftcache` | 112 | Cache manager, image/sbit/cmap caches, nodes, scaler descriptors. Needs subsystem design or explicit unsupported. |
| `ftstroke` | 86 | Stroker/border/export paths. Needs outline stroker implementation or unsupported ledger. |
| `ftmm` | 81 | Variation and Adobe MM APIs. Depends on `fvar`/`avar`/`gvar`/HVAR/MVAR and Adobe MM decisions. |
| `fterrdef` | 54 | Error-path fidelity. Many can become direct invalid-input rows. |
| `ftimage` | 49 | Outline decomposition, tags, glyph/slot image behavior. |
| `ftmodapi` | 47 | Module API. Likely explicit unsupported or limited public behavior. |
| `freetype` | 44 | General face/open/parameter/lifecycle residuals. |
| `ftgxval` | 41 | GX validator. Implement validator or classify unsupported. |
| `ftoutln` | 39 | Outline render/bitmap/decompose routes. |
| `ftbitmap` | 38 | Bitmap copy/convert/blend/embolden/done/new/init routes. Good R0 candidate. |

R0 deliverable:

- Add or refresh a generated table that lists every placeholder operation with:
  public C symbol, current category, implementation status, dependency, and
  disposition: real parity, unsupported, pending core, or compile contract.
- Keep `shape-incomplete fallback` at zero.
- Any new row must include a route category in the same change.

## R1: Coverage Bulk

Coverage should improve by exercising real public endpoints, not by unit tests
over private helpers. The largest missed-line files are:

| File | Missed lines | Missed functions | Primary meaning |
|---|---:|---:|---|
| `render.rs` | 557 | 43 | Rendering modes, bitmap emission, edge cases, non-normal pipelines. |
| `autohint/latin.rs` | 279 | 3 | Latin autohint topology/blue/stem branches. |
| `font.rs` | 253 | 42 | Core face/font behavior, variation/state edges, convenience-adjacent logic. |
| `autohint/globals_data.rs` | 230 | 1 | Script/global data tables; likely needs targeted script fixtures or accepted unreachable data. |
| `grays.rs` | 160 | 5 | Gray rasterizer cells/spans/error paths. |
| `scaler.rs` | 155 | 13 | Scaling, transforms, composite/bitmap/variation-sensitive paths. |
| `tt/sbit.rs` | 153 | 57 | Embedded bitmap formats and strike parsing. |
| `api.rs` | 53 | 0 | Public API branch/error surface. |
| `autohint/cjk.rs` | 47 | 1 | CJK topology and blue-zone residuals. |
| `ffi/handles.rs` | 35 | 1 | Public FFI error/lifecycle edges. |

Execution order:

1. `tt/sbit.rs` and `ftbitmap` routes: high missed functions plus concrete public bitmap APIs make this a good coverage/real-parity intersection.
2. `render.rs` + `grays.rs`: choose one rendered output bucket at a time: render mode, slot format, empty outline, mono/gray/LCD, unsupported state.
3. `autohint/latin.rs` and `autohint/cjk.rs`: add compact source fixtures for specific FreeType branch topology only after reading the matching C autohint function.
4. `scaler.rs`: cover transform/composite/bitmap/variation-sensitive paths through public load/render/metrics rows.
5. `font.rs`: separate public FreeType state from supplementary high-level helpers before adding rows.

Each coverage task must report:

- before/after line, branch, and function coverage;
- before/after route category counts;
- exact public operation rows added or converted;
- why the C behavior is known.

## R2: Real Parity Expansion

Current real parity is 3,581 rows. The next useful work is not multiplying
already-real rows; it is converting false-green rows into real routes.

Recommended first buckets:

1. Bitmap public APIs: `FT_Bitmap_New`, `FT_Bitmap_Init`, `FT_Bitmap_Copy`,
   `FT_Bitmap_Convert`, `FT_Bitmap_Embolden`, `FT_Bitmap_Blend`,
   `FT_GlyphSlot_Own_Bitmap`, `FT_Bitmap_Done`.
2. Outline public APIs: `FT_Outline_Decompose`, `FT_Outline_Render`,
   `FT_Outline_Get_Bitmap`, stroker/border export if implementation exists.
3. General error paths: replace `fterrdef` generic modeled errors with direct
   invalid-input or malformed-font public calls.
4. SBIT/embedded strike paths: convert `tt/sbit.rs` missed functions through
   `FT_Load_Glyph`, render, and bitmap routes.
5. Optional subsystems: cache, color, validators, module API. Decide
   implementation vs explicit unsupported before adding more placeholder rows.

## R3: Pending Core Rows

Full route/core pending rows:

1. `ftmm.FT_Set_Named_Instance.output_changes_to_named_instance`
2. `ftmm.FT_Set_Named_Instance.success_adobe_mm_resets_default`
3. `tttables.TT_VertHeader.sfnt_table_present_runtime.mvar_variation`

The two runtime pending rows are `ftmm.set_named_instance`; the MVAR vertical
header row is audit-visible route/core pending work. The earlier render
slot-state and `FT_Var_Named_Style.selected_instance_matches_descriptor` rows
are already real parity and must not be re-counted as pending work.

### Variation Implementation Scope

Use `doc/api-abi-slices/mm_variations.md` as the implementation contract.
Required pure-Rust pieces:

- `fvar`: axes, defaults, named instances, flags, name IDs, PostScript IDs.
- `avar`: design-to-normalized coordinate mapping.
- active variation state: design coordinates, blend coordinates, selected named
  instance, face-index bits, variation flag semantics.
- `gvar`: simple and composite glyph deltas before hinting, including
  fractional point precision for variable-font raster parity.
- `HVAR`/`VVAR`: advance and side-bearing deltas.
- `MVAR`: face and size metric deltas, including vertical header behavior.
- `cvar`: CVT deltas before native TrueType hinting.
- `FT_MM_Var` allocation and C ABI ownership through `FT_Get_MM_Var` and
  `FT_Done_MM_Var`.

FreeType reference areas:

- `src/base/ftmm.c`: public dispatch and service behavior.
- `src/truetype/ttgxvar.c`: OpenType variation parsing and deltas.
- `src/truetype/ttdriver.c`: variation service table.
- `src/type1/t1load.c`: Adobe Type 1 MM descriptors and weight vectors.
- `src/base/ftobjs.c`: variation selector public functions.
- `src/sfnt/ttcmap.c`: cmap format 14 behavior.

### External Dependency Policy

When a row depends on an external format or font family:

1. Read the exact FreeType C source path first and record the behavior to
   implement.
2. Prefer a compact generated fixture from repo scripts. The generator is the
   dependency endpoint.
3. If a real upstream font is needed, check license and reproducibility before
   adding it. Subset it deterministically if possible.
4. If the upstream font cannot be checked in or regenerated, use a
   version-pinned sample-output contract only as a temporary planning artifact;
   do not mark the runtime row real parity unless the input is reproducible.
5. If neither fixture nor reproducible sample is possible, mark the row
   untestable or pending with the exact missing dependency.

Adobe MM is not just a fixture problem. It needs Type 1 MM parser behavior,
design coordinates, blend/weight vector semantics, and named-instance reset
behavior matching FreeType. Until those exist, the Adobe MM row remains pending.

## R4: Verification Gates

Every completed bucket runs:

```bash
make -C pillow-rs-freetype test-unified-fixtures
make -C pillow-rs-freetype test-unified-condition-coverage
make -C pillow-rs-freetype route-audit
make -C pillow-rs-freetype test-ffi
make -C pillow-rs-freetype test-ffi-compat
make -C pillow-rs-freetype fmt
make -C pillow-rs-freetype lint
```

Report the route delta and coverage delta together. A coverage increase with no
route-audit improvement is suspicious unless the task was explicitly branch
coverage inside an already-real public route.
