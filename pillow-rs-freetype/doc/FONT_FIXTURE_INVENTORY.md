# Font Fixture Inventory

Status: active inventory
Recorded: 2026-07-11
Coverage plan: `doc/FONT_FIXTURE_COVERAGE_PLAN.md`

This inventory separates file paths, stored binaries, unique contents, cmap
reachability, distinct glyph geometry, and actual public-input selection. A font
name or Unicode mapping does not count as useful variability unless an explicit
input selects a glyph whose geometry or font tables enter a distinct behavior.

## Corpus Totals

| Corpus | Paths | Stored files | Symlinks | Unique SHA-256 contents | Stored size |
|---|---:|---:|---:|---:|---:|
| Active fixtures | 125 | 82 | 43 | 90 | 686 KiB |
| Deprecated corpus | 101 | 101 | 0 | 99 | 23 MiB |
| Compact active autohint set | 5 | 5 | 0 | 5 | 187 KiB |

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
| `f6fa317b2fbb` | 2.4 | 1 | `fonts/autohint/cjk-coverage.ttf` | source-backed glyf, 20 glyphs, 32 mappings, vertical metrics, CJK geometry plus Latin adjustment, blue-zone, and serif topologies |
| `38431987e24e` | 11.7 | 1 | `fonts/autohint/indic-coverage.ttf` | glyf, 37 glyphs, 29 mappings, native hinting, Devanagari geometry |
| `b4ff2e5f559c` | 10.7 | 1 | `fonts/autohint/latin-greek-cyrillic.ttf` | glyf, 39 glyphs, distinct Latin/Greek/Cyrillic geometry |
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
| `3380d1c030f9` | 8.8 | 1 | `fonts/variable/fvar-instance-array-short.ttf` | compact generated malformed control with a declared fvar instance array beyond table EOF |
| `cc6cc4e2f726` | 8.9 | 1 | `fonts/variable/fvar-instance-size-short.ttf` | compact generated malformed control with an instance record one byte below the two-axis minimum |
| `487a56138ec6` | 8.9 | 1 | `fonts/variable/fvar-instance-postscript-name.ttf` | compact generated variable control with explicit fvar instance PostScript name IDs |
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
| `298447f992df` | 1.5 | 1 | `fonts/names/name-short.ttf` | five-byte name table; owns the short-header face-open error |
| `fad5b6b1057c` | 1.5 | 1 | `fonts/names/name-record-overflow.ttf` | six-byte name header declaring one absent record; owns the record-array overflow error |
| `0a2206bc87ca` | 13.6 | 1 | `fonts/control/multiface-cjk-indic.ttc` | two-face collection combining the compact CJK and Indic fonts; owns TTC offset iteration and second-face selection |
| `a123d521381f` | 0.01 | 1 | `fonts/control/ttc-short.ttc` | eight-byte TTC short-header control |
| `0b66ccbde246` | 0.01 | 1 | `fonts/control/ttc-offset-overflow.ttc` | 12-byte TTC declaring one absent face offset |
| `1cca599d017f` | 0.02 | 1 | `fonts/control/ttc-face-offset-out-of-range.ttc` | complete one-face TTC header whose selected face offset is outside the stream |
| `debb925b92a2` | 0.01 | 1 | `fonts/control/otto-empty.otf` | 12-byte zero-table OTTO header; owns the OpenType sfVersion predicate outcome before required-table rejection |
| `bf004c57a16e` | 1.7 | 1 | `fonts/charmap/format6-range.ttf` | format-6-only trimmed range with mapped and zero glyph entries; owns direct lookup, gap, exhaustion, and wrapping iteration |
| `c1189f1a3962` | 3.9 | 1 | `fonts/cmap/cmap-format-language-matrix.ttf` | source-backed compact cmap matrix with format 4 language `0x0409`, format 6 language 17, format 12 language `0x12345678`, and format 14 variation-selector metadata |
| `251fcf468057` | 2.1 | 1 | `fonts/charmap/cmap-parser-matrix.ttf` | one valid format 6 plus unsupported, out-of-range, and independently malformed format 4/6/12 records |
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
| `f70053cf222f` | 3.8 | 1 | `fonts/glyf/hinter-control-matrix.ttf` | source-backed VM and render-topology matrix covering state, geometry, control flow, DELTA, invalid coordinate reads, six exact bytecode error classes, conic chains, intersections, thin outlines, mixed winding, degenerate contours, empty outlines, collapsed spans, mono low-precision raster selection, and scan-type dropout modes |

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
| `hdmx_observable.ttf` | U+0041 gid 36 (`A`) | 20 | default, compute-metrics, mono hdmx, and mono suppression conditions |
| `post-format-1.ttf` | gid 1 | name lookup only | `post` format 1.0 with non-258 glyph count returns FreeType's default `.notdef` instead of Mac standard names |
| `post-format-25.ttf` | gid 36 (`A`), gid 1 | name lookup only | format 2.5 signed-delta rows cover valid Mac-name lookup and invalid negative deltas mapping to `.notdef` / glyph index 0 |
| `post-format-unsupported.ttf` | gid 1 | name lookup only | unsupported non-3.0 format proves the public `FT_HAS_GLYPH_NAMES` / cleared-buffer `Invalid_Argument` path |
| `post-format-20-short.ttf`, `post-format-20-zero.ttf` | gid 1 | name lookup only | malformed format 2.0 headers default to `.notdef` with exact public status and buffer parity |
| `post-format-20-custom-truncated.ttf` | gid 1 | name lookup only | format 2.0 custom name index without string bytes returns `.notdef` like the C oracle |
| `post-format-25-short.ttf`, `post-format-25-zero.ttf`, `post-format-25-too-many.ttf` | gid 1 | name lookup only | malformed format 2.5 headers and above-limit counts default to `.notdef` with exact public status and buffer parity |
| `glyf-component-matrix.ttf` | gids 3-10 | 19, 20 | point attachment, word XY arguments, all component transforms, rounded/unrounded offsets, use-my-metrics, and composite instructions |
| `glyf-component-matrix.ttf` | gids 18, 19, 23 | 20 | accepted depth-8 boundary, rejected depth-9 recursion, and non-empty composite with an empty child |
| `glyf-malformed-matrix.ttf` | gids 1-19 | 20 | one explicitly selected malformed record per simple/composite parser boundary and table/reference error |
| `loca-short-truncated.ttf`, `loca-long-truncated.ttf` | gid 0 | 20 | one checked truncation failure for each loca record format |
| `cvt-empty.ttf`, `cvt-odd-length.ttf` | gid 1 | 20 | present-empty and odd-length CVT parser outcomes isolated with no-scale loading |
| `hinter-control-matrix.ttf` | gid 1, gids 24-40 | 20 | valid VM family matrices plus divide-zero, truncated pushes, glyph IDEF, unterminated FDEF, undefined-opcode errors, and mono scan-type 0/2 dropout controls |
| `hinter-control-matrix.ttf` | U+E032 gid 51 | 20 | branch-edge VM control covering zero-length vectors, invalid stack-index fallback, taken JROF, no-round dispatch, invalid contour shift, invalid coordinate reads, and empty twilight-zone SHZ |
| `hinter-control-matrix.ttf` | gid 21 | 20 | empty outline selected across normal, mono, LCD, LCD_V, and SDF render modes |
| `hinter-control-matrix.ttf` | U+E028 gid 41 | 20 | off-curve start and consecutive conic controls across normal, mono, LCD, LCD_V, and SDF modes; owns FreeType-compatible SDF conic subdivision |
| `hinter-control-matrix.ttf` | U+E029 gid 42 | 20 | self-intersecting bowtie plus a thin rectangle in mono and normal modes |
| `hinter-control-matrix.ttf` | U+E02A gid 43 | 20 | outer and opposite-winding inner contours plus a coincident-point degenerate contour in normal and SDF modes |
| `hinter-control-matrix.ttf` | U+E02B gid 44 | 20 | zero-width vertical contour selected across normal, mono, and SDF modes |
| `hinter-control-matrix.ttf` | U+E02C gid 45 | 20 | zero-height horizontal contour selected across normal, mono, and SDF modes |
| `hinter-control-matrix.ttf` | U+E02D gid 46, U+E02E gid 47 | 20 | scan types 4 and 5 on narrow vertical rectangles; owns smart dropout selection with and without stub inclusion |
| `hinter-control-matrix.ttf` | U+E02F gid 48, U+E030 gid 49, U+E031 gid 50 | 20 | collapsed x/y mono contours at negative-bias fractional positions plus a 130 px mono box that selects the low-precision raster path |

The custom fonts contain additional glyphs for future focused obligations:
Latin digits, round/straight/overshoot forms, combining marks, simple and
composite accents, Devanagari digits, and multiple CJK contour counts. They do
not count as coverage until an explicit public input selects them.

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
13. Five generated fvar derivatives cover truncated headers, unsupported
    versions, declared instance arrays beyond table EOF, too-short instance
    records, and explicit instance PostScript name IDs. The remaining uncovered
    fvar lines are the defensive instance-count overflow closure, which is
    unreachable with the table's 16-bit count and size fields.
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
    behavior. FreeType selects typographic metrics first when that bit is set,
    otherwise hhea when nonzero, then OS/2 typo and Windows fallbacks; Rust now
    uses that order consistently for face and size metrics. `tt/head.rs` and
    `tt/os2.rs` have 100% structural coverage, while `tt/post.rs` currently
    has full function coverage and 95/98 lines with the remaining direct
    resolver guards classified in the coverage plan.
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
23. Eight 1.7–2.1 KiB cmap controls cover format 4, 6, and 12 lookup and
    iteration boundaries, every parser error, unsupported-only faces, and
    range-offset validation. They exposed two real differences: format 6 wraps
    at `0xFFFFFFFF` while formats 4/12 stop, and format 12 advances past a zero
    start glyph within the same group. `tt/cmap.rs` now has 100% function,
    line, region, and branch coverage.

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
