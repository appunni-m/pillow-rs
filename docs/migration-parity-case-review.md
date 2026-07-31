# Migration parity case review

This is a deterministic selection ledger for input definitions. It is
not a parity, coverage, or benchmark result and contains no expected
outputs.

## Selection outcome

- Manifest operations: 204
- Manifest requirements: 1780
- Active parity workflows: 1181
- Unique active workflow signatures: 1181
- Active exact-duplicate groups: 0
- Deliberate nuanced workflows: 12

The generator merges only exact behavior-bearing duplicates. Case IDs
and `covers` membership are labels and therefore do not create a second
execution. Setup order, omitted versus explicit defaults, asset identity,
arguments, and observations remain part of the signature.

### Active cases by public surface

| surface | active workflows |
| --- | ---: |
| `PIL.Image` | 100 |
| `PIL.Image.Image` | 438 |
| `PIL.ImageChops` | 74 |
| `PIL.ImageColor` | 4 |
| `PIL.ImageDraw` | 3 |
| `PIL.ImageDraw.ImageDraw` | 223 |
| `PIL.ImageEnhance` | 16 |
| `PIL.ImageEnhance.Brightness` | 1 |
| `PIL.ImageEnhance.Color` | 1 |
| `PIL.ImageEnhance.Contrast` | 1 |
| `PIL.ImageEnhance.Sharpness` | 1 |
| `PIL.ImageFilter` | 101 |
| `PIL.ImageFont` | 19 |
| `PIL.ImageFont.FreeTypeFont` | 47 |
| `PIL.ImageFont.ImageFont` | 10 |
| `PIL.ImageFont.TransposedFont` | 10 |
| `PIL.ImageOps` | 90 |
| `PIL.ImagePalette` | 3 |
| `PIL.ImagePalette.ImagePalette` | 6 |
| `PIL.ImageSequence` | 2 |
| `PIL.ImageStat` | 4 |
| `PIL.ImageStat.Stat` | 27 |

## Deprecated corpus accounting

| corpus | rows | unique stimuli | duplicate rows removed |
| --- | ---: | ---: | ---: |
| fixtures | 823 | 775 | 48 |
| fixtures_2 | 769 | 718 | 51 |
| combined | 1592 | 1432 | 160 |

The old corpora are migration evidence only. Their duplicate rows
are not copied into the active lane by name.

## Nuanced workflows

- `PIL.Image.Image.convert.nuanced.alpha-conversion`
- `PIL.Image.Image.resize.nuanced.noninteger-ratio-lanczos`
- `PIL.Image.Image.rotate.nuanced.fractional-expanded`
- `PIL.ImageColor.getrgb.nuanced.named-css-color`
- `PIL.ImageDraw.ImageDraw.multiline_text.nuanced.three-line-spacing`
- `PIL.ImageDraw.ImageDraw.text.nuanced.unicode-anchor`
- `PIL.ImageDraw.ImageDraw.textbbox.nuanced.unicode-anchor`
- `PIL.ImageFilter.Kernel.nuanced.three-by-three-edge`
- `PIL.ImageFont.FreeTypeFont.getbbox.nuanced.unicode-multiline`
- `PIL.ImageFont.FreeTypeFont.getlength.nuanced.kerning-pair`
- `PIL.ImageFont.FreeTypeFont.getmask2.nuanced.multiline-stroked`
- `PIL.ImageOps.fit.nuanced.fractional-centering`

These cases cover high-risk behavior families that a broad default
matrix does not distinguish: Unicode/combining/multiline font text,
anchored drawing, non-integer image geometry, valid color syntax,
fractional centering, and a real three-by-three filter kernel.

## Review rules

1. Every active case calls manifest operations through public workflow
   steps; no fixture-only dispatcher IDs are accepted.
2. Exact workflow duplicates are merged while all requirement IDs remain
   in `covers` and coverage/benchmark selectors use the canonical case.
3. Edge/error requirements must change a stimulus or intentionally share
   a public no-op baseline; they may not be labels on the default call.
4. Non-JSON public values (for example `Image.point` callables) remain
   an explicit contract/auditor blocker until the fixed value interface
   defines their source-neutral representation.
5. Additional nuanced cases are allowed to reuse a requirement, but they
   never replace its canonical mapping or add expected output data.
