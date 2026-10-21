# GetColors I selected CPU baseline history

This page keeps the selected high-cardinality I-mode GetColors measurements in
time order. The initial tracking measurement below was slower than Pillow. The
latest source-build-bound result, recorded at the end, meets the serial CPU
median target for this one workload and is now the active strict baseline.

## Frozen scope

- Workload: `pipeline-chain.terminal-read.getcolors.high-cardinality-signed-run-i-128x64`
- Exact parity case: `PIL.Image.Image.getcolors.nuanced.i-high-cardinality-signed-run-fallback-128x64`
- Timed step: `call-fallback-4097`, including materialization of all 4,097 Python result rows
- Input: mode `I`, 128×64, 8,192 signed pixels
- Sampling: 5 warmups, 20 iterations × 5 samples, warm cache, concurrency 1 (100 observations)
- Reference: Pillow 12.2.0, CPython 3.12.13
- Target: `python-cpu`, release, source revision `418d5f0cde51ca64a97cbd007ea731da72d68abf`, dirty worktree
- Stable runner ID: `lazys-macbook-pro-arm64`; the same value was used for parity and timing because it participates in runtime fingerprints

The exact standalone parity comparison passed 1/1. The target receipt proves
one completed `GetColors` CPU operation, terminal completion, and no fallback.
The measured call includes Python result construction; runtime host-allocation
counters do not account for those Python objects.

## Current measurement

| Subject | Median call | P95 | Pillow / subject |
| --- | ---: | ---: | ---: |
| Pillow | 178.750 µs | 362.416 µs | 1.00000× |
| Serial CPU | 187.938 µs | 359.916 µs | 0.95111× |

Serial CPU latency is 5.14% higher than Pillow on this selected call. This is a
single-run prioritization point, not an A–B–B–A optimization claim. It is 1.28%
slower than the previous witness-bound CPU median of 185.5625 µs; that change
is not attributed to code because no implementation changed and the runs were
not interleaved.

The benchmark report is
[`reports/runner-id-bound-final-20261019/benchmark-result.json`](reports/runner-id-bound-final-20261019/benchmark-result.json), run
`migration-benchmark-ba2964a454aa499abed91871bd101ffc`, SHA-256
`e178edf9d0fd8aa3702ecd04e5cdac9814e4b8182d65f02f76ccb6cd13ec7cdd`. Its
selected parity report is
[`reports/runner-id-bound-20261019/parity-result.json`](reports/runner-id-bound-20261019/parity-result.json), run
`migration-parity-eaa564da5eb8439cab0b026be1dea2b3`, SHA-256
`b011a740e5e21d632870f4a5782b1e3f2a70251cbbee51d7c3ecddbeba5b47b7`.

## Proof and baseline status

The strict `direct-operation-parity-link@2` proof is
[`direct-operation-link-runner-id-v2.json`](direct-operation-link-runner-id-v2.json),
SHA-256 `cb53ba8a9a7032c291ec2548bfe81a7bde5fe05638b5587fc10e34505e5e0ae5`.
PerfDB verified parity, producer equivalence, runtime equivalence, source-build
binding, actual backend execution, and performance admission. The selected
build witness is [`build-witness-final.json`](build-witness-final.json), SHA-256
`913de8c7fceaa10c8662368fa873be720e8f065431ff9625d64b368c24644ab4`; its
artifact and the loaded `_core` both have SHA-256
`5a42dd70babf1242df00a534b250b57e640734789e09bceca178a0532d304ba3`.

PerfDB promoted workload
`pipeline-chain.terminal-read.getcolors.high-cardinality-signed-run-i-128x64`
to `getcolors-i-high-cardinality-cpu-arm64-tracking-20261019`. It is a tracking
baseline because the target is unmet. The generated DuckDB is in the separate
`pillow-perfdb` project and is not committed. The CLI rejects checking the
active baseline artifact against itself; a future compatible report will
exercise the regression comparison without repeating this measurement now.

The previous benchmark attempt failed its producer-output reservation before
timing. Another attempt failed the reuse preflight because the newly supplied
stable runner ID changed the runtime fingerprint captured by the old parity.
The repair was one selected parity case with the stable ID followed by one
selected benchmark workload. No full parity or benchmark campaign was run.

## Remaining scope

Only this workload, mode, size, and serial CPU profile were measured. SIMD,
Parallel CPU, GPU, other modes, other sizes, and other operations remain
unmeasured here. The operation remains open until every applicable target is
demonstrated. No coverage ran, and the commit/push batch remains 0/33.

## Direct Python list construction candidate — 2026-10-19

For the same high-cardinality I-mode call, the first exact-size-list attempt
materialized a Rust vector of Python tuples before constructing the final list.
That selected run measured 189.896 µs against same-run Pillow at 175.250 µs,
so the extra vector did not help and was rejected. The follow-up converts each
core result lazily through a small `IntoPyObject` adapter passed directly to
`PyList::new`, avoiding the intermediate vector.

The first direct-iterator run measured 175.1665 µs against Pillow at 173.750 µs
(0.99191× Pillow/CPU). A stable-runner repeat measured 168.729 µs against
175.021 µs Pillow (1.03729×), and the selected mode guard passed all 8 cases.
For a same-host comparison, the original append implementation measured
183.354 µs against 177.5625 µs Pillow; the direct-list candidate measured
170.1045 µs against 175.479 µs Pillow (1.03160×), 7.23% below the append
control median. Both candidate receipts prove actual CPU execution, one
completed operation, terminal completion and no fallback. The paired candidate
therefore clears the CPU latency target for this one workload, while the noisy
first P95 and the single workload limit the claim.

The direct candidate and selected parity reports are imported into generated
DuckDB history. A newly captured build witness hashes to
`916a51ed984d8077859cfc52e9d805cf4d0afede6921b9ad4d78f8f10c63fba1`, while the
extension used by the benchmark hashes to
`35979d251e4e49d8cee6b1ac8bc6608fd84dce8c5dca4426345d30d8294b0c25`. This
identity mismatch prevents strict source-build proof, so no strict regression
baseline was promoted. One earlier parity-reuse attempt failed oracle identity
preflight before timing and is excluded. Only the selected workload and eight
focused parity cases ran; no full benchmark or parity campaign ran.

## Witness-bound strict CPU baseline — 2026-10-19

The final capture uses a fresh Cargo build witness, and both selected parity
and benchmark attestations record the exact emitted extension hash. The strict
direct-operation link passes parity, runtime and producer equivalence, actual
backend execution, source-build binding, and performance admission.

| Subject | Median call | P95 | Samples |
| --- | ---: | ---: | ---: |
| Pillow 12.2.0 | 174.792 µs | 185.208 µs | 100 |
| Serial CPU | 170.292 µs | 325.041 µs | 100 |

The selected CPU median is 1.0264× Pillow, meeting the CPU≤Pillow median target
for this call. The P95 is substantially higher than Pillow, so this result does
not establish a tail-latency win. The timed `call-fallback-4097` step includes
the full Python list materialization. Runtime receipts show one completed
`GetColors` CPU operation, terminal completion, and no fallback.

The frozen nine-case mode/format guard passed 9/9 on this exact extension. A
separate one-case parity run with stable runner ID
`lazys-macbook-pro-arm64` is the parity input to the strict direct-operation
link. The captured extension SHA-256 is
`b904c05e3933800ad1aa6cf0ac49cc82e82f5ae548972f2a3b3b2f0dbddc590e`; the build
witness JSON SHA-256 is
`2ee87c10138a86876f0f2c9bafbfb6ad0caa89a65493d639d942c2fae7e1bc9a`. The
benchmark run is `migration-benchmark-a00ba850e7d04e868df9d813d839d3ea`, report
SHA-256 `b3cef3bb3aacd13b212b8eb84ab8d6c7bc8272e90df69aeb89c7d8fb4406cd37`;
the direct-operation link SHA-256 is
`3f1718deb9e69045f64c65cb4ca44323e467c893f2bffab686cc5aadbbf99d79`.

These reports promoted one row to the generated DuckDB baseline
`getcolors-i-high-cardinality-cpu-arm64-strict-20261019` (baseline ID prefix
`8e337dc3e0a3`). The database remains generated and uncommitted. Checking the
older 187.938 µs report against this baseline returns `not proven`: its parity
input corpus hash is `0ccae3…`, while the current corpus is `ae4e8e…`. The
benchmark-input hash is unchanged, but the strict workload signature includes
the full parity-input file hash, so that historical row cannot serve as a
compatible regression candidate. This check was not counted as a passing
regression demonstration.

Retained evidence: [fresh build witness](reports/witness-bound-strict-cpu-20261019/source-build/build-witness.json), [nine-case mode guards](reports/witness-bound-strict-cpu-20261019/mode-guards/parity-result.json), [paired parity](reports/witness-bound-strict-cpu-20261019/paired-run/parity-result.json), [benchmark result](reports/witness-bound-strict-cpu-20261019/paired-run/benchmark-result.json), [raw samples](reports/witness-bound-strict-cpu-20261019/paired-run/raw-samples.json), and [verified direct-operation link](reports/witness-bound-strict-cpu-20261019/paired-run/direct-operation-link.json).

Only this I-mode workload, size, and serial CPU profile are measured. Other
GetColors workloads, modes, architectures, SIMD, Parallel CPU, and GPU remain
open. No full benchmark or parity campaign and no coverage run occurred; the
commit/push batch remains 0/33.
