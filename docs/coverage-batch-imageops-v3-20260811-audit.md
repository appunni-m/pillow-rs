# CPU ImageOps coverage batch audit

This bounded audit used managed CPU snapshot
`2ee6544f-bfa2-4b7b-8ed4-bf0774c96fb6` against the worker baseline
`a5690c8ed`. No source or generated input was changed, and no post-change
managed run was started. Therefore the before/after counts are unchanged:

| scope | before | after |
| --- | ---: | ---: |
| active CPU lines | 23,496/26,647 | 23,496/26,647 |
| active CPU branches | 3,570/4,198 | 3,570/4,198 |
| active CPU functions | 1,982/2,468 | 1,982/2,468 |
| active CPU regions | 38,368/44,382 | 38,368/44,382 |
| `ops/imageops.rs` lines | 571/584 | 571/584 |
| `pool_cpu/ops/imageops.rs` lines | 450/522 | 450/522 |

## Exact queried gaps

`pillow-rs/src/ops/imageops.rs` had 13 uncovered lines and 13 partial-branch
lines: uncovered `183, 810, 814, 819, 836, 839, 842, 845, 849, 852, 855,
881, 886`; partial `807, 813, 835, 838, 841, 844, 848, 851, 854, 864-866,
885`.

`pillow-rs/src/compute/pool_cpu/ops/imageops.rs` had 64 uncovered lines and
10 partial-branch lines. Uncovered ranges were `149-153, 157-163, 243, 251,
262, 484, 515, 546, 557, 571, 650, 657, 659-661, 663-667, 670, 674-678,
681, 684, 687, 689-694, 696-703, 706, 709-716, 719, 722`; partial lines were
`42, 242, 250, 261, 482, 513, 543, 554, 570, 648`.

## Classification

Reachable through existing valid public inputs, but not coverage-verified in
this time-box:

- `ops/imageops.rs:183`: `ImageOps.pad` with a named color, including the
  existing `PIL.ImageOps.pad.nuanced.rgb-color-name` and
  `...rgba-color-name` cases (and the corresponding L/LA/F/HSV cases),
  reaches `resolve_imageops_color`'s `Name`/`getcolor` path.
- `ops/imageops.rs:807-886`: `ImageOps.exif_transpose` with the existing valid
  JPEG/TIFF orientation, no-orientation, and width-before-orientation cases
  reaches the retained-Exif parser and serializer paths.
- `pool_cpu/ops/imageops.rs:42`: the empty-image branch is represented by the
  existing valid empty-L autocontrast cases.
- `pool_cpu/ops/imageops.rs:570-571`: the existing valid
  `PIL.ImageOps.crop.nuanced.border-exceeds-image` case uses an 8-pixel border
  on a 16x16 image and reaches the public oversize-border error path.

Defensive or unreachable for valid public inputs:

- `pool_cpu/ops/imageops.rs:149-163`: `invert` buffer-shape error arms;
  `from_raw` receives the image's own shape-preserving byte buffer.
- `pool_cpu/ops/imageops.rs:242-262`: zero-span colorize LUT arms; the
  surrounding index inequalities cannot hold when the corresponding points
  are equal, even though equal points are valid parameters.
- `pool_cpu/ops/imageops.rs:482-557`: pad destination bounds guards; contain
  sizing and clamped centering keep source offsets in bounds.
- `pool_cpu/ops/imageops.rs:648-650`: expand destination bounds guard;
  expanded dimensions and the border offset preserve the invariant.
- `pool_cpu/ops/imageops.rs:657-722`: linear/radial gradient implementation,
  outside this `PIL.Image.Image`/`PIL.ImageOps` batch.

No new case IDs were added. Focused parity/coverage run IDs: none; no managed
command was launched after the time-box request, so there were no
infrastructure failures to report. Remaining reachable candidates are the
named-color, Exif, empty-autocontrast, and oversized-crop paths above; the
remaining invariant-protected bucket is classified rather than manufactured
with invalid inputs.
