# Transpose RGBA 32x32 packed CPU tile trial — x86_64

## Decision

Partial kernel gain; keep the 32x32 tile in the candidate source and leave the
operation open. Against the previous 16x16 packed-kernel runs, median CPU
latency fell 2.9% and p95 fell 5.3%. The new CPU-only runs still exceed paired
Pillow latency by 24% and 45%, so the operation target remains unmet.

## Frozen scope

- Workload: `pipeline-matrix.transpose-tiled.rgba-772x768-transpose-mirror.operation-materialized`
- Timed steps: `transpose-0`, `transpose-1`, `materialize`; allocation and full result materialization are included.
- Profile: serial CPU only, x86_64, release; 5 warmups and 20 iterations x 5 samples (100 observations per subject), concurrency 1.
- Main parity case: `PIL.Image.Image.transpose.nuanced.tiled-aligned-rgba-772x768-methods-5-0`.
- Odd-size parity guard: `PIL.Image.Image.transpose.nuanced.tiled-odd-rgba-1025x1023-methods-5-0`.
- No full benchmark or parity campaign and no coverage ran.

## Results

| Run | Pillow median (ms) | CPU median (ms) | CPU/Pillow | CPU p95 (ms) | CPU backend receipt | Main parity |
|---|---:|---:|---:|---:|---|---|
| 1 | 2.235557 | 2.780047 | 1.244x | 3.045492 | `cpu`, 2 operations, no fallback | Pass |
| 2 | 1.908202 | 2.764493 | 1.449x | 3.003703 | `cpu`, 2 operations, no fallback | Reused run 1 proof after runtime identity matched |

Each target call allocated 2,371,584 host bytes, with zero upload/readback bytes. The odd-size guard passed (1 selected, 1 executed).

## Candidate

Only the packed x86_64 RGBA Rotate270 kernel uses a 32x32 outer tile. The
existing generic RGBA fallback remains at 16x16. Candidate source hashes:

- `geometry.rs`: `83e261db28927037b9cf461be6f1b911932bec9648b9567de5cabe2e07f38017`
- `transpose_rgba/x86_64.rs`: `81c2a43d1e01cc1bc947943c52cb09c93a4ea53cc997df28d60ba7a8653c196e`

## Artifacts

Full reports and raw samples remain on the native runner at
`/home/appunni/work/pillow-rs-current-local-x86-20261019/docs/evidence/transpose/packed-4x4-cpu-x86-20261019/tile32/`:
`cpu-only/1/` contains the selected benchmark, raw samples, parity report, and
attestations; `cpu-only/2/` contains the second benchmark and reused parity
report; `cpu-only/odd-guard.json` contains the odd-size guard. The local
checkout keeps this summary and the four-row matrix extract only. No target
directory or full parity report was copied. An earlier harness-default run
under `B1/` measured additional profiles and is excluded from this CPU-only
result.
