# Migration parity case review

This is a deterministic selection ledger for input definitions. It is
not a parity, coverage, or benchmark result and contains no expected
outputs.

## Selection outcome

- Manifest operations: 204
- Manifest requirements: 1780
- Active parity workflows: 1244
- Unique active workflow signatures: 1244
- Active exact-duplicate groups: 0
- Deliberate nuanced workflows: 75

The generator merges only exact behavior-bearing duplicates. Case IDs
and `covers` membership are labels and therefore do not create a second
execution. Setup order, omitted versus explicit defaults, asset identity,
arguments, and observations remain part of the signature.

### Active cases by public surface

| surface | active workflows |
| --- | ---: |
| `PIL.Image` | 100 |
| `PIL.Image.Image` | 461 |
| `PIL.ImageChops` | 76 |
| `PIL.ImageColor` | 12 |
| `PIL.ImageDraw` | 3 |
| `PIL.ImageDraw.ImageDraw` | 223 |
| `PIL.ImageEnhance` | 16 |
| `PIL.ImageEnhance.Brightness` | 1 |
| `PIL.ImageEnhance.Color` | 1 |
| `PIL.ImageEnhance.Contrast` | 1 |
| `PIL.ImageEnhance.Sharpness` | 1 |
| `PIL.ImageFilter` | 101 |
| `PIL.ImageFont` | 25 |
| `PIL.ImageFont.FreeTypeFont` | 61 |
| `PIL.ImageFont.ImageFont` | 10 |
| `PIL.ImageFont.TransposedFont` | 13 |
| `PIL.ImageOps` | 97 |
| `PIL.ImagePalette` | 3 |
| `PIL.ImagePalette.ImagePalette` | 6 |
| `PIL.ImageSequence` | 2 |
| `PIL.ImageStat` | 4 |
| `PIL.ImageStat.Stat` | 27 |

## Deprecated corpus accounting

| corpus | rows | unique stimuli | duplicate rows removed |
| --- | ---: | ---: | ---: |
| suite0 | 823 | 775 | 48 |
| suite1 | 769 | 718 | 51 |
| combined | 1592 | 1432 | 160 |

The old corpora are migration evidence only. Their duplicate rows
are not copied into the active lane by name.

## Nuanced workflows

- `PIL.Image.Image.convert.nuanced.alpha-conversion`
- `PIL.Image.Image.convert.nuanced.cmyk-to-l`
- `PIL.Image.Image.convert.nuanced.cmyk-to-rgb`
- `PIL.Image.Image.convert.nuanced.f-to-rgb`
- `PIL.Image.Image.convert.nuanced.hsv-to-rgb`
- `PIL.Image.Image.convert.nuanced.i-to-rgb`
- `PIL.Image.Image.convert.nuanced.l-to-rgb`
- `PIL.Image.Image.convert.nuanced.l-to-rgba`
- `PIL.Image.Image.convert.nuanced.la-to-rgb`
- `PIL.Image.Image.convert.nuanced.p-to-rgb`
- `PIL.Image.Image.convert.nuanced.rgb-to-cmyk`
- `PIL.Image.Image.convert.nuanced.rgb-to-f`
- `PIL.Image.Image.convert.nuanced.rgb-to-hsv`
- `PIL.Image.Image.convert.nuanced.rgb-to-i`
- `PIL.Image.Image.convert.nuanced.rgb-to-ycbcr`
- `PIL.Image.Image.convert.nuanced.rgba-to-l`
- `PIL.Image.Image.convert.nuanced.ycbcr-to-rgb`
- `PIL.Image.Image.getbbox.nuanced.alpha-only-rgba`
- `PIL.Image.Image.getbbox.nuanced.blue-only-rgb`
- `PIL.Image.Image.getbbox.nuanced.green-only-rgb`
- `PIL.Image.Image.getbbox.nuanced.nonzero-rgb`
- `PIL.Image.Image.getbbox.nuanced.transparent-alpha-rgba`
- `PIL.Image.Image.getextrema.nuanced.nonzero-rgba`
- `PIL.Image.Image.histogram.nuanced.nonzero-rgba`
- `PIL.Image.Image.resize.nuanced.noninteger-ratio-lanczos`
- `PIL.Image.Image.rotate.nuanced.fractional-expanded`
- `PIL.ImageChops.invert.nuanced.la`
- `PIL.ImageChops.invert.nuanced.rgba`
- `PIL.ImageColor.getcolor.nuanced.hex-la-alpha`
- `PIL.ImageColor.getcolor.nuanced.hex-rgba`
- `PIL.ImageColor.getcolor.nuanced.named-l`
- `PIL.ImageColor.getcolor.nuanced.named-one`
- `PIL.ImageColor.getcolor.nuanced.rgb-syntax-l`
- `PIL.ImageColor.getrgb.nuanced.hex-with-alpha`
- `PIL.ImageColor.getrgb.nuanced.hsl-syntax`
- `PIL.ImageColor.getrgb.nuanced.named-css-color`
- `PIL.ImageColor.getrgb.nuanced.rgba-syntax`
- `PIL.ImageDraw.ImageDraw.multiline_text.nuanced.three-line-spacing`
- `PIL.ImageDraw.ImageDraw.text.nuanced.unicode-anchor`
- `PIL.ImageDraw.ImageDraw.textbbox.nuanced.unicode-anchor`
- `PIL.ImageFilter.Kernel.nuanced.three-by-three-edge`
- `PIL.ImageFont.FreeTypeFont.font_variant.nuanced.variable-font-size`
- `PIL.ImageFont.FreeTypeFont.get_variation_axes.nuanced.named-instances`
- `PIL.ImageFont.FreeTypeFont.get_variation_axes.nuanced.variable-font`
- `PIL.ImageFont.FreeTypeFont.get_variation_names.nuanced.named-instances`
- `PIL.ImageFont.FreeTypeFont.get_variation_names.nuanced.variable-font`
- `PIL.ImageFont.FreeTypeFont.getbbox.nuanced.invalid-anchor`
- `PIL.ImageFont.FreeTypeFont.getbbox.nuanced.unicode-multiline`
- `PIL.ImageFont.FreeTypeFont.getbbox.nuanced.unsupported-direction`
- `PIL.ImageFont.FreeTypeFont.getbbox.nuanced.unsupported-features`
- `PIL.ImageFont.FreeTypeFont.getlength.nuanced.kerning-pair`
- `PIL.ImageFont.FreeTypeFont.getlength.nuanced.unsupported-language`
- `PIL.ImageFont.FreeTypeFont.getmask2.nuanced.fractional-start`
- `PIL.ImageFont.FreeTypeFont.getmask2.nuanced.fractional-stroke`
- `PIL.ImageFont.FreeTypeFont.getmask2.nuanced.mode-one`
- `PIL.ImageFont.FreeTypeFont.getmask2.nuanced.multiline-stroked`
- `PIL.ImageFont.FreeTypeFont.set_variation_by_axes.nuanced.variable-font`
- `PIL.ImageFont.FreeTypeFont.set_variation_by_name.nuanced.variable-font`
- `PIL.ImageFont.TransposedFont.getbbox.nuanced.rotate-270`
- `PIL.ImageFont.TransposedFont.getlength.nuanced.rotate-90-length-error`
- `PIL.ImageFont.TransposedFont.getmask.nuanced.rotate-90`
- `PIL.ImageFont.truetype.nuanced.fractional-size`
- `PIL.ImageFont.truetype.nuanced.malformed-cff-name-index`
- `PIL.ImageFont.truetype.nuanced.malformed-cff-table`
- `PIL.ImageFont.truetype.nuanced.negative-fractional-size`
- `PIL.ImageFont.truetype.nuanced.oversized-size`
- `PIL.ImageFont.truetype.nuanced.zero-size`
- `PIL.ImageOps.colorize.nuanced.invalid-mid-points`
- `PIL.ImageOps.colorize.nuanced.invalid-points`
- `PIL.ImageOps.colorize.nuanced.mapped-points`
- `PIL.ImageOps.colorize.nuanced.mode-one`
- `PIL.ImageOps.colorize.nuanced.three-color`
- `PIL.ImageOps.colorize.nuanced.two-color`
- `PIL.ImageOps.fit.nuanced.fractional-centering`
- `PIL.ImageOps.invert.nuanced.p-mode`

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
