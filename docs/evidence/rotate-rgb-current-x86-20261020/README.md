# RGB nearest Rotate current-source rebaseline — x86_64, 2026-10-20

This selected run repairs the execution-proof gap for one current-source
`PIL.Image.Image.rotate` workload. It establishes that serial CPU beats Pillow
on this host and that the SIMD profile currently loses; it does not close
Rotate across modes, angles, sizes, architectures, or backends.

## Frozen workload and evidence

- Workload: `pipeline-op.rotate.material-rgb-nearest-noise-recovered-20261016-1024x768`
- Parity case: `PIL.Image.Image.rotate.nuanced.rgb-nearest-1deg-noise-recovered-20261016-1024x768`
- Input: RGB noise, 1024×768, nearest 1°, no expansion; input SHA-256
  `8443b82a55b2f7f0c748b5a6572469e5232ef13b27c407a7781d55869e5d845d`.
- Boundary: observed `call` + `observe-result` (`tobytes`). Setup is outside
  timing; Rotate and final byte materialization are included for Pillow and
  pillow-rs.
- Policy: release, warm cache, 5 warmups, 20 iterations × 5 samples (100
  observations per subject), concurrency 1.
- Host: AMD Ryzen 5 2600X, Linux x86_64, CPython 3.12.3, Pillow 12.2.0.
- Target revision `418d5f0cde51ca64a97cbd007ea731da72d68abf`; source worktree was
  dirty. Loaded extension SHA-256
  `13e9f40576c0956b96b8895285f2c4bd1dabda12f28d56c2f61f7988ff09a6b3`.
- Benchmark: `migration-benchmark-dc69129f2e3349b79f8388e92174aa61`.
- Fresh exact parity: `migration-parity-benchmark-gate-77fd480b02474917bb24213ce45c9dd3`,
  1 selected case × CPU/SIMD, 2/2 passed.

| Subject | Median | P95 | Median throughput | Receipt |
| --- | ---: | ---: | ---: | --- |
| Pillow 12.2.0 | 5.925813 ms | 8.148423 ms | 168.753 ops/s | Pillow oracle |
| Serial CPU | 4.207938 ms | 5.340898 ms | 237.646 ops/s | One completed `Rotate`; CPU; terminal; no fallback |
| SIMD | 6.339741 ms | 7.909601 ms | 157.735 ops/s | One completed `Rotate`; SIMD vector path; 156,672 vector blocks, 3,072 scalar-tail items; terminal; no fallback |

Same-run ratios: CPU is **1.408× faster than Pillow**. SIMD is **0.935× Pillow**
(6.98% slower) and takes **1.507× the CPU latency**. This is a measured SIMD
miss, not a parity or fallback issue.

## Published x86 comparison

The current GitHub Docs snapshots use an AMD EPYC 9V74 runner, so their
latencies are kept separate from the local Ryzen measurements. The main
snapshot (`2026-10-19T09:12:13Z`) reports 2.961423 ms Pillow, 2.891609 ms CPU,
and 5.619340 ms SIMD for this workload. The Pillow-SIMD snapshot
(`2026-10-19T10:32:16Z`) reports 2.839064 ms Pillow-SIMD, 3.365272 ms Pillow,
2.538921 ms CPU, and 4.606283 ms SIMD. Within that Pillow-SIMD cohort, target
SIMD is **0.616× Pillow-SIMD latency** (1.622× slower). Both snapshots mark the
target parity and backend receipts as passing; the SIMD rows report positive
vector work. See [main benchmark snapshot](https://appunni-m.github.io/pillow-rs/assets/benchmark.json)
and [Pillow-SIMD benchmark snapshot](https://appunni-m.github.io/pillow-rs/assets/pillow-simd-benchmark.json).

## Hot-path finding and next experiment

`simd_nearest_rotate_native` currently advances the fixed-point affine
coordinates per pixel, performs scalar bounds checks and indexed pixel copies,
then packs each block into `u8x16`. For this one-degree angle, the CPU path
already computes in-bounds spans and proves contiguous RGB runs before copying
byte spans. A focused SIMD candidate should reuse that run-bound reasoning in a
Rotate-family kernel and benchmark the full public call against the unchanged
SIMD control. Preserve the scalar fixed-point fallback for short or unproven
runs, and use the same exact parity case plus existing angle/edge guards. The
receipt’s vector-block count proves selected vector-path execution under the
current producer contract; it does not establish that coordinate generation
or source gathers are vectorized.

## Shallow-angle RGB SIMD candidate and repeat — 2026-10-20

A narrow SIMD route now copies fixed-point-proven contiguous RGB source runs in
16-byte vectors, leaving short or unproven runs on the existing gather path.
The new route is limited to shallow positive-angle geometry. The shared run
bound helper is in `pillow-rs/src/ops/rotate/nearest.rs`; the focused SIMD
kernel is in `pillow-rs/src/compute/pool_simd/ops/adapters/rotate/nearest.rs`.

Both runs used the same 1024×768 RGB nearest 1° workload and end-to-end
`call;observe-result` boundary, with 5 warmups and 100 measured observations.
Each selected one workload and freshly passed its exact parity case for both
CPU and SIMD (2/2). The candidate SIMD receipt reports one completed Rotate,
actual `simd`, the `native-copy` path, 135,871 vector blocks and 185,360 scalar
tail bytes, terminal completion, and no fallback.

| Run | Pillow median / P95 | Serial CPU median / P95 | SIMD median / P95 | SIMD speedup vs same-run Pillow | Parity |
| --- | ---: | ---: | ---: | ---: | --- |
| `migration-benchmark-40075e768c4a436995032538adea5bcb` | 4.839 / 6.078 ms | 4.155 / 4.572 ms | 3.163 / 4.485 ms | 1.530× | 2/2 pass |
| `migration-benchmark-c647b4ca5653493db5da9dbf2d409cf5` | 5.955 / 8.152 ms | 4.120 / 5.238 ms | 3.134 / 4.134 ms | 1.901× | 2/2 pass |

SIMD latency was stable across the candidate runs (3.163 ms and 3.134 ms), as
was serial CPU (4.155 ms and 4.120 ms). The Pillow median moved from 4.839 ms
to 5.955 ms, so each speedup is paired within its own run. The candidate is
near the 2× sequencing checkpoint but does not demonstrate it consistently;
the repository's 5× SIMD target is farther away. Keep Rotate open. The older
current-source SIMD measurement was 6.340 ms, but that run was not interleaved
with the candidate builds, so the difference is directional evidence rather
than a controlled causal estimate. This still covers only one mode, size, and
angle.

The candidate extension SHA-256 was
`61ff5b53119feff37667c4130ede525b0318268da3362c2bb8eb8573817f6fb4`. The
shared run-bound helper SHA-256 was
`bb94924b797c1ad830cc2f31c67c0d56bcd8cf6b39ee5110aa62a71dfa05d0c8`; the SIMD
kernel SHA-256 was
`57b4156c57de5746407eee25a33b7d350c74751d5bb39f24cc094f571e1478a0`.

Selected evidence remains on the single reused x86 host. Run 1 report/raw
SHA-256: `01009e4e3fe2273f2c18ab042d06089f5836e372f2ca200e05ee85c7a80b7707` /
`51125c63987ef6f51d4496c7dde376fb7ae229452f7b062c14b6af066e1c65d2`; parity
report/attestation SHA-256: `cd8b1826cdec8e8d0d9dafb2bda45a872118330fd11b55b636307e92d3fb95d1` /
`ae03d6e9069fbee0f0cebd6f129978e7b54284023e4f4bf424f6f47eff8e6b7c`. Run 2
report/raw SHA-256: `a5989d81e9713bfa1d50e6a15a3616c9bd02cc29c988b9733a9f8436a62f9eef` /
`3284ff42e6c9f3bd293b34761b6f41650a6915f3dfc6bddccbb7c36bf9b21707`; parity
report/attestation SHA-256: `b9c5399473586c771c3d23dc97ad6f847d7c0a27ef02b3cd6298459e96655507` /
`39268087136f97acbd33e053f0a84113e44f5ca82289017fa3e9518c51dd63c0`. Both
reports use source revision `418d5f0cde51ca64a97cbd007ea731da72d68abf` with a dirty
worktree. No target tree or repository copy was made on the host.

## Reproducibility and artifact retention

The benchmark report remains on the existing x86 host at
`/tmp/rotate-rgb-nearest-current-scopefix2-x86-20261020.json` (SHA-256
`81b882dfdb5bbaec845a82d489af9c6d6a3bb6b8de4e161cada6fc3573d67abe`). Raw
samples remain there at
`/tmp/rotate-rgb-nearest-current-scopefix2-raw-x86-20261020.jsonl` (SHA-256
`ee89f6643b8c562c6ee9a65f78fb21329c58763c28378724b3e4beaac8198fee`). The
compressed parity report remains at
`/tmp/rotate-rgb-nearest-current-scopefix2-parity-x86-20261020.json.gz` (SHA-256
`64ebf7a04e487e0f09dcd0eb59c2e4c783c31cfce534c6f32f2a088c9ee854cd`). The
benchmark attestation remains at
`/tmp/rotate-rgb-nearest-current-scopefix2-attestation-x86-20261020.json` (SHA-256
`502103778bf44ad47b6abf0d709e6a6e92b47ab6484d174ce8a247bfe4167fa8`), and the
parity attestation at
`/tmp/rotate-rgb-nearest-current-scopefix2-parity-attestation-x86-20261020.json`
(SHA-256 `e678984f43d038890c13904cc874e2081cedb9ad6fa0809ab21a28dd680f6821`).
No repository or target directory was copied to the host; benchmark artifacts
remain on that host instead of being copied back to this checkout.

An earlier partial-receipt report was excluded. Its untimed proof omitted the
selected timing scope, ran an extra intermediate materialization, and let a
diagnostics-only receipt replace the `Rotate` receipt. The proof command and
attestation now carry the selected boundary/steps, and receipts without an
operation count, operation telemetry, or backend identity are diagnostics only.
Focused harness tests passed 4/4 before this selected run.

This evidence has been recorded in the local per-operation results matrix and
Kata `pillow-rs#n0vj`. It remains one selected workload; the 33-item commit
batch is still open, and no commit or push was made.

## Fixed-step reciprocal-divider trial — 2026-10-20 (rejected)

The follow-up hypothesis replaced the per-run variable `i128` divisions in the
contiguous-run bound helper with precomputed exact 16.16 reciprocal dividers.
The reciprocal quotient was corrected with one multiply/compare, and focused
unit tests exhaustively checked the supported numerators plus the prior run
bound formula. The trial touched only the shared Rotate helper, its re-export,
and the shallow-angle RGB SIMD kernel; those source files were restored after
the measurements showed no repeatable gain.

All five runs selected only the frozen RGB 1024×768 nearest 1° Rotate workload,
measured `call;observe-result`, and used 5 warmups plus 20×5 observations at
concurrency one. Each report includes Pillow, serial CPU, and SIMD; each SIMD
receipt is actual SIMD, terminal-complete, `native-copy`, with 135,871 vector
blocks, 185,360 scalar-tail bytes, and no fallback.

| Run | Variant | Pillow median | CPU median | SIMD median | SIMD speedup vs same-run Pillow | Exact parity |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| `migration-benchmark-8957e68eec2c4dfe8100c7698d127f63` | A1 control | 5.444 ms | 3.925 ms | 2.849 ms | 1.911× | 2/2 fresh |
| `migration-benchmark-cba4bb60b63b4da0b8fd355b3d09443e` | B1 divider | 5.961 ms | 4.202 ms | 3.138 ms | 1.900× | 2/2 fresh |
| `migration-benchmark-b464a0b21c7a442da8248d074ad39ede` | B2 divider | 6.395 ms | 4.342 ms | 2.930 ms | 2.182× | 2/2 reused B1 |
| `migration-benchmark-99f4ee002eaf4f578da14881076d08b9` | A2 restored control | 5.776 ms | 4.385 ms | 3.826 ms | 1.509× | 2/2 fresh |
| `migration-benchmark-6c060cd64a2d4a109aa8802c2a23edee` | A2 repeat | 5.995 ms | 4.723 ms | 3.636 ms | 1.649× | 2/2 reused A2 |

B1 was 10.2% slower than A1 in SIMD latency; B2 was 2.9% slower. Their paired
speedups (1.900× and 2.182×) cross the 2× sequencing gate only once. CPU
latency also rose from 3.925 ms in A1 to 4.202/4.342 ms in B1/B2. The restored
A2 controls were substantially slower than A1 in every subject, including
Pillow, so the post-candidate bracket is noisy and does not support a causal
gain claim. The candidate is rejected and its code is not retained.

Fresh Cargo witnesses for A1, B1, and A2 report stable source snapshots on
dirty HEAD `418d5f0cde51ca64a97cbd007ea731da72d68abf`. A1/A2 raw artifact and
loaded extension hashes match at
`7b7433a32cd1a4c49dd4019d328b673f5cf960c16a6cef2133a6123ce118a066`; B1/B2
match at
`4fe595c815cba72844f5fe75e204c75ebd7442f314daaf5ce293c60ff70e6dd7`.
Parity reuse from A1 was rejected by the runtime-identity check after the A2
rematerialization, so A2 used fresh parity; B2 and A2 repeat reused only their
own matching passing receipts. Full report, raw-sample, parity, attestation,
and build-witness hashes are in the appended per-operation matrix rows. The
raw benchmark artifacts remain on the single existing x86 host under
`/tmp/rotate-recip-*-20261020*`; no checkout or target directory was copied.
