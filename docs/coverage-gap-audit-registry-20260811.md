# Compute registry and dispatch coverage gap audit

Date: 2026-08-11

This is an evidence-only audit of the compute registry and CPU/SIMD dispatch
registration paths. It uses the managed Coverage MCP snapshots requested for
this bucket and the source at baseline `6ebcd4473` in the isolated worktree.
No generator, fixture, source, runtime, GPU, crash-quarantine, `Image.show`,
pending 16-bit TIFF, or fontdone work was performed.

## Baselines and result

The Coverage MCP executable-line denominator is smaller than the physical
source-line count. The registry file has 2,734 physical lines but 1,364
counted executable lines in both snapshots.

| File and lane | Snapshot | Lines | Branches | Functions | Gap summary |
| --- | --- | ---: | ---: | ---: | --- |
| `pillow-rs/src/compute/registry.rs` CPU | `1f128a51-e2da-4f31-8d13-bea9de1aec43` | 1,000/1,364 (73.31%) | 75/176 (42.61%) | 91/108 (84.26%) | 349 uncovered lines; 86 partial-branch lines; 0 uncovered functions |
| `pillow-rs/src/compute/registry.rs` SIMD | `35d7073a-48f8-4a80-848d-9261a06c1de4` | 701/1,364 (51.39%) | 19/176 (10.80%) | 30/108 (27.78%) | 646 uncovered lines; 86 partial-branch lines; 0 uncovered functions |

The low SIMD line and branch rates do not mean SIMD registration is absent.
`register_all`'s SIMD wiring at `registry.rs:2630-2733` is line-covered in the
SIMD snapshot. `SimdPool::execute_batch` calls `entry.simd_fn`; it does not run
the scalar `cpu_fn` closures, which explains most SIMD-only gaps below.

## Exact registry line gaps

The following lists are source line numbers returned by managed coverage,
filtered to counted executable lines. The common list has 340 lines, CPU-only
has 9 lines, and SIMD-only has 306 lines; these sums match 349 CPU gaps and 646
SIMD gaps.

### Uncovered in both CPU and SIMD (340 lines)

```text
94, 335, 378-379, 394, 402-404, 426-428, 439-444, 612-613, 637, 640, 643, 646, 649-651, 654, 658-663, 665, 669, 673-684, 689, 692-693, 697-698, 702-703, 707, 710-712, 715, 719-721, 723-726, 728-729, 734-736, 738-741, 743-744, 748, 751-759, 761, 765, 768-780, 782, 786-789, 793-794, 797-800, 805-808, 810, 814, 817, 820, 823, 826, 830-835, 838-844, 850-855, 858-867, 869, 871-877, 879, 883, 886, 890-894, 899, 902, 906-909, 912-914, 916, 918-920, 925-927, 930-934, 936, 938, 942, 945, 948, 951, 954, 957-958, 964-968, 970-975, 980, 982, 1036, 1058, 1084, 1100, 1116, 1132, 1155, 1167-1170, 1172, 1174, 1191, 1214, 1235, 1251, 1267, 1283, 1299, 1315, 1331, 1347-1349, 1354, 1368-1370, 1375, 1391, 1407, 1423, 1439, 1455, 1471, 1495, 1511, 1527, 1550, 1573, 1589, 1605, 1621, 1641, 1659, 1672, 1685, 1698, 1711, 1724, 1737, 1750, 1763, 1776, 1789, 1802, 1815, 1828, 1844, 1860, 1869-1871, 1873, 1875, 1882-1884, 1886, 1888, 1902, 1918, 1936, 1952, 1968, 1984, 2006, 2027, 2040, 2066, 2078, 2096, 2112, 2132, 2145-2147, 2149, 2151, 2175, 2193, 2213, 2239, 2258, 2274, 2291, 2305-2307, 2309, 2311, 2323-2325, 2327, 2329, 2341, 2343-2350, 2352, 2354, 2356, 2390, 2418, 2448, 2476, 2502, 2516, 2546, 2578, 2610, 2622
```

### CPU-only uncovered lines (SIMD covers them; 9 lines)

```text
447-449, 460-465
```

These are the SIMD capability predicate's rotate exclusions and result path.
They are uncovered by the CPU lane because CPU routing never calls
`simd_supports`; they are not a missing SIMD registration.

### SIMD-only uncovered lines (CPU covers them; 306 lines)

```text
1048, 1050-1054, 1056, 1060, 1112-1114, 1118, 1128-1130, 1134, 1146, 1148, 1150-1151, 1153, 1157, 1184-1185, 1188-1189, 1193, 1205, 1207-1210, 1212, 1216, 1226, 1228-1231, 1233, 1237, 1247-1249, 1253, 1263-1265, 1269, 1279-1281, 1285, 1295-1297, 1301, 1311-1313, 1317, 1327-1329, 1333, 1345-1346, 1350-1352, 1356, 1366-1367, 1371-1373, 1377, 1387-1389, 1393, 1403-1405, 1409, 1419-1421, 1425, 1435-1437, 1441, 1451-1453, 1457, 1467-1469, 1473, 1483, 1485-1491, 1493, 1497, 1507-1509, 1513, 1523-1525, 1529, 1539, 1541-1546, 1548, 1552, 1562, 1564-1569, 1571, 1575, 1585-1587, 1591, 1601-1603, 1607, 1617-1619, 1623, 1632, 1634-1637, 1639, 1643, 1650, 1652-1655, 1657, 1661, 1668-1670, 1674, 1681-1683, 1687, 1694-1696, 1700, 1707-1709, 1713, 1720-1722, 1726, 1733-1735, 1739, 1746-1748, 1752, 1759-1761, 1765, 1772-1774, 1778, 1785-1787, 1791, 1798-1800, 1804, 1811-1813, 1817, 1824-1826, 1830, 1840-1842, 1846, 1856-1858, 1862, 1898-1900, 1904, 1914-1916, 1920, 1932-1934, 1938, 1948-1950, 1954, 1964-1966, 1970, 1980-1982, 1986, 2014, 2016-2018, 2021-2023, 2025, 2029, 2036-2038, 2042, 2058-2060, 2062-2064, 2068, 2074-2076, 2080, 2087, 2089-2092, 2094, 2098, 2108-2110, 2114, 2161, 2163-2170, 2172-2173, 2177, 2205, 2207-2209, 2211, 2215, 2250, 2252-2254, 2256, 2260
```

These are scalar closure bodies that the SIMD lane intentionally bypasses in
favor of registered adapters. Adding more SIMD input cases cannot execute a
scalar closure while the dispatch contract remains one adapter per operation.

### Partial branches

Both registry snapshots report 86 partial-branch source locations. The exact
locations are below; the CPU counts are shown as `covered/total`, and the SIMD
snapshot has the same locations with the normal CPU-closure branches less
covered because SIMD calls `simd_fn`.

```text
723[0/2], 738[0/2], 1033[1/2], 1049[1/2], 1071[1/2], 1097[1/2], 1113[1/2], 1129[1/2], 1147[1/2], 1168[0/2], 1185[1/2], 1206[1/2], 1227[1/2], 1248[1/2], 1264[1/2], 1280[1/2], 1296[1/2], 1312[1/2], 1328[1/2], 1346[1/2], 1347[0/4], 1351[1/2], 1367[1/2], 1368[0/4], 1436[1/2], 1452[1/2], 1484[1/2], 1508[1/2], 1524[1/2], 1540[1/2], 1563[1/2], 1586[1/2], 1602[1/2], 1618[1/2], 1633[1/2], 1651[1/2], 1669[1/2], 1682[1/2], 1695[1/2], 1708[1/2], 1721[1/2], 1734[1/2], 1747[1/2], 1760[1/2], 1773[1/2], 1786[1/2], 1799[1/2], 1812[1/2], 1825[1/2], 1841[1/2], 1857[1/2], 1870[0/2], 1883[0/2], 1933[1/2], 1949[1/2], 1965[1/2], 1981[1/2], 2003[1/2], 2015[1/2], 2037[1/2], 2059[1/2], 2075[1/2], 2088[1/2], 2109[1/2], 2129[1/2], 2146[0/2], 2162[1/2], 2190[1/2], 2206[1/2], 2229[1/2], 2251[1/2], 2271[1/2], 2288[1/2], 2306[0/2], 2324[0/2], 2342[0/2], 2368[1/2], 2394[1/2], 2422[1/2], 2452[1/2], 2480[1/2], 2506[1/2], 2520[1/2], 2550[1/2], 2582[1/2], 2614[1/2]
```

## Classification and public operation mapping

### No valid input can cover these gaps under this audit

| Lines | Classification | Evidence and required action |
| --- | --- | --- |
| `registry.rs:94` | Defensive registry-initialization error propagation | `register_all` succeeds for the current table; reaching this arm requires a persistent initialization error. No input batch is justified. |
| `registry.rs:335`, `378-379`, `394`, `402-404` | Legacy or direct-constructor variants | `Quantize` materializes directly in `pillow-rs/src/ops/quantize.rs:2146`; public blend/composite constructors push `BlendModule`/`CompositeModule`; public point/eval uses `Eval`; gradients and Mandelbrot construct images directly. The corresponding `PipelineOp` variants are retained for the internal/GPU ABI, not reachable through the current public pipeline. No runtime change is indicated. |
| `registry.rs:426-444` | GPU support predicate | GPU is explicitly out of scope. These lines require GPU-oriented support checks and must remain in the GPU/crash bucket. |
| `registry.rs:612-982` | GPU parameter extraction | `extract_params` is consumed by GPU dispatch. CPU/SIMD operations do not upload shader parameter blocks, so no valid non-GPU input can execute these lines. No runtime change is indicated. |
| `registry.rs:1036-2622` typed `expected ... op` arms | Backend-unreachable mismatch guards | The closure gets its own key from the same `PipelineOp` variant used in `variant_key`; valid public operations take the matching branch. These uncovered arms require deliberately mismatched internal values, not public parity inputs. Draw entries are CPU-only and their error arms are the same typed-guard pattern. |
| `registry.rs:1167-1170`, `1869-1871`, `1882-1884`, `2145-2147`, `2305-2307`, `2323-2325`, `2341`, `2343-2352` | Legacy/direct-constructor normal branches | These are the normal bodies for the internal `Quantize`, legacy `Blend`/`Composite`, `PointOp`, `LinearGradient`, `RadialGradient`, and `EffectMandelbrot` variants. The current public APIs materialize or route elsewhere, so no public input can reach them without a runtime/API change. |
| `registry.rs:1347-1349`, `1368-1370` | Internal mode-validation branches, public-prevalidated | These branches reject `LA`/`RGBA` inside the registry, but `ImageOps.autocontrast` and `ImageOps.equalize` reject those modes in `pillow-rs/src/ops/imageops.rs:245-257` and `279-286` before pushing a pipeline op. The public mode case IDs are valid negative tests, but they cannot cover these registry lines without changing the runtime validation boundary. |
| SIMD-only ranges above | Scalar CPU closures bypassed by SIMD | `SimdPool::execute_batch` selects `entry.simd_fn`, so adding a SIMD case cannot cover the scalar `cpu_fn` body. This is dispatch design, not a missing adapter registration. |

### Safe public case IDs for a later focused run

These are candidate IDs already present in the managed input plan. They are
listed for follow-up mapping only; this audit did not run them. Each should be
run as an isolated, explicit CPU or SIMD case and verified to contain one
pipeline operation before being used as a coverage batch.

| Candidate case ID(s) | Expected operation | Likely value |
| --- | --- | --- |
| `PIL.Image.Image.resize.behavior.default`, `PIL.Image.Image.crop.behavior.default`, `PIL.Image.Image.transpose.behavior.default`, `PIL.Image.Image.transform.behavior.default` | `Resize`, `Crop`, `Transpose`, `Transform` | Reachable normal dispatch; no runtime change expected |
| `PIL.Image.Image.getchannel.behavior.default` | `ExtractBand` | Preferred single-operation candidate over split |
| `PIL.ImageFilter.BoxBlur.behavior.default`, `PIL.ImageFilter.GaussianBlur.behavior.default`, `PIL.ImageFilter.MedianFilter.behavior.default`, `PIL.ImageFilter.MaxFilter.behavior.default`, `PIL.ImageFilter.MinFilter.behavior.default`, `PIL.ImageFilter.RankFilter.behavior.default` | Corresponding filter variants | Reachable CPU/SIMD adapter paths |
| `PIL.Image.Image.filter.parameter-combination.legacy-001` | `Filter3x3` or `Filter5x5` depending on the declared filter | Candidate only; exact variant must be confirmed from the input plan |
| `PIL.ImageOps.contain.behavior.default`, `PIL.ImageOps.cover.behavior.default`, `PIL.ImageOps.fit.behavior.default`, `PIL.ImageOps.pad.behavior.default`, `PIL.ImageOps.expand.behavior.default`, `PIL.ImageOps.crop.behavior.default` | `Contain`, `Cover`, `Fit`, `Pad`, `Expand`, `CropBorder` | Reachable dimension/image-op paths; verify operation count |
| `PIL.ImageOps.autocontrast.mode.la`, `PIL.ImageOps.autocontrast.mode.rgba`, `PIL.ImageOps.equalize.mode.la`, `PIL.ImageOps.equalize.mode.rgba` | Public mode-validation cases | Reachable public errors, but rejected before registry dispatch; they do not cover `registry.rs:1347-1349` or `1368-1370` without a runtime boundary change |
| `PIL.Image.Image.remap_palette.behavior.default` | `RemapPalette` | Reachable CPU/SIMD adapter path |
| `PIL.ImageChops.add.behavior.default`, `PIL.ImageChops.difference.behavior.default` | `Add`, `Difference` | Reachable dual-image adapter paths |
| `PIL.ImageChops.blend.behavior.default` | `BlendModule` | Current module path, not legacy `Blend` |
| `PIL.Image.composite.behavior.default` | `CompositeModule` | Current module path, not legacy `Composite` |
| `PIL.Image.merge.behavior.default` | `Merge` | Reachable CPU/SIMD path; SIMD has a dedicated P-mode result rule |
| `PIL.Image.eval.behavior.default` | `Eval` | Reachable CPU/SIMD path; public `point` also routes through eval validation |
| `PIL.Image.Image.putpixel.behavior.default`, `PIL.Image.Image.putdata.behavior.default`, `PIL.Image.Image.putalpha.behavior.default` | `PutPixel`, `PutData`, `PutAlpha` | Reachable mutating paths |
| `PIL.Image.Image.effect_spread.behavior.default`, `PIL.Image.effect_noise.behavior.default` | `EffectSpread`, `EffectNoise` | Valid CPU-only paths; not SIMD targets |
| `PIL.ImageFilter.Color3DLUT.behavior.default` | `Color3DLut` | Valid CPU-only candidate; not GPU/SIMD |
| `PIL.ImageDraw.ImageDraw.line.behavior.default`, `PIL.ImageDraw.ImageDraw.rectangle.behavior.default`, `PIL.ImageDraw.ImageDraw.rounded_rectangle.behavior.default`, `PIL.ImageDraw.ImageDraw.ellipse.behavior.default`, `PIL.ImageDraw.ImageDraw.circle.behavior.default`, `PIL.ImageDraw.ImageDraw.polygon.behavior.default`, `PIL.ImageDraw.ImageDraw.arc.behavior.default`, `PIL.ImageDraw.ImageDraw.chord.behavior.default`, `PIL.ImageDraw.ImageDraw.pieslice.behavior.default`, `PIL.ImageDraw.ImageDraw.point.behavior.default` | Corresponding `Draw*` variants | Valid CPU-only paths; their mismatched closure guards remain unreachable |

There is no valid public case ID for the exact legacy `Quantize`, `Blend`,
`Composite`, or `PointOp` registry variants, nor for the direct-constructor
`LinearGradient`, `RadialGradient`, and `EffectMandelbrot` entries. Their public
names must not be used as evidence that these registry branches are reachable.

## Adjacent dispatch files

### `pillow-rs/src/compute/pool_cpu/mod.rs`

Both snapshots report 20/20 executable lines and 4/4 functions. The one
partially covered region is not an uncovered line. CPU registration/execution
itself is therefore not a candidate for a runtime change.

### `pillow-rs/src/compute/pool_simd/mod.rs`

The SIMD snapshot has 62/70 lines, 2/2 branches, and 9/13 functions. Its only
uncovered SIMD-lane lines are:

```text
40-41, 53-54
```

These are `from_raw(...).ok_or_else(...)` shape-mismatch guards after the code
has collected exactly one byte per pixel for `P` or two bytes per pixel for
`PA`. A valid public image cannot produce the wrong vector length at this
point, so no valid input batch is justified and no runtime change is needed.

The CPU snapshot's SIMD file gaps are expected because the CPU lane does not
enter the SIMD pool:

```text
28, 32-41, 43-54, 56, 58, 69-71, 73, 79-80, 82-84, 88-94, 96-97, 103-106, 108
```

Line 103 is the merge-special-case branch. It is exercised by the SIMD merge
path and is not a missing registration.

### `pillow-rs/src/compute/mod.rs`

CPU uncovered lines:

```text
53, 151-153, 200-201, 208-210, 213, 215, 218, 240-244, 246, 275
```

SIMD uncovered lines:

```text
53, 151-153, 200-201, 240-244, 246, 275
```

Partial branch locations are CPU `200, 208, 213, 238` and SIMD `200, 238`.
These are backend-control API and router validation paths: unknown backend
parsing (`53`), backend state inspection (`151-153`), explicit routing and
support fallback (`200-218`), unsupported-operation formatting (`238-246`),
and unavailable-backend execution (`275`). They need dedicated backend-control
tests or compiled-backend configuration, not a normal image input batch. The
public core hook `Image::use_backend` is at `pillow-rs/src/image.rs:2670`, but
no current parity case was found that exercises these controls.

## Decision

No new input batch is justified for this bounded audit. The candidate IDs above
are useful for a later focused, managed run of reachable happy paths, but they
do not target the current uncovered set: the dominant registry gaps are GPU
parameter extraction, legacy/direct-constructor variants, wrong-variant guards,
and scalar closures bypassed by SIMD. The SIMD shape guards and backend-control
gaps are defensive/API-control cases rather than valid public image inputs.

This report intentionally makes no runtime, generator, fixture, or manifest
change. The next action for this bucket is a separately approved focused CPU /
SIMD input run only if the goal changes from explaining the gaps to measuring
reachable operation dispatch.
