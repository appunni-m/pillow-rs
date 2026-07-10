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

## Execution Phases

### Phase 0: Preserve The Explicit Baseline

Status: complete.

- Replaced implicit runtime Cartesian expansion with explicit grouped variants.
- Removed runtime folder discovery and all-glyph enumeration from public inputs.
- Established content-hashed font cache identity.
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

Status: pending.

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

1. Convert each of the 12 pending cases into a runnable explicit input where the
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

## Immediate Next Actions

Work must resume here unless a newer user request changes priority:

1. Move to the next highest-yield uncovered module after `tt/cmap.rs` and
   `tt/glyf.rs`, both of which now have complete structural coverage.
2. Classify its uncovered regions and condition outcomes by required font,
   glyph topology, parameter, or error-path property.
3. Extend focused fonts only for missing properties proven by coverage, then
   add complete explicit variants to the existing public manifest case.
4. Resolve the one visible embedded-strike pending case when its focused bitmap
   font and owning core table support are implemented.
5. Keep the deprecated corpus isolated until final cleanup is separately
   reviewed and approved.
