# SIMD migration-parity gap classification

This is an audit record for the SIMD coverage batch. It does not change
runtime code, fixture outputs, thresholds, or the parity input generator.

## Baseline

| item | value |
| --- | --- |
| commit | `bec899a5e38902642ca6347bd13e347ef28686bc` |
| suite | `migration-parity-rust-simd` |
| coverage snapshot | `135b0592-60b6-410f-811e-87d83261952b` |
| snapshot aggregate | lines `40759/68614`; branches `7218/14070`; functions `3048/5331`; regions `63389/106660` |
| managed SIMD scope supplied for this batch | lines `24942/30558`; branches `4234/5188`; functions `1992/2667`; regions `40987/51767` |

Target-file metrics in the snapshot:

| file | lines | branches | functions | regions |
| --- | ---: | ---: | ---: | ---: |
| `pillow-rs/src/compute/pool_simd/ops/adapters.rs` | `1125/1182` | `110/156` | `77/82` | `2045/2175` |
| `pillow-rs/src/compute/pool_simd/ops/scalar.rs` | `2670/2695` | `766/842` | `109/109` | `5112/5169` |

## Focused parity evidence

The existing valid SIMD/ImageChops/ImageOps/filter cases were run through
the repository Make target after building the worktree extension:

```text
VIRTUAL_ENV=/Users/lazytrot/work/pillow-rs/.venv \
PYTHON=/Users/lazytrot/work/pillow-rs/.venv/bin/python \
MIGRATION_TARGET_BACKEND=simd \
MIGRATION_PARITY_ARGS='--case-id PIL.ImageOps.solarize.nuanced.materialized-l --case-id PIL.ImageOps.solarize.nuanced.materialized-rgb-high-channels --case-id PIL.ImageOps.posterize.nuanced.materialized-rgb --case-id PIL.ImageChops.invert.nuanced.la --case-id PIL.ImageChops.invert.nuanced.rgba --case-id PIL.ImageChops.add.nuanced.materialized-p --case-id PIL.ImageOps.contain.nuanced.simd-native-scalar-fallback-f --case-id PIL.ImageOps.scale.nuanced.simd-native-scalar-fallback-f --case-id PIL.Image.Image.filter.nuanced.f-mode-max-filter --case-id PIL.Image.Image.filter.nuanced.f-mode-min-filter --case-id PIL.Image.Image.filter.nuanced.f-mode-median-filter --case-id PIL.Image.Image.filter.nuanced.f-mode-rank-filter --case-id PIL.Image.Image.filter.nuanced.i-mode --case-id PIL.Image.Image.filter.nuanced.i-mode-detail-fused-row' \
make migration-parity-test
```

The same command without `MIGRATION_TARGET_BACKEND=simd` was the CPU run.
Both runs were `14 selected / 14 executed / 14 passed / 0 failed`.
`make build-dev` was run first with the task-local `UV_CACHE_DIR` because the
default cache is not writable in this worktree. No coverage delta was claimed
from these parity-only runs.

## Classification

The coverage report contains both executable regions and branch arms. A
partial `if let PipelineOp::<expected>` line has an unreachable wrong-variant
arm in a correctly registered adapter; its paired call body is a separate
valid reachability question. The following guards are justified unreachable
for valid public inputs and should not receive synthetic error cases.

### `adapters.rs`: unreachable buckets

- Unknown mode/channel fallbacks: lines `45`, `79`.
- Internal packed-buffer shape failure: lines `174-175`.
- Wrong-operation error arms: lines `632-633`, `785-788`, `812`, `856`,
  `888`, `920`, `950`, `994`, `1018`, `1067`, `1084`, `1100`, `1123`,
  `1159`, `1176`, `1226`, `1246`, `1322`, `1359-1360`, `1373-1374`.
- Unsupported merge mode and invalid band-shape errors: lines `1517-1519`,
  `1540-1543`.
- The final expected-op merge error: lines `1550-1551`.

The valid adapter dispatch regions still require managed SIMD coverage
accounting if they remain uncovered: solarize/posterize/brightness/contrast/
saturation/sharpness/colorize/constant/offset (`290-427`), F/I filter dispatch
(`497-617`), dual ImageChops operations (`650-653`, `685-740`), geometry
dispatch and native F/I fallbacks (`832-1065`), transform no-fill (`1316`),
paste/put-data/put-alpha/eval/alpha-composite (`1416-1482`), and merge's valid
path (`1497-1549`). No invalid or wrong-variant inputs were added.

### `scalar.rs`: unreachable buckets

These are input guards or impossible fallback arms for the public adapters:

- Offset/geometry bounds and zero-work guards: `741-743`, `2041`, `2179`,
  `2231`, `2279-2280`, `2285`, `2426-2432`.
- Unknown transpose method fallback: `1872`.
- Invalid zero-destination thumbnail guard: `4325-4326`.
- Unmatched conversion fallback after the supported mode matrix:
  `4551-4558`.

The remaining scalar gaps are valid mode/geometry or data-dependent branches,
not proven unreachable. They are retained for a future managed run rather
than being represented by invalid inputs. Exact snapshot gap ranges are:

```text
18, 29, 64, 75, 89, 102,
534, 549-552, 567, 582-585, 600, 615-618, 676-677,
1872, 2041, 2179, 2231-2232, 2276, 2279-2280, 2282, 2285, 2338,
2373, 2426-2427, 2431-2432,
3267, 3278-3280, 3301, 3306, 3333, 3350, 3460, 3483,
3615-3616, 3619-3620, 3968, 3969, 3989-3990, 3994-3995,
4025, 4028, 4131, 4152, 4177, 4199, 4254, 4325-4326, 4370,
4488-4489, 4553-4554, 4556, 4558.
```

The `2276` fit zero-width-source path and the F/I filter and mode-preserving
geometry paths are already represented by valid generator cases; the focused
14-case result confirms their CPU/SIMD parity. This batch therefore makes no
coverage claim and adds no input case until a managed coverage run can show a
real delta.
