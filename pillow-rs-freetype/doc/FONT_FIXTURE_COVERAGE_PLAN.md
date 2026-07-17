# Font Fixture And Structural Coverage Plan

Status: active execution plan  
Started: 2026-07-10  
Scope: `pillow-rs-freetype` public API parity fixtures  
Owner: every agent changing the font corpus or unified public API inputs

This document is the source of truth for replacing the brute-force font corpus
with a small, intentional fixture library while driving the pure Rust core to
100% line, function, region, branch, and condition coverage through the existing
public API parity test. Update the progress ledger and baseline after every
verified batch. Do not keep progress or decisions only in chat.

## End State

The work is complete only when all of the following are true:

- The unified public API parity test uses explicit cases and explicit grouped
  input variants. It performs no implicit Cartesian expansion or runtime font
  discovery.
- One logical case may contain multiple complete input variants. Every concrete
  combination that runs is visible in JSON and was selected intentionally.
- The active font corpus is small enough to inspect and reason about. Each font
  and significant glyph has documented coverage obligations.
- No public API input, maintained test, benchmark, diagnostic, manifest entry,
  or symlink references `tests/fixtures/deprecated/`.
- The deprecated font directory is empty and can be deleted in one reviewed
  cleanup after explicit user approval.
- The Rust implementation reaches 100% line, function, and region coverage from
  the fixture-based unified public API parity test across the Rust FFI, C ABI,
  and WASM ABI paths.
- Nightly Rust coverage reaches 100% branch and condition coverage. Every atomic
  condition in a compound decision evaluates both true and false. Explicit
  fixture obligations additionally show that each condition can affect the
  decision independently, providing MC/DC-equivalent intent while rustc lacks a
  supported formal MC/DC instrumentation mode.
- Every runnable concrete case has exact parity with pinned C FreeType. Coverage
  must never be gained by weakening comparisons, expected outputs, or error
  checks.
- Lines that cannot currently be reached through a supported public operation
  remain visible and are classified. Code is removed only with semantic proof,
  independent of coverage, that it is duplicate or invalid; an uncovered line
  is never by itself evidence for deletion.
- Normal parity remains comfortably below 200,000 concrete cases and should
  remain close to the smallest set justified by unique behavior.

## Non-Negotiable Constraints

1. Fix the font corpus and public input JSON first. Do not introduce a separate
   fixture test, JSON builder, glyph-index parameter, or discovery script.
2. `tests/manifest.yaml` remains the public coverage contract.
3. `tests/fixtures/inputs/public-api/*.json` remains the execution definition.
4. Multiple inputs in one logical case are supported and preferred when they
   express deliberate variations of the same public behavior.
5. No folder scan, all-glyph loop, environment limit, default axis, or other
   hidden expansion may determine the concrete case count.
6. Do not cross-multiply fonts, sizes, glyphs, flags, and render modes merely
   because each dimension exists. Add only combinations tied to a feature,
   branch, error path, parity risk, or uncovered line.
7. Core behavior stays pure safe Rust. C FreeType is an oracle only.
8. A font may move out of `deprecated/` only by being replaced with an active
   focused fixture or by proving that its remaining cases duplicate another
   active fixture.
9. Do not delete the deprecated corpus incrementally. First reach zero
   references, then perform one separately reviewed cleanup.
10. Every batch must preserve exact Rust/C ABI/WASM parity and report concrete
    case count, pending count, line/function/region/branch/condition coverage, active
    font size, and deprecated dependency count.
11. Never remove implementation behavior, public helpers, defensive guards, or
    pinned-FreeType special cases merely to reduce the coverage denominator.
    Reachable behavior needs an explicit parity input; currently unreachable
    behavior remains visible until its call path or semantic status is proven.

### Coverage-Deletion Audit

The 2026-07-10 audit restored every public symbol and FreeType special-case
path previously removed during denominator reduction. The remaining private
source changes were reviewed independently of coverage:

| Surface | Disposition | Evidence |
|---|---|---|
| Autohint diagnostics, script helpers, blue-character lookup, and convenience APIs | restored | these were public Rust surfaces even though the unified executable did not call them |
| `AF_ADJUST_DOWN2` / `AF_ADJUST_TILDE_BOTTOM2` and second-lowest contour behavior | restored | current database reachability does not prove that pinned behavior is disposable |
| VM fetch helpers, round-mode conversion, fpgm/prep helpers, and `CallRecord` fields | restored; `fpgm`/`prep` helpers now covered through face construction | public helpers and record contracts must not change for coverage |
| Serif helper and constructed-edge defensive guards | restored | valid fixtures not reaching a guard is not semantic proof that it is unnecessary |
| `pick_typo_metrics` / `pick_os2_metrics` | retained as `face_metric_values` consolidation | explicit OS/2, hhea, and fallback fixtures prove the centralized FreeType selection order |
| `_use(GlyphLocation)` | remains removed | it was only an unused-import warning suppressor and had no runtime or public behavior |
| `parse_format0` fallible wrapper | retained as sanitized private parser | explicit kern fixtures prove FreeType ignores malformed optional subtables instead of returning a face error |

## Baseline

Recorded after the explicit-input migration and deprecated-directory split on
2026-07-10.

| Measure | Baseline |
|---|---:|
| Logical public API cases | 4,110 |
| Concrete explicit cases | 6,314 |
| Additional grouped variants | 2,204 |
| Implicit cases | 0 |
| Runnable parity comparisons | 6,302 |
| Exact parity | 6,302 / 6,302 |
| Pending cases | 12 |
| Covered Rust lines | 12,000 / 16,287 |
| Rust line coverage | 73.68% |
| Rust function coverage | 705 / 994 (70.93%) |
| Rust region coverage | 17,310 / 23,296 (74.30%) |
| Rust branch/condition coverage | 2,725 / 4,170 (65.35%) |
| Formal Rust MC/DC coverage | unavailable in the installed rustc; tracked by explicit independent-condition obligations |
| Active compact autohint fonts | 4 files, 44 KiB |
| Active fixture font paths | 379 paths |
| Stored active font binaries | 30 files, 9.8 MiB |
| Active symlink aliases | 349 links, all resolving |
| Unique active font contents | 28 SHA-256 identities |
| Deprecated brute-force fonts | 100 files, 98 unique contents, 23 MiB |
| Manifest font variability probes | 24 / 24 |

Structural coverage was measured with `rustc 1.99.0-nightly (2026-07-09)`,
`-Zcoverage-options=condition`, and `cargo-llvm-cov 0.6.14`. Totals include
only `pillow-rs-freetype/src/**`; the same run executes the thin C and WASM ABI
crates but excludes them from completion percentages.

Initial uncovered branch/condition outcomes are concentrated in:

| Module | Uncovered branch/condition outcomes | Uncovered regions |
|---|---:|---:|
| `src/autohint/latin.rs` | 477 | 1,188 |
| `src/tt/hinter/exec.rs` | 163 | 744 |
| `src/render.rs` | 153 | 1,105 |
| `src/autohint/cjk.rs` | 137 | 343 |
| `src/font.rs` | 91 | 761 |
| `src/grays.rs` | 73 | 337 |
| `src/ffi/handles.rs` | 47 | 189 |
| `src/scaler.rs` | 38 | 246 |

The 4 pending cases are existing unsupported or unresolved named-instance and
non-SFNT face inputs. They must remain visible and be converted to runnable
explicit cases during the coverage phases where their owning operations are
addressed.

## Current Verified Coverage State

Recorded on 2026-07-13 after converting `FT_Get_Gasp`,
`FT_Get_CMap_Format`, `FT_Get_CMap_Language_ID`, and
`FT_Get_SubGlyph_Info` from false-green adapters into real C oracle, Rust FFI,
C ABI, and WASM ABI parity, adding the `gasp` stream-length and malformed-EOF
controls, adding compact `post` format 1.0/2.5 plus malformed glyph-name
controls, exercising composite subglyph rows through the compact glyf
component matrix, and routing the compact `FT_Set_Named_Instance` selection,
clear, and invalid-index rows through real C oracle, Rust FFI, C ABI, and WASM
ABI execution, and reusing the shared signed big-endian `tt::read_i16` helper
from the public `post` table parser, routing optional raw `fpgm`/`prep` table
reads through their parser helpers, and extending the compact branch-edge TT
program with invalid coordinate reads that reach the zone out-of-range guards,
and centralizing the public fixed-math and matrix wrapper arithmetic on core
long-width FreeType helpers exercised by the existing fixed/vector/matrix
public fixture rows, and adding compact generated name-table controls for
Unicode/Mac fallback selection, Apple-only PostScript names, odd Windows
PostScript-name fallback, and Apple-only encoded named-instance PostScript
prefixes through public `FT_Get_Postscript_Name` variants, and adding a
rendered `FT_Set_Transform` row that exposed and fixed transform rendering of
the `LoadedOutline` bitmap snapshot in slot coordinates before presetting the
bitmap box, and adding a Unicode-only variation prefix/subfamily named-instance
control that exposed and fixed FreeType's stricter variation PostScript prefix
lookup, adding an odd-length Windows variation-prefix control that proves
Apple Roman fallback for encoded named-instance PostScript names, and adding a
missing-subfamily variation control that proves FreeType's coordinate-based
named-instance PostScript fallback for positive, zero, negative, and fractional
16.16 coordinates while closing three false-green route-audit shape
classifications, and moving public `FT_Vector_Length` execution onto a core
`FT_Long` CORDIC helper while keeping the 32-bit rasterizer helper as a wrapper,
and extending the source-backed branch-edge TrueType glyph with zero
`SPVFS`/`SFVFS` stack-vector probes, and adding a compact source-backed
`script-coverage.ttf` font plus explicit `FT_LOAD_FORCE_AUTOHINT` variants for
18 script standard-character and Indic CJK-writing-system paths, and adding
explicit `FT_LOAD_NO_AUTOHINT` precedence rows for
`FORCE_AUTOHINT | NO_AUTOHINT` and `TARGET_LIGHT | NO_AUTOHINT` while routing
FFI glyph-slot bbox conversion through the shared `From<BBox>` path, and
adding explicit `FT_LOAD_TARGET_MODE` rows that prove unknown target nibbles
are ignored for load-only calls but return `FT_Err_Cannot_Render_Glyph` when
`FT_LOAD_RENDER` asks the renderer to consume the invalid mode, and adding the
compact source-backed `cmap-format14-only.ttf` control plus explicit
`FT_Get_Char_Index`, `FT_Get_First_Char`, and `FT_Get_Next_Char` rows that
prove format-14-only charmaps return zero sentinels for direct lookup and
iteration, and adding compact generated name-table branch matrices for static
PostScript-name fallback and variable-font variation-prefix fallback through
`FT_Get_Postscript_Name`, then extending the compact autohint script font with
unequal ASCII digit advances so existing force-autohint rows exercise the
same-width digit false path without adding concrete cases, and extending the
source-backed branch-edge TrueType glyph and prep program with no-output VM
probes for `GETINFO`, delta count clamps, twilight-zone movement/intersection,
IDEF fallback, prep-range `INSTCTRL`, original-distance `MD`, negative-CVT
`MIRP`, twilight `UTP`/`SCFS`/`IP`, and invalid `ISECT` continuation without
adding concrete cases, and adding one explicit non-uniform pixel-size
`FT_Load_Glyph` variant for the same source-backed branch-edge TrueType glyph.
That row exposed and fixed a real C/Rust mismatch: pinned C used the active
20 px horizontal size for the glyph-slot advance (`896`), while Rust rebuilt a
square scaler from the 32 px height and returned `1408`. The scaler and
autohint metrics now consume the active FreeType size object's x/y scales and
TT interpreter ppem/point-size instead of reconstructing them from height.
Two additional explicit non-uniform `FT_Load_Glyph` variants for the
point-coordinate and point-move matrix glyphs then cover the remaining
non-square `MD[0]` and `IP` interpreter branches, while repeating `IUP[y]`
and `IUP[x]` inside the existing point-move matrix covers FreeType's
backward-compatibility early-return path after both axes have already
interpolated. The compact `script-coverage.ttf` public input set now selects
all 59 generated script glyphs through explicit `FT_LOAD_FORCE_AUTOHINT`
variants, proving the script coverage table paths without implicit discovery
or Cartesian expansion. Two compact hhea-zero metric controls now select the
remaining FreeType face-metric fallback order through public
`FT_Size_Metrics`: zero hhea with nonzero OS/2 typo metrics, then zero hhea
and zero OS/2 typo metrics falling through to OS/2 Windows ascent/descent.
The existing `FT_Get_Charmap_Index` public rows now route the Rust parity path
through the core public helper instead of bypassing it with the face-scoped
lookup. The C ABI wrapper still owns raw-pointer validation, but delegates the
actual owned-charmap return value back to core so the ABI remains thin. The
existing `FT_Get_CMap_Format` and `FT_Get_CMap_Language_ID` rows also verify
the core face+charmap metadata helpers agree with the raw public CMap helpers
for valid, null, and out-of-range charmaps. Selected existing `FT_LOAD_*`
rows now also verify safe Rust `Face::load_glyph` slot output agrees with the
Rust FFI `FT_Load_Glyph` slot for representative load flags, without adding
fonts or cases. The malformed `FT_Load_Glyph` matrix now includes explicit
no-autohint and force-autohint error rows over the existing compact malformed
`glyf` fixture, and the selected malformed rows compare safe
`Face::load_glyph` error parity against `FT_Load_Glyph` instead of checking
only success slots. Existing `FT_New_Size`, `FT_Done_Size`, and
`FT_Activate_Size` null rows now route through pinned C oracle commands and the
thin Rust FFI validation wrappers for null face, null output, and null size
handles. The compact CJK autohint source font now adds an isolated
`cjkStemSort` glyph and one explicit `FT_LOAD_FORCE_AUTOHINT` public variant
for CJK stem-width ordering, preserving the existing U+7530 coverage path while
adding the new topology as an additive row. Multi-size success lifecycle rows
remain visibly unsupported/generic until real secondary-size object ownership
is implemented. Three explicit `FT_Bitmap.public_fields_match_render_output`
variants now cover mono/SDF first-offcurve-last-on topology and SDF degenerate
conic flattening through existing compact charmap glyphs, the stale
`FT_LOAD_NO_RECURSE` composite selector now targets DejaVu `Agrave` instead of
a missing `Aring`, and two explicit native composite `FT_Load_Glyph` variants
cover point attachment and unrounded-offset scaler paths. Correcting the
no-recurse selector exposed and fixed a real C/Rust metrics mismatch: pinned C
keeps composite slots in composite format and computes metrics from the raw
`glyf` composite header bbox, while Rust previously used the resolved component
outline bbox. The source-backed `hinter-control-matrix.ttf` super-round glyph
now also packs a no-output S45ROUND clamp probe into the existing
`hinter-super-round-matrix` public row, covering the positive and negative
`Round_Super_45` clamp repairs without adding cases or changing glyph output.
One additional explicit `FT_Set_Transform.load_ignore_transform_behavior`
variant renders the compact empty glyph under the same non-identity transform,
covering the transformed render-outline empty guard while keeping exact
Rust/C ABI/WASM parity. The compact CJK autohint source font now also includes
an additive `cjkSerifM` glyph mapped at U+519D. One explicit
`FT_LOAD_FORCE_AUTOHINT` public row selects that glyph to exercise FreeType's
CJK 12-edge serif-`m` horizontal-axis stabilization path; the glyph top is kept
below the next pixel boundary so the row isolates the x-edge topology without
introducing an unrelated vertical metrics mismatch. A separate two-glyph
`cjk-width-order.ttf` fixture now omits U+7530 so Hani standard-width
initialization falls through to U+56D7, whose wide-then-narrow stems exercise
the descending insertion-sort and quantization branches without disturbing the
productive U+7530 rows.
The generated `cjk-blue-edge-cases.ttf` fixture keeps that productive glyph
set separate while mapping Hani blue-string probes to one contourless glyph,
one degenerate one-point-contour glyph, one top flat-only glyph, and one bottom
fill/flat inversion. A single explicit `FT_LOAD_FORCE_AUTOHINT` row covers the
empty-blue-glyph skip, degenerate-contour skip, flat-only blue zone, and
ref/shoot order-repair paths through pinned C, Rust FFI, C ABI, and WASM ABI
parity.
The generated `cjk-tiny-stem.ttf` fixture isolates a U+7530 Hani standard glyph
with a subpixel-width vertical stem, proving the CJK minimum snapped standard
width clamp through the same `FT_LOAD_FORCE_AUTOHINT` public route.
The generated `cjk-snap-below-standard.ttf` fixture keeps U+7530 as a
100-unit standard Hani stem and maps U+4ED6 to a 90-unit Hani stem. One
explicit `FT_LOAD_FORCE_AUTOHINT | FT_LOAD_TARGET_MONO` public row selects the
narrower glyph at 20 ppem, proving CJK's lower-side snap-to-reference branch
without adding a size or glyph Cartesian product.
Three named-instance obligations remain explicit pending rows: Adobe MM reset
behavior, `gvar`/HVAR glyph-output deltas, and `FT_MM_Var` namedstyle
coordinate parity.

The latest R0 route cleanup splits executable invalid-argument rows out of
previously inert aggregate declarations for `FT_Get_Sfnt_Name`,
`FT_Load_Glyph`, and `FT_Load_Char`. Invalid index and reserved load-flag
behavior now compares exact C oracle, Rust FFI, C ABI, and WASM ABI output.
Null-handle, null-output, non-SFNT, and bitmap-missing residuals stay visible
as shape-incomplete or pending route work instead of being replaced by normal
loaded-face rows.
The route audit also now recognizes `freetype.face_flags` as an existing real
parity route: the public runner already compares pinned C `--face-flags`,
Rust FFI, C ABI, and WASM ABI output for those rows, so keeping them in generic
fallback understated the trusted route count.
`FT_Render_Glyph.error_unloaded_or_unsupported_slot_format` now also contains
one executable public error row: rendering DejaVu `Agrave` after
`FT_LOAD_NO_RECURSE` leaves a composite slot, and pinned C returns
`FT_Err_Cannot_Render_Glyph`. The genuinely synthetic unloaded/unsupported
slot states remain separate route work.
`FT_Set_Char_Size` and `FT_Request_Size` now also split real public size-error
rows out of inert/future buckets. DejaVu oversized char-size requests compare
FreeType's `FT_Err_Invalid_Pixel_Size` through Rust FFI, C ABI, and WASM ABI,
including host-width values that exceed the core `i32` input range. Negative
face-index probe handles compare FreeType's `FT_Err_Invalid_Size_Handle` for
`FT_Set_Char_Size` and `FT_Request_Size`, covering the probe-only wrapper
guards without adding hidden route logic. The bitmap/malformed residuals
remain visible as future fixture work. `FT_Request_Size` rows that need runtime
core coverage must use explicit `params.request` or `params.requests` shapes:
`params.variants` is a probe-only parser/manifest route and does not execute
the runtime request-size path.
`FT_Set_Pixel_Sizes` now uses the same direct-operation route shape across
Rust FFI, C ABI, and WASM ABI: open the face without pre-sizing, call
`FT_Set_Pixel_Sizes`, then read metrics only on success. This keeps the normal
rows equivalent while allowing a negative face-index probe row to compare
FreeType's `FT_Err_Invalid_Size_Handle` through all public legs.
`FT_Load_Sfnt_Table` now has exact public rows for the pinned FreeType SFNT
table reader semantics instead of treating it as a modeled table lookup:
tag `0` reports and reads the whole font stream, tag `1` reports and reads
the table directory, oversized host tags return `FT_Err_Table_Missing`,
`*length == 0` probes ignore `offset`, nonzero reads apply the signed offset
to the raw stream position instead of clamping to the table record, and a null
length pointer performs a full copy without mutating caller length state. A
separate null-length out-of-stream row now covers the matching stream-error
return path. The missing-table/error case is now split into explicit variants,
including a live Type 1 non-SFNT face that covers FreeType's
`FT_Err_Invalid_Face_Handle` return before the SFNT service call. The C ABI
and WASM legs call their public wrappers directly for these rows, so the
wrappers remain thin pointer/handle surfaces over the Rust core behavior.
`FT_Sfnt_Table_Info` now also has explicit variants for nullable tag/length
out-pointers and table-index selection. Pinned C returns the table count when
`tag == NULL`, ignores `table_index` in that count-query mode, rejects
`length == NULL` with `FT_Err_Invalid_Argument`, and returns
`FT_Err_Table_Missing` for out-of-range table indexes when `tag` is non-null.
Rust FFI, C ABI, and WASM ABI now route through that same public pointer
contract instead of using a Rust-only modeled tuple helper.
`FT_Outline_Get_CBox.null_inputs_noop` now calls pinned C
`FT_Outline_Get_CBox(NULL, acbox)` and `FT_Outline_Get_CBox(outline, NULL)`,
and the Rust FFI, C ABI, and WASM ABI legs route the same nullable pointer
shapes through thin public wrappers. Live glyph CBox rows also execute the safe
Rust helper and verify it matches the loaded slot cbox, so null/no-op and
control-point/empty-outline behavior are both real route evidence.
Three compact generated format-1 `name` table controls exercise malformed
language-tag count, record-array, and string-range behavior through
`FT_New_Memory_Face`. Pinned C rejects structurally missing count/record data,
but retains an out-of-range tag string as an empty slot and still opens that
face. Public rows carry route-visible `font` aliases beside their memory-byte
sources so route audit counts them as real parity instead of fallback evidence.
Two additional compact name-table controls exercise successful fallback after
malformed name strings: an out-of-range English Windows typographic family
record falls back to Apple Roman family text through `FT_New_Memory_Face`, and
an out-of-range Apple PostScript-name record returns a null
`FT_Get_Postscript_Name` result.
`FT_Select_Charmap.error_missing_encoding` now has an explicit non-Unicode
format-6 font row for `FT_ENCODING_UNICODE`. That row exposed and fixed a real
C/Rust mismatch: pinned C `find_unicode_charmap` returns
`FT_Err_Invalid_CharMap_Handle` when no charmap is tagged
`FT_ENCODING_UNICODE`, while Rust previously fell back to charmap index 0 and
returned success.
Existing `FT_RENDER_MODE_NORMAL` and `FT_Get_Kerning` public rows now also
declare safe `Font` helper assertions instead of relying on a separate fixture
family. The normal render rows compare `Font::getmetrics`, `getlength`,
`glyph_metrics`, `getbbox`, and `getmask` against the same C-oracle-backed
size, slot, and rendered bitmap surfaces already exercised by the row,
including empty-text and empty-outline mask behavior. The kerning row compares
`Font::getkerning` against the `FT_KERNING_UNFITTED` vector from the existing
public kerning route. The primary `FT_Load_Glyph.render_and_target_modes`
matrix now also contains the missing normal `FT_LOAD_RENDER |
FT_LOAD_MONOCHROME` variant and routes all four monochrome render-target
combinations through the safe `Face::load_glyph` agreement hook. This keeps the
safe Rust convenience surface explicit while adding only one concrete input.
The existing `FT_Get_Charmap_Index.owned_charmap_indexes` row now also asserts
that safe `Font` charmap accessors and selection helpers agree with the same
C-oracle-backed charmap metadata rows. Existing render-mode rows now also
declare `RenderMode::fixture_name` and `PixelMode::fixture_name` obligations
for normal, mono, LCD, vertical LCD, and SDF render paths without adding fonts
or concrete cases. `FT_New_Memory_Face.valid_font_bytes` now includes one
compact generated face with the optional `name` table removed, proving that
pinned C FreeType, Rust FFI, C ABI, and WASM ABI all accept the constructor
fallback path. The cached-face execution route for `FT_Get_Kerning` now also
honors the existing `assert_font_getkerning_agrees` input declaration, so the
safe `Font::getkerning` helper is reached by the same C-oracle-backed row.
`FT_Get_Advance.success_horizontal_scaled_advance` now also declares a
codepoint-level safe `Font::glyph_hori_advance_26dot6` assertion on the existing
DejaVuSans `A` no-hinting row, comparing the 26.6 helper against the same
C-oracle-backed 16.16 advance rounded to 26.6.
Existing `FT_Render_Glyph.matrix_render` rows now also declare safe `Font`
render agreement for no-hinting, force-autohint, target-light, and no-autohint
normal-render outputs, so the safe load-mode dispatch paths are checked against
the same rendered glyphs already compared across pinned C, Rust FFI, C ABI, and
WASM ABI. The same matrix now includes explicit force-autohint target-mode rows
for MONO, LCD, and LCD_V, proving safe `Font` render agreement against
C-oracle-backed rendered glyphs without adding fonts or implicit cases.
The thin C ABI `FT_Render_Glyph` wrapper now delegates already-rendered bitmap
slots into the core `GlyphSlot::render` no-op path instead of carrying a
duplicate wrapper short-circuit. Existing pre-rendered public render rows
therefore prove the core bitmap render route across Rust FFI, C ABI, and WASM
ABI while keeping the wrapper limited to ABI conversion and error mapping. The
wrapper preserves the original load flags for this no-op path to match pinned
FreeType's `FT_Render_Glyph_Internal` bitmap behavior.
The compact generated `fvar-zero-axis.ttf` control now exercises the valid-header
but zero-axis `fvar` edge through `FT_FACE_FLAG_MULTIPLE_MASTERS`. The row
exposed and fixed a real C/Rust mismatch: pinned C rejects `num_axes == 0`
before setting `TT_FACE_FLAG_VAR_FVAR` in `sfobjs.c`, so the public
`FT_FACE_FLAG_MULTIPLE_MASTERS` bit is clear; Rust previously set the bit for
any parsed `fvar` table.
The compact generated `cjk-duplicate-edge.ttf` fixture now also carries a
`hani_serif_conflict` glyph. One explicit `FT_LOAD_FORCE_AUTOHINT` public row
selects U+51A0 to exercise the CJK edge cleanup path where a grouped edge has
both a stem link and a serif candidate; the row removed `src/autohint/cjk.rs`
line 550 from the missing-line report while preserving exact Rust FFI, C ABI,
and WASM ABI parity.
The latest pushed checkpoint also adds one `FT_New_Memory_Face` row for
`FT_Long::MIN` face-index validation, extends `script-coverage.ttf` with
Latin/Greek/Cyrillic blue-string aliases plus a three-contour double-top probe,
declares representative safe `Face::load_glyph` route agreement on existing
load rows, adds one explicit SDF conic-chain render row, and adds one compact
no-scale normal render row over `hinter-control-matrix.ttf`. `FT_Request_Size`
now also has one explicit ppem-overflow request row that reaches the core
invalid-pixel-size branch instead of staying on the probe-only `variants`
route. `FT_Get_Postscript_Name` now also exercises an fvar instance with an
explicit `postscriptNameID`, and `variable-name-missing-subfamily.ttf` uses a
real 0.5 16.16 coordinate for its existing fractional named-instance row.
`FT_Size_Metrics` now also selects a 4.1 KiB hhea-zero/no-OS2 metric fixture,
covering the final face metric fallback where both hhea and OS/2 metrics are
unavailable. The latest compact ftsynth rows extend the source-backed
`hinter-control-matrix.ttf` with one nearly-opposite sharp-turn outline and one
self-intersecting zero-area outline, selecting both through
`FT_GlyphSlot_AdjustWeight`. They cover FreeType's zero-shift embolden branch
and zero-area orientation-none area-accumulator branch without implicit
expansion. These rows keep the corpus explicit and preserve exact Rust FFI, C
ABI, and WASM ABI parity.
The latest SBIT public-error rows add two explicit
`FT_LOAD_SBITS_ONLY` incompatible-flag variants over the existing compact
`sbit_missing_bitmap.ttf` fixture: `FT_LOAD_SBITS_ONLY | FT_LOAD_NO_BITMAP` and
`FT_LOAD_SBITS_ONLY | FT_LOAD_NO_SCALE`. Pinned C returns
`FT_Err_Invalid_Argument` for both rows before SBIT lookup, and Rust FFI, C ABI,
and WASM ABI prove the same public guard without adding bitmap decoding, font
bytes, or implicit expansion.
The latest TrueType interpreter probes extend the source-backed
`hinter-control-matrix.ttf` bytecode with no-output indexed-stack fallback,
inverse `MIN`, `SCANCTRL` false-side, and twilight zone-pointer MD/MDRP/SHC
sequences plus one Y-touched DELTAP compatibility probe. The existing public
`FT_Load_Glyph` rows prove pinned C, Rust FFI, C ABI, and WASM ABI still agree
while `tt/hinter/exec.rs` covers the CINDEX/MINDEX, zone-pointer, and DELTAP
branch-side gaps without adding concrete cases.
The latest autohint script probe extends `script-coverage.ttf` with one
serifed three-stem Latin `m` glyph selected by an explicit
`FT_LOAD_FORCE_AUTOHINT` public row. Pinned C, Rust FFI, C ABI, and WASM ABI
agree while `autohint/latin.rs` covers the 12-edge serif symmetry movement path
without introducing a script loop or implicit expansion.
The latest Type 1 constructor batch adds one generated 1.7 KiB metadata probe
with `Weight (Bold)` and an unrecognized `isFixedPitch` token, plus one
positive face-index error probe over the existing compact Type 1 face. Both rows
run through public `FT_New_Memory_Face` and exact Rust FFI, C ABI, and WASM ABI
parity; together they remove the Type 1 metadata and one-face index guards from
the `font.rs` missing-line report without changing implementation code.
The latest size-selection batch turns `FT_Select_Size` into a real public
parity route.  The compact `sbit_gray_format1.ttf` fixture supplies the fixed
20 ppem EBLC/EBDT strike; Rust core now reads the strike ppem, updates the
active face size metrics through the same size object used by later loads, and
resets the scaler, autohint globals, and bytecode context.  Direct Rust FFI,
C ABI, and WASM ABI rows prove the null-face, no-fixed-size,
negative-index, out-of-range, and success paths against pinned C, and the
`ftsizes.activate_select_size_sequence` row proves strike selection mutates the
currently active size rather than a hidden singleton.
The latest SBIT bitmap-success row adds `sbit_mono_format1.ttf`, a compact
source-backed 1-bit EBLC/EBDT format-1 fixture.  One explicit
`FT_Load_Glyph.matrix_load@sbit-mono-format1-sbits-only` variant selects glyph
1 with `FT_LOAD_SBITS_ONLY`, and pinned C, Rust FFI, C ABI, and WASM ABI agree
on `FT_PIXEL_MODE_MONO`, pitch 2, `num_grays == 2`, and bitmap bytes
`a5805a00`.  Core SBIT decoding now maps FreeType's bit-depth 1 allocation path
to MONO while preserving the existing bit-depth 8 GRAY fixture behavior.
The latest packed SBIT batch adds four compact generated strikes:
`sbit_gray2_format1.ttf`, `sbit_gray4_format1.ttf`,
`sbit_bgra_format1.ttf`, and `sbit_gray_format3.ttf`.  Existing pixel-mode
manifest placeholders for GRAY2, GRAY4, and BGRA now route through real
`FT_Load_Glyph` public parity rows instead of generic build-dependent
fallbacks, while `FT_Load_Glyph.matrix_load` gains explicit 2-bit, 4-bit,
32-bit, and index-format-3 success variants.  Core SBIT decoding now mirrors
FreeType `sfnt/ttsbit.c:544-589,700-743` for bit-depth-to-pixel-mode mapping
and byte-aligned packed-row copying.
The latest SBIT negative-control batch adds two compact generated strikes:
`sbit_unsupported_bit_depth_format1.ttf` and
`sbit_unsupported_image_format.ttf`.  Explicit `FT_Load_Glyph.matrix_load`
variants select glyph 1 with `FT_LOAD_SBITS_ONLY` to prove pinned C, Rust FFI,
C ABI, and WASM ABI parity for unsupported bit depth 7 in image format 1 and
unsupported image format 10, without adding another bitmap-success axis.
The latest SBIT compound batch adds six compact generated strikes:
format-8 gray success, format-9 big-metrics success, MONO success, BGRA
success, negative component offset, and component out-of-bounds.  Core now
mirrors FreeType `sfnt/ttsbit.c:961-1012`: allocate the compound root bitmap
from root metrics, recursively load each component, OR component bytes into the
root canvas, and preserve root metrics for the returned slot.  This adds real
implementation code, so the coverage percentage shifts with a larger source
denominator even though absolute covered lines and SBIT behavior both move
forward.
The latest SBIT vertical-layout row reuses the compact format-9 compound strike
and adds `FT_LOAD_VERTICAL_LAYOUT | FT_LOAD_SBITS_ONLY` for glyph 2.  Pinned C
returns a vertical bitmap slot with zero bitmap-left/top from the big metrics;
Rust FFI, C ABI, and WASM ABI now prove the same public path without adding
font bytes.
The latest ftsynth bitmap-strength row adds two horizontal-only adjustments to
the existing MONO SBIT `FT_GlyphSlot_AdjustWeight` variant.  It keeps the
explicit case count flat while proving FreeType's positive-pitch mono bitmap
padding and same-row embolden paths through the public slot mutation route.
The latest Type 1 face-flag row adds one generated 1.7 KiB
`fixed-pitch-type1.pfb` fixture and a single explicit
`FT_FACE_FLAG_FIXED_WIDTH` variant.  Pinned C, Rust FFI, C ABI, and WASM ABI
agree that `/isFixedPitch true` sets the public fixed-width face flag on a
Type 1 face, covering the Type 1 fixed-pitch branch without adding any
implicit inputs.
The latest SBIT advance-fallback row adds one generated 4.8 KiB
`sbit_gray_format1_vmtx.ttf` fixture and one explicit `FT_Load_Glyph` variant.
It reuses the compact gray format-1 bitmap but sets the embedded horizontal
advance to zero and adds `vhea/vmtx`, so the public scalable-SBIT missing
horizontal and vertical advance fallbacks are proven against pinned C with real
font metrics instead of synthesized defaults.
The latest SBIT packed-compound row adds one generated 4.7 KiB
`sbit_composite_mono_carry_success_format8.ttf` fixture and one explicit
`FT_Load_Glyph.matrix_load` variant.  Glyph 2 is an image-format-8 compound
MONO bitmap that references a 10-bit child at a 7-pixel x offset, proving
FreeType's packed compound tail-bit carry into the second target byte through
pinned C, Rust FFI, C ABI, and WASM ABI parity.
The latest SBIT packed-depth rows add two generated 4.7 KiB fixtures:
`sbit_composite_gray2_success_format8.ttf` and
`sbit_composite_gray4_success_format8.ttf`.  Each selects glyph 2 through
`FT_Load_Glyph.matrix_load` with `FT_LOAD_SBITS_ONLY` and proves the distinct
2-bit and 4-bit packed compound dispatch arms against pinned C, Rust FFI,
C ABI, and WASM ABI without multiplying offsets, sizes, or render modes.
The latest SBIT zero-width component row adds one generated 4.7 KiB fixture:
`sbit_composite_mono_zero_width_component_format8.ttf`.  Glyph 2 is a compound
MONO bitmap that references a zero-width glyph 1 component, proving the packed
compound blitter's empty-line no-op branch against pinned C, Rust FFI, C ABI,
and WASM ABI through the existing public `FT_Load_Glyph.matrix_load` route.
The latest TrueType overlap rows append four glyphs to the existing generated
`render-coverage.ttf` fixture.  Gid 6 sets the simple-glyph first-point
`OVERLAP_SIMPLE` flag, gid 7 sets the first-component `OVERLAP_COMPOUND` flag,
gid 8 carries `OVERLAP_SIMPLE` on two overlapping contours with fractional
pixel edges, and gid 9 is a wide flagged outline that reaches FreeType's
smooth-overlap width overflow guard.  Three explicit
`freetype.load_glyph_outline` variants prove
pinned C, Rust FFI, C ABI, and WASM ABI expose `FT_OUTLINE_OVERLAP` for
no-scale loads and `FT_OUTLINE_OVERLAP | FT_OUTLINE_HIGH_PRECISION` for scaled
loads below 24 ppem.  Two explicit
`ftimage.FT_OUTLINE_OVERLAP.smooth_overlap_behavior` render variants now prove
FreeType's 4x smooth overlap oversampling path for NORMAL/LIGHT gray rendering.
Two explicit `fterrdef.FT_Err_Raster_Overflow.raster_buffer_or_cell_overflow`
variants prove the matching NORMAL/LIGHT width overflow error path.

| Measure | Current |
|---|---:|
| Logical public API cases | 4,166 |
| Concrete explicit cases | 6,844 |
| Additional grouped variants | 2,678 |
| Implicit cases | 0 |
| Runnable parity comparisons | 6,841 |
| Exact parity | 6,841 / 6,841 |
| Pending cases | 3 |
| Covered Rust lines | 17,771 / 19,781 (89.8387%) |
| Rust function coverage | 1,145 / 1,329 (86.1550%) |
| Rust instantiation coverage | 1,148 / 1,332 (86.1862%) |
| Rust region coverage | 25,586 / 28,478 (89.8448%) |
| Rust branch/condition coverage | 4,179 / 4,904 (85.2162%) |
| Formal Rust MC/DC coverage | 0 / 0; not emitted by the installed toolchain |
| Active fixture font paths | 174 |
| Stored active font binaries | 131 files, 916 KiB |
| Active symlink aliases | 43 |
| Unique active font contents | 143 SHA-256 identities |
| Deprecated brute-force fonts | 101 files, 99 unique contents, 23 MiB |

The current coverage target is not only line coverage. The maintained
completion gate is:

- `lines`: every executable source line is reached by fixture parity.
- `functions` and `instantiations`: every function body and monomorphized
  instance that remains in the core is exercised.
- `regions`: every LLVM source region is exercised, including expression
  subregions that line coverage can hide.
- `branches` / `conditions`: both true and false outcomes of decisions are
  covered. This is the answer to the "if condition" question: an `if` line is
  not complete just because the line ran once.
- `MC/DC`: not currently available from this rustc/llvm-cov report; when a
  compound predicate matters, the input JSON must include explicit
  independent-condition rows so each operand is shown to affect the public
  result.

Current largest uncovered buckets:

| File | Lines | Branches | Functions | Regions | Coverage path |
|---|---:|---:|---:|---:|---|
| `src/render.rs` | 1,721 / 2,277 | 351 / 426 | 121 / 164 | 2,426 / 3,221 | Render-mode and glyph-to-bitmap rows over focused outline, mono, LCD, cubic, and transformed fixtures |
| `src/font.rs` | 2,070 / 2,292 | 235 / 278 | 189 / 228 | 2,862 / 3,196 | Public route audit, charmap accessors, size variants, table lookup boundaries, layout/convenience wrappers |
| `src/autohint/latin.rs` | 2,538 / 2,828 | 1,006 / 1,282 | 70 / 73 | 3,648 / 4,207 | Latin blue-zone, serif, diagonal, link, and adjustment glyph roles in existing compact fonts |
| `src/scaler.rs` | 1,076 / 1,229 | 158 / 188 | 49 / 62 | 1,147 / 1,280 | Composite, no-scale, LCD/mono scaler entry points through public load/render rows |
| `src/autohint/globals_data.rs` | 63 / 293 | 0 / 0 | 1 / 2 | 117 / 234 | Script coverage rows; do not delete lookup data for coverage |
| `src/grays.rs` | 650 / 810 | 134 / 184 | 30 / 35 | 918 / 1,139 | Direct public outline/render rows that hit scan conversion edge cases |
| `src/ffi/handles.rs` | 1,858 / 1,892 | 340 / 376 | 203 / 204 | 2,522 / 2,571 | Public FFI route audit; wrappers stay thin and must delegate to core |
| `src/tt/sbit.rs` | 509 / 664 | 56 / 72 | 32 / 90 | 770 / 1,007 | Compact EBLC/EBDT fixtures for embedded bitmap success, malformed public errors, and compound assembly |
| `src/tt/hinter/exec.rs` | 1,351 / 1,379 | 381 / 416 | 40 / 43 | 2,740 / 2,945 | Add one TrueType program role per remaining VM state/opcode family |
| `src/autohint/cjk.rs` | 893 / 941 | 381 / 426 | 18 / 19 | 1,187 / 1,247 | CJK topology rows in the compact multiscript fixture |
| `src/api.rs` | 1,022 / 1,076 | 230 / 298 | 92 / 92 | 1,547 / 1,612 | Public API wrapper rows for render cache and glyph-slot surfaces |

Immediate `gasp` residuals: `src/tt/gasp.rs` is real parity and covers short
physical table data plus truncated range arrays. The only remaining uncovered
lines are arithmetic overflow closures that are mathematically unreachable from
a `u16` range count. They must stay classified as defensive-unreachable unless
a separate semantic refactor removes them; do not delete them only to improve
coverage.

Immediate `post` residuals: `src/tt/post.rs` now covers format 1.0, valid
format 2.0, valid format 2.5, short format 2.0/2.5 tables, zero format 2.0/2.5
name counts, format 2.0 missing custom strings, format 2.5 above-limit counts,
valid deltas, invalid negative deltas, glyph-name lookup, and name-index lookup
through public parity. The malformed controls exposed two correctness fixes:
unsupported `post` formats must not set `FT_FACE_FLAG_GLYPH_NAMES`, and missing
format 2.0 custom names surface as `.notdef`. Core now preserves FreeType's
layering: `Font::glyph_name` and `Font::name_index` own the public face-flag
equivalent for formats 1.0, 2.0, and 2.5, while `PostTable::glyph_name` mirrors
the `tt_face_get_ps_name` service's initialized `.notdef` result. The only
remaining line is the direct invalid-index guard (`src/tt/post.rs:47`).
`FT_Get_Glyph_Name` rejects `glyph_index >= num_glyphs` before service
dispatch, and `FT_Get_Name_Index` scans only `0..num_glyphs`; keep that guard
classified unless a supported public route is identified.

Immediate `name` residuals: compact public `FT_Get_Postscript_Name` rows now
cover unsupported name records, invalid Apple string offsets, Unicode family
fallback, Apple-Roman subfamily fallback, Apple-only PostScript names, odd
Windows PostScript-name rejection, Apple-only encoded named-instance
prefix/subfamily selection, Unicode-only variation prefix rejection, odd-length
Windows variation-prefix fallback, and missing-subfamily coordinate synthesis
for positive, zero, negative, and fractional fvar coordinates. `src/tt/name.rs`
is now 293 / 294 lines, 470 / 481 regions, and 116 / 134 branch outcomes
covered. The missing-subfamily
candidate exposed and fixed a real C/Rust mismatch: pinned C synthesized
`MissingVar_100wght`, while Rust previously kept the base `Ubuntu-Regular`
name.

Immediate fixed-math residuals: public `FT_MulDiv`, `FT_MulFix`,
`FT_DivFix`, `FT_RoundFix`, `FT_CeilFix`, `FT_FloorFix`,
`FT_Vector_Transform`, `FT_Matrix_Multiply`, and `FT_Matrix_Invert` now route
through core long-width helpers and pass exact Rust FFI, C ABI, and WASM ABI
fixture parity. The FFI wrapper no longer owns a separate arithmetic
implementation. This is semantic centralization to enforce thin wrappers, not
coverage-only deletion. `FT_RoundFix`, `FT_CeilFix`, and `FT_FloorFix` now keep
FreeType's native signed-long `FT_Fixed` domain instead of truncating through
the internal 32-bit helper shape; `fixed.rs` is line/function complete, with
only branch residuals left for vector-length and vector-normalization paths.

Immediate transform-render residuals: the compact
`FT_Set_Transform.load_ignore_transform_behavior@rendered-transformed-load`
row now covers the public face-transform plus `FT_LOAD_RENDER` path and passes
exact Rust FFI, C ABI, and WASM ABI parity. The fix was in core behavior, not
the harness: Rust now reconstructs the render snapshot into glyph-slot
coordinates, applies the transform there, then presets the bitmap box from the
transformed control box before rendering, matching pinned FreeType's
`FT_Load_Glyph` transform and `ft_glyphslot_preset_bitmap` order.

### Remaining Public Input Dependencies On Deprecated Fonts

| Deprecated font | Explicit references | Current obligation |
|---|---:|---|
| `DejaVuSans.ttf` | 0 | replaced by focused retain-GID metadata, hinting, outline, touch-tag, and cmap fixture |
| `NotoSans-Regular.ttf` | 0 | replaced by active compact Noto fixture |
| `Ubuntu.ttf` | 0 | replaced by focused 9 KiB variable subset |
| `DejaVuSans-Bold.ttf` | 0 | replaced by compact combined style fixture |
| `DejaVuSans-Oblique.ttf` | 0 | replaced by compact combined style fixture |
| `DejaVuSansMono.ttf` | 0 | replaced by compact active fixed-width fixture |
| `NotoSansMongolian-Regular.ttf` | 0 | replaced by compact CJK vertical fixture |

Reference counts are a migration aid, not a coverage score. One reference may
contain many explicit variants, and many references may exercise the same font
feature.

## Coverage Model

The unit of design is a **coverage obligation**, not a font and not a Cartesian
row. A coverage obligation records why a concrete variant exists.

Line coverage proves that a source line executed, but it does not prove both
outcomes of an `if` or the independent effect of each term in `a && b`. Region
coverage distinguishes executable source ranges on the same line. Branch
coverage requires decision outcomes. Condition coverage makes each atomic
boolean evaluate true and false. Each explicit obligation must also show that
the condition can independently change the enclosing decision, which is the
MC/DC property. The plan requires all of these even though rustc currently
reports branch/condition counts rather than a formal MC/DC percentage.

For `if a && b`, the minimal obligation set is normally `(false, true)`,
`(true, false)`, and `(true, true)`. The unused fourth combination is added only
when it reaches distinct behavior. This is deliberate condition selection, not
Cartesian expansion.

Each obligation must identify:

- Public manifest subject and case.
- Public operation used by the unified parity harness.
- Font feature or malformed condition required to enter the behavior.
- Exact glyph or character and why its topology matters.
- Size, load flags, render mode, transform, variation coordinates, or other
  state needed to enter the behavior.
- Expected public output shape and exact C-oracle comparison.
- Rust source lines or branch family reached after coverage is measured.
- Active font that owns the obligation.

Coverage obligations may share a font and a logical JSON case. They must not
share an unexplained input combination.

## Font And Glyph World Matrix

The exhaustive inventory must classify fonts and selected glyphs across these
dimensions. The inventory is descriptive; it does not imply cross
multiplication.

| Dimension | Required distinctions |
|---|---|
| Container | SFNT TrueType, CFF/OpenType, TTC/collection, Type 1, bitmap-only, malformed/unsupported |
| Outline storage | simple `glyf`, composite `glyf`, recursive composite, CFF cubic, empty outline, no outline |
| Glyph topology | line, conic, cubic, mixed contours, holes, overlap, reversed winding, off-curve starts, degenerate contours |
| Metrics | proportional, fixed width, vertical, linear advance, hdmx, kerning, negative bearings, zero advance, extreme bbox |
| Hinting | no programs, native `fpgm`/`prep`/glyph bytecode, autohint, no hinting, tricky flag, gasp behavior |
| Character maps | BMP, supplementary plane, symbol, legacy platform encodings, format 4/12/14, missing codepoint, glyph zero |
| Scripts | Latin, Greek, Cyrillic, CJK, Indic families, Arabic-like joining, Hebrew, Southeast Asian, historic/supplementary |
| Render output | gray, mono, LCD, LCD_V, light, SDF/error behavior, positive/negative pitch, empty bitmap |
| Embedded images | fixed strikes, grayscale bitmap, monochrome bitmap, color bitmap, missing requested strike |
| Color | COLR/CPAL, layered glyphs, root transforms, SVG, control font without color tables |
| Variation | fvar, named instance, default/non-default coordinates, avar mapping, variation selector cmap |
| Face metadata | family/style names, bold/italic flags, glyph names, fixed width, vertical, scalable, SFNT, multiple masters |
| Error construction | truncated table, invalid offset/length, malformed contour, invalid glyph index, unsupported format, null/empty input |

For every active font, the inventory must separately list table presence and
the exact selected glyphs. A font supporting a script does not prove that an
arbitrary selected glyph exercises that script's autohint or outline path.

## Intended Active Corpus Shape

The final number is determined by distinct font-level properties, not by a
preselected quota. The working target is no more than 12 scalable custom fonts
plus narrowly scoped bitmap, color, variation, Type 1, collection, and malformed
fixtures that cannot be represented safely in the scalable set.

The corpus should converge toward these ownership roles:

| Role | Purpose |
|---|---|
| Core TrueType | simple/composite outlines, native bytecode, names, kern, common charmaps, representative metrics |
| Multiscript autohint | small selected glyph set spanning script classes and writing systems |
| Vertical/CJK | vertical metrics, CJK topology, vertical layout, supplementary cmap where needed |
| Indic/complex scripts | representative glyph geometry for Indic and related autohint script branches |
| Metrics edge cases | fixed width, hdmx, zero/negative/extreme metrics, linear design behavior |
| Variable | fvar/avar, named instances, default and non-default coordinates |
| Color/SVG | COLR/CPAL and SVG public paths with a non-color control |
| Embedded bitmap | fixed sizes, mono/gray/color strikes and unavailable-size errors |
| CFF/Type 1 | cubic outlines, PostScript metadata, encoding and module-specific behavior |
| Collection | multiple faces and face-index/named-instance boundary behavior |
| Malformed family | minimal deterministic corruptions for parser and public error lines |

Do not merge font-level properties when the resulting font becomes difficult to
inspect, regenerate, or reason about. Reducing file count is secondary to
making each file's coverage role explicit.

## Explicit Input Contract

The active JSON model is grouped explicit variants:

```json
{
  "case_id": "freetype.FT_Load_Glyph.matrix_load",
  "subject": "freetype.FT_Load_Glyph",
  "case": "matrix_load",
  "operation": "freetype.load_glyph",
  "inputs": {
    "variants": [
      {
        "id": "native-simple-gray",
        "assets": {
          "font": { "kind": "ref", "id": "fonts/autohint/basic-latin.ttf" }
        },
        "params": {
          "glyph_index": 2,
          "pixel_size": 20,
          "load_flags": 4
        }
      },
      {
        "id": "composite-no-hinting",
        "assets": {
          "font": { "kind": "ref", "id": "fonts/autohint/basic-latin.ttf" }
        },
        "params": {
          "glyph_index": 7,
          "pixel_size": 13,
          "load_flags": 6
        }
      }
    ]
  }
}
```

The exact field shape must follow the validated runner schema. The important
contract is that each variant is complete and independently understandable.
Arrays inside a variant are operation data only; they must not silently become
axes.

`doc/unified_fixture_inputs.md` and
`doc/unified_fixture_migration_checklist.md` still describe the retired
variability-axis migration and must be reconciled in Phase 1.

## Evaluated Remaining Work

Evaluation checkpoint: 2026-07-13, latest verified unified condition-coverage run.

This is the active coverage identification ledger. It supersedes earlier
percentages in this section but does not replace the historical progress ledger
below. The unified public API suite currently has 4,165 logical cases, 6,787
concrete explicit cases, 6,784 runnable exact-parity cases, three explicit
pending obligations, and zero implicit cases.
`FT_Get_Postscript_Name.variation_instance_name_behavior` remains an active
parity row backed by real `FT_Set_Named_Instance` behavior, while
`ftmm.set_named_instance` now has direct selection, clear, and invalid-index
parity rows. The pending rows are Adobe MM named-instance reset, namedstyle
coordinate parity through `FT_MM_Var`, glyph-output deltas that require
`gvar`/HVAR support. The non-SFNT name path is runnable through the compact
Type 1 face fixture.

Core Rust structural coverage from
`make -C pillow-rs-freetype test-unified-condition-coverage` is:

| Measure | Covered | Total | Remaining |
|---|---:|---:|---:|
| Functions | 1,091 | 1,235 | 144 |
| Lines | 16,864 | 18,743 | 1,879 |
| Regions | 24,241 | 26,968 | 2,727 |
| Branches/conditions | 4,022 | 4,730 | 708 |

Formal MC/DC is not reported by the installed Rust coverage tooling
(`mcdc.count == 0`). Branch/condition coverage is therefore the instrumented
measure, and each compound predicate still needs explicit independent-effect
fixture obligations.

The remaining coverage divides exactly into these ownership groups:

| Group | Modules | Missing functions | Missing lines | Missing regions | Missing branches | Primary action |
|---|---|---:|---:|---:|---:|---|
| Face/API/scaler/FFI/SFNT metadata | `font.rs`, `scaler.rs`, `api.rs`, `ffi/handles.rs`, `ffi/convert.rs`, `ffi/types.rs`, `tt/name.rs`, `tt/post.rs`, `tt/cmap.rs`, `tt/gasp.rs`, `tt/fvar.rs` | 56 | 448 | 544 | 144 | public routing, wrapper thinness, metadata/state inputs |
| Rendering | `render.rs`, `grays.rs`, `outline.rs` | 48 | 712 | 1,012 | 128 | render topology, mode, clipping, pitch, SDF, and bitmap rows |
| Autohint | `latin.rs`, `cjk.rs`, `globals_data.rs`, `types.rs`, `coverage.rs`, `globals.rs`, `loader.rs` | 13 | 619 | 793 | 354 | script reachability audit, then glyph topology rows |
| TrueType interpreter | `tt/hinter/exec.rs`, `gs.rs`, `mod.rs`, `zone.rs`, `iup.rs`, `tt/mod.rs` | 3 | 32 | 219 | 57 | explicit bytecode-program glyph rows |
| Math/casts | `fixed.rs`, `casts.rs` | 0 | 0 | 3 | 6 | scalar boundary rows or semantic cleanup |

Per-file source gap ledger:

| Source | Missing lines | Line coverage | Missing funcs | Missing regions | Missing branches |
|---|---:|---:|---:|---:|---:|
| `src/render.rs` | 551 | 1721/2272 (75.75%) | 43 | 790 | 75 |
| `src/autohint/latin.rs` | 290 | 2538/2828 (89.75%) | 3 | 559 | 276 |
| `src/autohint/globals_data.rs` | 230 | 63/293 (21.50%) | 1 | 117 | 0 |
| `src/font.rs` | 227 | 2015/2242 (89.88%) | 39 | 338 | 44 |
| `src/grays.rs` | 160 | 650/810 (80.25%) | 5 | 221 | 50 |
| `src/scaler.rs` | 153 | 1073/1226 (87.52%) | 13 | 133 | 30 |
| `src/autohint/cjk.rs` | 48 | 893/941 (94.90%) | 1 | 60 | 45 |
| `src/autohint/types.rs` | 32 | 71/103 (68.93%) | 7 | 25 | 1 |
| `src/ffi/handles.rs` | 33 | 1839/1872 (98.24%) | 1 | 48 | 35 |
| `src/tt/hinter/exec.rs` | 27 | 1352/1379 (98.04%) | 3 | 204 | 34 |
| `src/api.rs` | 26 | 754/780 (96.67%) | 1 | 30 | 27 |
| `src/tt/cmap.rs` | 14 | 726/740 (98.11%) | 3 | 14 | 0 |
| `src/autohint/globals.rs` | 11 | 214/225 (95.11%) | 1 | 17 | 14 |
| `src/tt/fvar.rs` | 7 | 91/98 (92.86%) | 4 | 13 | 0 |
| `src/ffi/convert.rs` | 6 | 145/151 (96.03%) | 0 | 6 | 0 |
| `src/tt/hinter/iup.rs` | 2 | 98/100 (98.00%) | 0 | 2 | 6 |
| `src/tt/post.rs` | 1 | 96/97 (98.97%) | 0 | 6 | 1 |
| `src/tt/gasp.rs` | 2 | 45/47 (95.74%) | 2 | 6 | 0 |
| `src/autohint/loader.rs` | 1 | 226/227 (99.56%) | 0 | 2 | 2 |
| `src/tt/hinter/gs.rs` | 1 | 185/186 (99.46%) | 0 | 1 | 0 |
| `src/tt/hinter/mod.rs` | 1 | 277/278 (99.64%) | 0 | 6 | 4 |
| `src/casts.rs` | 0 | 50/50 (100.00%) | 0 | 0 | 4 |
| `src/fixed.rs` | 0 | 215/215 (100.00%) | 0 | 3 | 2 |
| `src/outline.rs` | 0 | 3/3 (100.00%) | 0 | 0 | 1 |
| `src/tt/hinter/zone.rs` | 0 | 37/37 (100.00%) | 0 | 0 | 0 |
| `src/tt/name.rs` | 0 | 333/333 (100.00%) | 0 | 2 | 10 |

The exact line-range inspection artifact for the latest run is generated at
`target/coverage/unified-condition-missing-lines.txt` by
`make -C pillow-rs-freetype test-unified-condition-coverage`. It is
intentionally not committed because `target/` is generated output; this table
is the source-controlled ownership view.

2026-07-13 supersession note: the `autohint/coverage.rs` row in this older
ledger is now closed by the Autohint Coverage-Bit checkpoint below. The
accumulator helpers are covered through an explicit assertion on an existing
public `FT_LOAD_FORCE_AUTOHINT` row, not by a standalone diagnostic test.

This concentration changes the execution strategy. More fonts alone cannot
close the report. Broad convenience/helper areas in `font.rs`, `api.rs`,
`render.rs`, and the remaining low-coverage autohint metadata modules must
first be classified as one of:

1. Required behavior already exposed by a manifest public operation.
2. Required behavior whose existing public operation does not yet delegate to
   the core implementation.
3. Private behavior reachable only after a missing font/table/glyph property is
   supplied.
4. Required behavior blocked by incomplete core implementation.
5. Duplicate, diagnostic-only, test-only, obsolete, or semantically invalid
   code that must be removed or feature-gated rather than artificially called.

No fixture is accepted for category 5. No new fixture test, JSON generator,
runtime discovery, glyph-index scan, or Cartesian axis is allowed.

### False-Green Adapter Ledger

These are not acceptable final coverage paths because the parity runner is
currently returning modeled values or routing a public ABI surface back to the
Rust leg. They must become real parity cases or explicit pending/failing work
before the 100% claim is trustworthy.

Resolved in the 2026-07-11 glyph-name batch:

- `freetype.get_glyph_name` now parses supported SFNT `post` glyph names,
  compares exact return status and output-buffer bytes through the C oracle,
  Rust core, C ABI, and WASM ABI, and uses active glyph-name/no-name fixture
  fonts.
- `freetype.get_name_index` now compares exact glyph-index lookup and zero
  sentinel behavior for known, unknown, unavailable, null-face, and null-name
  inputs through all three ABI legs.

Resolved in the 2026-07-11 gasp batch:

- `ftgasp.get_gasp` now parses optional SFNT `gasp` tables in core, compares
  null-face, no-table, version 1 range selection, after-last-range sentinel,
  version 0 high-bit masking, unsupported-version optional-table behavior,
  stream reads beyond the SFNT record length, physical short headers, and
  truncated ranges through the C oracle, Rust FFI, C ABI, and WASM ABI.
- The old `tests/fixtures/fonts/gasp/*` symlink aliases to `DejaVuSans.ttf`
  were replaced by seven generated compact fonts from
  `scripts/build_gasp_fixtures.py`, rebuilt with `make font-fixture-gasp`.

Resolved in the 2026-07-11 cmap batch:

- `tttables.get_cmap_format` now compares real `FT_Get_CMap_Format` rows for
  format 4, 6, 12, 14, null, and out-of-range-to-null variants through the C
  oracle, Rust FFI, C ABI, and WASM ABI.
- `tttables.get_cmap_language_id` now compares nonzero format 4/6/12 language
  fields, format 14's `0xFFFFFFFF` sentinel, and null/default zero behavior
  through the same four surfaces.
- The compact `fonts/cmap/cmap-format-language-matrix.ttf` fixture is generated
  from the source-backed hinter matrix by `scripts/build_cmap_fixtures.py` and
  rebuilt with `make font-fixture-cmap`. Its format-14 subtable now carries
  `FE00`, `FE0F`, and `E0101` selector records, including default, non-default,
  and empty-selector UVS behavior.

| Public operation | Current runner behavior | Why it is unsafe | Required path |
|---|---|---|---|
| Generic `oracle_fallback_args` rows | Returns `Unimplemented_Feature` for unmatched operations | Correct only for intentionally unsupported public surfaces | Audit each remaining fallback row against `manifest.yaml`; real implemented operations need explicit match arms in oracle, Rust, C ABI, and WASM ABI |

### R0 False-Green Route Audit Snapshot

Recorded from the active public input JSON on 2026-07-14. This is the current
source-level route audit from `tests/unified_fixture_parity.rs`; it identifies
the remaining categories that can still produce a green result without proving
the intended public behavior.

Updated R0 evidence on 2026-07-13: `make -C pillow-rs-freetype route-audit`
now generates `target/api-abi-audit/route_audit.json` and
`target/api-abi-audit/route_audit.md` from the maintained public input JSON.
The report expands grouped variants into the same concrete row model used by
the unified fixture runner and classifies each row as real parity,
real null-validation, compile/header contract, shape-incomplete fallback,
generic fallback, null-error fallback, void fallback, explicit unsupported, or
pending core work. This is an audit report only; it does not execute fixtures,
generate JSON, or change comparisons.

Current route-audit totals:

| Route category | Concrete rows | Required disposition |
|---|---:|---|
| Real C/Rust/C-ABI/WASM parity route | 3,572 | Use these rows for structural coverage evidence. |
| Real null-validation route | 8 | `FT_New_Size`, `FT_Done_Size`, `FT_Activate_Size`, `FT_OpenType_Validate`, and `FT_OpenType_Free` null rows execute pinned C oracle status checks and wrapper validation; size lifecycle null rows now use direct C/WASM lifecycle exports, and success rows live in real parity. |
| Wrapper null-validation route | 1 | `FT_Get_SubGlyph_Info` null-output rows intentionally validate the thin Rust/C/WASM wrapper guard after a native-C proof row establishes the composite slot state. |
| Raw-slot null-validation route | 4 | Runtime rows intentionally validate raw glyph-slot pointer handling after a concrete slot state is established. |
| Compile/header/scalar contract | 2,229 | Valid for ABI/header contracts, not runtime core coverage. |
| Shape-incomplete fallback | 0 | Keep this at zero; future incomplete declarations must become executable variants or explicit pending rows in the same change. |
| Generic modeled fallback | 884 | Classify operation-by-operation as real parity, unsupported, or pending. |
| Generic modeled error fallback | 139 | Replace implemented surfaces with real error-path execution. |
| Null-error fallback | 7 | Keep only exact null-handle probes; route implemented null cases directly. |
| Void fallback | 2 | Replace with real null/noop wrapper rows or classify as void API contract. |
| Explicit unsupported | 6 | Keep only where the public surface is intentionally unsupported. |
| Pending core | 5 | Convert to runnable parity when the named dependencies or compact fixtures exist. |
| Explicit unsupported stubs | 6 | Implement or keep visibly unsupported; do not count as coverage. |
| Pending core implementation | 5 | Adobe MM named-instance reset, `FT_MM_Var` namedstyle coordinates, `gvar`/HVAR glyph-output deltas, synthetic unloaded/unsupported render-slot states, and MVAR table variation rows remain pending. |

The former shape-incomplete ftsynth bitmap declarations are now exact real
parity rows through the compact format-1 SBIT strike.  They should remain in
the active fixture set because they prove bitmap mutation and metrics side
effects rather than only slot-metric placeholders.

| Route | Current behavior | Coverage risk | Required disposition |
|---|---|---|---|
| `oracle_fallback_args` default | Emits a generic FreeType error for any operation that reaches the default `_other` arm | A newly implemented public operation can still pass by agreeing with a modeled error | Every operation that reaches this path must be listed as intentionally unsupported, pending implementation, or converted to a real oracle arm |
| `oracle_fallback_args` null-operation classifier | No-font `expect_error` rows can be converted into classified null-handle errors | Valid only for pure null-handle probes; unsafe for operations whose failure depends on loaded face state | Keep only when the public C call is exactly a null-handle classification |
| No-asset non-error void route | Some null/no-asset non-error rows return `--void` / `{"void": true}` | Can hide missing wrapper behavior because no state or output is compared | Audit each row; either route through the real public wrapper or mark as a deliberately void API contract |
| Global Rust `_` fallback | Returns `FT_Err_Unimplemented_Feature` for unmatched operations | Rust core coverage cannot improve through this path and parity is only error agreement | Convert implemented operations to explicit Rust FFI handlers; leave unsupported optional modules visibly unsupported |
| C ABI / WASM `_other` fallback | Falls through to the Rust FFI runner for unsupported binding operations | Thin-wrapper coverage is not proven when the C/WASM leg never calls its public export | For every retained public C/WASM symbol, add direct wrapper execution or mark the symbol as intentionally Rust-only/test-only |
| C ABI / WASM explicit Rust delegation | Constants, layout probes, compile probes, several SFNT table routes, transforms, reference-face, unsupported stubs, size helpers, and `freetype.new_face` are routed to Rust | Acceptable for compile-time/header probes; unsafe for runtime public functions that should exercise ABI pointer handling | Split into compile-contract probes versus runtime ABI obligations; runtime functions need direct thin-wrapper rows |
| `ftsizes` lifecycle rows | Null validation and non-null sequence rows now execute pinned C oracle commands, Rust FFI, direct C ABI exports, and direct WASM ABI exports for secondary-size allocation, activation, destruction, active-size fallback, and `FT_Select_Size` active-size mutation | Future size regressions could hide in generic model rows if new lifecycle cases are not routed directly | Keep all implemented size rows in real parity; any new lifecycle case must call the public Rust/C/WASM wrappers directly |
| Explicit Rust unsupported stubs | `freetype.face_properties` returns `Unimplemented_Feature` directly | This is a public FreeType surface; final 100% correctness cannot treat it as covered behavior | Implement exact public behavior or keep manifest rows visibly pending/failing until implementation exists |
| Shape-incomplete fallback guards | Current route audit reports zero rows | These previously indicated declarative input that the runner did not execute | Keep this category at zero; future incomplete declarations must become executable variants or explicit pending rows in the same change |
| Closed named-instance row | `freetype.FT_Get_Postscript_Name.variation_instance_name_behavior` now executes real `FT_Set_Named_Instance` before `FT_Get_Postscript_Name` | This removed the last pending row, but also introduced honest `fvar` and named-instance parsing coverage obligations | Continue metadata coverage through explicit named-instance, name table, and malformed-`fvar` rows rather than hiding the new denominator |
| Direct `ftmm.set_named_instance` rows | Selection, clear, and invalid-index compact variable cases now execute pinned C oracle, Rust FFI, C ABI, and WASM ABI paths | The remaining Adobe MM, `FT_MM_Var`, and glyph-output rows are real implementation gaps, not runner coverage | Keep those rows explicit pending until Adobe MM design coordinates, namedstyle coordinates, and `gvar`/HVAR deltas are implemented in core |

R0 is not complete until this snapshot is replaced by an operation-by-operation
table with zero unclassified generic fallback rows for implemented surfaces.
The false-green audit should be run before any new font expansion batch because
extra font rows cannot cover code that is bypassed by modeled runner output.

### Full Coverage Identification Path

The remaining 100% coverage work must proceed in this order. Each item produces
repo-visible evidence before more fixture rows are added.

1. Regenerate line/region/branch coverage with the unified public API parity
   test and treat the generated denominator as authoritative for the batch.
2. Classify every uncovered function/line into one of five buckets: already
   public but missing input, public but routed through a false-green adapter,
   private but reachable from a named table/glyph/font property, blocked by an
   incomplete core implementation, or duplicate/diagnostic/obsolete code that
   needs separate disposition.
3. Remove false-green adapters before adding coverage-only rows. The runner may
   not return modeled values for an implemented public API; each implemented
   surface needs C oracle, Rust FFI, C ABI, and WASM ABI execution.
4. Convert ignored declarative arrays into explicit row arrays inside existing
   public input JSON. Do not reintroduce Cartesian expansion or glyph scans.
5. Extend compact source-backed fonts only for named table/glyph properties
   required by uncovered code. Reuse the current semantic containers before
   adding a new font identity.
6. For each uncovered branch or condition, add the smallest public input row
   that proves both behavior and exact C/Rust parity. If parity fails, classify
   it as a core bug bucket instead of weakening the fixture.
7. After each batch, run narrow case tests, `test-unified-condition-coverage`,
   `test-ffi-compat`, `fontdone-ffi`, `fmt`, and repo-map checks. The final
   claim requires zero pending public rows and no false-green fallback rows for
   implemented surfaces.

Coverage identification is complete only when this document has a row-level
owner for every uncovered source line and function. The required fields for
each ownership row are: source file and line range, public manifest subject,
existing or new public input case, fixture font or table property, success or
error behavior, expected C-oracle comparison surface, and one of the five
classification buckets above. A raw missing-line list is not enough; every
entry must identify the public route that will execute it or the semantic proof
for not executing it.

The generated artifact
`target/coverage/unified-condition-missing-lines.txt` is the line-number input
for the identification pass. The checked-in output is this plan, not the
generated target file. After every coverage run, copy only the summarized
ownership delta into the tables here so the repo records which missing paths are
still real work.

The next identification batch is R0 completion:

| Artifact | Required output |
|---|---|
| False-green route audit | List every remaining `oracle_fallback_args` operation and mark it as implemented-real-parity, intentionally unsupported, or pending implementation |
| Uncovered function ledger | For each missing function, record public subject, owning JSON file, and whether it needs input, delegation, font data, core implementation, or semantic cleanup |
| Uncovered line ledger | For each high-volume file, collapse adjacent lines into behavior families and assign a fixture/font property before adding any rows |
| Branch/condition ledger | For each compound predicate family, record the minimal independent-effect inputs, not all Cartesian combinations |
| Pending ledger | Track the current three named-instance pending rows explicitly; any newly identified unsupported surface must be explicit pending/unsupported rather than silently modeled |

Concrete queue from the current denominator:

1. Audit every remaining `oracle_fallback_args` route. If the operation is
   implemented, add typed C oracle, Rust FFI, C ABI, and WASM ABI execution; if
   it is not implemented, leave it visibly pending or unsupported.
2. Finish public wrapper routing in `api.rs`, `font.rs`, and `ffi/handles.rs`.
   These rows should mostly reuse existing fonts and expose already-maintained
   public methods instead of adding glyph data.
3. Close render/grays/scaler gaps with explicit mode/topology rows over the
   existing render and hinter-control fonts. Add a glyph only when the uncovered
   branch names a geometry not already present.
4. Close autohint gaps by script/topology obligation: Latin blue/serif/link
   rows, CJK topology rows, and script-data reachability. Do not delete
   `globals_data` entries only to improve coverage.
5. Close TrueType interpreter gaps with one bytecode glyph role per remaining
   opcode/state family, then stop; do not multiply by sizes or scripts.
6. Add scalar/cast boundary rows and final malformed-table controls only after
   their missing lines are classified as publicly reachable or defensively
   unreachable.

### R1 Full Coverage Identification Route

The next objective is a closed coverage ledger, not just a higher percentage.
Every uncovered executable item must have one owner and one disposition before
it is considered understood. The route is:

1. Regenerate the denominator with
   `make -C pillow-rs-freetype test-unified-condition-coverage`.
2. Freeze the generated totals for the batch: lines, functions,
   instantiations, regions, and branch/condition outcomes.
3. Classify every generic fallback in the public runner before adding more
   fonts. A green modeled error is not fixture parity.
4. For every uncovered function, identify the public subject that should reach
   it. If no public subject exists, classify it as defensive, obsolete,
   diagnostic, test-only, or missing public API.
5. Collapse adjacent uncovered line numbers into behavior families. A behavior
   family must name the source route, table/glyph/font property, and expected
   C-oracle comparison.
6. For every uncovered branch/condition family, write the minimal
   independent-effect input set. Do not use font or glyph discovery to find
   combinations at runtime.
7. Add or adjust only existing public API input JSON rows and compact
   source-backed fonts. Do not add a second fixture harness and do not
   reintroduce Cartesian axes.
8. Run the narrow public case, then the full unified condition coverage suite,
   then ABI/export gates. Update this document with the closed rows and the new
   denominator.

An ownership row is closed only when it contains all of:

| Field | Requirement |
|---|---|
| Source | File plus line range or function name from the coverage artifact |
| Public route | Manifest subject and existing/new public input JSON case |
| ABI surfaces | C oracle, Rust FFI, C ABI, and WASM ABI disposition |
| Fixture property | Exact font, table, glyph role, size, flags, render mode, transform, or malformed condition |
| Expected behavior | Success/error shape and exact comparison surface |
| Coverage kind | Lines, functions, regions, branches/conditions, or defensive-unreachable |
| Verification | Narrow command and full coverage command that closed the row |

No code may be removed only to improve coverage. Code removal is valid only
after the route audit proves the code is obsolete, duplicate, test-only,
diagnostic, or semantically unreachable, and that disposition is documented
before the change.

#### R1 Blocking Routes

These are the current high-risk blockers that must be resolved before the
coverage score can be trusted as correctness evidence.

| Blocker | Current state | Required closure |
|---|---|---|
| Generic oracle fallback | Any unmatched operation can still return a modeled FreeType error | Produce an operation table where every implemented public operation has a real C oracle arm; unsupported operations remain explicit pending/unsupported rows |
| Rust `_` fallback | Any unmatched Rust route returns `Unimplemented_Feature` | Add direct Rust FFI execution for implemented surfaces; leave unsupported optional modules visibly unsupported |
| C ABI/WASM runtime delegation | Several runtime operations still fall through to the Rust leg instead of the public C/WASM wrapper | Split compile/header probes from runtime ABI obligations; runtime symbols must call their thin wrapper |
| `freetype.select_size` | Closed; real parity over compact SBIT strike | Keep future fixed-strike cases explicit and sequence-based; do not reintroduce embedded-strike placeholders or Rust-only proof paths |
| `freetype.face_properties` | Explicit unsupported stub | Audit FreeType property tags, implement supported public behavior, and keep unsupported tags exact-error visible |
| Shape-incomplete JSON rows | Some rows lack selectors such as glyph, offset, size, or index and fall into fallback paths | Convert valid rows into complete explicit variants or mark invalid declarations pending/unsupported |
| Named-instance PostScript row | Closed; the row is active and exact across Rust, C ABI, and WASM ABI | Keep future variation rows explicit and sequence-based; do not make them implicit axes |
| Backend error coercion | Some backend setup errors are converted to `Unimplemented_Feature` | Keep only for documented unsupported backend limitations; real implemented paths must fail loudly |

`FT_Get_SubGlyph_Info` is now a closed R1 implementation route. It compares
raw component index, flags, arguments, and transform through the compact glyf
component matrix and covers word args, point attachment, uniform scale, xy
scale, two-by-two transform, rounded/use-my-metrics flags, component
instructions, out-of-range subglyphs, non-composite slots, and null-slot
errors. Future subglyph rows should be added only when they expose a new C
FreeType behavior or a remaining uncovered branch; this route is no longer a
false-green adapter.

#### R1 Workstream Order

The implementation work should be split by source ownership after the route
audit, not by font. Each stream owns one failure class and must update this
ledger before it adds rows.

| Order | Stream | Primary files | Identification output | Completion gate |
|---:|---|---|---|---|
| 0 | Denominator and fallback audit | `tests/unified_fixture_parity.rs`, input JSON, manifest | Operation-by-operation table with zero unclassified generic fallbacks for implemented surfaces | Full suite keeps exact runnable parity; the current three pending rows remain classified by core implementation gap |
| 1 | Public wrapper/API closure | `api.rs`, `font.rs`, `ffi/handles.rs`, `ffi/convert.rs`, `ffi/types.rs` | Missing functions mapped to public subjects or defensive disposition | No modeled runtime wrapper result for implemented C/WASM symbols |
| 2 | Composite/size/metadata features | `font.rs`, `scaler.rs`, `tt/name.rs`, `tt/post.rs`, `tt/cmap.rs` | Compact table/glyph rows for subglyphs, strikes, names, charmaps, metadata errors | Real C/Rust/C/WASM parity rows replace stubs |
| 3 | Render/scaler topology | `render.rs`, `grays.rs`, `scaler.rs`, `outline.rs` | Explicit render-mode/topology rows: gray, mono, LCD/LCD_V, empty, clipped, transformed, cubic/conic/degenerate | Pixel/bitmap/placement parity exact for every added row |
| 4 | Autohint script coverage | `autohint/latin.rs`, `autohint/cjk.rs`, `autohint/globals_data.rs`, `autohint/types.rs`, `autohint/coverage.rs` | Script and glyph-role matrix: Latin blue/serif/link/diagonal, CJK topology, Indic/other script activation | No deletion of script data; rows prove reachability or document unreachable data |
| 5 | TrueType VM coverage | `tt/hinter/exec.rs`, `gs.rs`, `zone.rs`, `iup.rs`, `tables.rs` | One bytecode glyph/program role per missing opcode or graphics-state branch | First-divergence parity for metrics/outline/render rows |
| 6 | Scalar and defensive cleanup | `fixed.rs`, `casts.rs`, residual parser guards | Boundary rows or semantic proof of unreachable overflow/invalid-state guards | Zero unclassified missing lines; cleanup commits explain why code is obsolete/unreachable |

After R1, subagents can be useful, but only with separate worktrees and one
owned stream each. The main branch should merge only verified commits that
report before/after counts, exact commands, changed files, and any C behavior
that shaped the fix.

#### R1 Final Gate

The final 100% claim requires all of these at the same commit:

- `test-unified-condition-coverage` reports zero missed executable lines,
  functions, regions, and branch/condition outcomes for the maintained core
  denominator, or every remaining item is documented as impossible to execute
  and removed/feature-gated in a separate semantic cleanup.
- Pending public rows are zero.
- Implemented public operations have no generic fallback or modeled-success
  route.
- C oracle, Rust FFI, C ABI, and WASM ABI routes either execute the real public
  wrapper or are explicitly compile/header-only probes.
- Active input JSON is explicit and grouped; implicit case count remains zero.
- New or changed fonts are compact, source-backed, inventoried, and tied to
  named obligations.
- `test-ffi-compat`, `fontdone-ffi`, formatting for touched crates, repo-map
  checks, and narrow parity commands pass.

### Remaining Case And Font Budget

The completion budget is deliberately conservative:

| Resource | Current | Completion ceiling | Rule |
|---|---:|---:|---|
| Concrete explicit cases | 6,560 | 7,016 | Add only named obligations, not product axes |
| Runnable parity cases | 6,557 | same as concrete | Retire pending rows only through real implementation |
| Pending cases | 3 | 0 | No symbolic final rows |
| New semantic font files | 17 in current metadata pass | review before adding more | Extend source-backed focused fonts first |
| New glyph programs/topologies | 0 in next pass | 160 | One glyph role per behavior family, not per glyph index |
| Implicit cases | 0 | 0 | Hidden discovery remains forbidden |

The 500-case allowance is a ceiling, not a target. A batch must justify every
variant by a named uncovered behavior. Existing focused fonts should be
extended before creating a new content identity.

At completion, consolidate the current 106 active unique font contents toward no
more than 30 inspectable semantic containers. The target shape is:

- One core TrueType topology/metadata matrix.
- One native TrueType bytecode program matrix.
- Up to three compact autohint script/topology fonts.
- One render topology font plus existing CFF/Type 1 controls.
- One fixed-strike bitmap font.
- Existing variable, color, collection, and Type 1/CFF controls.
- A small number of malformed collections or standalone physical-EOF controls.

Malformed fixtures may be bundled into TTC collections only when collection
wrapping does not change the behavior being tested. Physical-EOF and
top-level-directory corruptions remain standalone.

### Clear Path To 100%

The path is ordered so coverage cannot be inflated by fake runners or hidden
case growth.

| Order | Workstream | Primary files | Expected fixture/input work | Exit gate |
|---:|---|---|---|---|
| 0 | Freeze measurement and stale-data guard | coverage target and oracle cache | No new cases | Current JSON regenerated from content-hashed assets; gap table updated |
| 1 | Remove false-green adapters | `tests/unified_fixture_parity.rs`, core metadata/table helpers, thin C/WASM exports | Glyph-name, name-index, gasp, cmap format/language, and subglyph-info rows are now real parity; continue with generic fallback rows and the remaining modeled surfaces; no new discovery | No runnable public operation returns a canned value unless C also semantically does |
| 2 | Public route audit | `font.rs`, `api.rs`, `ffi/handles.rs`, `ffi/convert.rs`, `scaler.rs` | Reuse existing public API inputs; add variants only for missing states | Every uncovered public wrapper is either delegated, pending on core behavior, or semantically removed |
| 3 | Rendering/raster matrix | `render.rs`, `grays.rs`, `outline.rs` | Extend the source-backed render topology font; use `FT_Render_Glyph`, `FT_Glyph_To_Bitmap`, `FT_Outline_Render`, and load-with-render rows | All render modes have exact bytes, placement, pitch, metrics, and error parity |
| 4 | Autohint script and globals | `autohint/globals_data.rs`, `coverage.rs`, `types.rs`, `globals.rs` | Prefer existing Latin/Greek/Cyrillic, CJK, and Indic fonts; add script rows only when geometry differs | One authoritative script path; duplicate diagnostics removed only with semantic proof |
| 5 | Latin/CJK topology completion | `autohint/latin.rs`, `autohint/cjk.rs`, `loader.rs` | Add named glyph topologies for blue zones, tildes, serifs, linked edges, marks, diagonals, and counters | 100% autohint function/line/region/branch coverage through public force-autohint loads |
| 6 | TrueType interpreter edge programs | `tt/hinter/exec.rs`, `gs.rs`, `zone.rs`, `iup.rs`, `tables.rs` | Extend `hinter-control-matrix.ttx` with one program per remaining opcode/state family | Interpreter modules reach 100% structural coverage without per-opcode multiplication |
| 7 | Embedded strikes and named instances | face sizing, render dispatch, variation APIs | One real fixed-strike font; named-instance rows through `FT_Set_Named_Instance` | Pending count is zero and strike/named-instance behavior is real exact parity |
| 8 | Scalar and residual branch pass | `fixed.rs`, `casts.rs`, small leftover guards | Existing fixed/trigon/vector public inputs plus exact boundary rows | Every small module is complete or has documented semantic cleanup |
| 9 | Final consolidation | fonts, inputs, docs, validators | Move superseded active fonts to `deprecated/`; no deletion before approval | 100% functions, lines, regions, branches/conditions; zero implicit and pending cases; case count stays below ceiling |

For each workstream, the concrete workflow is:

1. Pick one uncovered family from the ledger above.
2. Identify the public manifest subject and existing input JSON that should own
   the behavior.
3. If the runner is modeled, make oracle/Rust/C ABI/WASM routing real first.
4. Add or extend the smallest focused font/glyph/table property required.
5. Add explicit grouped variants; do not add an axis or folder scan.
6. Run the narrow `make -C pillow-rs-freetype test-case CASE=<subject>` first.
7. Run full parity, `test-ffi-compat`, no-runtime-FFI, fmt, and nightly
   condition coverage.
8. Update this ledger with case delta, pass/pending count, and structural
   coverage delta before committing.

### Detailed Batch Notes

#### R-1: Oracle Cache Trust Gate

Status: complete.

Expected additions: zero fonts, zero cases.

The runtime face cache hashes font bytes, but the current C-oracle cache key
hashes canonical case JSON, oracle binaries, and argv paths without hashing the
resolved asset contents. Replacing a font at the same path can therefore reuse
stale C output.

1. Add every resolved file asset's byte length and SHA-256 identity to the
   oracle cache key in deterministic case/asset order.
2. Include referenced non-font binary assets used by an oracle operation, not
   only the primary font role.
3. Add a narrow cache-key regression check using the maintained unified fixture
   test infrastructure; do not create a separate fixture suite or JSON builder.
4. Force one oracle refresh after the fix and confirm all 6,413 runnable cases
   still pass.

Exit gate: changing any fixture byte at an unchanged path changes the C-oracle
cache key. Met: cache v3 hashes each case ID, ordered asset role, resolved path,
byte length, and SHA-256 digest, including inline assets. A forced refresh wrote
key `3b17c268174f6426a1f594fb4f00cdb06edb24bcd16f219f8c29bc65b0cf573f`;
the next run hit the same key and both runs passed 6,413 / 6,413.

#### R0: Public Reachability Audit

Expected additions: zero fonts, zero cases.

Status: in progress. Autohint audits have removed 30 uncovered functions: the
abandoned diagnostic bitmask, duplicate script and blue-zone entry points,
unused direction/contour helpers, no-caller serif wrapper, compiler-generated
option closures, and the second-bottom adjustment path. The latter had no entry
in `ADJUSTMENT_DATABASE`, so no public Unicode input could select it. The
retained runtime now has one script-selection path through `FaceGlobals`,
`STYLE_TABLE`, and `globals::detect_script`.

2026-07-13 reachability note, updated 2026-07-17: metadata parser guards should
not be assigned to new font rows without a public route. The former
`tt/post.rs` format 3.0 and unknown-format fallback arms mixed the public
`FT_FACE_FLAG_GLYPH_NAMES` gate with the private service behavior; core now
models those layers separately, leaving only the service invalid-index guard
that the public API preempts. Supported format 2.0/2.5 malformed rows cover the
public `.notdef` fallback behavior. The former `tt/cmap.rs`, `tt/fvar.rs`, and
`tt/gasp.rs` checked arithmetic closures were resolved on 2026-07-16 by
matching C's remaining-length validation, bounded header validation, and
direct count arithmetic; the maintained malformed public rows cover every
retained parser guard.

1. For every uncovered function, identify its public manifest operation and
   current call path.
2. Remove duplicate or obsolete internal wrappers and diagnostic coverage
   modules that are not part of runtime behavior.
3. Feature-gate intentionally test-only helpers.
4. Record functions that require actual public delegation separately from
   functions requiring font data.

Exit gate: every one of the 257 uncovered functions is classified; no fixture
work remains assigned to unreachable code.

#### R1: Native TrueType Bytecode Matrix

Primary modules: `tt/hinter/exec.rs`, `gs.rs`, `zone.rs`, `iup.rs`,
`mod.rs`.

Status: in progress. The source-backed control font now covers every retained
interpreter function with four additional glyphs and variants. Remaining work
is line, region, and condition completion within those functions.

Expected additions: extend `hinter-control-matrix.ttf`; at most two malformed
program derivatives; 35-45 named programs and 70-100 explicit variants.

1. Add one named glyph program per opcode family, not per opcode when one
   program can exercise the family safely.
2. Cover stack underflow/overflow, PUSH byte/word boundaries, FDEF/IDEF/CALL,
   storage, CVT, vectors, zones, reference points, rounding modes, DELTA,
   interpolation, scan controls, and instruction-control state.
3. Use separate glyphs for successful state transitions and public error
   outcomes. Do not combine independent failures in one program.
4. Select only sizes that change a ppem predicate, DELTA band, or rounding
   result.

Exit gate: interpreter modules reach 100% functions and lines, then 100%
regions and branches after unreachable guards are removed.

#### R2: Autohint Reachability And Script Dispatch

Primary modules: `autohint/globals_data.rs`, `globals.rs`, `types.rs`.

Expected additions: zero to one font; 10-20 explicit variants.

1. Determine whether the wholly uncovered script/coverage helpers duplicate
   the active style-class dispatch.
2. Remove or integrate them through existing public autohint operations.
3. Use the existing Latin/Greek/Cyrillic, CJK, and Indic fonts to prove script
   assignment and standard/blue character selection.

Exit gate: script dispatch has one authoritative runtime path and no uncovered
duplicate tables or diagnostic-only counters.

#### R3: Latin Autohint Topology Matrix

Primary module: `autohint/latin.rs`.

Expected additions: extend existing compact autohint fonts; 30-45 named glyph
topologies; 45-75 explicit variants.

Status: in progress. Nine explicit variants in the source-backed
`cjk-coverage.ttf` now own top, second-top, and bottom tilde adjustment plus
capital blue-edge suppression, single-reference IUP shifting, and mixed
flat/round blue calibration. A compact micro-serif also owns close-serif
overlap rejection. After restoring previously coverage-driven deletions,
`latin.rs` is at 70/73 functions, 2,506/2,828 lines, 3,607/4,207 regions,
and 974/1,282 branches. A shared-start reversal contour
covers both longer-segment selection outcomes; the rarer equal-direction
degenerate merge remains separately owned. The three existing tilde variants
pack both quadratic measurement directions and a no-stretch threshold without
adding another concrete case.

Required topology roles include serif/non-serif stems, linked/unlinked edges,
top and bottom tildes, accents, overshoots, holes, short/long segments,
degenerate contours, touching contours, multiple blue-zone candidates, and
positive/negative bearings.

Sizes are added only for small-size snapping, normal-size alignment, or a
specific branch threshold. Latin, Greek, and Cyrillic use distinct geometry;
Unicode aliases do not count.

Exit gate: `latin.rs` reaches complete structural coverage with every
special-case helper owned by a named glyph.

#### R4: CJK And Remaining Script Geometry

Primary modules: `autohint/cjk.rs` and shared autohint loader/types code.

Expected additions: extend `cjk-coverage.ttf` and
`indic-coverage.ttf`; 15-25 glyph roles; 25-45 explicit variants.

Status: in progress. Four Hani blue-string cmap aliases reuse distinct existing
CJK outlines as top/bottom fill and flat calibration candidates. They activate
blue scaling and linked-edge positioning without adding glyphs or cases;
`cjk.rs` is at 18/19 functions, 830/941 lines, 1,111/1,247 regions, and
337/426 branches. The remaining round-segment helper is retained and visibly
uncovered because pinned FreeType snapshots a zero segment limit before the
shared scanner.

Cover linked-edge position selection, round-segment marking, horizontal and
vertical stem combinations, enclosed counters, diagonal branches, zero-width
marks, and multi-contour ordering. Add a new script font only when the
algorithm selects a genuinely distinct writing-system class.

Exit gate: CJK and shared loader/type branches reach 100% without script-name
aliases standing in for geometry.

#### R5: Render And Raster Geometry Matrix

Primary modules: `render.rs`, `grays.rs`, `outline.rs`.

Expected additions: one scalable render-topology font; reuse existing
TrueType/CFF/Type 1/bitmap controls; 35-45 glyph roles; 60-100 explicit
variants.

Required roles include line/conic/cubic contours, upward/downward segments,
off-curve starts, consecutive controls, intersections, winding reversal,
degenerate contours, clipping, empty outlines, dropout modes, mono collapse,
overshoot, LCD/LCD_V filters, and SDF success/error behavior.

Preserve alternate render implementations, defensive paths, and convenience
entry points while classifying their reachability. Remove code only in a
separate cleanup with semantic proof independent of coverage; an uncovered
render path is a missing fixture obligation, not evidence that the behavior is
disposable.

Exit gate: every supported render mode has exact bytes, placement, pitch, and
metrics parity; unsupported modes have exact public errors.

#### R6: Face, Scaler, API, And Thin-Core FFI

Primary modules: `font.rs`, `scaler.rs`, `api.rs`,
`ffi/handles.rs`, `ffi/convert.rs`, `ffi/types.rs`.

Expected additions: reuse existing fonts; 40-70 explicit variants.

1. Map every uncovered public convenience method to an existing manifest
   operation or remove it if it duplicates the canonical path.
2. Cover size lifecycle, char-size versus pixel-size state, load-mode
   delegation, vertical layout, transforms, kerning, glyph names, SFNT table
   records, subglyph info, synthetic weight/slant, and null/output-pointer
   behavior.
3. Keep raw pointer and ABI record work in binding crates; core coverage must
   come from public behavior, not ABI-specific algorithms.

Exit gate: all retained public core methods are reached by fixture parity and
all wrappers remain thin.

#### R7: Embedded Strike Completion

Primary modules: face sizing, fixed-size selection, bitmap glyph loading, and
render dispatch.

Expected additions: one fixed-strike font; 8-20 explicit variants.

Status: pending real fixed-strike implementation. The previous symbolic
`first_available_size` expression has been replaced with an explicit value, and
the named-instance PostScript-name row is now exact parity. The current
embedded-strike asset is still a scalable-font alias, so real fixed-strike
support and successful/unavailable strike variants remain required. Do not
substitute a scalable font for the final R7 exit gate.

Exit gate: 0 pending cases, exact Rust/C/WASM strike parity, and exact
named-instance PostScript-name parity.

#### R8: Scalar, Cast, And Final Error Boundaries

Primary modules: `casts.rs` branch residuals, `fixed.rs` branch residuals, plus
residual small branches.

Expected additions: no fonts; 15-30 explicit variants.

Use existing fixed-math public API inputs for signed extremes, zero divisors,
rounding boundaries, normalization axes, and conversion limits. `fixed.rs` is
line-complete and `casts.rs` is line/function/region-complete; remaining work
there is branch outcome evidence or semantic reachability classification, not
missing wrapper-line coverage. The `i32_from_i64` and `usize_from_i64`
assertion failures are now classified as caller-invariant violations below;
the former `i16_from_i32` assertion was disproved by a valid full-range public
glyf route and replaced with C-compatible `FT_Short` narrowing.

Exit gate: every remaining small module reaches complete structural coverage.

#### R9: Final Sweep And Corpus Consolidation

Expected additions: 0-40 variants; no new semantic fonts.

1. Run line and nightly condition coverage and inspect every remaining region.
2. Resolve each gap with an explicit public input, implementation fix, or code
   removal.
3. Merge redundant valid font roles into the core matrices.
4. Bundle compatible malformed faces while preserving exact C behavior.
5. Move superseded active fonts into the deprecated area; do not delete them
   until the separately approved cleanup.
6. Re-run the full parity, ABI, FFI, formatting, lint, repo-map, and coverage
   gates after each consolidation.

Exit gate: 100% functions, lines, regions, and branches; zero pending and
implicit cases; no more than 6,914 concrete cases; active font corpus reduced
toward 30 unique semantic contents.

## Execution Phases

### Phase 0: Preserve The Explicit Baseline

Status: complete.

- Replaced implicit runtime Cartesian expansion with explicit grouped variants.
- Removed runtime folder discovery and all-glyph enumeration from public inputs.
- Established content-hashed runtime face-cache identity.
- Reduced the authoritative run to 6,314 concrete cases.
- Moved 100 old fonts into `tests/fixtures/deprecated/fonts_autohint/`.
- Added a deprecation policy and updated all live paths and symlinks.

Exit gate: 6,302 / 6,302 parity, 12 pending, zero implicit cases. Met.

### Phase 1: Freeze The Inventory And Documentation

Status: complete.

1. Reconcile the two obsolete unified-input documents with the explicit variant
   schema and remove instructions that encourage axis expansion.
2. Produce an exhaustive Markdown inventory of every active and deprecated font.
3. Record file size, format, face count, outline type, tables, charmaps, scripts,
   glyph count, variation/color/bitmap properties, and current reference sites.
4. For custom fonts, record every selected glyph's codepoint, glyph index,
   topology, contours, component structure, metrics, hinting program, and owned
   obligations.
5. Mark exact duplicates, supersets, and fonts with no unique current
   obligation. Do not delete them yet.
6. Record the seven deprecated fonts still used by public inputs as the first
   replacement queue.

Exit gate: every font has an inventory row and every active/deprecated public
reference maps to at least one named obligation.

### Phase 2: Establish Coverage Ownership

Status: in progress.

1. Run `make test-unified-coverage` without filters.
2. Run `make test-unified-condition-coverage` with the installed nightly toolchain.
3. Preserve both JSON reports under `target/coverage/` as uncommitted generated
   artifact and record summary numbers in this document.
4. Calculate completion totals from `pillow-rs-freetype/src/**` only. The C and
   WASM wrapper crates execute in every case but do not own parser/render logic.
5. Classify each uncovered function, line, region, branch, and atomic condition
   by module and public reachability.
6. Assign each reachable coverage group to a manifest case and required font/input
   property.
7. Classify obligations as success path, error path, state transition, format-specific,
   glyph-topology-specific, render-specific, or genuinely dead code.
8. For genuinely dead code, create a separate implementation cleanup item. Do
   not add artificial fixture inputs solely to call unsupported internals.
9. Prioritize groups that can retire a deprecated font or unlock many regions with
   one focused fixture feature.

Exit gate: every uncovered line has an owner, required input property, and
planned public operation, or is identified for code removal/refactoring.

### Phase 3: Replace The Seven Live Deprecated Dependencies

Status: complete.

Process one font obligation cluster at a time:

1. `DejaVuSansMono.ttf`: complete. `fixed-width.ttf` is now a 17 KiB focused
   font with post fixed-pitch metadata and uniform advances.
2. `DejaVuSans-Bold.ttf` and `DejaVuSans-Oblique.ttf`: complete. One compact
   bold-italic font enters both independent `head.macStyle` conditions.
3. `NotoSansMongolian-Regular.ttf`: complete. The compact CJK font owns the
   positive vertical flag and a vhea-only derivative owns the missing condition.
4. `Ubuntu.ttf`: complete. A 9 KiB focused subset retains fvar, avar, gvar,
   HVAR, STAT, both axes, and all 12 named instances.
5. `NotoSans-Regular.ttf`: complete. Both remaining references now use the
   existing active compact Noto font.
6. `DejaVuSans.ttf`: complete. A 143 KiB retain-GID fixture preserves the
   selected native-hint, outline, Latin/Greek blue-zone, touch-tag, kerning,
   post-name, and face-property obligations. One explicit existing public
   charmap case selects its controlled format-4 `idRangeOffset` mapping.

For each replacement, compare old and new C-oracle outputs before removing the
old variant. Output values do not need to match between different fonts; the
new font must enter the same intended code path and retain exact backend parity.

Exit gate: zero public API JSON references and zero active symlinks into
`deprecated/`.

### Phase 4: Expand Focused Fonts From Uncovered Code

Status: in progress.

Use the Phase 2 map in module-sized batches. For each batch:

1. Select uncovered lines with a common required font property.
2. Add the minimum table, glyph, corruption, or face property to an existing
   focused font when ownership remains clear.
3. Create a new focused font only when the property cannot be combined cleanly.
4. Add explicit input variants to existing public API JSON cases.
5. Include controls where branch meaning depends on presence versus absence of
   a table or feature.
6. Run narrow parity, full parity, stable coverage, then nightly condition coverage.
7. Keep the batch only when it adds the intended structural coverage or retires a
   documented obligation without reducing existing coverage.
8. Record the exact covered-line delta and concrete-case delta in the ledger.

Recommended batch order:

| Order | Coverage family |
|---:|---|
| 1 | Face metadata, sizing, charmaps, and table-presence branches |
| 2 | Simple, composite, recursive, empty, and malformed TrueType glyphs |
| 3 | Native TrueType bytecode and scaler state transitions |
| 4 | Autohint script classes and writing-system-specific branches |
| 5 | Gray, mono, LCD, LCD_V, light, and render error paths |
| 6 | Metrics, hdmx, vertical, fixed-size, and advance edge cases |
| 7 | CFF, Type 1, collections, variation, color, SVG, and bitmap formats |
| 8 | Parser truncation, invalid offsets, unsupported data, and lifecycle errors |

Exit gate: all publicly reachable core functions, lines, regions, branches, and
atomic conditions are covered and every added case has a named obligation that
records independent decision effect.

### Phase 5: Resolve Remaining Uncovered Lines And Pending Cases

Status: pending.

1. Convert the remaining pending case into a runnable explicit input where the
   public operation is implemented.
2. Re-run coverage and inspect every remaining uncovered function, line, region,
   branch, and atomic condition manually.
3. Add malformed or boundary fixtures for legitimate public error paths.
4. Remove or refactor dead private branches that cannot be reached from a
   supported public API.
5. Fix implementation behavior when a new C-oracle case exposes divergence.
6. Do not mark a manifest case covered until at least one exact parity variant
   passes through all three ABI paths.

Exit gate: 100% function, line, region, branch, and condition coverage for the
intended pure Rust core, explicit independent-effect evidence for compound
conditions, and no unexplained pending public cases.

### Phase 6: Remove Legacy Harness Assumptions

Status: pending.

After the fixture corpus is stable:

1. Remove obsolete variability terminology from docs, Makefile help, comments,
   and manifest descriptions.
2. Ensure validators reject implicit expansion fields and deprecated fixture
   paths in new public inputs.
3. Ensure reports always print logical cases, concrete cases, implicit cases,
   pending cases, and per-backend parity.
4. Keep multi-input grouped cases as the only supported variation mechanism.
5. Retain content-based oracle and face cache keys so fixture replacement cannot
   reuse stale outputs.

Exit gate: no runtime or documentation path suggests Cartesian discovery.

### Phase 7: Final Deprecated Corpus Cleanup

Status: pending.

1. Search all tracked text, JSON, manifests, tests, benchmarks, docs, and
   symlinks for `deprecated/` references.
2. Run all parity, API/ABI, no-runtime-FFI, formatting, and lint gates.
3. Record final font count, bytes, concrete cases, runtime, and all structural
   coverage measures.
4. Ask for explicit approval before deleting the 100-font deprecated directory.
5. Delete it in one reviewable change and rerun the same gates.

Exit gate: deprecated directory removed, active corpus documented, 100%
structural coverage retained, and exact parity unchanged.

## Batch Acceptance Gates

Every font or input batch must run these maintained workflows from
`pillow-rs-freetype/`:

```bash
make api-abi-check
make test-unified-fixtures
make test-unified-coverage
make test-unified-condition-coverage
make test-ffi
make fmt
make lint
```

Use a narrower maintained Make target first when a batch owns a specific lane.
An unrelated pre-existing lint failure may be reported, but files touched by the
batch must be clean.

Reject or revise a batch when any of these occur:

- Exact parity failures increase.
- Covered line count decreases without a corresponding intentional code removal.
- Concrete cases increase without named new obligations.
- A new font duplicates all properties and selected glyph behaviors of an
  existing active font.
- A deprecated font loses references before its obligations are reassigned.
- Runtime growth is disproportionate to new coverage.
- An input relies on implicit defaults that obscure the actual combination.

## Progress Ledger

Update one row after every verified batch. Covered lines are more important
than percentage because source line totals change as implementation is fixed.

| Date | Batch | Active content | Deprecated refs | Concrete | Runnable/pass | Pending | Covered/total lines | Result |
|---|---|---:|---:|---:|---:|---:|---:|---|
| 2026-07-10 | Explicit grouped-input migration | 28 unique hashes | 169 public references across 7 fonts | 6,314 | 6,302 / 6,302 | 12 | 12,000 / 16,287 | baseline established |
| 2026-07-10 | Move legacy corpus to `deprecated/` | 28 unique hashes | unchanged | 6,314 | 6,302 / 6,302 | 12 | unchanged | path isolation complete |
| 2026-07-10 | Nightly branch/condition baseline | 28 unique hashes | unchanged | 6,314 | 6,302 / 6,302 | 12 | 12,000 / 16,287 lines; 17,310 / 23,296 regions; 2,725 / 4,170 branches | condition instrumentation established |
| 2026-07-10 | Compact fixed-width replacement | 28 unique hashes | 168 references across 6 fonts | 6,314 | 6,302 / 6,302 | 12 | unchanged | removed deprecated symlink; 335 KiB replaced by 17 KiB |
| 2026-07-10 | Combined bold-italic style conditions | 29 unique hashes | 166 references across 4 fonts | 6,315 | 6,303 / 6,303 | 12 | 12,002 / 16,287 lines; 17,312 / 23,296 regions; 2,727 / 4,170 branches | +2 lines, +2 regions, +2 conditions from one variant |
| 2026-07-10 | Compact vertical and vhea-only control | 30 unique hashes | 165 references across 3 fonts | 6,316 | 6,304 / 6,304 | 12 | 12,009 / 16,294 lines; 17,316 / 23,300 regions; 2,728 / 4,170 branches | covered missing vmtx condition and corrected post-format-3 face flag parity |
| 2026-07-10 | Compact variable font and named instances | 31 unique hashes | 161 references to 1 font | 6,316 | 6,304 / 6,304 | 12 | 12,028 / 16,320 lines; 17,344 / 23,332 regions; 2,731 / 4,176 branches | 1 MiB Ubuntu replaced; encoded named indexes implemented |
| 2026-07-10 | Malformed fvar and named-index controls | 33 unique hashes | 161 references to 1 font | 6,319 | 6,307 / 6,307 | 12 | 12,036 / 16,321 lines; 17,348 / 23,333 regions; 2,736 / 4,178 branches | fvar parser reached 100% structural coverage |
| 2026-07-10 | Focused DejaVu obligation replacement | 34 unique hashes | 0 | 6,320 | 6,308 / 6,308 | 12 | 12,037 / 16,321 lines; 17,352 / 23,333 regions; 2,736 / 4,178 branches | final deprecated dependency removed; 143 KiB fixture exceeds old covered lines/regions and preserves condition outcomes |
| 2026-07-10 | maxp stream/version controls | 38 unique hashes | 0 | 6,324 | 6,312 / 6,312 | 12 | 12,050 / 16,322 lines; 17,359 / 23,334 regions; 2,738 / 4,172 branches | maxp reached 100% structural coverage; fixed below-1.0 acceptance and ignored-load-error behavior |
| 2026-07-10 | kern parser matrix | 43 unique hashes | 0 | 6,329 | 6,317 / 6,317 | 12 | 12,045 / 16,307 lines; 17,353 / 23,317 regions; 2,741 / 4,166 branches | kern reached 100% structural coverage; removed non-FreeType version and coverage behavior |
| 2026-07-10 | hdmx header and lookup matrix | 49 unique hashes | 0 | 6,337 | 6,325 / 6,325 | 12 | 12,053 / 16,306 lines; 17,361 / 23,315 regions; 2,750 / 4,166 branches | hdmx reached 100% structural coverage; one dead successful-search bounds region removed |
| 2026-07-10 | paired horizontal/vertical metric controls | 52 unique hashes | 0 | 6,341 | 6,329 / 6,329 | 12 | 12,064 / 16,302 lines; 17,367 / 23,313 regions; 2,755 / 4,164 branches | hmtx and vmtx reached 100% structural coverage; face opening now mirrors deferred FreeType metric reads |
| 2026-07-10 | physical-EOF metrics headers | 54 unique hashes | 0 | 6,343 | 6,331 / 6,331 | 12 | 12,070 / 16,302 lines; 17,372 / 23,315 regions; 2,757 / 4,164 branches | hhea and vhea reached 100% structural coverage; malformed present vhea now propagates C's error |
| 2026-07-10 | head, OS/2, post metadata controls | 57 unique hashes | 0 | 6,347 | 6,335 / 6,335 | 12 | 12,097 / 16,313 lines; 17,409 / 23,340 regions; 2,764 / 4,164 branches | head, OS/2, and post reached 100% structural coverage; face and size metrics now follow FreeType's selection order |
| 2026-07-10 | name and SFNT/TTC control matrices | 66 unique hashes | 0 | 6,356 | 6,344 / 6,344 | 12 | 12,151 / 16,312 lines; 17,484 / 23,330 regions; 2,788 / 4,164 branches | name and top-level SFNT parsing reached 100% structural coverage; fixed absolute TTC table offsets and removed one unreachable helper |
| 2026-07-10 | Explicit error-case asset completion | 66 unique hashes | 0 | 6,356 | 6,355 / 6,355 | 1 | 12,151 / 16,312 lines; 17,484 / 23,330 regions; 2,788 / 4,164 branches | 11 public error cases moved from pending to exact Rust/C/WASM comparison using existing focused SFNT and Type 1 assets |
| 2026-07-10 | cmap parser and lookup matrix | 74 unique hashes | 0 | 6,372 | 6,371 / 6,371 | 1 | 12,263 / 16,321 lines; 17,627 / 23,338 regions; 2,829 / 4,170 branches | cmap reached 100% structural coverage; fixed format-specific terminal iteration, format-12 zero-group advance, and format-4 range validation |
| 2026-07-10 | glyf topology and malformed matrices | 76 unique hashes | 0 | 6,407 | 6,406 / 6,406 | 1 | 12,366 / 16,333 lines; 17,743 / 23,345 regions; 2,855 / 4,164 branches | glyf reached 100% function, line, region, and branch coverage; fixed scaled point attachment, repeat overflow, invalid attachment handling, and simple instruction bounds |
| 2026-07-10 | short and long loca truncation | 78 unique hashes | 0 | 6,409 | 6,408 / 6,408 | 1 | 12,360 / 16,327 lines; 17,724 / 23,315 regions; 2,855 / 4,164 branches | loca reached 100% structural coverage; two checked record slices replaced twelve byte-position-specific regions |
| 2026-07-10 | empty and odd CVT controls | 80 unique hashes | 0 | 6,411 | 6,410 / 6,410 | 1 | 12,364 / 16,321 lines; 17,726 / 23,307 regions; 2,857 / 4,164 branches | hinter table parsing reached 100% structural coverage; removed unused fpgm/prep copy helpers |
| 2026-07-10 | hinter setup and scan-type controls | 81 unique hashes | 0 | 6,414 | 6,413 / 6,413 | 1 | 12,367 / 16,321 lines; 17,729 / 23,307 regions; 2,858 / 4,164 branches | one 1.8 KiB font covers empty fpgm/prep plus scan types 0 and 2; hinter/mod rises to 275/278 lines and 64/70 branches |
| 2026-07-10 | content-aware C-oracle cache | 81 unique hashes | 0 | 6,414 | 6,413 / 6,413 | 1 | unchanged | cache v3 hashes resolved file and inline asset bytes; forced refresh and subsequent cache hit both preserve exact parity |
| 2026-07-10 | Autohint reachability cleanup | 81 unique hashes | 0 | 6,414 | 6,413 / 6,413 | 1 | 12,365 / 16,005 lines; 17,727 / 23,069 regions; 2,857 / 4,124 branches | removed 15 uncovered duplicate or diagnostic functions and 316 unreachable lines; exact parity unchanged |
| 2026-07-10 | TrueType program control matrix | 81 unique hashes | 0 | 6,418 | 6,417 / 6,417 | 1 | 12,503 / 15,973 lines; 17,906 / 23,030 regions; 2,887 / 4,120 branches | four source-backed glyph programs add IDEF, UTP, super-round, and INSTCTRL coverage; all retained interpreter functions are covered |
| 2026-07-10 | TrueType stack and state program | 81 unique hashes | 0 | 6,419 | 6,418 / 6,418 | 1 | 12,550 / 15,973 lines; 17,994 / 23,030 regions; 2,903 / 4,120 branches | one 357-byte glyph program covers NPUSH, stack, arithmetic, storage/CVT, vector/zone, rounding, comparison, and scan-control families |
| 2026-07-10 | TrueType point geometry programs | 81 unique hashes | 0 | 6,422 | 6,421 / 6,421 | 1 | 12,575 / 15,973 lines; 18,079 / 23,030 regions; 2,912 / 4,120 branches | three glyphs explicitly own coordinate/vector, movement/interpolation, and point/CVT DELTA opcode families |
| 2026-07-10 | TrueType function and conditional flow | 81 unique hashes | 0 | 6,423 | 6,422 / 6,422 | 1 | 12,578 / 15,973 lines; 18,083 / 23,030 regions; 2,913 / 4,120 branches | one glyph explicitly owns FDEF/CALL/LOOPCALL and both conditional-jump outcomes; no further valid-flow variants justified |
| 2026-07-10 | TrueType malformed program errors | 81 unique hashes | 0 | 6,429 | 6,428 / 6,428 | 1 | 12,595 / 15,973 lines; 18,095 / 23,030 regions; 2,918 / 4,120 branches | six glyphs prove exact divide-zero, truncated-push, definition, and undefined-opcode errors across Rust/C/WASM |
| 2026-07-10 | Latin adjustment topology matrix | 81 unique hashes | 0 | 6,433 | 6,432 / 6,432 | 1 | 12,851 / 15,907 lines; 18,483 / 22,909 regions; 3,009 / 4,079 branches | four glyphs add 12 functions and broad tilde/blue coverage; eight no-caller or database-impossible functions removed |
| 2026-07-10 | Latin single-reference IUP topology | 81 unique hashes | 0 | 6,434 | 6,433 / 6,433 | 1 | 12,857 / 15,891 lines; 18,494 / 22,890 regions; 3,009 / 4,077 branches | one quadratic contour reaches the final uncovered Latin function; seven no-behavior closures/wrappers removed |
| 2026-07-10 | Latin shared-start segment reversals | 81 unique hashes | 0 | 6,435 | 6,434 / 6,434 | 1 | 12,888 / 15,891 lines; 18,539 / 22,890 regions; 3,018 / 4,077 branches | one zero-width reversal contour adds 31 lines, 45 regions, and 9 branches across segment retention/replacement |
| 2026-07-10 | Latin mixed flat/round blue calibration | 81 unique hashes | 0 | 6,437 | 6,436 / 6,436 | 1 | 12,907 / 15,878 lines; 18,570 / 22,873 regions; 3,036 / 4,075 branches | two round glyphs plus existing flat geometry add 19 lines and 18 branches; removed non-FreeType median-outlier heuristic exposed by lowercase metric parity |
| 2026-07-10 | Latin close-serif overlap topology | 81 unique hashes | 0 | 6,438 | 6,437 / 6,437 | 1 | 12,899 / 15,865 lines; 18,559 / 22,857 regions; 3,033 / 4,067 branches | one micro-serif owns overlap rejection; removed five impossible constructed-edge outcomes, reducing each uncovered structural gap by five |
| 2026-07-10 | Latin tilde measurement topology | 81 unique hashes | 0 | 6,438 | 6,437 / 6,437 | 1 | 12,937 / 15,865 lines; 18,608 / 22,857 regions; 3,067 / 4,067 branches | three existing tilde cases pack both quadratic measurement directions and a no-stretch threshold, adding 38 lines, 49 regions, and 34 branches with no case growth |
| 2026-07-10 | Hani blue calibration aliases | 81 unique hashes | 0 | 6,438 | 6,437 / 6,437 | 1 | 13,083 / 15,865 lines; 18,814 / 22,857 regions; 3,115 / 4,067 branches | four cmap aliases reuse existing CJK geometry and add 146 lines, 206 regions, 48 branches, and two functions with no glyph or case growth |
| 2026-07-10 | Coverage-deletion audit and restoration | 81 unique hashes | 0 | 6,438 | 6,437 / 6,437 | 1 | 13,097 / 16,243 lines; 18,846 / 23,196 regions; 3,129 / 4,126 branches | restored public autohint/VM/parser helpers, DOWN2/BOTTOM2 behavior, call-record contract, serif helper, and defensive guards; exact parity remains green with the honest larger denominator |
| 2026-07-10 | Deterministic source-backed font builds | 81 unique hashes | 0 | 6,438 | 6,437 / 6,437 | 1 | 13,097 / 16,242 lines; 18,846 / 23,196 regions; 3,129 / 4,126 branches | both maintained TTX targets preserve their embedded timestamps; rebuilds remain byte-identical after source mtime changes |
| 2026-07-10 | Render topology and SDF conic subdivision | 81 unique hashes | 0 | 6,447 | 6,446 / 6,446 | 1 | 13,236 / 16,301 lines; 19,092 / 23,292 regions; 3,187 / 4,148 branches | three glyphs and nine explicit modes add conic chains, mono/LCD variants, intersections, thin geometry, mixed winding, and degeneracy; the conic SDF case exposed and fixed Rust's non-FreeType subdivision rule with exact bytes across all ABIs |
| 2026-07-11 | Render empty, collapsed span, and dropout modes | 81 unique hashes | 0 | 6,460 | 6,459 / 6,459 | 1 | 13,255 / 16,301 lines; 19,111 / 23,292 regions; 3,202 / 4,148 branches | one source-backed font mutation adds five empty-outline modes, zero-width and zero-height collapsed spans, and two mono scan/dropout controls; exact Rust/C/WASM parity remains green with zero implicit cases |
| 2026-07-11 | Smart dropout scan modes | 81 unique hashes | 0 | 6,462 | 6,461 / 6,461 | 1 | 13,257 / 16,301 lines; 19,113 / 23,292 regions; 3,204 / 4,148 branches | two narrow glyph programs add scan types 4 and 5 to reach smart dropout selection without multiplying fonts, sizes, or render modes |
| 2026-07-11 | Public load-flag input cleanup | 81 unique hashes | 0 | 6,462 | 6,461 / 6,461 | 1 | 13,263 / 16,301 lines; 19,119 / 23,292 regions; 3,214 / 4,148 branches | replaced stale single-entry `load_flag_sets` inputs with executable `load_flags`; exact Rust/C/WASM parity remains green while existing cases now exercise their intended flag paths |
| 2026-07-11 | Request-size scale branch rows | 81 unique hashes | 0 | 6,462 | 6,461 / 6,461 | 1 | 13,267 / 16,301 lines; 19,122 / 23,292 regions; 3,217 / 4,148 branches | added three rows to the existing `FT_Request_Size` type matrix for CELL x-dominant scaling and SCALES single-axis zero fallback; exact Rust/C/WASM parity remains green with no concrete case growth |
| 2026-07-11 | No-scale vertical vmtx load | 81 unique hashes | 0 | 6,463 | 6,462 / 6,462 | 1 | 13,271 / 16,301 lines; 19,130 / 23,292 regions; 3,218 / 4,148 branches | one explicit `FT_Load_Glyph` variant reuses the compact CJK vertical fixture to prove `FT_LOAD_NO_SCALE | FT_LOAD_VERTICAL_LAYOUT` vmtx metrics; exact Rust/C/WASM parity remains green |
| 2026-07-11 | Executable kerning ppem variants | 81 unique hashes | 0 | 6,465 | 6,464 / 6,464 | 1 | 13,271 / 16,301 lines; 19,132 / 23,292 regions; 3,220 / 4,148 branches | converted the inert `size_ppem_values` list in `FT_Get_Kerning` into explicit 9, 20, and 32 ppem variants; exact Rust/C/WASM parity remains green and default kerning now covers the 25+ ppem no-downscale branch |
| 2026-07-11 | Latin adjustment aliases | 81 unique hashes | 0 | 6,467 | 6,466 / 6,466 | 1 | 13,274 / 16,301 lines; 19,138 / 23,292 regions; 3,223 / 4,148 branches | two cmap aliases in the compact source-backed CJK/autohint font add public `FT_LOAD_FORCE_AUTOHINT` rows for no-height-check and small-blue-ignore adjustment behavior; `latin.rs` gains 3 lines, 6 regions, and 3 branches with exact Rust/C/WASM parity |
| 2026-07-11 | Scaler conic bbox endpoint topology | 81 unique hashes | 0 | 6,468 | 6,467 / 6,467 | 1 | 13,276 / 16,301 lines; 19,140 / 23,292 regions; 3,224 / 4,148 branches | one explicit `FT_Load_Glyph` variant reuses an existing compact charmap font glyph whose contour starts off-curve and ends on-curve, covering the scaler exact-bbox endpoint branch; `scaler.rs` is now 915 / 1,201 lines, 1,047 / 1,254 regions, and 144 / 178 branches |
| 2026-07-11 | Render mono low-precision box | 81 unique hashes | 0 | 6,471 | 6,470 / 6,470 | 1 | 13,276 / 16,301 lines; 19,140 / 23,292 regions; 3,225 / 4,148 branches | three narrow `FT_Bitmap` render variants extend the compact hinter-control font with collapsed mono contours and a 130 px mono box; the low-precision mono selector branch is now covered with exact Rust/C/WASM parity |
| 2026-07-11 | Gray outline topology variants | 81 unique hashes | 0 | 6,474 | 6,473 / 6,473 | 1 | 13,346 / 16,315 lines; 19,269 / 23,316 regions; 3,241 / 4,150 branches | three `FT_Outline_Render` variants cover even-odd overlap, clipped cells, and cubic tags; `grays.rs` moved to 646 / 810 lines, 912 / 1,139 regions, and 131 / 184 branches with exact Rust/C/WASM parity |
| 2026-07-11 | CJK force-autohint mono render load | 81 unique hashes | 0 | 6,475 | 6,474 / 6,474 | 1 | 13,351 / 16,315 lines; 19,276 / 23,316 regions; 3,243 / 4,150 branches | one explicit `FT_Load_Char` variant reuses `cjk-coverage.ttf` U+7530 with `FT_LOAD_FORCE_AUTOHINT | FT_LOAD_RENDER | FT_LOAD_TARGET_MONO`; exact Rust/C/WASM parity remains green and CJK hint/render target handling gains 5 lines, 7 regions, and 2 branches |
| 2026-07-11 | Charmap and SFNT executable public paths | 81 unique hashes | 0 | 6,476 | 6,475 / 6,475 | 1 | 13,364 / 16,315 lines; 19,290 / 23,316 regions; 3,249 / 4,150 branches | converted stale SFNT table rows to concrete variants and routed Rust field-based charmap selection through the core path; +13 lines, +14 regions, +6 branches with exact Rust/C/WASM parity |
| 2026-07-11 | API monochrome target flag combinations | 81 unique hashes | 0 | 6,479 | 6,478 / 6,478 | 1 | 13,364 / 16,315 lines; 19,290 / 23,316 regions; 3,252 / 4,150 branches | three explicit `FT_Load_Glyph` render variants cover `api.rs` monochrome short-circuit outcomes for LCD, LCD_V, and LIGHT targets; exact Rust/C/WASM parity remains green |
| 2026-07-11 | Rust FFI lifecycle null-handle routing | 81 unique hashes | 0 | 6,479 | 6,478 / 6,478 | 1 | 13,366 / 16,315 lines; 19,292 / 23,316 regions; 3,254 / 4,150 branches | existing `FT_Done_Face` and `FT_Reference_Face` null-handle fixtures now call the thin Rust FFI handlers instead of a modeled shortcut; exact Rust/C/WASM parity remains green and `ffi/handles.rs` reaches 1,218 / 1,396 lines and 1,685 / 1,868 regions |
| 2026-07-11 | TrueType interpreter branch-edge glyph | 81 unique hashes | 0 | 6,480 | 6,479 / 6,479 | 1 | 13,378 / 16,315 lines; 19,311 / 23,316 regions; 3,260 / 4,150 branches | one source-backed `hinter-control-matrix.ttf` glyph adds zero-length vector, invalid stack-index fallback, taken JROF, NROUND, invalid SHC contour, and empty-twilight SHZ coverage through `FT_Load_Glyph`; remapped to glyph 51 after render glyphs 48-50 and `tt/hinter/exec.rs` reaches 1,222 / 1,340 lines, 2,463 / 2,901 regions, and 297 / 410 branches |
| 2026-07-11 | SFNT one-past-table boundary row | 81 unique hashes | 0 | 6,481 | 6,480 / 6,480 | 1 | 13,378 / 16,315 lines; 19,311 / 23,316 regions; 3,260 / 4,150 branches | merged the metadata worker's exact `TTAG_head` offset-55 boundary as a second explicit `FT_Load_Sfnt_Table` offset error case; exact Rust/C/WASM parity remains green and structural coverage is unchanged because the broader executable offset variant already covers the same guard |
| 2026-07-11 | Rust public render wrapper routing | 81 unique hashes | 0 | 6,481 | 6,480 / 6,480 | 1 | 13,393 / 16,315 lines; 19,332 / 23,316 regions; 3,260 / 4,150 branches | existing `FT_Render_Glyph` fixtures route the Rust leg through `Face::render_loaded_glyph` when the load did not already render the slot; exact Rust/C/WASM parity remains green and `api.rs` reaches 311 / 401 lines, 404 / 532 regions, and 35 / 45 functions |
| 2026-07-11 | Rust public load-char wrapper routing | 81 unique hashes | 0 | 6,481 | 6,480 / 6,480 | 1 | 13,396 / 16,315 lines; 19,340 / 23,316 regions; 3,260 / 4,150 branches | existing `FT_Load_Char` fixtures route the Rust leg through `Face::load_char` while retaining the same exact C/WASM comparison; `api.rs` reaches 314 / 401 lines, 412 / 532 regions, and 36 / 45 functions |
| 2026-07-11 | Rust public set-char-size wrapper routing | 81 unique hashes | 0 | 6,481 | 6,480 / 6,480 | 1 | 13,377 / 16,371 lines; 19,335 / 23,406 regions; 3,254 / 4,168 branches | successful `FT_Set_Char_Size` fixtures route the Rust leg through `Face::set_char_size`; exact Rust/C/WASM parity remains green, `api.rs` reaches 318 / 401 lines and 37 / 45 functions, and that coverage run exposed the need to treat fresh denominators as authoritative |
| 2026-07-11 | Real PostScript-name public API parity | 81 unique hashes | 0 | 6,481 | 6,480 / 6,480 | 1 | 13,460 / 16,391 lines; 19,448 / 23,457 regions; 3,279 / 4,194 branches | `FT_Get_Postscript_Name` no longer uses a fake `{"value":0}` runner path; static and null/no-name rows compare exact borrowed bytes/nullness through C oracle, Rust, C ABI, and WASM ABI. The named-instance row is now the only visible pending row for this subject until `FT_Set_Named_Instance` exists |
| 2026-07-11 | Coverage identification refresh | 81 unique hashes | 0 | 6,481 | 6,480 / 6,480 | 1 | 13,460 / 16,391 lines; 19,448 / 23,457 regions; 3,279 / 4,194 branches | recorded the current full gap ledger, grouped ownership totals, false-green adapter list, and ordered path to 100%; no fixture or implementation behavior changed |
| 2026-07-11 | Real glyph-name and name-index public API parity | 81 unique hashes | 0 | 6,488 | 6,487 / 6,487 | 1 | 13,565 / 16,517 lines; 19,634 / 23,702 regions; 3,296 / 4,226 branches | `FT_Get_Glyph_Name` and `FT_Get_Name_Index` no longer use fake value runners; 17 concrete variants compare supported `post` glyph names, truncation, null/error status, no-name controls, and zero sentinel behavior through C oracle, Rust core, C ABI, and WASM ABI |
| 2026-07-11 | Real gasp and cmap metadata public API parity | 78 unique hashes | 0 | 6,495 | 6,494 / 6,494 | 1 | 13,721 / 16,684 lines; 19,849 / 23,941 regions; 3,310 / 4,246 branches | `FT_Get_Gasp`, `FT_Get_CMap_Format`, and `FT_Get_CMap_Language_ID` now compare exact C oracle, Rust FFI, C ABI, and WASM ABI rows. One 3.9 KiB cmap matrix covers format 4, 6, 12, 14, null, and out-of-range charmap metadata without broad-font multiplication |
| 2026-07-11 | Gasp stream-length and malformed EOF controls | 81 unique hashes | 0 | 6,498 | 6,497 / 6,497 | 1 | 13,725 / 16,686 lines; 19,856 / 23,944 regions; 3,311 / 4,246 branches | three compact `gasp` controls cover FreeType's stream read beyond SFNT record length, a physical one-byte header, and a truncated range array. The stream-length row exposed and fixed Rust's previous table-length-capped parse; exact Rust/C/WASM parity remains green |
| 2026-07-11 | Compact post format name controls | 83 unique hashes | 0 | 6,503 | 6,502 / 6,502 | 1 | 13,747 / 16,686 lines; 19,898 / 23,944 regions; 3,317 / 4,246 branches | two compact `post` controls cover format 1.0 non-258-glyph default names and format 2.5 signed-delta name lookup through `FT_Get_Glyph_Name` and `FT_Get_Name_Index`; `tt/post.rs` now has 7/7 functions and 89/97 lines covered with exact Rust/C/WASM parity |
| 2026-07-11 | Malformed post name controls | 90 unique hashes | 0 | 6,510 | 6,509 / 6,509 | 1 | 13,753 / 16,687 lines; 19,907 / 23,946 regions; 3,323 / 4,246 branches | seven compact `post` controls cover short and zero-count format 2.0/2.5 tables, truncated format 2.0 custom names, above-limit format 2.5 counts, and unsupported post formats. Exact parity exposed and fixed Rust's unsupported-format glyph-name flag and missing-custom-name fallback |
| 2026-07-11 | Real subglyph public API parity | 90 unique hashes | 0 | 6,516 | 6,515 / 6,515 | 1 | 13,807 / 16,732 lines; 19,969 / 24,008 regions; 3,330 / 4,256 branches | `FT_Get_SubGlyph_Info` no longer uses an unsupported stub or C/WASM fallback delegation. Nine explicit rows reuse `glyf-component-matrix.ttf` to compare raw component index, flags, args, transform, null-slot errors, non-composite slots, and out-of-range subglyphs through C oracle, Rust core, C ABI, and WASM ABI |
| 2026-07-11 | Named-instance PostScript parity | 90 unique hashes | 0 | 6,516 | 6,516 / 6,516 | 0 | 13,935 / 16,899 lines; 20,169 / 24,275 regions; 3,342 / 4,298 branches | `FT_Set_Named_Instance` now selects or clears named instances in core and through thin C/WASM wrappers. The existing `FT_Get_Postscript_Name` row uses `named-instances.ttf` and compares `default`, instance 1, and instance 2 through the pinned C oracle, Rust FFI, C ABI, and WASM ABI, removing the final pending row |
| 2026-07-11 | Direct `FT_Set_Named_Instance` parity routing | 90 unique hashes | 0 | 6,516 | 6,513 / 6,513 | 3 | 13,937 / 16,899 lines; 20,173 / 24,275 regions; 3,342 / 4,298 branches | `ftmm.set_named_instance` no longer reaches the generic oracle fallback; select, clear, and invalid-index compact variable rows execute pinned C oracle, Rust FFI, C ABI, and WASM ABI. Three rows remain explicit pending: Adobe MM reset, `gvar`/HVAR glyph-output deltas, and `FT_MM_Var` namedstyle coordinates |
| 2026-07-11 | Shared signed SFNT helper coverage | 90 unique hashes | 0 | 6,516 | 6,513 / 6,513 | 3 | 13,940 / 16,899 lines; 20,177 / 24,275 regions; 3,342 / 4,298 branches | The public `post` table parser now reuses `tt::read_i16` for signed underline fields instead of duplicating byte decoding. Existing `FT_Get_Glyph_Name` post fixtures cover the helper through real C/Rust/C/WASM parity, closing `tt/mod.rs` structural coverage without adding cases or changing font assets |
| 2026-07-11 | Raw TrueType program table helper coverage | 90 unique hashes | 0 | 6,516 | 6,513 / 6,513 | 3 | 13,948 / 16,901 lines; 20,181 / 24,271 regions; 3,342 / 4,298 branches | Font construction now routes optional `fpgm` and `prep` table reads through the restored byte-copy helpers. Existing `FT_Load_Glyph` rows cover the path through real compact TT program fonts, making `tt/hinter/tables.rs` 100% covered without new cases, font assets, or fixture-only calls |
| 2026-07-11 | Branch-edge invalid coordinate reads | 90 unique hashes | 0 | 6,516 | 6,513 / 6,513 | 3 | 13,952 / 16,901 lines; 20,186 / 24,271 regions; 3,347 / 4,298 branches | Existing `branchEdgeMatrix` now packs invalid `GC[0]`, `GC[1]`, and `MDRP` point reads into its no-output TT program, reaching `GlyphZone` out-of-range guards through `FT_Load_Glyph` without adding concrete cases or changing parity output |
| 2026-07-11 | Compact fvar structural controls | 93 unique hashes | 0 | 6,519 | 6,516 / 6,516 | 3 | 13,992 / 16,920 lines; 20,230 / 24,298 regions; 3,345 / 4,290 branches | `scripts/build_fvar_fixtures.py` rebuilds the compact malformed fvar controls and adds three explicit public `FT_FACE_FLAG_MULTIPLE_MASTERS` variants for instance-array EOF, too-short instance records, and instance PostScript IDs. `tt/fvar.rs` reaches full branch coverage; the two remaining lines are the mathematically unreachable u16 instance-count overflow guard |
| 2026-07-11 | Compact name selection and PostScript fallback controls | 97 unique hashes | 0 | 6,523 | 6,520 / 6,520 | 3 | 14,024 / 16,920 lines; 20,290 / 24,298 regions; 3,360 / 4,290 branches | `scripts/build_name_fixtures.py` rebuilds four compact name-table controls and `FT_Get_Postscript_Name` now has explicit variants for unsupported/malformed family-name fallback, Apple-only PostScript names, odd Windows PostScript fallback, and Apple-only encoded named-instance prefixes. `tt/name.rs` moves to 232 / 240 lines and the rejected platform-0/missing-subfamily candidates are tracked as correctness buckets |
| 2026-07-11 | Rendered transform slot coverage | 97 unique hashes | 0 | 6,524 | 6,521 / 6,521 | 3 | 14,078 / 16,966 lines; 20,365 / 24,359 regions; 3,364 / 4,294 branches | one explicit `FT_Set_Transform` variant renders DejaVu glyph 36 after a non-identity matrix and delta. The row exposed a real bitmap-byte divergence; core now transforms the render snapshot in glyph-slot coordinates and recomputes the preset bitmap box before rasterization, matching pinned C FreeType with exact Rust/C/WASM parity |
| 2026-07-11 | Unicode variation prefix parity | 98 unique hashes | 0 | 6,525 | 6,522 / 6,522 | 3 | 14,131 / 17,016 lines; 20,444 / 24,441 regions; 3,385 / 4,326 branches | one compact variable font adds a named-instance row where nameID 25 exists only on Unicode/ISO-style platforms while the instance subfamily is Unicode. The row exposed a real divergence: pinned C returned `Ubuntu-Thin`, while Rust previously returned `UniVar-Thin`. Core now matches `sfnt_get_var_ps_name` by using only Windows 3/0, Windows 3/1, or Apple Roman records for the variation PostScript prefix while retaining general name lookup for the subfamily |
| 2026-07-11 | Odd Windows variation prefix fallback | 99 unique hashes | 0 | 6,526 | 6,523 / 6,523 | 3 | 14,136 / 17,020 lines; 20,445 / 24,441 regions; 3,387 / 4,326 branches | one compact variable font adds an encoded named-instance row where nameID 25 has an odd-length Windows record plus Apple Roman fallback. Exact C/Rust/C-ABI/WASM parity proves the invalid Windows prefix is rejected and Apple Roman is used, covering the remaining variation-prefix rejection branch. The route audit now also recognizes `FT_Request_Size` variant rows and the null-face `FT_Set_Charmap` row as explicit real routes, moving real-parity routes to 3,071 and shape-incomplete rows down to 38 without case multiplication |
| 2026-07-11 | Missing subfamily variation synthesis | 101 unique hashes | 0 | 6,531 | 6,528 / 6,528 | 3 | 14,236 / 17,125 lines; 20,590 / 24,591 regions; 3,415 / 4,366 branches | one compact variable font adds four encoded named-instance rows where the fvar subfamily IDs have only unsupported name records. The row exposed a real divergence: pinned C returned `MissingVar_100wght`, while Rust previously kept `Ubuntu-Regular`. Core now parses fvar axis defaults and instance coordinates, then matches `sfnt_get_var_ps_name` / `construct_instance_name` for positive, zero, negative, and fractional 16.16 coordinate descriptors plus sanitized axis tags. A compact malformed fvar control covers too-short axis records through `FT_FACE_FLAG_MULTIPLE_MASTERS`. Route audit real-parity rows are now 3,075 |
| 2026-07-11 | Core vector-length long-domain routing | 101 unique hashes | 0 | 6,531 | 6,528 / 6,528 | 3 | 14,249 / 17,133 lines; 20,596 / 24,593 regions; 3,414 / 4,364 branches | `FT_Vector_Length` now delegates to a core `fixed::ft_vector_length_long` helper that preserves the public `FT_Long` input domain, while the existing 32-bit rasterizer helper delegates through it. Existing public vector-length rows pass exact C/Rust/C-ABI/WASM parity, moving duplicate CORDIC math out of the wrapper without adding cases or fonts |
| 2026-07-11 | Branch-edge zero stack-vector probes | 101 unique hashes | 0 | 6,531 | 6,528 / 6,528 | 3 | 14,250 / 17,133 lines; 20,599 / 24,593 regions; 3,417 / 4,364 branches | The existing source-backed `branchEdgeMatrix` glyph now packs zero `SPVFS` and `SFVFS` bytecode probes beside its zero-length `SFVTL` control. C `Normalize` returns success without changing vectors for `(0,0)` stack vectors, and exact `FT_Load_Glyph` parity remains green while the shared zero-normalization guard in `fixed.rs` is covered without adding cases |
| 2026-07-11 | Compact malformed format-14 cmap controls | 102 unique hashes | 0 | 6,532 | 6,529 / 6,529 | 3 | 14,258 / 17,133 lines; 20,604 / 24,593 regions; 3,420 / 4,364 branches | `scripts/build_cmap_fixtures.py` now emits a raw `cmap-format14-malformed-matrix.ttf` with one valid Unicode format 6 subtable plus malformed format 14 short, length-short, and record-array-overflow subtables. One explicit `FT_Get_Char_Index` row proves pinned FreeType, Rust, C ABI, and WASM all ignore the malformed optional format-14 records while preserving the valid charmap, bringing `tt/cmap.rs` to 100% branch coverage without implicit case growth |
| 2026-07-11 | Compact autohint script standard-character fixture | 103 unique hashes | 0 | 6,545 | 6,542 / 6,542 | 3 | 14,297 / 17,133 lines; 20,656 / 24,593 regions; 3,423 / 4,364 branches | `scripts/build_autohint_script_fixtures.py` emits one 5.5 KiB `script-coverage.ttf` with one glyph per autohint script tag; 13 explicit `FT_LOAD_FORCE_AUTOHINT` variants cover Adlam, Arabic, Armenian, Bengali, Gurmukhi, Hebrew, Khmer, Kannada, Latin sub/superscript, Malayalam, Mongolian, and Thai standard-character rows, moving `globals_data.rs` from 25 / 293 to 61 / 293 lines with exact Rust/C/WASM parity |
| 2026-07-11 | Vai autohint script standard-character row | 103 unique hashes | 0 | 6,546 | 6,543 / 6,543 | 3 | 14,299 / 17,133 lines; 20,659 / 24,593 regions; 3,423 / 4,364 branches | one explicit `FT_LOAD_FORCE_AUTOHINT` row selects `script-coverage.ttf` U+A5CD, covering the Vai standard-character arm with exact Rust/C/WASM parity and no implicit case growth |
| 2026-07-11 | Indic CJK autohint script rows | 103 unique hashes | 0 | 6,550 | 6,547 / 6,547 | 3 | 14,312 / 17,146 lines; 20,693 / 24,623 regions; 3,428 / 4,370 branches | four explicit `FT_LOAD_FORCE_AUTOHINT` rows select Limbu, Oriya, Syloti Nagri, and Tibetan glyphs in `script-coverage.ttf`. Core routes FreeType's `STYLE_DEFAULT_INDIC` rows through CJK metrics/hints with no blue zones. The original cross-style ownership claim was incorrect: pinned no-HarfBuzz C accepts any mapped standard candidate, as corrected by the later standard-character fallback audit |
| 2026-07-11 | No-autohint precedence and shared bbox conversion | 103 unique hashes | 0 | 6,553 | 6,550 / 6,550 | 3 | 14,322 / 17,141 lines; 20,705 / 24,618 regions; 3,432 / 4,370 branches | two explicit `FT_LOAD_NO_AUTOHINT` variants prove `NO_AUTOHINT` masks `FORCE_AUTOHINT` and `TARGET_LIGHT` branch conditions through Rust, C ABI, and WASM `FT_Load_Char`; `bbox_to_ffi` now delegates to the existing `From<BBox>` conversion so public glyph-slot rows cover the shared field-copy path instead of bypassing it |
| 2026-07-11 | Invalid load-target mode parity rows | 103 unique hashes | 0 | 6,555 | 6,552 / 6,552 | 3 | 14,324 / 17,141 lines; 20,709 / 24,618 regions; 3,434 / 4,370 branches | two explicit `FT_LOAD_TARGET_MODE` rows prove pinned FreeType accepts an unknown target nibble for load-only calls but returns `FT_Err_Cannot_Render_Glyph` when `FT_LOAD_RENDER` requests rendering with that invalid mode. Exact Rust, C ABI, and WASM parity remains green; `ffi/convert.rs` no longer has missing branch outcomes |
| 2026-07-11 | Format-14-only cmap sentinel rows | 104 unique hashes | 0 | 6,558 | 6,555 / 6,555 | 3 | 14,326 / 17,141 lines; 20,711 / 24,618 regions; 3,434 / 4,370 branches | `scripts/build_cmap_fixtures.py` now emits compact `cmap-format14-only.ttf`; explicit `FT_Get_Char_Index`, `FT_Get_First_Char`, and `FT_Get_Next_Char` rows prove pinned C and Rust/C-ABI/WASM return zero sentinels for format-14-only direct lookup and iteration. `tt/cmap.rs` reaches 428 / 429 lines and 100% branch coverage without implicit case growth |
| 2026-07-11 | Name table branch-matrix controls | 106 unique hashes | 0 | 6,560 | 6,557 / 6,557 | 3 | 14,326 / 17,141 lines; 20,722 / 24,618 regions; 3,455 / 4,370 branches | `scripts/build_name_fixtures.py` now emits compact static and variable branch-matrix controls. Two explicit `FT_Get_Postscript_Name` variants prove pinned C and Rust/C-ABI/WASM agree on empty PostScript records, non-English Windows replacement rejection, invalid Windows filtering, Apple fallback filtering, and variation-prefix fallback synthesis. `tt/name.rs` moves to 470 / 481 regions and 116 / 134 branch outcomes without implicit case growth |
| 2026-07-11 | Autohint unequal digit-width probes | 106 unique hashes | 0 | 6,560 | 6,557 / 6,557 | 3 | 14,327 / 17,141 lines; 20,723 / 24,618 regions; 3,457 / 4,370 branches | `script-coverage.ttf` now appends two ASCII digit glyphs with unequal advances. Existing explicit `FT_LOAD_FORCE_AUTOHINT` rows prove pinned C and Rust/C-ABI/WASM agree while `FaceGlobals::digits_have_same_width` reaches its false path without increasing the public case count |
| 2026-07-11 | TrueType VM no-output branch probes | 106 unique hashes | 0 | 6,560 | 6,557 / 6,557 | 3 | 14,359 / 17,141 lines; 20,821 / 24,618 regions; 3,476 / 4,370 branches | Existing `branchEdgeMatrix` and its `prep` program now pack `GETINFO`, clamped `DELTAP`/`DELTAC`, twilight-zone `MDRP`/`MIRP`/`MSIRP`/`ISECT`, unknown-opcode IDEF dispatch, and prep-range no-op `INSTCTRL` probes into the source-backed `hinter-control-matrix.ttf`; exact Rust/C/WASM parity remains green with no concrete case growth |
| 2026-07-11 | TrueType VM twilight/interpolation probes | 106 unique hashes | 0 | 6,560 | 6,557 / 6,557 | 3 | 14,378 / 17,141 lines; 20,879 / 24,618 regions; 3,496 / 4,370 branches | Existing `branchEdgeMatrix` now adds twilight `UTP`, `SCFS`, original-distance `MD[1]`, negative-CVT `MIRP`, twilight `IP`, and invalid `ISECT` continuation probes. They remain no-output state exercises inside the same public `FT_Load_Glyph` row, moving `tt/hinter/exec.rs` to 1,274 / 1,340 lines and 339 / 410 branch outcomes |
| 2026-07-11 | Non-uniform TrueType size scale | 106 unique hashes | 0 | 6,561 | 6,558 / 6,558 | 3 | 14,417 / 17,174 lines; 20,945 / 24,673 regions; 3,506 / 4,380 branches | One explicit `FT_Load_Glyph.matrix_load` variant loads the source-backed `branchEdgeMatrix` glyph at 20x32 px. It exposed a real C/Rust mismatch where C returned horizontal advance `896` but Rust returned `1408` by rebuilding a square scale from height. Core scaler/autohint now read the active FreeType size metrics; `tt/hinter/exec.rs` reaches 1,277 / 1,340 lines and 340 / 410 branch outcomes |
| 2026-07-11 | Non-uniform TrueType MD/IP probes | 106 unique hashes | 0 | 6,563 | 6,560 / 6,560 | 3 | 14,428 / 17,174 lines; 20,981 / 24,673 regions; 3,509 / 4,380 branches | Two explicit `FT_Load_Glyph.matrix_load` variants load the existing point-coordinate and point-move matrix glyphs at 20x32 px. They cover the non-square `MD[0]` and `IP` interpreter branches without adding fonts or implicit expansion; `tt/hinter/exec.rs` reaches 1,288 / 1,340 lines and 343 / 410 branch outcomes |
| 2026-07-11 | TrueType MDRP single-width cut-in probes | 106 unique hashes | 0 | 6,563 | 6,560 / 6,560 | 3 | 14,433 / 17,174 lines; 20,988 / 24,673 regions; 3,516 / 4,380 branches | Existing `branchEdgeMatrix` now packs positive glyph-zone and negative twilight-zone `MDRP` single-width cut-in probes into its no-output branch program. Exact Rust/C/WASM parity remains green with no concrete case growth; `tt/hinter/exec.rs` reaches 1,293 / 1,340 lines and 350 / 410 branch outcomes |
| 2026-07-11 | TrueType empty-stack ROLL probe | 106 unique hashes | 0 | 6,563 | 6,560 / 6,560 | 3 | 14,434 / 17,174 lines; 20,989 / 24,673 regions; 3,517 / 4,380 branches | Existing `stackStateMatrix` now executes `ROLL` immediately after a `CLEAR`, covering FreeType-compatible empty-stack no-op behavior without changing glyph output or adding public cases. Exact Rust/C/WASM parity remains green; `tt/hinter/exec.rs` reaches 1,294 / 1,340 lines and 351 / 410 branch outcomes |
| 2026-07-11 | TrueType repeated IUP compatibility probe | 106 unique hashes | 0 | 6,563 | 6,560 / 6,560 | 3 | 14,436 / 17,174 lines; 20,991 / 24,673 regions; 3,519 / 4,380 branches | Existing `pointMoveMatrix` now repeats `IUP[y]` and `IUP[x]` after both axes have already interpolated. Pinned FreeType returns immediately in backward-compatibility mode once the state reaches `0x7`; exact Rust/C/WASM parity remains green without adding concrete cases, and `tt/hinter/exec.rs` reaches 1,296 / 1,340 lines and 353 / 410 branch outcomes |
| 2026-07-12 | Full compact script-glyph autohint selection | 106 unique hashes | 0 | 6,604 | 6,601 / 6,601 | 3 | 14,440 / 17,174 lines; 21,032 / 24,673 regions; 3,523 / 4,380 branches | The existing `script-coverage.ttf` public case now selects every generated script probe through explicit `FT_LOAD_FORCE_AUTOHINT` variants. Exact Rust/C ABI/WASM parity remains green while `autohint/globals_data.rs` reaches 117 / 234 regions and the script table paths are explicit instead of reserved hidden coverage |
| 2026-07-12 | hhea-zero metric fallback controls | 108 unique hashes | 0 | 6,606 | 6,603 / 6,603 | 3 | 14,454 / 17,174 lines; 21,061 / 24,673 regions; 3,529 / 4,380 branches | Two compact `FT_Size_Metrics` variants use generated hhea-zero fonts to prove FreeType's metric fallback order: zero hhea falls to OS/2 typo metrics, then to OS/2 Windows ascent/descent when typo metrics are also zero. Exact Rust/C ABI/WASM parity remains green and `font.rs` gains 14 lines, 29 regions, and 6 branch outcomes without implicit case growth |
| 2026-07-12 | Core `FT_Get_Charmap_Index` route | 108 unique hashes | 0 | 6,606 | 6,603 / 6,603 | 3 | 14,458 / 17,175 lines; 21,073 / 24,682 regions; 3,529 / 4,380 branches | Existing `freetype.FT_Get_Charmap_Index` rows now exercise the core public helper for owned, null, and foreign charmaps. C ABI keeps raw-pointer validation and delegates the owned-charmap index value to core; no fonts or cases were added |
| 2026-07-12 | Core CMap scoped helper routes | 108 unique hashes | 0 | 6,606 | 6,603 / 6,603 | 3 | 14,470 / 17,175 lines; 21,106 / 24,682 regions; 3,529 / 4,380 branches | Existing `tttables.FT_Get_CMap_Format` and `tttables.FT_Get_CMap_Language_ID` rows now verify `FT_Charmap_Info`, `FT_Charmap_Format`, and `FT_Charmap_Language_ID` agree with raw public CMap helpers for valid, null, and out-of-range charmaps. No fonts or cases were added |
| 2026-07-12 | Safe Rust load-glyph agreement route | 108 unique hashes | 0 | 6,606 | 6,603 / 6,603 | 3 | 14,473 / 17,175 lines; 21,111 / 24,682 regions; 3,529 / 4,380 branches | Seven existing `FT_LOAD_*` rows now assert `Face::load_glyph` matches the Rust FFI `FT_Load_Glyph` slot for compute metrics, force autohint, no hinting, no recurse, no scale, load-time render, and target-light representatives. The output JSON and case count stay unchanged |
| 2026-07-12 | Render-mode load-flag helper route | 108 unique hashes | 0 | 6,606 | 6,603 / 6,603 | 3 | 14,480 / 17,174 lines; 21,122 / 24,685 regions; 3,529 / 4,380 branches | `FT_Render_Glyph` now carries the returned slot's render target through the shared `load_flag_for_render_mode` helper. Existing explicit render-mode rows cover normal, mono, LCD, LCD_V, and SDF routing with exact Rust FFI, C ABI, and WASM parity; no fonts or cases were added |
| 2026-07-12 | Public render-cache and pixel-size helper routes | 108 unique hashes | 0 | 6,606 | 6,603 / 6,603 | 3 | 14,551 / 17,190 lines; 21,223 / 24,719 regions; 3,533 / 4,390 branches | Existing `FT_Render_Glyph` public rows now route `Face::render_loaded_glyph` through the shared render-font cache while clearing that cache on face size/charmap/named-instance mutations; existing `FT_Set_Pixel_Sizes` rows now execute `SizeMetrics::from_pixel_size` zero-dimension normalization directly. Exact Rust FFI, C ABI, and WASM parity remains green with no fonts or cases added, and `api.rs` reaches 462 / 486 lines and 53 / 54 functions |
| 2026-07-12 | Explicit render-cache load-mode variants | 108 unique hashes | 0 | 6,610 | 6,607 / 6,607 | 3 | 14,559 / 17,190 lines; 21,233 / 24,719 regions; 3,539 / 4,390 branches | Four explicit `FT_Render_Glyph.matrix_render` variants reuse existing DejaVu/Noto fonts to cover safe Rust render-cache load-mode selection for force autohint, target light, no autohint, and force-autohint masked by no-autohint. Exact Rust FFI, C ABI, and WASM parity remains green; implicit cases remain zero |
| 2026-07-12 | Mono negative-collapse glyph correction | 108 unique hashes | 0 | 6,610 | 6,607 / 6,607 | 3 | 14,561 / 17,190 lines; 21,235 / 24,719 regions; 3,541 / 4,390 branches | The existing source-backed `renderCollapseNegativeX` and `renderCollapseNegativeY` glyphs now sit at 330 font units instead of 375, which scales to a negative 26.6 collapse bias at the existing 20 ppem mono rows. This fixes stale fixture obligations without adding cases or changing the harness, and covers both negative `PixelBox::with_non_collapsed` arms with exact Rust/C ABI/WASM parity |
| 2026-07-12 | Explicit render-cache repeat row | 108 unique hashes | 0 | 6,611 | 6,608 / 6,608 | 3 | 14,562 / 17,190 lines; 21,237 / 24,719 regions; 3,542 / 4,390 branches | One explicit `FT_Render_Glyph.supported_render_modes_repeat_cache` row renders glyph 41 from `hinter-control-matrix.ttf` twice on the same face. The public C oracle, Rust FFI, C ABI, and WASM ABI compare both slot snapshots, proving the safe Rust `RenderFontCache::get_or_insert_with` cache-hit branch without implicit case growth |
| 2026-07-12 | Explicit load-glyph malformed facade errors | 108 unique hashes | 0 | 6,613 | 6,610 / 6,610 | 3 | 14,566 / 17,190 lines; 21,245 / 24,719 regions; 3,543 / 4,390 branches | Two explicit `FT_Load_Glyph.matrix_load` variants reuse `glyf-malformed-matrix.ttf` to cover no-autohint and force-autohint malformed glyph errors. Selected malformed rows now compare safe `Face::load_glyph` error parity against `FT_Load_Glyph`, keeping exact Rust/C ABI/WASM parity green and implicit cases at zero |
| 2026-07-12 | Size API null-validation routes | 108 unique hashes | 0 | 6,613 | 6,610 / 6,610 | 3 | 14,581 / 17,202 lines; 21,264 / 24,735 regions; 3,548 / 4,398 branches | Existing `FT_New_Size`, `FT_Done_Size`, and `FT_Activate_Size` null rows now execute pinned C oracle commands and the Rust FFI wrapper validation path for null face, null output, and null size handles. Exact Rust/C ABI/WASM parity remains green with no new cases; success multi-size lifecycle rows remain visibly unsupported/generic pending real size-object implementation |
| 2026-07-12 | Isolated CJK stem-sort topology row | 108 unique hashes | 0 | 6,614 | 6,611 / 6,611 | 3 | 14,583 / 17,202 lines; 21,265 / 24,735 regions; 3,549 / 4,398 branches | One explicit `FT_LOAD_FORCE_AUTOHINT` variant selects the new `cjkStemSort` glyph in the source-backed compact CJK font at U+519C. The glyph keeps U+7530 unchanged and adds two internal vertical stems with unequal widths, giving additive CJK autohint coverage with exact Rust/C ABI/WASM parity and zero implicit cases |
| 2026-07-12 | Composite no-recurse and render topology rows | 108 unique hashes | 0 | 6,619 | 6,616 / 6,616 | 3 | 14,626 / 17,230 lines; 21,333 / 24,773 regions; 3,557 / 4,398 branches | Three explicit `FT_Bitmap.public_fields_match_render_output` variants reuse `multiple-charmaps.ttf` glyphs 483 and 380 for mono/SDF off-curve-start and degenerate-conic render topology, the stale `FT_LOAD_NO_RECURSE` composite row now selects DejaVu `Agrave` instead of missing `Aring`, and two explicit `FT_Load_Glyph.matrix_load` variants reuse `hinter-control-matrix.ttf` glyphs 3 and 8 for native composite point attachment and unrounded offsets. The selector correction exposed and fixed composite no-recurse metrics to use the raw composite `glyf` header bbox, matching pinned C, with exact Rust/C ABI/WASM parity and zero implicit cases |
| 2026-07-12 | S45ROUND clamp probe in super-round glyph | 108 unique hashes | 0 | 6,619 | 6,616 / 6,616 | 3 | 14,628 / 17,230 lines; 21,335 / 24,773 regions; 3,559 / 4,398 branches | The source-backed `hinter-control-matrix.ttf` `superRoundMatrix` glyph now includes a selector `0x71` S45ROUND no-output probe that rounds `0` and `-1`, forcing FreeType's positive and negative clamp repairs while popping the results. The existing `hinter-super-round-matrix` public row keeps exact Rust/C ABI/WASM parity, case count, and implicit count unchanged |
| 2026-07-12 | Transform-render empty outline row | 108 unique hashes | 0 | 6,620 | 6,617 / 6,617 | 3 | 14,629 / 17,230 lines; 21,338 / 24,773 regions; 3,561 / 4,398 branches | One explicit `FT_Set_Transform.load_ignore_transform_behavior` variant loads `hinter-control-matrix.ttf` glyph 21 with `FT_LOAD_RENDER` under the existing non-identity matrix. It covers the transformed render-outline empty guard in `api.rs` without a new font, discovery axis, or harness path, and exact Rust/C ABI/WASM parity remains green |
| 2026-07-12 | Hani fallback standard-width order font | 109 unique hashes | 0 | 6,622 | 6,619 / 6,619 | 3 | 14,650 / 17,230 lines; 21,358 / 24,773 regions; 3,572 / 4,398 branches | One minimal `cjk-width-order.ttf` fixture omits U+7530 and maps U+56D7 to a two-stem glyph whose first stem is wider than the second. The explicit `cjk-width-order-20` force-autohint row covers CJK descending width insertion-sort and quantization branches with exact Rust/C ABI/WASM parity and zero implicit cases |
| 2026-07-12 | SDF self-intersection render topology row | 109 unique hashes | 0 | 6,623 | 6,620 / 6,620 | 3 | 14,651 / 17,230 lines; 21,359 / 24,773 regions; 3,573 / 4,398 branches | One explicit `FT_Bitmap.public_fields_match_render_output` variant reuses `hinter-control-matrix.ttf` glyph 42 in SDF mode. It pairs the existing mono/normal bowtie rows with an SDF self-intersection-thin public comparison, moving `render.rs` by one line, one region, and one branch while preserving exact Rust/C ABI/WASM parity and zero implicit cases |
| 2026-07-12 | Explicit format-14 `FT_Set_Charmap` rejection | 109 unique hashes | 0 | 6,623 | 6,620 / 6,620 | 3 | 14,652 / 17,230 lines; 21,360 / 24,773 regions; 3,574 / 4,398 branches | The stale future-asset `FT_Set_Charmap.error_format14_charmap` row now reuses active `cmap-format14-only.ttf` with explicit `all_charmaps` selection. It moves one row from shape-incomplete fallback to real Rust/C ABI/WASM parity, keeps case count flat, and covers the thin FFI format-14 rejection branch |
| 2026-07-12 | CBox and memory-face route cleanup | 109 unique hashes | 0 | 6,625 | 6,622 / 6,622 | 3 | 14,652 / 17,230 lines; 21,360 / 24,773 regions; 3,574 / 4,398 branches | `FT_Outline_Get_CBox` now uses source-backed `hinter-control-matrix.ttf` glyphs 41 and 21 for conic control-point and empty-outline cboxes, `FT_BBox.negative_and_empty_bounds` uses three explicit glyph variants for negative, empty, and zero-width boxes, and the compact `FT_New_Memory_Face.valid_font_bytes` variants expose matching `font` aliases for their memory sources. Structural coverage is unchanged because these parser/cbox paths were already hit, but route audit moves real parity to 3,196 rows and shape-incomplete fallback down to 11 without implicit discovery |
| 2026-07-12 | Executable invalid-error route split | 109 unique hashes | 0 | 6,629 | 6,626 / 6,626 | 3 | 14,654 / 17,230 lines; 21,362 / 24,773 regions; 3,576 / 4,398 branches | `FT_Get_Sfnt_Name.invalid_argument_errors` now executes an explicit invalid name index while preserving null-output and non-SFNT residuals as incomplete route work; `FT_Load_Glyph.error_out_of_range_null_face_or_invalid_flags` now executes out-of-range glyph and reserved-load-flag variants while preserving null-face as unrouted; `FT_Load_Char.error_null_face_or_invalid_flags` now executes the reserved-load-flag row while preserving null-face as unrouted. Exact Rust/C ABI/WASM parity remains green, route audit real parity moves to 3,200 rows, and implicit cases remain zero |
| 2026-07-12 | Face-flag route-audit classification | 109 unique hashes | 0 | 6,629 | 6,626 / 6,626 | 3 | unchanged | The 24 `freetype.face_flags` concrete rows already execute pinned C `--face-flags`, Rust FFI, C ABI, and WASM ABI routes; `check_public_api_inputs.py` now classifies that operation as real parity instead of generic fallback. Focused face-flag parity passes 45 / 45, route audit real parity rises to 3,224, and generic fallback drops to 961 without changing fixture outputs or coverage denominator |
| 2026-07-12 | Render composite-slot error split | 109 unique hashes | 0 | 6,630 | 6,627 / 6,627 | 3 | unchanged | `FT_Render_Glyph.error_unloaded_or_unsupported_slot_format` now has an executable `FT_LOAD_NO_RECURSE` DejaVu `Agrave` variant that compares pinned C's `FT_Err_Cannot_Render_Glyph` against Rust FFI, C ABI, and WASM ABI. The synthetic unloaded and unsupported-slot probes remain visibly unrouted. This moves route-audit real parity to 3,225 with no structural coverage delta, proving the old aggregate contained one real public error path and residual synthetic route work |
| 2026-07-12 | Size error and probe-face rows | 109 unique hashes | 0 | 6,635 | 6,632 / 6,632 | 3 | 14,658 / 17,230 lines; 21,366 / 24,773 regions; 3,580 / 4,398 branches | `FT_Set_Char_Size.error_oversized_dimensions` now has three executable DejaVu variants for ppem-too-large, width-over-core-range, and height-over-core-range requests; `FT_Set_Char_Size.error_probe_face_invalid_size_handle` and `FT_Request_Size.error_probe_face_invalid_size_handle` use negative face-index probe handles. Pinned C returns `FT_Err_Invalid_Pixel_Size` for oversized char-size requests and `FT_Err_Invalid_Size_Handle` for probe handles. Rust FFI, C ABI, and WASM ABI match exactly, route-audit real parity rises to 3,230, and implicit cases remain zero |
| 2026-07-12 | Direct pixel-size probe route | 109 unique hashes | 0 | 6,636 | 6,633 / 6,633 | 3 | 14,659 / 17,230 lines; 21,367 / 24,773 regions; 3,581 / 4,398 branches | `FT_Set_Pixel_Sizes` C ABI and WASM parity now open the face without pre-sizing, call the public pixel-size function directly, and read metrics only after success. One explicit negative face-index probe row compares pinned C's `FT_Err_Invalid_Size_Handle` through Rust FFI, C ABI, and WASM ABI. Route-audit real parity rises to 3,231, the probe-only branch in `ffi/handles.rs` is covered, and implicit cases remain zero |
| 2026-07-12 | Exact SFNT load-table stream semantics | 109 unique hashes | 0 | 6,643 | 6,640 / 6,640 | 3 | 14,679 / 17,253 lines; 21,407 / 24,819 regions; 3,582 / 4,398 branches | `FT_Load_Sfnt_Table` now has explicit whole-font, table-directory, signed-offset, raw-stream read, null-length, and oversized-tag variants. Core matches pinned `tt_face_load_any` stream behavior, Rust FFI maps missing tables and stream errors separately, and C ABI/WASM rows call their public wrappers directly with exact C/Rust parity and zero implicit cases |
| 2026-07-12 | Direct SFNT table-info pointer semantics | 109 unique hashes | 0 | 6,646 | 6,643 / 6,643 | 3 | 14,693 / 17,264 lines; 21,417 / 24,829 regions; 3,589 / 4,406 branches | `FT_Sfnt_Table_Info` now has explicit tag-null count-query, table-index 0/1, missing-index, and null-length variants. Rust FFI mirrors the public out-pointer contract, C ABI and WASM rows call their exported wrappers directly, route-audit real parity rises to 3,241, and implicit cases remain zero |
| 2026-07-12 | Direct SFNT-name nullable pointer routes | 109 unique hashes | 0 | 6,648 | 6,644 / 6,644 | 4 | 14,695 / 17,264 lines; 21,419 / 24,829 regions; 3,591 / 4,406 branches | `FT_Get_Sfnt_Name.invalid_argument_errors` now has explicit null-face and null-output variants that call pinned C `FT_Get_Sfnt_Name`, Rust FFI, C ABI, and WASM ABI with the same pointer shapes and compare the returned `Invalid_Argument` sequence. The non-SFNT placeholder remains explicit with indexes but is pending rather than counted as live non-SFNT coverage because the current Type 1 fixture fails at face opening. Route-audit real parity rises to 3,243, pending-core rises to 4, and shape-incomplete fallback drops to 10 without implicit case growth |
| 2026-07-12 | Direct load-char null-face route | 109 unique hashes | 0 | 6,648 | 6,644 / 6,644 | 4 | 14,695 / 17,264 lines; 21,419 / 24,829 regions; 3,591 / 4,406 branches | `FT_Load_Char.error_null_face_or_invalid_flags.null_face` now calls pinned C `FT_Load_Char(NULL, char_code, flags)` instead of the generic error oracle, and the Rust/C ABI/WASM legs route the same explicit null-face shape through their public validation surfaces. The C and WASM wrappers now return FreeType's `Invalid_Face_Handle` for invalid face handles, route-audit real parity rises to 3,244, and shape-incomplete fallback drops to 9 with no new cases or implicit discovery |
| 2026-07-12 | Direct load-glyph null-face route | 109 unique hashes | 0 | 6,648 | 6,644 / 6,644 | 4 | 14,695 / 17,264 lines; 21,419 / 24,829 regions; 3,591 / 4,406 branches | `FT_Load_Glyph.error_null_face_or_invalid_flags.null_face` now calls pinned C `FT_Load_Glyph(NULL, glyph_index, flags)` instead of the generic null fallback, and Rust/C ABI/WASM legs route the same explicit null-face shape through public validation. The C and WASM wrappers now match FreeType's `Invalid_Face_Handle` for invalid glyph-load handles, route-audit real parity rises to 3,245, and shape-incomplete fallback drops to 8 without case growth |
| 2026-07-12 | Direct unpatented-hinting post-load route | 109 unique hashes | 0 | 6,648 | 6,644 / 6,644 | 4 | 14,695 / 17,264 lines; 21,419 / 24,829 regions; 3,591 / 4,406 branches | `FT_Face_SetUnpatentedHinting.post_toggle_load_behavior` now calls pinned C `FT_Face_SetUnpatentedHinting` for the explicit toggle sequence and then compares the post-toggle `FT_Load_Glyph` slot through Rust FFI, C ABI, and WASM ABI. Pinned FreeType's deprecated function is a no-op returning false, so the row proves unchanged post-toggle load behavior instead of a generic boolean list. Route-audit real parity rises to 3,246 and shape-incomplete fallback drops to 7 without case or coverage growth |
| 2026-07-12 | Direct outline cbox nullable pointer route | 109 unique hashes | 0 | 6,648 | 6,644 / 6,644 | 4 | 14,720 / 17,289 lines; 21,454 / 24,864 regions; 3,595 / 4,410 branches | `FT_Outline_Get_CBox.null_inputs_noop` now calls pinned C, Rust FFI, C ABI, and WASM ABI pointer no-op shapes directly. Live `ftoutln.outline_get_cbox` glyph rows also call the safe Rust helper and verify it agrees with the loaded slot cbox for control-point and empty-outline cases. Route-audit real parity rises to 3,247 and shape-incomplete fallback drops to 6 without case growth |
| 2026-07-12 | Native-long fixed-math route parity | 109 unique hashes | 0 | 6,649 | 6,645 / 6,645 | 4 | 14,736 / 17,290 lines; 21,474 / 24,856 regions; 3,600 / 4,406 branches | Existing `FT_RoundFix`, `FT_CeilFix`, `FT_FloorFix`, `FT_MulDiv`, `FT_MulFix`, and `FT_DivFix` rows now classify as real runtime parity instead of compile contracts. The focused `FT_RoundFix.wraparound_matches_c` row exposed that C `FT_Fixed` is a native signed long and returns `2147483648` for the `2147483647` round boundary on this host, while the old core wrapper truncated to `-2147483648`. Rust FFI now delegates unary fixed functions to core native-long wrappers, `fixed.rs` reaches 215 / 215 lines, and route-audit real parity rises to 3,268 without new fonts, cases, or implicit discovery |
| 2026-07-12 | Shared CORDIC cast helper route | 109 unique hashes | 0 | 6,649 | 6,645 / 6,645 | 4 | 14,739 / 17,290 lines; 21,480 / 24,859 regions; 3,600 / 4,406 branches | Existing `fttrigon.FT_Vector_Length` rows exercise the FreeType CORDIC prenormalization path. The remaining raw `i64 as u32` low-word cast there now uses the shared `casts::u32_from_i64` helper documented for 32-bit parts, moving `casts.rs` to 51 / 51 lines, 14 / 14 functions, and 65 / 65 regions without adding cases, fonts, or test-only hooks |
| 2026-07-12 | Direct SFNT lang-tag route | 110 unique hashes | 0 | 6,650 | 6,646 / 6,646 | 4 | 14,864 / 17,361 lines; 21,638 / 24,966 regions; 3,616 / 4,422 branches | `FT_Get_Sfnt_LangTag` now uses a compact generated name-format-1 font with two language-tag records so explicit `0x8001` reaches FreeType's `langID - 0x8000` index rule and explicit `0x8002` proves the upper-bound invalid row. Existing public inputs now compare native C oracle, Rust core, C ABI fallback-to-core, and WASM fallback-to-core output with zero implicit cases; the previous build-dependent `FT_SfntLangTag` record row is now executable |
| 2026-07-12 | Direct cmap format-14 variant-index route | 110 unique hashes | 0 | 6,652 | 6,648 / 6,648 | 4 | 15,039 / 17,586 lines; 21,866 / 25,230 regions; 3,640 / 4,462 branches | `FT_Face_GetCharVariantIndex` now routes five explicit rows through pinned C FreeType, Rust FFI, C ABI, and WASM ABI using the compact `cmap-format-language-matrix.ttf` format-14 subtable. The rows cover non-default UVS, default UVS, missing char/selector, no-format14 control, and null-face zero behavior without adding fonts or implicit discovery. Core now parses format-14 selector records and default/non-default UVS tables; ABI wrappers stay thin and delegate to the shared Rust FFI. Route-audit real parity rises to 3,274, generic fallback drops to 958, void fallback drops to 2, and implicit cases remain zero |
| 2026-07-12 | Direct cmap format-14 default-query route | 110 unique hashes | 0 | 6,654 | 6,650 / 6,650 | 4 | 15,086 / 17,635 lines; 21,918 / 25,284 regions; 3,655 / 4,482 branches | `FT_Face_GetCharVariantIsDefault` now routes five explicit rows through pinned C FreeType, Rust FFI, C ABI, and WASM ABI using the same compact `cmap-format-language-matrix.ttf` format-14 subtable. The rows cover default UVS returning 1, non-default UVS returning 0, missing UVS, no-format14 control, and null-face `-1` behavior without adding fonts or implicit discovery. Core exposes the format-14 selector default query separately from glyph-index lookup because pinned FreeType does not require the active charmap to be Unicode for this API. Route-audit real parity rises to 3,279, generic fallback drops to 955, and implicit cases remain zero |
| 2026-07-12 | Direct cmap format-14 selector-list route | 110 unique hashes | 0 | 6,655 | 6,651 / 6,651 | 4 | 15,105 / 17,655 lines; 21,946 / 25,313 regions; 3,657 / 4,486 branches | `FT_Face_GetVariantSelectors` now routes four explicit rows through pinned C FreeType, Rust FFI, C ABI, and WASM ABI using the compact `cmap-format-language-matrix.ttf` format-14 subtable. The rows cover non-null selector lists, no-format14 `NULL`, null-face `NULL`, and copied face-owned result lifetime before invalidation. ABI wrappers keep only owned zero-terminated scratch storage and delegate selector discovery to Rust FFI. Route-audit real parity rises to 3,283, generic fallback drops to 952, and implicit cases remain zero |
| 2026-07-12 | Direct cmap format-14 char and selector UVS list routes | 110 unique hashes | 0 | 6,660 | 6,656 / 6,656 | 4 | 15,196 / 17,746 lines; 22,060 / 25,428 regions; 3,673 / 4,510 branches | `FT_Face_GetVariantsOfChar` and `FT_Face_GetCharsOfVariant` now route eleven explicit rows through pinned C FreeType, Rust FFI, C ABI, and WASM ABI using the compact format-14 cmap fixture. The rows cover a character with two selectors, C's non-null empty selector list for a format-14 character with no UVS entries, no-format14 `NULL`, null-face `NULL`, present selector char lists, absent selector `NULL`, empty selector `NULL`, and copied face-owned list lifetime. Route-audit real parity rises to 3,294, generic fallback drops to 946, and implicit cases remain zero |
| 2026-07-12 | Route OpenType validation null contracts | 110 unique hashes | 0 | 6,660 | 6,656 / 6,656 | 4 | 15,216 / 17,756 lines; 22,079 / 25,444 regions; 3,684 / 4,522 branches | `FT_OpenType_Validate` and `FT_OpenType_Free` null-face/null-output rows now call pinned C, Rust FFI, C ABI, and WASM ABI instead of modeled fallbacks. Route audit moves real-null-validation to 8, generic fallback to 942, and generic-error fallback to 141 with zero implicit cases |
| 2026-07-12 | Malformed name language-tag parser controls | 113 unique hashes | 0 | 6,662 | 6,658 / 6,658 | 4 | 15,223 / 17,756 lines; 22,085 / 25,444 regions; 3,685 / 4,522 branches | Two compact format-1 `name` table controls cover language-tag record-array overflow and language-tag string out-of-range behavior through public `FT_New_Memory_Face`. The record-array overflow face is rejected. The string-offset face opens because pinned C retains the tag with zero length; the earlier claim that both were rejected was corrected by the 2026-07-17 source re-audit and direct oracle payload. |
| 2026-07-12 | Malformed name language-tag count guard | 114 unique hashes | 0 | 6,663 | 6,659 / 6,659 | 4 | 15,224 / 17,756 lines; 22,088 / 25,444 regions; 3,685 / 4,522 branches | One compact format-1 `name` table control omits the language-tag count field after a complete zero-record header. Public `FT_New_Memory_Face` now compares pinned C and Rust rejection through Rust FFI, C ABI, and WASM ABI; route audit counts the row as real parity and `tt/name.rs` moves to 332 / 333 lines and 29 / 30 functions |
| 2026-07-12 | Name string out-of-range fallback controls | 116 unique hashes | 0 | 6,665 | 6,661 / 6,661 | 4 | 15,225 / 17,756 lines; 22,092 / 25,444 regions; 3,686 / 4,522 branches | Two compact name-table controls cover successful fallback after malformed name string offsets: `FT_New_Memory_Face` proves an out-of-range English Windows typographic family record falls back to Apple Roman, and `FT_Get_Postscript_Name` proves an out-of-range Apple PostScript record returns null. `tt/name.rs` reaches 333 / 333 lines, 30 / 30 functions, and 121 / 138 branch outcomes |
| 2026-07-14 | SBIT packed compound tail carry | 141 unique hashes | 0 | 6,839 | 6,836 / 6,836 | 3 | 17,647 / 19,660 lines; 25,434 / 28,320 regions; 4,171 / 4,896 branches | One compact `sbit_composite_mono_carry_success_format8.ttf` fixture selects a 10-bit MONO component at a 7-pixel x offset through `FT_Load_Glyph.matrix_load`. Pinned C, Rust FFI, C ABI, and WASM ABI agree exactly, and `tt/sbit.rs` lines 694-696 are no longer in the missing-line report without adding implicit cases |
| 2026-07-14 | SBIT packed compound GRAY2/GRAY4 dispatch | 143 unique hashes | 0 | 6,841 | 6,838 / 6,838 | 3 | 17,649 / 19,660 lines; 25,436 / 28,320 regions; 4,171 / 4,896 branches | Two compact `sbit_composite_gray2_success_format8.ttf` and `sbit_composite_gray4_success_format8.ttf` fixtures select image-format-8 compound glyph 2 through `FT_Load_Glyph.matrix_load` with `FT_LOAD_SBITS_ONLY`. Pinned C, Rust FFI, C ABI, and WASM ABI agree exactly, and `tt/sbit.rs` lines 601-602 are no longer in the missing-line report without adding implicit cases |
| 2026-07-14 | TrueType overlap outline flags | 143 unique hashes | 0 | 6,844 | 6,841 / 6,841 | 3 | 17,678 / 19,689 lines; 25,465 / 28,349 regions; 4,175 / 4,900 branches | `build_render_fixtures.py` appends gids 6 and 7 to `render-coverage.ttf`: gid 6 has the first simple-glyph flag byte set to `0x41`, and gid 7 has first component flags `0x0404`. Three explicit `ftimage.FT_GLYPH_FORMAT_OUTLINE.outline_payload_matches_format` rows select scaled simple, no-scale simple, and scaled compound overlap loads, proving pinned C, Rust FFI, C ABI, and WASM ABI agree on public `FT_Outline.flags`. Core now mirrors FreeType `ttgload.c:459-461,530-532,1917-1920,2569-2576`: retain `OVERLAP_SIMPLE` and first-component `OVERLAP_COMPOUND` in `FT_OUTLINE_OVERLAP`, mask public point tags back to curve bits, and add high precision below 24 ppem. Route audit reports 3,526 real-parity rows and zero implicit cases |
| 2026-07-14 | Smooth overlap render parity | 143 unique hashes | 0 | 6,844 | 6,841 / 6,841 | 3 | 17,758 / 19,771 lines; 25,583 / 28,478 regions; 4,178 / 4,904 branches | `build_render_fixtures.py` appends gid 8 to `render-coverage.ttf`, a flagged two-contour overlap glyph with fractional pixel edges. `ftimage.FT_OUTLINE_OVERLAP.smooth_overlap_behavior` now has explicit NORMAL and LIGHT render variants over that glyph. Core `render_normal` mirrors FreeType `src/smooth/ftsmooth.c:497-552,621-637`: flagged gray outlines render through a 4x oversampled pass and downsample with the same span accumulation rule. Focused `make -C pillow-rs-freetype test-case CASE=FT_OUTLINE_OVERLAP` passes 4 / 4 exact comparisons; full condition coverage passes 6,841 / 6,841 with the same three FTMM runtime-pending rows. Route audit moves the two overlap variants from pending-core to real-parity (`real-parity 3,526 -> 3,528`, `pending-core 7 -> 5`) without increasing concrete cases. The newly added overflow/error guards remain visible as residual coverage work and are not counted as complete |
| 2026-07-14 | Smooth overlap raster-overflow route | 143 unique hashes | 0 | 6,844 | 6,841 / 6,841 | 3 | 17,771 / 19,781 lines; 25,586 / 28,478 regions; 4,179 / 4,904 branches | `build_render_fixtures.py` appends gid 9 to `render-coverage.ttf`, a 9000 px wide flagged overlap outline at 1024 ppem. The existing `fterrdef.FT_Err_Raster_Overflow.raster_buffer_or_cell_overflow` row now uses explicit NORMAL and LIGHT `render_glyph` variants over that glyph instead of a future synthetic outline. Pinned C returns `FT_Err_Raster_Overflow` from `ftsmooth.c:511-515` when `bitmap->width * 4 > 0x7FFF`; Rust returns the same code before allocating the oversampled buffer. Focused `make -C pillow-rs-freetype test-case CASE=FT_Err_Raster_Overflow` passes 3 / 3 exact comparisons; full condition coverage still passes 6,841 / 6,841 with the same three FTMM runtime-pending rows. Route audit moves these two variants from generic-error-fallback to real-parity (`real-parity 3,528 -> 3,530`, `generic-error-fallback 141 -> 139`) without increasing concrete cases |

## Decision Log

| Date | Decision | Reason |
|---|---|---|
| 2026-07-10 | Use explicit grouped input variants only | Allows deliberate multi-input cases without hidden Cartesian growth |
| 2026-07-10 | Do not parameterize glyph-index discovery | Glyph selection must be explicit and tied to topology or behavior |
| 2026-07-10 | Measure Rust coverage only | Rust core owns behavior; C ABI and WASM ABI are thin wrappers exercised by the same parity cases |
| 2026-07-12 | Route cmap format-14 selector lists through the real call | Pinned C `FT_Face_GetVariantSelectors` finds the platform 0 encoding 5 format-14 charmap, returns a face-owned zero-terminated `FT_UInt32` selector list in subtable order, returns `NULL` for null face or no format-14 charmap, and allows the scratch result to be overwritten by the next FreeType call; fixtures must copy values immediately |
| 2026-07-12 | Preserve C UVS list null-versus-empty distinctions | Pinned C `FT_Face_GetVariantsOfChar` returns a non-null zero-terminated empty list when a format-14 charmap exists but no selector applies to the character, while `FT_Face_GetCharsOfVariant` returns `NULL` for an absent selector or a selector record with both default and non-default offsets zero |
| 2026-07-14 | Cover packed SBIT carry with one shifted compound glyph | The remaining packed compound blitter branch needed `x_shift + remaining_bits > 8`; a 10-bit MONO child shifted by seven pixels reaches that C behavior without multiplying bit depths, glyphs, or render modes |
| 2026-07-14 | Cover GRAY2/GRAY4 packed compound dispatch with two rows | The packed compound blitter has separate FreeType pixel-mode dispatch for bit depths 2 and 4; one zero-offset compound child for each proves those arms without multiplying x-offset, size, or render-mode axes |
| 2026-07-10 | Move old fonts before deleting them | Keeps active and legacy corpora distinct and allows safe obligation-by-obligation replacement |
| 2026-07-10 | Optimize fonts before broad input expansion | Font-level feature density removes more redundant cases than harness micro-optimization |
| 2026-07-10 | Mirror maxp's unbounded stream frame explicitly | Pinned FreeType reads maxp extras beyond the declared table length and ignores maxp load errors while constructing a zero-glyph face |
| 2026-07-10 | Model legacy kern as an optional sanitized table | Pinned FreeType ignores its top version, caps 32 subtables, clamps lengths, and only accepts format 0 with `(coverage & 3) == 1` |
| 2026-07-10 | Treat malformed metric bytes as deferred lookup data | Pinned FreeType records hmtx/vmtx offsets at face open and returns zero metrics when later reads or declared counts are unusable |
| 2026-07-10 | Keep the coverage denominator core-only | The parity executable still exercises Rust, C ABI, and WASM ABI for every runnable fixture, while reports exclude the thin wrapper source owned by separate crates |
| 2026-07-10 | Centralize FreeType face metric selection | Pinned `sfobjs.c` prioritizes OS/2 `USE_TYPO_METRICS`, then nonzero hhea, then OS/2 typo and Windows fallbacks for both face and size metrics |
| 2026-07-12 | Select hhea-zero metric fallback rows explicitly | The fallback code existed but public fixtures did not select zero hhea ascent/descent. Two generated `FT_Size_Metrics` variants cover OS/2 typo and OS/2 Windows fallback without broadening font discovery or multiplying unrelated metrics cases |
| 2026-07-12 | Keep charmap raw-pointer validation in ABI, not lookup semantics | `FT_Get_Charmap_Index` needs C ABI raw-pointer and lifetime validation, but the return value for live owned charmaps belongs in core. Existing public rows now call both the raw core helper and face-scoped helper to prove they agree without adding fixture rows |
| 2026-07-12 | Cover CMap scoped helpers through existing CMap rows | `FT_Charmap_Info`, `FT_Charmap_Format`, and `FT_Charmap_Language_ID` are core facade helpers for the same public CMap metadata already selected by `FT_Get_CMap_Format` and `FT_Get_CMap_Language_ID`; exercising them as agreement checks avoids new cases and keeps route intent explicit |
| 2026-07-12 | Reuse selected `FT_LOAD_*` rows for safe API slot agreement | `Face::load_glyph` is a public Rust facade over the same core behavior exposed by `FT_Load_Glyph`; asserting slot equality on representative existing rows covers facade output methods without widening the public input matrix |
| 2026-07-12 | Match `FT_Load_Sfnt_Table` raw stream behavior exactly | Pinned `tt_face_load_any` handles tag `0` as the whole stream, tag `1` as the directory, reports table size before applying offset on zero-length probes, then performs signed-offset reads against the stream rather than a table-clamped slice. Public inputs must include these boundary rows instead of relying on a generic table accessor |
| 2026-07-12 | Route rendered-slot target flags through the shared helper | `FT_Render_Glyph` already validates the public render mode before calling core rendering; preserving the selected render target in the returned slot's internal load flags keeps wrapper state centralized without adding cases or changing public output JSON |
| 2026-07-12 | Support explicit repeat rows only where stateful public behavior needs them | `repeat_count` is accepted only as a concrete input on existing `FT_Render_Glyph` public rows. It lets one case compare a deliberate sequence across Rust/C/C-ABI/WASM without reintroducing hidden Cartesian discovery or generic fixture generation |
| 2026-07-12 | Compare safe facade errors through selected public rows | Error-side coverage for `Face::load_glyph` must be proven by the same `FT_Load_Glyph` fixture cases and exact C/Rust/C-ABI/WASM parity, not by synthetic helper calls or broad routing |
| 2026-07-12 | Route size null validation before size lifecycle success | Null pointer validation belongs in the thin FFI wrapper and can be proven through existing `ftsizes` public rows. Non-null `FT_New_Size`, `FT_Done_Size`, and `FT_Activate_Size` success lifecycle rows stay generic/unsupported until real multi-size handle ownership is implemented instead of being modeled as C parity |
| 2026-07-14 | Match disabled `FT_Error_String` build behavior | The pinned FreeType oracle is built with `FT_ENABLE_ERROR_STRINGS=OFF`, so `FT_Error_String` returns `NULL` after its public range check even for valid base error codes. Public fixtures must compare that compiled behavior exactly instead of materializing strings from `fterrdef.h` |
| 2026-07-12 | Add new topology glyphs instead of mutating productive rows | Changing the existing U+7530 field glyph reduced net coverage by losing already-covered CJK paths. Additive CJK topology probes must use separate glyphs and explicit public variants so new behavior can only expand the measured union |
| 2026-07-12 | Use a separate Hani fallback-standard font for width sorting | CJK standard-width initialization always tries U+7530 before U+56D7. A dedicated two-glyph font without U+7530 exercises the fallback standard character and descending width order without changing productive U+7530 geometry or broadening discovery |
| 2026-07-11 | Pack no-output TT guard probes into existing branch-edge glyphs | Invalid coordinate reads exercise defensive zone access while preserving the same public `FT_Load_Glyph` output and avoiding extra Cartesian case growth |
| 2026-07-11 | Prefer no-output VM state probes before new TT rows | Stack-only calls, twilight-zone movement, and no-op prep instructions can cover VM branches through the existing public `FT_Load_Glyph` row when they do not alter glyph output or weaken parity |
| 2026-07-12 | Treat stale fixture obligations as font bugs | When an explicit row claims a structural branch but coverage shows it does not reach that branch, first correct the compact source font or selected glyph parameters instead of adding redundant cases |
| 2026-07-12 | Correct stale glyph selectors before adding rows | The no-recurse composite row selected U+00C5 in a trimmed font that lacked that codepoint and only exercised glyph 0; changing it to U+00C0 exposed a real C/Rust metrics mismatch and fixed core behavior to match C's composite-header bbox metrics |
| 2026-07-12 | Keep render topology rows in the bitmap public case | The `FT_Bitmap.public_fields_match_render_output` case already compares rendered bitmap fields and bytes through the public API legs, so targeted contour-topology variants there expand render coverage without adding a new fixture harness or modeled shortcut |
| 2026-07-12 | Pack S45ROUND clamp coverage into the existing super-round row | The clamp behavior is bytecode state only and the rounded values are popped immediately, so the existing `FT_Load_Glyph` row can prove the C/Rust interpreter path without adding a redundant glyph-output variant |
| 2026-07-12 | Cover transform-render guards with explicit glyph topology | The transform-render empty-outline guard belongs to public `FT_Set_Transform` plus `FT_LOAD_RENDER` behavior, so an explicit empty glyph variant in the existing transform case is preferred over a synthetic helper call |
| 2026-07-10 | Require every predicate operand outcome | Line execution alone missed non-Roman Mac and non-Windows fallback records; nightly branch coverage makes both sides of each short-circuit condition visible |
| 2026-07-11 | Do not keep C-mismatching name fixtures for coverage | Platform-0 variation-prefix and missing-subfamily candidate rows exposed real C/Rust PostScript-name differences; they remain correctness work rather than passing coverage rows |
| 2026-07-10 | Treat TTC table offsets as collection-absolute | Pinned `tt_face_load_font_dir` reads table offsets from the TTC stream origin; adding the selected face base a second time breaks every nonzero face |
| 2026-07-10 | Keep the embedded-strike request visibly pending | Existing bitmap-named aliases are scalable fonts and core has no embedded-strike table support; substituting a numeric size would falsely satisfy the manifest obligation |
| 2026-07-10 | Model cmap `char_next` per format | Pinned format 6 increments before its terminal check and wraps at `0xFFFFFFFF`; formats 4 and 12 reject their terminal inputs before advancing |
| 2026-07-10 | Validate a composite tree once before no-hint scaling | The public scaler always calls `load_glyph` first; the scaled helper consumes that validated tree and must not retain public-unreachable duplicate malformed-data branches |
| 2026-07-10 | Validate whole loca records | A single checked 4-byte or 8-byte slice expresses FreeType's truncated-record failure without byte-by-byte optional indexing or twelve redundant fonts |
| 2026-07-10 | Keep raw fpgm/prep storage direct while preserving helpers | Font construction consumes raw byte streams directly, but the existing public copy helpers remain available and visibly uncovered rather than being deleted for coverage |
| 2026-07-11 | Route raw fpgm/prep table reads through restored helpers | The helpers are equivalent byte-copy parsers for pinned `tt_face_load_fpgm` and `tt_face_load_prep`; using them from font construction ties coverage to real public `FT_Load_Glyph` execution instead of synthetic helper calls |
| 2026-07-10 | Keep scan conversion controls in one program font | Empty setup and scan-type variants share tables, geometry, size, and flags; only explicit glyph programs differ, avoiding a font/size/flag product |
| 2026-07-10 | Hash resolved assets in the C-oracle cache key | JSON paths do not identify mutable fixture contents; path, length, and SHA-256 now prevent stale C output after in-place font mutation |
| 2026-07-10 | Preserve existing autohint diagnostic and script surfaces | Runtime uses `FaceGlobals`, but fixture reachability alone does not authorize deleting public diagnostics, script helpers, or blue-character lookup data |
| 2026-07-10 | Preserve the existing `CallRecord` contract | LOOPCALL's repeated definition lookup is currently invariant-backed, but changing public record fields merely to remove an uncovered guard is not justified |
| 2026-07-10 | Extend the CJK fixture into a multiscript topology matrix | Four compact Latin adjustment glyphs reuse the existing source-backed font identity and add 256 lines and 91 branches from four explicit cases |
| 2026-07-10 | Preserve second-bottom Latin adjustment modes | `AF_ADJUST_DOWN2` and `AF_ADJUST_TILDE_BOTTOM2` have no current database selector, but that makes them visibly unreachable rather than disposable |
| 2026-07-10 | Use one strong corner with two weak controls for single-reference IUP | Keeping vectors beyond the near threshold and making only one incident direction cardinal gives one touched point without a second edge-aligned reference |
| 2026-07-10 | Keep straight and curved degenerate segment merges separate | Straight vertical reversals cover longer-segment retention/replacement; pinned `aflatin.c` documents equal-direction unification as a rarer already-merged zig-zag state requiring different geometry |
| 2026-07-10 | Preserve flat and round blue medians exactly | Pinned `af_latin_metrics_init_blues` takes each median verbatim and only then applies directional overshoot sanity; Rust's extra discrepancy heuristic caused a one-pixel lowercase metric-height error |
| 2026-07-10 | Keep CJK metrics dispatch inside shared edge hinting | CJK width initialization enters the shared Latin helper before the main apply path dispatches, so the internal CJK delegate is executed behavior and must not be removed as duplicate |
| 2026-07-10 | Preserve constructed-edge defensive guards | Current construction excludes endpoint and invalid-chain states, but fixtures not reaching those guards is insufficient reason to remove future-safety checks |
| 2026-07-10 | Pack tilde measurement outcomes into existing contours | Top, second-top, and bottom adjustment cases can own both quadratic measurement directions and the no-stretch threshold without another glyph or explicit input |
| 2026-07-10 | Do not delete code to improve coverage | Current fixture parity proves only selected inputs. Uncovered public helpers, defensive guards, and pinned-FreeType special cases remain visible until independent semantic evidence proves removal is correct |
| 2026-07-10 | Restore second-bottom adjustment modes | Absence from the current adjustment database proves no present public selector, not that the pinned FreeType behavior is disposable; keep `DOWN2/BOTTOM2` for correctness and future database changes |
| 2026-07-10 | Keep the CJK round-segment helper visible | Pinned FreeType's zero segment-limit snapshot prevents the current public path from calling it; preserving the helper exposes the gap instead of manufacturing coverage or deleting behavior |
| 2026-07-10 | Preserve embedded TTX timestamps | `ttx` otherwise hashes the source filesystem mtime into the binary, making identical fixture sources produce different fonts and invalidating content-addressed oracle caching |
| 2026-07-10 | Treat parity as input-scoped evidence | Exact C/Rust/WASM agreement proves behavior only for selected inputs; source paths remain preserved and uncovered paths remain explicit obligations so deleting code cannot silently erase an unrepresented special case |
| 2026-07-10 | Match FreeType SDF conic subdivision exactly | Pinned `ftsdf.c` chooses bisections from conic deviation, splits every conic at least once, and uses truncating midpoint arithmetic; the previous generic flattener produced many one-byte SDF differences on consecutive off-curve controls |
| 2026-07-11 | Use executable load flags in direct inputs | `load_flag_sets` is not consumed by the explicit parity runner; single concrete flag combinations must use `load_flags` so coverage and parity measure the intended public API behavior |
| 2026-07-11 | Prefer rows inside existing public API cases for scalar size permutations | `FT_Request_Size` already compares an output list, so adding deliberate request rows covers branch outcomes without adding logical cases or reintroducing Cartesian products |
| 2026-07-11 | Allow one-case growth when it proves a distinct font table behavior | The no-scale `vmtx` path requires a different font table state from DejaVu; one explicit CJK vertical fixture row is preferable to multiplying every no-scale flag variant |
| 2026-07-11 | Convert inert scalar lists to explicit variants | Declarative fields such as `size_ppem_values` do not affect execution unless the runner consumes them; public inputs must use concrete variants or supported row arrays so coverage measures the intended cases |
| 2026-07-11 | Do not alias lower-priority adjustment codepoints onto existing adjustment glyphs | The reverse-cmap adjustment lookup scans `ADJUSTMENT_DATABASE` order by glyph index; mapping lower codepoints such as `U+0122` or `U+01D5` onto the existing tilde glyphs changes pinned public metrics for the established tilde cases |
| 2026-07-11 | Honor exact outline tags in the gray rasterizer | `Outline` already carries FreeType tag bytes; using them lets public outline-render inputs reach cubic control pairs while preserving on-curve fallback behavior for older outlines |
| 2026-07-11 | Convert declarative charmap/SFNT rows to executable variants | Coverage only counts when the explicit runner consumes the field; ignored arrays such as multi-read SFNT declarations must become concrete variants or a maintained direct helper with exact C/Rust/WASM output parity |
| 2026-07-11 | Leave transformed render divergence as a core bug bucket | A candidate `FT_Set_Transform` render-after-transform input reached `api.rs` transformed render-outline code but produced Rust/C bitmap byte divergence, so it is not a coverage-only fixture addition |
| 2026-07-11 | Transform rendered snapshots in slot coordinates | Pinned `FT_Load_Glyph` applies the face transform to the slot outline before `ft_glyphslot_preset_bitmap`; Rust must reconstruct `LoadedOutline` back to slot coordinates, apply the transform, then normalize it into the recomputed bitmap box before rasterization |
| 2026-07-11 | Route null lifecycle fixtures through thin Rust FFI | Existing lifecycle fixtures should execute the same thin Rust FFI handlers as C/WASM for handle validation coverage; modeled error shortcuts are only for surfaces without a maintained direct Rust handler |
| 2026-07-11 | Rebase worker glyph additions onto current fixture glyph order | Worker font-source changes must preserve all previously merged glyph roles; the TT branch-edge glyph moved from id 48 to id 51 because render coverage already owns glyphs 48-50 |
| 2026-07-11 | Keep exact boundary rows even when broader guards are already covered | The one-past-head-table SFNT row adds no new structural counters after executable offset coverage, but it preserves a precise public boundary case from the metadata worker without multiplying unrelated inputs |
| 2026-07-11 | Preserve render-load slot semantics in public wrapper coverage | `Face::render_loaded_glyph` strips `FT_LOAD_RENDER` before loading, while C `FT_Render_Glyph` returns an already-rendered bitmap slot unchanged; public wrapper routing must therefore fall back to the FFI-shaped path for rows whose load flags already render |
| 2026-07-11 | Treat regenerated coverage output as authoritative | The refreshed condition-coverage run is the active denominator: 13,627 / 16,580 lines, 19,723 / 23,798 regions, and 3,303 / 4,234 branches; do not rely on stale incremental coverage artifacts |
| 2026-07-11 | Treat PostScript-name fixtures as real parity, not value stubs | `FT_Get_Postscript_Name` now compares face-borrowed bytes/nullness through C oracle, Rust, C ABI, and WASM ABI. Pinned FreeType uses `sfnt_get_name_id` plus `sfnt_is_postscript`, so Rust must filter invalid PostScript-name characters while leaving raw `FT_Get_Sfnt_Name` records unchanged |
| 2026-07-11 | Treat glyph-name fixtures as buffer/status parity, not value stubs | `FT_Get_Glyph_Name` must compare the returned `FT_Error` and the caller buffer snapshot. Pinned FreeType clears `buffer[0]` after validating the face and buffer but before invalid-glyph/no-name service errors, and `FT_Get_Name_Index` returns the first matching `post` name or zero for null/unavailable/unknown names |
| 2026-07-11 | Treat gasp fixtures as table-behavior parity, not value stubs | `FT_Get_Gasp` now reads compact generated SFNT controls instead of `DejaVuSans.ttf` symlink aliases. Pinned `ftgasp.c` returns `FT_GASP_NO_TABLE` for null/no usable table and masks version 0 flags with `& 3`; pinned `ttload.c` treats unsupported `gasp` versions as optional table load failures while keeping the face usable |
| 2026-07-11 | Treat cmap format/language fixtures as metadata parity, not stubs | `FT_Get_CMap_Format` and `FT_Get_CMap_Language_ID` now use a compact generated SFNT cmap matrix instead of modeled values. Pinned `ftobjs.c` returns `-1` for invalid format probes, `0` for invalid language probes, and `ttcmap.c` reports format 14 language as `0xFFFFFFFF` |
| 2026-07-11 | Treat malformed format-14 cmap records as load-time parser parity | Pinned FreeType ignores malformed optional format-14 records when another valid Unicode charmap remains usable. Public `FT_Set_Charmap` rejects format 14; the compact format-14-only font now proves that rejection explicitly, while direct lookup and char-iteration rows still prove zero-sentinel behavior |
| 2026-07-12 | Treat autohint script lookup coverage as explicit public rows | `script-coverage.ttf` exists to activate real `FT_LOAD_FORCE_AUTOHINT` script paths through selected Unicode codepoints. All generated script-tag glyphs are now explicit public variants; future work should add new script glyphs only when the generator grows a new documented obligation |
| 2026-07-14 | Pack Latin tilde branch outcomes into `script-coverage.ttf` | Top, second-top, no-measure, flat-accent, and bottom tilde adjustment branches are Latin autohint topology obligations, not font-family obligations. Keeping them in the existing compact script font adds explicit public rows with exact C/Rust/C-ABI/WASM parity while avoiding a broad DejaVu or script cross product |
| 2026-07-12 | Reject parity-green rows that do not move coverage | A candidate `FT_Render_Glyph.matrix_render` SDF row using DejaVu glyph 82 at 48 ppem passed exact Rust/C/WASM parity but did not change `render.rs` or total coverage, so it was removed instead of growing the fixture count |
| 2026-07-12 | Reject zero-extent render rows that do not move coverage | Candidate `FT_Render_Glyph.matrix_render` rows for `hinter-control-matrix.ttf` glyphs 44 and 45 in normal mode passed exact parity, but did not change line, region, function, or branch coverage beyond the accepted five-row render topology batch. They were removed to avoid increasing the optimized fixture set without measurable coverage value |
| 2026-07-12 | Invalidate cached render fonts on face mutations | Routing `Face::render_loaded_glyph` through `RenderFontCache` must clear cached font clones after size, charmap, or named-instance changes; otherwise a later render could reuse a stale font after the same face object mutates |
| 2026-07-11 | Add Tibetan only after the Indic CJK route fix | A candidate `script-coverage.ttf` U+0F40 Tibetan row exposed a real `FT_LOAD_FORCE_AUTOHINT` mismatch before the core fix. The row is now explicit only because Rust matches pinned C by routing `STYLE_DEFAULT_INDIC` through CJK/no-blue hinting and by not borrowing Latin `o` widths for Indic standard-character setup |
| 2026-07-11 | Match FreeType's `gasp` stream read length | Pinned `tt_face_load_gasp` seeks to the table and reads frames from the stream without using the SFNT record length as a cap. Rust must parse from the table offset to physical stream EOF for this optional table, while genuinely short physical data still degrades to `FT_GASP_NO_TABLE` |
| 2026-07-11 | Match FreeType's `post` format 2.5 tag and delta behavior | Pinned `ttpost.c` recognizes format 2.5 as `0x00025000`, computes `glyph_index + signed_delta`, and maps out-of-range results to Mac glyph index 0. Format 1.0 only returns Mac standard names when `maxp.numGlyphs == 258`; otherwise the public name stays `.notdef` |
| 2026-07-11 | Match malformed `post` public fallbacks | Pinned FreeType clears the output buffer and returns `Invalid_Argument` when an unsupported `post` format prevents `FT_HAS_GLYPH_NAMES`, while malformed format 2.0/2.5 name payloads that pass the header flag still return success with `.notdef`. Rust must keep scalar `post` metadata parsed while exposing glyph-name capability only for accepted formats 1.0, 2.0, and 2.5 |
| 2026-07-11 | Treat subglyph info as raw composite slot data | Pinned `FT_Get_SubGlyph_Info` succeeds only for a composite glyph slot with loaded subglyph records and a valid sub-index, then returns the raw component flags, args, glyph index, and 16.16 transform. Rust keeps composite flags from `glyf`, exposes them through the core glyph slot, and lets C/WASM wrappers only validate pointers and copy the core result |
| 2026-07-11 | Select named instances through face index high bits | Pinned FreeType stores a 1-based named-instance selector in bits 16..30 of `face_index`; `FT_Set_Named_Instance(0)` clears it. When an `fvar` instance lacks an explicit PostScript name ID, FreeType builds the name from nameID 25 plus a sanitized instance subfamily string |
| 2026-07-11 | Make named-instance gaps pending instead of fallback-green | `ftmm.set_named_instance` previously appeared green through the generic modeled-error path. Direct oracle routing proves the compact success/error rows and leaves Adobe MM reset, `FT_MM_Var` namedstyle coordinates, and `gvar`/HVAR glyph-output deltas visible until the core implementation exists |
| 2026-07-11 | Compare structured error output only by explicit opt-in | Existing expected-error rows intentionally tolerate several Rust/C error-classification differences. Rows that claim post-error state preservation or exact error classification, such as invalid named-instance selection and SFNT stream-boundary reads, must set `compare_error_output` and provide matching C oracle, Rust, C ABI, and WASM ABI error codes plus state snapshots |
| 2026-07-11 | Prefer shared table readers over duplicated byte decoding | Reusing existing SFNT endian helpers is valid coverage progress when the public parser already reads the same field. It does not remove behavior or add a fake test path, and keeps coverage tied to real public fixture execution |
| 2026-07-11 | Classify fvar instance-count overflow as unreachable | `instance_count` and `instance_size` are 16-bit SFNT fields, so their product fits in `usize` on supported 32-bit and 64-bit targets. This was superseded on 2026-07-16: Rust now applies C `sfnt_init_face` count and record-size limits first, making every subsequent offset bounded and removing the obsolete closures with exact public malformed-font proof |
| 2026-07-11 | Match variation PostScript prefix platform filtering | Pinned `sfnt_get_var_ps_name` calls `sfnt_get_name_id`, which accepts only Windows 3/0, Windows 3/1, and Apple Roman records for the variation prefix. It does not use the broader Unicode/ISO fallback from `tt_face_get_name`; the named-instance subfamily still uses that general lookup path |
| 2026-07-11 | Match missing-subfamily named-instance synthesis | Pinned `sfnt_get_var_ps_name` falls through to `construct_instance_name` when a named instance has no explicit PostScript name and no usable subfamily name. The fallback appends each non-default fvar coordinate as a shortest 16.16 decimal followed by sanitized axis-tag characters |
| 2026-07-11 | Treat route-audit shape as the explicit row contract | `FT_Request_Size` variants are maintained parser rows, and null-face `FT_Set_Charmap` rows still need an explicit selector shape. Audit classification must mirror the maintained runner contract instead of leaving real parity rows in shape fallback |
| 2026-07-12 | Keep memory-face source aliases route-visible | `FT_New_Memory_Face` parity reads bytes from the public memory source, but the route audit recognizes runnable font assets by `font`/`fixture` keys. Compact malformed font variants should carry a matching `font` alias beside `font_bytes` when both refer to the same source, so the row remains explicit real parity instead of shape fallback |
| 2026-07-12 | Match size-error codes from the C oracle | Pinned `FT_Set_Char_Size` reaches `FT_Request_Metrics` and returns `FT_Err_Invalid_Pixel_Size` for oversized ppem results, including host-width values that Rust core cannot store as `i32`. Negative face-index probe handles return `FT_Err_Invalid_Size_Handle` through `FT_Set_Char_Size` and `FT_Request_Size`; these are public parity rows, not generic invalid-argument shortcuts |
| 2026-07-12 | Route pixel-size parity through the public function | `FT_Set_Pixel_Sizes` rows should open the face, call the public size setter, and then inspect metrics only after success. Opening with size already applied hides public setter errors in C/WASM legs and cannot prove negative face-index probe behavior |
| 2026-07-12 | Match `FT_Sfnt_Table_Info` nullable out-pointer behavior | Pinned `sfnt_table_info` rejects `length == NULL` before table lookup, treats `tag == NULL` as a table-count query that ignores `table_index`, and returns `Table_Missing` for out-of-range indexes only when `tag` is non-null. Public inputs must keep these as explicit variants because pointer-state arrays are not executable coverage |
| 2026-07-12 | Route `FT_Get_Sfnt_Name` pointer errors through the real call | Pinned `FT_Get_Sfnt_Name` returns `Invalid_Argument` in the function output for null face and null `aname`; these rows must not go through the generic null-source handler, which reports a top-level invalid-face status instead. A non-SFNT row only proves the `FT_IS_SFNT` branch after both C FreeType and Rust can open the fixture as a live non-SFNT `FT_Face` |
| 2026-07-13 | Prove `FT_Get_Sfnt_Name` non-SFNT behavior with Type 1 | `fonts/type1/simple-type1.pfb` is now generated from `scripts/build_type1_fixtures.py` as a valid compact Type 1 face. Pinned `ftsnames.c` returns `Invalid_Argument` and copies no name fields because `FT_IS_SFNT(face)` is false; Rust mirrors this with a minimal non-SFNT face kind rather than fabricating SFNT name data |
| 2026-07-12 | Route `FT_Load_Char` null-face errors through the real call | Pinned `FT_Load_Char` checks `face == NULL` before charmap lookup or glyph loading and returns `FT_Err_Invalid_Face_Handle`. Public null-face rows must call that function directly; generic `--error` or null-source shortcuts are only fallback evidence and should not count as route parity |
| 2026-07-12 | Route `FT_Load_Glyph` null-face errors through the real call | Pinned `FT_Load_Glyph` checks `!face || !face->size || !face->glyph` before driver dispatch and returns `FT_Err_Invalid_Face_Handle`. Public null-face glyph-load rows must call `FT_Load_Glyph(NULL, ...)` directly; wrapper validation should mirror that error code without adding font logic |
| 2026-07-12 | Prove unpatented-hinting toggles through post-load behavior | Pinned `FT_Face_SetUnpatentedHinting` in `ftpatent.c` ignores the face and value and always returns false. Toggle-sequence rows should therefore compare the following public glyph load slot, not only the deprecated function's scalar return |
| 2026-07-12 | Route `FT_Outline_Get_CBox` nullable inputs through the real call | Pinned `ftoutln.c` does nothing when either `outline` or `acbox` is null, and writes a zero box only for a non-null outline with zero points. Public null/no-op rows must call `FT_Outline_Get_CBox` directly; generic void or modeled fallback output cannot prove the pointer contract |
| 2026-07-13 | Classify autohint blue-character lookup as no-route helper coverage | Fresh coverage at 6,755 concrete rows shows all 60 explicit `script-coverage.ttf` `FT_LOAD_FORCE_AUTOHINT` rows exercise the public `STYLE_TABLE` and standard-character script paths. The public autohint load route passes `style.blue_entries` directly into `metrics_init_blues_impl`; the unused duplicate `globals_data::blue_chars_for_script` Rust helper was subsequently removed instead of adding a synthetic glyph row |
| 2026-07-12 | Route cmap format-14 glyph-index queries through the real call | Pinned `FT_Face_GetCharVariantIndex` returns zero unless the active charmap is Unicode and a format-14 charmap exists, truncates `FT_ULong` charcode and selector inputs to `FT_UInt32`, uses the active Unicode charmap for default UVS glyph lookup, and uses the format-14 non-default GID table otherwise. Public inputs should keep default, non-default, missing, no-format14, and null-face rows explicit |
| 2026-07-12 | Route cmap format-14 default queries through the real call | Pinned `FT_Face_GetCharVariantIsDefault` finds the platform 0 encoding 5 format-14 selector charmap directly, truncates `FT_ULong` charcode and selector inputs to `FT_UInt32`, returns 1 for default UVS coverage, 0 for non-default UVS coverage with a nonzero glyph, and -1 for missing selector/char/no-format14/null face. Unlike `FT_Face_GetCharVariantIndex`, it does not require the active charmap to be Unicode |
| 2026-07-13 | Preserve format-14 UVS edge semantics | Public UVS rows now prove pinned C and Rust agree that a non-default UVS mapping whose glyph ID is zero does not count as non-default coverage for `FT_Face_GetCharVariantIsDefault` or `FT_Face_GetVariantsOfChar`, while a selector with a non-default table and no default table is still a present selector for `FT_Face_GetCharsOfVariant`. A platform-0 Unicode active charmap is valid for default UVS glyph-index lookup. The remaining `tt/cmap.rs` format-14 missing lines are only host-width checked-arithmetic overflow closures, not missing public UVS semantics |
| 2026-07-12 | Route `FT_Get_Transform` pointer rows through real transform calls | Existing rows now apply `FT_Set_Transform` sequences and nullable `FT_Get_Transform` output pointers through the pinned C oracle and Rust runner. Pinned `ftobjs.c` resets a null matrix to identity and a null delta to zero in `FT_Set_Transform`; Rust core must match that before the fixture can prove `returns_last_set_transform`. Refreshed condition coverage is 14,722 / 17,290 lines, 21,457 / 24,862 regions, and 3,596 / 4,406 branches with 6,644 / 6,644 runtime rows passing and four explicit pending rows |
| 2026-07-12 | Route request-size divide guards through real parity | The existing `fonts/glyf/glyf-malformed-matrix.ttf` compact font has a zero head bbox. Two explicit `FT_SIZE_REQUEST_TYPE_BBOX` rows now drive pinned `ftobjs.c:3301-3317` height and width divide guards through `FT_Request_Size`, covering the Rust `SizeRequestError::DivideByZero` mapping without increasing concrete cases. Route audit moves real parity to 3,249 and generic fallback to 960; refreshed condition coverage is 14,727 / 17,290 lines, 21,462 / 24,862 regions, and 3,600 / 4,406 branches with 6,645 / 6,645 runtime rows passing and four explicit pending rows |
| 2026-07-12 | Route `FT_Get_Sfnt_LangTag` through real format-1 name data | Pinned `ftsnames.c` requires `langID > 0x8000` but indexes `langTags[langID - 0x8000]`, making language-tag record zero unreachable through the public API. The compact fixture therefore carries two records and selects `0x8001`; null output, format-0, `0x8000`, and upper-bound rows must call the real public function instead of generic fallback |
| 2026-07-12 | Retire stale public operation names only when an equivalent maintained route exists | The `FT_Load_Sfnt_Table` table-missing row now uses `sfnt.load_sfnt_table` with the existing compact SFNT input, moving one row from generic fallback to real parity without runtime-code changes or weakened comparison shape. Pathname-driven rows such as missing-resource and zero-byte `FT_New_Face`, plus missing-post `FT_Get_Glyph_Name`, stay generic until their exact C source path has a compact fixture and route-equivalent output shape |
| 2026-07-12 | Route OpenType validation null contracts through real parity | `FT_OpenType_Validate` now matches pinned `ftotval.c` early exits for null face and null output pointers, with exact error-output comparison enabled on those public rows. `FT_OpenType_Free` null-face and null-table rows now call pinned C and the Rust FFI wrapper instead of falling through generic modeled errors. Route audit moves real-null-validation to 8, generic fallback to 942, and generic-error fallback to 141; refreshed condition coverage is 15,216 / 17,756 lines, 22,079 / 25,444 regions, and 3,684 / 4,522 branches with 6,656 / 6,656 runtime rows passing and four explicit pending rows |
| 2026-07-12 | Treat malformed format-1 language-tag controls as memory-face parser parity | Two generated format-1 `name` table controls drive language-tag record-array overflow and string-range behavior through `FT_New_Memory_Face`. Pinned C rejects the truncated record array but opens the out-of-range string case after zeroing that tag's length; the 2026-07-17 source re-audit corrected the earlier two-error classification. Refreshed condition coverage was 15,223 / 17,756 lines, 22,085 / 25,444 regions, and 3,685 / 4,522 branches with 6,658 / 6,658 runtime rows passing and four explicit pending rows. |
| 2026-07-12 | Keep malformed format-1 language-tag count as parser-open parity | A format-1 `name` table may have a complete zero-record header while omitting the language-tag count field. The compact control belongs in `FT_New_Memory_Face` error parity because pinned C and Rust reject the face during open; refreshed condition coverage is 15,224 / 17,756 lines, 22,088 / 25,444 regions, and 3,685 / 4,522 branches with 6,659 / 6,659 runtime rows passing and four explicit pending rows |
| 2026-07-12 | Treat malformed name string offsets as fallback behavior when the face opens | Out-of-range individual name strings do not necessarily reject the SFNT face. Compact public rows should route the later public behavior instead: family-name selection can fall back from malformed Windows UTF-16BE to Apple Roman, while a malformed Apple PostScript-name record yields a null `FT_Get_Postscript_Name`; refreshed condition coverage is 15,225 / 17,756 lines, 22,092 / 25,444 regions, and 3,686 / 4,522 branches with 6,661 / 6,661 runtime rows passing and four explicit pending rows |
| 2026-07-12 | Expand the malformed format-14 cmap matrix in one public row | The existing `FT_Get_Char_Index` malformed format-14 row now reuses the same compact font but covers offset-out-of-range, selector-order, default-UVS, and non-default-UVS parser errors in addition to the original short/record-array cases. Case count stays flat at 6,665 concrete rows with zero implicit rows; refreshed condition coverage is 15,257 / 17,756 lines, 22,112 / 25,444 regions, and 3,693 / 4,522 branches with 6,661 / 6,661 runtime rows passing and four explicit pending rows |
| 2026-07-12 | Keep nested malformed format-14 probes from changing default Unicode selection | Pinned `tt_cmap14_validate` checks nested default/non-default UVS payload reads against the whole cmap table limit after requiring offsets below the declared format-14 length. If such records are tagged as Apple Unicode and followed by more cmap bytes, FreeType may register them and default-select a format-14 charmap, changing plain `FT_Get_Char_Index`. Extra nested-malformed parser probes therefore use non-Unicode platform records unless deliberately placed as the final physical cmap subtable |
| 2026-07-12 | Route cmap residual public zero guards explicitly | `cmap-format14-malformed-matrix.ttf` now ends with a physical short format-14 subtable so Rust reaches the true `b.len() < 10` parser guard instead of reading later cmap bytes. A compact `cmap-nonunicode-format6.ttf` plus one `FT_Face_GetCharVariantIndex` row proves C/Rust/C-ABI/WASM return zero when no Unicode active charmap exists, and one `FT_Face_GetCharVariantIsDefault` row proves absent selector `-1` behavior. Case count is 6,667 concrete rows with zero implicit rows; refreshed condition coverage is 15,260 / 17,756 lines, 22,115 / 25,444 regions, and 3,697 / 4,522 branches with 6,663 / 6,663 runtime rows passing and four explicit pending rows |
| 2026-07-12 | Route default glyph loads through the core wrapper | Pinned `tt_loader_init` suppresses `size->widthp` only for `FT_LOAD_COMPUTE_METRICS`; normal default loads retain the hdmx path. The public load path now routes non-compute-metrics rows through `glyph_slot_load_default_with_layout_and_mode` while preserving the explicit disabled-hdmx call for compute-metrics rows. Case count remains 6,667 concrete rows with zero implicit rows; refreshed condition coverage is 15,275 / 17,761 lines, 22,128 / 25,451 regions, and 3,699 / 4,524 branches with 6,663 / 6,663 runtime rows passing and four explicit pending rows |
| 2026-07-12 | Route face metadata through public helpers | Existing `FT_FaceRec` rows already compare scalar face metadata across Rust, C ABI, and WASM ABI. `Font::face_info` now delegates `num_faces`, `face_index`, and family/style names through the public helper methods instead of duplicating field access. Case count is unchanged and refreshed condition coverage reaches 15,285 / 17,762 lines, 22,142 / 25,456 regions, and 3,699 / 4,524 branches with 6,663 / 6,663 runtime rows passing and four explicit pending rows |
| 2026-07-12 | Add non-public format-14 platform controls | `cmap-format14-non-uvs-platforms.ttf` is generated from `build_cmap_fixtures.py` with a valid base Unicode cmap plus valid format-14 subtables on platform/encoding pairs other than Apple Unicode variation selectors `0/5`. Four explicit public UVS variants prove pinned C and Rust ignore those subtables for `FT_Face_GetCharVariantIsDefault`, `FT_Face_GetVariantSelectors`, `FT_Face_GetVariantsOfChar`, and `FT_Face_GetCharsOfVariant`. Case count rises only to 6,671 concrete rows with zero implicit rows; refreshed condition coverage is 15,289 / 17,762 lines, 22,146 / 25,456 regions, and 3,707 / 4,524 branches with 6,667 / 6,667 runtime rows passing and four explicit pending rows |
| 2026-07-12 | Split variants-of-char default and non-default UVS inputs | The existing `FT_Face_GetVariantsOfChar.char_with_variants_returns_selector_list` row now has explicit grouped variants for U+0041 default UVS coverage and U+0042 non-default UVS coverage in the same compact `cmap-format-language-matrix.ttf` fixture. This adds one concrete row and no font bytes, covering the non-default `glyph_id != 0` predicate in the public format-14 char-variants path. Refreshed condition coverage is 15,289 / 17,762 lines, 22,147 / 25,456 regions, and 3,710 / 4,524 branches with 6,668 / 6,668 runtime rows passing and four explicit pending rows |
| 2026-07-12 | Unicode charmap missing-selection parity | One explicit `FT_Select_Charmap.error_missing_encoding` row reuses `cmap-nonunicode-format6.ttf` and requests `FT_ENCODING_UNICODE`. Pinned C returns `FT_Err_Invalid_CharMap_Handle` through `find_unicode_charmap`; Rust previously returned success by falling back to charmap index 0. Core now separates constructor fallback from explicit Unicode selection, the FFI wrapper maps the missing Unicode selection to the C error code, and exact Rust/C ABI/WASM parity passes with 6,669 / 6,669 runtime rows. Refreshed condition coverage is 15,290 / 17,759 lines, 22,146 / 25,452 regions, and 3,711 / 4,524 branches with zero implicit rows and four explicit pending rows |
| 2026-07-12 | SFNT null-length stream-error row | One explicit `FT_Load_Sfnt_Table.offset_and_length_errors` variant reuses `basic-ttf.ttf` with `TTAG_head`, `length == NULL`, an allocated buffer, and an out-of-stream offset. The row covers the null-length stream-error return in the Rust FFI wrapper and compares pinned C, Rust FFI, C ABI, and WASM ABI output exactly. Refreshed condition coverage is 15,291 / 17,759 lines, 22,147 / 25,452 regions, and 3,711 / 4,524 branches with 6,670 / 6,670 runtime rows, zero implicit rows, and four explicit pending rows |
| 2026-07-12 | Make safe wrapper route checks input-declared | The `Face::load_glyph` agreement hook now reads `assert_api_load_glyph_agrees` from explicit public input rows instead of a hardcoded case-id list. Existing rows retain the same parity checks, and the normal/compute hdmx variants now both declare their safe-wrapper intent. This intentionally did not change concrete case count or condition coverage; it removes hidden custom routing before the next coverage rows |
| 2026-07-12 | Route safe `Font::render_char_mode` through existing render-mode rows | Three existing render-mode dispatch rows (`NORMAL`, `LCD`, `LCD_V`) now declare `assert_font_render_mode_agrees` and compare `Font::render_char_mode("A", mode)` against the same C-oracle-backed `FT_Render_Glyph` bitmap JSON. The `MONO` direct render row is deliberately not opted in because `Font::render_char_mode(Mono)` loads with mono hinting while `FT_Render_Glyph(..., MONO)` renders the already-loaded default-hinted slot. Case count stays flat at 6,674 concrete rows with zero implicit rows; refreshed condition coverage is 15,391 / 17,759 lines, 22,245 / 25,452 regions, and 3,715 / 4,524 branches with 6,670 / 6,670 runtime rows and four explicit pending rows |
| 2026-07-12 | Add SDF and target-mono safe render agreement | The existing SDF render-mode row now opts into the safe render agreement hook, and `FT_RENDER_MODE_MONO` adds one explicit grouped variant that loads with `FT_LOAD_TARGET_MONO` before rendering so it matches `Font::render_char_mode(Mono)` semantics. The hook now compares both public safe render entry points, `Font::render_char_mode` and non-empty `Font::render_mode`, against the same C-oracle-backed bitmap JSON. Case count is 6,675 concrete rows with zero implicit rows; refreshed condition coverage is 15,408 / 17,759 lines, 22,271 / 25,452 regions, and 3,716 / 4,524 branches with 6,671 / 6,671 runtime rows and four explicit pending rows |
| 2026-07-12 | Add empty-outline safe render rows | Four explicit render-mode variants reuse DejaVuSans U+0020 to exercise safe `Font` empty-outline rendering for NORMAL, MONO, LCD_V, and SDF. C `FT_Render_Glyph` exposes no bitmap for the zero-sized modes, so the agreement hook now treats C `bitmap == null` as an explicit canonical-empty safe API assertion instead of byte equality; MONO remains strict bitmap equality. The same NORMAL row asserts `Font::render_mode("")` returns the canonical empty bitmap. The attempted LCD empty-outline row is intentionally not added: safe `Font::render_char_mode(' ', Lcd)` currently returns an LCD-shaped zero-row bitmap while C exposes no bitmap, which is a correctness item to evaluate separately. Case count is 6,679 concrete rows with zero implicit rows; refreshed condition coverage is 15,444 / 17,759 lines, 22,288 / 25,452 regions, and 3,722 / 4,524 branches with 6,675 / 6,675 runtime rows and four explicit pending rows |
| 2026-07-12 | Cover LCD empty-outline safe render parity | One explicit `FT_RENDER_MODE_LCD.render_glyph_mode_dispatch` variant reuses DejaVuSans U+0020 and opts into the safe `Font` render agreement hook. The row exposed the previously tracked divergence where `Font::render_char_mode(' ', Lcd)` returned an LCD-shaped zero-row bitmap while C `FT_Render_Glyph` exposed no bitmap; core now routes empty LCD safe rendering through the same canonical empty-outline result as loaded-outline rendering. Case count is 6,680 concrete rows with zero implicit rows; refreshed condition coverage is 15,445 / 17,759 lines, 22,290 / 25,450 regions, and 3,723 / 4,524 branches with 6,676 / 6,676 runtime rows and four explicit pending rows |
| 2026-07-12 | Declare safe `Font` convenience parity on existing rows | Existing `FT_RENDER_MODE_NORMAL.render_glyph_mode_dispatch` variants now opt into safe `Font::getmetrics`, `getlength`, `getbbox`, `getmask`, and empty-text mask agreement checks using the same C-oracle-backed size metrics, glyph slot advance, and normal rendered bitmap already produced by the row. Existing `FT_Get_Kerning.legacy_pair_unfitted_and_unscaled_modes` now opts into `Font::getkerning` agreement against the row's `FT_KERNING_UNFITTED` vector. This adds no fonts and no concrete cases; refreshed condition coverage is 15,574 / 17,759 lines, 22,460 / 25,450 regions, and 3,735 / 4,524 branches with 6,676 / 6,676 runtime rows and four explicit pending rows |
| 2026-07-12 | Cover safe glyph metrics and monochrome render-target declarations | The existing normal render row now also compares `Font::glyph_metrics('A')` with the same C-oracle-backed glyph-slot metrics already produced by `FT_Render_Glyph`. `FT_Load_Glyph.render_and_target_modes` adds the missing normal `FT_LOAD_RENDER` + `FT_LOAD_MONOCHROME` variant and marks all four monochrome target combinations for safe `Face::load_glyph` agreement. This adds one concrete input and no fonts; refreshed condition coverage is 15,578 / 17,759 lines, 22,469 / 25,450 regions, and 3,735 / 4,524 branches with 6,677 / 6,677 runtime rows and four explicit pending rows |
| 2026-07-12 | Cover safe charmap accessor parity | Existing `FT_Get_Charmap_Index.owned_charmap_indexes` now opts into a safe `Font` charmap assertion. The assertion compares `Font::charmaps`, `charmap`, `charmap_index`, and successful `set_charmap`/`select_charmap` against the same public charmap-index rows already compared across pinned C, Rust FFI, C ABI, and WASM ABI, then checks the safe API's explicit invalid index and missing-pair guards. This adds no fonts and no concrete cases; refreshed condition coverage is 15,587 / 17,759 lines, 22,477 / 25,450 regions, and 3,737 / 4,524 branches with 6,677 / 6,677 runtime rows and four explicit pending rows |
| 2026-07-12 | Cover safe default font constructor and face count helper | Existing `FT_RENDER_MODE_NORMAL.render_glyph_mode_dispatch` now declares safe `Font::truetype` constructor coverage for the default load-mode row and compares `Font::face_count` with the same face's public `FT_FaceRec.num_faces` view while the row continues to compare rendered glyph output across pinned C, Rust FFI, C ABI, and WASM ABI. This adds no fonts and no concrete cases; refreshed condition coverage is 15,593 / 17,759 lines, 22,488 / 25,450 regions, and 3,737 / 4,524 branches with 6,677 / 6,677 runtime rows and four explicit pending rows |
| 2026-07-12 | Cover public render and pixel fixture-name helpers | Existing render-mode dispatch rows now declare `assert_render_mode_fixture_name` and `assert_pixel_mode_fixture_name` for normal, mono, LCD, vertical LCD, and SDF outputs. The assertions run only from explicit public input rows that already compare the rendered glyph output across pinned C, Rust FFI, C ABI, and WASM ABI, so this adds no fonts and no concrete cases; refreshed condition coverage is 15,608 / 17,759 lines, 22,503 / 25,450 regions, and 3,737 / 4,524 branches with 6,677 / 6,677 runtime rows and four explicit pending rows |
| 2026-07-12 | Add missing-name-table constructor control | `scripts/build_name_fixtures.py` now generates `fonts/names/name-missing.ttf` by removing the optional `name` table from the compact base TrueType font, and `FT_New_Memory_Face.valid_font_bytes` selects it as one explicit variant. This covers the constructor fallback separate from the existing zero-record `name-empty.ttf` control; concrete cases rise by one to 6,682, and refreshed condition coverage is 15,616 / 17,759 lines, 22,509 / 25,450 regions, and 3,737 / 4,524 branches with 6,678 / 6,678 runtime rows and four explicit pending rows |
| 2026-07-12 | Route safe kerning assertion through cached-face execution | `FT_Get_Kerning.legacy_pair_unfitted_and_unscaled_modes` already declared `assert_font_getkerning_agrees`, but the cached-face runner path bypassed the assertion helper. The Rust route now honors the existing input declaration and compares `Font::getkerning('A', 'V')` with the same `FT_KERNING_UNFITTED` vector returned by pinned C, C ABI, and WASM ABI. This adds no fonts and no concrete cases; refreshed condition coverage is 15,628 / 17,759 lines, 22,539 / 25,450 regions, and 3,737 / 4,524 branches with 6,678 / 6,678 runtime rows and four explicit pending rows |
| 2026-07-12 | Cover safe horizontal advance helper from public advance row | Existing `FT_Get_Advance.success_horizontal_scaled_advance` now marks the DejaVuSans `A` no-hinting variant with `assert_font_hori_advance_agrees` and `advance_codepoint: 65`. The Rust runner compares `Font::glyph_hori_advance_26dot6(U+0041)` with the same C-oracle-backed `FT_Get_Advance` 16.16 value rounded to 26.6, while keeping C ABI and WASM ABI wrappers unchanged. This adds no fonts and no concrete cases; refreshed condition coverage is 15,636 / 17,759 lines, 22,556 / 25,450 regions, and 3,737 / 4,524 branches with 6,678 / 6,678 runtime rows and four explicit pending rows |
| 2026-07-12 | Route safe load-mode rendering through public render matrix | Existing `FT_Render_Glyph.matrix_render` variants for no-hinting, force-autohint, target-light, and no-autohint now declare `assert_font_render_mode_agrees` with matching `font_render_text` values. The assertions compare safe `Font::render_char_mode` and `Font::render_mode` through the same load modes against the already C-oracle-backed rendered glyph rows, adding no fonts and no concrete cases. Refreshed condition coverage is 15,706 / 17,759 lines, 22,620 / 25,450 regions, and 3,739 / 4,524 branches with 6,678 / 6,678 runtime rows and four explicit pending rows |
| 2026-07-12 | Route force-autohint target render modes through public render matrix | `FT_Render_Glyph.matrix_render` adds three explicit DejaVuSans `A` variants for `FT_LOAD_FORCE_AUTOHINT` combined with `FT_LOAD_TARGET_MONO`, `FT_LOAD_TARGET_LCD`, and `FT_LOAD_TARGET_LCD_V`, each asserting safe `Font` render agreement against the same C-oracle-backed rendered glyph row. This adds no fonts, raises concrete cases to 6,685, keeps implicit cases at zero, and refreshed condition coverage is 15,781 / 17,759 lines, 22,665 / 25,450 regions, and 3,739 / 4,524 branches with 6,681 / 6,681 runtime rows and four explicit pending rows |
| 2026-07-12 | Delegate pre-rendered bitmap slots through core render | `FT_Render_Glyph` in the C ABI wrapper no longer returns before core when the slot is already bitmap-formatted. This removes duplicate wrapper behavior, keeps the ABI layer thin, preserves the original bitmap no-op load flags from pinned `FT_Render_Glyph_Internal`, and lets existing pre-rendered public render rows exercise the core `GlyphSlot::render` bitmap no-op path through Rust FFI, C ABI, and WASM ABI. No fonts or cases were added; refreshed condition coverage is 15,781 / 17,758 lines, 22,667 / 25,451 regions, and 3,740 / 4,524 branches with 6,681 / 6,681 runtime rows and four explicit pending rows |
| 2026-07-12 | Compact render topology branch probes | `FT_Render_Glyph.matrix_render` adds five explicit variants over the existing source-backed `hinter-control-matrix.ttf` render glyphs: conic-chain normal rendering, mixed-winding normal rendering, bowtie mono rendering, and mono scan-type 4/5 glyphs. Each row uses `FT_LOAD_NO_HINTING` to isolate outline decomposition and rasterization from bytecode. This adds no fonts, raises concrete cases to 6,691, keeps implicit cases at zero, and exact Rust FFI, C ABI, and WASM ABI parity passes with 6,687 / 6,687 runtime rows and four explicit pending rows. Refreshed condition coverage is 15,787 / 17,764 lines, 22,669 / 25,453 regions, and 3,742 / 4,524 branches: no line/region/function delta, one additional branch outcome |
| 2026-07-12 | CJK empty standard-width fallback | `build_autohint_script_fixtures.py` now emits a 1.0 KiB `fonts/autohint/cjk-empty-standard.ttf` where U+7530 maps to a contourless Hani glyph. One explicit `FT_LOAD_FORCE_AUTOHINT` public variant selects it through `FT_Load_Char`, covering the CJK no-width standard fallback without mutating productive U+7530 geometry in `cjk-coverage.ttf`. Concrete cases rise to 6,692, implicit cases stay zero, and exact Rust FFI, C ABI, and WASM ABI parity passes with 6,688 / 6,688 runtime rows and four explicit pending rows. Refreshed condition coverage is 15,788 / 17,764 lines, 22,670 / 25,453 regions, and 3,743 / 4,524 branches |
| 2026-07-12 | CJK blue-zone edge fixture | `build_autohint_script_fixtures.py` now emits a 1.2 KiB `fonts/autohint/cjk-blue-edge-cases.ttf` where U+4ED6 is contourless, U+519B is the only usable top flat probe, and U+4E2A/U+4E3B invert bottom fill/flat ordering. One explicit `FT_LOAD_FORCE_AUTOHINT` public variant selects U+7530 from that font, covering the CJK empty-blue-glyph skip, flat-only blue zone, and ref/shoot order repair without mutating productive CJK coverage glyphs. Concrete cases rise to 6,693, implicit cases stay zero, and exact Rust FFI, C ABI, and WASM ABI parity passes with 6,689 / 6,689 runtime rows and four explicit pending rows. Refreshed condition coverage is 15,794 / 17,764 lines, 22,678 / 25,453 regions, and 3,747 / 4,524 branches |
| 2026-07-12 | CJK degenerate blue contours | The existing `cjk-blue-edge-cases.ttf` generated fixture now also maps U+4EEC to three one-point contours. The same explicit `cjk-blue-edge-cases-20` public row reaches the CJK contour-length guard and no-best-position fallback during blue initialization, adding no concrete cases and preserving exact Rust FFI, C ABI, and WASM ABI parity with 6,689 / 6,689 runtime rows and four explicit pending rows. Refreshed condition coverage is 15,796 / 17,764 lines, 22,680 / 25,453 regions, and 3,749 / 4,524 branches |
| 2026-07-12 | CJK tiny standard stem clamp | `build_autohint_script_fixtures.py` now emits a 1.0 KiB `fonts/autohint/cjk-tiny-stem.ttf` where U+7530 has a 20-unit vertical stem. One explicit `FT_LOAD_FORCE_AUTOHINT` public variant selects it at 20 ppem, proving CJK's minimum snapped standard-width clamp with exact Rust FFI, C ABI, and WASM ABI parity. Concrete cases rise to 6,694, implicit cases stay zero, runtime parity is 6,690 / 6,690 with four explicit pending rows, and refreshed condition coverage is 15,797 / 17,764 lines, 22,681 / 25,453 regions, and 3,750 / 4,524 branches |
| 2026-07-12 | CJK snap-below standard width | `build_autohint_script_fixtures.py` now emits `fonts/autohint/cjk-snap-below-standard.ttf`, keeping U+7530 as a 100-unit Hani standard stem and mapping U+4ED6 to a 90-unit Hani stem. One explicit `FT_LOAD_FORCE_AUTOHINT | FT_LOAD_TARGET_MONO` public variant selects U+4ED6 at 20 ppem, proving the lower-side `cjk_snap_width` reference branch through exact Rust FFI, C ABI, and WASM ABI parity. Concrete cases rise to 6,695, implicit cases stay zero, runtime parity is 6,691 / 6,691 with four explicit pending rows, and refreshed condition coverage is 15,800 / 17,764 lines, 22,683 / 25,453 regions, and 3,752 / 4,524 branches |
| 2026-07-12 | CJK round-round LIGHT threshold and target-light mask | `FT_Render_Glyph.matrix_render` adds one explicit `FT_LOAD_TARGET_LIGHT | FT_LOAD_NO_AUTOHINT` row, proving the `render_font` target-light condition's masked branch without adding a new axis. `build_autohint_script_fixtures.py` now emits `fonts/autohint/cjk-round-stem-light.ttf`, keeping U+7530 as a standard Hani stem and mapping U+51A2 to a quadratic ring selected by one `FT_LOAD_FORCE_AUTOHINT | FT_LOAD_TARGET_LIGHT` public row. The CJK row covers the no-stem-adjust round-round threshold for both dimensions. Concrete cases rise to 6,697, implicit cases stay zero, runtime parity is 6,693 / 6,693 with four explicit pending rows, and refreshed condition coverage is 15,803 / 17,764 lines, 22,687 / 25,453 regions, and 3,757 / 4,524 branches |
| 2026-07-12 | CJK duplicate-edge compatibility rejection | `build_autohint_script_fixtures.py` now emits `fonts/autohint/cjk-duplicate-edge.ttf`, where U+519E has two Hani rectangles sharing the same major edge position while their linked opposite edges are far apart. One explicit `FT_LOAD_FORCE_AUTOHINT` row reaches CJK edge grouping's linked-segment compatibility rejection and same-position major-edge insertion, and also exercises later skipped-edge interpolation from that topology. Concrete cases rise to 6,698, implicit cases stay zero, runtime parity is 6,694 / 6,694 with four explicit pending rows, and refreshed condition coverage is 15,812 / 17,764 lines, 22,699 / 25,453 regions, and 3,763 / 4,524 branches |
| 2026-07-12 | CJK leading skipped-edge interpolation | `cjk-duplicate-edge.ttf` now also maps U+51A4 to a Hani glyph with a short unlinked leading rectangle before a normal linked stem. One explicit `FT_LOAD_FORCE_AUTOHINT` row reaches the skipped-edge interpolation path where no previous done edge exists and a later done edge anchors the skipped edge. No new font file is added; concrete cases rise to 6,699, implicit cases stay zero, runtime parity is 6,695 / 6,695 with four explicit pending rows, and refreshed condition coverage is 15,820 / 17,764 lines, 22,709 / 25,453 regions, and 3,766 / 4,524 branches |
| 2026-07-13 | Verified render/scaler/autohint/name/CFF coverage checkpoint | Merged and pushed the compact OTTO metadata row, variable-name metadata rows, Latin small-ignore autohint fixture row, mono composite scaler fix and rows, SDF tiny-segment render fix and row, FT_Long::MIN face-index row, autohint script blue-string aliases and double-top probe, explicit safe `Face::load_glyph` route declarations, and SDF conic-chain render row. Concrete cases are 6,717 with zero implicit rows; runtime parity is 6,713 / 6,713 with four explicit pending rows. Refreshed condition coverage is 15,840 / 17,766 lines, 22,747 / 25,457 regions, and 3,791 / 4,524 branches. Route audit reports real-parity 3,352, compile-contract 2,229, generic-fallback 942, generic-error-fallback 141, pending-core 10, explicit-unsupported 12, real-null-validation 8, null-error-fallback 21, and void-fallback 2 |
| 2026-07-13 | No-scale outline render probe | `FT_Render_Glyph.matrix_render` adds one explicit `hinter-control-matrix.ttf` glyph 41 row with `FT_LOAD_NO_SCALE` and normal rendering. This keeps the probe in the existing compact render matrix, adds no font bytes, and covers one additional `grays.rs` scan-conversion line, region, and branch. Concrete cases are 6,718 with zero implicit rows; runtime parity is 6,714 / 6,714 with four explicit pending rows. Refreshed condition coverage is 15,841 / 17,766 lines, 22,748 / 25,457 regions, and 3,792 / 4,524 branches. Route audit reports the new row as real parity, raising real-parity routes to 3,353 |
| 2026-07-13 | Explicit request-size ppem overflow | `FT_Request_Size` adds one explicit `params.request` row with nominal 65536 px width/height, avoiding the probe-only `params.variants` route and covering `font.rs::ppem_from_scaled_26dot6` invalid-pixel-size branch. Concrete cases are 6,719 with zero implicit rows; runtime parity is 6,715 / 6,715 with four explicit pending rows. Refreshed condition coverage is 15,842 / 17,766 lines, 22,750 / 25,457 regions, and 3,793 / 4,524 branches. Route audit reports the new row as real parity, raising real-parity routes to 3,354 |
| 2026-07-13 | Explicit fvar PostScript-name branches | `FT_Get_Postscript_Name.variation_instance_name_behavior` adds one explicit encoded named-instance row over `fvar-instance-postscript-name.ttf`, covering the fvar instance `postscriptNameID` fast path. The generated `variable-name-missing-subfamily.ttf` fixture also changes its existing fractional named-instance coordinate from 1.0 to 0.5 so the already declared fractional row reaches the 16.16 decimal early-termination branch. Concrete cases are 6,720 with zero implicit rows; runtime parity is 6,716 / 6,716 with four explicit pending rows. Refreshed condition coverage is 15,846 / 17,766 lines, 22,755 / 25,457 regions, and 3,795 / 4,524 branches. Route audit reports the new row as real parity, raising real-parity routes to 3,355 |
| 2026-07-13 | Hhea-zero no-OS2 metric fallback | `build_metric_fixtures.py` now emits a compact `fonts/metrics/hhea-zero-no-os2-fallback.ttf` by clearing hhea ascent/descent/lineGap and removing OS/2. One explicit `FT_Size_Metrics.face_scaling_and_fallbacks` variant proves pinned C, Rust FFI, C ABI, and WASM ABI all return the zero face-metric fallback when no OS/2 metrics are available. Concrete cases are 6,721 with zero implicit rows; runtime parity is 6,717 / 6,717 with four explicit pending rows. Refreshed condition coverage is 15,847 / 17,766 lines, 22,758 / 25,457 regions, and 3,796 / 4,524 branches. Route audit reports the new row as real parity, raising real-parity routes to 3,356 |
| 2026-07-13 | Route null subglyph validation through real wrapper | The existing `FT_Get_SubGlyph_Info.error_null_slot` public row now bypasses the generic null-error shortcut and calls the real Rust FFI `FT_Get_SubGlyph_Info(None, ...)` path while C ABI and WASM ABI continue to compare the same public error. This adds no fonts and no concrete cases, converts a false-green null-slot branch into real wrapper execution, and covers the core null-glyph validation line. Concrete cases remain 6,721 with zero implicit rows; runtime parity is 6,717 / 6,717 with four explicit pending rows. Refreshed condition coverage is 15,848 / 17,766 lines, 22,759 / 25,457 regions, and 3,797 / 4,524 branches |
| 2026-07-13 | Add explicit subglyph null-output wrapper validation | `FT_Get_SubGlyph_Info.error_null_outputs` reuses `glyf-component-matrix.ttf` glyph 4 and explicitly passes null `index`, `flags`, `arg1`, `arg2`, and `transform` outputs one at a time. Pinned `ftobjs.c:5690-5719` dereferences all output pointers after validating a composite slot, so the native C oracle first proves the subglyph with non-null outputs and then classifies the null-output behavior as Rust FFI/C ABI/WASM ABI wrapper validation rather than native-C parity. Concrete cases are 6,722 with zero implicit rows; runtime comparison is 6,718 / 6,718 with four explicit pending rows. Refreshed condition coverage is 15,849 / 17,766 lines, 22,760 / 25,457 regions, and 3,798 / 4,524 branches. Route audit reports `wrapper-null-validation: 1` and keeps real-parity routes at 3,356 |
| 2026-07-13 | Route `FT_Select_Size` unsupported rows through the core stub | Existing `FT_Select_Size` rows remain `explicit-unsupported` because the core function is still an unimplemented stub and C/WASM ABI exports do not exist yet. The unified Rust runner now calls `FT_Select_Size(None, strike_index)` instead of returning a synthetic error, so the fixture covers the real preserved stub without claiming native-C parity or adding fonts/cases. Concrete cases remain 6,722 with zero implicit rows; runtime comparison is 6,718 / 6,718 with four explicit pending rows. Refreshed condition coverage is 15,852 / 17,766 lines, 22,763 / 25,457 regions, 3,798 / 4,524 branches, and 1,003 / 1,135 functions |
| 2026-07-13 | Cover public `FT_StreamDesc` default construction | Existing `ftsystem.FT_StreamRec.layout_matches_c` now declares and compares a `default_state` object from both the C oracle and Rust layout route. The Rust path safely constructs `FT_StreamDesc::default` and `FT_StreamRec::default` without reading union arms, proving the ABI default constructor path while preserving `#![deny(unsafe_code)]`. No fonts or concrete cases were added. Runtime comparison remains 6,718 / 6,718 with four explicit pending rows. Refreshed condition coverage is 15,857 / 17,766 lines, 22,766 / 25,457 regions, 3,798 / 4,524 branches, and 1,004 / 1,135 functions; `src/ffi/types.rs` is now 5 / 5 lines and 1 / 1 functions |
| 2026-07-13 | Extend render-target safe load assertions | Existing `FT_Load_Glyph.render_and_target_modes` variants for direct mono, LCD, and vertical LCD render-load now assert safe `Face::load_glyph` slot equality against the same C-oracle-backed Rust FFI, C ABI, and WASM ABI row. No fonts or concrete cases were added. The filtered case passed 8 / 8, and refreshed full condition coverage remained 15,857 / 17,766 lines, 22,766 / 25,457 regions, 3,798 / 4,524 branches, and 1,004 / 1,135 functions with 6,718 / 6,718 runtime rows and four explicit pending rows. The unchanged structural counters show this is correctness hardening, not a coverage-denominator shortcut |
| 2026-07-13 | Malformed load-glyph error routes | `FT_Load_Glyph.load-error-cases` now adds three explicit source-backed variants over `fonts/glyf/glyf-malformed-matrix.ttf` for `FT_LOAD_DEFAULT`, `FT_LOAD_TARGET_LIGHT`, and `FT_LOAD_COMPUTE_METRICS`. Each row compares the C-oracle-backed Rust FFI, C ABI, WASM ABI, and safe `Face::load_glyph` error path instead of broadening glyph discovery. Concrete cases rise to 6,725 with zero implicit rows; runtime comparison is 6,721 / 6,721 with four explicit pending rows. Refreshed condition coverage is 15,861 / 17,766 lines, 22,771 / 25,457 regions, 3,798 / 4,524 branches, and 1,004 / 1,135 functions |
| 2026-07-13 | Reject render variants without coverage movement | Candidate `FT_Render_Glyph.matrix_render` variants for conic-chain mono, bowtie SDF, and mixed-winding SDF over `hinter-control-matrix.ttf` passed exact parity but did not move line, region, function, or branch coverage. They were removed before commit so the optimized fixture set does not grow with rows that add no measured value |
| 2026-07-13 | Route OpenType validate preserved stub through Rust wrapper | Existing non-null `FT_OpenType_Validate` public rows now call the preserved Rust FFI wrapper with all five output pointers instead of returning the modeled `Unimplemented_Feature` error directly from the parity runner. This adds no fonts and no concrete cases, keeps the row visibly unsupported until real OpenType validation exists, and covers the real non-null wrapper branch. Concrete cases remain 6,725 with zero implicit rows; runtime comparison is 6,721 / 6,721 with four explicit pending rows. Refreshed condition coverage is 15,863 / 17,766 lines, 22,773 / 25,457 regions, 3,799 / 4,524 branches, and 1,004 / 1,135 functions |
| 2026-07-13 | No-scale zero-extent normal render probes | `FT_Render_Glyph.matrix_render` adds two explicit variants over the existing source-backed `hinter-control-matrix.ttf` render topology glyphs: `renderZeroWidth` and `renderZeroHeight` with `FT_LOAD_NO_SCALE` and normal rendering. These rows add no font bytes, preserve exact Rust FFI, C ABI, and WASM ABI parity, cover the `render_normal` zero-extent return block, and split the `width == 0 || height == 0` condition across both sides. Concrete cases are 6,727 with zero implicit rows; runtime comparison is 6,723 / 6,723 with four explicit pending rows. Refreshed condition coverage is 15,873 / 17,766 lines, 22,778 / 25,457 regions, 3,801 / 4,524 branches, and 1,004 / 1,135 functions |
| 2026-07-13 | Empty font-program native prepare fixture | `font-fixture-hinter` now builds `fonts/glyf/hinter-empty-fpgm.ttf` as a derived compact TrueType fixture with empty `fpgm`, non-empty `prep`, present `cvt`, and the same glyph programs as `hinter-control-matrix.ttf`. One explicit `FT_Load_Glyph.matrix_load` row selects gid 1 with `FT_LOAD_NO_AUTOHINT`, proving the native TrueType prepare path that skips `ctx.run_fpgm()` but still runs prep. Concrete cases are 6,728 with zero implicit rows; runtime comparison is 6,724 / 6,724 with four explicit pending rows. Refreshed condition coverage is 15,874 / 17,766 lines, 22,779 / 25,457 regions, 3,802 / 4,524 branches, and 1,004 / 1,135 functions |
| 2026-07-13 | Prep definition maxp-budget errors | `hinter-prep-definitions.ttf` and `hinter-prep-idef.ttf` are derived compact TrueType controls whose prep programs attempt additional FDEF/IDEF definitions after the base font program has consumed the font's maxp definition budgets. Pinned C returns definition-budget errors for these public loads, while Rust previously accepted the FDEF case; the two explicit `FT_Load_Glyph.matrix_load` rows prove exact Rust FFI, C ABI, and WASM ABI error parity. Concrete cases are 6,730 with zero implicit rows; runtime comparison is 6,726 / 6,726 with four explicit pending rows. Refreshed condition coverage is 15,873 / 17,765 lines, 22,782 / 25,454 regions, 3,803 / 4,526 branches, and 1,004 / 1,135 functions; missed source lines remain at 1,892 while this fixes a real C/Rust mismatch |
| 2026-07-13 | TrueType fpgm definition scanner and LOOPCALL fixtures | `tt/maxp.rs` now parses `maxFunctionDefs` and `maxInstructionDefs`, and `ExecContext` enforces FreeType's `ttinterp.c` definition rules: FDEF/IDEF are allowed in `fpgm` and `prep`, glyph-range definitions fail, maxp budgets are honored, and nested FDEF/IDEF opcodes return `Nested_DEFS`. `font-fixture-hinter` now emits six compact derived controls: one no-output LOOPCALL success font and five fpgm scanner error fonts for nested FDEF, nested IDEF, out-of-range IDEF opcode, unterminated FDEF, and unterminated IDEF. Six explicit `FT_Load_Glyph.matrix_load` rows prove exact Rust FFI, C ABI, and WASM ABI parity. Concrete cases are 6,736 with zero implicit rows; runtime comparison is 6,732 / 6,732 with four explicit pending rows. Refreshed condition coverage is 15,920 / 17,810 lines, 22,820 / 25,492 regions, 3,813 / 4,536 branches, and 1,004 / 1,135 functions; missed source lines drop to 1,890 |
| 2026-07-13 | Successful prep-range definition redefinition | `hinter-prep-redefine-defs.ttf` is a compact derived TrueType control whose prep program redefines existing FDEF 1 and IDEF 0x8F so it stays within the font's maxp definition budgets. This proves the corrected FreeType rule that definitions are allowed in `prep` when budgets permit, while the existing fpgm nested-FDEF control now redefines FDEF 1 before nesting so it reaches `Nested_DEFS` instead of a budget error. One explicit `FT_Load_Glyph.matrix_load` row proves exact Rust FFI, C ABI, and WASM ABI parity. Concrete cases are 6,737 with zero implicit rows; runtime comparison is 6,733 / 6,733 with four explicit pending rows. Refreshed condition coverage is 15,927 / 17,810 lines, 22,825 / 25,492 regions, 3,817 / 4,536 branches, and 1,004 / 1,135 functions; missed source lines drop to 1,883 |
| 2026-07-13 | Variation PostScript decimal rounding controls | `build_name_fixtures.py` now extends `variable-name-missing-subfamily.ttf` with encoded named instances 5-8, covering compact 16.16 coordinate cases for last-decimal-one, decrement-after-round, tie-even, and negative fractional synthesized PostScript suffixes. Four explicit `FT_Get_Postscript_Name.variation_instance_name_behavior` rows prove pinned C, Rust FFI, C ABI, and WASM ABI parity without adding a new font path. Concrete cases are 6,741 with zero implicit rows; runtime comparison is 6,737 / 6,737 with four explicit pending rows. Refreshed condition coverage is 15,930 / 17,810 lines, 22,831 / 25,492 regions, 3,824 / 4,536 branches, and 1,004 / 1,135 functions; missed source lines drop to 1,880 |
| 2026-07-13 | Asset-backed transform null-face wrapper route | `FT_Set_Transform.success_null_face_noop` now has one explicit asset-backed variant over `input/fonts/DejaVuSans.ttf` while still passing `face: null`. Pinned FreeType treats `FT_Set_Transform(NULL, matrix, delta)` as a void no-op; the added row avoids the assetless shortcut and proves the real thin Rust FFI, C ABI, and WASM ABI wrapper path without adding implementation logic or font bytes. Concrete cases are 6,742 with zero implicit rows; runtime comparison is 6,738 / 6,738 with four explicit pending rows. Refreshed condition coverage is 15,931 / 17,810 lines, 22,832 / 25,492 regions, 3,825 / 4,536 branches, and 1,004 / 1,135 functions; missed source lines drop to 1,879 |
| 2026-07-13 | Latin standard-width cluster fixture | `build_autohint_script_fixtures.py` now emits `fonts/autohint/latin-width-clusters.ttf`, a three-stem U+006F standard glyph selected by one explicit `FT_Load_Glyph.matrix_load` row with `FT_LOAD_FORCE_AUTOHINT`. The row reaches the Latin standard-width sort-and-quantize branch that advances from one width cluster to a later cluster, without adding a glyph loop or product axis. Concrete cases are 6,743 with zero implicit rows; runtime comparison is 6,739 / 6,739 with four explicit pending rows. Refreshed condition coverage is 15,934 / 17,810 lines, 22,834 / 25,492 regions, 3,828 / 4,536 branches, and 1,004 / 1,135 functions; missed source lines drop to 1,876 |
| 2026-07-13 | Render mono horizontal dropout fixture | `build_render_fixtures.py` now emits `fonts/glyf/render-coverage.ttf`, a 1024 UPEM compact TrueType fixture whose 16 ppem coordinates place vertical strokes before a thin horizontal mono dropout row. One explicit `FT_Render_Glyph.matrix_render` variant selects glyph 1 with `FT_LOAD_NO_HINTING` and `FT_RENDER_MODE_MONO`, covering the horizontal dropout alternate-set guard without a render-mode product axis. Concrete cases are 6,744 with zero implicit rows; runtime comparison is 6,740 / 6,740 with four explicit pending rows. Refreshed condition coverage is 15,947 / 17,810 lines, 22,852 / 25,492 regions, 3,831 / 4,536 branches, and 1,005 / 1,135 functions; missed source lines drop to 1,863 |
| 2026-07-13 | Render mono vertical dropout fixture | The same compact `render-coverage.ttf` now carries glyph 2, a mirrored dropout shape whose horizontal strokes pre-set pixels before a thin vertical mono dropout. One explicit `FT_Render_Glyph.matrix_render` variant selects glyph 2 with `FT_LOAD_NO_HINTING` and `FT_RENDER_MODE_MONO`, covering the vertical dropout alternate-set condition without a render-mode or glyph loop. Concrete cases are 6,745 with zero implicit rows; runtime comparison is 6,741 / 6,741 with four explicit pending rows. Refreshed condition coverage is 15,947 / 17,810 lines, 22,852 / 25,492 regions, 3,832 / 4,536 branches, and 1,005 / 1,135 functions; this is a branch-only condition coverage win and missed source lines remain 1,863 |
| 2026-07-13 | CJK multi-width snap fixture | `build_autohint_script_fixtures.py` now emits `fonts/autohint/cjk-multi-width-snap.ttf`, where the Hani standard glyph U+7530 has two vertical stem widths and U+4ED6 uses the narrower stem. One explicit `FT_LOAD_FORCE_AUTOHINT | FT_LOAD_TARGET_MONO` public row selects U+4ED6, covering the CJK `cjk_snap_width` later-width-not-closer branch without a script or glyph loop. Concrete cases are 6,746 with zero implicit rows; runtime comparison is 6,742 / 6,742 with four explicit pending rows. Refreshed condition coverage is 15,947 / 17,810 lines, 22,853 / 25,492 regions, 3,833 / 4,536 branches, and 1,005 / 1,135 functions; this is a branch-only condition coverage win and missed source lines remain 1,863 |
| 2026-07-13 | Scaler conic bbox left/top extrema row | `build_render_fixtures.py` extends the same compact `fonts/glyf/render-coverage.ttf` with glyph 3, a single quadratic contour whose off-curve control sits left of and above the on-curve endpoints. One explicit `FT_Load_Glyph.matrix_load` row selects it with `FT_LOAD_NO_HINTING`, proving exact Rust FFI, C ABI, and WASM ABI parity while covering the two remaining `bbox_conic_to` extrema condition outcomes in `scaler.rs`. Concrete cases are 6,756 with zero implicit rows; runtime comparison is 6,752 / 6,752 with four explicit pending rows. Refreshed condition coverage is 16,215 / 18,091 lines, 23,284 / 25,933 regions, 3,908 / 4,632 branches, and 1,024 / 1,150 functions; `scaler.rs` branch coverage moves from 156 / 188 to 158 / 188 |
| 2026-07-13 | Ftsynth zero-strength outline row | Existing `FT_GlyphSlot_AdjustWeight.outline_weight_adjusts_points_metrics_and_advances` now includes DejaVuSans glyph 3 and a zero 16.16 adjustment in the explicit row set. The zero row proves FreeType's no-strength outline embolden return through the existing C-oracle-backed Rust FFI, C ABI, and WASM ABI route, while glyph 3 keeps empty-outline metric side effects visible in the same public case. This adds no font bytes and no concrete cases; runtime comparison remains 6,741 / 6,741 with four explicit pending rows. Refreshed condition coverage is 16,192 / 18,080 lines, 23,243 / 25,910 regions, 3,896 / 4,638 branches, and 1,021 / 1,147 functions; missed source lines drop by one from the previous checkpoint |
| 2026-07-13 | Ftsynth orientation-none outline row | `FT_GlyphSlot_AdjustWeight` adds one manifest-backed explicit case over existing `fonts/glyf/hinter-control-matrix.ttf` gid 44, a non-empty zero-width outline. Pinned C, Rust FFI, C ABI, and WASM ABI all keep outline points unchanged after `FT_Outline_EmboldenXY` reaches `FT_ORIENTATION_NONE`, while the public glyph-slot helper still applies metric and advance side effects. This adds no font bytes and one real-parity concrete case; runtime comparison is 6,742 / 6,742 with four explicit pending rows. Refreshed condition coverage is 16,194 / 18,080 lines, 23,245 / 25,910 regions, 3,898 / 4,638 branches, and 1,021 / 1,147 functions; route audit reports 6,746 concrete cases, 3,386 real-parity routes, and zero implicit rows |
| 2026-07-13 | Ftsynth vertical-advance outline row | `FT_GlyphSlot_AdjustWeight` adds one manifest-backed explicit case over existing `input/fonts/DejaVuSans.ttf` gid 36 with `FT_LOAD_VERTICAL_LAYOUT`. Pinned C, Rust FFI, C ABI, and WASM ABI agree that the loaded public advance vector is vertical before ydelta is applied, covering the public slot `advance.y` mutation without a glyph loop or new font bytes. Runtime comparison is 6,743 / 6,743 with four explicit pending rows. Refreshed condition coverage is 16,195 / 18,080 lines, 23,247 / 25,910 regions, 3,899 / 4,638 branches, and 1,021 / 1,147 functions; route audit reports 6,747 concrete cases, 3,387 real-parity routes, and zero implicit rows |
| 2026-07-13 | Ftsynth PostScript-orientation outline row | `FT_GlyphSlot_AdjustWeight` adds one manifest-backed explicit case over existing `fonts/glyf/hinter-control-matrix.ttf` gid 1, whose positive area selects FreeType's PostScript-orientation embolden branch in `FT_Outline_EmboldenXY`. Pinned C, Rust FFI, C ABI, and WASM ABI agree on mutated outline points, cbox, metrics, and advances. This adds no font bytes and one real-parity concrete case; runtime comparison is 6,744 / 6,744 with four explicit pending rows. Refreshed condition coverage is 16,198 / 18,080 lines, 23,250 / 25,910 regions, 3,902 / 4,638 branches, and 1,021 / 1,147 functions |
| 2026-07-13 | Ftsynth mixed-winding degenerate segment row | `FT_GlyphSlot_AdjustWeight` adds one manifest-backed explicit case over existing `fonts/glyf/hinter-control-matrix.ttf` gid 43, whose mixed-winding outline includes a repeated-point degenerate contour. Pinned C, Rust FFI, C ABI, and WASM ABI agree while the embolden walker skips zero-length segment vectors through the public glyph-slot mutation route. This adds no font bytes and one real-parity concrete case; runtime comparison is 6,745 / 6,745 with four explicit pending rows. Refreshed condition coverage is 16,200 / 18,080 lines, 23,255 / 25,910 regions, 3,905 / 4,638 branches, and 1,021 / 1,147 functions; route audit reports 6,749 concrete cases, 3,389 real-parity routes, and zero implicit rows |
| 2026-07-13 | TrueType fpgm FDEF index overflow fixture | `font-fixture-hinter` now emits `fonts/glyf/hinter-fpgm-fdef-index-overflow.ttf`, a compact derived TrueType control whose font program attempts `FDEF 256` before scanning any function body. One explicit `FT_Load_Glyph.matrix_load` row proves pinned C, Rust FFI, C ABI, and WASM ABI all reject the out-of-range function definition through the public load route. Concrete cases are 6,750 with zero implicit rows; runtime comparison is 6,746 / 6,746 with four explicit pending rows. Refreshed condition coverage is 16,203 / 18,080 lines, 23,256 / 25,910 regions, 3,906 / 4,638 branches, and 1,021 / 1,147 functions; route audit reports 6,750 concrete cases, 3,390 real-parity routes, and zero implicit rows |
| 2026-07-13 | TrueType recursive IDEF call-depth fixture | `font-fixture-hinter` now emits `fonts/glyf/hinter-idef-recursive-depth.ttf`, a compact derived TrueType control whose ADJUST IDEF body calls the same IDEF opcode until the interpreter call-depth guard rejects the load. One explicit `FT_Load_Glyph.matrix_load` row proves pinned C, Rust FFI, C ABI, and WASM ABI all expose this as a public load error. Concrete cases are 6,751 with zero implicit rows; runtime comparison is 6,747 / 6,747 with four explicit pending rows. Refreshed condition coverage is 16,204 / 18,080 lines, 23,257 / 25,910 regions, 3,907 / 4,638 branches, and 1,021 / 1,147 functions; route audit reports 6,751 concrete cases, 3,391 real-parity routes, and zero implicit rows |
| 2026-07-13 | TrueType CALL/LOOPCALL invalid-reference fixture | `font-fixture-hinter` now emits one compact `fonts/glyf/hinter-fpgm-call-errors.ttf` with four selected glyph programs: `CALL -1`, `CALL 0`, `LOOPCALL count=1 function=0`, and recursive `CALL 1`. FreeType `ttinterp.c:3395-3549` rejects invalid/inactive FDEF references and checks call-stack overflow instead of silently continuing; Rust now shares this validation in `ExecContext::enter_function_call`. Four explicit `FT_Load_Glyph.matrix_load` rows prove pinned C, Rust FFI, C ABI, and WASM ABI all expose these as public load errors. Concrete cases are 6,755 with zero implicit rows; runtime comparison is 6,751 / 6,751 with four explicit pending rows. Refreshed condition coverage is 16,215 / 18,091 lines, 23,284 / 25,933 regions, 3,906 / 4,632 branches, and 1,024 / 1,150 functions; missed source lines remain 1,876 while this fixes a real C/Rust bytecode correctness gap |
| 2026-07-13 | Route TrueType round-state constants through shared conversion | `ExecContext` now sets `GS.round_state` through `RoundMode::from_u8` for RTG, RTHG, RTDG, ROFF, RUTG, RDTG, SROUND, and S45ROUND, matching FreeType's numeric `TT_Round_*` constants from `ttinterp.h` and `ttinterp.c:4268-4305`. Existing public `FT_Load_Glyph` rows over `stackStateMatrix` and `superRoundMatrix` already execute all valid round-state opcodes, so no fixture rows or font bytes were added. Concrete cases remain 6,755 with zero implicit rows; runtime comparison remains 6,751 / 6,751 with four explicit pending rows. Refreshed condition coverage is 16,226 / 18,091 lines, 23,295 / 25,933 regions, 3,906 / 4,632 branches, and 1,025 / 1,150 functions; only the defensive invalid-value fallback in `RoundMode::from_u8` remains uncovered because no TrueType opcode writes arbitrary round-state values |
| 2026-07-13 | Size lifecycle false-green audit | Superseded by the size lifecycle success route below. This audit identified that the non-null `ftsizes.new_size_sequence`, `ftsizes.done_size_sequence`, and `ftsizes.activate_size_sequence` rows could not count as parity while they used generic fallback or Rust-only proof paths. The retained blocker from that audit is `ftsizes.activate_select_size_sequence`, which still depends on real `FT_Select_Size` active-size mutation |
| 2026-07-13 | Ftsynth negative-bounds outline row | `FT_GlyphSlot_AdjustWeight` now has one explicit DejaVuSans gid 77 row with a nonzero weight adjustment. The glyph's loaded outline has negative x/y bounds, so the public slot-mutation route reaches FreeType's orientation scaling absolute-value branch while preserving exact Rust FFI, C ABI, and WASM ABI parity. This adds no font bytes and one real-parity concrete case. Concrete cases are 6,757 with zero implicit rows; runtime comparison is 6,753 / 6,753 with four explicit pending rows. Refreshed condition coverage is 16,227 / 18,091 lines, 23,297 / 25,933 regions, 3,909 / 4,632 branches, and 1,025 / 1,150 functions; missed source lines drop to 1,864 |
| 2026-07-13 | FT_Set_Named_Instance null-face route | One explicit `FT_Set_Named_Instance(NULL, 1)` row now calls the pinned C oracle plus Rust FFI, C ABI, and WASM ABI exports instead of the generic null shortcut. This covers the core wrapper's null-face branch without font bytes or synthetic glyph state. Concrete cases are 6,758 with zero implicit rows; runtime comparison is 6,754 / 6,754 with four explicit pending rows. Refreshed condition coverage is 16,228 / 18,091 lines, 23,298 / 25,933 regions, 3,910 / 4,632 branches, and 1,025 / 1,150 functions; missed source lines drop to 1,863 |
| 2026-07-13 | Char-iteration null-face rows | `FT_Get_First_Char(NULL, &agindex)` and `FT_Get_Next_Char(NULL, start, &agindex)` now call the pinned C oracle plus Rust FFI, C ABI, and WASM ABI routes directly instead of relying on declarative `face_variants`. This adds no font bytes and covers the public optional-face zero-sentinel branches for charmap iteration. Concrete cases are 6,760 with zero implicit rows; runtime comparison is 6,756 / 6,756 with four explicit pending rows. Refreshed condition coverage is 16,230 / 18,091 lines, 23,300 / 25,933 regions, 3,912 / 4,632 branches, and 1,025 / 1,150 functions; missed source lines drop to 1,861 |
| 2026-07-13 | Ftsynth composite-slot no-op rows | Existing `FT_GlyphSlot_AdjustWeight`, `FT_GlyphSlot_Slant`, and `FT_GlyphSlot_Oblique` no-op rows now use `glyf-component-matrix.ttf` gid 4 loaded with `FT_LOAD_NO_RECURSE`, proving the public non-outline composite-slot return paths without synthetic slot records or new font bytes. Rust also now exposes empty outline boxes for composite FFI slots, matching FreeType's `FT_LOAD_NO_RECURSE` shape while preserving header-bbox-derived metrics. Concrete cases remain 6,760 with zero implicit rows; route audit moves real parity to 3,403 and shape-incomplete fallback to 3. Runtime comparison is 6,756 / 6,756 with four explicit pending rows. Refreshed condition coverage is 16,233 / 18,092 lines, 23,303 / 25,934 regions, 3,916 / 4,634 branches, and 1,025 / 1,150 functions; missed source lines drop to 1,859 |
| 2026-07-13 | TrueType ENDF empty-call-stack probe | The source-backed `hinter-control-matrix.ttf` `branchEdgeMatrix` glyph now appends a no-output `ENDF` after its existing branch probes, reusing the existing `FT_Load_Glyph.matrix_load@hinter-branch-edge-matrix` public row. Pinned C treats `ENDF` with an empty call stack as a no-op, and Rust already matched that behavior; the prior gap was structural coverage for the interpreter's empty call-stack arm. Concrete cases remain 6,764 with zero implicit rows; runtime comparison is 6,760 / 6,760 with four explicit pending rows. Refreshed condition coverage is 16,261 / 18,095 lines, 23,334 / 25,934 regions, 3,923 / 4,634 branches, and 1,030 / 1,150 functions; `tt/hinter/exec.rs` moves from 1,350 / 1,379 lines, 369 / 416 branches, and 2,735 / 2,945 regions to 1,351 / 1,379 lines, 370 / 416 branches, and 2,736 / 2,945 regions |
| 2026-07-13 | Render mono zero-height profile sweep row | `build_render_fixtures.py` extends `fonts/glyf/render-coverage.ttf` with gid 4, a one-unit-high subpixel box. One explicit `FT_Render_Glyph.matrix_render` row selects it with `FT_LOAD_NO_HINTING` and `FT_RENDER_MODE_MONO`, creating `MonoOutlineProfileBuilder` entries whose heights are zero and covering `draw_mono_profile_sweep`'s empty-waiting return. Focused `render_glyph` coverage moved `render.rs` by one line, one region, and one branch; full `render.rs` coverage is now 1,720 / 2,272 lines, 2,425 / 3,216 regions, and 349 / 426 branches. Concrete cases are 6,766 with zero implicit rows; runtime comparison is 6,762 / 6,762 with four explicit pending rows. Refreshed condition coverage is 16,261 / 18,090 lines, 23,333 / 25,927 regions, 3,922 / 4,626 branches, and 1,030 / 1,150 functions |
| 2026-07-13 | Latin standard-character fallback fixtures | `build_autohint_script_fixtures.py` now emits `fonts/autohint/latin-missing-standard.ttf`, whose Latin-covered `A` glyph has no `o/O/0` standard-character fallback in the cmap, and `fonts/autohint/latin-empty-standard.ttf`, whose `o` standard glyph exists but has an empty outline. Two explicit `FT_Load_Glyph.matrix_load` rows select their real Latin glyphs with `FT_LOAD_FORCE_AUTOHINT`, covering the face-global Latin fallback-width path and `metrics_init_widths` empty-standard-glyph fallback without adding a glyph loop or reusing the previously rejected digit `.notdef` probe. Concrete cases are 6,770 with zero implicit rows; runtime comparison is 6,766 / 6,766 with four explicit pending rows. Refreshed condition coverage is 16,273 / 18,090 lines, 23,348 / 25,927 regions, 3,928 / 4,626 branches, and 1,030 / 1,150 functions |
| 2026-07-13 | Malformed Latin standard-character fixture | `build_autohint_script_fixtures.py` now emits `fonts/autohint/latin-malformed-standard.ttf`, whose selected U+0041/gid 2 glyph is valid while U+006F maps to a final glyph truncated to a two-byte `glyf` record. One explicit `FT_Load_Glyph.matrix_load` variant selects gid 2 with `FT_LOAD_FORCE_AUTOHINT`, so Latin metrics setup tries the malformed `o` standard glyph, ignores the failed load, and falls back exactly like pinned FreeType. Concrete cases are 6,771 with zero implicit rows; runtime comparison is 6,767 / 6,767 with four explicit pending rows. Refreshed condition coverage is 16,275 / 18,090 lines, 23,350 / 25,927 regions, 3,929 / 4,626 branches, and 1,030 / 1,150 functions. Route audit reports 3,411 real-parity rows |
| 2026-07-13 | TrueType prep empty-zone SHZ probe | The source-backed `hinter-control-matrix.ttf` prep program now appends a no-output `SZPS 1; SHZ[0]` sequence. Prep always runs against an empty glyph zone, so the existing `FT_Load_Glyph.matrix_load@hinter-branch-edge-matrix` row covers the interpreter's zero-contour target-zone `SHZ` branch without adding a concrete case or JSON input. The focused row passed exact Rust FFI, C ABI, and WASM ABI parity and hit `tt/hinter/exec.rs:1408`; the full gate remains at 6,771 concrete cases, zero implicit rows, 6,767 / 6,767 runtime comparisons, and four explicit pending rows. Refreshed condition coverage is 16,276 / 18,090 lines, 23,351 / 25,927 regions, 3,930 / 4,626 branches, and 1,030 / 1,150 functions |
| 2026-07-13 | TrueType prep empty-zone IUP probe | The source-backed `hinter-control-matrix.ttf` prep program now appends no-output `IUP[y]` and `IUP[x]` opcodes after the empty-zone SHZ probe. Prep executes before any glyph zone points are installed, so the existing `FT_Load_Glyph.matrix_load@hinter-branch-edge-matrix` public row covers FreeType's empty-zone IUP return without adding a concrete case or JSON input. Pinned C treats both IUP directions over an empty zone as no-ops, and Rust FFI, C ABI, and WASM ABI already matched that behavior. The full gate remains at 6,771 concrete cases, zero implicit rows, 6,767 / 6,767 runtime comparisons, and four explicit pending rows. Refreshed condition coverage is 16,277 / 18,090 lines, 23,352 / 25,927 regions, 3,931 / 4,626 branches, and 1,030 / 1,150 functions |
| 2026-07-13 | Ftsynth nearly-opposite vector row | The source-backed `hinter-control-matrix.ttf` now includes U+E035/gid 54, a compact sharp-turn outline whose adjacent normalized vectors are nearly opposite. One explicit `FT_GlyphSlot_AdjustWeight.outline_weight_nearly_opposite_vectors` row selects it with `FT_LOAD_NO_HINTING | FT_LOAD_NO_BITMAP`, proving pinned C, Rust FFI, C ABI, and WASM ABI all take the zero-shift branch in `FT_Outline_EmboldenXY` (`ftoutln.c:911-1047`) without implicit glyph expansion. Concrete cases are 6,772 with zero implicit rows; runtime comparison is 6,768 / 6,768 with four explicit pending rows. Refreshed condition coverage is 16,278 / 18,090 lines, 23,353 / 25,927 regions, 3,932 / 4,626 branches, and 1,030 / 1,150 functions. Route audit reports 3,412 real-parity rows |
| 2026-07-13 | Ftsynth zero-area orientation row | The source-backed `hinter-control-matrix.ttf` now includes U+E036/gid 55, a self-intersecting bowtie whose cbox is nondegenerate but whose signed-area accumulator is zero. One explicit `FT_GlyphSlot_AdjustWeight.outline_weight_zero_area_orientation_none` row selects it with `FT_LOAD_NO_HINTING | FT_LOAD_NO_BITMAP`, proving pinned C, Rust FFI, C ABI, and WASM ABI all reach `FT_Outline_Get_Orientation`'s `FT_ORIENTATION_NONE` area branch (`ftoutln.c:1055-1117`) while `FT_GlyphSlot_AdjustWeight` still applies metric and advance side effects through `ftsynth.c`. Concrete cases are 6,773 with zero implicit rows; runtime comparison is 6,769 / 6,769 with four explicit pending rows. Refreshed condition coverage is 16,279 / 18,090 lines, 23,354 / 25,927 regions, 3,933 / 4,626 branches, and 1,030 / 1,150 functions. Route audit reports 3,413 real-parity rows |
| 2026-07-13 | CJK wide standard snap fixture | `build_autohint_script_fixtures.py` now emits `fonts/autohint/cjk-wide-stem-snap.ttf`, where U+7530 supplies a 100 FU Hani standard stem and U+4ED6 supplies a 170 FU selected stem. One explicit `FT_LOAD_FORCE_AUTOHINT | FT_LOAD_TARGET_MONO` public row keeps the selected stem inside FreeType's `af_cjk_snap_width` reference search window but above `FT_PIX_ROUND(reference) + 48`, proving the upper-side no-snap branch in `afcjk.c:1440-1480` through exact Rust FFI, C ABI, and WASM ABI parity. Concrete cases are 6,774 with zero implicit rows; runtime comparison is 6,770 / 6,770 with four explicit pending rows. Refreshed condition coverage is 16,279 / 18,090 lines, 23,355 / 25,927 regions, 3,934 / 4,626 branches, and 1,030 / 1,150 functions; `src/autohint/cjk.rs` gains one covered region and one covered branch. Route audit reports 3,414 real-parity rows |
| 2026-07-13 | Render smart-dropout neighbor row | The source-backed `hinter-control-matrix.ttf` now includes U+E037/gid 56, a compact scan-type-4 glyph whose bytecode sets `SCANCTRL 255; SCANTYPE 4`. One explicit `FT_Render_Glyph.matrix_render` row selects it with `FT_LOAD_DEFAULT` and `FT_RENDER_MODE_MONO`, proving pinned C, Rust FFI, C ABI, and WASM ABI agree while the black rasterizer skips a smart-dropout primary because the alternate pixel is already set. This preserves FreeType's scan-mode handoff from `ttgload.c:838-840` and smart-dropout/alternate-pixel behavior from `ftraster.c:2176-2199,2377-2418`. Combined with the CJK probe above, concrete cases are 6,775 with zero implicit rows and runtime comparison is 6,771 / 6,771 with four explicit pending rows. Refreshed condition coverage is 16,280 / 18,090 lines, 23,356 / 25,927 regions, 3,935 / 4,626 branches, and 1,030 / 1,150 functions; `render.rs` reaches 1,721 / 2,272 lines, 2,426 / 3,216 regions, and 350 / 426 branches. Route audit reports 3,415 real-parity rows |
| 2026-07-13 | CJK degenerate glyph IUP zero-shift row | One explicit `FT_LOAD_FORCE_AUTOHINT` row selects U+4EEC from the existing source-backed `fonts/autohint/cjk-blue-edge-cases.ttf`, whose glyph has three one-point contours. Pinned C accepts the glyph and the public load path reaches the shared Latin segment start condition for single-point contours (`aflatin.c:1901-1907`) plus the zero-delta `af_iup_shift` return (`afhints.c:1592-1603`) without adding a glyph loop or mutating fixture outputs. Concrete cases are 6,776 with zero implicit rows; runtime comparison is 6,772 / 6,772 with four explicit pending rows. Refreshed condition coverage is 16,281 / 18,090 lines, 23,357 / 25,927 regions, 3,938 / 4,626 branches, and 1,030 / 1,150 functions; `src/autohint/latin.rs` gains one covered line, one region, and three branch outcomes. Route audit reports 3,416 real-parity rows |
| 2026-07-13 | No-recurse empty-glyph loader row | `FT_Load_Glyph.matrix_load` adds one explicit `dejavu-null-no-recurse` row over existing `input/fonts/DejaVuSans.ttf` gid 1 with `FT_LOAD_NO_RECURSE`, proving the public empty-glyph path where no composite recursion exists. FreeType `ttgload.c:1534-1560` zeroes metrics for empty glyf records, `ttgload.c:1800-1808` returns no-recurse subglyphs only inside the composite branch, and `ttgload.c:2556-2566` leaves non-composite empty slots as outline loads; Rust FFI, C ABI, and WASM ABI already matched this behavior. Concrete cases rise from 6,775 to 6,776 with zero implicit rows, runtime comparison rises from 6,771 / 6,771 to 6,772 / 6,772 with four explicit pending rows, and refreshed condition coverage moves from 16,280 / 18,090 lines, 23,356 / 25,927 regions, and 3,935 / 4,626 branches to 16,281 / 18,090 lines, 23,357 / 25,927 regions, and 3,936 / 4,626 branches. Route audit reports real-parity rising from 3,415 to 3,416 with pending-core 16 and shape-incomplete-fallback 0; `font.rs:1994` leaves the full missing-line report while the malformed short-glyf guard around `font.rs:2002` remains classified as residual |
| 2026-07-13 | Render folded dropout profile row | `build_render_fixtures.py` extends `fonts/glyf/render-coverage.ttf` with gid 5, a folded one-contour outline that yields same-contour one-row MONO profiles whose paired profiles are not adjacent in profile order. One explicit `FT_Render_Glyph.matrix_render` row selects it with `FT_LOAD_NO_HINTING` and `FT_RENDER_MODE_MONO`, proving pinned C, Rust FFI, C ABI, and WASM ABI agree while the mono dropout upper-stub test evaluates the non-adjacent false side. This preserves FreeType's `Draw_Sweep` dropout logic from `ftraster.c:2360-2375`; the Rust mirror is `MonoOutlineProfileBuilder::should_draw_profile_dropout` at `render.rs:1937-1951`. Concrete cases move from 6,775 to 6,776 with zero implicit rows, runtime comparison moves from 6,771 / 6,771 to 6,772 / 6,772 with four explicit pending rows, and route audit real-parity moves from 3,415 to 3,416. Refreshed condition coverage stays at 16,280 / 18,090 lines, 23,356 / 25,927 regions, and 1,030 / 1,150 functions while branches move from 3,935 / 4,626 to 3,936 / 4,626; `render.rs` moves from 350 / 426 to 351 / 426 branches with lines, regions, and functions unchanged |
| 2026-07-13 | TrueType ODD/EVEN zero-result branch probe | The source-backed `hinter-control-matrix.ttf` `stackStateMatrix` glyph now appends no-output zero-valued `ODD` and `EVEN` probes, popping both results inside the existing `FT_Load_Glyph.matrix_load@hinter-stack-state-matrix` row. Pinned C, Rust FFI, C ABI, and WASM ABI agree while `tt/hinter/exec.rs` covers the opposite branch outcomes for `ODD` and `EVEN` without new public rows or implicit expansion. Concrete cases remain 6,778 with zero implicit rows; runtime comparison remains 6,774 / 6,774 with four explicit pending rows. Refreshed condition coverage is 16,282 / 18,090 lines, 23,360 / 25,927 regions, 3,942 / 4,626 branches, and 1,030 / 1,150 functions; `tt/hinter/exec.rs` moves to 2,739 / 2,945 regions and 373 / 416 branches |
| 2026-07-13 | CJK far-below standard snap branch row | `build_autohint_script_fixtures.py` extends the existing `fonts/autohint/cjk-snap-below-standard.ttf` with U+4E1E, a 40 FU selected Hani stem measured against the existing U+7530 100 FU standard stem. One explicit `FT_LOAD_FORCE_AUTOHINT | FT_LOAD_TARGET_MONO` row selects it, proving the lower-side no-snap branch in FreeType `af_cjk_snap_width` (`afcjk.c:1440-1480`) where `width <= FT_PIX_ROUND(reference) - 48`; focused condition coverage flips `src/autohint/cjk.rs:808` from the baseline true-only side to the false side, and the full report now records that branch as 3 / 3. Concrete cases rise from 6,778 to 6,779 with zero implicit rows; runtime comparison rises from 6,774 / 6,774 to 6,775 / 6,775 with four explicit pending rows. Refreshed condition coverage moves from 16,282 / 18,090 lines, 23,358 / 25,927 regions, and 3,940 / 4,626 branches to 16,282 / 18,090 lines, 23,359 / 25,927 regions, and 3,941 / 4,626 branches; functions remain 1,030 / 1,150. Route audit reports real-parity rising from 3,418 to 3,419 with pending-core 16 and implicit cases remaining zero |
| 2026-07-13 | Size lifecycle success routes | `FT_New_Size.create_secondary_size_success`, `FT_Done_Size.remove_secondary_size_success`, and `FT_Activate_Size.switches_current_face_size` now execute real face-owned secondary size handles in safe Rust core plus direct thin C ABI and WASM ABI exports. The model mirrors FreeType `src/base/ftobjs.c`: `FT_New_Size` appends an inactive face-owned size, `FT_Activate_Size` assigns the active face size, and `FT_Done_Size` removes the size and falls back to the list head when the active size is destroyed. Focused exact parity passes for all `ftsizes` rows across Rust FFI, C ABI, and WASM ABI, including direct C/WASM null-validation routes. Full condition coverage is 16,489 / 18,305 lines, 23,637 / 26,216 regions, 3,968 / 4,668 branches, and 1,062 / 1,182 functions; `src/ffi/handles.rs` reaches 1,794 / 1,829 lines, 2,437 / 2,483 regions, 326 / 366 branches, and 197 / 198 functions. Route audit moves real-parity from 3,419 to 3,422 and pending-core from 16 to 13, with generic fallback at 926 and implicit cases still zero |
| 2026-07-13 | SBIT equal-offset Missing_Bitmap fixture | `font-fixture-sbit` now emits `fixtures/assets/fonts/sbit_missing_bitmap.ttf`, a compact source-backed TrueType face with one EBLC/EBDT strike at 20 ppem. Glyph 1 uses EBLC index format 1 with equal image offsets, matching FreeType's NoBitmap branch in `src/sfnt/ttsbit.c:1241-1441` and the top-level `Missing_Bitmap` return. Core now accepts `FT_LOAD_SBITS_ONLY`, parses just enough EBLC strike metadata to find the empty image record, and returns the exact `FT_Err_Missing_Bitmap` through Rust FFI, C ABI, and WASM ABI without adding bitmap decoding. Concrete cases remain 6,778 with zero implicit rows; runtime comparison remains 6,774 / 6,774 with four runtime-pending rows. Route audit moves real parity to 3,419 and pending-core to 15. Refreshed condition coverage is 16,433 / 18,294 lines, 23,628 / 26,306 regions, 3,963 / 4,666 branches, and 1,045 / 1,182 functions. The remaining SBIT blocker is the recursive composite missing-subglyph row: pinned C returns `Invalid_Composite` from `src/sfnt/ttsbit.c:1436-1441`, which still needs a compact composite SBIT fixture and recursive loader support |
| 2026-07-13 | TrueType JROF not-taken branch probe | The source-backed `hinter-control-matrix.ttf` `controlFlowMatrix` glyph now starts with a no-output `JROF` probe that pushes `offset=1` and `condition=1`, consumes both operands, and does not jump. The existing `FT_Load_Glyph.matrix_load@hinter-control-flow-matrix` public row reaches the bytecode; pinned C, Rust FFI, C ABI, and WASM ABI agree while `tt/hinter/exec.rs:1602` covers the not-taken side (`condition != 0`) without adding public rows or implicit expansion. Concrete cases remain 6,779 with zero implicit rows; runtime comparison remains 6,775 / 6,775 with four explicit pending rows. Refreshed condition coverage stays at 16,282 / 18,090 lines and 1,030 / 1,150 functions while regions move to 23,362 / 25,927 and branches move to 3,944 / 4,626; `tt/hinter/exec.rs` moves to 2,740 / 2,945 regions and 374 / 416 branches. Route audit remains real-parity 3,419 |
| 2026-07-13 | Format-14 UVS condition probes | `build_cmap_fixtures.py` extends the compact format-14 UVS fixture with a zero-glyph non-default mapping and a non-default-only selector, adds `cmap-platform0-variation.ttf` for the active platform-0 Unicode charmap branch, and adds one non-default-offset-out-of-range malformed format-14 subtable to the existing malformed cmap matrix. Six grouped public UVS variants cover below-default-range probes, glyph-id-zero non-default filtering, non-default-only char lists, and platform-0 default-lookup routing through exact pinned C, Rust FFI, C ABI, and WASM ABI parity. Concrete cases rise from 6,779 to 6,785 with zero implicit rows; runtime comparison rises from 6,775 / 6,775 to 6,781 / 6,781 with four explicit pending rows. Refreshed condition coverage moves from 16,282 / 18,090 lines, 23,361 / 25,927 regions, and 3,943 / 4,626 branches to 16,282 / 18,090 lines, 23,362 / 25,927 regions, and 3,953 / 4,626 branches; `src/tt/cmap.rs` reaches 164 / 164 branches. The remaining cmap lines are only 64-bit-unreachable `usize` overflow closures at lines 786-789, 866-867, and 914-915. Route audit reports real-parity rising from 3,419 to 3,425 with pending-core 16 and implicit cases remaining zero |
| 2026-07-13 | Latin bottom cedilla adjustment row | `build_autohint_script_fixtures.py` appends U+0122 to the source-backed `fonts/autohint/latin-small-ignore.ttf` as a compact two-contour bottom-accent glyph, preserving existing glyph indices. One explicit `FT_LOAD_FORCE_AUTOHINT` row selects it through `FT_Load_Char`, proving the FreeType bottom adjustment database path where `afadjust.c:167` maps U+0122 to `AF_ADJUST_DOWN` and `aflatin.c:3619,3821-3829` applies the lowest-contour bottom separation branch. Focused coverage covered previously missed `src/autohint/latin.rs` lines 2592, 2595, 2599, and 3951; full condition coverage moves from 16,282 / 18,090 lines, 23,361 / 25,927 regions, and 3,943 / 4,626 branches to 16,286 / 18,090 lines, 23,366 / 25,927 regions, and 3,948 / 4,626 branches, with functions unchanged at 1,030 / 1,150. Concrete cases rise from 6,779 to 6,780 with zero implicit rows; runtime comparison rises from 6,775 / 6,775 to 6,776 / 6,776 with four explicit pending rows. Route audit reports real-parity rising from 3,419 to 3,420 with pending-core 16 |
| 2026-07-13 | Combined worker integration checkpoint | Size lifecycle, SBIT Missing_Bitmap, live Type 1 non-SFNT opening, TrueType JROF, format-14 UVS, and Latin bottom-cedilla fixture work are merged on `compact-font-test-fixtures`. The source-backed JROF probe changed `hinter-control-matrix.ttf`, so derived SBIT and cmap fixtures were regenerated from that new base. Full unified condition coverage passes with 4,165 logical cases, 6,786 concrete explicit cases, zero implicit rows, and exact runtime parity for 6,783 / 6,783 runnable rows. The only runtime pending rows are the three named-instance FTMM obligations. Refreshed condition coverage is 16,864 / 18,743 lines, 24,241 / 26,968 regions, 4,022 / 4,730 branches, 1,091 / 1,235 functions, and 1,094 / 1,238 instantiations. Route audit reports 3,431 real-parity rows, 11 pending-core rows, and zero shape-incomplete fallback rows |
| 2026-07-13 | Recursive SBIT composite missing-subglyph rows | `font-fixture-sbit` now emits `sbit_composite_missing_subglyph.ttf` and `sbit_composite_missing_subglyph_format3.ttf`, two compact source-backed EBLC/EBDT strike controls. The grouped `fterrdef.FT_Err_Missing_Bitmap.composite_sbit_missing_subglyph` public case selects glyph 2 with `FT_LOAD_COLOR | FT_LOAD_SBITS_ONLY`; glyph 2 is a compound bitmap image that references glyph 1, whose image offsets are empty. Pinned FreeType returns `FT_Err_Invalid_Composite` from `src/sfnt/ttsbit.c:1436-1441` for the recursive miss. Rust core now parses EBDT compound image formats 8 and 9 enough to recurse while still leaving bitmap decoding unsupported for success paths. Exact Rust FFI, C ABI, and WASM ABI parity passes for both variants. Concrete cases rise to 6,787 with zero implicit rows; runtime comparison is 6,784 / 6,784 with three explicit FTMM pending rows. Refreshed condition coverage is 16,948 / 18,848 lines, 24,363 / 27,110 regions, 4,027 / 4,732 branches, 1,092 / 1,249 functions, and 1,095 / 1,252 instantiations. Route audit reports 3,433 real-parity rows and 10 pending-core rows |
| 2026-07-13 | SBIT exact public-error branch matrix | Supersedes the public-error interpretation in the two prior SBIT rows. Pinned FreeType `TT_Load_Glyph` first calls the SBIT decoder, but for scalable TrueType faces with `FT_LOAD_SBITS_ONLY` it replaces any failed SBIT load with `FT_Err_Invalid_Argument` (`src/truetype/ttgload.c:2401-2469`). Rust now mirrors that public conversion while preserving internal `MissingBitmap` and `InvalidComposite` scanner outcomes. `font-fixture-sbit` adds seven compact branch controls: no matching strike, range miss, missing range array, missing subtable header, unsupported index format, compound missing count, and compound truncated records. The grouped SBIT public rows now set `compare_error_output=true`, and the native C oracle cache proves all ten runtime SBIT variants return exact error code 6. Concrete cases rise to 6,794 with zero implicit rows; runtime comparison is 6,791 / 6,791 with the same three explicit FTMM pending rows. Refreshed condition coverage is 16,964 / 18,852 lines, 24,382 / 27,114 regions, 4,031 / 4,732 branches, 1,096 / 1,250 functions, and 1,099 / 1,253 instantiations. Route audit reports 3,440 real-parity rows and 10 pending-core rows. `src/tt/sbit.rs` missing lines drop from 29 to 20; the remaining SBIT misses are defensive overflow/unreachable arms plus the non-compound bitmap-success path, which must wait for real bitmap decoding parity |
| 2026-07-13 | Missing post-table public route correction | `build_post_fixtures.py` now emits `fonts/metadata/post-missing.ttf`, a compact generated SFNT with the optional `post` table removed. The `FT_Err_Post_Table_Missing` public input no longer uses stale generic-fallback assets; it calls the real `freetype.get_glyph_name` route for both absent-post and present-post controls. Pinned C returns top-level success with glyph-name status `FT_Err_Invalid_Argument` and clears only byte zero for the absent optional `post` service, so this row proves the error code 0x94 is not surfaced by `FT_Get_Glyph_Name`. Concrete cases stay at 6,794, coverage counts are unchanged, route-audit generic fallback drops from 926 to 924, and real-parity routes rise from 3,440 to 3,442 |
| 2026-07-13 | Size lifecycle explicit variants and probe-face correction | `FT_Done_Size.remove_secondary_size_success` now has active-secondary and inactive-secondary variants, covering non-active size removal without a new manifest case. `FT_New_Size.create_secondary_size_success` now has normal-face and negative-face-index-probe variants. Pinned C proves negative `FT_New_Memory_Face` probes start with `face->size == NULL` but can later allocate and activate a size through `FT_New_Size`; Rust now models probe faces with an empty size list and gates size-setting on active-size presence rather than the probe flag. Focused `ftsizes.new_size_sequence` and `ftsizes.done_size_sequence` each pass `2 / 2`. Full condition coverage passes with 6,796 concrete cases, 6,793 / 6,793 runtime rows, three FTMM pending rows, 17,015 / 18,901 lines, 24,446 / 27,180 regions, 4,035 / 4,730 branches, 1,105 / 1,259 functions, and 1,108 / 1,262 instantiations. Route audit reports 3,452 real-parity rows and zero implicit cases |
| 2026-07-13 | Explicit non-SFNT SFNT-load error row | `FT_Load_Sfnt_Table.missing_table_or_invalid_face_error` now has two explicit variants instead of one mixed declaration: an SFNT missing-table row and a Type 1 `input/fonts/type1/attach-afm-base.pfb` non-SFNT row. The new non-SFNT row calls pinned C, Rust FFI, C ABI, and WASM ABI through `sfnt.load_sfnt_table` and covers the Rust wrapper's `!font.is_sfnt()` `FT_Err_Invalid_Face_Handle` branch without changing code or comparison shape. Focused `sfnt.load_sfnt_table` parity passes 16 / 16. Full condition coverage passes with 6,797 concrete cases, 6,794 / 6,794 runtime rows, three FTMM pending rows, 17,016 / 18,901 lines, 24,447 / 27,180 regions, 4,036 / 4,730 branches, 1,105 / 1,259 functions, and 1,108 / 1,262 instantiations. Route audit reports 3,453 real-parity rows and zero implicit cases |
| 2026-07-14 | Type 1 constructor metadata and face-index probes | `scripts/build_type1_fixtures.py` now emits `fonts/type1/metadata-bold-invalid-bool.pfb`, a compact Type 1 face whose clear-text metadata has `Weight (Bold)` and an unrecognized `isFixedPitch` token. One `FT_New_Memory_Face.valid_font_bytes` row opens it successfully, and one `FT_New_Memory_Face.error_bad_size_or_unknown_format` row probes `face_index=1` on `simple-type1.pfb`. Focused constructor parity passes 29 / 29 and 16 / 16. Full condition coverage passes with 6,802 concrete cases, 6,799 / 6,799 runtime rows, three FTMM pending rows, 17,026 / 18,901 lines, 24,458 / 27,180 regions, 4,053 / 4,730 branches, 1,105 / 1,259 functions, and 1,108 / 1,262 instantiations. Route audit reports 3,458 real-parity rows and zero implicit cases |
| 2026-07-13 | SBIT incompatible flag public errors | `FT_Err_Missing_Bitmap.sbit_glyph_without_image` now includes two explicit variants for `FT_LOAD_SBITS_ONLY | FT_LOAD_NO_BITMAP` and `FT_LOAD_SBITS_ONLY | FT_LOAD_NO_SCALE` over the existing compact `sbit_missing_bitmap.ttf` fixture. Pinned C returns `FT_Err_Invalid_Argument` for both incompatible load-flag combinations before selecting an embedded bitmap strike; Rust FFI, C ABI, and WASM ABI prove the same public result without adding bitmap-success decoding. Focused `load_glyph` parity passes with 6,799 checked concrete cases and 348 load-glyph runtime rows. Full condition coverage passes with 6,799 concrete cases, 6,796 / 6,796 runtime rows, three FTMM pending rows, 17,019 / 18,901 lines, 24,448 / 27,180 regions, 4,038 / 4,730 branches, 1,105 / 1,259 functions, and 1,108 / 1,262 instantiations. Route audit reports 3,455 real-parity rows and zero implicit cases |
| 2026-07-13 | TrueType CINDEX out-of-range branch probe | The source-backed `hinter-control-matrix.ttf` `stackStateMatrix` glyph now appends a no-output indexed-stack fallback probe, then the existing `FT_Load_Glyph.matrix_load@hinter-stack-state-matrix` public row executes it. Pinned C, Rust FFI, C ABI, and WASM ABI agree while `tt/hinter/exec.rs` covers the CINDEX out-of-range fallback without adding a public row or increasing concrete cases. Full condition coverage remains at 6,799 concrete cases, 6,796 / 6,796 runtime rows, three FTMM pending rows, 17,019 / 18,901 lines, 24,448 / 27,180 regions, 1,105 / 1,259 functions, and 1,108 / 1,262 instantiations; branch coverage moves from 4,038 / 4,730 to 4,039 / 4,730, and `tt/hinter/exec.rs` moves from 374 / 416 to 375 / 416 branches. Route audit remains 3,455 real-parity rows and zero implicit cases |
| 2026-07-13 | TrueType indexed-stack and Latin serif-m probes | The source-backed `hinter-control-matrix.ttf` `stackStateMatrix` glyph now also appends no-output `MINDEX`, inverse `MIN`, and `SCANCTRL` threshold probes, reusing the existing `FT_Load_Glyph.matrix_load@hinter-stack-state-matrix` public row and moving `tt/hinter/exec.rs` to 378 / 416 branch outcomes without case growth. `script-coverage.ttf` also adds one serifed three-stem Latin `m` selected by one explicit `FT_LOAD_FORCE_AUTOHINT` row, covering the Latin 12-edge serif symmetry movement path. Full condition coverage now passes with 6,800 concrete cases, 6,797 / 6,797 runtime rows, three FTMM pending rows, 17,021 / 18,901 lines, 24,454 / 27,180 regions, 4,048 / 4,730 branches, 1,105 / 1,259 functions, and 1,108 / 1,262 instantiations. Route audit reports 3,456 real-parity rows and zero implicit cases |
| 2026-07-14 | TrueType twilight zone-pointer branch probes | The source-backed `hinter-control-matrix.ttf` now packs three no-output zone-pointer probes into existing public rows: `branchEdgeMatrix` runs twilight `zp0` MD/MDRP paths with invalid or twilight-only points, and `pointMoveMatrix` briefly switches `zp2` to the twilight zone before `SHC[0]`. Pinned C, Rust FFI, C ABI, and WASM ABI agree with unchanged glyph outputs while `tt/hinter/exec.rs` moves from 378 / 416 to 381 / 416 branch outcomes without adding fonts, JSON rows, implicit expansion, or line regressions. Full condition coverage stays at 6,800 concrete cases, 6,797 / 6,797 runtime rows, three FTMM pending rows, 17,021 / 18,901 lines, 24,454 / 27,180 regions, 1,105 / 1,259 functions, and 1,108 / 1,262 instantiations; branch coverage moves to 4,051 / 4,730. Route audit remains 3,456 real-parity rows and zero implicit cases |
| 2026-07-14 | TrueType DELTAP Y-touched compatibility probe | The source-backed `hinter-control-matrix.ttf` `deltaControlMatrix` glyph now adds a no-output Y-axis probe that touches point 1 with `MDAP[0]`, applies one matching `DELTAP1`, and restores the coordinate with `SCFS`. This covers FreeType's v40 compatibility branch where DELTAP movement is allowed for a Y-touched point, while the same existing `FT_Load_Glyph.matrix_load@hinter-delta-control-matrix` row keeps exact pinned C, Rust FFI, C ABI, and WASM ABI parity. Full condition coverage remains at 6,800 concrete cases, 6,797 / 6,797 runtime rows, three FTMM pending rows, 17,021 / 18,901 lines, 24,454 / 27,180 regions, 1,105 / 1,259 functions, and 1,108 / 1,262 instantiations; branch coverage moves to 4,052 / 4,730 and `tt/hinter/exec.rs` moves to 382 / 416 branch outcomes. Route audit remains 3,456 real-parity rows and zero implicit cases |
| 2026-07-14 | Constructor error asset aliases | Existing `FT_New_Memory_Face` constructor error rows already executed exact Rust FFI / C ABI / WASM ABI parity, but several used only `font_bytes` or `blob` asset keys and were classified as null fallbacks by route audit. The public input JSON now adds canonical `font` aliases beside those same byte assets for malformed SFNT/TTC/OTTO/cmap/name/header cases plus the unknown-format blob. Focused constructor and unknown-format parity stays green, concrete cases remain 6,802 with zero implicit rows, and route audit moves from 3,458 real-parity / 21 null-error-fallback rows to 3,472 real-parity / 7 null-error-fallback rows without changing line coverage |
| 2026-07-14 | Worker coverage rows integrated | Merged verified worker rows for `FT_Load_Glyph.error_out_of_range_null_face_or_invalid_flags@sbits-only-no-bitmap-conflict` and `FT_LOAD_FORCE_AUTOHINT@script-latin-serif-m-symmetry-12-edge`. The load-glyph row proves pinned C and Rust return `FT_Err_Invalid_Argument` for `FT_LOAD_SBITS_ONLY | FT_LOAD_NO_BITMAP` over DejaVuSans; the autohint row reuses the source-backed `script-coverage.ttf` serifed three-stem Latin `m`. The older CINDEX worker branch is recorded as merged but resolved to this branch's existing superset `hinter-control-matrix.ttf`, which already includes the CINDEX probe plus newer no-output indexed-stack probes. Focused parity passes for both public rows. Full condition coverage passes with 6,803 concrete cases, 6,800 / 6,800 runtime rows, three FTMM pending rows, 17,026 / 18,901 lines, 24,458 / 27,180 regions, 4,053 / 4,730 branches, 1,105 / 1,259 functions, and 1,108 / 1,262 instantiations. Route audit reports 3,473 real-parity rows and zero implicit cases |
| 2026-07-14 | Malformed SBIT table header controls | `font-fixture-sbit` now emits `sbit_invalid_eblc_version.ttf`, `sbit_empty_ebdt.ttf`, and `sbit_strike_count_overflow.ttf`, three compact malformed embedded-bitmap controls. The existing `FT_Err_Missing_Bitmap.sbit_glyph_without_image` public row selects each with `FT_LOAD_COLOR | FT_LOAD_SBITS_ONLY`; pinned C, Rust FFI, C ABI, and WASM ABI all return exact public `FT_Err_Invalid_Argument`. These rows cover `tt/sbit.rs` early-return paths for invalid EBLC version/empty EBDT and impossible declared strike count while keeping bitmap-success decoding pending. Full condition coverage passes with 6,806 concrete cases, 6,803 / 6,803 runtime rows, three FTMM pending rows, 17,028 / 18,901 lines, 24,461 / 27,180 regions, 4,057 / 4,730 branches, 1,105 / 1,259 functions, and 1,108 / 1,262 instantiations. Route audit reports 3,476 real-parity rows and zero implicit cases |
| 2026-07-14 | SBIT gray format-1 bitmap-success path | `font-fixture-sbit` now emits `sbit_gray_format1.ttf`, a compact source-backed TrueType face with one 20 ppem EBLC/EBDT strike. One explicit `FT_Load_Glyph.matrix_load@sbit-gray-format1-sbits-only` row selects glyph 1 with `FT_LOAD_SBITS_ONLY`, exercising index format 1, image format 1, 8-bit gray bitmap allocation/bytes, and FreeType's scalable-SBIT fallback from missing small-metrics `vertAdvance` to the glyph linear vertical advance (`truetype/ttgload.c:2401-2469`). The first focused run exposed Rust's zero `vertAdvance`; core now carries SBIT slot metrics in 26.6 units and fills missing scalable SBIT advances from `hmtx`/`vmtx` or synthesized vertical font metrics. Focused `FT_Load_Glyph.matrix_load` parity passes 118 / 118. Full condition coverage passes with 6,807 concrete cases, 6,804 / 6,804 runtime rows, three FTMM pending rows, 17,160 / 19,066 lines, 24,612 / 27,381 regions, 4,065 / 4,742 branches, 1,110 / 1,275 functions, and 1,113 / 1,278 instantiations. Route audit reports 3,477 real-parity rows and zero implicit cases |
| 2026-07-14 | Default SBIT load-before-outline order | One explicit `FT_Load_Glyph.matrix_load@sbit-gray-format1-default-render` row reuses `sbit_gray_format1.ttf` and loads glyph 1 with `FT_LOAD_RENDER` rather than `FT_LOAD_SBITS_ONLY`. Pinned FreeType first tries embedded bitmaps before outline loading when bitmap loading is allowed (`base/ftobjs.c:1028-1050`) and the TrueType driver repeats the SBIT attempt before falling through to outlines (`truetype/ttgload.c:2401-2474`). Rust now mirrors that order: successful SBIT loads return bitmap slots for normal load/render calls, while failed `FT_LOAD_SBITS_ONLY` attempts still map to public `FT_Err_Invalid_Argument`. Focused `FT_Load_Glyph.matrix_load` parity passes 119 / 119. Full condition coverage passes with 6,808 concrete cases, 6,805 / 6,805 runtime rows, three FTMM pending rows, 17,165 / 19,071 lines, 24,614 / 27,383 regions, 4,072 / 4,750 branches, 1,109 / 1,274 functions, and 1,112 / 1,277 instantiations. Route audit reports 3,478 real-parity rows and zero implicit cases |
| 2026-07-14 | Ftsynth bitmap-slot embolden parity | The existing `ftsynth.FT_GlyphSlot_AdjustWeight.bitmap_weight_owns_emboldens_and_updates_top` and `ftsynth.FT_GlyphSlot_Embolden.bitmap_embolden_mutates_bitmap_and_metrics` public rows now reuse `sbit_gray_format1.ttf` instead of the deprecated embedded-strike placeholder. The native oracle and unified Rust/C/WASM outputs now include bitmap descriptors, bitmap bytes, and bitmap-top values for ftsynth weight rows, proving the mutation rather than only slot metrics. Core `FT_GlyphSlot_AdjustWeight` now follows FreeType `base/ftsynth.c`: bitmap slots round synthetic strengths to full pixels, force a one-pixel horizontal embolden when the rounded x strength is zero, call the bitmap embolden path before metric side effects, and skip metric mutation when bitmap emboldening rejects negative pixel strength. Focused parity passes for both ftsynth bitmap rows. Full condition coverage passes with 6,808 concrete cases, 6,805 / 6,805 runtime rows, three FTMM pending rows, 17,253 / 19,176 lines, 24,783 / 27,574 regions, 4,102 / 4,796 branches, 1,113 / 1,278 functions, and 1,116 / 1,281 instantiations. Route audit reports 3,480 real-parity rows, 8 pending-core rows, and zero implicit cases |
| 2026-07-14 | Real `FT_Select_Size` route and active-size sequence | `FT_Select_Size` now uses the compact `sbit_gray_format1.ttf` strike instead of the deprecated embedded-strike placeholder. Rust core exposes fixed-size face flags from parsed SBIT strikes, selects strike ppem into the active size object, resets scaler/autohint/bytecode state, and maps pinned FreeType's null-face, no-fixed-size, negative-index, out-of-range, and success errors through Rust FFI, C ABI, and WASM ABI. The `ftsizes.activate_select_size_sequence` row now runs real pinned C/Rust/C-ABI/WASM-ABI parity and proves selection mutates the currently active secondary size before reactivating the initial size. Focused `FT_Select_Size` parity passes 4 / 4 and the ftsizes select sequence passes 1 / 1. Full condition coverage passes with 6,806 concrete cases, 6,803 / 6,803 runtime rows, three FTMM pending rows, 17,307 / 19,232 lines, 24,862 / 27,656 regions, 4,107 / 4,802 branches, 1,120 / 1,285 functions, and 1,123 / 1,288 instantiations. Route audit reports 3,485 real-parity rows, 7 pending-core rows, 6 explicit-unsupported rows, and zero implicit cases |
| 2026-07-14 | SBIT mono format-1 bitmap-success path | `font-fixture-sbit` now emits `sbit_mono_format1.ttf`, a compact source-backed TrueType face with one 20 ppem EBLC/EBDT strike. One explicit `FT_Load_Glyph.matrix_load@sbit-mono-format1-sbits-only` row selects glyph 1 with `FT_LOAD_SBITS_ONLY`, exercising index format 1, image format 1, bit-depth 1 MONO bitmap allocation, a two-byte pitch for a 9-pixel-wide image, and final-byte masking through exact pinned C, Rust FFI, C ABI, and WASM ABI parity. The first implementation check showed the public oracle expects `FT_PIXEL_MODE_MONO`, `num_grays == 2`, pitch 2, and bytes `a5805a00`; core SBIT decoding now maps bit-depth 1 to MONO while preserving the existing bit-depth 8 GRAY path. Focused mono-SBIT parity passes 1 / 1 and full `load_glyph` operation parity passes 355 / 355. Full condition coverage passes with 6,807 concrete cases, 6,804 / 6,804 runtime rows, three FTMM pending rows, 17,309 / 19,234 lines, 24,868 / 27,663 regions, 4,108 / 4,802 branches, 1,120 / 1,285 functions, and 1,123 / 1,288 instantiations. Route audit reports 3,486 real-parity rows and zero implicit cases |
| 2026-07-14 | Packed SBIT pixel-mode success matrix | `font-fixture-sbit` now emits `sbit_gray2_format1.ttf`, `sbit_gray4_format1.ttf`, `sbit_bgra_format1.ttf`, and `sbit_gray_format3.ttf`, four compact source-backed EBLC/EBDT strike controls. Explicit `FT_Load_Glyph.matrix_load` variants cover 2-bit GRAY2, 4-bit GRAY4, 32-bit BGRA, and index-format-3 gray success, while the existing `FT_PIXEL_MODE_GRAY2`, `FT_PIXEL_MODE_GRAY4`, and `FT_PIXEL_MODE_BGRA` manifest rows now run through real `load_glyph` parity instead of generic build-dependent fallbacks. Core maps SBIT bit depths 1/2/4/8/32 to FreeType pixel modes and packed-row pitches per `sfnt/ttsbit.c:544-589,700-743`. Focused `load_glyph` parity passes 362 / 362. Full condition coverage passes with 6,811 concrete cases, 6,808 / 6,808 runtime rows, three FTMM pending rows, 17,323 / 19,253 lines, 24,885 / 27,685 regions, 4,106 / 4,800 branches, 1,121 / 1,286 functions, and 1,124 / 1,289 instantiations. Route audit reports 3,493 real-parity rows, 913 generic-fallback rows, and zero implicit cases |
| 2026-07-14 | Ftsynth packed SBIT bitmap-slot parity | The existing `FT_GlyphSlot_AdjustWeight.bitmap_weight_owns_emboldens_and_updates_top` and `FT_GlyphSlot_Embolden.bitmap_embolden_mutates_bitmap_and_metrics` rows now use explicit variants over `sbit_gray_format1.ttf`, `sbit_mono_format1.ttf`, `sbit_gray2_format1.ttf`, `sbit_gray4_format1.ttf`, and `sbit_bgra_format1.ttf`. This adds no font bytes and proves the same compact SBIT pixel-mode set through public ftsynth slot mutation. Core bitmap emboldening now matches FreeType `base/ftbitmap.c`: MONO uses the packed-byte embolden loop, GRAY2/GRAY4 first convert to 8-bit gray with `num_grays` 4/16, and BGRA returns success without mutating bitmap bytes so ftsynth still applies metric/top side effects. Focused ftsynth bitmap parity passes 5 / 5 for each operation. Full condition coverage passes with 6,819 concrete cases, 6,816 / 6,816 runtime rows, three FTMM pending rows, 17,431 / 19,380 lines, 25,101 / 27,921 regions, 4,137 / 4,858 branches, 1,127 / 1,292 functions, and 1,130 / 1,295 instantiations. Route audit reports 3,501 real-parity rows, 913 generic-fallback rows, and zero implicit cases |
| 2026-07-14 | Post format-1 standard Mac-name control | `font-fixture-post` now emits `post-format-1-standard-count.ttf`, a compact format-1.0 `post` control with exactly 258 glyph slots. `FT_Get_Glyph_Name.post_format_1_default_names` keeps the non-258 fallback row and adds an exact-258 gid 36 row that returns Mac standard name `A`; `FT_Get_Name_Index.post_format_1_default_name_index` adds the reverse `A -> 36` lookup. Focused glyph-name and name-index parity each pass 2 / 2 through Rust FFI, C ABI, and WASM ABI. Full condition coverage passes with 6,821 concrete cases, 6,818 / 6,818 runtime rows, three FTMM pending rows, 17,431 / 19,380 lines, 25,106 / 27,921 regions, 4,138 / 4,858 branches, 1,127 / 1,292 functions, and 1,130 / 1,295 instantiations. `tt/post.rs` moves to 184 / 192 regions and 21 / 22 branches; the remaining missed lines are helper paths blocked by public glyph-name validation or face-flag gates. Route audit reports 3,503 real-parity rows, 913 generic-fallback rows, and zero implicit cases |
| 2026-07-14 | SBIT unsupported bit-depth and image-format controls | `font-fixture-sbit` now emits `sbit_unsupported_bit_depth_format1.ttf` and `sbit_unsupported_image_format.ttf`, two compact EBLC/EBDT controls. Explicit `FT_Load_Glyph.matrix_load` variants prove pinned C, Rust FFI, C ABI, and WASM ABI parity for unsupported bit depth 7 in image format 1 and unsupported image format 10 through the public `FT_LOAD_SBITS_ONLY` path. Focused `matrix_load` parity passes 1,702 / 1,702. Full condition coverage passes with 6,823 concrete cases, 6,820 / 6,820 runtime rows, three FTMM pending rows, 17,439 / 19,380 lines, 25,109 / 27,921 regions, 4,139 / 4,858 branches, 1,127 / 1,292 functions, and 1,130 / 1,295 instantiations. `tt/sbit.rs` moves to 319 / 398 lines and 29 / 34 branches; remaining misses are private status helpers, checked overflow guards, 64-bit conversion guards, the unreachable compound-format arm, and the compound-image success tail that still needs real compound SBIT success decoding. Route audit reports 3,505 real-parity rows, 913 generic-fallback rows, and zero implicit cases |
| 2026-07-14 | SBIT compound bitmap assembly | `font-fixture-sbit` now emits six compact compound controls: format-8 gray success, format-9 big-metrics success, MONO success, BGRA success, negative component offset, and component out-of-bounds. Core mirrors FreeType `sfnt/ttsbit.c:961-1012` by allocating the root bitmap from compound metrics, recursively loading components, ORing component bytes into the root canvas, and preserving root metrics. Focused `FT_Load_Glyph.matrix_load` parity passes 1,708 / 1,708. Full condition coverage passes with 6,829 concrete cases, 6,826 / 6,826 runtime rows, three FTMM pending rows, 17,544 / 19,534 lines, 25,277 / 28,141 regions, 4,154 / 4,880 branches, 1,133 / 1,310 functions, and 1,136 / 1,313 instantiations. `tt/sbit.rs` moves to 424 / 552 lines and 43 / 56 branches; absolute covered lines increased by 105 while the percentage shifted because real compound implementation code increased the denominator. Route audit reports 3,511 real-parity rows, 913 generic-fallback rows, and zero implicit cases |
| 2026-07-14 | SBIT vertical-layout bitmap slot | One explicit `FT_Load_Glyph.matrix_load@sbit-composite-success-format9-vertical-layout` row reuses `sbit_composite_success_format9.ttf` with `FT_LOAD_VERTICAL_LAYOUT | FT_LOAD_SBITS_ONLY`. Pinned C returns the compound SBIT bitmap using vertical big metrics, and Rust FFI, C ABI, and WASM ABI agree exactly. This adds no font bytes and closes the public `sbit_glyph_slot` vertical-layout branch in `api.rs`. Focused `matrix_load` parity passes 1,709 / 1,709. Full condition coverage passes with 6,830 concrete cases, 6,827 / 6,827 runtime rows, three FTMM pending rows, 17,543 / 19,535 lines, 25,278 / 28,142 regions, 4,155 / 4,880 branches, 1,133 / 1,310 functions, and 1,136 / 1,313 instantiations. Route audit reports 3,512 real-parity rows, 913 generic-fallback rows, and zero implicit cases |
| 2026-07-14 | Ftsynth MONO horizontal bitmap embolden rows | The existing `FT_GlyphSlot_AdjustWeight.bitmap_weight_owns_emboldens_and_updates_top@sbit-mono-format1` variant now carries two additional explicit adjustments: `xdelta_16_16=4096, ydelta_16_16=0` and `xdelta_16_16=24576, ydelta_16_16=0`. These keep the concrete case count flat while the row output compares additional public mutation states inside the existing variant. Pinned C, Rust FFI, C ABI, and WASM ABI agree exactly on the resulting MONO bitmap bytes, top, metrics, and advance. Focused ftsynth bitmap parity passes 5 / 5 variants. Full condition coverage passes with 6,830 concrete cases, 6,827 / 6,827 runtime rows, three FTMM pending rows, 17,544 / 19,535 lines, 25,279 / 28,142 regions, 4,156 / 4,880 branches, 1,133 / 1,310 functions, and 1,136 / 1,313 instantiations. Route audit remains 3,512 real-parity rows, 913 generic-fallback rows, and zero implicit cases |
| 2026-07-14 | Ftsynth rendered LCD bitmap embolden rows | The existing `FT_GlyphSlot_AdjustWeight.bitmap_weight_owns_emboldens_and_updates_top` case now adds three rendered-outline bitmap variants over `input/fonts/DejaVuSans.ttf`: normal gray, horizontal LCD, and vertical LCD. The LCD row exposed a real C/Rust mismatch where Rust treated LCD bitmap emboldening as unsupported and skipped slot metric/advance side effects; core now mirrors FreeType `base/ftbitmap.c:330-336` by treating LCD/LCD_V as 8-bit buffers and multiplying only the bitmap embolden footprint along the subpixel axis. Focused ftsynth bitmap parity passes 8 / 8 variants. Full condition coverage passes with 6,833 concrete cases, 6,830 / 6,830 runtime rows, three FTMM pending rows, 17,552 / 19,542 lines, 25,293 / 28,155 regions, 4,156 / 4,880 branches, 1,135 / 1,312 functions, and 1,138 / 1,315 instantiations. Route audit reports 3,515 real-parity rows, 913 generic-fallback rows, and zero implicit cases |
| 2026-07-14 | Ftsynth empty rendered bitmap no-op row | The existing `FT_GlyphSlot_AdjustWeight.bitmap_weight_owns_emboldens_and_updates_top` case adds one rendered DejaVuSans gid 3 empty-outline variant. Pinned C exposes a bitmap-format slot with no bitmap buffer, so `FT_Bitmap_Embolden` returns before ftsynth metric side effects; Rust FFI, C ABI, and WASM ABI now prove the same public no-op through `GlyphSlot::adjust_bitmap_weight`'s no-bitmap branch. Focused ftsynth bitmap parity passes 9 / 9 variants. Full condition coverage passes with 6,834 concrete cases, 6,831 / 6,831 runtime rows, three FTMM pending rows, 17,553 / 19,542 lines, 25,298 / 28,159 regions, 4,157 / 4,880 branches, 1,135 / 1,312 functions, and 1,138 / 1,315 instantiations. Route audit reports 3,516 real-parity rows, 913 generic-fallback rows, and zero implicit cases |
| 2026-07-14 | Type 1 fixed-pitch face-flag row | `font-fixture-type1` now emits `fixed-pitch-type1.pfb`, a compact Type 1 face with `/isFixedPitch true`. One explicit `FT_FACE_FLAG_FIXED_WIDTH.face_property_fixed_width_font@type1-fixed-pitch` variant proves pinned C, Rust FFI, C ABI, and WASM ABI all set `FT_FACE_FLAG_FIXED_WIDTH` for the Type 1 fixed-pitch face, covering `font.rs`'s Type 1 fixed-width branch without harness changes or implicit expansion. Focused face-flag parity passes 3 / 3 variants. Full condition coverage passes with 6,835 concrete cases, 6,832 / 6,832 runtime rows, three FTMM pending rows, 17,554 / 19,542 lines, 25,300 / 28,159 regions, 4,158 / 4,880 branches, 1,135 / 1,312 functions, and 1,138 / 1,315 instantiations. Route audit reports 3,517 real-parity rows, 913 generic-fallback rows, and zero implicit cases |
| 2026-07-14 | SBIT vmtx vertical-advance fallback row | `font-fixture-sbit` now emits `sbit_gray_format1_vmtx.ttf`, a compact gray format-1 EBLC/EBDT fixture with added `vhea/vmtx` vertical metrics. One explicit `FT_Load_Glyph.matrix_load@sbit-gray-format1-vmtx-sbits-only` variant selects glyph 1 with `FT_LOAD_SBITS_ONLY`, proving pinned C, Rust FFI, C ABI, and WASM ABI all fill the missing scalable SBIT vertical advance from `vmtx` instead of the synthesized font-wide fallback. Focused `matrix_load` parity passes 1,710 / 1,710. Full condition coverage passes with 6,836 concrete cases, 6,833 / 6,833 runtime rows, three FTMM pending rows, 17,555 / 19,542 lines, 25,304 / 28,159 regions, 4,159 / 4,880 branches, 1,135 / 1,312 functions, and 1,138 / 1,315 instantiations. Route audit reports 3,518 real-parity rows, 913 generic-fallback rows, and zero implicit cases |
| 2026-07-14 | SBIT packed compound x-offset blit | `font-fixture-sbit` now emits `sbit_composite_mono_shifted_success_format8.ttf`, a compact MONO compound control whose glyph 2 references glyph 1 at `dx=1`. Pinned C returns a successful shifted packed bitmap, while the previous Rust path returned `FT_Err_Invalid_Argument` from the packed nonzero-offset guard. Core now mirrors FreeType `sfnt/ttsbit.c:730-782` for byte-aligned packed compound blits by ORing shifted component bytes into the root bitmap. Focused `FT_Load_Glyph.matrix_load@sbit-composite-mono-shifted-success-format8` parity passes 1 / 1 and full `load_glyph` parity passes 373 / 373. Full condition coverage passes with 6,837 concrete cases, 6,834 / 6,834 runtime rows, three FTMM pending rows, 17,637 / 19,653 lines, 25,416 / 28,307 regions, 4,171 / 4,896 branches, 1,139 / 1,323 functions, and 1,142 / 1,326 instantiations. `tt/sbit.rs` moves from 422 / 553 lines and 43 / 56 branches to 504 / 664 lines and 55 / 72 branches. Route audit reports 3,519 real-parity rows, 913 generic-fallback rows, and zero implicit cases |
| 2026-07-14 | OpenType validate missing-service route | `FT_OpenType_Validate.service_missing_error` now uses existing `fonts/type1/simple-type1.pfb` instead of the nonexistent non-OpenType placeholder and calls pinned C FreeType for the non-null face path. Pinned `ftotval.c` returns `FT_Err_Unimplemented_Feature` when a valid Type 1 face has no `OPENTYPE_VALIDATE` service; the preserved Rust wrapper already returns the same public error. Focused `make -C pillow-rs-freetype test-case CASE=ftotval.FT_OpenType_Validate` passes 6 / 6. Full condition coverage stays at 6,844 concrete cases, 6,841 / 6,841 runtime rows, three FTMM pending rows, 17,771 / 19,781 lines, 25,586 / 28,478 regions, 4,179 / 4,904 branches, 1,145 / 1,329 functions, and 1,148 / 1,332 instantiations. Route audit moves this one row from generic fallback to real parity: real-parity 3,530 -> 3,531 and generic-fallback 913 -> 912, with zero implicit cases |
| 2026-07-14 | SBIT horizontal advance fallback | The existing `sbit_gray_format1_vmtx.ttf` fixture now sets image-format-1 small-metrics `horiAdvance` to zero while keeping the same explicit `FT_Load_Glyph.matrix_load@sbit-gray-format1-vmtx-sbits-only` row. Pinned C fills the missing scalable SBIT horizontal advance from the glyph's TrueType `hmtx` advance, and Rust FFI, C ABI, and WASM ABI agree exactly. Focused parity passes 1 / 1. Full condition coverage passes with 6,844 concrete cases, 6,841 / 6,841 runtime rows, three FTMM pending rows, 17,772 / 19,781 lines, 25,588 / 28,478 regions, 4,180 / 4,904 branches, 1,145 / 1,329 functions, and 1,148 / 1,332 instantiations. `font.rs:1556` is no longer listed in the missing-lines report, and implicit cases remain zero |
| 2026-07-14 | SBIT zero-width packed component | `font-fixture-sbit` now emits `sbit_composite_mono_zero_width_component_format8.ttf`, a compact compound MONO control whose glyph 2 references glyph 1 with small-metrics width zero. Pinned C treats the component blit as a successful no-op and returns the blank root bitmap; Rust FFI, C ABI, and WASM ABI agree exactly. Focused parity passes 1 / 1. Full condition coverage passes with 6,845 concrete cases, 6,842 / 6,842 runtime rows, three FTMM pending rows, 17,773 / 19,781 lines, 25,589 / 28,478 regions, 4,181 / 4,904 branches, 1,145 / 1,329 functions, and 1,148 / 1,332 instantiations. `tt/sbit.rs:623` is no longer listed in the missing-lines report; route audit reports 3,532 real-parity rows and zero implicit cases |
| 2026-07-14 | Latin tilde separation branch probes | `script-coverage.ttf` now adds seven compact Latin accent glyphs selected by explicit `FT_LOAD_FORCE_AUTOHINT` rows: U+00F1 top tilde, U+1E4D second-top tilde, U+00E3/U+00D1 top no-measure/flat, and U+1E1B/U+1E1A/U+1E75 bottom tilde/no-measure/flat. Pinned C, Rust FFI, C ABI, and WASM ABI agree exactly. Focused `FT_LOAD_FORCE_AUTOHINT` parity passes 192 / 192. Full condition coverage passes with 6,852 concrete cases, 6,849 / 6,849 runtime rows, three FTMM pending rows, 17,784 / 19,781 lines, 25,607 / 28,478 regions, 4,203 / 4,904 branches, 1,145 / 1,329 functions, and 1,148 / 1,332 instantiations. `autohint/latin.rs` moves from 2,538 / 2,828 lines and 1,006 / 1,282 branches to 2,549 / 2,828 lines and 1,028 / 1,282 branches, without implicit cases |
| 2026-07-14 | Native vertical-layout metrics parity | `FT_LOAD_VERTICAL_LAYOUT.vertical_layout_metrics` no longer uses the generic `load_glyph_pair` placeholder. It now has explicit `load_glyph` variants over `input/fonts/vertical/cjk-vertical-metrics.ttf` for horizontal-control glyph 36, vertical-layout glyph 36, and empty vmtx glyph 1. The real row exposed a Rust/C mismatch: Rust scaled `vmtx.tsb` directly, while FreeType `ttgload.c:1337-1347,1991-2079` derives vertical metrics from pp3/pp4 and the final hinted cbox. Core now carries native pp3/pp4 through the TrueType hinter and computes `vertBearingY` / `vertAdvance` from those phantoms. Focused `make -C pillow-rs-freetype test-case CASE=freetype.FT_LOAD_VERTICAL_LAYOUT` passes 4 / 4 exact Rust FFI, C ABI, and WASM ABI comparisons. Full condition coverage passes with 6,854 concrete cases, 6,851 / 6,851 runtime rows, three FTMM pending rows, 17,831 / 19,829 lines, 25,701 / 28,573 regions, 4,212 / 4,914 branches, 1,147 / 1,331 functions, and 1,150 / 1,334 instantiations. Route audit reports 3,542 real-parity rows, 911 generic-fallback rows, and zero implicit cases |
| 2026-07-14 | FT_LOAD_SBITS_ONLY placeholder retirement | `FT_LOAD_SBITS_ONLY.embedded_bitmap_only_behavior` no longer uses the missing `fonts/bitmap/embedded-strike.ttf` and generic `freetype.load_glyph_pair` route. It is now an explicit `load_glyph` group over maintained fixtures: `sbit_gray_format1.ttf` proves a matching embedded bitmap succeeds, `DejaVuSans.ttf` proves an outline-only face returns the public error under `FT_LOAD_SBITS_ONLY`, and `sbit_no_matching_strike.ttf` proves a valid SBIT face with the wrong strike also errors. Focused `make -C pillow-rs-freetype test-case CASE=freetype.FT_LOAD_SBITS_ONLY` passes 4 / 4 exact Rust FFI, C ABI, and WASM ABI comparisons. Full condition coverage passes with 6,856 concrete cases, 6,853 / 6,853 runtime rows, three FTMM pending rows, 17,831 / 19,829 lines, 25,701 / 28,573 regions, 4,212 / 4,914 branches, 1,147 / 1,331 functions, and 1,150 / 1,334 instantiations. Route audit moves this public flag row from generic fallback to real parity variants: real-parity 3,542 -> 3,545 and generic-fallback 911 -> 910, with zero implicit cases |
| 2026-07-14 | Pixel-mode survey placeholder retirement | `FT_PIXEL_MODE_MONO.embedded_bitmap_mono_preserves_mode`, `FT_Pixel_Mode.bitmap_pixel_mode_matches_render_output`, and `FT_PIXEL_MODE_MAX.not_emitted_as_runtime_bitmap_mode` no longer use the generic `freetype.load_embedded_bitmap`, `freetype.render_glyph_bitmap`, or `freetype.bitmap_pixel_mode_survey` routes. They now use explicit `load_glyph` variants over DejaVuSans rendered outline modes and the maintained compact SBIT MONO, GRAY2, GRAY4, and BGRA fixtures, proving exact emitted `FT_Bitmap.pixel_mode` values rather than a modeled survey. Focused parity passes `5 / 5` for `FT_PIXEL_MODE_MONO`, `9 / 9` for `FT_Pixel_Mode`, and `9 / 9` for `FT_PIXEL_MODE_MAX`. Full condition coverage remains 6,856 concrete cases, 6,853 / 6,853 runtime rows, three FTMM pending rows, 17,831 / 19,829 lines, 25,701 / 28,573 regions, 4,212 / 4,914 branches, 1,147 / 1,331 functions, and 1,150 / 1,334 instantiations. Route audit moves 17 rows from generic fallback to real parity without concrete-case growth: real-parity 3,545 -> 3,562 and generic-fallback 910 -> 893, with zero implicit cases |
| 2026-07-14 | Bitmap glyph-format placeholder split | `FT_GLYPH_FORMAT_BITMAP.produced_by_rendered_or_embedded_bitmap` no longer uses the generic `freetype.load_render_glyph` row with a missing `fonts/bitmap/embedded-strike.ttf` asset. It now has two explicit `load_glyph` variants: a rendered DejaVuSans outline and the compact `sbit_gray_format1.ttf` embedded bitmap. Focused `make -C pillow-rs-freetype test-case CASE=ftimage.FT_GLYPH_FORMAT_BITMAP` passes 3 / 3 exact Rust FFI, C ABI, and WASM ABI comparisons. Full condition coverage passes with 6,857 concrete cases, 6,854 / 6,854 runtime rows, three FTMM pending rows, 17,831 / 19,829 lines, 25,701 / 28,573 regions, 4,212 / 4,914 branches, 1,147 / 1,331 functions, and 1,150 / 1,334 instantiations. Route audit moves this public row from one generic fallback to two real parity variants: real-parity 3,562 -> 3,564 and generic-fallback 893 -> 892, with zero implicit cases |
| 2026-07-14 | SFNT language-tag route classification | `FT_Get_Sfnt_LangTag` rows already execute pinned C `FT_Get_Sfnt_LangTag`, Rust FFI, C ABI, and WASM ABI for format-1 success, format-0 invalid-table behavior, invalid pointer/argument variants, and the `FT_SfntLangTag` record shape. The route audit had not listed `ftsnames.get_sfnt_lang_tag` as a real parity operation, so those four exact comparisons were still counted as generic fallback. Focused `make -C pillow-rs-freetype test-case CASE=ftsnames.FT_Get_Sfnt_LangTag` and `CASE=ftsnames.FT_SfntLangTag` pass 4 / 4 and 2 / 2 respectively. Full condition coverage remains 6,857 concrete cases, 6,854 / 6,854 runtime rows, three FTMM pending rows, 17,831 / 19,829 lines, 25,701 / 28,573 regions, 4,212 / 4,914 branches, 1,147 / 1,331 functions, and 1,150 / 1,334 instantiations. Route audit moves four rows from generic fallback to real parity: real-parity 3,564 -> 3,568 and generic-fallback 892 -> 888, with zero implicit cases |
| 2026-07-14 | Error-string route execution | `FT_Error_String` no longer uses the generic `--error` placeholder. The existing four manifest rows now call pinned C `FT_Error_String`, Rust FFI, C ABI, and WASM ABI with valid, negative, too-large, and module-expression error codes. Pinned FreeType is built with `FT_ENABLE_ERROR_STRINGS=OFF`, so C returns `NULL` after the range check; Rust mirrors that disabled-string behavior instead of fabricating strings. Focused `make -C pillow-rs-freetype test-case CASE=fterrors.FT_Error_String` passes 4 / 4 exact Rust FFI, C ABI, and WASM ABI comparisons. Full condition coverage passes with 6,857 concrete cases, 6,854 / 6,854 runtime rows, three FTMM pending rows, 17,838 / 19,838 lines, 25,709 / 28,583 regions, 4,215 / 4,917 branches, 1,148 / 1,332 functions, and 1,151 / 1,335 instantiations. Route audit moves four rows from generic fallback to real parity: real-parity 3,568 -> 3,572 and generic-fallback 888 -> 884, with zero implicit cases |
| 2026-07-14 | OpenType validation ABI route execution | `FT_OpenType_Validate` and `FT_OpenType_Free` no longer use the unified runner's Rust-FFI fallback for C ABI or WASM ABI legs. Thin C and WASM exports now delegate to the existing Rust FFI facade, preserving wrapper boundaries while validating raw output-pointer null handling, null-face behavior, missing-service behavior, and no-op free calls through the public ABI paths. Focused `make -C pillow-rs-freetype test-case CASE=ftotval.FT_OpenType_Validate` passes 6 / 6 and `CASE=ftotval.FT_OpenType_Free` passes 4 / 4 exact comparisons. Full condition coverage remains 6,857 concrete cases, 6,854 / 6,854 runtime rows, three FTMM pending rows, 17,838 / 19,838 lines, 25,709 / 28,583 regions, 4,215 / 4,917 branches, 1,148 / 1,332 functions, and 1,151 / 1,335 instantiations because the report excludes ABI wrapper crates. Route audit categories remain `real-parity 3,572`, `real-null-validation 8`, `generic-fallback 884`, and zero implicit cases; the audit reason now matches real C-oracle/Rust-FFI/C-ABI/WASM-ABI execution. Selected-table and malformed-table OpenType validation rows remain generic until the real OT validator exists |
| 2026-07-14 | SBIT malformed small-metrics width | `font-fixture-sbit` now emits `sbit_missing_small_metrics_width.ttf`, a compact EBLC/EBDT control whose image offsets select exactly one byte. The new `FT_Err_Missing_Bitmap.sbit_glyph_without_image@missing-small-metrics-width` row proves pinned C, Rust FFI, C ABI, and WASM ABI all return exact public `FT_Err_Invalid_Argument` through `FT_LOAD_COLOR | FT_LOAD_SBITS_ONLY`. Focused Missing_Bitmap parity passes 17 / 17. Full condition coverage passes with 6,858 concrete cases, 6,855 / 6,855 runtime rows, three FTMM pending rows, 17,839 / 19,838 lines, 25,712 / 28,583 regions, 4,215 / 4,917 branches, 1,149 / 1,332 functions, and 1,152 / 1,335 instantiations. Route audit classifies the row as real parity, moving real-parity 3,572 -> 3,573 with zero implicit cases. `tt/sbit.rs:335` is no longer listed in the missing-line report; `tt/sbit.rs` moves to 511 / 664 lines and 33 / 90 functions |
| 2026-07-14 | CJK malformed blue-string load skip | `font-fixture-autohint-scripts` now emits `cjk-malformed-blue.ttf`, a compact Hani control with a valid U+7530 selected glyph and a deliberately truncated U+4E2A bottom-fill blue glyph. One explicit `FT_LOAD_FORCE_AUTOHINT.load_char_force_autohint_behavior@cjk-malformed-blue-load-error-20` row proves pinned C, Rust FFI, C ABI, and WASM ABI all ignore the malformed blue-string load during CJK metrics setup and still load the public glyph exactly. Focused parity passes 1 / 1. Full condition coverage passes with 6,859 concrete cases, 6,856 / 6,856 runtime rows, three FTMM pending rows, 17,840 / 19,838 lines, 25,713 / 28,583 regions, 4,216 / 4,917 branches, 1,149 / 1,332 functions, and 1,152 / 1,335 instantiations. Route audit classifies the row as real parity, moving real-parity 3,573 -> 3,574 with zero implicit cases. `autohint/cjk.rs:208` is no longer listed in the missing-line report; `autohint/cjk.rs` moves to 894 / 941 lines and 382 / 426 branches |
| 2026-07-14 | Autohint helper/default route cleanup | Existing public `FT_LOAD_FORCE_AUTOHINT` rows now exercise `Direction::{is_horizontal,is_vertical,as_i8}`, `GlyphHints::num_contours`, `AfLatinAxisMetrics::default`, `AxisHints::default`, and `AFEdge::default` through the normal Latin/CJK autohint construction and segment-linking paths instead of leaving duplicate helper logic uncovered. No fixture, font, input JSON, manifest row, or route count changed. Focused `make -C pillow-rs-freetype test-case CASE=freetype.FT_LOAD_FORCE_AUTOHINT` passes 193 / 193 exact Rust FFI, C ABI, and WASM ABI comparisons. Full condition coverage passes with 6,861 concrete cases, 6,858 / 6,858 runtime rows, three FTMM pending rows, 17,945 / 19,944 lines, 25,808 / 28,675 regions, 4,221 / 4,919 branches, 1,160 / 1,339 functions, and 1,163 / 1,342 instantiations. Measured against the pre-cleanup current-branch baseline, covered lines move 17,919 -> 17,945 and `autohint/types.rs` reaches 103 / 103 lines and 14 / 14 functions. Route audit remains zero implicit cases with 3,578 real-parity rows |
| 2026-07-14 | Apple variation-selector route cleanup | `TT_APPLE_ID_VARIANT_SELECTOR.variation_selector_cmap_runtime` no longer points at the missing `input/fonts/name-cmap/apple-variation-selector.ttf` future asset or generic `variation.get_char_variant_index` operation. It now uses the maintained compact `fonts/cmap/cmap-format-language-matrix.ttf` fixture and calls real `freetype.face_get_char_variant_index` for U+0042 plus U+FE0F. Full condition coverage still passes with 6,861 concrete cases, 6,858 / 6,858 runtime rows, three FTMM pending rows, 17,945 / 19,944 lines, 25,808 / 28,675 regions, 4,221 / 4,919 branches, and 1,160 / 1,339 functions. Route audit moves one false-green placeholder to real parity without changing case count: real-parity 3,578 -> 3,579 and generic-fallback 882 -> 881 |
| 2026-07-14 | FT_Select_Size null-face variant split | `FT_Select_Size.error_no_fixed_sizes_or_null_face` no longer hides the null-face and no-fixed-size probes inside `params.variants`. It now uses the maintained `inputs.variants` model so `null-face` and `no-fixed-sizes` are separate concrete rows, both routed through pinned C, Rust FFI, C ABI, and WASM ABI. Focused `make -C pillow-rs-freetype test-case CASE=freetype.FT_Select_Size` passes 6 / 6. Full condition coverage passes with 6,862 concrete cases, 6,859 / 6,859 runtime rows, three FTMM pending rows, 17,946 / 19,944 lines, 25,809 / 28,675 regions, 4,222 / 4,919 branches, and 1,160 / 1,339 functions. `src/ffi/handles.rs:858` is no longer listed in the missed-line report; route audit rises to 3,580 real-parity rows with zero implicit cases |

## Residual Coverage Classification - 2026-07-14

Fresh `test-unified-condition-coverage` reports 1,998 uncovered source lines
after the Apple variation-selector route cleanup and `FT_Select_Size`
null-face variant split. The current split is:

| Measure | Count |
|---|---:|
| Logical public API cases | 4,166 |
| Concrete explicit cases | 6,862 |
| Runnable parity comparisons | 6,859 / 6,859 |
| Pending cases | 3 |
| Covered Rust lines | 17,946 / 19,944 (89.9819%) |
| Rust region coverage | 25,809 / 28,675 (90.0052%) |
| Rust branch/condition coverage | 4,222 / 4,919 (85.8305%) |
| Rust function coverage | 1,160 / 1,339 (86.6318%) |
| Route audit split | real-parity 3,580; generic-fallback 881; generic-error-fallback 139; null-error-fallback 7; raw-slot-null-validation 4; pending-core 5; explicit-unsupported 6; compile-contract 2,229; real-null-validation 8; void-fallback 2; wrapper-null-validation 1 |

### Coverage Goal Surface Split - 2026-07-14

The 100% coverage target must be interpreted against public FreeType parity,
not every historical Rust convenience method that happens to live in
`fontdone`. Current audit boundaries:

| Surface | Current status | Coverage decision |
|---|---|---|
| Public C FreeType subjects in `tests/manifest.yaml` | 1,543 public API input files, 4,166 logical cases, 6,862 concrete explicit rows, and zero implicit expansion | This is the primary fixture-driven coverage surface. Add compact fonts and explicit input rows here only when C FreeType, Rust FFI, C ABI, and WASM ABI can be compared exactly |
| Runtime real parity rows | 3,581 real-parity rows plus 8 real null-validation rows | These rows are valid coverage evidence and should keep growing by retiring false-green placeholders or implementing real missing behavior |
| Green placeholder rows | 1,039 rows: 880 generic fallback, 139 generic-error fallback, 7 null-error fallback, 6 explicit unsupported, 4 raw-slot null-validation, 2 void fallback, and 1 wrapper null-validation | Retire only when the replacement is a real public route or an explicitly documented unsupported/pending state. Do not add rows that merely increase this count |
| Compile/static ABI contract rows | 2,229 rows | Keep separate from runtime glyph/font parity. They validate constants, layout, macros, imports, and ABI shapes, but they are not evidence for render/parser/hinter behavior |
| Former extra `fontdone::ffi::handles` helpers not present as public C FreeType functions | Removed from the Rust facade: `FT_Face_Info`, `FT_Size_Metrics(face)`, `FT_Face_Charmap_Count`, `FT_Face_Charmap`, `FT_Face_Charmap_Info`, `FT_Face_Active_Charmap_Index`, `FT_Charmap_Info`, `FT_Charmap_Format`, `FT_Charmap_Language_ID`, `FT_Get_Charmap_Index_For_Face`, `FT_Get_Sfnt_OS2`, and `FT_Sfnt_Table_Count` | Keep future Rust, C ABI, and WASM harness routes on public FreeType-shaped APIs: `FT_Face` fields for face metrics/charmaps, `FT_Get_CMap_*` for CMap metadata, and SFNT table APIs for table data |
| Pillow adapter surface in `pillow-rs/src/font/imagingft.rs` | `getname`, `getmetrics`, `getlength`, `getbbox`, `getmask`, `render_text`, and `render_text_binary` model Pillow `_imagingft.c`, not C FreeType public API | Keep this logic in `pillow-rs` and its own parity matrix. Do not count these as missing FreeType manifest paths |
| High-level `fontdone::Font` convenience/PIL-style methods | `Font::getname`, `getmetrics`, `getlength`, `getbbox`, `getmask`, `layout_glyphs`, and related layout helpers still account for many `font.rs` misses | Treat as legacy/convenience surface unless a method backs an existing public FreeType route. Do not add synthetic fixture tests just to call these helpers; either route through real C FreeType parity or refactor the surface out of the FreeType coverage goal with independent compatibility proof |

| Bucket | Evidence | Action |
|---|---|---|
| Fixture/font reachable | `autohint/latin.rs`, `autohint/cjk.rs`, `scaler.rs`, `tt/hinter/exec.rs`, and parts of `render.rs` still have real branch gaps tied to glyph topology, script selection, bytecode state, or render geometry | Add or extend compact source-backed fonts and explicit public rows only when the selected glyph moves the measured branch or line |
| Public unsupported implementation paths | `FT_OpenType_Validate` non-null behavior still returns preserved stubs today | Implement the real public behavior first, then add parity rows; do not add fake success fixtures |
| Public-construction unreachable guards | Short required `head`/`hhea` tables fail face construction before `face_to_ffi`; short optional `vhea` currently fails in `Font::truetype`; `tt/post.rs` format-1 exact-258 Mac names are now covered, leaving only helper arms that public `FT_Get_Glyph_Name`/`FT_Get_Name_Index` skip through invalid-glyph validation or face-flag gates | Leave visible and documented unless parser semantics change or a true public route appears |
| SBIT residuals | `tt/sbit.rs` unsupported simple bit-depth, image-format, malformed small-metrics width, compound success, one packed MONO nonzero x-offset path, tail-bit carry, GRAY2/GRAY4 packed-depth dispatch arms, and zero-width packed component no-op are now covered through public rows; remaining misses are the unused/private load-status helper, checked arithmetic and 64-bit conversion guards, impossible compound count/component-read guards, impossible metric-conversion/pixel-mode mismatch guards, and defensive packed-blit overflow/truncation guards | Leave private helper and impossible overflow guards visible; add only real C-observable SBIT rows that move measured coverage through exact parity |
| Defensive invalid helper guards | `RoundMode::from_u8`'s invalid-value fallback remains missed after all valid FreeType `TT_Round_*` constants are routed through public `FT_Load_Glyph` rows | Leave visible; do not add a synthetic invalid round-state path unless a real public opcode or ABI surface can supply one |
| Private/no-route helpers | `Font::layout_glyphs`, `Font::layout_bounds`, `layout_bounds_from_glyphs`, `grays::rasterize`, `grays::rasterize_shifted_in_box`, and `grays::render_scanline` are not selected by the current public FreeType fixtures | Do not call these through synthetic tests; either expose a real public operation with C parity or prove and remove independently of coverage |
| Coverage instrumentation artifacts | `Font::load_sfnt_table` and several wrapper functions show zero-count closure symbols even while the public body is heavily executed; many missed `api.rs` and `font.rs` lines are trailing call arguments in covered functions | Use function bodies, contiguous blocks, and branch counters to choose cases; do not grow JSON for tail-line artifacts alone |

### SBIT Residual Map - 2026-07-14

Current `src/tt/sbit.rs` misses are classified as follows before adding any new
SBIT fixtures:

| Lines | Disposition | Reason |
|---|---|---|
| 141-150 | Private/no-route helper | `SbitTable::load_glyph_status` has no caller in `src/` or public input routing. Public loads use `Font::load_sbit_only_glyph`, which calls `load_glyph` so bitmap descriptors and bytes can be compared against C. |
| 164-168, 184-185 | Host-width unreachable EBLC arithmetic | `index_array_count`, `index_array_offset`, and per-range subtable offsets are 32-bit EBLC fields. On the 64-bit coverage host, converting them to `usize` and adding/multiplying within the parsed table range cannot overflow before the later slice bounds checks classify malformed public fonts. |
| 322-326, 380-384 | Host-width unreachable EBDT offset conversion | `SbitImageRecord` image offsets are `u32`; after the existing checked `u32` additions, `usize::try_from` cannot fail on the 64-bit host. Existing malformed offset rows cover public C-observable out-of-range behavior through the slice checks instead. |
| 391, 400-401, 413-414 | Internal invariant arms | `load_compound_image` is called only for image formats 8 and 9; compound records are sliced to an exact `num_components * 4` byte range before `chunks_exact(4)`, so the per-record glyph read cannot fail through public EBLC/EBDT bytes. |
| 460-461, 477-485, 511-513, 523-530 | SBIT format invariant arms | Small and big metrics store unsigned byte dimensions that are multiplied by 64, so negative or non-26.6 dimensions cannot be constructed by a public font. Component pixel mode must match because every component is loaded from the same strike bit depth. `dx`/`dy` are signed bytes, so the checked `u32` additions cannot overflow after the negative-offset guard. |
| 557, 560-590, 635-658 | Defensive blitter overflow/truncation guards | Public compound fixtures now cover GRAY, BGRA, MONO, GRAY2, GRAY4, shifted packed blits, tail carry, zero-width no-op, negative offsets, and out-of-bounds placement. The remaining guards require buffer lengths, pitches, or offsets inconsistent with `blank_compound_glyph`, `bitmap_layout_for_bit_depth`, and previously validated component bitmap lengths. |

### Scaler Residual Map - 2026-07-16

Current `src/scaler.rs` misses after the active-scale cleanup are not all fixture
candidates.  The verified branch baseline is `190 / 200` with runtime parity
`7,045 / 7,045` and route audit `real-parity=3,840`.

| Lines | Disposition | Reason |
|---|---|---|
| 734 | No-hinting plus autohint metrics | The false side of `latin_metrics.is_none()` inside the unhinted TrueType phantom branch would require `allow_bytecode=false` and autohint metrics at the same time. Public `FT_LOAD_NO_HINTING` dispatch supplies no autohint metrics, while public force-autohint/target modes keep `allow_bytecode=true` in the metrics route. |
| 771, 1583, 1774 | Empty-outline split states | Public glyph loading produces empty outlines with both `num_contours == 0` and no points, and `scale_glyph_impl` returns before exact-bbox decomposition or autohint mutation helpers. The missing sides require non-empty points with zero contours or empty contour vectors after point loading, which valid C FreeType glyph loading does not pass to these helpers. |
| 846, 848 | Nonexistent autohint style combinations | Public target-light reaches the `no_horizontal_hinting && !stem_adjust && !horz_snap && !vert_snap` style. Public LCD reaches `no_horizontal_hinting && !stem_adjust && horz_snap && !vert_snap`. No public load target currently supplies `no_horizontal_hinting` with `stem_adjust=true` or `vert_snap=true`. |
| 1434 | Public-construction unreachable owned context | Recursive native composite scaling receives a prepared bytecode context from `Font::native_bytecode_context_for_mode` whenever `fpgm` and `cvt` exist; without those tables, the inner prepare branch cannot execute. |
| 1621 | Parser-validated before scaler | Public glyf loading validates contour bounds before `decompose_bbox` consumes the outline tree. The remaining guards require contour endpoints that point before the current contour start or beyond the point array. |
| 1727-1747 | Private pixel helpers | Public scaler/render code uses the `ft_pix_*` helpers directly; these conversion wrappers need a real caller, not synthetic coverage rows. |

Resolved after this map: the `can_execute_native_bytecode` second-operand
branch at `scaler.rs:904` is covered by the public
`render-fpgm-no-cvt-default` route.  The compact generated font carries an
empty `fpgm` table but no `cvt`, so default TrueType loading proves the
FreeType fallback where native bytecode cannot execute even though `fpgm`
exists.  The later scaler cleanup also removed two impossible zero autohint
vertical-scale fallbacks, one redundant composite tag fallback, and the
active-vs-square scale comparison whose two return values were identical when
the condition was false; all were verified with exact runtime parity unchanged.

### Rejected Candidate Audit - 2026-07-13

These candidates were exact-parity probes but deliberately not kept because
they did not improve measured condition coverage or did not prove the intended
public surface:

| Bucket | Candidate | Result | Decision |
|---|---|---|---|
| Autohint script helpers | A candidate `script-notdef-glyph-force-autohint` row over existing `script-coverage.ttf` glyph 0 with `FT_LOAD_FORCE_AUTOHINT` | Focused `load_glyph` parity passed, and full runtime parity rose to 6,752 / 6,752, but condition coverage stayed exactly flat at 16,215 / 18,091 lines, 23,284 / 25,933 regions, 3,906 / 4,632 branches, and 1,024 / 1,150 functions; `globals.rs` and `globals_data.rs` missing lines were unchanged | Do not add more script rows for helper-only coverage. The maintained public route uses face-global style coverage plus `style.blue_entries`; the duplicate blue-character lookup and duplicate `globals::detect_script` helper were removed after caller and pinned-C audits |
| Autohint glyph-zero metrics | `FT_Load_Glyph.matrix_load@autohint-notdef-glyph-zero` over `fonts/autohint/digit-notdef-cmap.ttf` gid 0 with `FT_LOAD_FORCE_AUTOHINT` | Focused exact Rust FFI / C ABI / WASM ABI parity passed 1 / 1 and kept implicit cases at zero, but the focused coverage JSON showed `autohint/globals.rs:95,100,102,106` all at zero hits; the public load path bypasses `FaceGlobals::get_metrics` for this row | Do not add this row. It increases concrete cases without covering the glyph-zero metrics branch; a future candidate must prove source-line movement before entering the optimized fixture set |
| Scaler malformed composite | `glyf-malformed-invalid-point-attachment-no-hinting` over `fonts/glyf/glyf-malformed-matrix.ttf` gid 17 with `FT_LOAD_NO_HINTING` | Focused `FT_Load_Glyph` parity passed, but full condition coverage stayed flat at 16,215 / 18,091 lines, 23,284 / 25,933 regions, 3,906 / 4,632 branches, and 1,024 / 1,150 functions | Do not add this row. The public parser rejects the invalid attachment before the scaled composite helper's defensive error branch |
| Autohint topology | `latin-double-top-glyph-force-autohint`, `cjk-snap-below-standard-normal-force-autohint`, and `cjk-snap-below-standard-lcd-force-autohint` over existing compact autohint fonts | Each focused row passed exact parity, but full condition coverage stayed flat at 15,947 / 17,810 lines, 22,852 / 25,492 regions, 3,832 / 4,536 branches, and 1,005 / 1,135 functions | Do not re-add these rows. The next autohint improvement needs genuinely new glyph topology, not another explicit row over the current compact fonts |
| Autohint topology/load target | `cjk-wide-stem-snap-target-lcd-20` over `fonts/autohint/cjk-wide-stem-snap.ttf` U+4ED6 with `FT_LOAD_FORCE_AUTOHINT | FT_LOAD_TARGET_LCD` | Focused exact Rust FFI / C ABI / WASM ABI parity passed 1 / 1 and kept implicit cases at zero, but comparing the focused condition JSON against the 6,775-case baseline showed no newly covered baseline-missed lines or autohint branch outcomes | Do not add an LCD-only row over the existing wide-stem snap topology; a future target-mode row needs distinct geometry that moves a measured branch or line |
| Autohint CJK link-update topology | `cjk-link-update-closer-peer-20` over a candidate extension to `fonts/autohint/cjk-duplicate-edge.ttf` | Focused parity passed and full runtime parity rose to 6,797 / 6,797, but total coverage stayed flat except for the separate TT branch probe: `autohint/cjk.rs` remained 893 / 941 lines, 1,187 / 1,247 regions, and 381 / 426 branches | Do not add this CJK row. Future CJK rows need measured autohint source movement, not another close-link topology over the existing duplicate-edge fixture |
| Latin tilde-loop topology | `probe-script-latin-tilde-top-flat-loop` over existing `script-coverage.ttf` U+00C3 with `FT_LOAD_FORCE_AUTOHINT` at 20 ppem | Focused `make -C pillow-rs-freetype test-case CASE=probe-script-latin-tilde-top-flat-loop` passed exact parity 1 / 1, and full `make -C pillow-rs-freetype test-unified-condition-coverage` passed runtime 7,046 / 7,046 with four pending rows, but `src/autohint/latin.rs` stayed flat at 2,608 / 2,844 lines, 1,079 / 1,286 branches, and 70 / 73 functions | Do not add another row for the existing top-tilde flat-loop glyph. The remaining Latin tilde misses need different contour topology or implementation proof, not this already-generated glyph |
| TrueType GETINFO LCD target | `FT_Load_Glyph.matrix_load@hinter-stack-state-lcd-getinfo` over existing `fonts/glyf/hinter-control-matrix.ttf` gid 30 with `FT_LOAD_TARGET_LCD` | Focused exact Rust FFI / C ABI / WASM ABI parity passed 1 / 1 and route audit classified the row as real parity, but full condition coverage stayed exactly flat at 17,839 / 19,838 lines, 25,712 / 28,583 regions, 4,215 / 4,917 branches, and 1,149 / 1,332 functions | Do not add this row. Existing stack-state bytecode already covers the observable public path; this LCD-target variant increases case count without moving measured source coverage |
| TrueType empty-zone SHZ | A derived `hinter-empty-composite-shz.ttf` font whose empty composite glyph carried `PUSHB[0] 0; SHZ[0]`; the retained source-backed prep probe appends `SZPS 1; SHZ[0]` to `hinter-control-matrix.ttf` | The derived glyph row passed exact C/Rust/C-ABI/WASM parity but stayed flat. The retained prep route hits `tt/hinter/exec.rs:1408` because prep executes against an empty glyph zone before glyph loading | Do not add the derived font. Keep the existing base-prep route because it covers the public empty-zone `SHZ` branch with no concrete case growth |
| Render SDF/cubic | A possible CFF/CFF2 `FT_Render_Glyph` SDF row | Current Rust face loading still relies on glyf/loca fallback for the compact CFF fixture, so C would render cubic charstrings while Rust would not preserve a cubic public glyph outline through this path | Implement a real public cubic-outline loader route before adding SDF cubic render fixture rows |
| Render mono/profile | Remaining `render.rs` mono/profile branches are in the active `MonoOutlineProfileBuilder` sweep and dropout helpers | The old segment-based profile builder, intersection rasterizer, horizontal center-edge pass, and low-level no-precision wrappers were independently proven no-call duplicate/private code and removed | Add compact glyphs only for active public render branches after a focused condition delta proves movement |
| Historical size lifecycle sketch | Rust FFI-only implementation sketch for `FT_New_Size`, `FT_Done_Size`, `FT_Activate_Size`, and `FT_Select_Size` success sequences | Superseded by the verified face-owned size implementation with direct C ABI and WASM ABI lifecycle exports. Focused sequence parity and full condition coverage now pass with the size lifecycle and fixed-strike selection rows classified as real parity | Do not reintroduce Rust-only C/WASM delegation for lifecycle rows. Future size work should target new public behavior from the route audit rather than generic modeled rows |
| Safe render no-value rows | Adding `assert_font_render_mode_agrees` to the existing Noto `FORCE_AUTOHINT | NO_AUTOHINT` render row passed focused `render_glyph` parity and full runtime parity, but total condition coverage stayed fixed at 16,227 / 18,091 lines, 23,297 / 25,933 regions, 3,909 / 4,632 branches, and 1,025 / 1,150 functions | Do not add more safe render agreement flags to rows that already exercise the same load-mode branch |
| Ftsynth bitmap zero-strength no-op | Adding `{xdelta_16_16: 0, ydelta_16_16: 0}` to the existing `sbit-gray8-format1` `FT_GlyphSlot_AdjustWeight.bitmap_weight_owns_emboldens_and_updates_top` adjustment list | Focused exact Rust FFI / C ABI / WASM ABI parity passed 21 / 21, but full condition coverage stayed exactly flat at 17,840 / 19,838 lines, 25,713 / 28,583 regions, 4,216 / 4,917 branches, and 1,149 / 1,332 functions; `api.rs:942` stayed in the missing-line list | Do not add zero-strength bitmap ftsynth rows. The public ftsynth bitmap path forces a one-pixel horizontal embolden before reaching the helper, so this input does not cover the private bitmap no-op branch |
| Render top-edge horizontal dropout | `FT_Render_Glyph.matrix_render@render-coverage-top-edge-horizontal-dropout-mono`, an appended `render-coverage.ttf` glyph with a near-top horizontal strip rendered at 16 ppem MONO | Focused exact Rust FFI / C ABI / WASM ABI parity passed 1 / 1, but full condition coverage stayed exactly flat at 17,840 / 19,838 lines, 25,713 / 28,583 regions, 4,216 / 4,917 branches, and 1,149 / 1,332 functions; `render.rs:1745,1759,1772` stayed in the missing-line list | Do not add this topology. The current public mono path does not reach the out-of-bounds horizontal dropout helper from this shape; future render rows need a focused condition delta before entering the optimized set |
| Zero-width normal render row | A candidate `FT_RENDER_MODE_NORMAL` row over `fonts/glyf/hinter-control-matrix.ttf` U+E02B (`renderZeroWidth`) with safe render and getmask assertions passed exact parity, raising concrete rows locally to 6,758, but condition coverage and missing lines were unchanged | Do not keep this row. The current public load/render path does not reach a new zero-extent `Font::getmask_single_glyph` or render guard from that glyph |
| Render top-boundary gray clip | A candidate `render-coverage.ttf` grid-aligned 3x3 box rendered in normal mode passed focused `render_glyph` parity and raised concrete rows locally to 6,766 | It only increased already-covered `grays.rs` top/right clipping counts and left total, `render.rs`, and `grays.rs` line/region/branch/function coverage unchanged | Do not add top-boundary gray rows unless the focused condition report shows a new uncovered outcome, not just higher execution counts |
| Render mono/SDF residuals | Candidate rows `render-coverage-non-grid-profile-close-mono`, `render-coverage-left-edge-dropout-mono`, and `render-coverage-large-box-sdf` | Each candidate passed focused parity, and the non-grid profile candidate also passed full coverage parity, but none moved total or `render.rs`/`grays.rs` line, region, or branch counters | Do not retry these render geometries. Remaining render work needs a real uncovered public route such as overlap behavior or a source path proven by focused condition deltas |
| Render normal empty loaded outline | Candidate row `FT_Render_Glyph.matrix_render@dejavu-space-default-empty-outline-normal` over DejaVuSans glyph 3 with `FT_LOAD_DEFAULT` and `FT_RENDER_MODE_NORMAL` | Focused exact Rust FFI / C ABI / WASM ABI parity passed 1 / 1, and full condition coverage passed with runtime parity 7,046 / 7,046, but `src/render.rs` stayed 2,099 / 2,597 lines and 379 / 434 branches. `render.rs:402` still misses only the `n_contours == 0` operand; the candidate exercises the already-covered `points.is_empty()` side. | Do not add normal space rows for this guard. Covering the remaining operand requires a public loaded outline with non-empty points and zero contours; valid font loading does not produce that state, and malformed synthetic outlines need a separate C-oracle public route. |
| TrueType IUP duplicate contour | A derived `hinter-duplicate-contour-iup.ttf` probe replaced gid 55 with a one-point simple glyph whose two contour end-points were both zero and whose glyph program ran `IUP[y]; IUP[x]` | Focused selection stayed to one explicit `FT_Load_Glyph.matrix_load` row, but the pinned FreeType oracle returned error 20 for the load before any comparable slot state existed | Do not add duplicate or non-advancing contour-end probes. They are invalid public glyph data for the C oracle, so they cannot be used to cover Rust-only IUP defensive branches without sacrificing parity correctness |

### Table And FFI No-Route Addendum - 2026-07-13

These misses were audited after the 6,757-case checkpoint. They are not
fixture-row candidates unless a new public route appears:

| Source lines | Classification | Reason |
|---|---|---|
| `casts.rs:28,86` | Caller-invariant debug assertions | `i32_from_i64` receives raster values widened from i32 26.6 coordinates and shifted back within that range, matching `ftgrays.c`'s `TPos`/`TCoord` conversion. `usize_from_i64` receives validated i16 contour endpoints only after negative and decreasing endpoints have been rejected. Reaching either false side requires private state outside the pinned public C contracts; keep the assertions and do not add malformed placeholder rows |
| `autohint/coverage.rs:110-135` | Resolved by public-row assertion | The existing `FT_LOAD_FORCE_AUTOHINT.load_char_force_autohint_behavior@latin-italic-no-horizontal` row now declares `assert_autohint_coverage_bits_include: [32]`, proving the safe public load route records `COV_ITALIC_NO_HORZ` without adding a standalone fixture test |
| `tt/post.rs:47` | Public-gate unreachable | `FT_Get_Glyph_Name` validates `glyph_index < num_glyphs` before `PostTable::glyph_name`, and `FT_Get_Name_Index` scans only valid glyph indexes. The format 3.0 and unsupported-format residuals were resolved by assigning the face-flag gate to `Font` and retaining the C service's initialized `.notdef` behavior in `PostTable` |
| `autohint/loader.rs:339` | Private metric-less hints fallback | Public `apply_hints` builds `GlyphHints` and the Latin/CJK setup paths install metrics before direction-chain construction. The default `near_limit_chain = 20` fallback only exists for private or diagnostic `GlyphHints` values without metrics; do not add synthetic autohint calls for it |
| Former `tt/fvar.rs:58-59` | Resolved by C validation parity | Rust now mirrors `sfnt_init_face`'s version, axis/instance count, record-size, and table-length limits before direct offset arithmetic. Five exact public face-flag rows prove malformed minor versions, oversized records, and count limits; no synthetic parser call is used |
| Former `tt/cmap.rs:786-789,866-867,914-915` | Resolved by format-14 validator parity | Rust now mirrors `tt_cmap14_validate` by comparing selector/default/non-default counts with remaining bytes divided by record width. The existing exact malformed format-14 matrix proves all three rejection paths; no overflow-only row is needed |
| Former `tt/gasp.rs:59,62` | Resolved by C arithmetic parity | `num_ranges` is a 16-bit SFNT field, so the supported native/wasm32 range array is bounded to 262,144 bytes. Rust now mirrors C's direct arithmetic, while 13 exact public `FT_Get_Gasp` rows retain all observable malformed and valid behavior |
| Former `tt/hinter/gs.rs:59` | Resolved by typed C-state assignment | Pinned C assigns one of eight `TT_Round_*` constants directly in each rounding opcode. Rust now does the same with `RoundMode` variants; the non-FreeType `from_u8` helper and impossible invalid-value fallback were removed. Existing exact `stackStateMatrix` and `superRoundMatrix` rows cover all eight states |
| Former `tt/hinter/mod.rs:284,373,394` | Resolved by C-aligned caller contract | Pinned `TT_Hint_Glyph` consumes the size execution state prepared before glyph loading; it neither runs `fpgm`/`prep` nor receives an empty outline with executable glyph instructions. Rust now establishes one prepared context at the scaler boundary, reuses it for composite recursion, and gives the crate-private `hint_glyph` the same non-empty outline contract. This removes a duplicate private composite setup path and three fallback branch sides that no public C glyph load can select |
| `ffi/convert.rs` residual conversions | Public gate or internal-driver state | `GlyphFormat::None` requires the maintained unloaded-slot runner route. `MissingBitmap` and the SBIT form of `InvalidComposite` are internal `ttsbit.c` errors that `TT_Load_Glyph` converts to `Invalid_Argument`; the malformed TrueType composite route still needs real core support. `ExecutionTooLong` already has an exact public load row, but LLVM does not attribute the inlined converter arm. The former `UnsupportedCmapFormat` and `UnsupportedLoadFlags` core variants were removed: unsupported SFNT cmap classes are ignored while building the face, and raw load-flag rejection occurs in the FFI flag parser before a core error can exist. |
| `tt/hinter/exec.rs:265-283` | Private/no-route fetch helpers | The active interpreter loop uses `fetch_byte_glyph`; the older public `ExecContext::fetch_byte`/`fetch_word` helpers are not selected by public glyph-load execution |
| `tt/hinter/exec.rs:293` | Private call-site preempted by prepare path | The compact `hinter-empty-fpgm.ttf` row already proves empty font-program handling through public `FT_Load_Glyph`, but the prepare path skips `run_fpgm` entirely when `fpgm` is empty, so the internal `run_fpgm` empty-return line has no public fixture route |
| `tt/hinter/exec.rs:446` | Caller-guarded no-route branch | Public opcode handlers that need original twilight coordinates route through `org_in` whenever any zone pointer is twilight. The remaining `orus_in(..., zp=0)` fallback is defensive; valid `MD`, `MDRP`, `MIRP`, and `IP` public bytecode paths do not select it |
| `tt/hinter/exec.rs:508` | Short-circuit no-route branch | `SHPIX` compatibility handling sets `in_twilight` when any zone pointer is twilight and short-circuits before consulting `tag_in`; non-twilight paths use glyph tags. The twilight `tag_in` arm has no public opcode route |
| `tt/hinter/exec.rs:715-717` | Invariant-backed inactive-definition guard | Public FDEF/IDEF scanners either create active definitions or reject invalid, nested, over-budget, and unterminated definitions before calls. Existing call-error rows cover absent and invalid references; no public route produces an inactive definition record that can later be called |
| `tt/hinter/exec.rs:1464-1467` | Call-record contract guard | `enter_function_call` pushes `CallRecord` only after resolving an active definition index. The repeated `LOOPCALL` ENDF path therefore cannot lose its definition between pop and repeat without mutating private VM state; keep the guard, but do not add synthetic record corruption tests |
| Former `tt/hinter/iup.rs:39-41,44,55,91` | Resolved by parser-boundary parity and internal zone contract | Pinned `TT_Load_Simple_Glyph` rejects duplicate/decreasing contour endpoints before bytecode, and the sole live Rust constructor builds point, tag, original, and unscaled arrays one-for-one. A generated duplicate-endpoint font now proves the public invalid-outline result through all four lanes; `iup` is crate-private and retains only C's legal empty-interval return for adjacent touched points |

### Render/Raster Residual Audit - 2026-07-13

The current `route-audit` split for `FT_Render_Glyph` is 183 real-parity rows,
one `null-error-fallback` row, and one `pending-core` row. The remaining
non-real render routes are
`freetype.FT_Render_Glyph.error_null_or_unowned_slot` and
`freetype.FT_Render_Glyph.error_unloaded_or_unsupported_slot_format`. The null
row lacks a maintained glyph-slot selector, and the unloaded/unsupported-slot
row needs explicit synthetic slot-state runner support. The former
`ftimage.FT_OUTLINE_OVERLAP.smooth_overlap_behavior` variants are now real
source-backed render parity rows.

Remaining uncovered render/raster source families:

| Source family | Classification | Decision |
|---|---|---|
| `grays::{rasterize,rasterize_shifted_in_box}` wrapper lines | Private/no-route from `FT_Render_Glyph`; direct outline surfaces own any future public `FT_Raster_Params` or clipping route | Do not add synthetic render rows. Add a real outline-render route only when the ABI surface exists and exact C/Rust/C-ABI/WASM parity can be compared |
| `grays::{ft_div_mod,Worker::render_scanline}` | Obsolete/no-call scanline helper path; current gray rendering reaches the DDA line/conic/cubic raster path instead | Keep visible until independently proven duplicate and removed in a cleanup not justified by coverage alone |
| `grays.rs` trace and debug-dump lines | Coverage instrumentation artifacts gated by debug logging or environment-controlled dumps | Do not add parity rows for logging-only execution |
| Removed `render::render_loaded_char_mode_for_index` | Safe convenience helper with no caller and no current public FreeType manifest route; `FT_Render_Glyph` exercises the loaded slot render path instead | Do not re-add helper-only coverage. Add a real route only if a public safe Rust API obligation is introduced |
| `render::render_normal` and `render::render_sdf` zero-extent guards | Mostly shadowed by `render_loaded_outline` empty/box checks for valid `FT_Render_Glyph` rows | Add no row unless a valid C fixture reaches the same guard and moves coverage |
| `render::SdfFlattener` cubic or invalid-contour paths | Fixture/font reachable only after a real public cubic-outline loader route exists; current compact CFF probe would make C render cubic charstrings while Rust does not preserve that outline through this path | Implement cubic outline loading/parity first, then add the smallest SDF row |
| Removed render mono duplicate helpers | The old `MonoProfileBuilder`, intersection rasterizer, horizontal center-edge pass, low-precision line/bezier wrappers, and unused SDF winding helper had no caller from the public render path; current mono output uses `MonoOutlineProfileBuilder`, and the horizontal/vertical dropout guards are already covered by `render-coverage.ttf` rows | Do not grow `FT_Render_Glyph` JSON with duplicate topology or re-add helper-only tests |
| `render::MonoOutlineProfileBuilder` branch residuals | Potentially fixture/font reachable, but only for exact topology branches not already covered by `hinter-control-matrix.ttf` or `render-coverage.ttf`; the folded-profile non-adjacent upper-stub branch is now covered by `render-coverage-folded-dropout-mono` | Add compact glyphs only after a focused candidate moves measured condition coverage with exact parity |
| `render::render_normal_overlap` residual error return | The compact wide glyph now proves the public width-overflow branch and `FontError::RasterOverflow` conversion. The remaining missed line is the propagated error return from the gray rasterizer call inside the overlap helper | Add no synthetic row. A future case must make pinned C return an error from the oversampled gray rasterizer after passing the width check, with exact public error parity |

Readable zero-count functions from `llvm-cxxfilt` fall into this first
ledger. Treat this as the next owner list, not as deletion evidence:

| Disposition | Zero-count functions or families | Route decision |
|---|---|---|
| Implement before fixture parity | `ffi::handles::FT_New_Face` | This is a public stub; fixtures must not fake success until core behavior exists and C/WASM ABI wrappers remain thin |
| Resolved duplicate/no-route helper | Former `globals_data::blue_chars_for_script` | Removed after proving that it had no caller or public FreeType endpoint and duplicated the generated `blue_strings.rs` data selected through `StyleClass::blue_entries` |
| Public helper not owned by current FreeType manifest route | `Direction::{as_i8,is_horizontal,is_vertical}`, `GlyphHints::num_contours`, `ExecContext::{fetch_byte,fetch_word}` | Keep visible; either route through an existing public manifest subject with real C parity or decide separately whether these helpers belong in public Rust surface |
| Private/no-route implementation helpers | `Font::{layout_glyphs,layout_bounds,slot_metrics_from_scaled,native_bytecode_context}`, `layout_bounds_from_glyphs`, `grays::{rasterize,rasterize_shifted_in_box,ft_div_mod,Worker::render_scanline}`, `render::unpack_mono_row` | Do not add synthetic tests. A real public operation must need them, or they need independent semantic cleanup after proving they are duplicate/obsolete |
| Covered body with closure artifact | `Font::load_sfnt_table` closures, `Font::truetype_face_with_load_mode` closures, `SizeMetrics::from_char_size` closure, and `api::GlyphSlot::new` closure | Do not add fixture rows for closure symbols alone. Add rows only if a public error branch or output difference is missing |
| Fixture/font reachable candidates | `cjk::cjk_mark_round_segments`, parts of `SdfFlattener`, `MonoOutlineProfileBuilder`, `scaler` metric/composite helpers, and `tt::hinter::exec::run_program` closure | Add compact glyph/topology/program rows only after identifying the exact branch and proving the row moves coverage with exact parity. `latin::find_second_lowest_contour` remains preserved code, but pinned FreeType 2.14.3 defines `AF_ADJUST_DOWN2` / `AF_ADJUST_TILDE_BOTTOM2` without any adjustment-database entries, so it is not currently reachable through a real public `char_code` fixture row |

2026-07-17 autohint helper cleanup: `globals::detect_script` duplicated the
maintained implementation in `script.rs`, while `latin::metrics_init_blues` and
`latin::metrics_init_blues_greek` had no callers and did not correspond to
public FreeType entry points.  The face-global route already selects a
`STYLE_TABLE` entry and calls `metrics_init_blues_impl`, matching
`af_face_globals_get_metrics`; the duplicate detector and wrapper exports were
retired without changing that implementation path.

2026-07-17 explicit-autohinter index ordering: pinned `FT_Load_Glyph` dispatches
explicit FORCE_AUTOHINT and target-light loads to `af_loader_load_glyph` before
the font driver validates the glyph index.  `af_face_globals_get_metrics`
therefore returns `FT_Err_Invalid_Argument` for an out-of-range index, not the
driver's `FT_Err_Invalid_Glyph_Index`.  Rust core now owns that ordering; thin
C and WASM wrappers defer explicit-autohinter validation to the shared core.
The combined bucket moved `autohint/globals.rs` from 215 / 225 lines, 67 / 80
branches, and 13 / 14 functions to 206 / 206 lines, 66 / 66 branches, and
13 / 13 functions. Runtime parity moved from 7,100 / 7,100 to 7,102 / 7,102,
real-parity routes moved from 3,901 to 3,903, and the four explicit pending
rows were unchanged.

### FTSynth Null-Slot Checkpoint - 2026-07-13

This batch made the remaining public ftsynth null-slot no-op obligation
explicit without adding a synthetic slot model or a public WASM-only escape
hatch.  `ftsynth.glyphslot_null_noop` calls the pinned C oracle, Rust FFI, and
C ABI raw `FT_GlyphSlot` surface for `FT_GlyphSlot_AdjustWeight`,
`FT_GlyphSlot_Embolden`, `FT_GlyphSlot_Oblique`, and `FT_GlyphSlot_Slant`.
The WASM ABI is explicitly non-applicable for these rows because its public
surface exposes face handles, not raw glyph-slot pointers.  The existing
`FT_GlyphSlot_Embolden.null_or_unsupported_format_noop` row now uses the real
`glyf-component-matrix.ttf` composite slot loaded with `FT_LOAD_NO_RECURSE`
instead of a future synthetic slot asset.

Verified counts after `make -C pillow-rs-freetype test-unified-condition-coverage`:

| Measure | Count |
|---|---:|
| Logical public API cases | 4,163 |
| Concrete explicit cases | 6,764 |
| Runnable parity comparisons | 6,760 / 6,760 |
| Pending cases | 4 |
| Covered Rust lines | 16,238 / 18,095 (89.7375%) |
| Rust region coverage | 23,305 / 25,934 (89.8627%) |
| Rust branch/condition coverage | 3,918 / 4,634 (84.5490%) |
| Rust function coverage | 1,025 / 1,150 (89.1304%) |
| Route audit split | real-parity 3,404; raw-slot-null-validation 4; shape-incomplete-fallback 2 |

The previously missed `ffi/handles.rs` null-return lines for
`FT_GlyphSlot_AdjustWeight` and `FT_GlyphSlot_Slant` are now covered.  The two
remaining ftsynth shape-incomplete rows are the embedded-bitmap strike cases
for `FT_GlyphSlot_AdjustWeight` and `FT_GlyphSlot_Embolden`.

### Autohint Coverage-Bit Checkpoint - 2026-07-13

The existing
`FT_LOAD_FORCE_AUTOHINT.load_char_force_autohint_behavior@latin-italic-no-horizontal`
row now declares `assert_autohint_coverage_bits_include: [32]`, proving the
safe public load route records `COV_ITALIC_NO_HORZ` from the italic Latin
autohint branch. The unified harness resets and reads the autohint accumulator
only when a public input row asks for that assertion; it does not add a
separate fixture test or hidden case expansion.

Verified counts after `make -C pillow-rs-freetype test-unified-condition-coverage`:

| Measure | Count |
|---|---:|
| Logical public API cases | 4,163 |
| Concrete explicit cases | 6,764 |
| Runnable parity comparisons | 6,760 / 6,760 |
| Pending cases | 4 |
| Covered Rust lines | 16,260 / 18,095 (89.8591%) |
| Rust region coverage | 23,333 / 25,934 (89.9707%) |
| Rust branch/condition coverage | 3,922 / 4,634 (84.6353%) |
| Rust function coverage | 1,030 / 1,150 (89.5652%) |
| Route audit split | real-parity 3,404; raw-slot-null-validation 4; shape-incomplete-fallback 2 |

The delta from the FTSynth checkpoint is +22 covered lines, +28 covered
regions, +4 covered branches, +5 covered functions, and +5 covered
instantiations, with no concrete case-count increase. `autohint/coverage.rs`
no longer appears in the missing-line report and is now covered at 28 / 28
lines, 35 / 35 regions, 7 / 7 functions, and 4 / 4 branches.

### Autohint Digit Cmap Glyph-Zero Checkpoint - 2026-07-13

`build_autohint_script_fixtures.py` now emits
`fonts/autohint/digit-notdef-cmap.ttf`, a 1.1 KiB source-backed glyf fixture
whose cmap explicitly maps U+0030 to `.notdef`/glyph 0 while U+006F selects a
normal Latin ring. One public `FT_LOAD_FORCE_AUTOHINT` row selects U+006F, so
the face-global metrics setup scans a cmap-covered digit that FreeType reports
as glyph index 0 without rendering the `.notdef` glyph itself. This covers the
Rust branch in `digits_have_same_width` that previously remained missed because
existing compact fonts either omitted digits or mapped them to nonzero glyphs.

Verified counts after `make -C pillow-rs-freetype test-unified-condition-coverage`:

| Measure | Count |
|---|---:|
| Logical public API cases | 4,163 |
| Concrete explicit cases | 6,765 |
| Runnable parity comparisons | 6,761 / 6,761 |
| Pending cases | 4 |
| Covered Rust lines | 16,261 / 18,095 (89.8646%) |
| Rust region coverage | 23,335 / 25,934 (89.9784%) |
| Rust branch/condition coverage | 3,925 / 4,634 (84.7000%) |
| Rust function coverage | 1,030 / 1,150 (89.5652%) |
| Route audit split | real-parity 3,405; raw-slot-null-validation 4; shape-incomplete-fallback 2 |

The delta from the autohint coverage-bit checkpoint is +1 covered line, +2
covered regions, and +3 covered branches, with one additional concrete public
case. `autohint/globals.rs:317` no longer appears in the missing-line report.

### Size Null-Validation Coverage Attribution - 2026-07-13

The `ftsizes.FT_New_Size.null_output_pointer_error` row was rechecked with:

```bash
FONTDONE_UNIFIED_OPERATION_FILTER="ftsizes.new_size" \
FONTDONE_UNIFIED_CASE_FILTER="null_output_pointer_error" \
FONTDONE_UNIFIED_ORACLE_REFRESH=1 \
make -C pillow-rs-freetype test-unified-condition-coverage \
  CONDITION_COVERAGE_OUTPUT=target/coverage/probes/ftsizes-new-size-null-output-summary.json \
  CONDITION_COVERAGE_LINES_OUTPUT=target/coverage/probes/ftsizes-new-size-null-output-missing-lines.txt
```

The row passes one exact Rust FFI / C ABI / WASM ABI parity comparison. The
LLVM coverage export shows `FT_New_Size` entered three times and the
`size.is_none()` condition taking the true branch three times. The standalone
`return FT_Err_Invalid_Argument` source line still reported as missed because
coverage was attributed to the guard condition line, not to the following
return-only line. `FT_New_Size`, `FT_Done_Size`, and `FT_Activate_Size` now use
rustfmt-stable match arms so covered null-validation behavior is represented on
covered source lines. The non-null `Unimplemented_Feature` arms are
unchanged and remain pending-core lifecycle work, not coverage-complete
behavior.

Verified counts after the follow-up
`make -C pillow-rs-freetype test-unified-condition-coverage` run:

| Measure | Count |
|---|---:|
| Logical public API cases | 4,163 |
| Concrete explicit cases | 6,764 |
| Runnable parity comparisons | 6,760 / 6,760 |
| Pending cases | 4 |
| Covered Rust lines | 16,258 / 18,090 (89.8729%) |
| Rust region coverage | 23,329 / 25,927 (89.9796%) |
| Rust branch/condition coverage | 3,917 / 4,626 (84.6736%) |
| Rust function coverage | 1,030 / 1,150 (89.5652%) |

`ffi/handles.rs` now reports 1,610 / 1,637 covered lines and only the
non-null size lifecycle arms at lines 245, 252, and 259 remain missed for the
size APIs. Those three lines are blocked by the real multi-size implementation,
not by missing null-validation inputs.

### FTSynth Large-CBox Orientation Guard - 2026-07-13

`FT_GlyphSlot_AdjustWeight.negative_bounds_outline_weight_uses_unsigned_abs`
now has one additional concrete variant over existing compact
`fonts/glyf/hinter-control-matrix.ttf` glyph 49 at 65,535 ppem. The selected
outline's scaled control box exceeds FreeType's orientation helper coordinate
guard, so `FT_Outline_Get_Orientation` returns `FT_ORIENTATION_NONE` while the
public glyph-slot mutation still succeeds and updates the exact slot metrics,
advance, outline points, and control box through Rust FFI, C ABI, and WASM ABI
routes.

Integrated branch counts after
`FONTDONE_UNIFIED_ORACLE_REFRESH=1 make -C pillow-rs-freetype test-unified-condition-coverage`:

| Measure | Count |
|---|---:|
| Logical public API cases | 4,163 |
| Concrete explicit cases | 6,768 |
| Runnable parity comparisons | 6,764 / 6,764 |
| Pending cases | 4 |
| Covered Rust lines | 16,262 / 18,090 (89.8950%) |
| Rust region coverage | 23,335 / 25,927 (90.0027%) |
| Rust branch/condition coverage | 3,924 / 4,626 (84.8249%) |
| Rust function coverage | 1,030 / 1,150 (89.5652%) |

The ftsynth row itself contributes one additional concrete public case plus +1
covered line, +1 covered region, and +1 covered branch.
`src/api.rs:1059` no longer appears in the full missing-line report.

### Public Outline Orientation Route - 2026-07-13

`FT_Outline_Get_Orientation` is now a maintained public parity route for the
existing `ftoutln.get_orientation` manifest rows.  The route uses the pinned C
oracle's `FT_Outline_Get_Orientation` over a compact deterministic outline
model set, then compares the same null, empty, positive-area, negative-area,
collapsed, oversized, and zero-area shapes through Rust FFI, C ABI, and WASM
ABI.  The C and WASM wrappers stay thin: they only copy the raw outline record
into the existing `FT_OutlineSnapshot` boundary and delegate to the core
orientation helper.

Focused verification:

```bash
make -C pillow-rs-freetype test-op OP=ftoutln.get_orientation
```

Result: `9 / 9` focused comparisons passed.  The route audit moved the eight
`ftoutln.get_orientation` rows from `generic-fallback` to `real-parity`
(`real-parity 3,442 -> 3,450`, `generic-fallback 924 -> 916`).

Integrated counts after
`make -C pillow-rs-freetype test-unified-condition-coverage`:

| Measure | Count |
|---|---:|
| Logical public API cases | 4,165 |
| Concrete explicit cases | 6,794 |
| Runnable parity comparisons | 6,791 / 6,791 |
| Pending cases | 3 |
| Covered Rust lines | 17,008 / 18,897 (90.0037%) |
| Rust region coverage | 24,444 / 27,182 (89.9272%) |
| Rust branch/condition coverage | 4,035 / 4,736 (85.1985%) |
| Rust function coverage | 1,104 / 1,258 (87.7583%) |

Compared with the previous checkpoint this added +44 covered lines, +62
covered regions, +4 covered branches, and +8 covered functions while adding
one new missed defensive conversion line in `ffi/handles.rs`.

### Size Lifecycle Probe Variants - 2026-07-13

`FT_Done_Size.remove_secondary_size_success` now uses two explicit grouped
inputs instead of one aggregate row: one destroys the active secondary size,
and one destroys an inactive secondary size while the initial size remains
active.  This keeps the same manifest case but makes the actual public size
list variation visible in JSON.  Focused
`make -C pillow-rs-freetype test-op OP=ftsizes.done_size_sequence` passes
`2 / 2` exact Rust FFI, C ABI, and WASM ABI comparisons against pinned C.
The inactive-size variant covers the `FaceSizeState::remove` non-active
fallback branch; `ffi/handles.rs:129` from the prior report no longer appears
in the missing-line file.

`FT_New_Size.create_secondary_size_success` likewise now uses explicit
`normal-face` and `negative-face-index-probe` inputs.  The probe row records a
native FreeType nuance from `FT_New_Memory_Face(face_index=-1)`: the opened
face starts with `face->size == NULL`, but `FT_New_Size` may still allocate a
secondary size and `FT_Activate_Size` may activate it.  Rust previously guarded
`FT_New_Size`, `FT_Set_Char_Size`, `FT_Set_Pixel_Sizes`, and
`FT_Request_Size` on the probe flag itself; the core now matches FreeType by
initializing probe faces with an empty size list and using active-size
presence as the size-operation gate.  Focused
`make -C pillow-rs-freetype test-op OP=ftsizes.new_size_sequence` passes
`2 / 2` exact comparisons.

Integrated counts after
`make -C pillow-rs-freetype test-unified-condition-coverage`:

| Measure | Count |
|---|---:|
| Logical public API cases | 4,165 |
| Concrete explicit cases | 6,797 |
| Runnable parity comparisons | 6,794 / 6,794 |
| Pending cases | 3 |
| Covered Rust lines | 17,016 / 18,901 (90.0270%) |
| Rust region coverage | 24,447 / 27,180 (89.9448%) |
| Rust branch/condition coverage | 4,036 / 4,730 (85.3277%) |
| Rust function coverage | 1,105 / 1,259 (87.7681%) |

Route audit now reports `6,797` concrete cases with `3,453` real-parity
routes, `916` generic fallbacks, and `10` pending-core rows.  Runtime parity
passes `6,794 / 6,794`; the remaining three pending rows are the explicit
FTMM named-instance obligations.
| 2026-07-14 | TrueType undefined function call no-op | The existing source-backed `hinter-control-matrix.ttf` control-flow glyph now executes undefined `CALL` and `LOOPCALL` probes against function 9. Pinned C FreeType treats a missing FDEF as a successful no-op for these instructions, and Rust already matched that behavior; the row makes the public path visible without changing glyph output, adding fonts, or increasing concrete cases. Exact Rust FFI, C ABI, and WASM ABI parity passes with 6,696 / 6,696 runtime rows and four explicit pending rows. Refreshed condition coverage is 15,823 / 17,764 lines, 22,712 / 25,453 regions, and 3,769 / 4,524 branches; `tt/hinter/exec.rs` moves to 1,298 / 1,340 lines, 2,678 / 2,901 regions, and 355 / 410 branches |
| 2026-07-15 | Latin serif intermediate-overlap branch row | `script-coverage.ttf` now includes U+0244, a compact serifed Latin stem with an intermediate vertical edge in the serif/base span. One explicit `FT_LOAD_FORCE_AUTOHINT` public row selects it, proving the FreeType Latin Phase 4 serif-overlap break route (`aflatin.c:4733-4813`) through exact Rust FFI, C ABI, and WASM ABI parity. A rejected capital-accent candidate did not increase `src/autohint/latin.rs` branch coverage and was not kept. Concrete cases rise from 6,972 to 6,973 with zero implicit rows; runtime comparison rises from 6,969 / 6,969 to 6,970 / 6,970 with the same three FTMM pending rows. Refreshed condition coverage moves `src/autohint/latin.rs` from 2,585 / 2,825 lines, 3,719 / 4,212 regions, and 1,062 / 1,286 branches to 2,586 / 2,825 lines, 3,720 / 4,212 regions, and 1,063 / 1,286 branches. Route audit real-parity rises from 3,723 to 3,724 |
| 2026-07-15 | Latin vertical-cusp target-mono parity fix | `script-coverage.ttf` U+0245 now has an explicit `FT_LOAD_FORCE_AUTOHINT | FT_LOAD_TARGET_MONO` public row at 17 ppem. The candidate exposed C `advance.x=704` versus Rust `640`; first divergence was Latin segment construction, where C's `aflatin.c:1812-1843` replacement path refreshes the saved `prev_*` merge buffers, keeping both horizontal cusp segments `dir=-2` and unlinked, while Rust kept stale buffers and linked them as a stem. The pure-Rust fix refreshes those buffers and the row proves exact Rust FFI, C ABI, and WASM ABI parity. Concrete cases are 7,018 with zero implicit rows; runtime comparison is 7,015 / 7,015 with the same three FTMM pending rows. Refreshed condition coverage is 18,812 / 20,606 lines, 26,996 / 29,635 regions, and 4,426 / 5,001 branches; `src/autohint/latin.rs` is 2,605 / 2,844 lines, 3,742 / 4,240 regions, and 1,070 / 1,286 branches. Route audit real-parity is 3,769 |
| 2026-07-15 | Latin disjoint top-accent overlap rejection | `script-coverage.ttf` now includes U+1E02, a compact two-contour Latin glyph whose top accent is horizontally disjoint from the base glyph. One explicit `FT_LOAD_FORCE_AUTOHINT` public row selects it, proving the vertical-separation overlap rejection branch through exact Rust FFI, C ABI, and WASM ABI parity. Four rejected target-mode candidates over existing Latin glyphs passed exact parity but did not move `src/autohint/latin.rs` branch coverage and were not kept. Concrete cases rise from 7,035 to 7,036 with zero implicit rows; runtime comparison rises from 7,032 / 7,032 to 7,033 / 7,033 with the same three FTMM pending rows. Refreshed condition coverage moves `src/autohint/latin.rs` from 2,605 / 2,844 lines, 3,744 / 4,240 regions, and 1,072 / 1,286 branches to 2,605 / 2,844 lines, 3,745 / 4,240 regions, and 1,075 / 1,286 branches. Route audit real-parity rises from 3,786 to 3,787 |
| 2026-07-15 | CFF Type2 residual public routes | `build_cff_fixtures.py` extends `pure-cff-cubic.otf` with three compact Type2 charstrings: no-`endchar` EOF after `hmoveto`, malformed `rlineto` after width has been consumed, and malformed `rrcurveto` after width has been consumed. Three explicit `FT_Load_Glyph.matrix_load` rows prove pinned C, Rust FFI, C ABI, WASM ABI, and safe `Face::load_glyph` all return exact public load errors. Full condition coverage passes with 7,028 concrete cases, 7,025 / 7,025 runtime rows, and the same three FTMM pending rows. `src/tt/cff.rs` moves from 432 / 501 lines and 72 / 76 branches to 432 / 501 lines and 75 / 76 branches. Route audit real-parity rises from 3,776 to 3,779. The remaining CFF branch is `close_contour`'s empty-contour arm; real Type2 routes in this decoder always push a point when starting a contour, so leave it visible unless parser semantics change. |
| 2026-07-15 | Outline get-bitmap public route execution | `ftoutln.FT_Outline_Get_Bitmap` no longer uses the generic fallback rows. The unified oracle now calls pinned C `FT_Outline_Get_Bitmap` for gray/LCD/LCD_V target selection, mono default-raster target output, and null/delegated error scenarios; Rust FFI, C ABI, and WASM ABI call thin public wrappers over the safe Rust outline bitmap path. The mono target exposed the important public behavior that `FT_Outline_Get_Bitmap` leaves AA unset and writes packed 1bpp rows into the caller pitch. Focused parity passes 3 / 3 exact comparisons, and full `real-parity-verify` passes with runtime parity 7,033 / 7,033 plus the same three FTMM pending rows. `src/grays.rs` branch coverage is unchanged at 177 / 178 = 99.44%. Route audit real-parity rises from 3,787 to 3,790; generic-fallback drops 856 -> 854 and generic-error-fallback drops 131 -> 130. The seven `ftimage.*` rows that share `ftoutln.outline_get_bitmap` remain fallback until their independent bitmap/dropout/invalid-target shapes are routed. |
| 2026-07-16 | Latin non-base combining-mark route | `script-coverage.ttf` now includes U+0303, a zero-advance compact Latin combining tilde selected by one explicit `FT_LOAD_FORCE_AUTOHINT` public row. The row proves the public LATN non-base path that skips blue-edge assignment in `af_latin_hints_apply` through exact Rust FFI, C ABI, and WASM ABI parity. It exposed a Rust metrics-only divergence for zero-advance autohinted marks: pinned C translates the hinted outline by `-loader->pp1.x` before slot metric bbox calculation (`afloader.c:522-532`), while Rust left the metrics bbox at the pre-translation x origin. Core now applies that translation for autohinted zero-advance glyph metrics. Concrete cases rise from 7,036 to 7,037 with zero implicit rows; runtime comparison rises from 7,033 / 7,033 to 7,034 / 7,034 with the same three FTMM pending rows. Refreshed condition coverage moves `src/autohint/latin.rs` from 2,605 / 2,844 lines, 3,745 / 4,240 regions, and 1,075 / 1,286 branches to 2,607 / 2,844 lines, 3,748 / 4,240 regions, and 1,076 / 1,286 branches. Route audit real-parity rises from 3,790 to 3,791 |
| 2026-07-16 | TrueType VM execution-too-long route | `hinter-execution-too-long-loop.ttf` is generated by `build_hinter_edge_fixtures.py` from the compact hinter control matrix with gid 1 replaced by a negative `JMPR` loop. One explicit `FT_Load_Glyph.matrix_load` row proves the public TrueType interpreter guard through exact Rust FFI, C ABI, and WASM ABI parity. The pure-Rust VM guard now matches C `TT_RunIns` / `TT_CONFIG_OPTION_MAX_RUNNABLE_OPCODES` by returning `FT_Err_Execution_Too_Long` instead of generic invalid outline. Focused parity passes 1 / 1. Full condition coverage passes with 7,036 / 7,036 runtime rows and the same three FTMM pending rows; `src/tt/hinter/exec.rs` moves from 1,351 / 1,379 lines, 2,741 / 2,946 regions, and 381 / 416 branches to 1,352 / 1,379 lines, 2,742 / 2,946 regions, and 382 / 416 branches. Route audit real-parity rises from 3,792 to 3,793. |
| 2026-07-16 | Bitmap copy public route execution | `ftbitmap.FT_Bitmap_Copy` no longer uses generic fallback rows. The unified oracle now calls pinned C `FT_Bitmap_Copy` for deep-copy, source-target alias no-op, null source buffer, opposite pitch-flow, null-source error, and target-buffer replacement scenarios. Rust FFI mirrors `src/base/ftbitmap.c:63-116` with safe owned-buffer tracking; C ABI and WASM ABI wrappers only marshal records and caller bytes into that core route. Focused `make -C pillow-rs-freetype test-case CASE=ftbitmap.FT_Bitmap_Copy` passes 6 / 6 exact Rust FFI, C ABI, and WASM ABI comparisons. Full condition coverage keeps runtime parity at 7,036 / 7,036 with the same three FTMM pending rows; `src/ffi/handles.rs` covered branches move from 357 / 395 to 381 / 431. Route audit real-parity rises from 3,794 to 3,800 and generic-fallback drops from 854 to 848. The remaining ftbitmap rows are `bitmap_convert`, `bitmap_done`, `bitmap_embolden`, `bitmap_blend`, and `glyphslot_own_bitmap`. |
| 2026-07-16 | Bitmap convert/done public route execution | `ftbitmap.FT_Bitmap_Convert` and `ftbitmap.FT_Bitmap_Done` no longer use generic fallback rows. The unified oracle now calls pinned C `FT_Bitmap_Convert` for MONO/GRAY2/GRAY4/GRAY/LCD/LCD_V/BGRA conversion, alignment and pitch-flow behavior, repeated target reallocation, empty null-buffer conversion, null arguments, and unsupported pixel modes; `FT_Bitmap_Done` covers allocated, empty, repeated lifecycle, null-library, and null-bitmap behavior. Rust FFI mirrors `src/base/ftbitmap.c:493-715` and `src/base/ftbitmap.c:1109-1125` with safe owned-buffer tracking; C ABI and WASM ABI wrappers only marshal public bitmap records and caller bytes. The route compares exact public fields and deterministic active grayscale bytes. It intentionally does not claim exact parity for alignment padding bytes because FreeType allocates those rows with `FT_QALLOC` and leaves padding uninitialized. Focused `make -C pillow-rs-freetype test-case CASE=FT_Bitmap_Convert` passes 6 / 6 and `make -C pillow-rs-freetype test-case CASE=FT_Bitmap_Done` passes 4 / 4 exact Rust FFI, C ABI, and WASM ABI comparisons. Full `make -C pillow-rs-freetype real-parity-verify` passes with runtime parity 7,037 / 7,037 and the same three FTMM pending rows. `src/ffi/handles.rs` branch coverage is 437 / 505 = 86.53%. Route audit real-parity rises from 3,801 to 3,811 and generic-fallback drops from 848 to 838 at the worker baseline. |
| 2026-07-16 | Bitmap embolden public route execution | `ftbitmap.FT_Bitmap_Embolden` no longer uses generic fallback rows. The unified oracle now calls pinned C `FT_Bitmap_Embolden` for packed GRAY2/GRAY4 conversion, MONO/GRAY/LCD/LCD_V/BGRA modes, positive and negative pitch flow, strength rounding, null-library/null-bitmap/null-buffer errors, unsupported pixel mode, negative strength, and buffer reallocation. Rust FFI mirrors `src/base/ftbitmap.c:135-412` with safe owned-buffer tracking; C ABI and WASM ABI wrappers only marshal records and caller bytes into the core route. The misleading placeholder `error_mono_strength_limit` now proves C's actual behavior: MONO x strength is clamped to 8 pixels, and the byte embolden loop relies on C integer promotion so `tmp >> 8` contributes zero. Focused `make -C pillow-rs-freetype test-case CASE=FT_Bitmap_Embolden` passes 6 / 6 exact Rust FFI, C ABI, and WASM ABI comparisons. Full condition coverage keeps runtime parity at 7,037 / 7,037 with the same three FTMM pending rows; `src/ffi/handles.rs` covered branches move from 381 / 431 to 459 / 571. Route audit real-parity rises from 3,801 to 3,807 and generic-fallback drops from 848 to 842. The remaining ftbitmap rows are `bitmap_convert`, `bitmap_done`, `bitmap_blend`, and `glyphslot_own_bitmap`. |
| 2026-07-16 | Scaler request-size preload route | `FT_Load_Glyph.matrix_load` now includes `dejavu-a-default-preload-request-scales`, which opens DejaVuSans, applies public `FT_Request_Size(FT_SIZE_REQUEST_TYPE_SCALES, 40960, 40960)`, then loads glyph 36 through exact C oracle, Rust FFI, C ABI, and WASM parity. The unified oracle and runtime face caches now serialize `preload_request_size` explicitly, so the C oracle does not silently fall back to the equivalent 20px pixel-size setup. The row covers the active-size side of `ScaleMetrics::from_font_data`; `src/scaler.rs` branch coverage moves from 200 / 218 to 201 / 218. Full `make -C pillow-rs-freetype real-parity-verify` passes with runtime parity 7,044 / 7,044, four explicit pending rows, and route audit real-parity rising from 3,838 to 3,839. |
| 2026-07-16 | Scaler fpgm-without-cvt default load route | `build_render_fixtures.py` now generates `render-fpgm-no-cvt.ttf` from the compact render topology font by adding an empty `fpgm` table while leaving `cvt` absent. One explicit `FT_Load_Glyph.matrix_load` row default-loads glyph 3 and proves exact C oracle, Rust FFI, C ABI, and WASM parity for the TrueType fallback where `fpgm` is present but native bytecode still cannot execute because `cvt` is absent. Focused `make -C pillow-rs-freetype test-case CASE=render-fpgm-no-cvt-default` passes 1 / 1. Full condition coverage passes with runtime parity 7,045 / 7,045 and the same four pending rows; `src/scaler.rs` branch coverage moves from 201 / 218 to 202 / 218. Route audit real-parity rises from 3,839 to 3,840. |
| 2026-07-16 | TrueType INSTCTRL and phantom-rounding parity | The source-backed `hinter-control-matrix.ttf` now encodes `INSTCTRL` operands in FreeType's value-then-selector stack order in both prep and glyph ranges. Its glyph-range selector-3 waiver copies rounded pp2 into every real point; the glyph has a nonzero left phantom, and a new 19 ppem `FT_Load_Glyph.matrix_load@hinter-instruction-control-phantom-rounding` row makes the non-grid phantom value, saveback, and final pp1 translation observable through public metrics. The first diagnostic exposed two independent Rust divergences: `exec.rs` popped selector/value in reverse order, and `hint_glyph` skipped pre-program phantom rounding while v40 compatibility was active. C `Ins_INSTCTRL` reads selector from `args[1]` and value from `args[0]` (`ttinterp.c:4678-4734`); `TT_Hint_Glyph` always rounds pp1-pp4 before glyph bytecode and only uses compatibility to decide whether current phantoms are copied back (`ttgload.c:812-857`). Core now follows both rules and uses a saved current pp1 for final simple-outline translation after a per-glyph waiver. Full Coverage MCP condition parity passes with 7,080 / 7,080 runnable rows and the same four explicit pending rows; route audit real-parity rises from 3,874 to 3,875. `src/tt/hinter/exec.rs` remains at 1,352 / 1,379 lines and 382 / 416 branches, while `src/tt/hinter/mod.rs` moves from 286 / 287 lines and 68 / 72 branches to 283 / 284 lines and 67 / 70 branches, improving branch rate from 94.44% to 95.71% by removing the incorrect conditional-rounding path. |
| 2026-07-16 | Outline reverse and transform public routes | `FT_Outline_Reverse` and `FT_Outline_Transform` now execute as real public endpoints instead of generic fallback rows. `api.rs` owns safe contour-buffer reversal and 64-bit `FT_MulFix` coordinate transforms; Rust FFI, C ABI, and WASM wrappers only marshal public outline records. The six maintained rows cover two-contour point/tag reversal, repeat flag toggling, null reverse, mixed-sign matrix output, null outline/matrix/points transform no-ops, and post-reflection cbox/orientation. Pinned C source behavior comes from `ftoutln.c:545-600` and `ftoutln.c:695-734`. Route audit moves six rows from generic fallback to real parity, raising real-parity from 3,875 to 3,881 and reducing generic fallback from 817 to 811. |
| 2026-07-16 | PostScript name non-ASCII filtering route | `build_name_fixtures.py` now emits a static SFNT whose Windows nameID 6 contains a UTF-16BE code unit with a nonzero high byte and whose Apple fallback starts with a non-ASCII byte. Pinned `sfdriver.c:get_win_string` rejects the Windows code unit, then `get_apple_string` rejects the non-ASCII byte while retaining `ApplePS`; one exact `FT_Get_Postscript_Name.static_face_name_success` row proves the same result through Rust FFI, C ABI, and WASM ABI. Coverage MCP passes with 7,081 / 7,081 runtime rows and four unchanged pending rows. `src/tt/name.rs` condition coverage moves from 128 / 138 to 130 / 138, covering the false sides at the Windows high-byte and PostScript ASCII-range predicates without adding placeholder proof. |
| 2026-07-16 | Validated SFNT name selection and ASCII conversion parity | Pinned FreeType drops zero-length and out-of-range name records while loading the table (`ttload.c:984-1040`), then converts public face names with ASCII-oriented Windows and Apple decoders (`sfobjs.c:59-124`): odd UTF-16BE tails are ignored, NUL terminates the value, and non-ASCII units become `?`. Rust previously selected family/style records before validating their slices, rejected odd UTF-16BE lengths, decoded general Unicode, and continued Apple strings after NUL. `name.rs` now selects from validated records and matches those conversion rules. Three exact `FT_Get_Postscript_Name` rows cover an English Windows named-subfamily replacement with an odd tail and embedded NUL, an Apple named subfamily with non-ASCII and embedded NUL, and an all-invalid Apple static PostScript name returning null. Coverage MCP passes with 7,084 / 7,084 runtime rows and four unchanged pending rows; route audit real-parity rises from 3,882 to 3,885. Structural removal of the obsolete pre-validation decoder paths moves `src/tt/name.rs` from 333 / 333 lines, 130 / 138 branches, and 30 / 30 functions to 274 / 274 lines, 106 / 108 branches, and 23 / 23 functions, improving branch rate from 94.20% to 98.15%. The two remaining branch sides reject empty strings in APIs that accept manually constructed public Rust `NameTable` records; parsed public FreeType routes cannot reach them because record loading already drops zero-length strings, so those guards remain intentionally uncovered rather than being removed for coverage. |
| 2026-07-16 | Bounded `gasp` range allocation parity | Reading all residual `src/tt/gasp.rs` lines against C `tt_face_load_gasp` (`ttload.c:1456-1508`) showed that the only uncovered lines were Rust-only `checked_mul` and `checked_add` error closures around the range-array length. The source count is a `u16`, so `4 + numRanges * 4` is bounded to 262,144 bytes on the supported native and wasm32 targets; pinned C performs that arithmetic directly as `num_ranges * 4L`. Rust now does the same. Existing exact public `FT_Get_Gasp` rows still cover short headers, unsupported versions, truncated arrays, table-record length behavior, all range selections, and version-zero flag masking: the focused lane passes 13 / 13 C-oracle, Rust FFI, C ABI, and WASM comparisons, while full Coverage MCP parity remains 7,084 / 7,084 with four unchanged pending rows. `gasp.rs` moves from 45 / 47 lines, 8 / 8 branches, and 6 / 8 functions to 40 / 40 lines, 8 / 8 branches, and 6 / 6 functions; all 55 regions are covered. No route was added and no parser error observable through C was removed. |
| 2026-07-16 | Complete `fvar` header validation parity | Pinned `sfnt_init_face` (`sfobjs.c:605-658`) exposes GX variation support only when the complete `fvar` header has version `0x00010000`, axis size 20, a nonzero axis count no greater than `0x3FFE`, an instance count no greater than `0x7EFF`, an instance size exactly `4 + 4 * axes` or `6 + 4 * axes`, and table-bounded arrays. Rust previously checked only the major version, accepted axis records larger than 20 and arbitrary oversized instance records, and carried four checked-arithmetic closures instead of applying C's limits first. The parser now mirrors those limits; the resulting arithmetic is bounded as documented by `TT_Get_MM_Var` in `ttgxvar.c`. Five reproducible public `FT_FACE_FLAG_MULTIPLE_MASTERS` rows cover a nonzero minor version, oversized axis and instance records, and both C count limits. The focused lane passes 13 / 13 exact C-oracle, Rust FFI, C ABI, and WASM comparisons. Full Coverage MCP parity rises from 7,084 / 7,084 to 7,089 / 7,089 with four unchanged pending rows; route audit real-parity rises from 3,885 to 3,890. `src/tt/fvar.rs` moves from 91 / 98 lines, 14 / 14 branches, 2 / 6 functions, and 119 / 132 regions to 85 / 85 lines, 20 / 20 branches, 2 / 2 functions, and 118 / 118 regions. |
| 2026-07-17 | Format-14 remaining-length validation parity | Pinned `tt_cmap14_validate` (`ttcmap.c:3013-3131`) rejects malformed selector, default-range, and non-default-mapping arrays by dividing the bytes remaining in the declared subtable by record widths 11, 4, and 5 before iteration. Rust previously modeled those checks with six zero-hit checked-arithmetic error lines and private helper offset checks that their validated caller could never fail. The parser now uses the same remaining-length predicates and slice-based record iteration; private helpers rely on the caller's already-proved nonzero offset-below-length invariant, matching C's `table + defOff` / `table + nondefOff` sequence. The existing exact `FT_Get_Char_Index.matrix_char_code` lane passes 15 / 15 C-oracle, Rust FFI, C ABI, and WASM comparisons, including the malformed format-14 matrix. Full parity remains 7,089 / 7,089 with four unchanged pending rows and no route-count change. `src/tt/cmap.rs` moves from 726 / 740 lines, 164 / 164 branches, 56 / 59 functions, and 957 / 971 regions to 711 / 711 lines, 164 / 164 branches, 53 / 53 functions, and 934 / 936 regions. The two residual LLVM regions have no uncovered source line or branch side. |
| 2026-07-17 | Zero-contour glyph and IUP contour-state parity | Pinned `TT_Load_Simple_Glyph` starts its last endpoint at -1, so a valid zero-contour `glyf` record has zero points; Rust previously defaulted the absent endpoint to zero and attempted to decode one point, returning error 20. A reproducible derived font retains a raw zero-contour record with an instruction body, and one exact `FT_Load_Glyph.matrix_load@hinter-empty-glyph-iup-phantoms` row now passes through the C oracle, Rust FFI, C ABI, and WASM ABI. Reading the downstream C path also proved that `TT_Load_Glyph` shortcuts empty glyphs before parsing instructions and that `Ins_IUP` returns for `n_contours == 0`; Rust's synthetic fallback that treated four phantom points as one contour was therefore removed. Full Coverage MCP parity rises from 7,089 / 7,089 to 7,090 / 7,090 with four unchanged pending rows, and route-audit real parity rises from 3,890 to 3,891. `src/tt/glyf.rs` remains fully covered at 531 / 531 lines and 90 / 90 branches. `src/tt/hinter/iup.rs` moves from 99 / 102 lines, 53 / 60 branches, and 202 / 206 regions to 98 / 100 lines, 52 / 58 branches, and 200 / 202 regions, improving branch rate from 88.33% to 89.66%; the residual six branch sides are malformed contour/array guards, not public fixture candidates. |
| 2026-07-17 | SFNT glyph-name gate and service layering | Pinned `FT_Get_Glyph_Name` and `FT_Get_Name_Index` in `ftobjs.c` apply `FT_HAS_GLYPH_NAMES` before glyph-dictionary service dispatch; `sfobjs.c` sets that SFNT face flag only after a successful `post` load and excludes format 3.0, while `tt_face_get_ps_name` in `ttpost.c` separately initializes its service result to `.notdef`. Rust previously mixed these two layers inside `PostTable::glyph_name` and duplicated the face-flag check in the FFI wrapper. `Font::glyph_name` and `Font::name_index` now own public format availability, `PostTable` retains service semantics, and the thin FFI wrappers delegate availability to core. Existing exact lanes pass 23 / 23 `FT_Get_Glyph_Name` and 9 / 9 `FT_Get_Name_Index` comparisons through C oracle, Rust FFI, C ABI, and WASM ABI. Full Coverage MCP parity remains 7,090 / 7,090 with four unchanged pending rows and no route-count change. `src/tt/post.rs` moves from 95 / 98 lines, 21 / 22 branches, and 184 / 192 regions to 96 / 97 lines, 23 / 24 branches, and 184 / 190 regions. Its only remaining miss is the service's invalid-index guard, which the public API must preempt to return `FT_Err_Invalid_Glyph_Index`. |
| 2026-07-17 | Full-range `FT_Short` glyph-metric narrowing parity | Reading all residual `src/casts.rs` branches against pinned `ftgrays.c`, `aflatin.c`, and `ttgload.c` exposed one invalid Rust assumption rather than a missing boundary row: a valid TrueType outline can span the complete signed 16-bit coordinate range, so its derived height is 65,535 even though every stored point fits `FT_Short`. The new reproducible `script-coverage.ttf` glyph splits each edge into encodable deltas and exact `FT_Load_Glyph.matrix_load@latin-extreme-coordinate-force-autohint` parity first panicked at Rust's `i16_from_i32` range assertion. Removing only that assertion exposed the observable second divergence: C narrows the no-`vmtx` TrueType bbox height through `FT_Short` in `compute_glyph_metrics` and returns `vertBearingY=640`, while Rust retained the wide height and returned `-43008`. Core now performs the same two's-complement narrowing before synthesizing TrueType vertical metrics while preserving wide CFF behavior. Focused parity passes 1 / 1 through C oracle, Rust FFI, C ABI, and WASM ABI; full Coverage MCP parity rises from 7,090 / 7,090 to 7,091 / 7,091 with four unchanged pending rows, and route-audit real parity rises from 3,891 to 3,892. `src/casts.rs` moves from 51 / 51 lines, 6 / 12 branches, 14 / 14 functions, and 65 / 65 regions to 50 / 50 lines, 4 / 8 branches, 14 / 14 functions, and 62 / 62 regions; the four remaining branch outcomes are caller-invariant debug-assert failures documented in the no-route addendum. `src/scaler.rs` moves from 1,211 / 1,275 lines, 190 / 200 branches, and 1,324 / 1,383 regions to 1,214 / 1,278 lines, 192 / 202 branches, and 1,331 / 1,390 regions. |
| 2026-07-17 | TrueType invalid zone-reference parity | Reading all of `tt/hinter/zone.rs` and the pinned `Ins_SHP` / `Ins_UTP` implementations (`ttinterp.c:5001-5047,6013-6045`) showed that the two safe bounds-check misses are observable public VM behavior: out-of-zone points are ignored in normal mode but return `FT_Err_Invalid_Reference` under `FT_LOAD_PEDANTIC`. Rust already produced the normal no-op through safe `GlyphZone` writes, but incorrectly suppressed the pedantic error and had no exact error category for it. The source-backed `hinter-control-matrix.ttf` branch program now includes an invalid twilight-zone SHP followed by invalid glyph-zone UTP; a dedicated glyph isolates UTP for the second pedantic row. Core checks each opcode's active zone count, preserves the safe non-pedantic no-op, and maps `FontError::InvalidReference` through the thin FFI layers. Full Coverage MCP parity rises from 7,091 / 7,091 to 7,093 / 7,093 with four unchanged pending rows; route-audit real parity rises from 3,892 to 3,894. `src/tt/hinter/zone.rs` moves from 37 / 37 lines, 10 / 12 branches, and 45 / 47 regions to complete 37 / 37 lines, 12 / 12 branches, 8 / 8 functions, and 47 / 47 regions. The new `exec.rs` checks add 12 covered lines, 12 covered branches, and 16 covered regions with no file-level regression. |
| 2026-07-17 | Autohint loader C-contract cleanup and low-UPEM route | Reading every residual `src/autohint/loader.rs` line against pinned `af_glyph_hints_reload` (`afhints.c:960-1015,1089-1192`) proved that Rust's missing-raw-point and missing-metrics fallbacks were not public FreeType states: C consumes one `FT_Outline` point count and unconditionally reads UPEM from installed style metrics. All three Rust callers construct scaled points one-for-one and install metrics before reload, so the internal API now receives mandatory UPEM and indexes the paired raw/scaled arrays under that contract. The audit also found Rust's non-C `.max(1)` near-limit clamp. A generated 64-UPEM Latin font with an adjacent duplicate point and one exact `FT_LOAD_FORCE_AUTOHINT` row proves the legal zero-threshold path through C oracle, Rust FFI, C ABI, and WASM ABI. Full Coverage MCP parity rises from 7,093 / 7,093 to 7,094 / 7,094 with four unchanged pending rows; `src/autohint/loader.rs` moves from 226 / 227 lines, 66 / 68 branches, and 406 / 408 regions to complete 225 / 225 lines, 64 / 64 branches, 5 / 5 functions, and 402 / 402 regions. |
| 2026-07-17 | TrueType IUP validated-zone contract | Reading every residual `src/tt/hinter/iup.rs` branch against `TT_Load_Simple_Glyph` and `Ins_IUP` (`ttgload.c:371-381`, `ttinterp.c:6048-6285`) showed that one branch modeled a duplicate/non-advancing contour endpoint and five modeled truncated internal zone arrays. Rust's simple-glyph parser was missing C's strict endpoint-order check, while the live `GlyphZone` constructor already keeps all coordinate/tag arrays aligned. A generated two-contour duplicate-endpoint font and one exact `FT_Load_Glyph` error row now prove C-oracle, Rust FFI, C ABI, and WASM ABI parity. The parser rejects the malformed state at the public boundary, `iup` is crate-private, and its live loop relies on the validated zone contract while preserving C's legal adjacent-touch empty-interval return. Full Coverage MCP parity rises from 7,094 / 7,094 to 7,095 / 7,095 with four unchanged pending rows; route-audit real parity rises from 3,895 to 3,896. `src/tt/glyf.rs` remains complete at 539 / 539 lines, 92 / 92 branches, 19 / 19 functions, and 677 / 677 regions; `src/tt/hinter/iup.rs` becomes complete at 93 / 93 lines, 46 / 46 branches, 5 / 5 functions, and 178 / 178 regions. |
| 2026-07-17 | Typed TrueType rounding-state dispatch | Reading the sole `src/tt/hinter/gs.rs` miss against pinned `Ins_RTG`, `Ins_RTHG`, `Ins_RTDG`, `Ins_ROFF`, `Ins_RUTG`, `Ins_RDTG`, `Ins_SROUND`, and `Ins_S45ROUND` (`ttinterp.c:4194-4306`) showed that C assigns fixed `TT_Round_*` enum constants directly. Rust routed those same constants through a public numeric conversion helper with an impossible invalid-value fallback. The opcode dispatch now assigns typed `RoundMode` variants directly and removes that non-FreeType helper. Existing exact `hinter-stack-state-matrix` and `hinter-super-round-matrix` public rows continue to prove all eight valid states through C oracle, Rust FFI, C ABI, and WASM ABI; runtime parity remains 7,095 / 7,095 with four unchanged pending rows and no route-count change. `src/tt/hinter/gs.rs` becomes complete at 174 / 174 lines, 28 / 28 branches, 20 / 20 functions, and 190 / 190 regions. |
| 2026-07-17 | TrueType prepared-context ownership contract | Reading all three residual `src/tt/hinter/mod.rs` branches against pinned `TT_Hint_Glyph` and `TT_Process_Composite_Glyph` (`ttgload.c:770-865,1180-1270`) showed that C receives size execution state prepared before glyph loading and never runs executable glyph instructions for an empty outline. Rust instead exposed `hint_glyph` publicly, let it run `fpgm`/`prep` as a fallback, prepared a second context inside direct composite scaling with drifted `tt_scale`/`point_size`, and guarded first-contour/tag writes for impossible empty vectors. The scaler now owns one reusable prepared context for both top-level and component loads, `Font` and direct scaler paths share one active-size constructor, and crate-private `hint_glyph` consumes the same non-empty prepared-state contract as C. Existing exact public native-hinter rows keep runtime parity at 7,095 / 7,095 with four unchanged pending rows and no route-count change. `src/tt/hinter/mod.rs` moves from 283 / 284 lines, 67 / 70 branches, and 415 / 421 regions to complete 281 / 281 lines, 64 / 64 branches, 11 / 11 functions, and 411 / 411 regions. The caller consolidation also moves `src/scaler.rs` from 1,215 / 1,278 lines, 192 / 202 branches, 58 / 62 functions, and 1,332 / 1,390 regions to 1,242 / 1,296 lines, 196 / 206 branches, 59 / 63 functions, and 1,346 / 1,386 regions. |
| 2026-07-17 | Format-1 name language-tag compaction parity | Re-reading `tt_face_load_name` (`ttload.c:937-1040`) corrected the earlier malformed-language-tag classification: an out-of-range language-tag string does not fail face creation. C retains the tag slot with `stringLength=0`, exposes it through `FT_Get_Sfnt_LangTag` as `NULL` plus zero length, and compacts away name records that reference that tag or an index outside `numLangTagRecords`. Rust now follows that load-time contract, including rejecting strings that point before the format-1 storage area. One generated format-1 font carries a valid record plus records referencing an empty and a missing tag; a second sets `storageOffset` to the table header so both name and tag strings fall before storage. Five exact public rows cover both empty-tag causes, name count, and indexed name compaction through C oracle, Rust FFI, C ABI, and WASM ABI. The selector helpers are crate-private and no longer recheck empty strings already excluded by the loader, removing two post-loader-impossible branch sides. Full Coverage MCP parity rises from 7,095 / 7,095 to 7,100 / 7,100 with four unchanged pending rows; route-audit real parity rises from 3,896 to 3,901. `src/tt/name.rs` moves from 274 / 274 lines, 106 / 108 branches, and 23 / 23 functions to complete 292 / 292 lines, 116 / 116 branches, and 24 / 24 functions. |
| 2026-07-17 | TrueType VM invalid-reference and compatibility matrix | Pinned `Ins_RS`, `Ins_WS`, `Ins_WCVTP`, `Ins_WCVTF`, `Ins_RCVT`, and `Ins_DELTAC` (`ttinterp.c:2739-2870,6395-6464`) ignore an out-of-range storage/CVT reference in normal mode but return `FT_Err_Invalid_Reference` in pedantic mode. `Ins_INSTCTRL` (`ttinterp.c:4678-4730`) applies the same split to invalid selectors, nonzero values that do not equal the selector flag, and selectors outside their permitted program range. Rust previously suppressed the pedantic errors for all of these cases; DELTAC's local bounds guard also made its ignored `set_cvt` result in-bounds while incorrectly skipping C's preceding pedantic error. A generated compact font isolates each reference class plus SHP, and 20 exact public `FT_Load_Glyph` rows prove normal/pedantic and LCD/LCD_V behavior through C oracle, Rust FFI, C ABI, and WASM ABI. SHP has a dedicated simple-glyph program so its proof does not depend on continuing past an earlier invalid DELTAC. The source-backed branch program also packs additional twilight, ISECT, SHPIX, DELTA, and post-IUP compatibility probes without adding rows. C's `TT_DefRecord.active` state remains represented directly by the public Rust `DefRecord::active` field and guarded before function or instruction dispatch; Rust call records retain the definition slot identity corresponding to C's direct `TT_DefRecord*` and reread the mutable record on every LOOPCALL repeat. Rust now also retains C's separate initiating `iniRange`: CALL/ENDF change only the active range, while FDEF/IDEF legality and INSTCTRL continue to use the range that started fpgm, prep, or glyph execution. A broken-font LOOPCALL redefinition row plus normal/pedantic called-FDEF INSTCTRL rows prove both corrections through all four lanes. The public `ExecContext::fetch_byte` and `fetch_word` helpers remain visible pending the separate Rust API decision recorded above. Full Coverage MCP parity rises from 7,102 / 7,102 to 7,125 / 7,125 with four unchanged pending rows; route-audit real parity rises from 3,903 to 3,926. The final `src/tt/hinter/exec.rs` coverage denominator will be refreshed when this public-surface restoration is measured. |
| 2026-07-17 | TrueType VM code-range and composite compatibility routes | Pinned `SkipCode`, `Ins_IF`, and `Ins_ELSE` (`ttinterp.c:3070-3175`) return `FT_Err_Code_Overflow` when a push operand or skipped control-flow body crosses `codeSize`; Rust previously returned generic `Invalid_Outline` for direct operand fetches and accepted unterminated IF/ELSE scans. Core now preserves the distinct error through Rust, C ABI, and WASM mappings. Pinned `Ins_SCFS` (`ttinterp.c:4352-4379`) also proved the normal invalid-point no-op / pedantic `Invalid_Reference` split missing in Rust. Seven generated fonts cover direct-fpgm INSTCTRL, variable-push FDEF scanner truncation, unterminated control flow, invalid twilight SCFS, and instructed-composite SHPIX/DELTAP compatibility. The exact public `FT_Load_Glyph` matrix has twelve rows: ten new routes plus the two existing truncated-push routes moved out of their older non-exact error matrix without duplicating their IDs. All twelve pass through the C oracle, Rust FFI, C ABI, and WASM ABI. Full Coverage MCP parity is 7,139 / 7,139 with six unchanged pending rows; concrete cases rise from 7,135 to 7,145 and route-audit real parity rises from 3,933 to 3,943. The current route classifier already credited asset-backed `load_glyph` errors as real parity before checking `compare_error_output`, so its generic-error count remains 129; the move corrects the proof without producing the otherwise expected two-row category transfer. `src/tt/hinter/exec.rs` moves from 1,398 / 1,425 lines and 430 / 448 branches to 1,406 / 1,433 lines and 445 / 454 branches. The public `fetch_byte`/`fetch_word`, empty-fpgm guard, inactive-definition guards, and mutable-FDEF presence check remain intact because they represent Rust API or defensive state rather than legal C opcode routes. |
| 2026-07-17 | Exact expected-error route ledger | The unified harness treats an `expect_error` row without `expectation.compare.compare_error_output=true` as satisfied by any Rust error and returns before comparing the pinned C status or ABI outputs. The route audit previously classified many such asset-backed rows by operation first, falsely crediting them as exact public parity. `check_public_api_inputs.py` now records `compare_error_output` per concrete row and classifies every non-pending expected-error row without exact comparison as `generic-error-fallback` before any real-route or null-validation classification. At 7,165 concrete cases this corrects the ledger from 3,963 to 3,681 real-parity rows, from 9 to 4 real-null-validation rows, and from 129 to 524 generic-error fallbacks; 108 prior generic fallbacks are also reclassified into the more precise error bucket. Runtime parity behavior is unchanged. Future error-route work must enable exact comparison and prove the pinned C status through Rust FFI, C ABI, and WASM ABI before the row can count as real parity. |
| 2026-07-17 | Exact-error and generated route-evidence guard | The unified harness no longer treats paired C/backend errors as real parity. A row that opts into `compare_error_output` must produce an error in every lane and match the exact status and structured output; a successful row fails if the C oracle errors even when Rust, C ABI, and WASM ABI return the same error. Runtime selection now consumes the generated route-audit category for every concrete case, so unresolved assets and lifecycle dependencies remain visible as `pending-route` instead of passing through generic modeled output. Re-auditing all 7,169 concrete rows exposed 39 rows previously credited as real: 11 malformed SBIT/outline routes now compare exact errors, one composite no-recurse row now inspects the loaded slot without an invalid render call, and 27 immediate unresolved routes became pending. The complete asset audit identifies 242 `pending-route` rows, seven `pending-core` rows, and 1,192 green placeholder-style rows; six core rows are runtime pending while the MVAR vertical-header row is audit-only, giving 248 runtime pending and 249 route-or-core pending. The stricter full Coverage MCP run `1ff4e349-80cc-4def-a3fc-76e3423237bc` passes 6,921 / 6,921 exact runtime comparisons with no failures; normalized snapshot `f9714647-1361-4e7e-a57f-47a6e5dc4aa8` records 18,873 / 20,721 lines, 27,003 / 30,270 regions, 4,494 / 5,343 branches, and 1,193 / 1,366 functions. During that audit, the isolated `hinter-glyph-code-overflow.ttf` fixture proved another implementation divergence: pinned `TT_Hint_Glyph` (`ttgload.c:828-837`) preserves the partially interpreted zone and suppresses `TT_Run_Context` errors unless `FT_LOAD_PEDANTIC`, while Rust propagated every glyph-program error. Core now follows C; four malformed programs produce exact `Code_Overflow` under pedantic loading and preserve exact glyph output under non-pedantic loading. |

### TrueType VM residuals after the invalid-reference matrix

| `exec.rs` line | Classification | Exact dependency |
|---:|---|---|
| 271-289 | Public Rust API only | `fetch_byte` and `fetch_word` are public Rust helpers but are not called by the VM or any FreeType C endpoint. Both program-selection outcomes and both functions require a separate Rust API contract decision, not a C parity fixture. |
| 298-299 | Caller-preempted | `prepare_context` skips `run_fpgm` when the font program is empty. Pinned `tt_size_run_fpgm` performs the equivalent check internally, and the public empty-`fpgm` font proves the outer route. |
| 477-478 | Caller invariant | `MD`, `MDRP`, `MIRP`, and `IP` use scaled `org` coordinates whenever any selected zone is twilight. `orus_in(zp=0)` is therefore not a legal opcode path. |
| 512-516 | Caller invariant | `set_org_in` is reached only by valid twilight `SCFS`; the new invalid normal/pedantic rows prove C returns before the write. Its glyph-zone and out-of-bounds arms are safe Rust defenses, not C opcode routes. |
| 539-540 | Short-circuit invariant | SHPIX computes `in_twilight` first and never consults point tags on a twilight route; non-twilight routes always read glyph tags. |
| 726-727, 750-751 | Rust defensive state | Pinned FDEF/IDEF creation sets `TT_DefRecord.active=TRUE` and no C opcode deactivates a record. The public Rust record retains the field, so inactive guards remain visible but cannot be driven by a font. |
| 1531-1534 | Rust ownership invariant | Pinned `Ins_ENDF` holds a direct `TT_DefRecord*`; a successful LOOPCALL repeat cannot lose its definition. Rust's `Option` check preserves safe ownership. The existing redefinition fixture proves the record is reread and mutable without requiring a missing-record state. |

| 2026-07-17 | No-HarfBuzz standard-character fallback parity | Pinned `af_latin_metrics_init_widths` and `af_cjk_metrics_init_widths` (`aflatin.c:95-138`, `afcjk.c:102-140`) iterate every character in `script_class->standard_charstring`; `af_shaper_get_cluster_nohb` (`afshaper.c:631-653`) accepts the first nonzero `FT_Get_Char_Index` result without checking style ownership. Rust previously carried complete fallback lists only for Latin, Latin sub/superscript, and Hani, then rejected mapped candidates whose `gstyles` entry belonged to another style. The new table exactly matches all 60 pinned script entries and 115 candidates while the public `standard_char_for_script` compatibility helper remains available with its original first-candidate/unknown-`'o'` behavior. A deterministic Arabic font omits U+0644 Lam but maps later U+062D Ha; its exact target-mono row first showed C `advance.x=576` versus old Rust `512` and now passes C oracle, Rust FFI, C ABI, and WASM ABI. Removing the ownership filter then exposed the existing Tibetan cross-style row at C `horiBearingX=64` versus Rust `128`: both implementations measured the borrowed Latin `o`, but pinned `af_cjk_metrics_scale_dim` (`afcjk.c:648-742`) leaves standard widths in font units with `cur/fit` zero while Rust incorrectly scaled them. Correcting that second C contract restores the row, and all 92 maintained `script-` rows pass exactly. Full Coverage MCP parity rises from 7,106 / 7,106 to 7,107 / 7,107 with four unchanged pending rows; concrete cases rise 7,110 -> 7,111, route-audit real parity rises 3,907 -> 3,908, and all other route buckets are unchanged. Bounded coverage moves `globals.rs` from 206 / 206 lines and 66 / 66 branches to 193 / 193 and 60 / 60 after removing the partial-list/filter path; `globals_data.rs` moves from 63 / 64 lines and 1 / 1 functions to 63 / 70 and 1 / 2 because the preserved compatibility wrapper and unknown-tag slice arm are intentionally not exercised by runtime fixtures; `cjk.rs` moves from 861 / 887 lines and 378 / 408 branches to 853 / 882 and 373 / 408 after removing the non-C scaled-width state. No synthetic helper-only row was added. |

### API Residual Public-Route Audit - 2026-07-17

Every missing line and branch side in `src/api.rs` at Coverage MCP snapshot
`51ff11fb-3a0f-4810-b9e7-a47972350634` was read against its public callers and
the pinned FreeType 2.14.3 implementation.  The baseline is 1,081 / 1,131
lines, 1,616 / 1,675 regions, 244 / 302 branches, and 97 / 97 functions.

| Rust lines | Pinned C path | Public-route decision |
|---|---|---|
| `500-501` | `ftobjs.c:975-1016`; `afglobal.c:444-462`; `ttgload.c:1442-1448` | Add the real out-of-range TARGET_LIGHT row: C returns `Invalid_Argument` from autohint metrics lookup, and C oracle, Rust FFI, C ABI, WASM ABI, and safe `Face::load_glyph` take that same path.  Reject the FORCE_AUTOHINT plus NO_AUTOHINT probe: C and Rust FFI correctly return `Invalid_Glyph_Index` from the TrueType driver, but safe `Face::load_glyph` returns `Invalid_Outline` because this base has no core invalid-glyph-index error category.  Keep that branch visible rather than remapping an existing error in the harness or crossing this `api.rs` bucket into the separately integrated error cleanup. |
| `603,613,616` | `ftobjs.c:1079-1084,1162-1177` | Retain.  These are render-after-load failure propagation paths.  Public font loading validates outlines before the slot reaches this safe renderer; unsupported synthetic slot states remain explicit pending work. |
| `810-811,857-873,949-954` | `ftobjs.c:1065-1114`; `ftsynth.c:106-177` | Retain.  Every successful public outline load constructs paired slot/render outline snapshots.  Missing one side is a private inconsistent `GlyphSlot`, not a C glyph-slot state. |
| `909-910,984-1118` | `ftbitmap.c:135-412` | Retain.  Public loaded bitmaps satisfy dimensions, pitch, buffer length, and packed-depth invariants before the private in-place weight helper runs.  Direct public `FT_Bitmap_Embolden` has a separate exact route in the FFI layer. |
| `1146-1238` | `ftbitmap.c:135-276,283-412` | Retain.  The error arms require malformed or overflowing private bitmap storage.  For valid tight MONO rows, byte-aligned `bit_last` equals `pitch * 8` and returns before `shift == 0`; a row that reaches `1238` would require caller-supplied excess pitch rather than a loaded glyph slot. |
| `1359-1360` | `ftobjs.c:1116-1147` | Retain.  The helper is called only after the public load path rejects an empty outline, so a missing cbox here is a caller-contract violation. |
| `1422-1443` | `ftoutln.c:917-1045`; `ftsynth.c:151-166` | Do not claim through FTSynth.  These guards model invalid direct-outline storage, while C FTSynth deliberately ignores `FT_Outline_EmboldenXY` errors.  The separate `FT_Outline_Embolden` and `FT_Outline_EmboldenXY` manifest rows remain generic fallback until complete direct public wrappers exist. |
| `1557-1598` | `ftoutln.c:1051-1117` | Retain.  Exact public orientation rows cover null, empty, valid, collapsed, oversized, and zero-area outlines.  The residuals require negative/mismatched counts or endpoints that cannot be represented by the validated owned outline snapshot without manufacturing invalid raw-pointer storage. |

No implementation guard is removed by this audit.  Candidates that pass parity
without proving one of the listed public outcomes are not retained.

The retained row passed the focused Coverage MCP run
`020d89e8-5107-4154-9cac-705e3fc30a4c` (4 / 4 exact comparisons) and the
definitive full run `ae3676a8-848a-4e1d-b94b-1d56cb967464` (7,103 / 7,103
runtime parity, four unchanged pending rows).  Route audit moves from 7,106 to
7,107 concrete cases and from 3,903 to 3,904 real-parity cases; generic fallback
remains 811, generic-error fallback remains 129, and pending-core remains 6.
The API-only report from that exact full-run artifact is Coverage MCP snapshot
`a4dd2495-d71d-4d29-ace4-00090679b9d5`: `src/api.rs` remains 1,081 / 1,131
lines, 1,616 / 1,675 regions, and 97 / 97 functions, while branches improve
from 244 / 302 to 245 / 302.  Line 501 is the sole newly covered branch side;
line 500 still has one uncovered side.

The definitive zero-hit source records are `613`, `616`, `811`, `859`, `870`,
`873`, `910`, `954`, `1017`, `1020`, `1023`, `1027-1030`, `1034`, `1037`,
`1048`, `1083`, `1100`, `1103`, `1106`, `1109`, `1112`, `1115`, `1118`,
`1151`, `1154`, `1157`, `1160`, `1163`, `1166`, `1176`, `1179`, `1182`,
`1220`, `1238`, `1360`, `1423`, `1426`, `1434`, `1437`, `1440`, `1443`,
`1558`, `1577`, `1580`, `1589`, `1592`, `1595`, and `1598`.  The exact
partial-branch records are `500`, `603`, `810`, `857`, `868`, `871`, `909`,
`949`, `984-985`, `988-989`, `1012`, `1019`, `1022`, `1026`, `1033`, `1036`,
`1079`, `1095`, `1102`, `1105`, `1108`, `1111`, `1114`, `1117`, `1146`,
`1153`, `1156`, `1159`, `1162`, `1165`, `1169`, `1175`, `1178`, `1181`,
`1216`, `1235`, `1359`, `1422`, `1425`, `1433`, `1436`, `1439`, `1442`,
`1557`, `1576`, `1579`, `1588`, `1591`, `1594`, and `1597`.

### API exact render-error refresh - 2026-07-17

The current bucket started from Coverage MCP run
`e69e354c-2520-4320-bd3f-5767ba41dac2` and normalized snapshot
`d7b55494-895b-4603-8bce-28730abe45a0`: `src/api.rs` was 1,075 / 1,125
lines, 1,605 / 1,664 regions, 240 / 296 branches, and 96 / 96 functions.
Every zero-hit line and missing branch outcome was reread against its Rust
caller and pinned FreeType 2.14.3 control flow before selecting routes.

The retained case is
`freetype.FT_Load_Glyph.error_out_of_range_null_face_or_invalid_flags.render_overlap_raster_overflow`.
Pinned `FT_Load_Glyph` calls `FT_Render_Glyph` when `FT_LOAD_RENDER` is set and
the loaded slot is neither bitmap nor composite, then returns that renderer
error (`ftobjs.c:1159-1178`).  The overlap renderer returns
`FT_Err_Raster_Overflow` when the scaled preset width exceeds `0x7FFF`
(`ftsmooth.c:511-515,621-637`).  Rust already propagated the corresponding
`render_loaded_outline` error through `?` at `api.rs:606`, but no exact public
load row reached it.  The older grouped load-error row accepted any Rust error.
The new standalone row sets case-level `compare_error_output=true` and compares
`status`, `error`, and `slot_snapshot`.  Focused Coverage MCP run
`1e5f8c3e-ecda-458f-bf51-6d3f4dd7ca84` passes 1 / 1; its C-oracle cache key
`65904b3ead61ebbb8a44a514f7d92afd2aa618de7a78a26b7b2e1dc332004e3f`
contains `status.kind=error` and `error_code=98`, independently proving that C
returned `FT_Err_Raster_Overflow` rather than `OK`.

An exact `FT_LOAD_RENDER` SBIT probe was also planned and focused run
`97f0fae1-7675-480b-b15a-bbccae6f4fa1` passed 1 / 1.  It tested C's
`slot->format == FT_GLYPH_FORMAT_BITMAP` render bypass at `ftobjs.c:1163-1178`,
but full condition coverage remained flat for `api.rs`.  The row was removed;
it neither moved this bucket nor retired a placeholder.

Expected-error rows that remain disqualified as `api.rs` proof are:

| Case | Safe API relevance | Exact-error blocker |
|---|---|---|
| `freetype.FT_LOAD_PEDANTIC.pedantic_error_behavior` | Sets `assert_api_load_glyph_agrees=true` | Its case-level comparison omits `compare_error_output=true`, so any Rust error can satisfy the old harness path. |
| `freetype.FT_Load_Glyph.error_out_of_range_null_face_or_invalid_flags` | The `target-light-out-of-range-autohinter-order` variant sets `assert_api_load_glyph_agrees=true` | The grouped parent comparison omits `compare_error_output=true`; this variant is not exact C-status proof. |
| `fterrdef.FT_Err_Raster_Overflow.raster_buffer_or_cell_overflow` | Exercises public `FT_Render_Glyph`, not safe `Face::load_glyph` | It also omits exact error-output comparison and cannot prove `api.rs:606`. |

The definitive full run is
`f44fb849-cb7e-48e8-a77f-1f438e0d88bf`, normalization run
`1fb9c5f3-16a2-404e-89ea-d678cda20853`, and snapshot
`3e084d87-64ff-46f3-9091-288bec050a3c`.  Runtime parity is 7,160 / 7,160
with the same six runtime pending rows.  `src/api.rs` moves to 1,076 / 1,125
lines and 1,606 / 1,664 regions; branches remain 240 / 296 and functions remain
96 / 96.  Line 606 is the sole newly covered source record, with four hits and
no regressions.

Corrected exact-error route audit run
`2d989730-fa10-4d3f-9e37-5fe38a157137` applies the already-verified classifier
from commit `4a774272` without including it in this bucket.  Corrected counts
move from 7,165 to 7,166 concrete cases and from 3,681 to 3,682 real-parity
cases.  The other relevant buckets stay at 701 generic fallback, 524 generic
error fallback, 7 pending core, and 4 real null validation.  The uncorrected
historical ledger was 3,963 real parity, 809 generic fallback, and 129 generic
error fallback and must not be used as proof.

| Remaining Rust lines | Pinned C path | Precise blocker |
|---|---|---|
| `596,609` | `ftobjs.c:1159-1178` | C bypasses rendering for a slot already in bitmap format.  The exact scalable SBIT probe passed but did not reach the safe API's absent-render-outline arm; a real supported bitmap-only safe `Face` load is required. |
| `803-804` | `ftobjs.c:4732-4993` | A valid outline slot always carries C's outline storage.  Rust's outline format with a missing owned render snapshot is an inconsistent private slot; unloaded/unsupported public slot states remain an explicit runtime dependency. |
| `850-866,942-947` | `ftobjs.c:1065-1156`; `ftsynth.c:106-177` | Public outline loading constructs paired slot and render snapshots.  A one-sided optional snapshot or recompute without slot outline is not a C glyph-slot state. |
| `902-903,977-1231` | `ftsynth.c:137-160`; `ftbitmap.c:135-438` | Valid loaded bitmaps have checked dimensions, pitch, and storage before the private helper.  Remaining conversion, length, checked-arithmetic, pitch, and allocation failures require malformed private storage or host-width/allocator overflow; C uses unchecked raw buffers and has no deterministic oracle for those states. |
| `1352-1353` | `ftobjs.c:1129-1178`; `ftsmooth.c:595-619` | The caller rejects an empty loaded outline before repositioning.  Missing point cbox is caller-inconsistent. |
| `1415-1436,1550-1591` | `ftoutln.c:911-1117`; `ftsynth.c:151-166` | Negative/mismatched contour counts and out-of-range endpoints require invalid owned outline arrays.  C validates fewer raw-array invariants, can read out of bounds, and FTSynth ignores direct embolden errors, so these cannot be deterministic public C-oracle routes. |

The final zero-hit records are `609`, `804`, `852`, `863`, `866`, `903`,
`947`, `1010`, `1013`, `1016`, `1020-1023`, `1027`, `1030`, `1041`, `1076`,
`1093`, `1096`, `1099`, `1102`, `1105`, `1108`, `1111`, `1144`, `1147`,
`1150`, `1153`, `1156`, `1159`, `1169`, `1172`, `1175`, `1213`, `1231`,
`1353`, `1416`, `1419`, `1427`, `1430`, `1433`, `1436`, `1551`, `1570`,
`1573`, `1582`, `1585`, `1588`, and `1591`.  The final partial-branch records
are `596`, `803`, `850`, `861`, `864`, `902`, `942`, `977-978`, `981-982`,
`1005`, `1012`, `1015`, `1019`, `1026`, `1029`, `1072`, `1088`, `1095`,
`1098`, `1101`, `1104`, `1107`, `1110`, `1139`, `1146`, `1149`, `1152`,
`1155`, `1158`, `1162`, `1168`, `1171`, `1174`, `1209`, `1228`, `1352`,
`1415`, `1418`, `1426`, `1429`, `1432`, `1435`, `1550`, `1569`, `1572`,
`1581`, `1584`, `1587`, and `1590` (56 uncovered branch outcomes total).

### Font.rs Residual Public-Route Audit - 2026-07-17

Coverage MCP snapshot `60cb8c8d-7f89-4a58-9752-c42b3bce4706`
reports `src/font.rs` at 2,294 / 2,356 lines, 3,115 / 3,266 regions,
278 / 326 branches, and 201 / 219 functions.  The corresponding full run
`90cf941a-0ec0-4d75-b87c-de842a7ef09a` passes 7,106 / 7,106 runtime
comparisons with four explicit pending rows.  The bounded zero-function audit
is Coverage MCP run `cff86f32-20a9-4abe-824b-ef207e2fe249`.

Every zero-hit source record and partial branch was read in context and against
the pinned FreeType 2.14.3 implementation and callers before selecting cases:

| Rust function or route | Zero-hit lines | Partial branch lines | Pinned C evidence and disposition |
|---|---|---|---|
| `type1_cleartext` | none | `86` | `type1/t1parse.c:68-238` distinguishes the first ASCII PFB segment from invalid segment tags. Add an exact invalid-first-segment `FT_New_Memory_Face` row. |
| `type1_bbox` | `246` | `246` | `psaux/psobjs.c` accepts both procedure and array delimiters for `/FontBBox`. Add a bracketed-bbox Type 1 face row. |
| `truetype_face_with_load_mode` | `679,715,720` | `675,714,719` | `ftobjs.c:1510-1578`, `ttobjs.c:724-743`, and `ttpload.c:63-179` define named-instance naming and missing `loca`/`glyf` construction errors. Retain the malformed-table guards; named-instance coverage remains blocked by complete variation behavior. |
| `name_index` | `941` | `940` | `ftobjs.c:4263-4285`, `sfobjs.c:1118-1121`, and `ttpost.c:410-473` gate the glyph dictionary service by accepted `post` formats. Add an exact unsupported-format zero result. |
| `glyph_slot_load_truetype_no_scale` | none | `1756` | `ttgload.c:1534-1560,1970-2086` makes a valid empty glyph have both zero contours and zero points. The opposite split state is parser-unreachable and remains visible. |
| `glyph_slot_load_cff_no_scale` | `1850` | `1848` | `cffgload.c:411-428,617-742` sets high precision only below 24 ppem, including no-scale loads. Add the exact 24-ppem false boundary. |
| `glyph_slot_load_no_recurse` | `1907` | none | `ttgload.c:1804-1813,2389-2645` returns no-recurse subglyph data only for composites. The remaining error propagation needs a public malformed composite that reaches the second load. |
| `getmask_single_glyph` | `2112,2115-2117,2138` | `2099,2111` | This is a high-level `fontdone::Font`/Pillow-style mask helper, not a public C FreeType route. Public rendering is covered through `FT_Load_Glyph`/`FT_Render_Glyph`; do not add an internal helper test. |
| default/native metric scaling | `2279,2289-2294,2347,2357,2403-2408` | `2288,2339,2395` | `ftobjs.c:905-1178` and `ttgload.c:2188-2645` define public native/autohint dispatch. Context errors require real bytecode failures. The 16,384 pathological fallback has no pinned C counterpart and must not gain a coverage-only font without a separate behavior audit. |
| `layout_advance` | `2504,2508` | `2503` | `ftadvanc.c:54-201` implements public `FT_Get_Advance(s)`, while this helper is convenience text layout that suppresses missing glyphs and errors. Keep it outside public FreeType coverage. |
| `slot_load_from_scaled` | `2605` | `2604,2616` | `ttgload.c:1970-2086` and `cffgload.c:617-742` define scaled horizontal and vertical metrics. Zero scale and vertical-layout sides remain reachable only through exact public load states. |
| `glyph_is_composite` | `2669` | `2668` | `ttgload.c` consumes a parser-validated glyph header; fewer than two bytes is a malformed-glyf guard that normally fails earlier. Retain it unless a public short-glyph row reaches this caller. |
| `synthesize_vertical_metrics` | `2826,2828,2831` | `2825,2826,2829,2833` | `ftobjs.c:3143-3166` supplies the exact negative/positive bearing and zero-advance algorithm. Future rows must expose these states through a public glyph load. |
| `vertical_advance_font_units` | `2864-2865` | `2862` | `ttgload.c:2017-2032` uses OS/2 typo metrics when present and hhea otherwise. Add a no-OS/2 no-scale vertical TrueType row. |
| pathological metric predicates | none | `2879-2884,2888-2889` | No matching predicate exists in pinned C. Keep all eight short-circuit sides visible pending a root-cause audit of the Rust fallback. |
| `default_unicode_charmap_index` | none | `2905,2915` | `ftobjs.c:1371-1448,1565-1576,3712-3731` scans the directory in reverse, first for UCS-4 Unicode maps and then for any Unicode map, including ISO platform maps. Rust scans forward by hard-coded format/platform buckets. Add exact UCS-4, Apple Unicode fallback, and ISO fallback-order fonts, then fix core selection. |
| `SizeMetrics::from_size_request` | `3079` | `3090` | `ftobjs.c:3239-3373` accepts zero nominal scales, then `truetype/ttdriver.c:349-413` calls `tt_size_reset`, whose `ttobjs.c:1238-1248` zero-ppem guard returns `Invalid_PPem` (151). Add nominal zero-by-zero to the exact public error matrix; retain the caller-preempted inner `Scales` arm. |
| `ft_div_fix_i64`, `ft_mul_div_i64` | `3216,3227` | `3215,3226` | `ftcalc.c:161-250` returns a saturated value for direct public zero divisors, while `FT_Request_Metrics` rejects zero face dimensions before calling it. These request helpers preserve explicit divide-by-zero errors; no public request can reach them after the C-equivalent guards. |
| `named_instance_postscript_name` | none | `3276` | `sfdriver.c:804-1064` falls back from invalid direct PostScript IDs to subfamily/coordinate construction. Additional coverage requires real named-instance variation support. |
| `fixed_16_16_to_short_decimal` | `3346,3361` | `3345,3351,3355,3357,3366` | `sfdriver.c:700-793` (`fixed2float`) defines truncation and rounding. Cover only through `FT_Get_Postscript_Name` named-instance rows after the variation route is real. |
| `face_metric_values` | none | `3405,3418` | `sfobjs.c:1330-1419` uses hhea when either ascender or descender is nonzero, then OS/2 typo when either typo field is nonzero. One-sided generated metric fonts are valid future exact rows. |

The 18 uncovered LLVM functions are compiler-generated symbols, grouped as:
two `glyph_is_composite` error closures (`2659,2667`), one
`SizeMetrics::from_char_size` closure (`2980`), one `Font::type1_face` closure
(`573`), three `load_sfnt_table(s)` closures (`1403,1405,1409`), five
`truetype_face_with_load_mode` closures (`638,641,647,652,657`), one
`type1_bbox` closure (`247`), and five `parse_type1_metadata` closures
(`96,98-100,102`).  They are instrumentation artifacts attached to the public
constructor/parser paths, not independent helper functions.

The selected exact case plan is: three `FT_Get_Char_Index` directory-order
fonts, covering UCS-4 preference plus Apple Unicode and ISO fallback order;
Type 1 array-bbox success and invalid-PFB-segment error rows, one
unsupported-`post` `FT_Get_Name_Index` row, CFF no-scale at 24 ppem, TrueType
no-OS/2 no-scale vertical metrics, and nominal zero-by-zero in the exact-error
request matrix to prove the public preemption behavior. Every row uses the existing
C-oracle/Rust-FFI/C-ABI/WASM-ABI comparison path.

### Descender-only face metric predicates

Pinned FreeType `sfnt_load_face` (`src/sfnt/sfobjs.c:1388-1410`) tests the
horizontal ascender and descender independently before consulting OS/2, then
tests `sTypoAscender` and `sTypoDescender` independently before falling back to
Windows metrics. The earlier public rows covered ascender-first and both-zero
outcomes but not the second true operand in either predicate.

`build_metric_fixtures.py` now emits `hhea-descender-only.ttf` and
`hhea-zero-typo-descender-only.ttf`. Their `FT_Size_Metrics` variants compare
the C oracle, Rust FFI, C ABI, and WASM ABI exactly. Runtime parity moves
7,138 -> 7,140 with six unchanged pending rows, and `font.rs` condition
coverage moves 285/324 -> 287/324 branches.

## `ffi/handles.rs` Real-Route Audit (2026-07-17)

Coverage MCP snapshot `a5c69a1b-6a26-431d-b8fb-b53d3e559b13` is the frozen
baseline for this bucket: 2,469 / 2,661 lines, 538 / 697 branches, and
228 / 245 functions.  The LLVM file summary has 159 unique missing branch
outcomes.  Its line projection contains 160 missing branch records because one
LLVM region is attributed to two source segments; the file summary is the
canonical count.

The verified result is Coverage MCP full run
`32e10913-e896-421c-94f6-0e866ecdcd45`, normalization run
`54e56d71-8195-4ade-a88e-33cd774347a1`, and snapshot
`2024b96c-1b91-4bbd-94ff-e1cccc37f804`.  Runtime parity is exact at
7,138 / 7,138 with six existing pending dependencies.  `handles.rs` moved to
2,492 / 2,655 lines, 561 / 693 branches, 231 / 245 functions, and
3,793 / 4,032 regions.  Against the frozen baseline this is +23 covered lines,
+23 covered branch outcomes, +3 covered functions, and +47 covered regions;
the four removed branch outcomes include the safe-core alias condition that
valid Rust references cannot express.  The remaining canonical branch miss
count is 132.  Strict no-runtime-FFI and API/ABI verification passed in MCP run
`55d4c49f-a09b-4366-95ee-4af87cd91bdf` with no stderr.  Route audit is
`real-parity=3942`, `generic-fallback=809`, and `pending-core=7`.

Every partial branch was read against FreeType 2.14.3.  The complete inventory
is classified below by owning function group; no generic fallback row is
accepted as proof.

| Rust function group | Pinned C path | Classification and route decision |
|---|---|---|
| `FT_Error_String` | `base/fterrors.c:26-44` | Build configuration.  The missing side is the build without `FT_CONFIG_OPTION_ERROR_STRINGS`, not a runtime public route. |
| Bitmap ownership registry (`bitmap_owned_bytes`, `bitmap_source_bytes`, `FT_Bitmap_Set_Owned_Buffer`, `FT_Bitmap_Owned_Buffer_Bytes`) | C uses raw `FT_Bitmap.buffer` pointers | Thin Rust ABI storage validation.  Missing absent, poisoned, truncated, and overflow sides cannot use C as a deterministic oracle because C has no buffer length and would read or write through the caller pointer. |
| `FT_Bitmap_Copy` | `base/ftbitmap.c:63-128` | Real null-library/source/target and both pitch-flow routes are exact.  The public alias no-op remains in the raw C/WASM wrappers; the equivalent safe-core alias test was removed because valid `&T` and `&mut T` cannot alias.  Remaining length/registry failures are thin validation. |
| `FT_Bitmap_Convert` and row unpackers | `base/ftbitmap.c:491-690` | Real positive/negative alignment, source flow, and target flow are exact.  Residual slice, pitch, multiplication, and packed-row errors require malformed or truncated storage that C does not bound-check. |
| `FT_Bitmap_Embolden`, `ft_bitmap_assure_buffer`, packed conversion | `base/ftbitmap.c:135-438` | Real zero, x-only, y-only, negative-x, negative-y, overflow, packed-depth, MONO, LCD, LCD_V, BGRA, and both-flow rows are exact.  The private assure helper's GRAY2/GRAY4 arms cannot be reached through C 2.14.3 because the public caller converts those modes to GRAY first.  Remaining checked arithmetic and slice guards are safe-Rust validation. |
| `FT_Bitmap_Blend` | `base/ftbitmap.c:762-1058` | Existing mode, offset, allocation, and flow rows are public parity.  Residual byte-range and undersized-buffer checks are thin validation; C's negative-target-pitch branches are empty `/* XXX */` blocks. |
| Outline bitmap/orientation/reverse validation | `base/ftoutln.c:545-690,1051-1117` | Valid, empty, null, orientation, and reverse routes are public.  Residual contour-array, endpoint, pitch, and write-range paths require malformed raw storage; C validates fewer of these and can access out of bounds. |
| Size handle registry (`sync_active_size_state`, `FT_Done_Size`, `FT_Activate_Size`, `FT_Select_Size`) | `base/ftobjs.c:3039-3079,3380-3428` | Valid/null lifecycle routes are exact.  Unknown, dangling, poisoned-registry, and dead-owner states are thin handle validation; C dereferences an invalid `FT_Size` and has no deterministic public result. |
| `FT_GlyphSlot_Own_Bitmap` | `base/ftbitmap.c:1084-1102` | Null slot and loaded non-bitmap slot are exact.  The sole pending row is allocator fault injection for the public deep-copy failure. |
| `FT_Get_Sfnt_LangTag`, `FT_Get_SubGlyph_Info`, `FT_Set_Named_Instance`, `FT_Sfnt_Table_Info` index conversions | corresponding public base/SFNT functions | `FT_UInt` to `usize` failure is impossible on supported 64-bit native and wasm32 targets.  Named-instance behavior remains explicit pending until Adobe MM, `FT_MM_Var`, and gvar/HVAR support exists.  The redundant second SFNT tag-option decision was simplified to match C's single count-mode branch. |
| SFNT parsers (`parse_tt_header`, `parse_tt_horiheader`, `parse_tt_vertheader`) | `sfnt/ttload.c` loaders | Defensive bridge parsing.  A face that exposes these tables has already passed the driver's minimum table-size checks; truncated inputs are rejected before these helpers. |
| Charmap metadata, `FT_Set_Charmap`, iteration | `base/ftobjs.c:3952-3995,4475-4530` | Foreign-charmap and normal iteration are public.  Registry poisoning and impossible null-record tails are thin validation.  Null `agindex` for first/next char is a remaining real public route and needs explicit runner output that omits the write. |
| `FT_Load_Glyph`, `FT_Get_Advance`, `advance_fast_path_supported` | `base/ftobjs.c:1079-1177`; `base/ftadvanc.c:26-141` | Independent transform operands and FAST_ONLY target-light are exact routes.  Residual probe/no-size lifecycle and oversized-glyph conversion paths remain either public lifecycle work or target-width-impossible conversions. |

Extra Rust helpers in this file are not FreeType C exports:

| Helper | Current owners | Removal plan |
|---|---|---|
| `FT_Bitmap_Set_Owned_Buffer` | core bitmap operations, thin C/WASM bitmap adapters, parity runner | Keep as a Rust ownership bridge.  If the Rust module surface is later cleaned, move it with the byte accessor into a clearly named bridge module consumed only by the ABI crates; do not export a C symbol. |
| `FT_Bitmap_Owned_Buffer_Bytes` | embolden/convert/blend core paths and parity result extraction | Same bridge-module move as above.  Deleting it now would force raw-pointer reads or duplicate ownership logic into thin wrappers. |
| Private registry/parser/handle helpers | `handles.rs` public implementations only | Keep private.  They model storage and validation that C implements with raw pointers or private static functions; they are not manifest endpoints and should not receive standalone green rows. |

## `scaler.rs` Residual Public-Route Audit (2026-07-17)

Coverage MCP baseline snapshot `5f19ac79-cde6-42dd-8a72-4d6d5d815a45`
reports `src/scaler.rs` at 1,242 / 1,296 lines, 195 / 206 branches,
59 / 63 functions, and 1,346 / 1,386 regions.  Every zero-hit line,
partial branch, and uncovered function was read against its Rust callers and
the pinned FreeType 2.14.3 `ttgload.c`, `ttobjs.c`, `ttdriver.c`, and
autohinter paths before selecting public routes.

The retained public coverage route is
`freetype.FT_Glyph_Metrics.metrics_reference_cases.hinter-large-default-pathological-metrics`.
It loads glyph 49 from the source-backed `hinter-control-matrix.ttf` at 40
ppem with `FT_LOAD_DEFAULT` and exactly compares C oracle, Rust FFI, C ABI,
and WASM ABI metrics.  This reaches
`scale_glyph_for_metrics_with_autohint_preserve_advance`; no synthetic
outline, impossible scaler state, or fallback expectation is involved.

The coherent exact-error group is
`hinter-invalid-shp-pedantic` and `hinter-invalid-utp-pedantic`.  Both existing
public `FT_Load_Glyph` rows now opt into exact error-output comparison.  Every
lane returns `FT_Err_Invalid_Reference`; glyph-slot output is not committed on
the failing first load, and all four lane outputs remain the same null error
output.  This moves the `load_glyph` route ledger from 114 generic-error
fallbacks and 429 real-parity rows to 112 and 431 respectively.  Pinned
`TT_Hint_Glyph` (`ttgload.c:828-837`) suppresses `TT_Run_Context` failures in
non-pedantic mode and returns an error only in pedantic mode.  Rust
`tt::hinter::hint_glyph` already owns that same policy, so the scaler's second
non-pedantic suppression arm was unreachable under its loader contract and is
removed; the remaining error propagation is documented at the call site.

A broader `FT_LOAD_PEDANTIC.pedantic_error_behavior` probe was rejected.  Its
first C/Rust divergence occurs while preparing bytecode state, before the
intended glyph-program/scaler route: C reports `Invalid_Reference` (134) from
the prep program, while Rust reports `Invalid_Outline` (20).  The probe was
removed instead of bulk-enabling exact comparison or remapping an error in the
harness.

Definitive Coverage MCP run `127182bb-de1c-4c68-949f-311bd737c495` passes
7,024 / 7,024 runtime comparisons and ingests snapshot
`12b0d97c-d3e4-4f42-9a47-18aaf3d630d6`.  `src/scaler.rs` moves to 1,260 /
1,292 lines, 194 / 204 branches, 60 / 63 functions, and 1,354 / 1,383
regions.  The uncovered totals fall from 54 to 32 lines, 11 to 10 branch
outcomes, four to three functions, and 40 to 29 regions.  The remaining
zero-hit source records are `784-785`, `896-897`, `912`, `929-931`, `935`,
`937-939`, `1567-1568`, `1570-1571`, `1616`, `1654`, and `1807`; the partial
branch records are `763`, `800`, `875`, `877`, `922`, `1615`, `1653`, and
`1806`.

The complete residual classification is:

| Baseline Rust lines / branches | Classification | Exact dependency |
|---|---|---|
| `396-423` | Resolved by exact public route | The pathological native-metrics row reaches the complete autohint-preserve-advance function through public glyph metrics. |
| `784-785` | Constructor invariant | Both `FontData` constructors install `self_arc` before any public scaler call; the fallback closure requires an incompletely constructed private object. |
| `896-897` | Debug logging only | This is the disabled side of `log_enabled!`, not observable FreeType behavior. |
| `912` | Loader-preempted | Public `load_glyph_outline` validates the composite tree first; the no-hint scaled helper cannot independently fail for a validated tree. |
| `922,929-939` | Prepared-context ownership | All public native-hinter callers prepare and pass the active size bytecode context.  The owned-context fallback requires bypassing the public loader contract. |
| `1143-1146` | Resolved by pinned C contract | `hint_glyph` already suppresses non-pedantic interpreter errors exactly where C does.  Its `Err` result is pedantic-only and must propagate. |
| `1569-1573` | Parser-preempted composite attachment | `tt::glyf` rejects invalid component point indices before scaler attachment; pinned `TT_Process_Composite_Component` returns `Invalid_Composite` at the equivalent boundary. |
| `1617-1623` | Validated outline invariant | The caller returns for empty outlines, and the parser proves contour endpoints and point storage consistent before bbox decomposition. |
| `1655-1656` | Parser-validated contour bounds | Public glyph parsing rejects contour endpoints outside the point array before the scaler consumes them. |
| `1808-1809` | Caller-preempted autohint outline | The autohint caller returns before constructing a zero-contour outline. |
| partial branches at `763,800,875,877` | Retained | The remaining boolean outcomes require either an inconsistent zero-contour/point split or target-mode combinations not yet proven by an exact public C route.  No field override or synthetic internal state is accepted. |

No defensive malformed-outline or composite guard is removed by this audit.
The remaining paths require a real public loader state or a separate
source-backed first-divergence proof; passing rows that do not move one of
these records are not retained.
### Render bitmap-SDF error retention and residual audit - 2026-07-17

Pinned FreeType's bitmap-SDF renderer accepts the SDF render mode, allocates a
temporary target, and then returns `FT_Err_Unimplemented_Feature` for GRAY2,
GRAY4, LCD, LCD_V, and BGRA sources (`sdf/ftbsdf.c:805-810`).  Its renderer
wrapper frees only that temporary target on failure and leaves the source slot
unchanged (`sdf/ftsdfrend.c:552-601`).  Rust previously returned
`FT_Err_Cannot_Render_Glyph` and the harness discarded the post-error slot, so
the first divergence was both error taxonomy and observable retention.

`ftimage.FT_Bitmap.sdf_unsupported_source_preserves_bitmap` now loads real
20-ppem GRAY2, GRAY4, and BGRA SBIT strikes, calls the public render endpoint,
and compares the exact error plus retained slot through the pinned C oracle,
Rust FFI, C ABI, and WASM ABI.  The existing render matrix also gains a real
MONO SBIT SDF success row.  No slot state is synthesized.  Coverage MCP run
`61842b3a-150c-4e96-a983-e49f363a36d9` and snapshot
`10459f6b-4ee1-4f8f-8c25-571cfd5e7df5` pass 7,027 / 7,027 runnable rows with
153 unchanged pending rows.  The route ledger moves from 7,176 to 7,180
concrete rows and from 3,597 to 3,601 real-parity rows; generic, generic-error,
null-error, and pending categories do not move.  `src/render.rs` moves from
2,325 / 2,459 lines, 419 / 490 branches, 144 / 158 functions, and
3,246 / 3,432 regions to 2,339 / 2,459 lines, 423 / 490 branches,
144 / 158 functions, and 3,274 / 3,432 regions.
The operation-specific `render_glyph` ledger is now 206 real-parity rows while
the existing nine generic-error rows, one null-error row, and one pending-core
unloaded/unsupported-slot row remain visible and unchanged.

The follow-up
`ftimage.FT_Bitmap.public_fields_match_render_output@m5-sbit-gray-opaque-neighborhood-sdf`
row adds glyph 2 to the maintained format-1 gray SBIT fixture as a fully
opaque 3x3 bitmap.  FreeType 2.14.3 `bsdf_is_edge`
(`sdf/ftbsdf.c:311-359`) classifies the center as non-edge only after all
eight neighbors have been visited and found nonzero; the Rust renderer follows
the same topology, so there is no output divergence to mask.  Coverage MCP run
`89b1e418-7d71-452b-8d8a-751f5bc1f408` passes 7,032 / 7,032 runnable rows
with 153 pending rows and ingests snapshot
`c138e0ad-b87e-4374-aed0-79d44025987b`.  The route ledger moves to 7,185
concrete rows and 3,608 real-parity rows, while fallback and pending categories
remain unchanged.  `src/render.rs` moves from 2,339 to 2,341 covered lines,
423 to 424 covered branch outcomes, and 3,274 to 3,276 covered regions;
covered functions remain 144 / 158.  The exact new records are lines 2697 and
2715 plus the previously missing branch outcome at line 2696.

Every residual line record and partial branch was read against its Rust caller
and pinned C before selecting the SBIT subgroup.  The LLVM file summary is
canonical: 118 missing instrumented lines, 66 missing branch outcomes,
14 missing functions, and 156 missing regions.  Coverage MCP's normalized
source projection exposes 80 distinct zero-hit source records and 49 partial
branch records; it does not expose stable LLVM function or region identities,
so the 14 functions and 156 regions are classified by their owning source
scope below rather than credited as separate endpoints.

| Residual source scope | Zero-hit lines | Partial-branch lines | Pinned C disposition / dependency |
|---|---|---|---|
| Pixel-mode metadata | `148-149` | none | GRAY2/GRAY4 SBITs are public, but the failing SDF route retains C's source `num_grays` instead of constructing a Rust output through `PixelMode::num_grays`.  A future exact non-SDF bitmap-output route must move these lines. |
| Empty/render dispatch | `237-238,305,309,579` | `362,578` | MONO and LCD match arms are preempted by earlier returns; normal empty rendering takes the smooth-render preset path.  The remaining `points nonempty && n_contours == 0` side is not a valid loaded C outline.  Main commit `5bd9d91a` independently owns the empty pre-rendered bitmap-SDF API route and must not be duplicated here. |
| Raster error propagation | `408,489,564,868,2906,2947` | none | Valid loaded outlines do not make the gray, SDF, mono, LCD, or LCD_V raster calls fail.  Exact coverage needs a public malformed/renderer-failure lifecycle or deterministic allocation failure, not a broad accepted error. |
| Mono setup and profile entry | `718,771,800,1109,1134,1165` | `716,770,799,1108,1164` | `ttgload.c:2566-2618` clears `FT_OUTLINE_SINGLE_PASS` and derives only dropout flags.  Zero dimensions/empty profiles and opposite-flow profile transitions need a public glyph topology that survives the loader and produces a measured delta. |
| Mono malformed decomposition | `1222,1234,1243-1244,1268,1278,1315,1332,1343-1346,1349` | `1221,1233,1271-1272,1274,1314,1329-1330,1339` | Out-of-order contour ends, invalid first cubic points, broken conic/cubic tag sequences, and missing contour points are rejected or normalized before `FT_Render_Glyph`.  They remain blocked by a real public malformed-outline route. |
| Mono sweep/dropout topology | `1435,1437,1534,1536,1603,1617,1630,2014,2028,2041,2046` | `1397,1432,1484,1523,1582,1584,1602,1616,1629,1809,2013,2016,2025,2040,2045` | These are genuine black-raster profile-link, clipping, neighbor, and dropout shapes.  Existing exact scan-type/topology rows cover the reachable common states; further rows require compact glyphs that demonstrate new condition coverage, not helper calls. |
| SDF malformed decomposition | `2083,2205,2216,2224-2225,2275,2293,2306-2307,2309,2312` | `2204,2215,2274,2290-2291,2300` | As in C's outline decomposition, invalid endpoints and curve tag sequences require a malformed public outline after loading.  Font loading preempts these states. |
| SDF vector geometry | `2429` | `2428,2533` | Real outline geometry can still cover the remaining distance/cross-product sides.  Negative SDF saturation beyond 128 is unreachable because the caller clamps to renderer spread before mapping. |
| Bitmap-SDF storage validation | `2588-2589,2591-2592,2603-2604,2606-2607,2609-2610` | none | Dimension overflow, negative MONO pitch, and truncated owned buffers are safe-Rust validation.  Public loaded SBITs have consistent positive pitch/storage, while C uses unchecked raw buffers and has no deterministic oracle for truncated storage. |
| Bitmap-SDF edge topology | `2706,2727,2781` | `2647,2701-2704,2726,2780` | GRAY and MONO SBIT success routes are real.  The opaque 3x3 SBIT resolves the all-eight-neighbors non-edge side.  Remaining border, alpha-gradient, and zero-radicand sides need source-backed SBIT alpha/mono shapes with exact retained bytes. |
| Orientation, LCD box, and empty cbox | `2869,3023,3029` | `2868,3028` | Invalid contour endpoints and nonempty points with no usable contours are loader-inconsistent.  The non-LCD padding arm is private-call only because public dispatch calls this helper for LCD/LCD_V. |
| `unpack_mono_row` | `2969-2974,2976,2979-2980` | `2973` | No production renderer caller exists.  Direct harness invocation, including historical commit `2e0f2637`, is helper-only and must not count as a FreeType route. |

The exact source projection after the verified change is therefore:

- zero-hit: `148-149,237-238,305,309,408,489,564,579,718,771,800,868,1109,1134,1165,1222,1234,1243-1244,1268,1278,1315,1332,1343-1346,1349,1435,1437,1534,1536,1603,1617,1630,2014,2028,2041,2046,2083,2205,2216,2224-2225,2275,2293,2306-2307,2309,2312,2429,2588-2589,2591-2592,2603-2604,2606-2607,2609-2610,2706,2727,2781,2869,2906,2947,2969-2974,2976,2979-2980,3023,3029`;
- partial branch: `362,578,716,770,799,1108,1164,1221,1233,1271-1272,1274,1314,1329-1330,1339,1397,1432,1484,1523,1582,1584,1602,1616,1629,1809,2013,2016,2025,2040,2045,2204,2215,2274,2290-2291,2300,2428,2533,2647,2701-2704,2726,2780,2868,2973,3028`.

The preserved render worktrees were also reconciled against this exact
snapshot.  Coverage MCP run `b9bcd856-e500-4a92-88c9-417f18fcf500` and
snapshot `69763016-8a4a-4a88-b79a-2ea345774971` adapt the clipped-close mono,
off-grid-close mono, and transform-plus-MONO candidates to the current public
runner.  All three pass exact C/Rust/C-ABI/WASM parity (7,030 / 7,030), but
their combined `render.rs` metrics are identical to the preceding snapshot, so
none may be retained for this file bucket.  The wave2 centerline-SDF candidate
is already present in the current generated font and render matrix.  The
`current-render-untouched-lines` match-arm simplification and unused
`MonoOutlineProfileBuilder::move_to` removal are caller-proven cleanup only,
with no public route or coverage gain.  Archive all four preserved trees after
review; if the cleanup is desired, submit it separately as route-neutral code
maintenance rather than as parity coverage.

## Immediate Next Actions

Work must resume here unless a newer user request changes priority:

1. Remove false-green public adapters before adding more coverage-only rows.
   `FT_Get_Glyph_Name`, `FT_Get_Name_Index`, `FT_Get_Gasp`,
   `FT_Get_CMap_Format`, `FT_Get_CMap_Language_ID`, and
   `FT_Get_SubGlyph_Info`, `FT_Get_Postscript_Name`, `FT_Get_Sfnt_Name`,
   `FT_Load_Char`, `FT_Load_Glyph`, `FT_Face_SetUnpatentedHinting`, and
   `FT_Outline_Get_CBox`, `FT_Get_Sfnt_LangTag`, and
   `FT_Set_Named_Instance`-driven named-instance selection are now real parity.
   `FT_New_Size`, `FT_Done_Size`, `FT_Activate_Size`, and `FT_Select_Size`
   null-validation and non-null lifecycle rows are now exact
   C-oracle/Rust-FFI/C-ABI/WASM-ABI routes. `FT_OpenType_Validate` null-face,
   null-output, and missing-service rows and `FT_OpenType_Free` null rows now
   execute real Rust-FFI/C-ABI/WASM-ABI wrapper paths; continue with the
   remaining generic fallback rows, including selected/malformed OpenType
   validation rows that still need real OT validator support, and any other
   modeled public surfaces.
2. Complete R0 by turning the residual classification above into a per-function
   table: public, font-reachable, missing delegation, blocked by incomplete
   implementation, duplicate with independent proof, private/no-route, or
   currently unreachable but preserved.
3. Resume explicit fixture expansion in the active order: public route audit,
   render/raster matrix, autohint script/topology, TrueType interpreter edge
   programs, then scalar residuals. The safe LCD empty-outline divergence and
   safe `Font` convenience helper routes are now covered by explicit public
   rows; do not add hidden render or helper dimensions for them.
4. Keep all 248 runtime pending rows explicit and do not count them as
   coverage. The ledger currently consists of 242 unresolved public assets or
   runner lifecycles plus six runnable core dependencies: unloaded/reset slot
   lifecycle, render of unloaded or unsupported slots, allocator fault
   injection, Adobe MM, `FT_MM_Var`, and `gvar`/HVAR. The MVAR vertical-header
   row remains the seventh audit-only core dependency. The live non-SFNT face
   path is covered by the compact Type 1 fixture, and the ftsynth bitmap-slot
   rows run as real parity through the compact SBIT format-1 strike. Select
   future pending-core work from the refreshed route audit, not from older
   embedded-strike placeholders.
5. Keep the deprecated corpus isolated until final cleanup is separately
   reviewed and approved.
