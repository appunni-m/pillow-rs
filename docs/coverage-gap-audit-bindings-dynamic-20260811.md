# Binding and dynamic-format coverage gap audit

Date: 2026-08-11
Status: bounded, evidence-only audit; no source, generator, fixture, or denominator changes

## Result

No new input batch is justified for this bucket. The public parity corpus already
contains valid cases for the reachable 8-bit and `Luma16` dynamic paths and for
the relevant binding helpers. A small set of `Luma16` conversion arms could be
reached only by adding a new cross-operation chain after `frombytes`; the larger
remaining dynamic gap requires native `LumaA16`, `Rgb16`, `Rgba16`, `Rgb32F`, or
`Rgba32F` values, or direct Rust-only trait and constructor calls. Creating the
latter through the public encoded-input route would enter the pending
16-bit/float format work, which is explicitly outside this audit. The remaining
wrapper gaps are protocol/error mappings, formatting adapters, registration,
backend controls, or font endpoints.

No runtime implementation change is supported by the evidence collected here.
The Python wrapper delegates image behavior to `pillow_rs::Image`/`_core`; its
loops are limited to Python protocol and result-shape marshaling. The one
algorithmic-looking helper, `mesh_flatten`, only validates and flattens a Python
mesh before the Rust transform path; it does not implement image behavior.

## Baseline and method

The audit was performed against source commit
`6ebcd44736aa706e0c0661b223406b0b240c5086` and managed CPU coverage snapshot
`1f128a51-e2da-4f31-8d13-bea9de1aec43` (`main`, `migration-parity-rust`, LLVM).
Coverage MCP file queries were used; `coverage show` was not used.

| File | Executable lines | Branches | Functions | Regions |
| --- | ---: | ---: | ---: | ---: |
| `pillow-rs-py/src/lib.rs` | 3,113 / 3,647 (85.36%) | 281 / 350 (80.29%) | 400 / 502 (79.68%) | 4,653 / 5,907 (78.77%) |
| `pillow-rs/src/raster/dynamic.rs` | 388 / 865 (44.86%) | 2 / 4 (50.00%) | 60 / 119 (50.42%) | 741 / 1,636 (45.29%) |

The line ranges below are executable-line gaps in that snapshot. A line can be
listed because one of its branch records is only partially covered.

## Public input mapping

| Candidate public input | Exact relevant code | Finding and classification |
| --- | --- | --- |
| `PIL.ImageSequence.Iterator` on an ordinary image and a single-frame GIF | `lib.rs:61, 72-74, 93, 96` | The active plan already has `PIL.ImageSequence.Iterator.__iter__.behavior.default`, `__next__.behavior.default`, and `__next__.nuanced.opened-single-frame-gif`. Missing lines are `_min_frame` absence/type/error and EOF/non-EOF exception protocol. **Binding protocol / negative-contract branch; no dynamic implementation change.** |
| `Image.open` with a missing `str` path, missing `bytes` path, and a path containing an embedded NUL | `lib.rs:101-147` (`108-109, 131, 135, 138, 145-146`) | These are host path extraction and exception-shape preservation. A missing bytes path is a valid candidate for line 135 and a missing string/path-like value for line 138; embedded NUL is a valid negative contract for 108-109. They exercise the wrapper’s filesystem boundary, not `dynamic.rs`. **Binding protocol; out of the dynamic batch.** |
| `Image.frombytes` with native 16-bit grayscale, followed by a conversion | `lib.rs:1468-1479` (covered binding path); candidate core arms `dynamic.rs:291-296, 307-311, 322-326` | The corpus already has `PIL.Image.frombytes.nuanced.valid-i16n`, `valid-i16`, `valid-i16l`, `valid-i16b` and the corresponding `PIL.Image.Image.frombytes` cases. A new `frombytes("I;16", ...).convert("RGBA"/"L"/"LA")` chain is a valid narrow test candidate for the remaining Luma16 conversion arms, but it requires a new cross-operation fixture and would not address the native La16/Rgb16/Rgba16/F32 gaps. **Test-only follow-up, not justified or added in this bounded audit.** |
| Rotate/resize invalid and non-canonical arguments | `lib.rs:149-221` (`158, 160-162, 171-172, 177, 179, 184, 197, 200, 211-212`) | Existing generator cases include `rotate.parameter.invalid-resample-name`, `invalid-resample-none`, `invalid-resample-lanczos`, `invalid-resample-box`, `invalid-resample-hamming`, `invalid-resample-unknown-code`, `non-boolean-expand`, `invalid-center-arity`, and `invalid-translate-arity`. These functions only classify Python values and pass Rust enums/records to core. **Thin binding normalization; additional wrapper-only coverage would be broader than this checkpoint.** |
| Palette/transform/paste/quantize argument shapes | `lib.rs:223-358` (`258-259, 333-334, 344-345, 357-358`) | Valid candidates are the existing `convert` palette-image case, transform mesh/affine malformed cases, and `reduce`/center/box error cases. The adapter delegates validation and operation semantics to core. **Thin binding normalization; no dynamic-format input is created.** |
| Existing public forwarding methods | `lib.rs:675, 681, 688-691, 862-863, 886, 925-1000, 1075-1144, 1254-1425, 1502-1603` | Candidate operations already exist in the coverage plan: `crop`, `getdata`, `getextrema`, `getpixel`, `load`, `point`, `putalpha`, `putdata`, `putpixel`, `transform`, `transpose`, `effect_noise`, `blend`, `composite`, and `merge`. The methods are short delegates or Python result-shape conversion. **Public wrapper coverage gap, not a runtime gap; adding cases would not unlock the dynamic 16/32-bit arms.** |
| `PIL.Image.fromarray` | `lib.rs:1923-1937, 2004-2054` (`1923-1937, 2013-2020`) | Valid existing cases are `buffer-backed-rgb-values`, `buffer-backed-luma-1d`, `unsupported-dtype`, `scalar-array-empty-shape`, `bytes-object-rejected`, `height-overflow`, and `width-overflow`. Layout policy is in `pillow-rs::resolve_array_layout`; the binding reads `__array_interface__` and marshals bytes. **Thin protocol boundary; no wrapper logic change justified.** |
| `Image.transform` mesh data | `lib.rs:2061-2078` | Existing candidates are `missing-mesh-data`, `empty-mesh-data`, `empty-mesh-data-p-scalar-fill`, `flat-mesh-data`, `identity-mesh`, and malformed mesh cases. `mesh_flatten` validates tuple shape and copies values; transform behavior remains in Rust core. **Boundary marshaling; not a dynamic-format gap.** |
| `Image.point` and `Image.eval` callable/LUT cases | `lib.rs:2082-2102` | Existing candidates include `point` `l-replicated-lut`, `rgb-replicated-lut`, `rgb-expanded-lut`, `la-replicated-lut`, `rgb-callable-lut`, `rgb-invalid-lut-length`, plus `eval` `rgb-replicated-lut`, `rgb-expanded-lut`, `invalid-lut-length`, `clamp-shift-callable`, `l-callable-expanded-path`, `la-alpha-lut`, and `rgba-alpha-lut`. The callback is invoked from the binding, while LUT construction/validation is delegated to core. **Python callback protocol; no dynamic-format arm is reached by these cases.** |

The following wrapper ranges are deliberately not proposed as new work in this
audit: `lib.rs:1944-1992` (backend control, including the GPU path),
`lib.rs:1998-2002` (ImageQt/BMP row padding), `lib.rs:2182` (module
registration), and the font-related method blocks beginning at
`lib.rs:2333`, `2441`, `2931`, `3083`, and `3197`. They are outside the
binding/dynamic CPU bucket or explicitly excluded by the task.

## Complete wrapper gap index

This is the complete compact line-range index returned by the file-level query;
the ranges are grouped by source window to keep the report reviewable.

```text
lib.rs: 61, 72-74, 93, 96, 108-109, 131, 135, 138, 145-146,
         158, 160-162, 171-172, 177, 179, 184, 197, 200, 211-212,
         258-259, 333-334, 344-345, 357-358,
         456, 464, 489-490, 550, 555, 558, 561-562, 569-570,
         573-574, 577-583, 586-587, 608, 611, 613-615,
         675, 681, 688-691,
         862-863, 886,
         925-927, 933-935, 948-950, 952-954, 956-958, 960-962,
         976-978, 984-987, 989, 991-994, 996, 998-1000,
         1075-1077, 1114-1116, 1128-1130, 1139-1144,
         1254-1256, 1269-1272, 1299, 1308, 1321-1323, 1351-1353,
         1382-1384, 1393-1398, 1419-1420, 1422, 1425,
         1502-1506, 1509, 1515-1520, 1523, 1529-1535, 1538-1543,
         1582-1584, 1601-1603,
         1644, 1652, 1660, 1714-1716, 1718, 1720, 1725,
         1730-1735, 1738-1739, 1741, 1744-1750, 1752, 1756-1759,
         1798-1800, 1819-1820,
         1821-1824, 1826, 1828, 1832, 1851, 1875-1877, 1879,
         1881-1883,
         1923-1937, 1998-2002, 2013-2020, 2061-2078, 2082-2102,
         2125, 2182,
         2333-2337, 2341, 2347-2349, 2357, 2362-2367, 2391-2392,
         2396, 2398, 2400, 2441, 2465, 2472-2480, 2490-2496,
         2529-2535, 2598, 2603-2616, 2619, 2624-2639, 2691-2697,
         2763-2765, 2768-2770, 2773-2775, 2778-2780, 2783-2785,
         2788-2790, 2793-2795, 2798-2800,
         2931, 2958-2959, 2963-2965, 2968-2969, 2992-2996,
         3040, 3065-3067, 3074, 3083, 3102-3103, 3118-3120,
         3135, 3145, 3147-3148, 3152, 3197,
         3381, 3385-3388, 3390-3391.
```

The font blocks in this index are recorded for completeness but are not
classified further here because fontdone parity is handled separately. The
backend block is likewise not a test target for this report.

## Dynamic dispatch audit

`DynamicImage::new` is not a public binding construction path: current call
sites use the typed constructors or `Image::new`/`Image::frombytes`. The
uncovered `clone_from` and accessor arms are also Rust-internal APIs. The
public `Image` layer maps logical Pillow modes as follows at this baseline:

- `L`, `LA`, `RGB`, and `RGBA` use the native 8-bit variants.
- `I` and `F` use four-byte RGBA storage with an explicit logical mode tag.
- `I;16*` uses native `Luma16` storage for `Image.new`/`Image.frombytes`.
- Decoded `L16` can reach `DynamicImage::ImageLuma16`; the remaining RGB/RGBA
  16-bit and float variants require decoded format support not supplied by the
  active safe batch.

| Dynamic code | Relevant gap area (complete ranges below) | Candidate public input | Classification |
| --- | --- | --- | --- |
| `Clone::clone_from`, generic constructor, typed 16/32-bit constructors | `dynamic.rs:138-158, 166-185, 189-245` | No Pillow operation calls `clone_from` or the generic Rust constructor. Direct typed constructors are not exposed through the binding. | **Internal Rust API / no valid public parity input.** |
| Generic conversions and owned conversions | `dynamic.rs:291-296, 307-311, 322-326, 328, 334-337, 339, 343-346, 348, 355, 361-364, 366, 370-373, 375, 379-382, 384, 406-411, 417-420, 422, 426-429, 431, 438, 444-447, 449, 453-456, 458, 462-465, 467, 471-474, 476, 480-483, 485, 489-492, 494, 498-501, 503` | `Image.convert` and the existing `RGB/RGBA/L/LA` operation cases reach the 8-bit conversion arms. The unhit arms are 16-bit/float targets. | **Native storage dependency; no wrapper change.** |
| Accessors, bytes, and color metadata | `dynamic.rs:511-514, 516, 522, 528-531, 533, 539, 545-548, 550, 553-556, 558, 562-565, 567, 573, 579-582, 584, 587-590, 592, 596-599, 601, 604-607, 609, 613-616, 618, 624, 630-633, 635, 638-641, 643, 647-650, 652, 655-658, 660, 664-667, 669, 672-675, 677` | Public `getdata`, `getpixel`, `tobytes`, and metadata cases cover supported 8-bit images. The unhit accessors are for Rust-native 16-bit/float image references. | **Internal typed accessors / format dependency.** |
| Flip and quarter-turn dispatch | `dynamic.rs:704-708, 721-725, 743-745, 776-780, 813-817, 843-847, 850-854` | Existing `Image.transpose` and `ImageOps.flip` cases cover 1/8-bit modes, and the Luma16 arms at `775`, `812`, and `849` are covered. The missing arms are LumaA16/Rgb16/Rgba16/Rgb32F/Rgba32F. | **Reachable only after broader native-format coverage; not a binding gap.** |
| Crop dispatch | `dynamic.rs:899-903, 929-933, 936-937, 939-943, 1002-1007, 1010, 1012-1017, 1020, 1022-1027, 1030, 1032-1037, 1040, 1042-1047, 1050` | Existing `crop.mode.*` cases cover supported logical modes, and the Luma16 crop arm (`992-1000`) is covered by `l16-png-opened`. The gaps are LumaA16/Rgb16/Rgba16/F32 arms. | **Format dependency; no valid new batch in scope.** |
| Decoded-image materialization | `dynamic.rs:1071-1072, 1078-1082, 1089, 1099, 1102-1104, 1107, 1110-1112, 1115, 1118-1120, 1123, 1126-1128, 1131, 1134-1136, 1138-1139` | `Image.open`/`Image.load` can reach the decoded path. The L8/Luma16 paths are covered; the gaps are La8/La16/Rgb16/Rgba16/F32 and intentional unsupported-mode returns. Pending 16-bit TIFF and float decoding are excluded. | **Codec/decoded-storage dependency; investigate with the format lane, not here.** |
| Generic image-view/write dispatch | `dynamic.rs:1174-1176, 1180-1182, 1186-1188, 1192-1194, 1243-1244, 1249-1251, 1253-1259, 1264-1267, 1269-1273, 1275, 1277-1282, 1284, 1286-1291, 1293, 1295-1299, 1302, 1304-1308, 1311, 1317, 1319-1333, 1335-1336` | Public `putpixel`, `putdata`, and blend/composite operations cover the 8-bit paths. The remaining branches are internal trait behavior or native 16/32-bit variants. | **Internal trait/format dependency; no wrapper implementation change.** |

### Complete dynamic gap index

```text
dynamic.rs: 138-142, 146-158, 160, 166-171, 174-183, 185,
             189-191, 195-197, 201-203, 213-215, 219-221, 225-227,
             231-233, 237-239, 243-245,
             291-296, 307-311, 322-326, 328, 334-337, 339, 343-346,
             348, 355, 361-364, 366, 370-373, 375, 379-382, 384,
             406-411, 417-420, 422, 426-429, 431, 438, 444-447, 449,
             453-456, 458, 462-465, 467, 471-474, 476, 480-483, 485,
             489-492, 494, 498-501, 503,
             511-514, 516, 522, 528-531, 533, 539, 545-548, 550,
             553-556, 558, 562-565, 567, 573, 579-582, 584, 587-590,
             592, 596-599, 601, 604-607, 609, 613-616, 618, 624,
             630-633, 635, 638-641, 643, 647-650, 652, 655-658, 660,
             664-667, 669, 672-675, 677,
             704-708, 721-725, 743-745, 776-780, 813-817, 843-847,
             850-854,
             899-903, 929-933, 936-937, 939-943, 1002-1007, 1010,
             1012-1017, 1020, 1022-1027, 1030, 1032-1037, 1040,
             1042-1047, 1050,
             1071-1072, 1078-1082, 1089, 1099, 1102-1104, 1107,
             1110-1112, 1115, 1118-1120, 1123, 1126-1128, 1131,
             1134-1136, 1138-1139,
             1174-1176, 1180-1182, 1186-1188, 1192-1194,
             1243-1244, 1249-1251, 1253-1255, 1257-1259, 1264-1267,
             1269-1273, 1275, 1277-1282, 1284, 1286-1291, 1293,
             1295-1299, 1302, 1304-1308, 1311, 1317, 1319-1333,
             1335-1336.
```

The dynamic operations are called from Rust imageops/geometry code, so the
8-bit dispatch is reachable without adding Python logic. No line in this audit
demonstrates a Python wrapper defect that prevents a valid in-scope input from
reaching a supported dynamic arm.

## Final classification and next action

| Class | Meaning | Exact areas |
| --- | --- | --- |
| A | Valid public wrapper protocol/negative cases; optional future coverage work | `lib.rs:61-147, 149-221, 223-358, 925-1000, 1075-1144, 1254-1603, 1644-1883` |
| B | Valid public helper cases already represented by the corpus; no runtime change | `lib.rs:1923-1937, 2004-2054, 2061-2102` |
| C | Internal Rust-only or typed dispatch not constructible through the current public parity input contract | `dynamic.rs:138-245, 511-677, 1174-1336` |
| D | Native 16/32-bit storage or decoder dependency; pending format work | `dynamic.rs:291-503, 685-710, 749-1050, 1070-1139` |
| X | Explicitly excluded: GPU/backend, Image.show, crash quarantine, pending 16-bit TIFF, fontdone | `lib.rs:1944-1992, 1998-2002, 2333-3391` (selected gaps listed above) |

The bounded audit is complete. The correct next step is to preserve this report
and revisit classes C/D when the format/storage lane is ready; do not add a
synthetic parity batch or alter coverage accounting for these paths.
