# Fontdone coverage batch audit — 2026-08-11

This is a documentation-only audit performed in the isolated checkout
`/private/tmp/pillow-rs-fontdone-coverage-audit`, branch
`codex/fontdone-coverage-audit`, based on Pillow-RS commit `9742b4d53`. It does
not change Pillow-RS runtime code or the sibling
`/Users/lazytrot/work/fontdone` checkout.

Paths beginning with `fontdone/` below are relative to
`/Users/lazytrot/work/fontdone`. Source and manifest line ranges are exact for
the inspected checkout.

## Evidence boundary and freshness

The sibling fontdone checkout was clean at source `HEAD`
`fdc509f3dcca3ae1bb65c4e0dcfc83c8df78b432` when inspected. The exact
source-matched production evidence is the generated LLVM report
`/Users/lazytrot/work/fontdone/target/coverage/unified-runtime-all-lanes.json`.
Its build marker is `251c26b8e661b11634ee11245c5f8a84e5de9599-clean`. The
following read-only source-equivalence check returned exit 0:

```text
git diff --quiet 251c26b8e661b11634ee11245c5f8a84e5de9599..HEAD -- \
  src/pfr.rs src/tt/svg.rs src/tt/sbix.rs src/tt/sbit.rs \
  src/autohint/cjk.rs src/autohint/coverage.rs src/autohint/globals.rs \
  src/autohint/latin.rs src/autohint/loader.rs src/autohint/types.rs \
  src/api.rs src/font.rs src/grays.rs src/ffi/handles.rs \
  src/scaler.rs src/render.rs src/tt/cff.rs src/tt/cmap.rs \
  src/tt/name.rs src/tt/post.rs src/tt/fvar.rs src/tt/gvar.rs \
  src/tt/hvar.rs src/tt/mvar.rs src/tt/varstore.rs
```

Therefore the report’s per-file production metrics below are source-matched
to current fontdone `HEAD`. The report’s whole-project total also includes an
older test-harness/preparation state, so it is not presented as a current
`HEAD` aggregate denominator.

| Evidence | Result and limitation |
|---|---|
| `fontdone/target/coverage/unified-runtime-all-lanes.json` plus `.build-state` and preparation state | Source-matched production file evidence: report totals are 51,839/54,811 lines (94.58%), 10,383/12,598 branches (82.42%), 3,516/3,847 functions (91.40%), and 71,470/75,922 regions (94.14%), but those aggregate totals are not a current-`HEAD` harness baseline. |
| `fontdone/doc/compatibility_snapshot.json` | Historical committed snapshot at source `06a8ce2b91d041d1bc75618e579480d11f182dcd`, run `669d796d-c1aa-411c-ad12-98a6d2089fc0`; not used as current coverage evidence. |
| `fontdone/doc/runtime_parity_evidence.json` (`evidence_kind=full_runtime_parity`) | Historical recorded parity at source `2330ee507a9de66c504ded2c3a6ccde0fd3dce57`: 7,853 runnable, 7,853 passed, 0 failed, 3 pending; not a fresh run at current sibling `HEAD`, and not Pillow parity. |
| `fontdone/doc/FREETYPE_SUPPORT.md`, `fontdone/doc/DEVELOPMENT.md`, and `fontdone/doc/ROADMAP.md` | Current source-adoption classifications, maintained probe descriptions, and source-matched historical coverage decisions. |

The auto-selected Pillow operation snapshot was deliberately not used as a
fontdone baseline. No crash, GPU, or pending-TIFF lane was run or treated as
evidence.

Coverage is not parity: executing a line or branch does not prove that its
result matches C. The LLVM report’s line/region/function summaries are used for
the exact file metrics below; raw segment records are not promoted to
line-level gap claims. No fontdone test, parity, coverage, or fixture command
was run for this audit.

## Source-matched production metrics

These are LLVM executable-line, region, branch, and function summaries from
the report above. They are not physical `wc -l` counts and are not a public
parity score.

| Area | Lines | Regions | Branches | Functions |
|---|---:|---:|---:|---:|
| `src/pfr.rs` | 285/285 | 550/627 | 70/70 | 23/23 |
| `src/tt/svg.rs` | 59/59 | 104/104 | 14/14 | 6/6 |
| `src/tt/sbix.rs` | 146/153 | 206/235 | 22/22 | 13/20 |
| `src/tt/sbit.rs` | 785/826 | 1,142/1,295 | 96/96 | 44/85 |
| `src/autohint/cjk.rs` | 858/879 | 1,152/1,180 | 372/398 | 18/18 |
| `src/autohint/coverage.rs` | 28/28 | 35/35 | 4/4 | 7/7 |
| `src/autohint/globals.rs` | 272/272 | 444/446 | 88/88 | 18/18 |
| `src/autohint/latin.rs` | 2,891/2,978 | 4,189/4,299 | 1,142/1,269 | 67/69 |
| `src/autohint/loader.rs` | 229/229 | 404/404 | 64/64 | 5/5 |
| `src/autohint/types.rs` | 102/102 | 69/69 | 0/0 | 14/14 |
| `src/scaler.rs` | 1,448/1,497 | 1,572/1,618 | 218/246 | 70/72 |
| `src/render.rs` | 2,353/2,463 | 3,305/3,441 | 423/486 | 144/159 |
| `src/tt/cff.rs` | 883/913 | 1,261/1,363 | 132/132 | 67/94 |
| `src/tt/cmap.rs` | 853/853 | 1,126/1,128 | 174/174 | 62/62 |
| `src/tt/name.rs` | 374/374 | 517/518 | 124/124 | 38/38 |
| `src/tt/post.rs` | 96/97 | 184/190 | 23/24 | 7/7 |
| `src/tt/fvar.rs` | 99/99 | 130/130 | 20/20 | 2/2 |
| `src/tt/gvar.rs` | 577/582 | 938/965 | 126/126 | 46/51 |
| `src/tt/hvar.rs` | 45/47 | 70/79 | 6/6 | 5/7 |
| `src/tt/mvar.rs` | 64/67 | 97/113 | 6/6 | 4/7 |
| `src/tt/varstore.rs` | 243/252 | 419/452 | 66/66 | 14/20 |

The adjacent dispatch/ABI files are also source-matched, but their large
denominators must not be treated as Pillow public-operation gaps:

| Context file | Lines | Regions | Branches | Functions |
|---|---:|---:|---:|---:|
| `src/api.rs` | 1,311/1,360 | 1,902/1,966 | 273/326 | 112/112 |
| `src/font.rs` | 5,900/6,307 | 8,417/9,146 | 809/962 | 482/587 |
| `src/grays.rs` | 806/839 | 1,099/1,131 | 179/190 | 35/36 |
| `src/ffi/handles.rs` | 11,221/12,073 | 15,247/16,555 | 2,175/2,865 | 817/881 |

These files mix public route dispatch, ABI record/lifetime handling, and
defensive validation. Any remaining ABI gap requires an existing manifest route
or a separately authorized ABI contract case; null-pointer fabrication,
allocator tricks, and direct private-handle calls are not safe coverage inputs.

## Classified count

The requested scope contains **9/9 lane families classified**. The
classifications intentionally overlap by lane, so this audit does not invent a
single additive “remaining gap” count. Five families have source-backed public
input follow-up candidates (SBIT, autohint, scaler/render, CFF, and
variations); PFR and SVG have no remaining executable line/branch/function
gap; SBIX and CMap/SFNT are constrained by decoder or adoption status. The
remaining records are split between public follow-up candidates,
unsupported/planned surfaces, defensive invariants, and one coverage-only
helper.

| Classification | Count | Meaning |
|---|---:|---|
| Source-backed public follow-up | Five lane families | A maintained byte/input-only route can reach the public operation or its public Rust equivalent without unsafe memory fabrication. |
| Unsupported/planned or not application-ready | Lane-specific | The route is decoder-limited or marked `Partial`, `Planned`, or `Implemented; mapping incomplete` by the adoption contract. |
| Defensive/unreachable | Lane-specific | A guard is caller-proven, parser-invariant, architecture-specific, or removed as unreachable; manufacturing invalid internal state is not a safe probe. |
| Coverage-only non-public helper | One helper | A direct synthetic helper exists only to close a measured implementation branch and must not be counted as public parity coverage. |

## Lane findings and candidate probes

### 1. PFR

**Observed evidence.** PFR parsing and the internal metric/advance/kerning
service are in `fontdone/src/pfr.rs:47-389`; PFR face construction is
`fontdone/src/font.rs:3464-3534`; the public C-ABI routes are
`fontdone/src/ffi/handles.rs:14582-14713`. The working report summarizes
`pfr.rs` at 285/285 lines, 70/70 branches, and 23/23 functions (550/627
regions). Existing valid and malformed inputs are under
`fontdone/tests/fixtures/input/fonts/pfr/`.

**Classification: no remaining executable-line, branch, or function gap.** The
source-matched report has only region-only residuals, and the current roadmap
records the PFR malformed-input cluster as having no relevant line or branch
gaps (`fontdone/doc/ROADMAP.md:553`). The manifest names
`FT_Get_PFR_Advance`,
`FT_Get_PFR_Kerning`, and `FT_Get_PFR_Metrics` cases at
`fontdone/tests/manifest.yaml:9979-10019`. The support contract marks all
three `Complete` at `fontdone/doc/FREETYPE_SUPPORT.md:231-236`.

**Optional public evidence only.** If a later fontdone parity task needs to
explain the remaining region-only records, open `basic-metrics-and-kerning.pfr`
or
`fixed-advance.pfr` through the memory-face route, then probe the three
services with a valid glyph/kerning pair and the optional output combinations;
use a non-PFR face as the ordinary error/fallback control. Dependencies are a
real PFR face, a valid glyph index, and output buffers only where the case
contract requires them. This is safe because all data are bounded fixture
bytes and results are returned as values/errors. Null-handle/output ABI rows
are separate validation cases; they were not run because this audit excludes
crash-oriented lanes.

The malformed PFR corpus already covers header, directory, resolution,
descriptor, and item truncation errors. Do not invent extra internal parser
states merely to consume the remaining region count.

### 2. SVG

**Observed evidence.** The parser is `fontdone/src/tt/svg.rs:34-120` and is
fully summarized at 59/59 lines, 104/104 regions, 14/14 branches, and 6/6
functions. Table
integration is `fontdone/src/font.rs:3836-3838`; the public load-side document
route is `fontdone/src/font.rs:5784-5815`. Maintained malformed inputs are
under `fontdone/tests/fixtures/input/fonts/svg/`.

**Classification: parser coverage complete; SVG/color rendering remains
unsupported or not application-ready.** `FT_LOAD_NO_SVG` and `FT_LOAD_SVG_ONLY`
are specified at `fontdone/tests/manifest.yaml:946-1002`, but both need an
OpenType font containing an SVG glyph and a fallback/non-SVG glyph. The
adoption contract explicitly excludes SVG/color compositing because the
renderer is a monochrome alpha-mask renderer (`fontdone/doc/FREETYPE_SUPPORT.md:61-69`).

No coverage-fixing case is justified by the current source-matched report.
If a future parity task expands SVG behavior, use `otsvg-glyph.ttf` and
`otsvg-glyph-range-gap.ttf` to
open a real face and compare `FT_Load_Glyph` with the default, `NO_SVG`, and
`SVG_ONLY` flags, including a glyph in the range gap. Use the short,
out-of-bounds, truncated-record, and gzip fixtures for face-open errors. The
dependencies are a valid SVG table, glyph IDs, and the normal face/load
lifecycle; no renderer or external SVG library is needed for parser/error
coverage. This is safe as fixture input, but it must be reported as partial
SVG behavior rather than complete color-glyph parity.

Do not call `SvgTable::read_u16`/`read_u32` directly (`svg.rs:110-120`) to
manufacture bounds failures; their callers establish the table bounds.

### 3. SBIX

**Observed evidence.** SBIX parsing and strike metadata are
`fontdone/src/tt/sbix.rs:32-99`; glyph error/decoder dispatch is
`fontdone/src/tt/sbix.rs:101-202`. Table integration is
`fontdone/src/font.rs:3830-3835`, and `FT_Select_Size` dispatch is
`fontdone/src/font.rs:5116-5164`. The working report summarizes 146/153 lines,
22/22 branches, and 13/20 functions. Existing SBIX inputs are under
`fontdone/tests/fixtures/input/fonts/sbix/`.

**Classification: public strike/error probes are covered; bitmap decoding is
unsupported; one internal precondition is defensive/unreachable.** The
maintained coverage-only `FT_Select_Size` probe is public Rust API usage at
`fontdone/tests/unified_fixture_parity.rs:112-133`, invoked from the existing
case at `fontdone/tests/unified_fixture_parity.rs:51588-51599`; it exercises
the valid and invalid strike indices for `sbix-bitmap-only.ttf`. The broader
`FT_Load_Glyph` SBIX error matrix is described at
`fontdone/tests/manifest.yaml:1731-1756`.

The current roadmap records `sbix.rs` at 22/22 branches with no relevant gaps
after the strike-invariant cleanup (`fontdone/doc/ROADMAP.md:551`). The
remaining 7 executable lines, 29 regions, and 7 functions are not sufficient
evidence of a missing public route: they include decoder/error variants and
the already-proven strike precondition. A later parity task may open
`sbix-bitmap-only.ttf`, select strike 0 and an
invalid strike, then load present/missing glyphs through `FT_Load_Glyph` and
the documented load flags. Add or retain PNG/JPEG/TIFF/`rgbl`/unknown-type
fixtures only as expected decoder errors: the Rust implementation deliberately
does not claim those external bitmap decoders. The public dependencies are a
real face, selected ppem, glyph index, and bounded table bytes.

The selected-strike precondition in `sbix.rs:101-120` is caller-proven by the
face load path. It is not a safe public gap: do not directly invoke the
internal loader with an invalid selected strike or treat the `unreachable!`
arm as a missing probe.

### 4. SBIT / EBLC / CBLC

**Observed evidence.** SBIT table parsing and public load entry points are
`fontdone/src/tt/sbit.rs:206-305`; strike lookup, simple/bit-aligned/compound
loads, and blitting are `fontdone/src/tt/sbit.rs:307-1085`. The face-level
missing-glyph fallback is `fontdone/src/font.rs:5698-5781`. The working report
summarizes 785/826 lines, 96/96 branches, and 44/85 functions. The maintained
SBIT corpus under `fontdone/tests/fixtures/input/fixtures/assets/fonts/`
contains gray, mono, packed, BGRA, compound, missing, invalid, truncated,
out-of-bounds, unsupported-depth, and no-matching-strike cases.

**Classification: reachable public bitmap probes; architecture-specific
overflow arms are defensive/unreachable for this measured target.** The
existing CBLC missing-glyph probe is an input-only public route at
`fontdone/tests/unified_fixture_parity.rs:135-155`: it opens
`sbit_cblc_cbdt_gray_format1.ttf`, sets 20x20, loads glyph 0 with
`SBITS_ONLY`, and checks the zero-sized bitmap fallback. It is invoked by the
existing `FT_Select_Size` case at
`fontdone/tests/unified_fixture_parity.rs:51588-51599`. The public SBIT-only
manifest route is `fontdone/tests/manifest.yaml:980-990`.

The current roadmap records 96/96 SBIT branches after the shape-invariant
cleanup (`fontdone/doc/ROADMAP.md:550`). Remaining line, region, and function
records are therefore not automatically public gaps; they include helper
variants and unsupported/defensive paths. A later parity task may use
`FT_Select_Size`/`FT_Load_Glyph` with `SBITS_ONLY` on
the maintained strike fixtures, varying a real strike, a hit, a missing glyph,
and a no-matching-strike size. Compare bitmap bytes, dimensions, pitch,
pixel mode, and metrics. This is safe because the input is a complete font and
all decoding stays inside the normal face lifecycle.

The checked-arithmetic arms in `sbit.rs:7-53` are only compiled for the
non-64-bit path; forcing an artificial overflow through a direct internal
call would not be a valid public probe. The earlier bitmap-shape/strike
defensive guards were removed after caller proofs while preserving parity, as
recorded in `fontdone/README.md:196-213`.

### 5. Autohint

**Observed evidence.** The measured implementation files are
`fontdone/src/autohint/cjk.rs`, `coverage.rs`, `globals.rs`, `latin.rs`,
`loader.rs`, and `types.rs`; the working summaries are respectively 858/879,
28/28, 272/272, 2,891/2,978, 229/229, and 102/102 lines. The public load
flag cases are `fontdone/tests/manifest.yaml:818-832` for FORCE_AUTOHINT and
`:882-892` for NO_AUTOHINT. The maintained autohint fixture corpus is under
`fontdone/tests/fixtures/input/fonts/autohint/`.

**Classification: residual public script/size paths plus one non-public
coverage-only helper.** The existing synthetic Latin segment probe is
`fontdone/tests/unified_fixture_parity.rs:72-110`, called only for the
U+0245 target-mono case at `:4829-4850`. It reaches a measured segment-merge
branch but does not add a parity case or public API evidence. Coverage-mask
reset/assert handling is `:65866-65930`.

The public residuals are valid candidates for a separately authorized
fontdone parity batch. Add or retain maintained public `FT_Load_Char`/
`FT_Load_Glyph` inputs across the Latin, CJK, Indic, Arabic, and mixed-script
fixtures, varying size, target mode, FORCE_AUTOHINT, and NO_AUTOHINT. Compare
the complete glyph slot/bitmap/metrics output and, where deliberately
specified, the autohint coverage mask. Dependencies are a real glyph and
font/size configuration; no synthetic `GlyphHints` state is needed for public
parity. Do not count the direct helper as a public probe or use it to justify a
Pillow parity claim.

### 6. Scaler and render

**Observed evidence.** Scaler public dispatches are
`fontdone/src/scaler.rs:230-732` with implementation in `:734-1433`; the
working report summarizes 1,448/1,497 lines, 218/246 branches, and 70/72
functions. Render-mode dispatch and loaded-outline rendering are
`fontdone/src/render.rs:193-324`; normal, overlap, SDF, bitmap-SDF, and MONO
paths begin at `:391-671`, with the working summary at 2,353/2,463 lines,
423/486 branches, and 144/159 functions.

**Classification: reachable public mode probes remain; LCD is partial; excluded
lanes remain out of scope.** `FT_Render_Glyph` is `Complete` in the adoption
contract (`fontdone/doc/FREETYPE_SUPPORT.md:93-105`); LCD filter/geometry
operations are `Partial` at `:167-169`.

**Safe candidate.** Use maintained `FT_Load_Glyph`/`FT_Load_Char` cases with
default, no-hinting, FORCE_AUTOHINT, no-scale, RENDER, MONO, LCD, and LCD_V
flags where the manifest supplies them. Use cubic CFF inputs for the CFF
scaler path and compare exact outline/bitmap bytes, metrics, and bbox/cbox
outputs. Dependencies are normal face/size/glyph inputs and the existing
renderer; no GPU lane is required. Remaining uncovered helper regions are not
automatically unsupported—there must be a public route or a source-level
invariant before adding a probe.

### 7. CFF / CFF2 / CID

**Observed evidence.** CFF/CFF2 table parsing is
`fontdone/src/tt/cff.rs:66-221`; top-dict/charset/index/dictionary parsing and
Type 2 decoding cover `:224-1565`. The working report summarizes 883/913
lines, 132/132 branches, and 67/94 functions. CFF/CFF2 fixtures, including
pure CFF cubic, CFF2, CID, malformed header/index/dict/charset/charstring,
and ROS cases, are under `fontdone/tests/fixtures/input/fonts/cff/`.

**Classification: reachable public face-open/load probes; internal malformed
decoder states are not a public coverage obligation.** The branch summary is
complete (132/132), while the remaining function/region records need a public
route before they become an actionable coverage item. `FT_Load_Glyph` is
`Complete` (`fontdone/doc/FREETYPE_SUPPORT.md:88-95`), and the CID metadata
functions are `Complete` at `:231-233`.

**Safe candidate.** Open valid pure CFF, CFF2, and CID fixtures through the
memory-face API, then load a known glyph at a real size and query the mapped
CID metadata. Feed malformed fixtures only through face open and assert the
documented error; do not call `Type2Decoder` or dictionary helpers directly
with fabricated offsets. Recent CFF operand guards were removed as
unreachable after parser/caller proofs, so their absence is not a missing safe
probe.

### 8. CMap and SFNT metadata/tables

**Observed evidence.** CMap public methods are
`fontdone/src/tt/cmap.rs:124-300`; format implementations and parsing are
`:302-1114`. The working report is effectively complete for CMap: 853/853
lines, 174/174 branches, and 62/62 functions. Format-14 face methods are
`fontdone/src/font.rs:5337-5374`; the manifest matrix is
`fontdone/tests/manifest.yaml:1348-1428`. Name/post metadata is in
`fontdone/src/tt/name.rs` (374/374 lines, 124/124 branches, 38/38 functions)
and `fontdone/src/tt/post.rs` (96/97 lines, 23/24 branches, 7/7 functions).
SFNT table methods are `fontdone/src/font.rs:5376-5443`.

**Classification: CMap parser coverage is effectively complete; public SFNT
name/language mapping is planned or incomplete.** `FT_Get_CMap_Format`, `FT_Sfnt_Table_Info`, and
`FT_Load_Sfnt_Table` are `Complete`, while `FT_Get_Sfnt_Table` is `Partial`;
the adoption statuses are `fontdone/doc/FREETYPE_SUPPORT.md:170-177`.
`FT_Get_CMap_Language_ID` and the SFNT name/lang APIs remain `Planned` at
`:171-174`, even though their needs-input manifest cases are present at
`fontdone/tests/manifest.yaml:10184-10270`.

**Safe candidate.** Use the format-4/6/12/13/14 fixtures, including malformed
format-13/14 matrices and non-Unicode/platform variants, through the public
charmap and variant-index APIs. Use valid and truncated SFNT table fixtures
through `sfnt_table_info` and `load_sfnt_table`; use indexed name records only
if the corresponding mapping contract is intentionally expanded. Dependencies
are a concrete font, selected charmap, character/variation-selector values,
and bounded table/name output. Do not label a passing internal name parser as
application-ready SFNT-name parity.

### 9. Variations

**Observed evidence.** FVAR/HVAR/MVAR integration is
`fontdone/src/font.rs:3818-3825`; FVAR parsing is in
`fontdone/src/tt/fvar.rs:7-130`; GVAR, HVAR, MVAR, and varstore summaries in
the working report are 577/582, 45/47, 64/67, and 243/252 lines. Their branch
summaries are 126/126, 6/6, 6/6, and 66/66 respectively. Core named-instance
and coordinate handling begins at `fontdone/src/font.rs:3999-4032`; public ABI
entry points are grouped in `fontdone/src/ffi/handles.rs:12495-13085` and
variant-selector entry points at `:14490-14537`. Variable fixtures are under
`fontdone/tests/fixtures/input/fonts/variable/`.

**Classification: reachable variable-font input probes; most variation ABI
operations are planned; varstore invalid arithmetic is defensive/unreachable.**
`fvar.rs` and all variation branch summaries are complete in the source-matched
report; residual line/function/region records are concentrated in `gvar`,
`hvar`, `mvar`, and `varstore` helpers. The adoption contract marks the
format-14 variant APIs
`Implemented; mapping incomplete` and most MM/variation APIs `Planned`
(`fontdone/doc/FREETYPE_SUPPORT.md:210-230`). Thus an internal FVAR/GVAR/HVAR/
MVAR path being exercised is not evidence that the full public variation
surface is application-ready.

**Safe candidate.** Open a maintained variable font, select its default or
named instance, set a valid design-coordinate vector, then read face metrics
and load the same glyph at a fixed size. Compare exact coordinates, advances,
outline/bitmap output, and MVAR/HVAR effects. Dependencies are a fixture with
the declared axis count, valid coordinate ranges, a known glyph, and normal
output ownership. Keep format-14 variant-selector cases separate from axis
variation cases.

Do not use null pointers, allocator tricks, or fabricated variation-store
indices as coverage probes. `fontdone/src/tt/varstore.rs:43-306` validates
store structure and caller ranges; the documented cleanup removed parser-proven
fallback/zero-denominator/sign branches, leaving only genuinely checked
overflow behavior as a possible measured gap. That is a defensive/invariant
classification, not an actionable public probe.

## Non-probe dispositions

The following must stay out of the candidate count:

- Direct calls to private parser, decoder, blitter, or `GlyphHints` helpers to
  manufacture impossible offsets, missing selected strikes, or malformed
  internal state.
- Architecture-only SBIT overflow arms that cannot be reached on the measured
  64-bit build.
- SVG/color compositing and unavailable SBIX bitmap decoders, which are
  unsupported product behavior rather than untested complete behavior.
- The three separately documented safety extensions in
  `fontdone/doc/ROADMAP.md:574-594`: foreign/null library handle, null face
  properties, and the outline internal-pointer extension. Their rejection
  without dereference is a safety contract, not a missing runtime parity lane.

## Result

The evidence supports committing this audit as a separate documentation
artifact. It records nine classified lane families, source-matched production
metrics, five source-backed public follow-up families, and explicit
unsupported/planned, defensive, and coverage-only dispositions. The separate
historical fontdone parity record contains exactly three pending
safety-extension cases:
`freetype.FT_Done_FreeType.error_invalid_or_foreign_library_handle`,
`freetype.FT_Face_Properties.error_null_face`, and
`ftimage.FT_Outline.null_internal_pointer_safety_extension`. They are excluded
from the pinned-C numerator/denominator because the inputs are undefined or
memory-unsafe, and fontdone rejects them without dereference; they are not
missing runtime routes. The recorded 7,853 passed / 0 failed / 3 pending
result is historical evidence only and is not combined with Pillow parity or
with any coverage percentage. No source, manifest, fixture, threshold, or
runtime file was changed by this audit.
