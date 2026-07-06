# Manifest Review Checklist

`tests/manifest.yaml` is the source of truth for the public C FreeType
interface. The generated catalog must be reviewed in 20-entry slices before it
is treated as complete.

## Worker Contract

Each worker owns one contiguous manifest slice by zero-based subject index.
Workers must not edit `tests/manifest.yaml` directly. They write a proposed
slice artifact:

```text
pillow-rs-freetype/manifest-review/manifest.<offset>.<limit>.yaml
```

For example, offset `120`, limit `20` writes:

```text
pillow-rs-freetype/manifest-review/manifest.0120.0020.yaml
```

The artifact must contain only the reviewed replacement entries for that slice,
plus YAML comments for review notes when needed. The orchestrating agent merges
reviewed artifacts into `tests/manifest.yaml`.

For every assigned subject, verify and enrich:

- C public status: confirm it belongs to the public C interface and is not
  header/config/internal plumbing.
- Header namespace: confirm the subject id matches the declaring public header
  stem, with `freetype/freetype.h` using `freetype`.
- Kind: confirm `function`, `type`, `record`, `enum`, `enum_variant`,
  `constant`, `flag`, `error`, `tag`, or `macro`.
- Cases: ensure each subject has meaningful coverage cases. Do not leave a
  generic case if success/error/value/layout/font-driven behavior needs more
  precise cases.
- Inputs: identify whether existing fixture inputs already cover a case; if
  new inputs are needed, add a `# needs-input:` note with exact operation,
  fixture asset, params, expected output shape, and comparison rule.
- Error paths: for functions and macros that can fail or classify errors, add
  explicit error-case intent where applicable.
- Success paths: for functions and data-bearing APIs, add explicit successful
  behavior case intent where applicable.
- Runtime variability: add or retain `coverage.font_variability` only where
  font/size/glyph/load/render variation changes observable output.

Do not weaken existing runtime cases or remove font variability requirements.

## Slice Status

Legend:

```text
pending   not assigned
active    assigned to worker
returned  worker artifact received
merged    artifact reviewed and merged into manifest.yaml
blocked   worker found ambiguity needing owner decision
```

| offset | limit | status | owner | focus |
| ---: | ---: | --- | --- | --- |
| 0 | 20 | merged | wave1-0000 | freetype.h enums and FT_Encoding variants |
| 20 | 20 | merged | wave1-0020 | freetype.h enum variants and render/size variants |
| 40 | 20 | merged | wave1-0040 | face flags and FSType flags |
| 60 | 20 | merged | wave1-0060 | load flags |
| 80 | 20 | merged | wave1-0080 | load target/open/style flags |
| 100 | 20 | merged | wave1-0100 | subglyph flags and early freetype.h functions |
| 120 | 20 | merged | wave1-0120 | charmap, kerning, load, and query functions |
| 140 | 20 | merged | wave1-0140 | face creation/render/size functions and face macros |
| 160 | 20 | merged | wave1-0160 | face macros and freetype.h records |
| 180 | 20 | merged | wave1-0180 | freetype.h records/types and first secondary headers |
| 200 | 20 | merged | wave2-0200 | bdf/bitmap/bzip2/cache slice |
| 220 | 20 | merged | wave2-0220 | cache/color slice |
| 240 | 20 | merged | wave2-0240 | color enums/constants/functions |
| 260 | 20 | merged | wave2-0260 | color records/types |
| 280 | 20 | merged | wave2-0280 | color paint/functions |
| 300 | 20 | merged | wave2-0300 | color records |
| 320 | 20 | merged | wave2-0320 | color records/types and driver constants |
| 340 | 20 | merged | wave2-0340 | driver props and error codes |
| 360 | 20 | merged | wave2-0360 | error codes |
| 380 | 20 | merged | wave2-0380 | error codes and glyph APIs |
| 400 | 20 | merged | wave3-0400 | glyph/gx validation |
| 420 | 20 | merged | wave3-0420 | gx validation/image |
| 440 | 20 | merged | wave3-0440 | image flags/functions/macros |
| 460 | 20 | merged | wave3-0460 | image records/types |
| 480 | 20 | merged | wave3-0480-local | gx validation flags/indexes |
| 500 | 20 | merged | wave3-0500 | mm constants/functions |
| 520 | 20 | merged | wave3-0520 | image callback macros and pixel/glyph format enum variants |
| 540 | 20 | merged | wave3-0540 | modapi/module flags |
| 560 | 20 | merged | wave3-0560 | modapi/module records/types |
| 580 | 20 | merged | wave3-0580 | incremental/lcd/list APIs |
| 600 | 20 | merged | wave4-0600 | outline records/types/ot validation |
| 620 | 20 | merged | wave4-0620 | params/pfr/render/sfnt |
| 640 | 20 | merged | wave4-0640 | size/stroker |
| 660 | 20 | merged | wave4-0660 | stroker |
| 680 | 20 | merged | wave4-0680 | stroke enums/functions/types |
| 700 | 20 | merged | wave4-0700 | synth/trigon/tt tables |
| 720 | 20 | merged | wave4-0720 | tt tables/name ids |
| 740 | 20 | merged | wave4-0740 | ttnameid constants |
| 760 | 20 | merged | wave4-0760 | ttnameid constants |
| 780 | 20 | merged | wave4-0780 | ttnameid constants |
| 800 | 20 | merged | wave5-0800 | ttnameid constants |
| 820 | 20 | merged | wave5-0820 | ttnameid constants |
| 840 | 20 | merged | wave5-0840 | ttnameid constants |
| 860 | 20 | merged | wave5-0860 | ttnameid constants |
| 880 | 20 | merged | wave5-0880 | ttnameid constants |
| 900 | 20 | merged | wave5-0900 | ttnameid constants |
| 920 | 20 | merged | wave5-0920 | ttnameid constants |
| 940 | 20 | merged | wave5-0940 | ttnameid constants |
| 960 | 20 | merged | wave5-0960 | ttnameid constants |
| 980 | 20 | merged | wave5-0980 | ttnameid constants |
| 1000 | 20 | merged | wave6-1000 | ttnameid constants |
| 1020 | 20 | merged | wave6-1020 | ttnameid constants |
| 1040 | 20 | merged | wave6-1040 | ttnameid constants |
| 1060 | 20 | merged | wave6-1060 | ttnameid constants |
| 1080 | 20 | merged | wave6-1080 | ttnameid constants |
| 1100 | 20 | merged | wave6-1100 | ttnameid constants |
| 1120 | 20 | merged | wave6-1120 | ttnameid constants |
| 1140 | 20 | merged | wave6-1140 | ttnameid constants |
| 1160 | 20 | merged | wave6-1160 | ttnameid constants |
| 1180 | 20 | merged | wave6-1180 | ttnameid constants |
| 1200 | 20 | merged | wave7-1200 | ttnameid constants |
| 1220 | 20 | merged | wave7-1220 | ttnameid constants |
| 1240 | 20 | merged | wave7-1240 | ttnameid constants |
| 1260 | 20 | merged | wave7-1260 | ttnameid constants |
| 1280 | 20 | merged | wave7-1280 | ttnameid constants |
| 1300 | 20 | merged | wave7-1300 | ttnameid constants |
| 1320 | 20 | merged | wave7-1320 | ttnameid constants |
| 1340 | 20 | merged | wave7-1340 | ttnameid constants |
| 1360 | 20 | merged | wave7-1360 | ttnameid constants |
| 1380 | 20 | merged | wave7-1380 | ttnameid constants |
| 1400 | 20 | merged | wave8-1400 | ttnameid constants |
| 1420 | 20 | merged | wave8-1420 | ttnameid constants/types |
| 1440 | 20 | merged | wave8-1440 | tttags/type1/winfont |
| 1460 | 20 | merged | wave8-1460 | winfont constants |
| 1480 | 20 | merged | wave8-1480 | winfont constants |
| 1500 | 20 | merged | wave8-1500 | winfont constants |
| 1520 | 20 | merged | wave8-1520 | winfont constants |
| 1540 | 4 | merged | wave8-1540 | final winfont constants |
