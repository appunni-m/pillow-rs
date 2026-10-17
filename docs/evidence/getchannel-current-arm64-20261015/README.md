# GetChannel current ARM64 receipt baseline — 2026-10-15

This is a current-tree diagnostic capture for RGB GetChannel, not a clean
matrix update or an optimization result. The source tree was dirty at revision
`9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`, so these numbers must not be mixed
with the pinned clean-host operation matrix.

## Frozen workload and parity

- Workload: `pil-image-image.getchannel.materialized.rgb-1024x768`
- Parity case: `PIL.Image.Image.getchannel.nuanced.performance-rgb-1024x768`
- Input: RGB, 1024×768; output mode L.
- Boundary: public `call` plus `observe-result`; input setup is outside timing.
- Cache warm; five warmups; 20 iterations × five samples; concurrency one.
- Pillow: 12.2.0; CPython 3.12.13; macOS 15.7.7 ARM64.
- Focused parity report: 2 selected, 2 executed, 2 passed, 0 failed (CPU and
  SIMD).

## Results and execution receipts

| Profile | Median latency | P95 | Speedup vs Pillow | Receipt |
| --- | ---: | ---: | ---: | --- |
| Pillow | 0.138104 ms | 0.142084 ms | 1.000× | Pillow oracle |
| Serial CPU | 0.185313 ms | 0.533584 ms | 0.745× | actual CPU, complete, no fallback; ExtractBand path `cpu` |
| SIMD | 0.080792 ms | 0.086125 ms | 1.709× | actual SIMD, complete, no fallback; `neon-deinterleave`, 49,152 vector blocks, zero scalar tail |

Both target executions report one ExtractBand operation and one 786,432-byte
output allocation. Their receipt accounts for two host buffers totaling
3,145,728 bytes and one mode conversion. Result observation/materialization is
included in the public-call boundary. The SIMD run proves actual NEON vector
execution; its measured speedup is still below the campaign's 2× checkpoint
and the repository's 5× SIMD target. The serial CPU result is slower than
Pillow.

An initial all-profile parity attempt selected CPU, SIMD, and GPU. CPU and SIMD
passed; the GPU call failed because this host enumerated no GPU adapters
(`enumerated=0 adapters=[]`). This is a host capability limitation, not GPU
parity evidence or accelerated execution. The subsequent measured capture
therefore selected only CPU and SIMD. No GPU timing is available.

## Reports

- `cpu-simd-benchmark.json` — focused measurements and execution/resource
  receipts; run `migration-benchmark-ef5b30f977774737ba8babb0ec15ce62`.
- `cpu-simd-parity.json` — focused CPU/SIMD parity proof; run
  `migration-parity-benchmark-gate-f9ca7ad8e2924e2dbee1d172e4bc6eb0`.
- `cpu-simd-raw-samples.json` — raw samples referenced by the report.
- `cpu-simd-benchmark.attestation.json` and
  `cpu-simd-benchmark.producer-attestation.json` — execution and producer
  identities.
- `benchmark.json` and `parity.json` — initial all-profile diagnostic,
  including the unavailable GPU lane.

The three successful subject rows are in
`../performance-optimization-local-runs.csv`. They are marked as a current
tree baseline; the canonical matrix remains unchanged. No full campaign,
coverage, commit, or push was run for this capture.
