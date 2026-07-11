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
| VM fetch helpers, round-mode conversion, fpgm/prep helpers, and `CallRecord` fields | restored | public helpers and record contracts must not change for coverage |
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

The 12 pending cases are existing unsupported or unresolved inputs. They must
remain visible and be converted to runnable explicit cases during the coverage
phases where their owning operations are addressed.

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

Evaluation checkpoint: 2026-07-11, commit `98d75812` plus pending input
normalization.

The current unified public API suite has 4,110 logical cases, 6,481 concrete
explicit cases, 6,481 runnable exact-parity cases, zero implicit cases, and
zero pending cases. The previous `freetype.request_size:first_available_size`
pending row now uses an explicit 26.6 height value and participates in the
normal Rust/C ABI/WASM parity comparison. Core Rust structural coverage is:

| Measure | Covered | Total | Remaining |
|---|---:|---:|---:|
| Functions | 770 | 990 | 220 |
| Lines | 13,377 | 16,371 | 2,994 |
| Regions | 19,335 | 23,406 | 4,071 |
| Branches/conditions | 3,254 | 4,168 | 914 |

The remaining regions and branches divide exactly into five ownership groups:

| Group | Modules | Missing functions | Missing lines | Missing regions | Missing branches | Primary action |
|---|---|---:|---:|---:|---:|---|
| Autohint | `autohint/latin`, `cjk`, `globals*`, `script`, `coverage`, `types`, `loader` | 49 | 1,372 | 1,913 | 674 | reachability audit, then topology/script fonts |
| Rendering | `render`, `grays`, `outline` | 64 | 1,021 | 1,442 | 227 | reachability audit, then geometry and render-mode matrix |
| Face/API/scaler | `font`, `scaler`, `api`, `ffi/handles`, `ffi/convert`, `ffi/types` | 127 | 1,151 | 1,336 | 188 | public-operation routing, wrapper cleanup, focused state inputs |
| TrueType interpreter | `tt/hinter/exec`, `gs`, `zone`, `mod`, `iup` | 11 | 381 | 853 | 203 | explicit bytecode-program glyph matrix |
| Math/casts | `fixed`, `casts` | 6 | 29 | 34 | 14 | scalar boundary inputs or dead-helper removal |

This concentration changes the execution strategy. More fonts alone cannot
close the report. Entire modules such as `autohint/script.rs` and
`autohint/coverage.rs`, plus many convenience methods in `font.rs`,
`api.rs`, and `render.rs`, have no covered functions. Each must first be
classified as one of:

1. Required behavior already exposed by a manifest public operation.
2. Required behavior whose existing public operation does not yet delegate to
   the core implementation.
3. Private behavior reachable only after a missing font/table/glyph property is
   supplied.
4. Duplicate, diagnostic-only, test-only, or obsolete code that must be removed
   or feature-gated rather than artificially called.

No fixture is accepted for category 4. No new fixture test, JSON generator,
runtime discovery, glyph-index scan, or Cartesian axis is allowed.

### Remaining Case And Font Budget

The completion budget is deliberately conservative:

| Resource | Current | Maximum addition | Completion ceiling |
|---|---:|---:|---:|
| Concrete explicit cases | 6,481 | 433 | 6,914 |
| New semantic font files | 0 | 6 | 6 |
| New glyph programs/topologies | 0 | 160 | 160 |
| Implicit cases | 0 | 0 | 0 |
| Pending cases | 0 | 0 | 0 |

The 500-case allowance is a ceiling, not a target. A batch must justify every
variant by a named uncovered behavior. Existing focused fonts should be
extended before creating a new content identity.

At completion, consolidate the current 81 active unique font contents toward no
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

### Ordered Remaining Batches

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

Status: pending real fixed-strike implementation. The single symbolic
`first_available_size` expression has been replaced with an explicit value, so
the suite has zero pending cases and exact parity for the current request-size
row. This does not complete embedded-strike parity: the current asset is still a
scalable-font alias, so real fixed-strike support and successful/unavailable
strike variants remain required. Do not substitute a scalable font for the final
R7 exit gate.

Exit gate: 0 pending cases and exact Rust/C/WASM strike parity.

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
| 2026-07-11 | Rust public set-char-size wrapper routing | 81 unique hashes | 0 | 6,481 | 6,480 / 6,480 | 1 | 13,377 / 16,371 lines; 19,335 / 23,406 regions; 3,254 / 4,168 branches | successful `FT_Set_Char_Size` fixtures route the Rust leg through `Face::set_char_size`; exact Rust/C/WASM parity remains green, `api.rs` reaches 318 / 401 lines and 37 / 45 functions, and the current condition run also exposes `autohint/script.rs` in the denominator |

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
| 2026-07-10 | Require every predicate operand outcome | Line execution alone missed non-Roman Mac and non-Windows fallback records; nightly branch coverage makes both sides of each short-circuit condition visible |
| 2026-07-10 | Treat TTC table offsets as collection-absolute | Pinned `tt_face_load_font_dir` reads table offsets from the TTC stream origin; adding the selected face base a second time breaks every nonzero face |
| 2026-07-10 | Keep the embedded-strike request visibly pending | Existing bitmap-named aliases are scalable fonts and core has no embedded-strike table support; substituting a numeric size would falsely satisfy the manifest obligation |
| 2026-07-10 | Model cmap `char_next` per format | Pinned format 6 increments before its terminal check and wraps at `0xFFFFFFFF`; formats 4 and 12 reject their terminal inputs before advancing |
| 2026-07-10 | Validate a composite tree once before no-hint scaling | The public scaler always calls `load_glyph` first; the scaled helper consumes that validated tree and must not retain public-unreachable duplicate malformed-data branches |
| 2026-07-10 | Validate whole loca records | A single checked 4-byte or 8-byte slice expresses FreeType's truncated-record failure without byte-by-byte optional indexing or twelve redundant fonts |
| 2026-07-10 | Keep raw fpgm/prep storage direct while preserving helpers | Font construction consumes raw byte streams directly, but the existing public copy helpers remain available and visibly uncovered rather than being deleted for coverage |
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
| 2026-07-11 | Route null lifecycle fixtures through thin Rust FFI | Existing lifecycle fixtures should execute the same thin Rust FFI handlers as C/WASM for handle validation coverage; modeled error shortcuts are only for surfaces without a maintained direct Rust handler |
| 2026-07-11 | Rebase worker glyph additions onto current fixture glyph order | Worker font-source changes must preserve all previously merged glyph roles; the TT branch-edge glyph moved from id 48 to id 51 because render coverage already owns glyphs 48-50 |
| 2026-07-11 | Keep exact boundary rows even when broader guards are already covered | The one-past-head-table SFNT row adds no new structural counters after executable offset coverage, but it preserves a precise public boundary case from the metadata worker without multiplying unrelated inputs |
| 2026-07-11 | Preserve render-load slot semantics in public wrapper coverage | `Face::render_loaded_glyph` strips `FT_LOAD_RENDER` before loading, while C `FT_Render_Glyph` returns an already-rendered bitmap slot unchanged; public wrapper routing must therefore fall back to the FFI-shaped path for rows whose load flags already render |
| 2026-07-11 | Treat current coverage denominator as authoritative | A fresh non-incremental condition-coverage build lists `autohint/script.rs` as uncovered source; keep it visible as a real obligation instead of relying on stale incremental coverage output |

## Immediate Next Actions

Work must resume here unless a newer user request changes priority:

1. Complete R0 and classify every uncovered function as public, font-reachable,
   missing delegation, duplicate with independent proof, or currently
   unreachable but preserved.
2. Continue R5 from the remaining uncovered render and gray-raster branches;
   add cubic/CFF, clipping, dropout, and empty/error roles only as explicit
   variants with measured structural gain.
3. Resolve the one visible embedded-strike pending case in R7 when its focused bitmap
   font and owning core table support are implemented.
4. Keep the deprecated corpus isolated until final cleanup is separately
   reviewed and approved.
