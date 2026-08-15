# Fontdone coverage gap audit

This document is a read-only audit of the pinned GitHub `fontdone`
implementation (`https://github.com/appunni-m/fontdone.git`, revision
`95dc33f12790c896f9a5c95571eec4360f22412e`).
It intentionally contains no `fontdone` source, fixture, manifest, or threshold
changes. Fontdone parity is tracked separately from the Pillow-RS coverage
campaign.

## Evidence baseline

The latest managed Pillow-RS Rust snapshots were collected at commit
`4fafda8ef320f12a3fdae21dc573fa65e03c9338`:

- CPU snapshot: `7cbc341e-75c2-4489-8622-675e47d99449`
- SIMD snapshot: `e0d57644-1c2b-4793-9b9d-91c55a3096f3`

The fontdone files with the largest reachable-looking gaps are:

| File | Lines | Regions | Branches | Functions | Current interpretation |
| --- | ---: | ---: | ---: | ---: | --- |
| `src/pfr.rs` | 0/285* | 0/627* | 0/70* | 0/23* | Historical/source-mismatched MCP denominator; requires a source-matched run. |
| `src/tt/svg.rs` | 0/59* | 0/104* | 0/14* | 0/6* | Historical MCP result; current local source-matched artifact reports 100%. |
| `src/ffi/handles.rs` | 2,351/11,083 | 3,133/15,487 | 340/2,761 | 193/804 | Large ABI/lifecycle denominator; do not inflate with synthetic pointer cases. |

Coverage numbers are evidence of exercised code, not a claim that every
uncovered branch is reachable through the Pillow public contract.

The stored MCP report is not source-matched to the current pinned checkout:
the current `fontdone/src/pfr.rs` is 389 lines and `fontdone/src/tt/svg.rs` is
120 lines, while the stored report describes 285 and 59 lines respectively.
The current local fontdone LLVM artifact reports `src/tt/svg.rs` at 59/59
lines, 104/104 regions, 14/14 branches, and 6/6 functions. Therefore SVG is
not currently an implementation coverage gap, and PFR has no defensible
current percentage until a source-matched run is registered.

## Future, separately authorized batches

1. **PFR:** after a source-matched run, reuse an existing valid PFR fixture to
   target the wide-character and wide-adjustment kerning path in
   `parse_kerning_item`. Do not duplicate already-covered advance or metrics
   cases.
2. **SVG:** no coverage-fixing case is currently justified. Future inclusive
   range, glyph-zero, or transformed callback cases would add contract
   evidence only; SVG rasterization and gzip documents remain unsupported.
3. **SBIX/SBIT, autohint, render/scaler, CFF, CMap/SFNT, and variations:**
   audit only until a separate fontdone implementation task authorizes fixture
   or source changes. Record unsupported formats and missing fixture mappings
   as explicit blockers.

## Safety and accounting rules

- Do not edit the pinned `fontdone` checkout in the Pillow-RS campaign.
- Do not execute GPU, crash-quarantine, or pending 16-bit TIFF cases.
- Do not count ABI boilerplate, malformed-input guards, or unsupported formats
  as public parity failures without a valid public stimulus.
- Any future fontdone run must use its own managed parity/coverage evidence and
  must not be merged into the Pillow CPU/SIMD numerator.

## Unsupported public error mapping batch

The active Pillow parity corpus contains 33 public case IDs whose names include
`unsupported`. The retained image error-mapping batch covered 30 of them; the
three fontdone-owned cases were run separately because this document is not
part of the Pillow-RS image roadmap:

- `PIL.ImageFont.FreeTypeFont.getbbox.nuanced.unsupported-direction`
- `PIL.ImageFont.FreeTypeFont.getbbox.nuanced.unsupported-features`
- `PIL.ImageFont.FreeTypeFont.getlength.nuanced.unsupported-language`

Evidence: `build/migration-parity/parity-unsupported-fontdone-cpu-v2.json` and
`build/migration-parity/parity-unsupported-fontdone-simd-v2.json`. Both CPU and
SIMD selected and passed 3/3 cases with zero failures, not-run cases, or
infrastructure errors. The source and target returned the same public error
class, kind, message, stage, and code through the maintained comparator. The
shared Pillow message is `setting text direction, language or font features is
not supported without libraqm`. No fontdone source, fixture, expected output,
threshold, or denominator was changed.
