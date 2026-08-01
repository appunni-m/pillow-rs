# Migration parity case review

This is a deterministic selection ledger for input definitions. It is
not a parity, coverage, or benchmark result and contains no expected
outputs.

## Selection outcome

- Manifest operations: 203
- Manifest requirements: 1776
- Active parity workflows: 1648
- Unique active workflow signatures: 1648
- Active exact-duplicate groups: 0
- Deliberate nuanced workflows: 481

The generator merges only exact behavior-bearing duplicates. Case IDs
and `covers` membership are labels and therefore do not create a second
execution. Setup order, omitted versus explicit defaults, asset identity,
arguments, and observations remain part of the signature.

### Active cases by public surface

| surface | active workflows |
| --- | ---: |
| `PIL.Image` | 135 |
| `PIL.Image.Image` | 720 |
| `PIL.ImageChops` | 79 |
| `PIL.ImageColor` | 30 |
| `PIL.ImageDraw` | 3 |
| `PIL.ImageDraw.ImageDraw` | 290 |
| `PIL.ImageEnhance` | 16 |
| `PIL.ImageEnhance.Brightness` | 1 |
| `PIL.ImageEnhance.Color` | 1 |
| `PIL.ImageEnhance.Contrast` | 1 |
| `PIL.ImageEnhance.Sharpness` | 1 |
| `PIL.ImageFilter` | 104 |
| `PIL.ImageFont` | 25 |
| `PIL.ImageFont.FreeTypeFont` | 61 |
| `PIL.ImageFont.ImageFont` | 10 |
| `PIL.ImageFont.TransposedFont` | 13 |
| `PIL.ImageOps` | 106 |
| `PIL.ImagePalette` | 3 |
| `PIL.ImagePalette.ImagePalette` | 9 |
| `PIL.ImageSequence` | 2 |
| `PIL.ImageStat` | 5 |
| `PIL.ImageStat.Stat` | 33 |

## Deprecated corpus accounting

| corpus | rows | unique stimuli | duplicate rows removed |
| --- | ---: | ---: | ---: |
| suite0 | 823 | 775 | 48 |
| suite1 | 769 | 718 | 51 |
| combined | 1592 | 1432 | 160 |

The old corpora are migration evidence only. Their duplicate rows
are not copied into the active lane by name.

## Nuanced workflows

- `PIL.Image.Image.alpha_composite.nuanced.offset-dest`
- `PIL.Image.Image.alpha_composite.nuanced.offset-source`
- `PIL.Image.Image.alpha_composite.nuanced.rgba-dest-rgb-source-mode-error`
- `PIL.Image.Image.alpha_composite.nuanced.source-four-tuple`
- `PIL.Image.Image.alpha_composite.nuanced.source-larger-than-dest`
- `PIL.Image.Image.alpha_composite.nuanced.source-smaller-offset-dest`
- `PIL.Image.Image.alpha_composite.nuanced.source-smaller-than-dest`
- `PIL.Image.Image.apply_transparency.nuanced.png-p-transparency`
- `PIL.Image.Image.apply_transparency.nuanced.png-p-transparency-table`
- `PIL.Image.Image.convert.nuanced.alpha-conversion`
- `PIL.Image.Image.convert.nuanced.attached-palette-transparency-table`
- `PIL.Image.Image.convert.nuanced.cmyk-to-1`
- `PIL.Image.Image.convert.nuanced.cmyk-to-l`
- `PIL.Image.Image.convert.nuanced.cmyk-to-la`
- `PIL.Image.Image.convert.nuanced.cmyk-to-rgb`
- `PIL.Image.Image.convert.nuanced.cmyk-to-rgba`
- `PIL.Image.Image.convert.nuanced.f-to-1`
- `PIL.Image.Image.convert.nuanced.f-to-cmyk`
- `PIL.Image.Image.convert.nuanced.f-to-rgb`
- `PIL.Image.Image.convert.nuanced.from-cmyk-to-l`
- `PIL.Image.Image.convert.nuanced.from-f-to-l`
- `PIL.Image.Image.convert.nuanced.from-hsv-to-rgb`
- `PIL.Image.Image.convert.nuanced.from-one-to-l`
- `PIL.Image.Image.convert.nuanced.from-one-to-rgb`
- `PIL.Image.Image.convert.nuanced.from-ycbcr`
- `PIL.Image.Image.convert.nuanced.hsv-to-1`
- `PIL.Image.Image.convert.nuanced.hsv-to-cmyk`
- `PIL.Image.Image.convert.nuanced.hsv-to-l`
- `PIL.Image.Image.convert.nuanced.hsv-to-rgb`
- `PIL.Image.Image.convert.nuanced.i-to-1`
- `PIL.Image.Image.convert.nuanced.i-to-cmyk`
- `PIL.Image.Image.convert.nuanced.i-to-rgb`
- `PIL.Image.Image.convert.nuanced.l-to-p`
- `PIL.Image.Image.convert.nuanced.l-to-rgb`
- `PIL.Image.Image.convert.nuanced.l-to-rgba`
- `PIL.Image.Image.convert.nuanced.la-to-rgb`
- `PIL.Image.Image.convert.nuanced.one-to-cmyk`
- `PIL.Image.Image.convert.nuanced.one-to-l`
- `PIL.Image.Image.convert.nuanced.one-to-rgb`
- `PIL.Image.Image.convert.nuanced.opened-l`
- `PIL.Image.Image.convert.nuanced.opened-p-to-pa`
- `PIL.Image.Image.convert.nuanced.opened-p-transparency`
- `PIL.Image.Image.convert.nuanced.opened-p-transparency-table`
- `PIL.Image.Image.convert.nuanced.opened-p-transparency-table-to-la`
- `PIL.Image.Image.convert.nuanced.opened-p-transparency-table-to-pa`
- `PIL.Image.Image.convert.nuanced.opened-p-transparency-table-to-rgb`
- `PIL.Image.Image.convert.nuanced.opened-p-transparency-to-l`
- `PIL.Image.Image.convert.nuanced.opened-p-transparency-to-la`
- `PIL.Image.Image.convert.nuanced.opened-p-transparency-to-pa`
- `PIL.Image.Image.convert.nuanced.opened-p-transparency-to-rgb`
- `PIL.Image.Image.convert.nuanced.opened-rgb`
- `PIL.Image.Image.convert.nuanced.opened-rgba`
- `PIL.Image.Image.convert.nuanced.p-putalpha-to-rgba`
- `PIL.Image.Image.convert.nuanced.p-putpalette-putalpha-to-rgba`
- `PIL.Image.Image.convert.nuanced.p-to-cmyk`
- `PIL.Image.Image.convert.nuanced.p-to-rgb`
- `PIL.Image.Image.convert.nuanced.rgb-matrix-four`
- `PIL.Image.Image.convert.nuanced.rgb-matrix-twelve`
- `PIL.Image.Image.convert.nuanced.rgb-to-cmyk`
- `PIL.Image.Image.convert.nuanced.rgb-to-f`
- `PIL.Image.Image.convert.nuanced.rgb-to-hsv`
- `PIL.Image.Image.convert.nuanced.rgb-to-i`
- `PIL.Image.Image.convert.nuanced.rgb-to-p`
- `PIL.Image.Image.convert.nuanced.rgb-to-ycbcr`
- `PIL.Image.Image.convert.nuanced.rgba-to-l`
- `PIL.Image.Image.convert.nuanced.unknown-mode`
- `PIL.Image.Image.convert.nuanced.ycbcr-to-1`
- `PIL.Image.Image.convert.nuanced.ycbcr-to-cmyk`
- `PIL.Image.Image.convert.nuanced.ycbcr-to-l`
- `PIL.Image.Image.convert.nuanced.ycbcr-to-rgb`
- `PIL.Image.Image.copy.nuanced.opened-rgb`
- `PIL.Image.Image.crop.nuanced.opened-rgb`
- `PIL.Image.Image.entropy.nuanced.bad-mask-mode`
- `PIL.Image.Image.entropy.nuanced.mask-size-mismatch`
- `PIL.Image.Image.entropy.nuanced.masked-region`
- `PIL.Image.Image.entropy.nuanced.opened-l`
- `PIL.Image.Image.filter.nuanced.f-mode`
- `PIL.Image.Image.filter.nuanced.invalid-filter`
- `PIL.Image.Image.filter.nuanced.opened-rgb`
- `PIL.Image.Image.filter.nuanced.p-mode-filter`
- `PIL.Image.Image.frombytes.nuanced.valid-cmyk`
- `PIL.Image.Image.frombytes.nuanced.valid-f`
- `PIL.Image.Image.frombytes.nuanced.valid-i`
- `PIL.Image.Image.frombytes.nuanced.valid-l`
- `PIL.Image.Image.frombytes.nuanced.valid-la`
- `PIL.Image.Image.frombytes.nuanced.valid-p`
- `PIL.Image.Image.frombytes.nuanced.valid-packed`
- `PIL.Image.Image.frombytes.nuanced.valid-rgba`
- `PIL.Image.Image.frombytes.nuanced.valid-ycbcr`
- `PIL.Image.Image.getbands.nuanced.cmyk`
- `PIL.Image.Image.getbands.nuanced.f-mode`
- `PIL.Image.Image.getbands.nuanced.hsv`
- `PIL.Image.Image.getbands.nuanced.i-mode`
- `PIL.Image.Image.getbands.nuanced.pa-mode`
- `PIL.Image.Image.getbands.nuanced.ycbcr`
- `PIL.Image.Image.getbbox.nuanced.alpha-only-false-rgb`
- `PIL.Image.Image.getbbox.nuanced.alpha-only-rgba`
- `PIL.Image.Image.getbbox.nuanced.blank-rgba`
- `PIL.Image.Image.getbbox.nuanced.blue-only-rgb`
- `PIL.Image.Image.getbbox.nuanced.corner-pixel`
- `PIL.Image.Image.getbbox.nuanced.green-only-rgb`
- `PIL.Image.Image.getbbox.nuanced.la-zero-rgb-nonzero-alpha`
- `PIL.Image.Image.getbbox.nuanced.nonzero-1`
- `PIL.Image.Image.getbbox.nuanced.nonzero-alpha`
- `PIL.Image.Image.getbbox.nuanced.nonzero-alpha-rgba`
- `PIL.Image.Image.getbbox.nuanced.nonzero-l`
- `PIL.Image.Image.getbbox.nuanced.nonzero-p`
- `PIL.Image.Image.getbbox.nuanced.nonzero-rgb`
- `PIL.Image.Image.getbbox.nuanced.png-p-transparency`
- `PIL.Image.Image.getbbox.nuanced.png-rgba-opened`
- `PIL.Image.Image.getbbox.nuanced.rgba-blue-only-zero-alpha`
- `PIL.Image.Image.getbbox.nuanced.rgba-green-only-zero-alpha`
- `PIL.Image.Image.getbbox.nuanced.rgba-nonzero-rgb-zero-alpha`
- `PIL.Image.Image.getbbox.nuanced.rgba-zero-rgb-nonzero-alpha`
- `PIL.Image.Image.getbbox.nuanced.transparent-alpha-rgba`
- `PIL.Image.Image.getchannel.nuanced.opened-rgba-alpha`
- `PIL.Image.Image.getcolors.nuanced.i-nonzero`
- `PIL.Image.Image.getcolors.nuanced.la-nonzero`
- `PIL.Image.Image.getcolors.nuanced.opened-p`
- `PIL.Image.Image.getcolors.nuanced.opened-rgba`
- `PIL.Image.Image.getdata.nuanced.opened-rgb`
- `PIL.Image.Image.getexif.nuanced.jpeg-exif`
- `PIL.Image.Image.getexif.nuanced.jpeg-without-exif`
- `PIL.Image.Image.getexif.nuanced.png-without-exif`
- `PIL.Image.Image.getexif.nuanced.tiff-container`
- `PIL.Image.Image.getextrema.nuanced.nonzero-rgba`
- `PIL.Image.Image.getextrema.nuanced.opened-rgb`
- `PIL.Image.Image.getextrema.nuanced.png-rgba-opened`
- `PIL.Image.Image.getpalette.nuanced.attached-alpha-rgbx`
- `PIL.Image.Image.getpalette.nuanced.attached-channel-b`
- `PIL.Image.Image.getpalette.nuanced.attached-channel-g`
- `PIL.Image.Image.getpalette.nuanced.attached-channel-invalid`
- `PIL.Image.Image.getpalette.nuanced.attached-channel-r`
- `PIL.Image.Image.getpalette.nuanced.attached-rgbx`
- `PIL.Image.Image.getpalette.nuanced.opened-p`
- `PIL.Image.Image.getpalette.nuanced.opened-p-rgba`
- `PIL.Image.Image.getprojection.nuanced.opened-rgb`
- `PIL.Image.Image.histogram.nuanced.mask-size-mismatch`
- `PIL.Image.Image.histogram.nuanced.masked-region`
- `PIL.Image.Image.histogram.nuanced.nonzero-rgb`
- `PIL.Image.Image.histogram.nuanced.nonzero-rgba`
- `PIL.Image.Image.histogram.nuanced.opened-rgba`
- `PIL.Image.Image.load.nuanced.bmp-rgb-opened`
- `PIL.Image.Image.load.nuanced.gif-p-opened`
- `PIL.Image.Image.load.nuanced.jpeg-rgb-opened`
- `PIL.Image.Image.load.nuanced.p-resize-putalpha`
- `PIL.Image.Image.load.nuanced.png-l-opened`
- `PIL.Image.Image.load.nuanced.png-p-opened`
- `PIL.Image.Image.load.nuanced.png-p-transparency`
- `PIL.Image.Image.load.nuanced.png-rgb-opened`
- `PIL.Image.Image.load.nuanced.png-rgba-opened`
- `PIL.Image.Image.load.nuanced.quantized-pipeline`
- `PIL.Image.Image.load.nuanced.tiff-rgb-opened`
- `PIL.Image.Image.load.nuanced.webp-rgb-opened`
- `PIL.Image.Image.load.nuanced.webp-rgba-opened`
- `PIL.Image.Image.paste.nuanced.color-cmyk`
- `PIL.Image.Image.paste.nuanced.color-hsv-four-tuple`
- `PIL.Image.Image.paste.nuanced.color-i`
- `PIL.Image.Image.paste.nuanced.color-l`
- `PIL.Image.Image.paste.nuanced.color-la`
- `PIL.Image.Image.paste.nuanced.color-one`
- `PIL.Image.Image.paste.nuanced.color-p`
- `PIL.Image.Image.paste.nuanced.color-pa-int`
- `PIL.Image.Image.paste.nuanced.color-pa-two-tuple`
- `PIL.Image.Image.paste.nuanced.color-rgb`
- `PIL.Image.Image.paste.nuanced.color-rgb-four-tuple`
- `PIL.Image.Image.paste.nuanced.color-rgba`
- `PIL.Image.Image.paste.nuanced.color-two-tuple-box`
- `PIL.Image.Image.paste.nuanced.color-ycbcr-four-tuple`
- `PIL.Image.Image.paste.nuanced.color-zero-region`
- `PIL.Image.Image.paste.nuanced.f-int-fill`
- `PIL.Image.Image.paste.nuanced.i-two-tuple-error`
- `PIL.Image.Image.paste.nuanced.l-from-rgb`
- `PIL.Image.Image.paste.nuanced.l-source-into-rgb`
- `PIL.Image.Image.paste.nuanced.l-two-tuple-error`
- `PIL.Image.Image.paste.nuanced.la-four-tuple-error`
- `PIL.Image.Image.paste.nuanced.la-mask`
- `PIL.Image.Image.paste.nuanced.la-source-into-rgb`
- `PIL.Image.Image.paste.nuanced.p-from-l`
- `PIL.Image.Image.paste.nuanced.p-source-into-pa`
- `PIL.Image.Image.paste.nuanced.pa-from-p`
- `PIL.Image.Image.paste.nuanced.region-mask-mismatch`
- `PIL.Image.Image.paste.nuanced.rgb-from-rgba`
- `PIL.Image.Image.paste.nuanced.rgb-int-fill`
- `PIL.Image.Image.paste.nuanced.rgb-two-tuple-error`
- `PIL.Image.Image.paste.nuanced.rgba-mask`
- `PIL.Image.Image.paste.nuanced.rgba-rgb-tuple`
- `PIL.Image.Image.paste.nuanced.rgba-source-into-rgb`
- `PIL.Image.Image.paste.nuanced.scalar-color-la`
- `PIL.Image.Image.point.nuanced.opened-rgb`
- `PIL.Image.Image.putalpha.nuanced.cmyk-mask`
- `PIL.Image.Image.putalpha.nuanced.cmyk-scalar`
- `PIL.Image.Image.putalpha.nuanced.f-unsupported`
- `PIL.Image.Image.putalpha.nuanced.hsv-unsupported`
- `PIL.Image.Image.putalpha.nuanced.i-unsupported`
- `PIL.Image.Image.putalpha.nuanced.l-mask`
- `PIL.Image.Image.putalpha.nuanced.l-scalar`
- `PIL.Image.Image.putalpha.nuanced.la-mask`
- `PIL.Image.Image.putalpha.nuanced.la-scalar`
- `PIL.Image.Image.putalpha.nuanced.mask-size-mismatch`
- `PIL.Image.Image.putalpha.nuanced.one-mask`
- `PIL.Image.Image.putalpha.nuanced.p-mask`
- `PIL.Image.Image.putalpha.nuanced.p-scalar`
- `PIL.Image.Image.putalpha.nuanced.rgb-scalar`
- `PIL.Image.Image.putalpha.nuanced.rgba-scalar`
- `PIL.Image.Image.putalpha.nuanced.ycbcr-unsupported`
- `PIL.Image.Image.putdata.nuanced.clipped-values`
- `PIL.Image.Image.putdata.nuanced.cmyk-tuples`
- `PIL.Image.Image.putdata.nuanced.f-mode`
- `PIL.Image.Image.putdata.nuanced.i-mode`
- `PIL.Image.Image.putdata.nuanced.l-bytes`
- `PIL.Image.Image.putdata.nuanced.la-tuples`
- `PIL.Image.Image.putdata.nuanced.one-mode`
- `PIL.Image.Image.putdata.nuanced.p-indices`
- `PIL.Image.Image.putdata.nuanced.rgb-tuples-scale-offset`
- `PIL.Image.Image.putdata.nuanced.rgba-clipped-tuples`
- `PIL.Image.Image.putdata.nuanced.rgba-flat`
- `PIL.Image.Image.putdata.nuanced.rgba-tuples`
- `PIL.Image.Image.putdata.nuanced.scale-offset`
- `PIL.Image.Image.putpalette.nuanced.invalid-rawmode`
- `PIL.Image.Image.putpalette.nuanced.la-palette`
- `PIL.Image.Image.putpalette.nuanced.la-palette-l-image`
- `PIL.Image.Image.putpalette.nuanced.la-receiver`
- `PIL.Image.Image.putpalette.nuanced.oversized-la-palette`
- `PIL.Image.Image.putpalette.nuanced.oversized-rgb-palette`
- `PIL.Image.Image.putpalette.nuanced.oversized-rgba-palette`
- `PIL.Image.Image.putpalette.nuanced.rgba-palette`
- `PIL.Image.Image.putpixel.nuanced.one-tuple-equals-scalar`
- `PIL.Image.Image.putpixel.nuanced.p-index`
- `PIL.Image.Image.putpixel.nuanced.p-mode-rgb-color`
- `PIL.Image.Image.putpixel.nuanced.p-one-tuple-index`
- `PIL.Image.Image.putpixel.nuanced.p-palette-append`
- `PIL.Image.Image.putpixel.nuanced.p-palette-exact-match`
- `PIL.Image.Image.putpixel.nuanced.p-rgba-tuple-error`
- `PIL.Image.Image.putpixel.nuanced.p-tuple`
- `PIL.Image.Image.quantize.nuanced.fast-octree-rgb`
- `PIL.Image.Image.quantize.nuanced.libimagequant-unavailable`
- `PIL.Image.Image.quantize.nuanced.maxcoverage-16`
- `PIL.Image.Image.quantize.nuanced.maxcoverage-32-colors`
- `PIL.Image.Image.quantize.nuanced.maxcoverage-4`
- `PIL.Image.Image.quantize.nuanced.maxcoverage-kmeans-1`
- `PIL.Image.Image.quantize.nuanced.maxcoverage-kmeans-2`
- `PIL.Image.Image.quantize.nuanced.maxcoverage-kmeans-5`
- `PIL.Image.Image.quantize.nuanced.mediancut-16`
- `PIL.Image.Image.quantize.nuanced.mediancut-32-colors`
- `PIL.Image.Image.quantize.nuanced.mediancut-4`
- `PIL.Image.Image.quantize.nuanced.mediancut-default`
- `PIL.Image.Image.quantize.nuanced.mediancut-kmeans-1`
- `PIL.Image.Image.quantize.nuanced.mediancut-kmeans-2`
- `PIL.Image.Image.quantize.nuanced.rgba-mediancut-invalid`
- `PIL.Image.Image.reduce.nuanced.non-square-factors`
- `PIL.Image.Image.reduce.nuanced.odd-size-factor-three`
- `PIL.Image.Image.remap_palette.nuanced.attached-alpha-remap`
- `PIL.Image.Image.remap_palette.nuanced.explicit-rgba-source-palette`
- `PIL.Image.Image.remap_palette.nuanced.oversized-dest-map`
- `PIL.Image.Image.resize.nuanced.box-filter`
- `PIL.Image.Image.resize.nuanced.hamming-filter`
- `PIL.Image.Image.resize.nuanced.noninteger-ratio-lanczos`
- `PIL.Image.Image.resize.nuanced.opened-rgb`
- `PIL.Image.Image.rotate.nuanced.fractional-expanded`
- `PIL.Image.Image.rotate.nuanced.opened-rgb`
- `PIL.Image.Image.save.nuanced.f-png-error`
- `PIL.Image.Image.save.nuanced.i-bmp-error`
- `PIL.Image.Image.save.nuanced.i-png`
- `PIL.Image.Image.save.nuanced.opened-rgb`
- `PIL.Image.Image.save.nuanced.opened-rgb-bmp`
- `PIL.Image.Image.save.nuanced.p-png`
- `PIL.Image.Image.save.nuanced.p-short-palette`
- `PIL.Image.Image.save.nuanced.quantized-pipeline`
- `PIL.Image.Image.save.nuanced.rgb-nonzero`
- `PIL.Image.Image.split.nuanced.opened-p`
- `PIL.Image.Image.split.nuanced.opened-rgba`
- `PIL.Image.Image.split.nuanced.p-mode`
- `PIL.Image.Image.tobytes.nuanced.rgb-bgr-raw`
- `PIL.Image.Image.tobytes.nuanced.rgba-bgra-raw`
- `PIL.Image.Image.transform.nuanced.p-affine-scalar-fill`
- `PIL.Image.Image.transform.nuanced.p-affine-tuple-fill`
- `PIL.Image.Image.transform.nuanced.rgb-affine-tuple-fill`
- `PIL.Image.Image.transpose.nuanced.opened-rgb`
- `PIL.Image.Image.verify.nuanced.bmp-rgb-opened`
- `PIL.Image.Image.verify.nuanced.jpeg-rgb-opened`
- `PIL.Image.Image.verify.nuanced.png-rgb-opened`
- `PIL.Image.Image.verify.nuanced.quantized-pipeline`
- `PIL.Image.Image.verify.nuanced.resize-pipeline`
- `PIL.Image.Image.verify.nuanced.tiff-rgb-opened`
- `PIL.Image.Image.verify.nuanced.webp-rgb-opened`
- `PIL.Image.Image.verify.nuanced.webp-rgba-opened`
- `PIL.Image.alpha_composite.nuanced.mismatched-sizes`
- `PIL.Image.composite.nuanced.one-mask`
- `PIL.Image.composite.nuanced.rgba-mask`
- `PIL.Image.effect_mandelbrot.nuanced.quality-200`
- `PIL.Image.effect_mandelbrot.nuanced.quality-one-error`
- `PIL.Image.effect_mandelbrot.nuanced.zero-size`
- `PIL.Image.eval.nuanced.clamp-shift-callable`
- `PIL.Image.eval.nuanced.rgb-replicated-lut`
- `PIL.Image.frombytes.nuanced.valid-cmyk`
- `PIL.Image.frombytes.nuanced.valid-f`
- `PIL.Image.frombytes.nuanced.valid-hsv`
- `PIL.Image.frombytes.nuanced.valid-i`
- `PIL.Image.frombytes.nuanced.valid-l`
- `PIL.Image.frombytes.nuanced.valid-la`
- `PIL.Image.frombytes.nuanced.valid-p`
- `PIL.Image.frombytes.nuanced.valid-packed`
- `PIL.Image.frombytes.nuanced.valid-rgba`
- `PIL.Image.frombytes.nuanced.valid-ycbcr`
- `PIL.Image.linear_gradient.nuanced.f-mode`
- `PIL.Image.linear_gradient.nuanced.i-mode`
- `PIL.Image.new.nuanced.float-scalar-f`
- `PIL.Image.new.nuanced.integer-scalar-i`
- `PIL.Image.new.nuanced.rejected-leading-space-string`
- `PIL.Image.new.nuanced.rejected-rgb-nondigit-string`
- `PIL.Image.new.nuanced.rejected-rgb-short-string`
- `PIL.Image.new.nuanced.rejected-rgba-nondigit-string`
- `PIL.Image.new.nuanced.rejected-rgba-short-string`
- `PIL.Image.new.nuanced.rgb-percent-string`
- `PIL.Image.new.nuanced.rgb-string`
- `PIL.Image.new.nuanced.rgba-string`
- `PIL.Image.new.nuanced.tuple-p`
- `PIL.Image.open.nuanced.formats-accepted`
- `PIL.Image.open.nuanced.formats-rejected`
- `PIL.Image.radial_gradient.nuanced.f-mode`
- `PIL.Image.radial_gradient.nuanced.i-mode`
- `PIL.ImageChops.add.nuanced.scale-offset`
- `PIL.ImageChops.blend.nuanced.extrapolate-alpha`
- `PIL.ImageChops.invert.nuanced.la`
- `PIL.ImageChops.invert.nuanced.rgba`
- `PIL.ImageChops.subtract.nuanced.scale-offset`
- `PIL.ImageColor.getcolor.nuanced.hex-la-alpha`
- `PIL.ImageColor.getcolor.nuanced.hex-rgba`
- `PIL.ImageColor.getcolor.nuanced.named-cmyk`
- `PIL.ImageColor.getcolor.nuanced.named-f`
- `PIL.ImageColor.getcolor.nuanced.named-hsv`
- `PIL.ImageColor.getcolor.nuanced.named-i`
- `PIL.ImageColor.getcolor.nuanced.named-i16`
- `PIL.ImageColor.getcolor.nuanced.named-i16b`
- `PIL.ImageColor.getcolor.nuanced.named-l`
- `PIL.ImageColor.getcolor.nuanced.named-la`
- `PIL.ImageColor.getcolor.nuanced.named-one`
- `PIL.ImageColor.getcolor.nuanced.named-rgba`
- `PIL.ImageColor.getcolor.nuanced.over-range-hsv`
- `PIL.ImageColor.getcolor.nuanced.over-range-l`
- `PIL.ImageColor.getcolor.nuanced.rgb-syntax-l`
- `PIL.ImageColor.getrgb.nuanced.hex-with-alpha`
- `PIL.ImageColor.getrgb.nuanced.hsl-syntax`
- `PIL.ImageColor.getrgb.nuanced.named-css-color`
- `PIL.ImageColor.getrgb.nuanced.rejected-hsla`
- `PIL.ImageColor.getrgb.nuanced.rejected-rgba-float-alpha`
- `PIL.ImageColor.getrgb.nuanced.rejected-rgba-short`
- `PIL.ImageColor.getrgb.nuanced.rejected-transparent`
- `PIL.ImageColor.getrgb.nuanced.rgb-over-range`
- `PIL.ImageColor.getrgb.nuanced.rgb-percent`
- `PIL.ImageColor.getrgb.nuanced.rgb-percent-over-range`
- `PIL.ImageColor.getrgb.nuanced.rgba-over-range`
- `PIL.ImageColor.getrgb.nuanced.rgba-syntax`
- `PIL.ImageDraw.ImageDraw.arc.nuanced.fill-width`
- `PIL.ImageDraw.ImageDraw.bitmap.nuanced.canvas-1-one-mask`
- `PIL.ImageDraw.ImageDraw.bitmap.nuanced.canvas-cmyk`
- `PIL.ImageDraw.ImageDraw.bitmap.nuanced.canvas-f-tuple-fill-error`
- `PIL.ImageDraw.ImageDraw.bitmap.nuanced.canvas-hsv`
- `PIL.ImageDraw.ImageDraw.bitmap.nuanced.canvas-l-l-mask`
- `PIL.ImageDraw.ImageDraw.bitmap.nuanced.canvas-l-one-mask`
- `PIL.ImageDraw.ImageDraw.bitmap.nuanced.canvas-la`
- `PIL.ImageDraw.ImageDraw.bitmap.nuanced.canvas-p`
- `PIL.ImageDraw.ImageDraw.bitmap.nuanced.canvas-rgb-l-mask`
- `PIL.ImageDraw.ImageDraw.bitmap.nuanced.canvas-rgb-one-mask`
- `PIL.ImageDraw.ImageDraw.bitmap.nuanced.canvas-rgb-rgba-mask`
- `PIL.ImageDraw.ImageDraw.bitmap.nuanced.canvas-rgba-rgba-mask`
- `PIL.ImageDraw.ImageDraw.bitmap.nuanced.canvas-ycbcr`
- `PIL.ImageDraw.ImageDraw.chord.nuanced.fill-outline`
- `PIL.ImageDraw.ImageDraw.circle.nuanced.bbox`
- `PIL.ImageDraw.ImageDraw.ellipse.nuanced.canvas-la`
- `PIL.ImageDraw.ImageDraw.ellipse.nuanced.fill-outline-width`
- `PIL.ImageDraw.ImageDraw.line.nuanced.canvas-ycbcr-int-fill`
- `PIL.ImageDraw.ImageDraw.line.nuanced.flat-points`
- `PIL.ImageDraw.ImageDraw.line.nuanced.wide`
- `PIL.ImageDraw.ImageDraw.line.nuanced.wide-joint-curve`
- `PIL.ImageDraw.ImageDraw.line.nuanced.width-three`
- `PIL.ImageDraw.ImageDraw.multiline_text.nuanced.centered-anchored`
- `PIL.ImageDraw.ImageDraw.multiline_text.nuanced.three-line-spacing`
- `PIL.ImageDraw.ImageDraw.pieslice.nuanced.fill-outline-width`
- `PIL.ImageDraw.ImageDraw.point.nuanced.flat-points`
- `PIL.ImageDraw.ImageDraw.point.nuanced.la-canvas`
- `PIL.ImageDraw.ImageDraw.polygon.nuanced.canvas-cmyk`
- `PIL.ImageDraw.ImageDraw.polygon.nuanced.fill-outline`
- `PIL.ImageDraw.ImageDraw.polygon.nuanced.outline-width`
- `PIL.ImageDraw.ImageDraw.polygon.nuanced.paired-points`
- `PIL.ImageDraw.ImageDraw.polygon.nuanced.two-points-line`
- `PIL.ImageDraw.ImageDraw.polygon.nuanced.two-points-outline`
- `PIL.ImageDraw.ImageDraw.rectangle.nuanced.canvas-1`
- `PIL.ImageDraw.ImageDraw.rectangle.nuanced.canvas-cmyk`
- `PIL.ImageDraw.ImageDraw.rectangle.nuanced.canvas-hsv-int-fill`
- `PIL.ImageDraw.ImageDraw.rectangle.nuanced.canvas-l`
- `PIL.ImageDraw.ImageDraw.rectangle.nuanced.canvas-la`
- `PIL.ImageDraw.ImageDraw.rectangle.nuanced.canvas-p`
- `PIL.ImageDraw.ImageDraw.rectangle.nuanced.fill-outline-width`
- `PIL.ImageDraw.ImageDraw.rectangle.nuanced.nested-box`
- `PIL.ImageDraw.ImageDraw.regular_polygon.nuanced.heptagon-rotated`
- `PIL.ImageDraw.ImageDraw.regular_polygon.nuanced.pentagon-rotated`
- `PIL.ImageDraw.ImageDraw.regular_polygon.nuanced.rotated-hexagon`
- `PIL.ImageDraw.ImageDraw.regular_polygon.nuanced.triangle`
- `PIL.ImageDraw.ImageDraw.rounded_rectangle.nuanced.radius`
- `PIL.ImageDraw.ImageDraw.rounded_rectangle.nuanced.radius-zero-fallback`
- `PIL.ImageDraw.ImageDraw.shape.nuanced.canvas-f-default-ink`
- `PIL.ImageDraw.ImageDraw.shape.nuanced.canvas-i-default-ink`
- `PIL.ImageDraw.ImageDraw.shape.nuanced.canvas-l-default-ink`
- `PIL.ImageDraw.ImageDraw.shape.nuanced.curve-outline`
- `PIL.ImageDraw.ImageDraw.shape.nuanced.filled-and-outlined`
- `PIL.ImageDraw.ImageDraw.text.nuanced.canvas-1`
- `PIL.ImageDraw.ImageDraw.text.nuanced.canvas-cmyk`
- `PIL.ImageDraw.ImageDraw.text.nuanced.canvas-f`
- `PIL.ImageDraw.ImageDraw.text.nuanced.canvas-hsv`
- `PIL.ImageDraw.ImageDraw.text.nuanced.canvas-i`
- `PIL.ImageDraw.ImageDraw.text.nuanced.canvas-l`
- `PIL.ImageDraw.ImageDraw.text.nuanced.canvas-l-small`
- `PIL.ImageDraw.ImageDraw.text.nuanced.canvas-la`
- `PIL.ImageDraw.ImageDraw.text.nuanced.canvas-one-small`
- `PIL.ImageDraw.ImageDraw.text.nuanced.canvas-p`
- `PIL.ImageDraw.ImageDraw.text.nuanced.canvas-p-small`
- `PIL.ImageDraw.ImageDraw.text.nuanced.canvas-rgba-tuple-fill`
- `PIL.ImageDraw.ImageDraw.text.nuanced.canvas-ycbcr`
- `PIL.ImageDraw.ImageDraw.text.nuanced.negative-position`
- `PIL.ImageDraw.ImageDraw.text.nuanced.stroked-rgba`
- `PIL.ImageDraw.ImageDraw.text.nuanced.unicode-anchor`
- `PIL.ImageDraw.ImageDraw.textbbox.nuanced.unicode-anchor`
- `PIL.ImageFilter.Kernel.nuanced.bad-size`
- `PIL.ImageFilter.Kernel.nuanced.five-by-five`
- `PIL.ImageFilter.Kernel.nuanced.short-kernel`
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
- `PIL.ImageOps.autocontrast.nuanced.p-mode`
- `PIL.ImageOps.autocontrast.nuanced.unsupported-mode-cmyk`
- `PIL.ImageOps.autocontrast.nuanced.unsupported-mode-f`
- `PIL.ImageOps.autocontrast.nuanced.unsupported-mode-i`
- `PIL.ImageOps.autocontrast.nuanced.unsupported-mode-one`
- `PIL.ImageOps.colorize.nuanced.invalid-mid-points`
- `PIL.ImageOps.colorize.nuanced.invalid-points`
- `PIL.ImageOps.colorize.nuanced.mapped-points`
- `PIL.ImageOps.colorize.nuanced.mode-one`
- `PIL.ImageOps.colorize.nuanced.three-color`
- `PIL.ImageOps.colorize.nuanced.two-color`
- `PIL.ImageOps.equalize.nuanced.unsupported-mode-cmyk`
- `PIL.ImageOps.equalize.nuanced.unsupported-mode-f`
- `PIL.ImageOps.equalize.nuanced.unsupported-mode-i`
- `PIL.ImageOps.equalize.nuanced.unsupported-mode-one`
- `PIL.ImageOps.fit.nuanced.fractional-centering`
- `PIL.ImageOps.invert.nuanced.p-mode`
- `PIL.ImagePalette.ImagePalette.getcolor.nuanced.rgb-tuple-append`
- `PIL.ImagePalette.ImagePalette.getcolor.nuanced.rgba-tuple-append`
- `PIL.ImagePalette.ImagePalette.getcolor.nuanced.short-tuple-append`
- `PIL.ImageStat.Stat.extrema.nuanced.cmyk-mode`
- `PIL.ImageStat.Stat.extrema.nuanced.f-mode`
- `PIL.ImageStat.Stat.extrema.nuanced.i-mode`
- `PIL.ImageStat.Stat.extrema.nuanced.la-mode`
- `PIL.ImageStat.Stat.extrema.nuanced.one-mode`
- `PIL.ImageStat.Stat.extrema.nuanced.p-mode`
- `PIL.ImageStat.Stat.nuanced.from-histogram-list`

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
