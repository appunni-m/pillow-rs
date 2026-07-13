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
return path. The C ABI and WASM legs call their public wrappers directly for
these rows, so the wrappers remain thin pointer/handle surfaces over the Rust
core behavior.
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
Three compact generated format-1 `name` table controls now exercise malformed
language-tag count, record-array, and string-range guards through
`FT_New_Memory_Face`. Pinned C FreeType and Rust both reject the faces during
open, and the public rows carry route-visible `font` aliases beside their
memory-byte sources so route audit counts them as real parity instead of
fallback evidence.
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

| Measure | Current |
|---|---:|
| Logical public API cases | 4,165 |
| Concrete explicit cases | 6,786 |
| Additional grouped variants | 2,621 |
| Implicit cases | 0 |
| Runnable parity comparisons | 6,783 |
| Exact parity | 6,783 / 6,783 |
| Pending cases | 3 |
| Covered Rust lines | 16,864 / 18,743 (89.97%) |
| Rust function coverage | 1,091 / 1,235 (88.34%) |
| Rust instantiation coverage | 1,094 / 1,238 (88.37%) |
| Rust region coverage | 24,241 / 26,968 (89.89%) |
| Rust branch/condition coverage | 4,022 / 4,730 (85.03%) |
| Formal Rust MC/DC coverage | 0 / 0; not emitted by the installed toolchain |
| Active fixture font paths | 153 |
| Stored active font binaries | 110 files, 823 KiB |
| Active symlink aliases | 43 |
| Unique active font contents | 121 SHA-256 identities |
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
| `src/render.rs` | 1,720 / 2,272 | 349 / 426 | 121 / 164 | 2,425 / 3,216 | Render-mode and glyph-to-bitmap rows over focused outline, mono, LCD, cubic, and transformed fixtures |
| `src/font.rs` | 1,772 / 1,987 | 209 / 252 | 167 / 199 | 2,416 / 2,713 | Public route audit, charmap accessors, size variants, table lookup boundaries, layout/convenience wrappers |
| `src/autohint/latin.rs` | 2,531 / 2,828 | 992 / 1,282 | 70 / 73 | 3,637 / 4,207 | Latin blue-zone, serif, diagonal, link, and adjustment glyph roles in existing compact fonts |
| `src/scaler.rs` | 1,073 / 1,226 | 158 / 188 | 49 / 62 | 1,147 / 1,280 | Composite, no-scale, LCD/mono scaler entry points through public load/render rows |
| `src/autohint/globals_data.rs` | 63 / 293 | 0 / 0 | 1 / 2 | 117 / 234 | Script coverage rows; do not delete lookup data for coverage |
| `src/grays.rs` | 650 / 810 | 134 / 184 | 30 / 35 | 918 / 1,139 | Direct public outline/render rows that hit scan conversion edge cases |
| `src/ffi/handles.rs` | 1,610 / 1,637 | 301 / 324 | 169 / 170 | 2,195 / 2,228 | Public FFI route audit; wrappers stay thin and must delegate to core |
| `src/tt/hinter/exec.rs` | 1,352 / 1,379 | 373 / 416 | 40 / 43 | 2,739 / 2,945 | Add one TrueType program role per remaining VM state/opcode family |
| `src/autohint/cjk.rs` | 893 / 941 | 379 / 426 | 18 / 19 | 1,185 / 1,247 | CJK topology rows in the compact multiscript fixture |
| `src/api.rs` | 723 / 746 | 154 / 182 | 71 / 72 | 1,039 / 1,068 | Public API wrapper rows for render cache and glyph-slot surfaces |

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
format 2.0 custom names surface as `.notdef`. Remaining lines are the direct
invalid-index guard (`src/tt/post.rs:47`) plus format 3.0
(`src/tt/post.rs:66`) and unsupported-format (`src/tt/post.rs:70`) direct
fallbacks inside the private resolver. `FT_Get_Glyph_Name` rejects
`glyph_index >= num_glyphs` before calling into `post.rs`, `FT_Get_Name_Index`
only scans `0..num_glyphs`, and both wrappers reject format 3.0/unsupported
formats through `FT_FACE_FLAG_GLYPH_NAMES` before the private resolver runs.
Keep them classified unless a supported public route is identified.

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
below. The unified public API suite currently has 4,165 logical cases, 6,786
concrete explicit cases, 6,783 runnable exact-parity cases, three explicit
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
| Math/casts | `fixed.rs`, `casts.rs` | 0 | 0 | 3 | 8 | scalar boundary rows or semantic cleanup |

Per-file source gap ledger:

| Source | Missing lines | Line coverage | Missing funcs | Missing regions | Missing branches |
|---|---:|---:|---:|---:|---:|
| `src/render.rs` | 552 | 1720/2272 (75.70%) | 43 | 791 | 77 |
| `src/autohint/latin.rs` | 297 | 2531/2828 (89.50%) | 3 | 570 | 290 |
| `src/autohint/globals_data.rs` | 230 | 63/293 (21.50%) | 1 | 117 | 0 |
| `src/font.rs` | 215 | 1772/1987 (89.18%) | 32 | 297 | 43 |
| `src/grays.rs` | 160 | 650/810 (80.25%) | 5 | 221 | 50 |
| `src/scaler.rs` | 153 | 1073/1226 (87.52%) | 13 | 133 | 30 |
| `src/autohint/cjk.rs` | 48 | 893/941 (94.90%) | 1 | 62 | 47 |
| `src/autohint/types.rs` | 32 | 71/103 (68.93%) | 7 | 25 | 1 |
| `src/ffi/handles.rs` | 27 | 1610/1637 (98.35%) | 1 | 33 | 23 |
| `src/tt/hinter/exec.rs` | 27 | 1352/1379 (98.04%) | 3 | 206 | 43 |
| `src/api.rs` | 23 | 723/746 (96.92%) | 1 | 29 | 28 |
| `src/tt/cmap.rs` | 14 | 726/740 (98.11%) | 3 | 14 | 0 |
| `src/autohint/globals.rs` | 11 | 214/225 (95.11%) | 1 | 17 | 14 |
| `src/tt/fvar.rs` | 7 | 91/98 (92.86%) | 4 | 13 | 0 |
| `src/ffi/convert.rs` | 4 | 138/142 (97.18%) | 0 | 4 | 0 |
| `src/tt/hinter/iup.rs` | 3 | 99/102 (97.06%) | 0 | 4 | 8 |
| `src/tt/post.rs` | 3 | 95/98 (96.94%) | 0 | 13 | 2 |
| `src/tt/gasp.rs` | 2 | 45/47 (95.74%) | 2 | 6 | 0 |
| `src/autohint/loader.rs` | 1 | 226/227 (99.56%) | 0 | 2 | 2 |
| `src/tt/hinter/gs.rs` | 1 | 185/186 (99.46%) | 0 | 1 | 0 |
| `src/tt/hinter/mod.rs` | 1 | 277/278 (99.64%) | 0 | 6 | 4 |
| `src/casts.rs` | 0 | 51/51 (100.00%) | 0 | 0 | 6 |
| `src/fixed.rs` | 0 | 215/215 (100.00%) | 0 | 3 | 2 |
| `src/outline.rs` | 0 | 3/3 (100.00%) | 0 | 0 | 1 |
| `src/tt/hinter/zone.rs` | 0 | 37/37 (100.00%) | 0 | 2 | 2 |
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

Recorded from the active public input JSON on 2026-07-13. This is the current
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
| Real C/Rust/C-ABI/WASM parity route | 3,431 | Use these rows for structural coverage evidence. |
| Real null-validation route | 8 | `FT_New_Size`, `FT_Done_Size`, `FT_Activate_Size`, `FT_OpenType_Validate`, and `FT_OpenType_Free` null rows execute pinned C oracle status checks and wrapper validation; size lifecycle null rows now use direct C/WASM lifecycle exports, and success rows live in real parity. |
| Wrapper null-validation route | 1 | `FT_Get_SubGlyph_Info` null-output rows intentionally validate the thin Rust/C/WASM wrapper guard after a native-C proof row establishes the composite slot state. |
| Raw-slot null-validation route | 4 | Runtime rows intentionally validate raw glyph-slot pointer handling after a concrete slot state is established. |
| Compile/header/scalar contract | 2,229 | Valid for ABI/header contracts, not runtime core coverage. |
| Shape-incomplete fallback | 0 | Keep this at zero; future incomplete declarations must become executable variants or explicit pending rows in the same change. |
| Generic modeled fallback | 926 | Classify operation-by-operation as real parity, unsupported, or pending. |
| Generic modeled error fallback | 141 | Replace implemented surfaces with real error-path execution. |
| Null-error fallback | 21 | Keep only exact null-handle probes; route implemented null cases directly. |
| Void fallback | 2 | Replace with real null/noop wrapper rows or classify as void API contract. |
| Explicit unsupported | 12 | Keep only where the public surface is intentionally unsupported. |
| Pending core | 11 | Convert to runnable parity when the named dependencies or compact fixtures exist. |
| Explicit unsupported stubs | 12 | Implement or keep visibly unsupported; do not count as coverage. |
| Pending core implementation | 11 | Named-instance Adobe MM, `FT_MM_Var`, `gvar`/HVAR, synthetic unloaded/unsupported slot states, compact overlap rendering, recursive composite SBIT missing-subglyph behavior, MVAR table variation rows, `FT_Select_Size` active-size mutation, and ftsynth bitmap-slot synthesis rows remain pending. |

The former two shape-incomplete ftsynth bitmap declarations are now explicit
pending-core rows. FreeType `src/base/ftsynth.c:106-180` accepts
`FT_GLYPH_FORMAT_BITMAP`, rounds the requested x/y strengths to pixels, calls
`FT_GlyphSlot_Own_Bitmap` and `FT_Bitmap_Embolden`, then updates slot advance,
metrics, and `bitmap_top`.  Rust currently handles only outline slots in
`src/ffi/handles.rs`, so these rows require core bitmap-slot behavior plus an
embedded-bitmap strike that loads a real bitmap slot before they can become
real parity.

| Route | Current behavior | Coverage risk | Required disposition |
|---|---|---|---|
| `oracle_fallback_args` default | Emits a generic FreeType error for any operation that reaches the default `_other` arm | A newly implemented public operation can still pass by agreeing with a modeled error | Every operation that reaches this path must be listed as intentionally unsupported, pending implementation, or converted to a real oracle arm |
| `oracle_fallback_args` null-operation classifier | No-font `expect_error` rows can be converted into classified null-handle errors | Valid only for pure null-handle probes; unsafe for operations whose failure depends on loaded face state | Keep only when the public C call is exactly a null-handle classification |
| No-asset non-error void route | Some null/no-asset non-error rows return `--void` / `{"void": true}` | Can hide missing wrapper behavior because no state or output is compared | Audit each row; either route through the real public wrapper or mark as a deliberately void API contract |
| Global Rust `_` fallback | Returns `FT_Err_Unimplemented_Feature` for unmatched operations | Rust core coverage cannot improve through this path and parity is only error agreement | Convert implemented operations to explicit Rust FFI handlers; leave unsupported optional modules visibly unsupported |
| C ABI / WASM `_other` fallback | Falls through to the Rust FFI runner for unsupported binding operations | Thin-wrapper coverage is not proven when the C/WASM leg never calls its public export | For every retained public C/WASM symbol, add direct wrapper execution or mark the symbol as intentionally Rust-only/test-only |
| C ABI / WASM explicit Rust delegation | Constants, layout probes, compile probes, several SFNT table routes, transforms, reference-face, unsupported stubs, size helpers, and `freetype.new_face` are routed to Rust | Acceptable for compile-time/header probes; unsafe for runtime public functions that should exercise ABI pointer handling | Split into compile-contract probes versus runtime ABI obligations; runtime functions need direct thin-wrapper rows |
| `ftsizes` lifecycle rows | Null validation rows still prove handle-error behavior. The three non-null sequence rows now execute pinned C oracle commands, Rust FFI, direct C ABI exports, and direct WASM ABI exports for secondary-size allocation, activation, destruction, and active-size fallback | The remaining size lifecycle gap is `ftsizes.activate_select_size_sequence`, which depends on real `FT_Select_Size` strike selection and active-size mutation | Keep the implemented rows in real parity; leave the select-size sequence explicit pending until core strike selection matches FreeType |
| Explicit Rust unsupported stubs | `freetype.face_properties` and `freetype.select_size` return `Unimplemented_Feature` directly | These are public FreeType surfaces; final 100% correctness cannot treat them as covered behavior | Implement exact public behavior or keep manifest rows visibly pending/failing until implementation exists |
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
| `freetype.select_size` | Explicit unsupported stub | Add available-size/strike fixture support or keep rows pending until embedded bitmap strikes are represented |
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

2026-07-13 reachability note: the current condition report still lists several
metadata parser guards that should not be assigned to new font rows without a
new public route. `tt/post.rs` format 3.0 and unknown-format fallback arms are
blocked by the public `FT_FACE_FLAG_GLYPH_NAMES` gate in `FT_Get_Glyph_Name`;
supported format 2.0/2.5 malformed rows already cover the public `.notdef`
fallback behavior. `tt/fvar.rs` and `tt/gasp.rs` checked arithmetic overflow
closures are fed by 16-bit SFNT counts, so those overflow arms cannot overflow
on the current 64-bit target through a valid or malformed font file. Keep these
lines classified as preserved defensive guards unless a maintained public API
call path is added that can exercise them honestly.

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
missing wrapper-line coverage.

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
| 2026-07-11 | Indic CJK autohint script rows | 103 unique hashes | 0 | 6,550 | 6,547 / 6,547 | 3 | 14,312 / 17,146 lines; 20,693 / 24,623 regions; 3,428 / 4,370 branches | four explicit `FT_LOAD_FORCE_AUTOHINT` rows select Limbu, Oriya, Syloti Nagri, and Tibetan glyphs in `script-coverage.ttf`. Core now routes FreeType's `STYLE_DEFAULT_INDIC` rows through CJK metrics/hints with no blue zones and rejects standard-character glyphs assigned to another style, matching pinned C/Rust/C-ABI/WASM parity without implicit case growth |
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
| 2026-07-12 | Malformed name language-tag parser controls | 113 unique hashes | 0 | 6,662 | 6,658 / 6,658 | 4 | 15,223 / 17,756 lines; 22,085 / 25,444 regions; 3,685 / 4,522 branches | Two compact format-1 `name` table controls cover language-tag record-array overflow and language-tag string out-of-range guards through public `FT_New_Memory_Face`. Pinned C and Rust both reject the faces at open, route audit counts both rows as real parity, and `tt/name.rs` moves to 331 / 333 lines with exact Rust/C ABI/WASM parity |
| 2026-07-12 | Malformed name language-tag count guard | 114 unique hashes | 0 | 6,663 | 6,659 / 6,659 | 4 | 15,224 / 17,756 lines; 22,088 / 25,444 regions; 3,685 / 4,522 branches | One compact format-1 `name` table control omits the language-tag count field after a complete zero-record header. Public `FT_New_Memory_Face` now compares pinned C and Rust rejection through Rust FFI, C ABI, and WASM ABI; route audit counts the row as real parity and `tt/name.rs` moves to 332 / 333 lines and 29 / 30 functions |
| 2026-07-12 | Name string out-of-range fallback controls | 116 unique hashes | 0 | 6,665 | 6,661 / 6,661 | 4 | 15,225 / 17,756 lines; 22,092 / 25,444 regions; 3,686 / 4,522 branches | Two compact name-table controls cover successful fallback after malformed name string offsets: `FT_New_Memory_Face` proves an out-of-range English Windows typographic family record falls back to Apple Roman, and `FT_Get_Postscript_Name` proves an out-of-range Apple PostScript record returns null. `tt/name.rs` reaches 333 / 333 lines, 30 / 30 functions, and 121 / 138 branch outcomes |

## Decision Log

| Date | Decision | Reason |
|---|---|---|
| 2026-07-10 | Use explicit grouped input variants only | Allows deliberate multi-input cases without hidden Cartesian growth |
| 2026-07-10 | Do not parameterize glyph-index discovery | Glyph selection must be explicit and tied to topology or behavior |
| 2026-07-10 | Measure Rust coverage only | Rust core owns behavior; C ABI and WASM ABI are thin wrappers exercised by the same parity cases |
| 2026-07-12 | Route cmap format-14 selector lists through the real call | Pinned C `FT_Face_GetVariantSelectors` finds the platform 0 encoding 5 format-14 charmap, returns a face-owned zero-terminated `FT_UInt32` selector list in subtable order, returns `NULL` for null face or no format-14 charmap, and allows the scratch result to be overwritten by the next FreeType call; fixtures must copy values immediately |
| 2026-07-12 | Preserve C UVS list null-versus-empty distinctions | Pinned C `FT_Face_GetVariantsOfChar` returns a non-null zero-terminated empty list when a format-14 charmap exists but no selector applies to the character, while `FT_Face_GetCharsOfVariant` returns `NULL` for an absent selector or a selector record with both default and non-default offsets zero |
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
| 2026-07-11 | Classify fvar instance-count overflow as unreachable | `instance_count` and `instance_size` are 16-bit SFNT fields, so their product fits in `usize` on supported 32-bit and 64-bit targets. Keep the defensive guard visible for now instead of deleting it to manufacture line coverage |
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
| 2026-07-13 | Classify autohint blue-character lookup as no-route helper coverage | Fresh coverage at 6,755 concrete rows shows all 60 explicit `script-coverage.ttf` `FT_LOAD_FORCE_AUTOHINT` rows exercise the public `STYLE_TABLE` and standard-character script paths. The remaining `globals_data::blue_chars_for_script` arms are not called by the public autohint load route, which passes `style.blue_entries` directly into `metrics_init_blues_impl`; adding another glyph row over the same compact font does not reach them |
| 2026-07-12 | Route cmap format-14 glyph-index queries through the real call | Pinned `FT_Face_GetCharVariantIndex` returns zero unless the active charmap is Unicode and a format-14 charmap exists, truncates `FT_ULong` charcode and selector inputs to `FT_UInt32`, uses the active Unicode charmap for default UVS glyph lookup, and uses the format-14 non-default GID table otherwise. Public inputs should keep default, non-default, missing, no-format14, and null-face rows explicit |
| 2026-07-12 | Route cmap format-14 default queries through the real call | Pinned `FT_Face_GetCharVariantIsDefault` finds the platform 0 encoding 5 format-14 selector charmap directly, truncates `FT_ULong` charcode and selector inputs to `FT_UInt32`, returns 1 for default UVS coverage, 0 for non-default UVS coverage with a nonzero glyph, and -1 for missing selector/char/no-format14/null face. Unlike `FT_Face_GetCharVariantIndex`, it does not require the active charmap to be Unicode |
| 2026-07-13 | Preserve format-14 UVS edge semantics | Public UVS rows now prove pinned C and Rust agree that a non-default UVS mapping whose glyph ID is zero does not count as non-default coverage for `FT_Face_GetCharVariantIsDefault` or `FT_Face_GetVariantsOfChar`, while a selector with a non-default table and no default table is still a present selector for `FT_Face_GetCharsOfVariant`. A platform-0 Unicode active charmap is valid for default UVS glyph-index lookup. The remaining `tt/cmap.rs` format-14 missing lines are only host-width checked-arithmetic overflow closures, not missing public UVS semantics |
| 2026-07-12 | Route `FT_Get_Transform` pointer rows through real transform calls | Existing rows now apply `FT_Set_Transform` sequences and nullable `FT_Get_Transform` output pointers through the pinned C oracle and Rust runner. Pinned `ftobjs.c` resets a null matrix to identity and a null delta to zero in `FT_Set_Transform`; Rust core must match that before the fixture can prove `returns_last_set_transform`. Refreshed condition coverage is 14,722 / 17,290 lines, 21,457 / 24,862 regions, and 3,596 / 4,406 branches with 6,644 / 6,644 runtime rows passing and four explicit pending rows |
| 2026-07-12 | Route request-size divide guards through real parity | The existing `fonts/glyf/glyf-malformed-matrix.ttf` compact font has a zero head bbox. Two explicit `FT_SIZE_REQUEST_TYPE_BBOX` rows now drive pinned `ftobjs.c:3301-3317` height and width divide guards through `FT_Request_Size`, covering the Rust `SizeRequestError::DivideByZero` mapping without increasing concrete cases. Route audit moves real parity to 3,249 and generic fallback to 960; refreshed condition coverage is 14,727 / 17,290 lines, 21,462 / 24,862 regions, and 3,600 / 4,406 branches with 6,645 / 6,645 runtime rows passing and four explicit pending rows |
| 2026-07-12 | Route `FT_Get_Sfnt_LangTag` through real format-1 name data | Pinned `ftsnames.c` requires `langID > 0x8000` but indexes `langTags[langID - 0x8000]`, making language-tag record zero unreachable through the public API. The compact fixture therefore carries two records and selects `0x8001`; null output, format-0, `0x8000`, and upper-bound rows must call the real public function instead of generic fallback |
| 2026-07-12 | Retire stale public operation names only when an equivalent maintained route exists | The `FT_Load_Sfnt_Table` table-missing row now uses `sfnt.load_sfnt_table` with the existing compact SFNT input, moving one row from generic fallback to real parity without runtime-code changes or weakened comparison shape. Pathname-driven rows such as missing-resource and zero-byte `FT_New_Face`, plus missing-post `FT_Get_Glyph_Name`, stay generic until their exact C source path has a compact fixture and route-equivalent output shape |
| 2026-07-12 | Route OpenType validation null contracts through real parity | `FT_OpenType_Validate` now matches pinned `ftotval.c` early exits for null face and null output pointers, with exact error-output comparison enabled on those public rows. `FT_OpenType_Free` null-face and null-table rows now call pinned C and the Rust FFI wrapper instead of falling through generic modeled errors. Route audit moves real-null-validation to 8, generic fallback to 942, and generic-error fallback to 141; refreshed condition coverage is 15,216 / 17,756 lines, 22,079 / 25,444 regions, and 3,684 / 4,522 branches with 6,656 / 6,656 runtime rows passing and four explicit pending rows |
| 2026-07-12 | Treat malformed format-1 language-tag controls as memory-face parser parity | Two generated format-1 `name` table controls now drive language-tag record-array overflow and string-range guards through `FT_New_Memory_Face`. Pinned C FreeType and Rust both reject the face during open; refreshed condition coverage is 15,223 / 17,756 lines, 22,085 / 25,444 regions, and 3,685 / 4,522 branches with 6,658 / 6,658 runtime rows passing and four explicit pending rows |
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

## Residual Coverage Classification - 2026-07-13

Fresh `test-unified-condition-coverage` still reports 1,879 uncovered core
source lines. The current split is:

| Measure | Count |
|---|---:|
| Logical public API cases | 4,165 |
| Concrete explicit cases | 6,786 |
| Runnable parity comparisons | 6,783 / 6,783 |
| Pending cases | 3 |
| Covered Rust lines | 16,864 / 18,743 (89.9749%) |
| Rust region coverage | 24,241 / 26,968 (89.8880%) |
| Rust branch/condition coverage | 4,022 / 4,730 (85.0317%) |
| Rust function coverage | 1,091 / 1,235 (88.3401%) |
| Route audit split | real-parity 3,431; raw-slot-null-validation 4; pending-core 11; shape-incomplete-fallback 0 |

| Bucket | Evidence | Action |
|---|---|---|
| Fixture/font reachable | `autohint/latin.rs`, `autohint/cjk.rs`, `scaler.rs`, `tt/hinter/exec.rs`, and parts of `render.rs` still have real branch gaps tied to glyph topology, script selection, bytecode state, or render geometry | Add or extend compact source-backed fonts and explicit public rows only when the selected glyph moves the measured branch or line |
| Public unsupported implementation paths | `FT_OpenType_Validate` non-null behavior and the ftsynth bitmap-slot paths for `FT_GlyphSlot_AdjustWeight` / `FT_GlyphSlot_Embolden` have non-null or mutation paths that return preserved stubs today | Implement the real public behavior first, then add parity rows; do not add fake success fixtures |
| Public-construction unreachable guards | Short required `head`/`hhea` tables fail face construction before `face_to_ffi`; short optional `vhea` currently fails in `Font::truetype`; `tt/cmap.rs` format-14 branch outcomes are now closed, leaving only checked-multiply overflow closures at lines 786-789, 866-867, and 914-915 that cannot overflow on 64-bit from u32 counts; `tt/fvar.rs` checked-multiply overflow closures likewise cannot overflow from u16 counts | Leave visible and documented unless parser semantics change or a true public route appears |
| Defensive invalid helper guards | `RoundMode::from_u8`'s invalid-value fallback remains missed after all valid FreeType `TT_Round_*` constants are routed through public `FT_Load_Glyph` rows | Leave visible; do not add a synthetic invalid round-state path unless a real public opcode or ABI surface can supply one |
| Private/no-route helpers | `Font::layout_glyphs`, `Font::layout_bounds`, `layout_bounds_from_glyphs`, `grays::rasterize`, `grays::rasterize_shifted_in_box`, `grays::render_scanline`, and `render::render_loaded_char_mode_for_index` are not selected by the current public FreeType fixtures | Do not call these through synthetic tests; either expose a real public operation with C parity or prove and remove independently of coverage |
| Coverage instrumentation artifacts | `Font::load_sfnt_table` and several wrapper functions show zero-count closure symbols even while the public body is heavily executed; many missed `api.rs` and `font.rs` lines are trailing call arguments in covered functions | Use function bodies, contiguous blocks, and branch counters to choose cases; do not grow JSON for tail-line artifacts alone |

### Scaler Residual Map - 2026-07-13

Current `src/scaler.rs` misses are not all fixture candidates:

| Lines | Disposition | Reason |
|---|---|---|
| 193-234, 369-396, 428-472 | Private/no-route wrappers | Public `FT_Load_Glyph` and render routes enter through `Font` dispatchers that supply load mode, native hint mode, bytecode context, and hdmx policy directly; these convenience wrappers are not public FreeType routes |
| 750-756 | Public-construction unreachable fallback | Normal face construction installs `FontData::self_arc`; the clone fallback is defensive for hand-built `FontData`, not reachable from public fixtures |
| 861-862, 1084 | Diagnostic-only | These are guarded trace/debug logging lines. Rows should target the underlying branch or public output, not logging side effects |
| 1327-1347 | Public-construction unreachable owned context | Recursive native composite scaling receives a prepared bytecode context from `Font::native_bytecode_context_for_mode` whenever `fpgm` and `cvt` exist; without those tables, the inner prepare branch cannot execute |
| 1427-1431, 1525 | Parser-validated before scaler | Public glyf loading validates composite attachment and contour bounds before the scaled helper consumes the outline tree |
| 1462-1467 | Defensive tag fallback | Public scaled subglyphs carry outline tags; the fallback only synthesizes tags for a private no-tag outline |
| 1487-1492, 1678 | Preempted empty-outline guards | `scale_glyph_impl` returns empty outlines before exact-bbox decomposition or autohint mutation helpers run |
| 1630-1650 | Private pixel helpers | Public scaler/render code uses the `ft_pix_*` helpers directly; these conversion wrappers need a real caller, not synthetic coverage rows |

### Rejected Candidate Audit - 2026-07-13

These candidates were exact-parity probes but deliberately not kept because
they did not improve measured condition coverage or did not prove the intended
public surface:

| Bucket | Candidate | Result | Decision |
|---|---|---|---|
| Autohint script helpers | A candidate `script-notdef-glyph-force-autohint` row over existing `script-coverage.ttf` glyph 0 with `FT_LOAD_FORCE_AUTOHINT` | Focused `load_glyph` parity passed, and full runtime parity rose to 6,752 / 6,752, but condition coverage stayed exactly flat at 16,215 / 18,091 lines, 23,284 / 25,933 regions, 3,906 / 4,632 branches, and 1,024 / 1,150 functions; `globals.rs` and `globals_data.rs` missing lines were unchanged | Do not add more script rows for `blue_chars_for_script` or `globals::detect_script`. The maintained public route uses face-global style coverage plus `style.blue_entries`, not these helper functions |
| Autohint glyph-zero metrics | `FT_Load_Glyph.matrix_load@autohint-notdef-glyph-zero` over `fonts/autohint/digit-notdef-cmap.ttf` gid 0 with `FT_LOAD_FORCE_AUTOHINT` | Focused exact Rust FFI / C ABI / WASM ABI parity passed 1 / 1 and kept implicit cases at zero, but the focused coverage JSON showed `autohint/globals.rs:95,100,102,106` all at zero hits; the public load path bypasses `FaceGlobals::get_metrics` for this row | Do not add this row. It increases concrete cases without covering the glyph-zero metrics branch; a future candidate must prove source-line movement before entering the optimized fixture set |
| Scaler malformed composite | `glyf-malformed-invalid-point-attachment-no-hinting` over `fonts/glyf/glyf-malformed-matrix.ttf` gid 17 with `FT_LOAD_NO_HINTING` | Focused `FT_Load_Glyph` parity passed, but full condition coverage stayed flat at 16,215 / 18,091 lines, 23,284 / 25,933 regions, 3,906 / 4,632 branches, and 1,024 / 1,150 functions | Do not add this row. The public parser rejects the invalid attachment before the scaled composite helper's defensive error branch |
| Autohint topology | `latin-double-top-glyph-force-autohint`, `cjk-snap-below-standard-normal-force-autohint`, and `cjk-snap-below-standard-lcd-force-autohint` over existing compact autohint fonts | Each focused row passed exact parity, but full condition coverage stayed flat at 15,947 / 17,810 lines, 22,852 / 25,492 regions, 3,832 / 4,536 branches, and 1,005 / 1,135 functions | Do not re-add these rows. The next autohint improvement needs genuinely new glyph topology, not another explicit row over the current compact fonts |
| Autohint topology/load target | `cjk-wide-stem-snap-target-lcd-20` over `fonts/autohint/cjk-wide-stem-snap.ttf` U+4ED6 with `FT_LOAD_FORCE_AUTOHINT | FT_LOAD_TARGET_LCD` | Focused exact Rust FFI / C ABI / WASM ABI parity passed 1 / 1 and kept implicit cases at zero, but comparing the focused condition JSON against the 6,775-case baseline showed no newly covered baseline-missed lines or autohint branch outcomes | Do not add an LCD-only row over the existing wide-stem snap topology; a future target-mode row needs distinct geometry that moves a measured branch or line |
| TrueType empty-zone SHZ | A derived `hinter-empty-composite-shz.ttf` font whose empty composite glyph carried `PUSHB[0] 0; SHZ[0]`; the retained source-backed prep probe appends `SZPS 1; SHZ[0]` to `hinter-control-matrix.ttf` | The derived glyph row passed exact C/Rust/C-ABI/WASM parity but stayed flat. The retained prep route hits `tt/hinter/exec.rs:1408` because prep executes against an empty glyph zone before glyph loading | Do not add the derived font. Keep the existing base-prep route because it covers the public empty-zone `SHZ` branch with no concrete case growth |
| Render SDF/cubic | A possible CFF/CFF2 `FT_Render_Glyph` SDF row | Current Rust face loading still relies on glyf/loca fallback for the compact CFF fixture, so C would render cubic charstrings while Rust would not preserve a cubic public glyph outline through this path | Implement a real public cubic-outline loader route before adding SDF cubic render fixture rows |
| Render mono/profile | Remaining `render.rs` mono/profile helpers such as the old intersection rasterizer and low-level line/bezier helper families | No current `FT_Render_Glyph` row reaches these as public code; the profile-based horizontal and vertical dropout guards are already covered | Keep visible as private/no-route until a real public operation requires them or they are independently proven duplicate/obsolete |
| Historical size lifecycle sketch | Rust FFI-only implementation sketch for `FT_New_Size`, `FT_Done_Size`, and `FT_Activate_Size` success sequences | Superseded by the verified face-owned size implementation with direct C ABI and WASM ABI lifecycle exports. Focused sequence parity and full condition coverage now pass with the three success rows classified as real parity | Do not reintroduce Rust-only C/WASM delegation for lifecycle rows. Future size work should target the remaining `FT_Select_Size` active-size sequence blocker |
| Safe render no-value rows | Adding `assert_font_render_mode_agrees` to the existing Noto `FORCE_AUTOHINT | NO_AUTOHINT` render row passed focused `render_glyph` parity and full runtime parity, but total condition coverage stayed fixed at 16,227 / 18,091 lines, 23,297 / 25,933 regions, 3,909 / 4,632 branches, and 1,025 / 1,150 functions | Do not add more safe render agreement flags to rows that already exercise the same load-mode branch |
| Zero-width normal render row | A candidate `FT_RENDER_MODE_NORMAL` row over `fonts/glyf/hinter-control-matrix.ttf` U+E02B (`renderZeroWidth`) with safe render and getmask assertions passed exact parity, raising concrete rows locally to 6,758, but condition coverage and missing lines were unchanged | Do not keep this row. The current public load/render path does not reach a new zero-extent `Font::getmask_single_glyph` or render guard from that glyph |
| Render top-boundary gray clip | A candidate `render-coverage.ttf` grid-aligned 3x3 box rendered in normal mode passed focused `render_glyph` parity and raised concrete rows locally to 6,766 | It only increased already-covered `grays.rs` top/right clipping counts and left total, `render.rs`, and `grays.rs` line/region/branch/function coverage unchanged | Do not add top-boundary gray rows unless the focused condition report shows a new uncovered outcome, not just higher execution counts |
| TrueType IUP duplicate contour | A derived `hinter-duplicate-contour-iup.ttf` probe replaced gid 55 with a one-point simple glyph whose two contour end-points were both zero and whose glyph program ran `IUP[y]; IUP[x]` | Focused selection stayed to one explicit `FT_Load_Glyph.matrix_load` row, but the pinned FreeType oracle returned error 20 for the load before any comparable slot state existed | Do not add duplicate or non-advancing contour-end probes. They are invalid public glyph data for the C oracle, so they cannot be used to cover Rust-only IUP defensive branches without sacrificing parity correctness |

### Table And FFI No-Route Addendum - 2026-07-13

These misses were audited after the 6,757-case checkpoint. They are not
fixture-row candidates unless a new public route appears:

| Source lines | Classification | Reason |
|---|---|---|
| `autohint/coverage.rs:110-135` | Resolved by public-row assertion | The existing `FT_LOAD_FORCE_AUTOHINT.load_char_force_autohint_behavior@latin-italic-no-horizontal` row now declares `assert_autohint_coverage_bits_include: [32]`, proving the safe public load route records `COV_ITALIC_NO_HORZ` without adding a standalone fixture test |
| `tt/post.rs:47,66,70` | Public-gate unreachable | `FT_Get_Glyph_Name` validates `glyph_index < num_glyphs` before `PostTable::glyph_name`; `FT_Get_Name_Index` scans only valid glyph indexes; both wrappers suppress format 3.0 and unsupported `post` formats with `FT_FACE_FLAG_GLYPH_NAMES` before direct name lookup |
| `autohint/loader.rs:339` | Private metric-less hints fallback | Public `apply_hints` builds `GlyphHints` and the Latin/CJK setup paths install metrics before direction-chain construction. The default `near_limit_chain = 20` fallback only exists for private or diagnostic `GlyphHints` values without metrics; do not add synthetic autohint calls for it |
| `tt/fvar.rs:58-59` | Host-width unreachable defensive overflow | `instance_count` and `instance_size` are 16-bit SFNT fields, so their product cannot overflow `usize` on the supported 64-bit coverage target. Keep the guard for portability; do not manufacture a parser row |
| `tt/cmap.rs:786-789,866-867,914-915` | Host-width unreachable defensive overflow | Format-14 selector/default/non-default counts are `u32`. On the supported 64-bit target their record-byte products cannot overflow `usize`; malformed fonts instead reach the normal exceeds-length guards already covered by compact cmap rows |
| `tt/gasp.rs:59,62` | Host-width unreachable defensive overflow | `num_ranges` is a 16-bit SFNT field, so `num_ranges * 4 + 4` cannot overflow `usize` on the supported coverage target. Compact gasp rows already cover short, unsupported-version, truncated-range, and valid-range behavior; do not manufacture a parser-only overflow row |
| `tt/hinter/gs.rs:59` | Defensive invalid helper guard | Public TrueType opcodes route only valid `TT_Round_*` numeric constants through `RoundMode::from_u8`; existing `stackStateMatrix` and `superRoundMatrix` rows cover all valid round modes. No public opcode writes an arbitrary invalid round-state value |
| `tt/hinter/mod.rs:284` | Public call-site preempted by prepared context | Current public native load and metrics paths call `Font::native_bytecode_context_for_mode` before `hint_glyph`, so `prepared_context` is populated whenever `fpgm` and `cvt` exist. Without those tables, the bytecode branch is skipped before `hint_glyph`; the direct `prepare_context` fallback is only for private helper callers |
| `ffi/convert.rs:166,194-195,198` | No public conversion source | No current public runner produces `GlyphFormat::None`, `UnsupportedCmapFormat`, `RasterOverflow`, or `UnsupportedLoadFlags` through the thin FFI converter. Keep these mappings for boundary completeness, but do not add private synthetic conversion tests |
| `tt/hinter/exec.rs:265-283` | Private/no-route fetch helpers | The active interpreter loop uses `fetch_byte_glyph`; the older public `ExecContext::fetch_byte`/`fetch_word` helpers are not selected by public glyph-load execution |
| `tt/hinter/exec.rs:293` | Private call-site preempted by prepare path | The compact `hinter-empty-fpgm.ttf` row already proves empty font-program handling through public `FT_Load_Glyph`, but the prepare path skips `run_fpgm` entirely when `fpgm` is empty, so the internal `run_fpgm` empty-return line has no public fixture route |
| `tt/hinter/exec.rs:446` | Caller-guarded no-route branch | Public opcode handlers that need original twilight coordinates route through `org_in` whenever any zone pointer is twilight. The remaining `orus_in(..., zp=0)` fallback is defensive; valid `MD`, `MDRP`, `MIRP`, and `IP` public bytecode paths do not select it |
| `tt/hinter/exec.rs:508` | Short-circuit no-route branch | `SHPIX` compatibility handling sets `in_twilight` when any zone pointer is twilight and short-circuits before consulting `tag_in`; non-twilight paths use glyph tags. The twilight `tag_in` arm has no public opcode route |
| `tt/hinter/exec.rs:715-717` | Invariant-backed inactive-definition guard | Public FDEF/IDEF scanners either create active definitions or reject invalid, nested, over-budget, and unterminated definitions before calls. Existing call-error rows cover absent and invalid references; no public route produces an inactive definition record that can later be called |
| `tt/hinter/exec.rs:1464-1467` | Call-record contract guard | `enter_function_call` pushes `CallRecord` only after resolving an active definition index. The repeated `LOOPCALL` ENDF path therefore cannot lose its definition between pop and repeat without mutating private VM state; keep the guard, but do not add synthetic record corruption tests |
| `tt/hinter/iup.rs:33,42-43` | Public-gate unreachable or C-oracle rejected | Public `scale_glyph_impl` returns before bytecode hinting when `outline_raw.num_contours == 0`, so a non-empty point zone with an empty contour vector cannot reach `IUP` through `FT_Load_Glyph`. A focused duplicate-contour-end probe reached the JSON selector but pinned FreeType rejected the glyph with error 20, so non-advancing contour ends are not valid parity fixtures |

### Render/Raster Residual Audit - 2026-07-13

The current `route-audit` split for `FT_Render_Glyph` is 181 real-parity rows,
one `null-error-fallback` row, and three `pending-core` rows. The non-real
routes are `freetype.FT_Render_Glyph.error_null_or_unowned_slot`,
`freetype.FT_Render_Glyph.error_unloaded_or_unsupported_slot_format`, and the
two `ftimage.FT_OUTLINE_OVERLAP.smooth_overlap_behavior` variants. The null
row lacks a maintained glyph-slot selector, the unloaded/unsupported-slot row
needs explicit synthetic slot-state runner support, and overlap remains a core
fixture/behavior gap rather than a generic render-mode row.

Remaining uncovered render/raster source families:

| Source family | Classification | Decision |
|---|---|---|
| `grays::{rasterize,rasterize_shifted_in_box}` wrapper lines | Private/no-route from `FT_Render_Glyph`; direct outline surfaces own any future public `FT_Raster_Params` or clipping route | Do not add synthetic render rows. Add a real outline-render route only when the ABI surface exists and exact C/Rust/C-ABI/WASM parity can be compared |
| `grays::{ft_div_mod,Worker::render_scanline}` | Obsolete/no-call scanline helper path; current gray rendering reaches the DDA line/conic/cubic raster path instead | Keep visible until independently proven duplicate and removed in a cleanup not justified by coverage alone |
| `grays.rs` trace and debug-dump lines | Coverage instrumentation artifacts gated by debug logging or environment-controlled dumps | Do not add parity rows for logging-only execution |
| `render::render_loaded_char_mode_for_index` | Safe convenience helper with no current public FreeType manifest route; `FT_Render_Glyph` exercises the loaded slot render path instead | Leave as private/no-route unless a real safe Rust API parity obligation is added |
| `render::render_normal` and `render::render_sdf` zero-extent guards | Mostly shadowed by `render_loaded_outline` empty/box checks for valid `FT_Render_Glyph` rows | Add no row unless a valid C fixture reaches the same guard and moves coverage |
| `render::SdfFlattener` cubic or invalid-contour paths | Fixture/font reachable only after a real public cubic-outline loader route exists; current compact CFF probe would make C render cubic charstrings while Rust does not preserve that outline through this path | Implement cubic outline loading/parity first, then add the smallest SDF row |
| `render::MonoProfileBuilder`, `rasterize_mono_intersections`, and low-level line/bezier wrappers | Private/no-route or obsolete duplicate mono raster helpers; current mono output uses `MonoOutlineProfileBuilder`, and the horizontal/vertical dropout guards are already covered by `render-coverage.ttf` rows | Do not grow `FT_Render_Glyph` JSON with duplicate topology. Remove only with independent semantic proof |
| `render::MonoOutlineProfileBuilder` branch residuals | Potentially fixture/font reachable, but only for exact topology branches not already covered by `hinter-control-matrix.ttf` or `render-coverage.ttf`; the folded-profile non-adjacent upper-stub branch is now covered by `render-coverage-folded-dropout-mono` | Add compact glyphs only after a focused candidate moves measured condition coverage with exact parity |
| `ftimage.FT_OUTLINE_OVERLAP.smooth_overlap_behavior` | Pending core/fixture route; needs compact overlap-heavy source-backed outline/font coverage and matching overlap behavior before becoming a real render row | Keep pending; do not substitute a no-value `FT_Render_Glyph` row |

Readable zero-count functions from `llvm-cxxfilt` fall into this first
ledger. Treat this as the next owner list, not as deletion evidence:

| Disposition | Zero-count functions or families | Route decision |
|---|---|---|
| Implement before fixture parity | `ffi::handles::FT_New_Face` | This is a public stub; fixtures must not fake success until core behavior exists and C/WASM ABI wrappers remain thin |
| Public helper not owned by current FreeType manifest route | `autohint::globals::detect_script`, `globals_data::blue_chars_for_script`, `latin::metrics_init_blues`, `latin::metrics_init_blues_greek`, `Direction::{as_i8,is_horizontal,is_vertical}`, `GlyphHints::num_contours`, `ExecContext::{fetch_byte,fetch_word}` | Keep visible; either route through an existing public manifest subject with real C parity or decide separately whether these helpers belong in public Rust surface |
| Private/no-route implementation helpers | `Font::{layout_glyphs,layout_bounds,slot_metrics_from_scaled,native_bytecode_context}`, `layout_bounds_from_glyphs`, `grays::{rasterize,rasterize_shifted_in_box,ft_div_mod,Worker::render_scanline}`, `render::MonoProfileBuilder::*`, `render::rasterize_mono_intersections`, `render::{line_up,line_down,bezier_up_2,bezier_down_2,unpack_mono_row,apply_horizontal_center_edges}` | Do not add synthetic tests. A real public operation must need them, or they need independent semantic cleanup after proving they are duplicate/obsolete |
| Covered body with closure artifact | `Font::load_sfnt_table` closures, `Font::truetype_face_with_load_mode` closures, `SizeMetrics::from_char_size` closure, `tt::{cmap,fvar,gasp}` checked-overflow closures, `api::GlyphSlot::new` closure | Do not add fixture rows for closure symbols alone. Add rows only if a public error branch or output difference is missing |
| Fixture/font reachable candidates | `cjk::cjk_mark_round_segments`, parts of `SdfFlattener`, `MonoOutlineProfileBuilder`, `scaler` metric/composite helpers, and `tt::hinter::exec::run_program` closure | Add compact glyph/topology/program rows only after identifying the exact branch and proving the row moves coverage with exact parity. `latin::find_second_lowest_contour` remains preserved code, but pinned FreeType 2.14.3 defines `AF_ADJUST_DOWN2` / `AF_ADJUST_TILDE_BOTTOM2` without any adjustment-database entries, so it is not currently reachable through a real public `char_code` fixture row |

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

## Immediate Next Actions

Work must resume here unless a newer user request changes priority:

1. Remove false-green public adapters before adding more coverage-only rows.
   `FT_Get_Glyph_Name`, `FT_Get_Name_Index`, `FT_Get_Gasp`,
   `FT_Get_CMap_Format`, `FT_Get_CMap_Language_ID`, and
   `FT_Get_SubGlyph_Info`, `FT_Get_Postscript_Name`, `FT_Get_Sfnt_Name`,
   `FT_Load_Char`, `FT_Load_Glyph`, `FT_Face_SetUnpatentedHinting`, and
   `FT_Outline_Get_CBox`, `FT_Get_Sfnt_LangTag`, and
   `FT_Set_Named_Instance`-driven named-instance selection are now real parity.
   `FT_New_Size`, `FT_Done_Size`, and `FT_Activate_Size` null-validation and
   non-null lifecycle rows are now exact C-oracle/Rust-FFI/C-ABI/WASM-ABI
   routes. Continue with generic fallback rows, especially
   `FT_Select_Size` active-size mutation, `FT_OpenType_Validate`, and any
   other modeled public surfaces.
2. Complete R0 by turning the residual classification above into a per-function
   table: public, font-reachable, missing delegation, blocked by incomplete
   implementation, duplicate with independent proof, private/no-route, or
   currently unreachable but preserved.
3. Resume explicit fixture expansion in the active order: public route audit,
   render/raster matrix, autohint script/topology, TrueType interpreter edge
   programs, then scalar residuals. The safe LCD empty-outline divergence and
   safe `Font` convenience helper routes are now covered by explicit public
   rows; do not add hidden render or helper dimensions for them.
4. Keep the current three runtime pending rows explicit and do not count them
   as coverage until the core Adobe MM, `FT_MM_Var`, and `gvar`/HVAR behavior
   exists. The live non-SFNT face path is now covered by the compact Type 1
   fixture. The ftsynth embedded-strike bitmap rows are route-audit
   pending-core rows, not runtime pending rows, until core bitmap-slot
   synthesis and a real bitmap-strike load route exist.
5. Keep the deprecated corpus isolated until final cleanup is separately
   reviewed and approved.
