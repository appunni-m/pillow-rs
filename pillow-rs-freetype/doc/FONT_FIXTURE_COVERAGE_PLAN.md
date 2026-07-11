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

The 3 pending cases are existing unsupported or unresolved named-instance
inputs. They must remain visible and be converted to runnable explicit cases
during the coverage phases where their owning operations are addressed.

## Current Verified Coverage State

Recorded on 2026-07-12 after converting `FT_Get_Gasp`,
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
fonts or cases.
Three named-instance obligations remain explicit pending rows: Adobe MM reset
behavior, `gvar`/HVAR glyph-output deltas, and `FT_MM_Var` namedstyle
coordinate parity.

| Measure | Current |
|---|---:|
| Logical public API cases | 4,136 |
| Concrete explicit cases | 6,606 |
| Additional grouped variants | 2,470 |
| Implicit cases | 0 |
| Runnable parity comparisons | 6,603 |
| Exact parity | 6,603 / 6,603 |
| Pending cases | 3 |
| Covered Rust lines | 14,551 / 17,190 (84.65%) |
| Rust function coverage | 875 / 1,066 (82.08%) |
| Rust instantiation coverage | 878 / 1,069 (82.13%) |
| Rust region coverage | 21,223 / 24,719 (85.86%) |
| Rust branch/condition coverage | 3,533 / 4,390 (80.48%) |
| Formal Rust MC/DC coverage | 0 / 0; not emitted by the installed toolchain |
| Active fixture font paths | 140 |
| Stored active font binaries | 97 files, 772 KiB |
| Active symlink aliases | 43 |
| Unique active font contents | 108 SHA-256 identities |
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
| `src/render.rs` | 1,566 / 2,275 | 323 / 428 | 109 / 164 | 2,262 / 3,221 | Render-mode and glyph-to-bitmap rows over focused outline, mono, LCD, cubic, and transformed fixtures |
| `src/font.rs` | 1,415 / 1,908 | 174 / 250 | 127 / 186 | 1,924 / 2,597 | Public route audit, size variants, table lookup boundaries, layout/convenience wrappers |
| `src/autohint/latin.rs` | 2,510 / 2,828 | 980 / 1,282 | 70 / 73 | 3,611 / 4,207 | Latin blue-zone, serif, diagonal, link, and adjustment glyph roles in existing compact fonts |
| `src/scaler.rs` | 934 / 1,220 | 150 / 188 | 41 / 61 | 1,067 / 1,274 | Composite, no-scale, LCD/mono scaler entry points through public load/render rows |
| `src/autohint/globals_data.rs` | 63 / 293 | 0 / 0 | 1 / 2 | 117 / 234 | Script coverage rows; do not delete lookup data for coverage |
| `src/grays.rs` | 646 / 810 | 131 / 184 | 30 / 35 | 912 / 1,139 | Direct public outline/render rows that hit scan conversion edge cases |
| `src/ffi/handles.rs` | 1,338 / 1,478 | 233 / 280 | 146 / 162 | 1,879 / 2,024 | Public FFI route audit; wrappers stay thin and must delegate to core |
| `src/tt/hinter/exec.rs` | 1,296 / 1,340 | 353 / 410 | 37 / 40 | 2,676 / 2,901 | Add one TrueType program role per remaining VM state/opcode family |
| `src/autohint/cjk.rs` | 839 / 941 | 343 / 426 | 18 / 19 | 1,124 / 1,247 | CJK topology rows in the compact multiscript fixture |
| `src/api.rs` | 462 / 486 | 67 / 84 | 53 / 54 | 625 / 660 | Public API wrapper rows for render cache and glyph-slot surfaces |

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
invalid-index guard plus format 3.0 and unsupported direct fallbacks inside the
private resolver; current public wrappers validate or reject those states before
calling into `post.rs`. Keep them classified unless a supported public route is
identified.

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
coverage-only deletion. Remaining `fixed.rs` lines are private 32-bit wrapper
helpers plus vector-length and vector-normalization branches that need either
existing public route inputs or a separate reachability classification.

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

Evaluation checkpoint: 2026-07-11, latest verified unified condition-coverage run.

This is the active coverage identification ledger. It supersedes earlier
percentages in this section but does not replace the historical progress ledger
below. The unified public API suite currently has 4,136 logical cases, 6,560
concrete explicit cases, 6,557 runnable exact-parity cases, three explicit
pending named-instance obligations, and zero implicit cases.
`FT_Get_Postscript_Name.variation_instance_name_behavior` remains an active
parity row backed by real `FT_Set_Named_Instance` behavior, while
`ftmm.set_named_instance` now has direct selection, clear, and invalid-index
parity rows. The pending rows are Adobe MM named-instance reset, namedstyle
coordinate parity through `FT_MM_Var`, and glyph-output deltas that require
`gvar`/HVAR support.

Core Rust structural coverage from
`make -C pillow-rs-freetype test-unified-condition-coverage` is:

| Measure | Covered | Total | Remaining |
|---|---:|---:|---:|
| Functions | 858 | 1,062 | 204 |
| Lines | 14,378 | 17,141 | 2,763 |
| Regions | 20,879 | 24,618 | 3,739 |
| Branches/conditions | 3,496 | 4,370 | 874 |

Formal MC/DC is not reported by the installed Rust coverage tooling
(`mcdc.count == 0`). Branch/condition coverage is therefore the instrumented
measure, and each compound predicate still needs explicit independent-effect
fixture obligations.

The remaining coverage divides exactly into these ownership groups:

| Group | Modules | Missing functions | Missing lines | Missing regions | Missing branches | Primary action |
|---|---|---:|---:|---:|---:|---|
| Face/API/scaler/FFI/SFNT metadata | `font.rs`, `scaler.rs`, `api.rs`, `ffi/handles.rs`, `ffi/convert.rs`, `ffi/types.rs`, `tt/name.rs`, `tt/post.rs`, `tt/cmap.rs`, `tt/gasp.rs`, `tt/fvar.rs` | 105 | 966 | 1,113 | 199 | public routing, wrapper thinness, metadata/state inputs |
| Rendering | `render.rs`, `grays.rs`, `outline.rs` | 60 | 873 | 1,186 | 159 | render topology, mode, clipping, pitch, SDF, and bitmap rows |
| Autohint | `latin.rs`, `cjk.rs`, `globals_data.rs`, `types.rs`, `coverage.rs`, `globals.rs`, `loader.rs` | 18 | 722 | 915 | 413 | script reachability audit, then glyph topology rows |
| TrueType interpreter | `tt/hinter/exec.rs`, `gs.rs`, `mod.rs`, `zone.rs`, `iup.rs`, `tt/mod.rs` | 4 | 66 | 257 | 77 | explicit bytecode-program glyph rows |
| Math/casts | `fixed.rs`, `casts.rs` | 4 | 12 | 25 | 9 | scalar boundary rows or semantic cleanup |

Per-file source gap ledger:

| Source | Missing lines | Line coverage | Missing funcs | Missing regions | Missing branches |
|---|---:|---:|---:|---:|---:|
| `src/render.rs` | 709 | 1566/2275 (68.84%) | 55 | 959 | 105 |
| `src/font.rs` | 493 | 1415/1908 (74.16%) | 59 | 673 | 76 |
| `src/autohint/latin.rs` | 318 | 2510/2828 (88.76%) | 3 | 596 | 302 |
| `src/scaler.rs` | 286 | 934/1220 (76.56%) | 20 | 207 | 38 |
| `src/autohint/globals_data.rs` | 230 | 63/293 (21.50%) | 1 | 117 | 0 |
| `src/grays.rs` | 164 | 646/810 (79.75%) | 5 | 227 | 53 |
| `src/ffi/handles.rs` | 140 | 1338/1478 (90.53%) | 16 | 145 | 47 |
| `src/tt/hinter/exec.rs` | 44 | 1296/1340 (96.72%) | 3 | 225 | 57 |
| `src/autohint/cjk.rs` | 102 | 839/941 (89.16%) | 1 | 123 | 83 |
| `src/api.rs` | 24 | 462/486 (95.06%) | 1 | 35 | 17 |
| `src/tt/name.rs` | 1 | 293/294 (99.66%) | 1 | 11 | 18 |
| `src/autohint/types.rs` | 32 | 71/103 (68.93%) | 7 | 25 | 1 |
| `src/autohint/coverage.rs` | 22 | 6/28 (21.43%) | 5 | 28 | 4 |
| `src/fixed.rs` | 9 | 206/215 (95.81%) | 3 | 22 | 3 |
| `src/ffi/convert.rs` | 4 | 138/142 (97.18%) | 0 | 4 | 0 |
| `src/tt/fvar.rs` | 7 | 91/98 (92.86%) | 4 | 13 | 1 |
| `src/tt/hinter/gs.rs` | 14 | 172/186 (92.47%) | 1 | 14 | 2 |
| `src/autohint/globals.rs` | 13 | 214/227 (94.27%) | 1 | 20 | 18 |
| `src/tt/cmap.rs` | 1 | 428/429 (99.77%) | 1 | 3 | 0 |
| `src/ffi/types.rs` | 5 | 0/5 (0.00%) | 1 | 3 | 0 |
| `src/autohint/loader.rs` | 5 | 222/227 (97.80%) | 0 | 6 | 5 |
| `src/tt/hinter/mod.rs` | 4 | 274/278 (98.56%) | 0 | 11 | 7 |
| `src/tt/hinter/iup.rs` | 4 | 98/102 (96.08%) | 0 | 5 | 9 |
| `src/tt/post.rs` | 3 | 95/98 (96.94%) | 0 | 13 | 2 |
| `src/casts.rs` | 3 | 48/51 (94.12%) | 1 | 3 | 6 |
| `src/tt/gasp.rs` | 2 | 45/47 (95.74%) | 2 | 6 | 0 |
| `src/outline.rs` | 0 | 3/3 (100.00%) | 0 | 0 | 1 |
| `src/tt/hinter/zone.rs` | 0 | 37/37 (100.00%) | 0 | 2 | 2 |

The exact line-range inspection artifact for the latest run is generated at
`target/coverage/unified-condition-missing-lines.txt` by
`make -C pillow-rs-freetype test-unified-condition-coverage`. It is
intentionally not committed because `target/` is generated output; this table
is the source-controlled ownership view.

This concentration changes the execution strategy. More fonts alone cannot
close the report. Entire modules such as `autohint/coverage.rs` and
`ffi/types.rs`, plus many convenience methods in `font.rs`, `api.rs`, and
`render.rs`, have no covered functions. Each must first be
classified as one of:

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
  rebuilt with `make font-fixture-cmap`.

| Public operation | Current runner behavior | Why it is unsafe | Required path |
|---|---|---|---|
| Generic `oracle_fallback_args` rows | Returns `Unimplemented_Feature` for unmatched operations | Correct only for intentionally unsupported public surfaces | Audit each remaining fallback row against `manifest.yaml`; real implemented operations need explicit match arms in oracle, Rust, C ABI, and WASM ABI |

### R0 False-Green Route Audit Snapshot

Recorded after commit `97b131d4`. This is the current source-level route audit
from `tests/unified_fixture_parity.rs`; it identifies the remaining categories
that can still produce a green result without proving the intended public
behavior.

Updated R0 evidence on 2026-07-11: `make -C pillow-rs-freetype route-audit`
now generates `target/api-abi-audit/route_audit.json` and
`target/api-abi-audit/route_audit.md` from the maintained public input JSON.
The report expands grouped variants into the same concrete row model used by
the unified fixture runner and classifies each row as real parity,
compile/header contract, shape-incomplete fallback, generic fallback,
null-error fallback, void fallback, explicit unsupported, or pending core work.
This is an audit report only; it does not execute fixtures, generate JSON, or
change comparisons.

Current route-audit totals:

| Route category | Concrete rows | Required disposition |
|---|---:|---|
| Real C/Rust/C-ABI/WASM parity route | 3,075 | Use these rows for structural coverage evidence. |
| Compile/header/scalar contract | 2,248 | Valid for ABI/header contracts, not runtime core coverage. |
| Shape-incomplete fallback | 38 | Convert to complete explicit variants or mark invalid/pending. |
| Generic modeled fallback | 986 | Classify operation-by-operation as real parity, unsupported, or pending. |
| Generic modeled error fallback | 145 | Replace implemented surfaces with real error-path execution. |
| Null-error fallback | 21 | Keep only exact null-handle probes; route implemented null cases directly. |
| Void fallback | 3 | Replace with real null/noop wrapper rows or classify as void API contract. |
| Explicit unsupported | 12 | Keep only where the public surface is intentionally unsupported. |
| Pending core | 3 | Convert to runnable parity when the named-instance dependencies exist. |
| Explicit unsupported stubs | 12 | Implement or keep visibly unsupported; do not count as coverage. |
| Pending core implementation | 3 | Named-instance Adobe MM, `FT_MM_Var`, and `gvar`/HVAR rows remain pending. |

The first R0 closure bucket is the 41 shape-incomplete rows because these are
usually JSON/input fixes rather than new core features:

| Operation | Rows | First action |
|---|---:|---|
| `new_memory_face` | 23 | Convert null/error variants to real memory-face rows or explicit null probes. |
| `ftoutln.outline_get_cbox` | 4 | Add glyph selectors or retire inert declarations. |
| `load_glyph` | 3 | Add concrete glyph selector rows for invalid/null flag cases. |
| `render_glyph` | 3 | Add slot/glyph selectors or classify unsupported unloaded-slot cases. |
| `freetype.request_size` | 2 | Add explicit request rows for null/error variants. |
| `freetype.set_charmap` | 2 | Add charmap selector rows or classify null-face only. |
| `ftsnames.get_sfnt_name` | 1 | Add explicit name indexes. |
| `freetype.face_set_unpatented_hinting` | 1 | Add explicit boolean state rows. |
| `load_char` | 1 | Add a concrete `char_code` or classify as null-only. |
| `sfnt.get_sfnt_table.record` | 1 | Replace the inert variation sequence with a real table-read route. |

| Route | Current behavior | Coverage risk | Required disposition |
|---|---|---|---|
| `oracle_fallback_args` default | Emits a generic FreeType error for any operation that reaches the default `_other` arm | A newly implemented public operation can still pass by agreeing with a modeled error | Every operation that reaches this path must be listed as intentionally unsupported, pending implementation, or converted to a real oracle arm |
| `oracle_fallback_args` null-operation classifier | No-font `expect_error` rows can be converted into classified null-handle errors | Valid only for pure null-handle probes; unsafe for operations whose failure depends on loaded face state | Keep only when the public C call is exactly a null-handle classification |
| No-asset non-error void route | Some null/no-asset non-error rows return `--void` / `{"void": true}` | Can hide missing wrapper behavior because no state or output is compared | Audit each row; either route through the real public wrapper or mark as a deliberately void API contract |
| Global Rust `_` fallback | Returns `FT_Err_Unimplemented_Feature` for unmatched operations | Rust core coverage cannot improve through this path and parity is only error agreement | Convert implemented operations to explicit Rust FFI handlers; leave unsupported optional modules visibly unsupported |
| C ABI / WASM `_other` fallback | Falls through to the Rust FFI runner for unsupported binding operations | Thin-wrapper coverage is not proven when the C/WASM leg never calls its public export | For every retained public C/WASM symbol, add direct wrapper execution or mark the symbol as intentionally Rust-only/test-only |
| C ABI / WASM explicit Rust delegation | Constants, layout probes, compile probes, several SFNT table routes, transforms, reference-face, unsupported stubs, size helpers, and `freetype.new_face` are routed to Rust | Acceptable for compile-time/header probes; unsafe for runtime public functions that should exercise ABI pointer handling | Split into compile-contract probes versus runtime ABI obligations; runtime functions need direct thin-wrapper rows |
| Explicit Rust unsupported stubs | `freetype.face_properties` and `freetype.select_size` return `Unimplemented_Feature` directly | These are public FreeType surfaces; final 100% correctness cannot treat them as covered behavior | Implement exact public behavior or keep manifest rows visibly pending/failing until implementation exists |
| Shape-incomplete fallback guards | `set_char_size` variants, `ftsnames.get_sfnt_name` without indexes, SFNT variation table requests, `sfnt.load_sfnt_table` missing read selectors, `sfnt.table_info` without index selectors, incomplete load/render glyph rows, and incomplete outline cbox rows intentionally fall back | These usually indicate declarative input that the runner does not execute | Convert valid rows into explicit grouped variants; remove or mark invalid row shapes rather than keeping inert declarations |
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

Primary modules: `fixed.rs`, `casts.rs`, plus residual small branches.

Expected additions: no fonts; 15-30 explicit variants.

Use existing fixed-math public API inputs for signed extremes, zero divisors,
rounding boundaries, normalization axes, and conversion limits. Remove private
conversion helpers with no production caller.

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

## Decision Log

| Date | Decision | Reason |
|---|---|---|
| 2026-07-10 | Use explicit grouped input variants only | Allows deliberate multi-input cases without hidden Cartesian growth |
| 2026-07-10 | Do not parameterize glyph-index discovery | Glyph selection must be explicit and tied to topology or behavior |
| 2026-07-10 | Measure Rust coverage only | Rust core owns behavior; C ABI and WASM ABI are thin wrappers exercised by the same parity cases |
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
| 2026-07-12 | Route rendered-slot target flags through the shared helper | `FT_Render_Glyph` already validates the public render mode before calling core rendering; preserving the selected render target in the returned slot's internal load flags keeps wrapper state centralized without adding cases or changing public output JSON |
| 2026-07-11 | Pack no-output TT guard probes into existing branch-edge glyphs | Invalid coordinate reads exercise defensive zone access while preserving the same public `FT_Load_Glyph` output and avoiding extra Cartesian case growth |
| 2026-07-11 | Prefer no-output VM state probes before new TT rows | Stack-only calls, twilight-zone movement, and no-op prep instructions can cover VM branches through the existing public `FT_Load_Glyph` row when they do not alter glyph output or weaken parity |
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
| 2026-07-11 | Treat malformed format-14 cmap records as load-time parser parity | Pinned FreeType ignores malformed optional format-14 records when another valid Unicode charmap remains usable. Public `FT_Set_Charmap` rejects format 14, so the active format-14 `FT_Get_Char_Index` and char-iteration arms remain public-unreachable rather than coverage rows to force |
| 2026-07-12 | Treat autohint script lookup coverage as explicit public rows | `script-coverage.ttf` exists to activate real `FT_LOAD_FORCE_AUTOHINT` script paths through selected Unicode codepoints. All generated script-tag glyphs are now explicit public variants; future work should add new script glyphs only when the generator grows a new documented obligation |
| 2026-07-12 | Reject parity-green rows that do not move coverage | A candidate `FT_Render_Glyph.matrix_render` SDF row using DejaVu glyph 82 at 48 ppem passed exact Rust/C/WASM parity but did not change `render.rs` or total coverage, so it was removed instead of growing the fixture count |
| 2026-07-12 | Invalidate cached render fonts on face mutations | Routing `Face::render_loaded_glyph` through `RenderFontCache` must clear cached font clones after size, charmap, or named-instance changes; otherwise a later render could reuse a stale font after the same face object mutates |
| 2026-07-11 | Add Tibetan only after the Indic CJK route fix | A candidate `script-coverage.ttf` U+0F40 Tibetan row exposed a real `FT_LOAD_FORCE_AUTOHINT` mismatch before the core fix. The row is now explicit only because Rust matches pinned C by routing `STYLE_DEFAULT_INDIC` through CJK/no-blue hinting and by not borrowing Latin `o` widths for Indic standard-character setup |
| 2026-07-11 | Match FreeType's `gasp` stream read length | Pinned `tt_face_load_gasp` seeks to the table and reads frames from the stream without using the SFNT record length as a cap. Rust must parse from the table offset to physical stream EOF for this optional table, while genuinely short physical data still degrades to `FT_GASP_NO_TABLE` |
| 2026-07-11 | Match FreeType's `post` format 2.5 tag and delta behavior | Pinned `ttpost.c` recognizes format 2.5 as `0x00025000`, computes `glyph_index + signed_delta`, and maps out-of-range results to Mac glyph index 0. Format 1.0 only returns Mac standard names when `maxp.numGlyphs == 258`; otherwise the public name stays `.notdef` |
| 2026-07-11 | Match malformed `post` public fallbacks | Pinned FreeType clears the output buffer and returns `Invalid_Argument` when an unsupported `post` format prevents `FT_HAS_GLYPH_NAMES`, while malformed format 2.0/2.5 name payloads that pass the header flag still return success with `.notdef`. Rust must keep scalar `post` metadata parsed while exposing glyph-name capability only for accepted formats 1.0, 2.0, and 2.5 |
| 2026-07-11 | Treat subglyph info as raw composite slot data | Pinned `FT_Get_SubGlyph_Info` succeeds only for a composite glyph slot with loaded subglyph records and a valid sub-index, then returns the raw component flags, args, glyph index, and 16.16 transform. Rust keeps composite flags from `glyf`, exposes them through the core glyph slot, and lets C/WASM wrappers only validate pointers and copy the core result |
| 2026-07-11 | Select named instances through face index high bits | Pinned FreeType stores a 1-based named-instance selector in bits 16..30 of `face_index`; `FT_Set_Named_Instance(0)` clears it. When an `fvar` instance lacks an explicit PostScript name ID, FreeType builds the name from nameID 25 plus a sanitized instance subfamily string |
| 2026-07-11 | Make named-instance gaps pending instead of fallback-green | `ftmm.set_named_instance` previously appeared green through the generic modeled-error path. Direct oracle routing proves the compact success/error rows and leaves Adobe MM reset, `FT_MM_Var` namedstyle coordinates, and `gvar`/HVAR glyph-output deltas visible until the core implementation exists |
| 2026-07-11 | Compare structured error output only by explicit opt-in | Existing expected-error rows intentionally tolerate several Rust/C error-classification differences. Rows that claim post-error state preservation, such as invalid named-instance selection, must set `compare_error_output` and provide matching C oracle, Rust, C ABI, and WASM ABI state snapshots |
| 2026-07-11 | Prefer shared table readers over duplicated byte decoding | Reusing existing SFNT endian helpers is valid coverage progress when the public parser already reads the same field. It does not remove behavior or add a fake test path, and keeps coverage tied to real public fixture execution |
| 2026-07-11 | Classify fvar instance-count overflow as unreachable | `instance_count` and `instance_size` are 16-bit SFNT fields, so their product fits in `usize` on supported 32-bit and 64-bit targets. Keep the defensive guard visible for now instead of deleting it to manufacture line coverage |
| 2026-07-11 | Match variation PostScript prefix platform filtering | Pinned `sfnt_get_var_ps_name` calls `sfnt_get_name_id`, which accepts only Windows 3/0, Windows 3/1, and Apple Roman records for the variation prefix. It does not use the broader Unicode/ISO fallback from `tt_face_get_name`; the named-instance subfamily still uses that general lookup path |
| 2026-07-11 | Match missing-subfamily named-instance synthesis | Pinned `sfnt_get_var_ps_name` falls through to `construct_instance_name` when a named instance has no explicit PostScript name and no usable subfamily name. The fallback appends each non-default fvar coordinate as a shortest 16.16 decimal followed by sanitized axis-tag characters |
| 2026-07-11 | Treat route-audit shape as the explicit row contract | `FT_Request_Size` variants are maintained parser rows, and null-face `FT_Set_Charmap` rows still need an explicit selector shape. Audit classification must mirror the maintained runner contract instead of leaving real parity rows in shape fallback |

## Immediate Next Actions

Work must resume here unless a newer user request changes priority:

1. Remove false-green public adapters before adding more coverage-only rows.
   `FT_Get_Glyph_Name`, `FT_Get_Name_Index`, `FT_Get_Gasp`,
   `FT_Get_CMap_Format`, `FT_Get_CMap_Language_ID`, and
   `FT_Get_SubGlyph_Info`, `FT_Get_Postscript_Name`, and
   `FT_Set_Named_Instance`-driven named-instance selection are now real parity;
   continue with generic fallback rows and any remaining modeled public
   surfaces.
2. Complete R0 and classify every uncovered function as public, font-reachable,
   missing delegation, blocked by incomplete implementation, duplicate with
   independent proof, or currently unreachable but preserved.
3. Resume explicit fixture expansion in the active order: public route audit,
   render/raster matrix, autohint script/topology, TrueType interpreter edge
   programs, then scalar residuals.
4. Keep the current three named-instance pending rows explicit and do not count
   them as coverage until the core Adobe MM, `FT_MM_Var`, and `gvar`/HVAR
   behavior exists. Embedded-strike request handling remains an R7 fixture
   obligation, but it is not currently a pending runtime row.
5. Keep the deprecated corpus isolated until final cleanup is separately
   reviewed and approved.
