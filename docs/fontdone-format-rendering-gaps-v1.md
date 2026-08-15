# Fontdone format and rendering gap classification

Status: **separate read-only audit; not part of the Pillow-RS image-pipeline roadmap**

This note classifies format and rendering boundaries in the standalone
[appunni-m/fontdone](https://github.com/appunni-m/fontdone) source. It does
not change Pillow-RS code, fontdone code, fixtures, manifests, expected
outputs, thresholds, or any coverage denominator.

## Audit identity

| Field | Value |
|---|---|
| Source repository | `https://github.com/appunni-m/fontdone.git` |
| Branch | `main` |
| Source revision | `95dc33f12790c896f9a5c95571eec4360f22412e` |
| Source commit | `Cover malformed COLR v0 layer records` |
| Pinned oracle described by the source | FreeType 2.14.3 |
| Audit date | 2026-08-13 |
| Inspection checkout | `/private/tmp/fontdone-readonly-v1` (temporary, read-only) |

## Evidence rules

- **Proved from source:** a behavior or boundary is explicit in the inspected
  source at the revision above.
- **Declared:** a status comes from the generated `doc/FREETYPE_SUPPORT.md` or
  the checked-in snapshot. It is an adoption statement, not a fresh execution
  result.
- **Unknown:** this audit did not execute parity, coverage, or unit tests, so
  no current-revision runtime claim is made.
- **Intentional exclusion:** the source declares a surface outside the current
  product boundary. It is not an implementation gap and must not be used to
  lower another project's denominator.

The checked-in adoption map declares 218 pinned public FreeType functions:
52 complete, 5 implemented with incomplete mapping, 29 partial, 69 planned,
and 63 intentionally excluded. Those are fontdone's declared contract states;
they are not Pillow-RS coverage percentages.

The checked-in runtime and coverage evidence is source-bound to older commits
(`doc/runtime_parity_evidence.json` records
`ad9b2d805eab690c4237b68cf0558c58dc25b26b`, and its coverage snapshot names
`06a8ce2b91d041d1bc75618e579480d11f182dcd`). Those artifacts were not treated
as proof for `95dc33f...`, and no replacement run was started because this
request forbids unit tests.

## Classified format and rendering gaps

| ID | Area | Classification at `95dc33f...` | What remains open or bounded |
|---|---|---|---|
| FDG-01 | PFR | **Partial: metrics service, not a demonstrated glyph renderer** | PFR metrics/advance/kerning are maintained surfaces. PFR glyph-outline extraction and rendered-mask parity are not established; this is a fontdone gap, not an image-pipeline coverage denominator. |
| FDG-02 | OpenType SVG | **Partial: table/document loading without rasterization** | A real SVG renderer and exact rendered-output parity remain open if color/SVG glyph rendering is in scope. Document/slot metadata must not be reported as rendered-pixel support. |
| FDG-03 | SBIX | **Partial: strike metadata and error behavior, no general graphic decoder** | Full SBIX glyph-image decoding and rendering are open. Adding Pillow inputs cannot make an unavailable decoder route complete. |
| FDG-04 | SBIT / EBLC / EBDT / CBLC / CBDT | **Implemented bounded subset** | Exact parity breadth for every supported strike/image format and configuration remains unverified; unsupported bit depths remain explicit errors. |
| FDG-05 | CFF / CFF2 | **Partial: compact Type2 outline subset** | General CFF1/CFF2 parity remains open: subroutines, the complete Type2 operator set, CFF2 variation/blend behavior, and broader CID/production-font cases. |
| FDG-06 | CMap / SFNT metadata | **Core mapping present; public metadata adoption incomplete** | Public SFNT/CMap metadata contracts must be measured separately from table-parser coverage. |
| FDG-07 | Variations | **Partial: table/scaler plumbing, incomplete public MM/variation API** | Complete variation-coordinate state, glyph/metric deltas, named instances, CFF2 blend routes, and public API parity remain open. |
| FDG-08 | Autohint and scaler | **Implemented paths; current breadth unknown** | Source inspection does not prove complete script, size, variation, composite, or render-mode parity at this revision. |
| FDG-09 | Outline rasterization and stroking | **Raster modes present; composite/SVG and broad stroker behavior remain bounded** | Separate outline-load, render, and stroke contracts. Successful outline load does not prove rendered bitmap or general stroke geometry. |
| FDG-10 | COLR / CPAL / color glyph composition | **Intentional product-surface exclusion** | Do not count excluded color-glyph APIs as missing Pillow-RS coverage. A future boundary change needs a fontdone-specific denominator review. |

## Priority if fontdone work is separately authorized

This is classification only; no implementation is proposed or performed here.

1. Decide whether SVG, SBIX image decoders, and COLR/CPAL color composition are
   in the supported product boundary.
2. Complete CFF/CFF2 subroutine and Type2 operator support before using complex
   CFF fonts as broad rendering evidence.
3. Close public CMap/SFNT metadata APIs independently of table-parser coverage.
4. Complete variation state/API behavior, including CFF2 blend routes and named
   instances.
5. Add PFR glyph-outline/rendering behavior only if the product contract
   requires rendered PFR glyphs; otherwise keep PFR explicitly metrics-only.
6. Re-run source-bound parity and coverage for the exact revision before
   changing any status or claiming completion.

## Boundary statement

This file is intentionally independent of
[docs/image-pipeline-performance-roadmap.md](image-pipeline-performance-roadmap.md).
It must not be merged into that roadmap or used to change its operation list,
coverage denominator, thresholds, or completion status.
