# AVX2 RGBA transpose reverse-row A–B–B–A — no-go (x86_64, 2026-10-19)

## Decision

Do not retain the candidate. Loading the full tile rows in reverse for the
`reverse_y` orientation removed the per-vector lane reversal, and the candidate
passed both selected parity cases. Its SIMD median was about 1.5–2.5% lower
than the adjacent control runs, but the Pillow oracle also shifted: the
same-report SIMD/Pillow ratio moved from 1.036× to 1.135× in A1→B1, then was
nearly unchanged at 1.024× versus 1.025× in B2→A2. The measured public pipeline
still ran slower than Pillow in every report. This does not establish a stable,
material end-to-end advantage, so the experiment is a no-go for retention.

The current kernel was restored to control source SHA-256
`a66fb6b97212b0504bb355d2501c84113c1e787b08f58e0f28507722937bf3ff`.
The candidate source SHA-256 was
`45929abe3c8cb94fec55be887fd9e4bbec553c7b3fd0a24106a0789a51f36cc0`.
Both files are kept here for inspection.

## Frozen scope

- Workload: `pipeline-matrix.transpose-tiled.rgba-772x768-transpose-mirror.operation-materialized`
- Timed boundary: `transpose-0;transpose-1;materialize`
- Backend: `python-simd` only; actual execution receipt required AVX2.
- Warm release; 5 warmups; 20 iterations × 5 samples; concurrency 1; 100 latency observations per subject.
- Mode and size: RGBA, 772×768.
- Main parity case: `PIL.Image.Image.transpose.nuanced.tiled-aligned-rgba-772x768-methods-5-0`.
- Odd-size parity guard: `PIL.Image.Image.transpose.nuanced.tiled-odd-rgba-1025x1023-methods-5-0`.
- Manifest SHA-256: `ff0d8fa434e60ed844d9d3606396919e28e8b59bdcc590c80d0d73564ec5d885`.
- Selected benchmark input SHA-256: `9f0a8fdbef7c4eb921ac9897a7ddd1810c213df4fe962fb8e7d9870f541202a8`.
- Runner: AMD Ryzen 5 2600X, Linux x86_64; benchmark Python 3.12.3 and Pillow 12.2.0.

Each benchmark report selected and measured one workload. No full benchmark or
parity campaign was run. All three distinct parity reports selected the two
cases above and passed 2/2 with zero diffs; B2 reused the B1 proof for the same
candidate extension.

## A–B–B–A results

Latency is the median for the full materialized public pipeline, in
milliseconds. `SIMD/Pillow` compares each target with its Pillow result in the
same report; values above 1 are slower than Pillow.

| Run | Treatment | Pillow ms | SIMD ms | SIMD/Pillow | Parity |
| --- | --- | ---: | ---: | ---: | --- |
| A1 | Control | 2.522149 | 2.613485 | 1.0362× | 2/2 pass |
| B1 | Reverse-row candidate | 2.268720 | 2.574697 | 1.1349× | 2/2 pass |
| B2 | Same candidate binary | 2.458910 | 2.517806 | 1.0240× | Reused B1 proof |
| A2 | Control | 2.518424 | 2.582030 | 1.0253× | 2/2 pass |

The B1 target median was 1.48% below A1, but its Pillow median was 10.05%
lower, making the relative ratio worse. B2's target median was 2.49% below A2,
while its same-report ratio improved by only about 0.13%. Do not interpret the
small target-only changes as a demonstrated public-call gain.

## Execution and source identity

All four receipts report `actual_backend=simd`, no fallback, and the
`avx2-transpose` path with 73,728 vector blocks and 12,288 scalar-tail items.
The composed second transpose used `native-copy`. The report records one
2,371,584-byte host allocation and two materialized buffers totaling
4,743,168 bytes. These execution and resource counts were unchanged by the
candidate.

All treatments ran on the x86 runner at HEAD `418d5f0cd`. The A/B change was
limited to the AVX2 kernel source and rebuilt extension: A1/A2 extension SHA
`db63fd7f293642e67845813a3db4e76ebaaddf559bc728d44e5907df0f474248`; B1/B2
extension SHA `c2bc37080b92140489de0a97b67d37aff8dc6cb0dfd0ed493bbe4d25e2cc543f`.
The selected kernel, backend module, parity case objects, and benchmark inputs
were checked against the local checkout. The local and runner worktrees were
not otherwise identical: the adapter facade, compute module, Python binding,
and benchmark producer differed. Treat this as a controlled within-runner
kernel A–B–B–A result, not as a full-source reproduction of the local worktree.

The runner's parity virtual environment lacked both `pip` and `ensurepip`, so
`make build-parity` could not provision the extension. No dependencies were
installed and no target directory was copied. The locked release Rust extension
was built directly and installed in place for these focused runs; all four
builds completed with the existing missing-documentation warnings.

## Run identities and retained artifacts

| Run | Benchmark run ID | Benchmark report SHA-256 | Parity run ID | Full parity report SHA-256 |
| --- | --- | --- | --- | --- |
| A1 | `migration-benchmark-f8832766077e452db85bf2acdf06d8ef` | `6473c4b39fbf99c8d3023e96c1f8a472d07e0bca9b97209df2cce992e57ce29a` | `migration-parity-c844e1d47b354010b9c4b5c396dc9039` | `7c9e60c33dc15532b6032639093c068dfa7e6934721227c29cd2e867f5cac202` |
| B1 | `migration-benchmark-70210411ee4a474491b13100c02a07e9` | `353c0abccc1a34d07467da512332ce65307cb0785cff05cc4bbe0e88d47abf2f` | `migration-parity-b3acf95ca09142b7a176ab860e5587f5` | `e0b7c9e8000e9783d01d77454c440427d25a57e66beadf8c192093647decbe33` |
| B2 | `migration-benchmark-4199bf68e443433ea0631a4d5068f9ee` | `22f64a1c7c340e1cd1ef56be058f6750ff649199fdbbe58a536fe6b304e3fc86` | Reused B1 | Reused B1 |
| A2 | `migration-benchmark-b2efafdd7c854f86afda8a9dedee9557` | `d51e033bbc1b8b06f0fa3378946769bc1c8df5f67e93325a8110be1e515c446f` | `migration-parity-0449c35d3d9943d99dc1c0f0f86a8ee6` | `6e47c08ced60758d348e661306d4ea06178586949f794c86c589c4cd2bd3a38e` |

Benchmark reports, raw samples, producer files, and execution-attestation
children are retained under `A1/`, `B1/`, `B2/`, and `A2/`. Compact parity
attestations and their invocation children are under each run's `parity/`
directory. Each full parity report was 35,024,231 bytes and remains on the
runner at `build/migration-parity/transpose-avx2-reverse-y-abba-20261019/`;
only its run ID and SHA-256 were copied here. `SHA256SUMS` covers the retained
local files.
