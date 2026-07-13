# Font Fixture Inventory

Status: active inventory
Recorded: 2026-07-13
Coverage plan: `doc/FONT_FIXTURE_COVERAGE_PLAN.md`

This inventory separates file paths, stored binaries, unique contents, cmap
reachability, distinct glyph geometry, and actual public-input selection. A font
name or Unicode mapping does not count as useful variability unless an explicit
input selects a glyph whose geometry or font tables enter a distinct behavior.

## Corpus Totals

| Corpus | Paths | Stored files | Symlinks | Unique SHA-256 contents | Stored size |
|---|---:|---:|---:|---:|---:|
| Active fixtures | 152 | 109 | 43 | 120 | 818 KiB |
| Deprecated corpus | 101 | 101 | 0 | 99 | 23 MiB |
| Compact active autohint set | 7 | 7 | 0 | 7 | 193 KiB |

All 43 active symlinks resolve. `fonts/metrics/fixed-width.ttf` is now an
independent focused binary rather than an alias into the deprecated corpus.

## Inventory Parameters

Every retained or newly created font is evaluated on:

- Content identity, byte size, container, face count, and outline format.
- Table set, especially cmap, glyf/CFF, hmtx/vmtx, hdmx, kern/GPOS, name/post,
  native hint programs, gasp, bitmap, color, SVG, and variation tables.
- Glyph count and Unicode cmap count.
- Script reachability and whether scripts have distinct glyph geometry.
- Selected glyph IDs, codepoints, contour/component topology, advances,
  bearings, and instruction programs.
- Explicit public cases and condition/branch obligations.
- Exact duplicates, supersets, and properties not selected by any public input.

## Active Unique Contents

`Paths` counts all resolving aliases for the content. Script names describe
Unicode cmap reachability, not proof of distinct script geometry.

| Hash | KiB | Paths | Canonical stored path | Format and distinguishing properties |
|---|---:|---:|---|---|
| `7078fe9e41a8` | 0.03 | 3 | `assets/fonts/module-dependent-type1.pfa` | intentionally invalid/minimal Type 1 control |
| `4c44206cbb02` | 17.2 | 72 | `input/fonts/DejaVuSans.ttf` | glyf, 131 glyphs, 18 Unicode, native hinting; compact general control |
| `e6bc1ae1a7f7` | 11.5 | 1 | `fonts/autohint/basic-latin.ttf` | glyf, 42 glyphs, 45 Unicode mappings, native hinting, GPOS |
| `f6fa317b2fbb` | 2.4 | 1 | `fonts/autohint/cjk-coverage.ttf` | source-backed glyf, 22 glyphs, 37 Unicode mappings, vertical metrics, CJK geometry plus Latin adjustment, blue-zone, stem-sort, and serif topologies |
| `b17d2e85af72` | 0.7 | 1 | `fonts/autohint/cjk-width-order.ttf` | source-backed glyf, 2 glyphs, one U+56D7 mapping, no U+7530; owns Hani fallback-standard width ordering with descending stems |
| `99ab69fa2b56` | 1.1 | 1 | `fonts/autohint/digit-notdef-cmap.ttf` | source-backed glyf, 3 glyphs, U+0030 explicitly maps to glyph 0 while U+006F selects a Latin ring; owns the face-global digit-width scan glyph-zero branch |
| `38431987e24e` | 11.7 | 1 | `fonts/autohint/indic-coverage.ttf` | glyf, 37 glyphs, 29 mappings, native hinting, Devanagari geometry |
| `b4ff2e5f559c` | 10.7 | 1 | `fonts/autohint/latin-greek-cyrillic.ttf` | glyf, 39 glyphs, distinct Latin/Greek/Cyrillic geometry |
| `053fcf674ea8` | 1.1 | 1 | `fonts/autohint/latin-malformed-standard.ttf` | source-backed glyf, 4 glyphs, valid U+0041 load glyph plus U+006F mapped to a deliberately truncated final glyph; owns the ignored malformed Latin standard-character load during autohint metrics setup |
| `1ff7a44a2e54` | 5.5 | 1 | `fonts/autohint/script-coverage.ttf` | source-backed glyf, 61 glyphs, 109 Unicode mappings, one compact glyph per autohint script tag plus standard-character aliases |
| `c454a86ebd36` | 143.3 | 1 | `fonts/native/dejavu-coverage.ttf` | retain-GID glyf, 6,128 slots and 121 Unicode mappings; Latin/Greek blue strings, Cyrillic, emoji, native hinting, legacy kern, GPOS/GSUB/GDEF, MATH, post names; controlled format-4 `idRangeOffset` segment |
| `8f92c06e515e` | 771.3 | 211 | `input/fonts/generated/sfnt-name-records.ttf` | glyf, 6,253 glyphs, broad cmap, name-record source for 211 aliases |
| `a94c8f524e93` | 0.7 | 1 | `fonts/color/colr-v0-foreground-layer-ffff.ttf` | intentionally parser-rejected color edge fixture |
| `7fb197f341db` | 361.2 | 3 | `input/fonts/generated/kerning/no-kern-table.ttf` | glyf, 2,602 glyphs, no kern/GPOS control |
| `fa111fe3db40` | 740.8 | 2 | `input/fonts/generated/fstype/editable-embedding.ttf` | broad glyf font with editable embedding OS/2 bits |
| `afd8bf872cb5` | 740.8 | 2 | `input/fonts/generated/fstype/no-subsetting.ttf` | broad glyf font with no-subsetting OS/2 bit |
| `29c1b1813188` | 740.8 | 2 | `input/fonts/generated/fstype/preview-print.ttf` | broad glyf font with preview/print OS/2 bits |
| `b9d2ba2872e4` | 740.8 | 2 | `input/fonts/generated/fstype/restricted-license.ttf` | broad glyf font with restricted-license OS/2 bits |
| `14bf865633cc` | 366.4 | 3 | `input/fonts/generated/kerning/legacy-av-kern.ttf` | glyf, 2,602 glyphs, legacy kern pair |
| `a0fe392f2c97` | 740.8 | 2 | `input/fonts/generated/fstype/bitmap-embedding-only.ttf` | broad glyf font with bitmap-only embedding bit |
| `684a1fd94057` | 16.9 | 1 | `fonts/metrics/fixed-width.ttf` | compact glyf, 131 glyphs, post fixed-pitch flag, uniform 1,401-unit advances |
| `3296851697c9` | 17.6 | 1 | `fonts/metrics/hdmx_observable.ttf` | compact glyf, 131 glyphs, native hinting plus hdmx |
| `2f2fa75c7c3b` | 1.8 | 1 | `fonts/metrics/vertical-vhea-only.ttf` | compact glyf negative control with vhea present and vmtx absent |
| `17f780c4bcbe` | 4.1 | 1 | `fonts/metrics/hhea-zero-typo-fallback.ttf` | compact glyf with zero hhea ascent/descent/lineGap and nonzero OS/2 typo metrics; owns FreeType's hhea-zero typo fallback for face and size metrics |
| `233f86bc5e71` | 4.1 | 1 | `fonts/metrics/hhea-zero-win-fallback.ttf` | compact glyf with zero hhea and zero OS/2 typo metrics plus nonzero OS/2 Windows metrics; owns FreeType's final Windows metric fallback |
| `6f630b9ef12c` | 17.3 | 1 | `fonts/metadata/style-bold-italic.ttf` | compact glyf, head macStyle bold+italic, OS/2 weight 700, post italic angle |
| `b77a0b580098` | 0.6 | 2 | `fonts/type1/simple-type1.pfb` | focused binary Type 1 fixture |
| `5a2f6febcb80` | 754.2 | 3 | `input/fonts/generated/vertical/cjk-vertical-metrics.ttf` | broad glyf font with vhea/vmtx |
| `f1ad285ec056` | 0.6 | 1 | `generated/sfnt/zero-units-per-em-autohint.ttf` | intentionally invalid units-per-em edge fixture |
| `2cf2f480ecb6` | 12.3 | 1 | `input/fonts/LiberationSerif-Regular.ttf` | compact glyf, 131 glyphs, 17 Latin mappings |
| `c950ae3feb03` | 9.7 | 1 | `input/fonts/NotoSans-Regular.ttf` | compact glyf, 131 glyphs, 18 Latin mappings |
| `9d7fb1debb16` | 740.8 | 2 | `input/fonts/generated/fstype/installable-fstype.ttf` | broad glyf font with installable embedding bits |
| `8040be090b5b` | 740.8 | 2 | `input/fonts/generated/fstype/restricted-no-subset.ttf` | broad glyf font with combined OS/2 restrictions |
| `84dbef30b5ea` | 681.3 | 2 | `input/fonts/generated/face-properties/no-post-names.ttf` | broad glyf font without post glyph names |
| `2a314e9ffb38` | 379.4 | 2 | `input/fonts/generated/kerning/gpos-only-av.ttf` | glyf, 2,602 glyphs, GPOS-only AV adjustment |
| `7d4ee1626b98` | 743.4 | 52 | `input/fonts/generated/os2-unicode-ranges.ttf` | broad multiscript cmap and OS/2 Unicode/codepage range source |
| `b85c38ecea8a` | 555.9 | 1 | `input/fonts/generated/variable/ubuntu-sans-variable.ttf` | glyf font with STAT but no fvar/avar/gvar; retained non-variable control despite filename |
| `c7ed80798946` | 8.9 | 3 | `fonts/variable/compact-variable.ttf` | 20-glyph variable glyf; fvar/avar/gvar/HVAR/STAT, 2 axes, 12 named instances |
| `7594a1df018a` | 8.7 | 1 | `fonts/variable/fvar-short.ttf` | compact generated malformed control with an 8-byte truncated fvar header |
| `9aa6f372453b` | 8.7 | 1 | `fonts/variable/fvar-version-2.ttf` | compact generated malformed control with unsupported fvar major version 2 |
| `18652ff465b1` | 8.9 | 1 | `fonts/variable/fvar-axis-size-short.ttf` | compact generated malformed control with fvar axis records one byte below the required 20-byte OpenType size |
| `3380d1c030f9` | 8.8 | 1 | `fonts/variable/fvar-instance-array-short.ttf` | compact generated malformed control with a declared fvar instance array beyond table EOF |
| `cc6cc4e2f726` | 8.9 | 1 | `fonts/variable/fvar-instance-size-short.ttf` | compact generated malformed control with an instance record one byte below the two-axis minimum |
| `487a56138ec6` | 8.9 | 1 | `fonts/variable/fvar-instance-postscript-name.ttf` | compact generated variable control with explicit fvar instance PostScript name IDs |
| `fa98aa0ffd8e` | 7.9 | 1 | `fonts/variable/variable-name-apple-prefix.ttf` | compact generated variable control whose encoded named instance uses Apple-only nameID 25 and subfamily records plus an unsupported name record |
| `64de26dc0261` | 7.9 | 1 | `fonts/variable/variable-name-unicode-prefix.ttf` | compact generated variable control whose encoded named instance has Unicode-only nameID 25 and subfamily records, proving FreeType ignores the Unicode variation prefix while accepting the Unicode subfamily |
| `3bd1b44980a3` | 7.9 | 1 | `fonts/variable/variable-name-odd-win-prefix.ttf` | compact generated variable control whose encoded named instance has an odd-length Windows nameID 25 plus Apple Roman fallback, proving invalid Windows variation prefixes fall through |
| `3aedb3aabe04` | 7.9 | 1 | `fonts/variable/variable-name-missing-subfamily.ttf` | compact generated variable control whose encoded named instances lack usable subfamily names and force coordinate-based PostScript synthesis |
| `2b81d81f82a5` | 17.1 | 1 | `fonts/control/maxp-version-05.ttf` | six-byte maxp version 0.5 header; owns the below-1.0 zero-extra-profile path |
| `1c06b1400c33` | 17.2 | 1 | `fonts/control/maxp-version-2.ttf` | full maxp version 2.0; owns FreeType's version-at-least-1 extra-frame path |
| `c9a29ffba75b` | 17.1 | 1 | `fonts/control/maxp-v1-header-only.ttf` | version-1 maxp header at physical EOF; load failure is ignored into a zero profile |
| `26286ab7d37c` | 17.1 | 1 | `fonts/control/maxp-too-short.ttf` | five-byte maxp at physical EOF; owns ignored short-header failure and zero-glyph face construction |
| `14a976bde721` | 17.2 | 1 | `fonts/kerning/kern-short.ttf` | optional kern table shorter than its four-byte header |
| `1b640f3d44da` | 17.2 | 1 | `fonts/kerning/kern-header-missing.ttf` | 33 declared subtables with no subtable header; owns count cap and missing-header exit |
| `7efa2ab9e0b8` | 17.2 | 1 | `fonts/kerning/kern-length-14.ttf` | format-0 subtable at FreeType's rejected 14-byte length boundary |
| `b01081384eed` | 17.2 | 1 | `fonts/kerning/kern-truncated.ttf` | declared 100-byte kern subtable clamped to six available bytes |
| `8e1af1366d62` | 17.2 | 1 | `fonts/kerning/kern-coverage-matrix.ttf` | top version 1 plus unsupported-format, invalid-coverage, and valid A/V format-0 subtables |
| `1e857fba5bac` | 17.2 | 1 | `fonts/metrics/hdmx-short.ttf` | optional hdmx table shorter than its eight-byte header |
| `cc854edb7fde` | 17.6 | 1 | `fonts/metrics/hdmx-high-word-size.ttf` | valid records with FreeType's `0xFFFF0088` record-size repair form |
| `4de298d1f587` | 17.2 | 1 | `fonts/metrics/hdmx-zero-records.ttf` | zero-record rejection control |
| `3d2d43cbad0c` | 17.2 | 1 | `fonts/metrics/hdmx-256-records.ttf` | above-255 record-count rejection control |
| `807990b2b294` | 17.2 | 1 | `fonts/metrics/hdmx-size-mismatch.ttf` | record size one byte above the glyph-count-derived value |
| `d2b58ca3a3ac` | 17.2 | 1 | `fonts/metrics/hdmx-truncated-record.ttf` | valid header with absent record body |
| `a7c1a6193645` | 1.9 | 1 | `fonts/metrics/metrics-count-zero.ttf` | paired hhea/vhea zero long-metric counts with present hmtx/vmtx data |
| `d78900ad2720` | 1.9 | 1 | `fonts/metrics/metrics-count-overflow.ttf` | paired hhea/vhea counts one above the 11-glyph maximum |
| `1f0145f2facc` | 1.8 | 1 | `fonts/metrics/metrics-tables-short.ttf` | present one-byte hmtx and vmtx tables with otherwise valid headers |
| `f5a7badf5399` | 1.9 | 1 | `fonts/metrics/hhea-short-eof.ttf` | 35-byte hhea at physical EOF; required-header stream error control |
| `1eee7f2e8396` | 1.9 | 1 | `fonts/metrics/vhea-short-eof.ttf` | 35-byte vhea at physical EOF; present malformed vertical-header error control |
| `acb83f0642a9` | 16.9 | 1 | `fonts/metadata/short-os2-post.ttf` | compact glyf with a 77-byte OS/2 table and 15-byte post table; owns both optional short-table fallbacks |
| `12b8e116037d` | 3.0 | 1 | `fonts/metadata/post-format-1.ttf` | compact glyf with `post` format 1.0 and non-258 glyph count; owns FreeType's default `.notdef` glyph-name behavior |
| `2571fddb58ac` | 3.1 | 1 | `fonts/metadata/post-format-25.ttf` | compact glyf with FreeType's historical `post` format 2.5 tag `0x00025000`; owns valid signed-delta names and out-of-range deltas mapping to Mac glyph 0 |
| `dbe5cef750a9` | 3.0 | 1 | `fonts/metadata/post-format-unsupported.ttf` | compact glyf with unsupported `post` format 4.0; owns C's cleared-buffer `Invalid_Argument` public glyph-name behavior |
| `18fc60864980` | 3.0 | 1 | `fonts/metadata/post-format-20-short.ttf` | compact glyf with format 2.0 table shorter than the glyph-name count field; owns default-name fallback after ignored load failure |
| `712b4bbd33d0` | 3.0 | 1 | `fonts/metadata/post-format-20-zero.ttf` | compact glyf with format 2.0 declaring zero glyph names; owns the zero-count default-name path |
| `401588233b68` | 3.0 | 1 | `fonts/metadata/post-format-20-custom-truncated.ttf` | compact glyf with a format 2.0 custom-name index and no Pascal string bytes; owns the missing-custom-name `.notdef` fallback |
| `2f50d6f217bd` | 3.0 | 1 | `fonts/metadata/post-format-25-short.ttf` | compact glyf with format 2.5 table shorter than the glyph-name count field; owns default-name fallback after ignored load failure |
| `86500c6b0a3b` | 3.0 | 1 | `fonts/metadata/post-format-25-zero.ttf` | compact glyf with format 2.5 declaring zero glyph names; owns the zero-count default-name path |
| `149337a4d1da` | 3.0 | 1 | `fonts/metadata/post-format-25-too-many.ttf` | compact glyf with format 2.5 declaring 387 glyph deltas; owns FreeType's above-theoretical-limit rejection |
| `9e63ed2c07b6` | 17.2 | 1 | `fonts/metadata/os2-use-typo-metrics.ttf` | compact glyf with `USE_TYPO_METRICS`; owns OS/2 typographic face and size metric selection |
| `5df1e876cc25` | 1.9 | 1 | `fonts/metadata/head-short-eof.ttf` | 53-byte required head table at physical EOF; owns the short-header face-open error |
| `4e4f32fced92` | 1.6 | 1 | `fonts/names/name-record-matrix.ttf` | nine-record name table covering Windows decode failures, Mac Roman and arbitrary-Windows fallbacks, invalid ranges, zero lengths, and every preference predicate outcome |
| `936734c2d182` | 1.5 | 1 | `fonts/names/name-empty.ttf` | valid zero-record name table; owns the `Unknown` family and `Regular` subfamily defaults |
| `c2023eedc045` | 3.7 | 1 | `fonts/names/name-missing.ttf` | compact generated TrueType face with the optional `name` table removed; owns face construction fallback when the table is absent |
| `298447f992df` | 1.5 | 1 | `fonts/names/name-short.ttf` | five-byte name table; owns the short-header face-open error |
| `fad5b6b1057c` | 1.5 | 1 | `fonts/names/name-record-overflow.ttf` | six-byte name header declaring one absent record; owns the record-array overflow error |
| `ed624513a6fd` | 3.6 | 1 | `fonts/names/name-selection-fallbacks.ttf` | compact generated name-table control with unsupported platform, invalid Apple offset, Unicode family fallback, Apple-Roman subfamily, and Windows PostScript name |
| `e7ba7ed18dac` | 3.5 | 1 | `fonts/names/name-apple-postscript.ttf` | compact generated name-table control whose PostScript name is Apple Roman only |
| `4d73600e7fce` | 3.5 | 1 | `fonts/names/name-win-postscript-odd-apple.ttf` | compact generated name-table control with an odd-length Windows PostScript candidate that falls back to Apple Roman |
| `0a2206bc87ca` | 13.6 | 1 | `fonts/control/multiface-cjk-indic.ttc` | two-face collection combining the compact CJK and Indic fonts; owns TTC offset iteration and second-face selection |
| `a123d521381f` | 0.01 | 1 | `fonts/control/ttc-short.ttc` | eight-byte TTC short-header control |
| `0b66ccbde246` | 0.01 | 1 | `fonts/control/ttc-offset-overflow.ttc` | 12-byte TTC declaring one absent face offset |
| `1cca599d017f` | 0.02 | 1 | `fonts/control/ttc-face-offset-out-of-range.ttc` | complete one-face TTC header whose selected face offset is outside the stream |
| `debb925b92a2` | 0.01 | 1 | `fonts/control/otto-empty.otf` | 12-byte zero-table OTTO header; owns the OpenType sfVersion predicate outcome before required-table rejection |
| `bf004c57a16e` | 1.7 | 1 | `fonts/charmap/format6-range.ttf` | format-6-only trimmed range with mapped and zero glyph entries; owns direct lookup, gap, exhaustion, and wrapping iteration |
| `f059f367c976` | 4.2 | 1 | `fonts/cmap/cmap-format-language-matrix.ttf` | source-backed compact cmap matrix with format 4 language `0x0409`, format 6 language 17, format 12 language `0x12345678`, and format 14 `FE00` empty-selector plus `FE0F`/`E0101` default and non-default variation-selector metadata |
| `251fcf468057` | 2.1 | 1 | `fonts/charmap/cmap-parser-matrix.ttf` | one valid format 6 plus unsupported, out-of-range, and independently malformed format 4/6/12 records |
| `3facc94f8e88` | 4.4 | 1 | `fonts/charmap/cmap-format14-malformed-matrix.ttf` | raw cmap control with one valid format 6 plus malformed Unicode and non-Unicode format 14 short, physical-tail short, offset, selector-order, default-UVS, and non-default-UVS subtables |
| `03f64c98e923` | 4.1 | 1 | `fonts/charmap/cmap-nonunicode-format6.ttf` | compact Macintosh Roman format-6-only control proving public variant-index zero behavior when no active Unicode charmap exists |
| `1b33f5e1562` | 4.1 | 1 | `fonts/charmap/cmap-format14-only.ttf` | compact format-14-only control with `FE00`, `FE0F`, and `E0101` selector records and no direct Unicode lookup charmap |
| `217751378cbc` | 1.7 | 1 | `fonts/charmap/cmap-record-overflow.ttf` | cmap header declaring one absent encoding record |
| `93678acbb630` | 1.7 | 1 | `fonts/charmap/cmap-short.ttf` | three-byte cmap short-header control |
| `b085a0e2a109` | 1.7 | 1 | `fonts/charmap/format6-terminal.ttf` | format 6 range at U+FFFF with a zero glyph; owns terminal iteration |
| `b089ca982745` | 1.7 | 1 | `fonts/charmap/format12-zero-start.ttf` | format 12 groups beginning at glyph zero; owns within-group zero skipping and single-code zero fallback |
| `5b3b0195e1d7` | 1.7 | 1 | `fonts/charmap/format4-range-offset.ttf` | valid format 4 range-offset array with zero and mapped entries plus adjacent direct zero/nonzero segments |
| `19667c565a1b` | 1.7 | 1 | `fonts/charmap/cmap-unsupported-only.ttf` | valid SFNT whose sole cmap record has unsupported format 99; owns the no-active-charmap state |
| `267a07b84237` | 1.6 | 1 | `fonts/glyf/glyf-component-matrix.ttf` | 24-glyph TrueType matrix covering point and XY attachment, word arguments, uniform/independent/2x2 transforms, rounded offsets, instructions, depth 8/9, and mixed empty components |
| `fb3a52457f92` | 1.7 | 1 | `fonts/glyf/glyf-malformed-matrix.ttf` | isolated simple/composite parser failures: short records, contour/flag/delta/instruction overflow, repeat overflow, transform truncation, invalid attachment/reference, and loca beyond glyf |
| `26a737107a48` | 1.6 | 1 | `fonts/glyf/loca-short-truncated.ttf` | short-loca control with only three bytes, one byte below a complete glyph-0 offset pair |
| `2868a722bff5` | 1.6 | 1 | `fonts/glyf/loca-long-truncated.ttf` | long-loca control with only seven bytes, one byte below a complete glyph-0 offset pair |
| `697619c0847e` | 1.6 | 1 | `fonts/glyf/cvt-empty.ttf` | valid TrueType control with a present zero-length cvt table |
| `6175105e1748` | 1.6 | 1 | `fonts/glyf/cvt-odd-length.ttf` | valid TrueType control with a one-byte cvt table rejected by Rust parsing and ignored by face construction |
| `f8e4a4dc0dd7` | 1.0 | 1 | `fonts/glyf/render-coverage.ttf` | compact generated glyf fixture covering mono dropout guards, a subpixel zero-height mono profile sweep, and a quadratic scaler bbox-extrema branch probe |
| `167afcbc9243` | 4.2 | 1 | `fonts/glyf/hinter-control-matrix.ttf` | source-backed VM, render-topology, and ftsynth matrix covering state, geometry, control flow, DELTA, invalid coordinate reads, repeated post-IUP compatibility return, prep-range empty-zone SHZ and IUP, six exact bytecode error classes, conic chains, intersections, thin outlines, mixed winding, degenerate contours, empty outlines, collapsed spans, mono low-precision raster selection, scan-type dropout modes, PostScript-orientation embolden, zero-length embolden segment skipping, and nearly-opposite embolden vector zero-shift |
| `53c1301bb8e6` | 4.2 | 1 | `fonts/glyf/hinter-empty-fpgm.ttf` | derived source-backed TrueType control with empty `fpgm`, non-empty `prep`, present `cvt`, and the same glyph programs as `hinter-control-matrix.ttf`; owns native prepare-context empty-font-program coverage |
| `11cf04a95006` | 4.2 | 1 | `fonts/glyf/hinter-prep-definitions.ttf` | derived source-backed TrueType control whose prep program attempts additional FDEF then IDEF definitions beyond the font's maxp definition budgets; owns C-compatible too-many-definition error parity |
| `eab87c67c493` | 4.2 | 1 | `fonts/glyf/hinter-prep-idef.ttf` | derived source-backed TrueType control whose prep program attempts only a new IDEF beyond the font's maxp instruction-definition budget; owns the paired too-many-IDEF error route |
| `ef972cb3b8e1` | 4.2 | 1 | `fonts/glyf/hinter-prep-redefine-defs.ttf` | derived source-backed TrueType control whose prep program redefines existing FDEF 1 and IDEF 0x8F within the font's maxp budgets; owns the successful prep-definition path |
| `1d54c42ad62c` | 4.2 | 1 | `fonts/glyf/hinter-fpgm-loopcall.ttf` | derived source-backed TrueType control whose base glyph calls the existing no-op FDEF twice with LOOPCALL; owns repeated call-frame return coverage |
| `b8716f4f3396` | 4.2 | 1 | `fonts/glyf/hinter-fpgm-call-errors.ttf` | derived source-backed TrueType control with one self-recursive FDEF and four glyph programs for negative CALL, undefined CALL, undefined LOOPCALL, and recursive CALL stack overflow error parity |
| `2ecbae44bdbf` | 4.2 | 1 | `fonts/glyf/hinter-fpgm-fdef-index-overflow.ttf` | derived source-backed TrueType control whose font program attempts FDEF 256, beyond FreeType's fixed TT_DefRecord function array; owns the fpgm function-index overflow error route |
| `a330d4803c69` | 4.2 | 1 | `fonts/glyf/hinter-idef-recursive-depth.ttf` | derived source-backed TrueType control whose ADJUST IDEF recursively calls itself; owns the public IDEF call-depth guard error route |
| `45104023cf28` | 4.2 | 1 | `fonts/glyf/hinter-fpgm-nested-fdef.ttf` | derived source-backed TrueType control whose font program redefines existing FDEF 1 with a nested FDEF body; owns FreeType `Nested_DEFS` error parity |
| `dfce40861b9a` | 4.2 | 1 | `fonts/glyf/hinter-fpgm-idef-opcode-overflow.ttf` | derived source-backed TrueType control whose font program attempts IDEF opcode 0x100; owns out-of-range instruction-definition error parity |
| `391dd34d2585` | 4.2 | 1 | `fonts/glyf/hinter-fpgm-nested-idef.ttf` | derived source-backed TrueType control whose font program contains nested IDEF; owns FreeType `Nested_DEFS` error parity for instruction definitions |
| `5725da0c5c17` | 4.2 | 1 | `fonts/glyf/hinter-fpgm-unterminated-fdef.ttf` | derived source-backed TrueType control whose font program starts FDEF without ENDF; owns unterminated-FDEF scanner error parity |
| `a60b05251f89` | 4.2 | 1 | `fonts/glyf/hinter-fpgm-unterminated-idef.ttf` | derived source-backed TrueType control whose font program starts IDEF without ENDF; owns unterminated-IDEF scanner error parity |

### Legacy Alias Concentration

These directories account for the historical alias-heavy fixture areas and
reveal where later cleanup has the highest storage and reasoning value. The
current active public-input symlink count is 43; this table is cleanup context,
not the current execution count.

| Alias area | Paths | Primary content role |
|---|---:|---|
| `input/fonts/name-cmap` | 164 | aliases into the broad SFNT name source |
| `input/fonts/names` | 28 | name-table variants and aliases |
| `input/fonts/os2-ranges` | 27 | aliases into the broad OS/2 range source |
| `input/fonts/os2` | 24 | OS/2 metadata aliases |
| `input/fonts/sfnt` | 17 | SFNT table aliases |
| `generated/truetype` | 16 | generated TrueType controls |
| `input/fonts/generated/fstype` | 7 | embedding-bit variants |
| all other active areas | 96 | focused format/property controls |

## Actual Custom Glyph Selection

Only the glyphs below are selected by current explicit public inputs. Sizes are
listed because they enter different hinting and scaling conditions.

| Font | Selected codepoints/glyphs | Sizes | Distinct obligation |
|---|---|---|---|
| `basic-latin.ttf` | U+0041 gid 14 (`A`) | 10, 20, 32 | Latin autohint size behavior and script-map control |
| `basic-latin.ttf` | U+0393 and U+4E00, both aliases to gid 14 (`A`) | 20 | cmap/script assignment controls only; no distinct Greek/CJK geometry |
| `latin-greek-cyrillic.ttf` | U+0041 gid 14 (`A`) | 10 | Latin control |
| `latin-greek-cyrillic.ttf` | U+0393 gid 32 (`Gamma`), U+03B1 gid 33 (`alpha`) | 20, 32 | distinct Greek line and curved geometry |
| `latin-greek-cyrillic.ttf` | U+0411 gid 35 (`uni0411`), U+043E gid 36 composite of `o` | 20, 32 | distinct Cyrillic and component behavior |
| `indic-coverage.ttf` | U+0915 gid 4, U+0920 gid 5, U+0930 gid 6 | 10, 20, 32 | Devanagari base/headline geometry |
| `indic-coverage.ttf` | U+093E gid 7, U+094D gid 8 | 20, 10 | spacing mark and zero-advance combining mark |
| `indic-coverage.ttf` | U+0041 alias to gid 4 | 20 | cross-script assignment control, not distinct Latin geometry |
| `cjk-coverage.ttf` | U+3007 alias to gid 4 (`A`) | 10 | script assignment control only |
| `cjk-coverage.ttf` | U+4E00 gids 5, U+4E09 gid 6 | 20, 32 | one-stroke and three-stroke geometry |
| `cjk-coverage.ttf` | U+53E3 gid 7, U+65E5 gid 8 | 10, 20 | enclosed two/three-contour geometry |
| `cjk-coverage.ttf` | U+6C38 gid 9, U+7530 gid 10 | 32, 20 | diagonal/branching and five-contour geometry |
| `cjk-coverage.ttf` | U+4E2A/U+4E3B and U+4ED6/U+519B aliases | calibration only | distinct existing CJK outlines supply bottom/top fill and flat Hani blue-string candidates without new glyphs |
| `cjk-coverage.ttf` | U+0041 and U+0915 aliases to gid 4 | 20 | cross-script assignment controls |
| `cjk-coverage.ttf` | U+00C3 gid 11, U+1E4C gid 12 | 20 | top and second-top tilde separation, both quadratic measurement directions, stretch/no-stretch thresholds, alignment, and contour movement |
| `cjk-coverage.ttf` | U+1E1A gid 13 | 20 | one bottom contour packs both quadratic measurement directions plus stretch, alignment, and contour movement |
| `cjk-coverage.ttf` | U+00D8 gid 14 | 20 | capital top/bottom blue-edge suppression |
| `cjk-coverage.ttf` | U+0056 gid 15 | 20 | one strong corner and two weak controls exercise single-reference IUP shifting |
| `cjk-coverage.ttf` | U+004D gid 16 | 20 | vertical reversals exercise shared-start segment retention and replacement |
| `cjk-coverage.ttf` | U+004F gid 17, U+006F gid 18 | 20 | distinct capital/lowercase round extrema paired with flat H/n calibration geometry |
| `cjk-coverage.ttf` | U+0049 gid 19 | 20 | compact four-edge micro-serif with close cross-links and intermediate-edge overlap rejection |
| `cjk-width-order.ttf` | U+56D7 gid 1 | 20 | Hani fallback standard character without U+7530; wide-then-narrow stems exercise CJK standard-width insertion sort and quantization |
| `digit-notdef-cmap.ttf` | U+006F gid 2, with U+0030 cmap-covered as gid 0 | 20 | public force-autohint metrics setup scans an explicitly covered digit that resolves to glyph 0, exercising the face-global digit-width skip branch without selecting `.notdef` as the rendered glyph |
| `latin-malformed-standard.ttf` | U+0041 gid 2, with U+006F gid 3 truncated to a two-byte glyf record | 20 | public force-autohint load keeps the selected Latin glyph valid while standard-width initialization tries and ignores the malformed `o` standard glyph, matching pinned FreeType's metrics fallback behavior |
| `script-coverage.ttf` | all generated `SCRIPT_PROBES` codepoints from `scripts/build_autohint_script_fixtures.py` | 20 | explicit `FT_LOAD_FORCE_AUTOHINT` script-selection rows for all 59 compact script glyphs; this proves each selected script tag through real Rust/C ABI/WASM parity without an implicit script matrix |
| `hdmx_observable.ttf` | U+0041 gid 36 (`A`) | 20 | default, compute-metrics, mono hdmx, and mono suppression conditions |
| `hhea-zero-typo-fallback.ttf` | face open and active size metrics only | 20 | hhea ascent/descent/lineGap are zero and OS/2 `USE_TYPO_METRICS` is clear, so public `FT_Size_Metrics` selects the OS/2 typo fallback branch |
| `hhea-zero-win-fallback.ttf` | face open and active size metrics only | 20 | hhea and OS/2 typo metrics are zero, so public `FT_Size_Metrics` selects the OS/2 Windows ascent/descent fallback branch |
| `post-format-1.ttf` | gid 1 | name lookup only | `post` format 1.0 with non-258 glyph count returns FreeType's default `.notdef` instead of Mac standard names |
| `post-format-25.ttf` | gid 36 (`A`), gid 1 | name lookup only | format 2.5 signed-delta rows cover valid Mac-name lookup and invalid negative deltas mapping to `.notdef` / glyph index 0 |
| `post-format-unsupported.ttf` | gid 1 | name lookup only | unsupported non-3.0 format proves the public `FT_HAS_GLYPH_NAMES` / cleared-buffer `Invalid_Argument` path |
| `post-format-20-short.ttf`, `post-format-20-zero.ttf` | gid 1 | name lookup only | malformed format 2.0 headers default to `.notdef` with exact public status and buffer parity |
| `post-format-20-custom-truncated.ttf` | gid 1 | name lookup only | format 2.0 custom name index without string bytes returns `.notdef` like the C oracle |
| `post-format-25-short.ttf`, `post-format-25-zero.ttf`, `post-format-25-too-many.ttf` | gid 1 | name lookup only | malformed format 2.5 headers and above-limit counts default to `.notdef` with exact public status and buffer parity |
| `name-selection-fallbacks.ttf` | face open and name lookup only | name table only | unsupported and malformed family-name candidates fall through to Unicode/Mac Roman fallbacks while `FT_Get_Postscript_Name` still returns exact Windows bytes |
| `name-missing.ttf` | face open only | name table absent | public `FT_New_Memory_Face` opens a valid SFNT without an optional `name` table, driving the constructor fallback separate from the zero-record `name-empty.ttf` control |
| `name-apple-postscript.ttf` | face open and name lookup only | name table only | Apple Roman nameID 6 is selected by `FT_Get_Postscript_Name` when no Windows PostScript name exists |
| `name-win-postscript-odd-apple.ttf` | face open and name lookup only | name table only | odd-length Windows nameID 6 is rejected and exact C/Rust/C ABI/WASM parity falls back to Apple Roman |
| `variable-name-apple-prefix.ttf` | encoded named instance 1 | name lookup only | Apple-only variation prefix/subfamily records build the encoded named-instance PostScript name through public `FT_Get_Postscript_Name` |
| `variable-name-unicode-prefix.ttf` | encoded named instance 1 | name lookup only | Unicode-only variation prefix is ignored by FreeType's variation PostScript prefix path, while the Unicode subfamily is accepted through the general name lookup path |
| `variable-name-odd-win-prefix.ttf` | encoded named instance 1 | name lookup only | odd-length Windows variation prefix is rejected before Apple Roman fallback constructs the encoded named-instance PostScript name |
| `variable-name-missing-subfamily.ttf` | encoded named instances 1-8 | name lookup and fvar coordinates | unsupported or missing subfamily records force FreeType's normal variation-instance PostScript synthesis for positive, zero, negative, fractional, negative-fractional, and compact rounding-sensitive 16.16 coordinates plus sanitized axis tags |
| `input/fonts/DejaVuSans.ttf` | gids 3, 36, 57 | 28 | outline ftsynth weight-adjustment rows covering empty-outline no-strength behavior, horizontal metric/advance side effects, and vertical-layout advance.y side effects |
| `glyf-component-matrix.ttf` | gids 3-10 | 19, 20 | point attachment, word XY arguments, all component transforms, rounded/unrounded offsets, use-my-metrics, and composite instructions |
| `glyf-component-matrix.ttf` | gids 18, 19, 23 | 20 | accepted depth-8 boundary, rejected depth-9 recursion, and non-empty composite with an empty child |
| `glyf-malformed-matrix.ttf` | gids 1-19 | 20 | one explicitly selected malformed record per simple/composite parser boundary and table/reference error |
| `loca-short-truncated.ttf`, `loca-long-truncated.ttf` | gid 0 | 20 | one checked truncation failure for each loca record format |
| `cvt-empty.ttf`, `cvt-odd-length.ttf` | gid 1 | 20 | present-empty and odd-length CVT parser outcomes isolated with no-scale loading |
| `hinter-control-matrix.ttf` | gid 1, gids 24-40 | 20 | valid VM family matrices plus empty-stack ROLL no-op, repeated post-IUP compatibility return, divide-zero, truncated pushes, glyph IDEF, unterminated FDEF, undefined-opcode errors, mono scan-type 0/2 dropout controls, and gid 1 positive-area ftsynth PostScript-orientation embolden coverage |
| `hinter-empty-fpgm.ttf` | gid 1 | 20 | native TrueType prepare path with empty font program, non-empty prep program, and present CVT |
| `hinter-prep-definitions.ttf`, `hinter-prep-idef.ttf` | gid 1 | 20 | prep-range FDEF/IDEF definitions allowed by FreeType syntax but rejected by this font's maxp definition budgets, matching pinned C error behavior |
| `hinter-prep-redefine-defs.ttf` | gid 1 | 20 | prep-range redefinition of existing FDEF 1 and IDEF 0x8F within maxp budgets, proving FreeType-compatible successful prep definitions |
| `hinter-fpgm-loopcall.ttf` | gid 1 | 20 | no-output repeated LOOPCALL of the existing function definition, covering call-frame repeat and ENDF return without changing points |
| `hinter-fpgm-call-errors.ttf` | gids 0, 1, 2, 24 | 20 | compact CALL/LOOPCALL public error matrix: negative function reference, undefined CALL, undefined LOOPCALL, and recursive CALL stack overflow |
| `hinter-fpgm-fdef-index-overflow.ttf` | gid 1 | 20 | invalid fpgm FDEF 256 probes FreeType's function-definition index overflow before body scanning, exposed as an explicit public load error |
| `hinter-idef-recursive-depth.ttf` | gid 1 | 20 | recursive ADJUST IDEF calls itself until the interpreter call-depth guard rejects the glyph load, exposed as an explicit public load error |
| `hinter-fpgm-nested-fdef.ttf`, `hinter-fpgm-nested-idef.ttf`, `hinter-fpgm-idef-opcode-overflow.ttf`, `hinter-fpgm-unterminated-fdef.ttf`, `hinter-fpgm-unterminated-idef.ttf` | gid 1 | 20 | invalid fpgm scanner controls for nested definitions, out-of-range IDEF opcode, and unterminated FDEF/IDEF, each exposed as an explicit public load error |
| `hinter-control-matrix.ttf` | U+E032 gid 51 | 20 | branch-edge VM control covering zero-length line and stack vectors, invalid stack-index fallback, taken JROF, no-round dispatch, invalid contour shift, invalid coordinate reads, glyph-range empty twilight-zone SHZ, prep-range empty-zone SHZ/IUP, and positive/negative single-width MDRP cut-in |
| `hinter-control-matrix.ttf` | gid 21 | 20 | empty outline selected across normal, mono, LCD, LCD_V, and SDF render modes |
| `hinter-control-matrix.ttf` | U+E028 gid 41 | 20 | off-curve start and consecutive conic controls across normal, mono, LCD, LCD_V, and SDF modes; owns FreeType-compatible SDF conic subdivision |
| `hinter-control-matrix.ttf` | U+E029 gid 42 | 20 | self-intersecting bowtie plus a thin rectangle in mono, normal, and SDF modes |
| `hinter-control-matrix.ttf` | U+E02A gid 43 | 20 | outer and opposite-winding inner contours plus a coincident-point degenerate contour in normal, SDF, and ftsynth weight-adjustment modes; owns the public zero-length embolden segment skip |
| `hinter-control-matrix.ttf` | U+E02B gid 44 | 20 | zero-width vertical contour selected across normal, mono, SDF, and ftsynth weight-adjustment modes; owns the public glyph-slot orientation-none embolden return with metric side effects |
| `hinter-control-matrix.ttf` | U+E035 gid 54 | 20 | sharp turn whose adjacent normalized outline vectors are nearly opposite; owns the public `FT_GlyphSlot_AdjustWeight` zero-shift branch in `FT_Outline_EmboldenXY` |
| `hinter-control-matrix.ttf` | U+E02C gid 45 | 20 | zero-height horizontal contour selected across normal, mono, and SDF modes |
| `hinter-control-matrix.ttf` | U+E02D gid 46, U+E02E gid 47 | 20 | scan types 4 and 5 on narrow vertical rectangles; owns smart dropout selection with and without stub inclusion |
| `hinter-control-matrix.ttf` | U+E02F gid 48, U+E030 gid 49, U+E031 gid 50 | 20 | collapsed x/y mono contours at negative-bias fractional positions plus a 130 px mono box that selects the low-precision raster path |
| `render-coverage.ttf` | gids 1-2, gid 3, gid 4 | 16, 20 | compact mono horizontal/vertical dropout guards, a one-unit-high subpixel mono box that reaches the zero-height profile sweep, plus a single quadratic contour whose off-curve control covers scaler exact-bbox left/top extrema branches |

The custom fonts contain additional glyphs for future focused obligations:
Latin digits, round/straight/overshoot forms, combining marks, simple and
composite accents, Devanagari digits, multiple CJK contour counts, and the
remaining autohint script-tag probes in `script-coverage.ttf`. They do not
count as coverage until an explicit public input selects them.

## Deprecated Inventory

Every deprecated file is listed below. `Scripts` is abbreviated cmap
reachability: Lat, Gre, Cyr, Heb, Ara, Ind, Tha, Tib, Mya, Eth, Can, Khm, Mon,
CJK, Han, and Sup. `Properties` records only broad differentiators; exact table
and selected-glyph obligations still come from explicit inputs.

| Font | KiB | Glyphs | Scripts | Properties/current status |
|---|---:|---:|---|---|
| `DejaVuSans-Bold.ttf` | 692.3 | 6,196 | Lat,Gre,Cyr,Heb,Ara,Tha,Can,CJK,Sup | hint,kern,GPOS; replaced by compact combined style fixture |
| `DejaVuSans-BoldOblique.ttf` | 630.5 | 5,413 | Lat,Gre,Cyr,Heb,Tha,Can,CJK,Sup | hint,kern,GPOS; no public input reference |
| `DejaVuSans-ExtraLight.ttf` | 347.5 | 2,032 | Lat,Gre,Cyr,Ara,Tha | hint,kern,GPOS; no public input reference |
| `DejaVuSans-Oblique.ttf` | 622.7 | 5,355 | Lat,Gre,Cyr,Heb,Tha,Can,CJK,Sup | hint,kern,GPOS; replaced by compact combined style fixture |
| `DejaVuSans.ttf` | 741.9 | 6,253 | Lat,Gre,Cyr,Heb,Ara,Tha,Can,CJK,Sup | hint,kern,GPOS; zero public references after focused replacement |
| `DejaVuSansCondensed.ttf` | 587.4 | 5,355 | Lat,Gre,Cyr,Heb,Tha,Can,CJK,Sup | hint,kern,GPOS; no public input reference |
| `DejaVuSansMono.ttf` | 335.1 | 3,377 | Lat,Gre,Cyr,Ara,Tha,Sup | fixed pitch; replaced by active focused fixture |
| `DejaVuSerif-Bold.ttf` | 348.3 | 3,506 | Lat,Gre,Cyr,Tha,Sup | hint,kern,GPOS; no public input reference |
| `DejaVuSerif-BoldItalic.ttf` | 339.9 | 3,506 | Lat,Gre,Cyr,Tha,Sup | hint,kern,GPOS; no public input reference |
| `DejaVuSerif-Italic.ttf` | 338.4 | 3,507 | Lat,Gre,Cyr,Tha,Sup | hint,kern,GPOS; no public input reference |
| `DejaVuSerif.ttf` | 371.7 | 3,528 | Lat,Gre,Cyr,Tha,Sup | hint,kern,GPOS; no public input reference |
| `DroidSansFallbackFull.ttf` | 3,938.9 | 49,382 | Lat,Tha,CJK,Han,Sup | vertical metrics; perf-only legacy reference |
| `LiberationSans-Bold.ttf` | 404.9 | 2,620 | Lat,Gre,Cyr,Heb | hint,kern,GPOS; no public input reference |
| `LiberationSans-Regular.ttf` | 401.2 | 2,620 | Lat,Gre,Cyr,Heb | hint,kern,GPOS; no public input reference |
| `LiberationSansNarrow-Bold.ttf` | 107.3 | 681 | Lat,Gre,Cyr | hint,kern,GPOS; no public input reference |
| `LiberationSerif-Regular.ttf` | 384.5 | 2,602 | Lat,Gre,Cyr,Heb | exact duplicate of `LiberationSerif.ttf` |
| `LiberationSerif.ttf` | 384.5 | 2,602 | Lat,Gre,Cyr,Heb | exact duplicate of `LiberationSerif-Regular.ttf` |
| `NotoLoopedThai-Bold.ttf` | 68.9 | 212 | Lat,Tha,Khm | hint,GPOS; no public input reference |
| `NotoNaskhArabic-Bold.ttf` | 176.9 | 1,602 | Lat,Ara | hint,GPOS; no public input reference |
| `NotoSans-Bold.ttf` | 503.7 | 3,317 | Lat,Gre,Cyr | hint,GPOS; no public input reference |
| `NotoSans-Regular.ttf` | 500.7 | 3,317 | Lat,Gre,Cyr | exact duplicate of `NotoSans.ttf`; replaced by active compact Noto fixture |
| `NotoSans.ttf` | 500.7 | 3,317 | Lat,Gre,Cyr | exact duplicate of `NotoSans-Regular.ttf` |
| `NotoSansAdlam-Bold.ttf` | 92.2 | 361 | Lat,Ara,Sup | hint,GPOS; no public input reference |
| `NotoSansAdlamUnjoined-Bold.ttf` | 36.0 | 155 | Lat,Ara,Sup | hint,GPOS; no public input reference |
| `NotoSansAdlamUnjoined-Regular.ttf` | 36.0 | 155 | Lat,Ara,Sup | hint,GPOS; no public input reference |
| `NotoSansArabic-Bold.ttf` | 247.0 | 1,648 | Lat,Ara | hint,GPOS; no public input reference |
| `NotoSansAvestan-Regular.ttf` | 22.5 | 76 | Lat,Sup | hint; no public input reference |
| `NotoSansBamum-Bold.ttf` | 224.0 | 662 | Lat,Sup | hint; no public input reference |
| `NotoSansBamum-Regular.ttf` | 223.6 | 662 | Lat,Sup | hint; no public input reference |
| `NotoSansBengali-Bold.ttf` | 206.1 | 679 | Lat,Ind | hint,GPOS; no public input reference |
| `NotoSansBengali-Regular.ttf` | 197.2 | 679 | Lat,Ind | hint,GPOS; no public input reference |
| `NotoSansBuhid-Regular.ttf` | 11.2 | 44 | Lat | hint,GPOS; no public input reference |
| `NotoSansCanadianAboriginal-Regular.ttf` | 84.3 | 746 | Lat,Can | hint; no public input reference |
| `NotoSansCarian-Regular.ttf` | 12.5 | 54 | Lat,Sup | hint; no public input reference |
| `NotoSansChakma-Regular.ttf` | 58.2 | 212 | Lat,Ind,Mya,Sup | hint,GPOS; no public input reference |
| `NotoSansCherokee-Bold.ttf` | 106.0 | 273 | Lat | hint,GPOS; no public input reference |
| `NotoSansCherokee-Regular.ttf` | 92.4 | 273 | Lat | hint,GPOS; no public input reference |
| `NotoSansCoptic-Regular.ttf` | 44.0 | 224 | Lat,Gre,Sup | hint,GPOS; no public input reference |
| `NotoSansCypriot-Regular.ttf` | 14.7 | 60 | Lat,Sup | hint; no public input reference |
| `NotoSansDeseret-Regular.ttf` | 19.5 | 85 | Lat,Sup | hint; no public input reference |
| `NotoSansDevanagari-Bold.ttf` | 232.1 | 954 | Lat,Ind | hint,GPOS; no public input reference |
| `NotoSansDevanagari-Regular.ttf` | 224.0 | 954 | Lat,Ind | hint,GPOS; no public input reference |
| `NotoSansEthiopic-Regular.ttf` | 335.1 | 566 | Lat,Eth | hint,GPOS; no public input reference |
| `NotoSansGlagolitic-Regular.ttf` | 38.9 | 142 | Lat,Cyr,Sup | hint,GPOS; no public input reference |
| `NotoSansGothic-Regular.ttf` | 11.4 | 40 | Lat,Sup | hint,GPOS; no public input reference |
| `NotoSansGujarati-Regular.ttf` | 200.3 | 798 | Lat,Ind | hint,GPOS; no public input reference |
| `NotoSansGurmukhi-Regular.ttf` | 51.6 | 306 | Lat,Ind | hint,GPOS; no public input reference |
| `NotoSansHanifiRohingya-Regular.ttf` | 26.6 | 179 | Lat,Ara,Sup | hint,GPOS; no public input reference |
| `NotoSansKannada-Bold.ttf` | 152.7 | 481 | Lat,Ind | hint,GPOS; no public input reference |
| `NotoSansKannada-Regular.ttf` | 148.6 | 481 | Lat,Ind | hint,GPOS; no public input reference |
| `NotoSansKayahLi-Bold.ttf` | 17.3 | 60 | Lat | hint,GPOS; no public input reference |
| `NotoSansKayahLi-Regular.ttf` | 17.3 | 60 | Lat | hint,GPOS; no public input reference |
| `NotoSansKhmer-Bold.ttf` | 109.7 | 363 | Lat,Khm | hint,GPOS; no public input reference |
| `NotoSansKhmer-Regular.ttf` | 110.7 | 363 | Lat,Khm | hint,GPOS; no public input reference |
| `NotoSansMalayalam-Bold.ttf` | 113.2 | 354 | Lat,Ind | hint,GPOS; no public input reference |
| `NotoSansMedefaidrin-Bold.ttf` | 29.7 | 97 | Lat,Sup | no native hint programs; no public input reference |
| `NotoSansMedefaidrin-Regular.ttf` | 30.0 | 97 | Lat,Sup | no native hint programs; no public input reference |
| `NotoSansMongolian-Regular.ttf` | 236.6 | 1,563 | Lat,Mon,CJK,Sup | vertical metrics; replaced by compact CJK vertical fixture |
| `NotoSansMyanmar-Bold.ttf` | 205.3 | 610 | Lat,Mya | hint,GPOS; no public input reference |
| `NotoSansMyanmar-Regular.ttf` | 192.0 | 610 | Lat,Mya | hint,GPOS; no public input reference |
| `NotoSansNKo-Regular.ttf` | 37.5 | 184 | Lat,Ara | hint,GPOS; no public input reference |
| `NotoSansOlChiki-Bold.ttf` | 14.2 | 55 | Lat | hint; no public input reference |
| `NotoSansOlChiki-Regular.ttf` | 14.8 | 55 | Lat | hint; no public input reference |
| `NotoSansOldTurkic-Regular.ttf` | 14.8 | 78 | Lat,Sup | hint; no public input reference |
| `NotoSansOsage-Regular.ttf` | 20.5 | 82 | Lat,Sup | hint,GPOS; no public input reference |
| `NotoSansOsmanya-Regular.ttf` | 16.6 | 45 | Lat,Sup | hint; no public input reference |
| `NotoSansSaurashtra-Regular.ttf` | 34.5 | 96 | Lat | hint,GPOS; no public input reference |
| `NotoSansShavian-Regular.ttf` | 13.1 | 53 | Lat,Sup | hint; no public input reference |
| `NotoSansSinhala-Bold.ttf` | 331.9 | 645 | Lat,Ind,Sup | hint,GPOS; no public input reference |
| `NotoSansSundanese-Bold.ttf` | 22.4 | 89 | Lat | hint,GPOS; no public input reference |
| `NotoSansSundanese-Regular.ttf` | 22.4 | 89 | Lat | hint,GPOS; no public input reference |
| `NotoSansTaiViet-Regular.ttf` | 30.3 | 83 | Lat | hint,GPOS; no public input reference |
| `NotoSansTamil-Bold.ttf` | 74.0 | 244 | Lat,Ind,Sup | hint,GPOS; no public input reference |
| `NotoSansTamil-Regular.ttf` | 70.9 | 244 | Lat,Ind,Sup | hint,GPOS; no public input reference |
| `NotoSansTelugu-Regular.ttf` | 190.2 | 791 | Lat,Ind | hint,GPOS; no public input reference |
| `NotoSansThai-Regular.ttf` | 36.9 | 140 | Lat,Tha | hint,GPOS; no public input reference |
| `NotoSansTifinaghAPT-Regular.ttf` | 38.6 | 167 | Lat | hint,GPOS; no public input reference |
| `NotoSansTifinaghAir-Regular.ttf` | 38.6 | 168 | Lat | hint,GPOS; no public input reference |
| `NotoSansVai-Regular.ttf` | 89.5 | 305 | Lat | hint; no public input reference |
| `NotoSerifBengali-Bold.ttf` | 252.1 | 640 | Lat,Ind | hint,GPOS; no public input reference |
| `NotoSerifDevanagari-Regular.ttf` | 257.7 | 871 | Lat,Ind | hint,GPOS; no public input reference |
| `NotoSerifEthiopic-Bold.ttf` | 276.4 | 566 | Lat,Eth | hint,GPOS; no public input reference |
| `NotoSerifEthiopic-Regular.ttf` | 282.8 | 566 | Lat,Eth | hint,GPOS; no public input reference |
| `NotoSerifGujarati-Bold.ttf` | 144.2 | 456 | Lat,Ind | hint,GPOS; no public input reference |
| `NotoSerifGujarati-Regular.ttf` | 141.5 | 456 | Lat,Ind | hint,GPOS; no public input reference |
| `NotoSerifGurmukhi-Bold.ttf` | 52.4 | 294 | Lat,Ind | hint,GPOS; no public input reference |
| `NotoSerifGurmukhi-Regular.ttf` | 51.4 | 294 | Lat,Ind | hint,GPOS; no public input reference |
| `NotoSerifKannada-Bold.ttf` | 209.8 | 417 | Lat,Ind | hint,GPOS; no public input reference |
| `NotoSerifKhmer-Bold.ttf` | 145.5 | 361 | Lat,Khm | hint,GPOS; no public input reference |
| `NotoSerifLao-Regular.ttf` | 37.6 | 117 | Lat,Tha | hint,GPOS; no public input reference |
| `NotoSerifMalayalam-Bold.ttf` | 108.6 | 354 | Lat,Ind | hint,GPOS; no public input reference |
| `NotoSerifMalayalam-Regular.ttf` | 107.4 | 354 | Lat,Ind | hint,GPOS; no public input reference |
| `NotoSerifMyanmar-Bold.ttf` | 265.0 | 725 | Lat,Mya | hint,GPOS; no public input reference |
| `NotoSerifSinhala-Bold.ttf` | 326.7 | 645 | Lat,Ind,Sup | hint,GPOS; no public input reference |
| `NotoSerifSinhala-Regular.ttf` | 315.3 | 645 | Lat,Ind,Sup | hint,GPOS; no public input reference |
| `NotoSerifTamil-Regular.ttf` | 77.8 | 222 | Lat,Ind,Sup | hint,GPOS; no public input reference |
| `NotoSerifTelugu-Bold.ttf` | 320.0 | 728 | Lat,Ind | hint,GPOS; no public input reference |
| `NotoSerifTelugu-Regular.ttf` | 300.9 | 728 | Lat,Ind | hint,GPOS; no public input reference |
| `NotoSerifThai-Bold.ttf` | 45.9 | 140 | Lat,Tha | hint,GPOS; no public input reference |
| `Ubuntu.ttf` | 1,047.8 | 1,843 | Lat,Gre,Cyr | variable fvar/avar; replaced by 9 KiB focused variable subset |

## Value Findings

1. No public input JSON references the deprecated corpus. The 100 files remain
   isolated only for legacy diagnostics and a separately approved cleanup.
2. Two deprecated pairs are byte-identical: Liberation Serif regular aliases
   and Noto Sans regular aliases.
3. `DroidSansFallbackFull.ttf` is the largest file and is referenced only by the
   performance matrix, not the authoritative public parity inputs.
4. Most deprecated script fonts differ in script and style but currently own no
   explicit glyph, branch, region, or condition obligation.
5. Active aliases hide additional concentration: 211 paths share one 771 KiB
   name font, and 52 paths share one 743 KiB OS/2 range font.
6. The compact CJK and Indic fonts provide real script geometry. Script aliases
   in `basic-latin.ttf` provide assignment controls but must not be counted as
   geometric script coverage.
7. `fixed-width.ttf` is independent and 17 KiB after replacing the 335 KiB
   deprecated DejaVuSansMono dependency.
8. Current custom inputs exercise only one hdmx glyph. Additional hdmx glyphs
   are not useful until coverage identifies a width-record condition requiring
   them.
9. One 17 KiB bold-italic fixture replaces separate 692 KiB bold and 623 KiB
   oblique dependencies while entering both `head.macStyle` conditions.
10. The existing 1.9 KiB CJK font replaces the 237 KiB Mongolian vertical
    dependency; a 1.8 KiB vhea-only control enters the missing vmtx condition.
11. The active 9.7 KiB compact Noto font replaces both references to the 501 KiB
    deprecated Noto font without adding a case or content identity.
12. A 9.1 KiB, 20-glyph Ubuntu subset preserves two variation axes and all 12
    named instances, replacing the 1.0 MiB source and two false variable aliases.
13. Six generated fvar derivatives cover truncated headers, unsupported
    versions, too-short axis records, declared instance arrays beyond table EOF,
    too-short instance records, and explicit instance PostScript name IDs. The
    remaining uncovered fvar lines are the defensive instance-count overflow
    closure, which is unreachable with the table's 16-bit count and size fields.
14. A 143 KiB retain-GID DejaVu fixture replaces the final 760 KiB dependency.
    Its selected outlines cover Latin and Greek blue strings, Cyrillic, emoji,
    touch tags, forced autohinting, face metadata, and native hint programs. Its
    format-4 cmap deliberately maps U+0022 through the retained number-sign
    glyph so the first segment uses `idRangeOffset`; an explicit Apple Unicode
    2.0 U+0041 variant owns that decoder obligation.
15. Four 17 KiB maxp derivatives cover the version 0.5, version 2.0,
    stream-short version 1.0, and physically short header behaviors. Pinned
    FreeType reads maxp extras from the underlying stream and ignores maxp load
    errors during face construction; the Rust loader now matches both details,
    and `tt/maxp.rs` has 100% structural coverage.
16. Five 17 KiB kern derivatives cover all optional-table exits and one valid
    pair without adding glyphs. Rust now ignores the top-level kern version,
    caps subtable count, clamps lengths, and requires FreeType's exact coverage
    bits; `tt/kern.rs` has 100% structural coverage.
17. Six 17 KiB hdmx derivatives cover short headers, high-word repair, both
    invalid record-count operands, size mismatch, and record truncation. Two
    explicit mono loads distinguish ppem conversion failure from a valid ppem
    record miss; `tt/hdmx.rs` has 100% structural coverage.
18. Three paired 2 KiB metric derivatives cover zero counts, above-glyph
    counts, and present-but-short hmtx/vmtx data. A glyph-0 vertical load covers
    the in-range long vertical metric; both `tt/hmtx.rs` and `tt/vmtx.rs` have
    100% structural coverage.
19. Two physical-EOF 2 KiB fixtures cover short hhea and vhea stream errors.
    Rust now propagates a present malformed vhea like FreeType while retaining
    an empty vmtx for a present but unreadable metrics table; `tt/hhea.rs` and
    `tt/vhea.rs` have 100% structural coverage.
20. Twelve metadata controls cover a physically short required head, optional
    short OS/2 and post tables, OS/2 `USE_TYPO_METRICS`, valid `post` format
    1.0/2.5 glyph-name behavior, and malformed format 2.0/2.5 public fallback
    behavior. Two additional 4.1 KiB metric controls generated by
    `make font-fixture-metrics` prove the hhea-zero OS/2 typo and OS/2 Windows
    fallback branches through public `FT_Size_Metrics`. FreeType selects
    typographic metrics first when the use-typo bit is set, otherwise hhea when
    nonzero, then OS/2 typo and Windows fallbacks; Rust now uses that order
    consistently for face and size metrics. `tt/head.rs` and `tt/os2.rs` have
    100% structural coverage, while `tt/post.rs` currently has full function
    coverage and 95/98 lines with the remaining direct resolver guards
    classified in the coverage plan.
21. Four 1.5–1.6 KiB name controls cover both malformed-table exits, raw-record
    filtering, Windows UTF-16 failures, Mac Roman and arbitrary-Windows
    fallbacks, empty defaults, and each operand outcome in the preference
    predicates. `tt/name.rs` has 100% function, line, region, and branch
    coverage.
22. One 13.6 KiB CJK/Indic collection, three 8–16 byte malformed TTC headers,
    and one 12-byte OTTO header cover collection iteration, second-face
    selection, all TTC header/offset errors, and both accepted SFNT magics.
    TTC table offsets are absolute from the collection start; fixing Rust's
    previous double addition restored second-face parity. `tt/mod.rs` has 100%
    structural coverage.
23. Nine 1.7-3.9 KiB cmap controls cover format 4, 6, 12, and malformed
    format 14 lookup, iteration, and parser boundaries, unsupported-only faces,
    and range-offset validation. They exposed two real differences: format 6
    wraps at `0xFFFFFFFF` while formats 4/12 stop, and format 12 advances past
    a zero start glyph within the same group. `tt/cmap.rs` now has 100%
    branch coverage and 426/429 lines; the remaining active format-14
    lookup/iteration arms are guarded by `FT_Set_Charmap` rejecting format 14.

24. Two 1.6-1.7 KiB glyf matrices cover every valid composite transform,
    attachment mode, empty/depth boundary, and isolated malformed parser exit.
    They exposed three C/Rust divergences: scaled point attachment was omitted,
    excessive flag repeats were truncated, and invalid attachment indices used
    a zero offset. Rust now matches `TT_Process_Composite_Component` and
    `TT_Load_Simple_Glyph`; `tt/glyf.rs` has 100% function, line, region, and
    branch coverage.

25. Two 1.6 KiB loca controls cover the short and long record truncation exits.
    Reading each complete record through one checked slice removes nine
    byte-position-specific regions and avoids ten additional near-duplicate
    fonts; `tt/loca.rs` has 100% function, line, region, and branch coverage.

26. Two 1.6 KiB CVT controls cover present-empty and odd-length table
    outcomes without executing hint programs. Font construction now routes
    present `fpgm` and `prep` byte streams through the restored byte-copy
    helpers, and the compact TT program font covers those helpers through real
    `FT_Load_Glyph` execution; `tt/hinter/tables.rs` has 100% function, line,
    region, and branch coverage.

27. One 3.5 KiB hinter control font combines compact IDEF and FDEF bodies,
    present-empty `prep`, a valid CVT, nineteen focused program glyphs, five
    render-topology glyphs, and one reusable empty-outline glyph.
    Eighteen explicit loads cover scan types 0/2, IDEF dispatch, UTP, the SROUND and
    S45ROUND selector/sign matrix, INSTCTRL validation, FDEF/CALL/LOOPCALL and
    conditional flow, plus consolidated VM stack/state, point geometry,
    interpolation, DELTA, and six malformed-program error families without
    multiplying sizes or flags. Twenty-four render variants additionally select
    conic chains, intersections, thin geometry, winding reversal, a degenerate
    contour, empty outlines, collapsed horizontal/vertical spans, and mono
    dropout scan modes 0/2/4/5 across normal, mono, LCD, LCD_V, and SDF modes. Its
    inspectable source is `tests/fixtures/font-sources/hinter-control-matrix.ttx`;
    rebuild it with `make font-fixture-hinter`.

28. Seven compact 3.8 KiB gasp controls replace the previous
    `fonts/gasp/*.ttf` symlink aliases to `DejaVuSans.ttf`. They are generated
    from the source-backed hinter matrix by `scripts/build_gasp_fixtures.py`
    and rebuilt with `make font-fixture-gasp`. The set covers version 1
    multi-range selection and after-last sentinel behavior, no `gasp` table,
    version 0 high-bit masking, and unsupported version 2 optional-table
    failure while keeping the SFNT face loadable. It also includes a directory
    record whose length is shorter than the readable physical `gasp` bytes,
    matching FreeType's stream-read behavior, plus physical EOF controls for a
    short header and truncated range array.

29. One compact 3.9 KiB cmap matrix covers the `FT_Get_CMap_Format` and
    `FT_Get_CMap_Language_ID` public helpers without relying on broad fonts.
    It is generated from the source-backed hinter matrix by
    `scripts/build_cmap_fixtures.py` and rebuilt with
    `make font-fixture-cmap`. The matrix exposes format 4, 6, 12, and 14 rows,
    including nonzero language IDs and the format 14 `0xFFFFFFFF` sentinel.

30. One compact 5.5 KiB autohint script fixture provides one glyph per script
    tag plus standard-character cmap aliases. It is generated by
    `scripts/build_autohint_script_fixtures.py` and rebuilt with
    `make font-fixture-autohint-scripts`. Current public inputs select all 59
    generated script glyphs through explicit `FT_LOAD_FORCE_AUTOHINT` variants;
    this is a deliberate script obligation set, not an implicit script matrix.

31. Two compact hhea-zero metric fixtures are generated from the source-backed
    hinter matrix by `scripts/build_metric_fixtures.py` and rebuilt with
    `make font-fixture-metrics`. They add two explicit `FT_Size_Metrics`
    variants and cover the remaining face-metric fallback order without adding
    glyph rows or multiplying unrelated size/flag combinations.

32. Eight compact SFNT table fixtures are generated from the source-backed
    hinter matrix by `scripts/build_sfnt_fixtures.py` and rebuilt with
    `make font-fixture-sfnt`. The set covers standard HEAD/MAXP/HHEA/POST
    table access, raw table loading and directory-info probes, PCLT present
    and zero-version nullness, short optional PCLT parsing, VHEA/VMTX vertical
    table presence, and no-PCLT/no-VHEA/no-OS/2 optional-table controls without
    introducing a broad font dependency.

## Replacement Queue

| Order | Deprecated dependency | Minimal replacement property |
|---:|---|---|
| 1 | `DejaVuSansMono.ttf` | complete: compact fixed-pitch metadata and uniform advances |
| 2 | `DejaVuSans-Bold.ttf` | complete: combined compact bold-italic fixture |
| 3 | `DejaVuSans-Oblique.ttf` | complete: combined compact bold-italic fixture |
| 4 | `NotoSansMongolian-Regular.ttf` | complete: compact CJK vhea/vmtx plus vhea-only control |
| 5 | `Ubuntu.ttf` | complete: 9 KiB subset preserves axes, instances, and variation tables |
| 6 | `NotoSans-Regular.ttf` | complete: both references use the active compact Noto fixture |
| 7 | `DejaVuSans.ttf` | complete: 143 KiB retain-GID focused fixture plus one explicit format-4 lookup variant |

Update this file after every font mutation. A new hash, selected glyph, table,
alias, or public obligation must be reflected here before the batch is accepted.
