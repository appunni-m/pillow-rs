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
- Lines that cannot be reached through a supported public operation are
  removed or refactored. They are not hidden with coverage exclusions.
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

Evaluation checkpoint: 2026-07-10, commit `2366de79`.

The current unified public API suite has 4,110 logical cases, 6,414 concrete
explicit cases, 6,413 runnable exact-parity cases, zero implicit cases, and one
pending embedded-strike case. Core Rust structural coverage is:

| Measure | Covered | Total | Remaining |
|---|---:|---:|---:|
| Functions | 732 | 989 | 257 |
| Lines | 12,367 | 16,321 | 3,954 |
| Regions | 17,729 | 23,307 | 5,578 |
| Branches/conditions | 2,858 | 4,164 | 1,306 |

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
| Concrete explicit cases | 6,414 | 500 | 6,914 |
| New semantic font files | 0 | 6 | 6 |
| New glyph programs/topologies | 0 | 160 | 160 |
| Implicit cases | 0 | 0 | 0 |
| Pending cases | 1 | 0 | 0 |

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

Status: in progress. The first autohint audit removed 15 uncovered functions:
the abandoned diagnostic bitmask, a duplicate script detector, and an unused
blue-character lookup. The retained runtime now has one script-selection path
through `FaceGlobals`, `STYLE_TABLE`, and `globals::detect_script`.

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

Before adding geometry, remove alternate render implementations and convenience
entry points that no public operation can call.

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

Implement the required bitmap table support in pure Rust, then replace the
single pending `first_available_size` expression with explicit successful and
unavailable-strike inputs. Do not substitute a scalable font.

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
| 2026-07-10 | Keep raw fpgm/prep storage direct | Font construction already copies these byte streams; unused parser wrappers added functions without behavior and were removed instead of fixture-covered artificially |
| 2026-07-10 | Keep scan conversion controls in one program font | Empty setup and scan-type variants share tables, geometry, size, and flags; only explicit glyph programs differ, avoiding a font/size/flag product |
| 2026-07-10 | Hash resolved assets in the C-oracle cache key | JSON paths do not identify mutable fixture contents; path, length, and SHA-256 now prevent stale C output after in-place font mutation |
| 2026-07-10 | Remove abandoned autohint coverage surfaces | Runtime script selection already uses `FaceGlobals` and `STYLE_TABLE`; an unread diagnostic mask, duplicate detector, and no-caller blue-character table cannot be justified by public fixtures |
| 2026-07-10 | Store resolved LOOPCALL definition coordinates in call records | A loop call record is created only after resolving an active FDEF; retaining its range/start removes an impossible missing-definition re-lookup and keeps malformed definitions rejected at the actual resolution boundary |

## Immediate Next Actions

Work must resume here unless a newer user request changes priority:

1. Complete R0 and classify every uncovered function as public, font-reachable,
   missing delegation, or removable.
2. Execute R1 by extending the existing hinter control font with explicit
   opcode-family glyph programs.
3. Resolve the one visible embedded-strike pending case in R7 when its focused bitmap
   font and owning core table support are implemented.
4. Keep the deprecated corpus isolated until final cleanup is separately
   reviewed and approved.
