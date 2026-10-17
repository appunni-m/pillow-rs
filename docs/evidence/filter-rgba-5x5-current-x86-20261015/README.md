# Current-source native x86 Filter5x5 baseline — 2026-10-15

## Scope and identity

This diagnostic selected two full-workflow RGBA cases on the native x86 runner:

- `pipeline-chain.convolution.native.rgba-5x5-uniform.1024x768`
- `pipeline-chain.convolution.native.rgba-5x5-nearuniform-tail.1024x768`
- parity cases `PIL.Image.Image.filter.nuanced.performance-rgba-kernel-5x5-uniform-1024x768` and `PIL.Image.Image.filter.nuanced.performance-rgba-kernel-5x5-nearuniform-tail-1024x768`

Both create a 1024×768 RGBA image, construct a 5×5 all-ones `Kernel` with
scale 25, call `Image.filter`, and materialize with `tobytes`. The near-uniform
case changes the final pixel's red byte from 17 to 18, forcing a full uniform
scan miss. Parity observes both the filtered image and terminal bytes. The
measurement uses `whole_workflow`, warm cache, five warmups, 20 iterations ×
five samples, concurrency one, and CPU only.

The host is an AMD Ryzen 5 2600X (`ryzen-2600x-affinity-4`), Linux x86_64,
CPython 3.12.11, Pillow 12.2.0, and Rust 1.96.1. The source checkout is dirty
at `f2b63f559e91022a85cb43f7264bdf8b586a448d`. The benchmark child receipt
proves it loaded
`/home/appunni/pillow-rs-getchannel-x86-baseline-20261015/pillow-rs-py/python/pillow_rs/_core.abi3.so`
with SHA-256 `0a5b618df3e87b75d65c5ead9860bb9205d42b30251d28bf15d688bf7b1faa8f`.
The child executed actual serial CPU `Filter5x5`, terminal materialization
completed, no fallback was recorded, and vector-block count was zero. The
near-uniform workflow also executed its `PutPixel` setup operation on CPU.

## Result

| Input | Pillow median / P95 | Serial CPU median / P95 | CPU speedup vs Pillow | CPU throughput |
| --- | ---: | ---: | ---: | ---: |
| Uniform | 48.252 / 60.651 ms | 196.695 / 213.493 ms | 0.245× | 5.084 ops/s |
| Last-pixel mismatch | 48.051 / 52.513 ms | 191.011 / 214.164 ms | 0.252× | 5.235 ops/s |

The pre-trial uniform RGBA path is about four times slower than Pillow. The
read-only Kata audit `pillow-rs#gz43` recommended a narrowly gated trial: scan
large native RGBA input for an actually uniform tuple, evaluate the center
tuple in the existing f32 tap and row order, and skip convolution only when all
four output bytes equal that tuple. The near-uniform run is the full-scan-miss
guard. The subsequent candidate measurements are recorded below; these baseline
rows remain diagnostic and do not receive clean-matrix credit.

## Narrow serial-CPU identity trial

The local candidate in `pillow-rs/src/compute/pool_cpu/ops/filter.rs` scans
large native RGBA images, computes the uniform tuple's output with the same
normalized coefficients and arithmetic order, and returns an independent copy
only when every output byte equals the input tuple. A last-pixel mismatch takes
the regular convolution path. The shortcut is excluded from the opt-in
Parallel CPU build. SIMD has a separate implementation and was not measured by
this CPU trial. The candidate extension SHA-256 was
`1a8e15be2b7c81b67ec217409b3e290758376469b1ed927751624d34c48ceb4e`.

| Workload | Control A1 CPU / Pillow | Candidate B1 CPU / Pillow | Candidate B2 CPU / Pillow | Control A2 CPU / Pillow |
| --- | ---: | ---: | ---: | ---: |
| Uniform | 196.695 / 48.252 ms | 5.806 / 50.105 ms | not repeated | 183.787 / 52.622 ms |
| Last-pixel mismatch | 191.011 / 48.051 ms | 219.066 / 48.563 ms | 179.900 / 49.822 ms | 181.814 / 52.940 ms |

For the uniform input, candidate B1 was 8.63× Pillow and about 32.8× the mean
of the two bracket controls. Both exact-parity cases passed in B1. The
near-uniform B1 timing was a 20% outlier above A2, so only that workload was
repeated with the candidate and its exact one-case parity proof. B2 measured
179.900 ms median / 197.660 ms P95; the A2 control was 181.814 / 196.759 ms.
The A1 and A2 controls measured 191.011 and 181.814 ms median. B2 is inside
that control range, so the first scan-miss result is not a repeatable
regression. The full-scan miss still runs Filter5x5 and remains roughly four
times slower than Pillow.

The candidate and both controls completed terminal `tobytes` materialization.
Receipts report actual serial CPU `Filter5x5`, no fallback, and zero vector
blocks. The near-uniform workflow also runs `PutPixel`. Candidate B1 parity
passed 2/2, candidate B2 parity passed 1/1, and A2 reused the exact A1 parity
proof after confirming the loaded control extension hash
`0a5b618df3e87b75d65c5ead9860bb9205d42b30251d28bf15d688bf7b1faa8f`.
All benchmark and parity result schema validators passed.

This supports a promising, narrowly scoped uniform-input CPU fast path; it does
not close Filter5x5 or its per-mode matrix row. Other RGBA content, sizes,
modes, kernel parameters, SIMD, Parallel CPU, GPU, and Pillow-SIMD remain
unmeasured. These dirty-tree trials are not clean-matrix credit.

## Narrowed r8 guard

The candidate predicate was narrowed to exact normalized all-ones coefficients,
scale 25, offset 0, native RGBA mode, and an image larger than the existing
small-uniform threshold. This avoids scanning unrelated kernels. The selected
1024×768 uniform and last-pixel-mismatch workflows both passed exact parity
(2/2). Their median whole-workflow times were 46.680 ms Pillow / 5.142 ms CPU
(9.08×) and 46.714 ms Pillow / 177.660 ms CPU (0.263×), respectively. The
candidate extension SHA-256 was
`8b9fb3b306a0bff54bec55f034fd422266a7999add0427fbd2ad54a5f6ddda9f`; receipts
show actual CPU `Filter5x5`, completed terminal materialization, no fallback,
and zero vector blocks. The full-scan miss remains about 3.8 times slower than
Pillow, so this does not close Filter5x5.

The Make target built the release extension but stopped before benchmarking:
its remote sparse checkout did not contain
`scripts/test_migration_benchmark_selection.py`, so unittest discovery found
zero tests and returned exit 5. I then invoked the same benchmark runner with
the frozen two workload IDs and manifest directly; selected parity passed,
and the benchmark and parity schema validators passed. This is a sparse-checkout
layout limitation, not a product or test defect. The direct run is
`migration-benchmark-816154aea51b4049b7b9a2f4135db5e0`, parity gate
`migration-parity-benchmark-gate-af519f55226b4840bef7c53b0b33982c`; benchmark
report SHA-256 is
`d2448d551c6e08b804cd28970f1504129daf5bfd9d24f4ac5f8c65148fa2c4a0`, and the
compressed parity result SHA-256 is
`b7b5a6ba19de1fa784a4804ff741da33200c9661c71c79ab5ef7d5b6aa2e5830`. Four
subject rows bring the local-run ledger to 7,712 data rows.

## Shared-helper performance guard

To check the low-level helper extraction, I ran only
`pipeline-op.filter5x5.matrix-32x24` and its exact parity case
`PIL.Image.Image.filter.nuanced.performance-rgb-kernel-5x5-materialized-32x24`.
The workflow is RGB 32×24, `Image.filter`, and terminal `tobytes`, measured
warm with five warmups and 20×5 samples on the same native x86 runner. Every
run passed parity 1/1 and reported actual CPU `Filter5x5`, terminal completion,
no fallback, and zero vector blocks.

| Source variant | Run | Pillow median | CPU median | CPU P95 |
| --- | --- | ---: | ---: | ---: |
| Baseline A1 | `migration-benchmark-086581310bd34fe2a40d9d0dbfc0e9f5` | 0.123800 ms | 0.098495 ms | 0.107770 ms |
| Shared helper with four-byte copy B1 | `migration-benchmark-4cefbf8481414e28ae6486f7b6005a9c` | 0.124425 ms | 0.105915 ms | 0.120000 ms |
| Shared helper with four-byte copy B2 | `migration-benchmark-58d37711628846cd853d12c7b38874e0` | 0.123655 ms | 0.102705 ms | 0.170751 ms |
| Baseline A2 | `migration-benchmark-49bf9b9032504448baa1c2511b546464` | 0.123520 ms | 0.098085 ms | 0.121391 ms |
| Shared helper without copy B3 | `migration-benchmark-ef65277ddb7c47338454e3e071e956ce` | 0.142186 ms | 0.099245 ms | 0.132501 ms |

The copied-input helper candidate measured 4.7–7.5% slower than its adjacent
baseline medians. I changed the helper to borrow the original sample bytes,
retaining a single shared pixel kernel without copying the tuple. B3 then
measured within 1% of the mean of A1/A2 on CPU median; the B3 Pillow baseline
and tail latencies varied more, so this single B3 run only supports “no
material median regression” for this small RGB case. It does not establish a
gain or cover other modes and image sizes. A1/A2 used the committed filter
source SHA-256 `bd2911ceeb609c42d9d53554d1bf323789bd9495184638e5f5f21086c8b243bc`
and extension SHA-256
`0a5b618df3e87b75d65c5ead9860bb9205d42b30251d28bf15d688bf7b1faa8f`. B1/B2
used the tuple-copy candidate source SHA-256
`b5a5bf544b5297d44f98f4289a4212b481c67f71000d3e6e60e784ecc30487eb` and
extension SHA-256
`8b9fb3b306a0bff54bec55f034fd422266a7999add0427fbd2ad54a5f6ddda9f`. The
final helper source SHA-256 is
`a636ad89373952d4c03d00de588097e2802c421358f3e2ebc210cf93e810b62e`; the B3
extension SHA-256 is
`1cf72cda41359eeedc79f018aa1f95a833e400c189bdec25aaff3f5e6841de75`. Baseline
and candidate extension identities are recorded in the raw reports and
attestations under `candidate-r8/helper-guard/`. Fourteen subject rows (the
four-row r8 run and these five two-subject helper runs) bring the local-run
ledger to 7,722 data rows.

Runs: A1 `migration-benchmark-d0181dd2091f4ad182b8f7ced6562537`; B1
`migration-benchmark-39b95c78b67444e583d63615401602ef`; B2
`migration-benchmark-fbb0dd7d49ec414fbb6e0980a1bc10f5`; A2
`migration-benchmark-771c46e767064ce18608ac9c0f80a65d`. Candidate reports,
compressed parity proofs, raw samples, execution and producer attestations are
retained in `candidate-r3/` and `candidate-r6/`. A2's benchmark report,
samples, and attestations are retained with the `r7-control-a2` suffix. The
33 MB expanded A2 parity result was not copied: A2 reused A1's hash-matched,
two-case parity proof.

Key artifact checksums:

- B1 benchmark report: SHA-256 `aefc20166dd81a99a3d2bbe0e054afb5805f688e847e5845ca54dfab612a6878`; parity proof: `a46e1b8246837448ca9dae24e6071487f05e3be7ebd2c88728661e80de1b7689`.
- B2 benchmark report: SHA-256 `b2056f18c65b57d9d2d47bc5a0ad1ef83d01143b63b698c38940773079738a89`; parity proof: `ffe201c995bd3695a52b5e12344d9503fb793b1e8ac0e0cdba69c3f88758c86d`.
- A2 benchmark report: SHA-256 `33159cbc16625a297002ed48f420811e76767767c92da46320c407878b514979`; execution attestation: `ed894a24d5da20ad72b0896c1533dc5bac75200ea2029b215c405b19bb5554df`; raw samples: `71b84cac9cc91e856238ab463b31b94d2b24483fd2a1152f2fa949750b990929`.

The original benchmark row whose ID ends in `invert` had only a
`successful_execution` correctness gate and did not execute an invert step.
It was not used as parity proof. The first focused run (`r1`) observed the
filtered image and passed exact parity; `r2` supersedes it with both image and
byte observations plus the near-uniform guard. The initial custom slice draft
was rejected because it observed only `tobytes` while declaring `filter` as
the primary operation. The manifest validator behaved correctly; no product
or test defect was found.

## Evidence files

Authoritative selected run: `migration-benchmark-d0181dd2091f4ad182b8f7ced6562537`.
Its parity gate is `migration-parity-benchmark-gate-5a73f7be165847d7810c45163ee8eabe`.

- `benchmark-result-r2.json` — SHA-256 `ec560b4bb7c7871c918c1b0f8eac36b8dcae0b22d93511836aaddcc73e493932`
- `parity-result-r2.json.gz` — SHA-256 `1aedb61ee8f346ed4a84d9af9a28eee242f167df854f8df83646a2b52c85585c`
- `raw-samples-r2.json` — SHA-256 `d120e60bd189f7d9fba5713f062471e473d61e5ece60980ad3d6a645c1ff1a73`
- `attestations-r2.tar.gz` — SHA-256 `349b27e5bb1fbac2a578a1c72ae0263ad0e4ab35da3fec55f6aa02ec79770596`
- `input-slice.tar.gz` — SHA-256 `ab9921a813b09c30f0ef259e3d37110579e04f34e78a8efb0a94e9b2207e2807`

The corrected slice has manifest SHA-256
`40bb4ee333f797cdf73fb4efcb276331fd009489a4c856c6f1fb4a32a94538a3`, parity
input SHA-256 `1f2277bbdbb2c0786bc9792d4c0f429be9549e9ba7518219751238f6598ff42e`,
and benchmark input SHA-256
`1313949032c0c85a8b7d3d3f2d0b72dd3ebdb14610d202729b9adbdf18c67bc5`.

Four diagnostic subject rows were appended to
`performance-optimization-local-runs.csv` (7,698 data rows). No full campaign,
coverage, commit, or push ran. The native runner worktree and all prior files
were left in place; no `target/` directory or parity corpus was transferred.

### Candidate r9: rolling integer rows for large RGBA Filter5x5

The r9 candidate replaces the scalar 25-tap pass after a full-raster identity
scan miss with five ring-buffered horizontal channel-sum rows. It is guarded
to large native RGBA images, explicit mode `None`/`RGBA`, the all-ones 5×5
kernel with scale 25 and offset 0, and serial CPU builds. Its source SHA-256
is `dc6297e356471217fdb37b8b52b6a43c99f7b47640eceb2fb95ac7775add8dbf`; the
loaded extension SHA-256 is
`6282923bed4284d8118f4d52260bf036420e485b1791e1b6cb3908886a293d7e`.

On two warm 1024×768 RGBA workflows, exact Pillow 12.2.0 parity passed 2/2 in
B1 and the identical B2 candidate reused that hash-matched parity gate. Both
workflows include setup, Filter5x5, and terminal `tobytes`; each used five
warmups, 20×5 observations, and concurrency one.

| Workload | Run | Pillow median | CPU median | CPU/Pillow |
| --- | --- | ---: | ---: | ---: |
| Uniform | B1 | 46.446 ms | 5.330 ms | 8.71× |
| Uniform | B2 | 46.664 ms | 5.256 ms | 8.88× |
| Final-pixel nearuniform miss | B1 | 46.826 ms | 14.388 ms | 3.25× |
| Final-pixel nearuniform miss | B2 | 48.206 ms | 14.470 ms | 3.33× |

Receipts prove actual serial CPU `Filter5x5`, completed terminal materialization,
no fallback, and zero vector blocks. The timer includes output and scratch
allocation, but allocation telemetry reports zero allocated host objects and
bytes despite the code's output copy and five row buffers; the counters
undercount and are documented in the candidate evidence. This is scoped
candidate evidence only: SIMD, Parallel CPU, GPU, other modes, kernels, sizes,
and data distributions remain open. Full evidence is in
[`candidate-r9/`](candidate-r9/README.md). The local-run ledger now has 7,730
data rows. No full campaign, coverage, commit, or push ran.
