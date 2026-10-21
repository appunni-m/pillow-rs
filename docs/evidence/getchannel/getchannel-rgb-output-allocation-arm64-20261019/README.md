# GetChannel RGB output-allocation trial — ARM64 — 2026-10-19

## Scope

This is an uncommitted candidate experiment for the public
`PIL.Image.Image.getchannel` operation. It tests whether avoiding a zero-fill
pass before the RGB SIMD gather improves the complete call. It is not a new
baseline: the measurements are mixed and do not establish a stable full-call
gain.

- Workload: `pil-image-image.getchannel.materialized.rgb-1024x768`.
- Full-size parity case: `PIL.Image.Image.getchannel.nuanced.performance-rgb-1024x768`.
- Boundary: warm `call;observe-result`, including materialization; 5 warmups,
  20 iterations × 5 samples, concurrency 1.
- Host: `Lazys-MacBook-Pro.local`, macOS 15.7.7 ARM64, CPython 3.12.13.
- Source revision: `418d5f0cde51ca64a97cbd007ea731da72d68abf`, dirty worktree.
- Build: `make build-parity`, release, default features, Parallel CPU disabled.
- Candidate extension SHA-256:
  `466b6ca4d1391a5bb7332b882b89a16c627d3bae2134bef89c17dc4bfe03ba20`.

The earlier control is in
[`rgb-current-source-cpu-simd-arm64-20261019`](../rgb-current-source-cpu-simd-arm64-20261019/README.md).
Its ExtractBand and native-channel source hashes match `HEAD` at the revision
above, so those two source files are the control for this trial.

## Candidate change

The RGB path now reserves the final output without zero-filling it, then writes
every output byte through the existing const-channel gather. AArch64 keeps the
same unrolled `vld3q_u8`/`vst1q_u8` loop and scalar-tail semantics. The selected
channel is resolved outside the pixel loop. The final `GrayImage` still owns
the same single output allocation. No pixel arithmetic or Pillow semantics
changed.

## Results

All values are per-report medians. Ratios compare the Pillow and target rows in
the same report.

| Source cohort | Pillow | Serial CPU | SIMD | Pillow / CPU | Pillow / SIMD |
| --- | ---: | ---: | ---: | ---: | ---: |
| Control | 167.979 µs | 51.8125 µs | 86.8335 µs | 3.242× | 1.934× |
| Candidate, CPU+SIMD cohort | 251.0005 µs | 60.7915 µs | 173.354 µs | 4.129× | 1.448× |
| Candidate, SIMD-only repeat 1 | 208.708 µs | not run | 104.126 µs | — | 2.004× |
| Candidate, SIMD-only repeat 2 | 261.042 µs | not run | 79.187 µs | — | 3.297× |

The combined CPU+SIMD cohort misses the 2× SIMD target. The two SIMD-only
repeats clear it, but the measured target median ranges from 79 to 173 µs and
the Pillow median also moves substantially. These runs do not show a stable
candidate improvement over the 86.834 µs control. Do not select only the
passing repeats, claim an A/B gain, or promote a strict baseline from this
trial. A controlled same-source A–B–B–A comparison is still needed.

Every candidate benchmark report recorded actual SIMD execution on this ARM64
host: `neon-deinterleave`, 49,152 vector blocks, zero scalar tail, no fallback,
one 786,432-byte output allocation, and terminal completion. The CPU row in
the combined cohort recorded actual CPU execution and remained faster than
Pillow.

## Parity and limits

- Full-size CPU parity: 1/1 passed.
- Full-size strict SIMD parity: 1/1 passed.
- Small RGB plus six RGB tail-residue cases on strict SIMD: 7/7 passed.
- Each benchmark's selected workload also passed its correctness preflight.

The trial covers ARM64 RGB, including vector-tail residues. It does not prove
HSV/YCbCr, other modes, x86 SSSE3, Pillow-SIMD, Parallel CPU, GPU latency or
throughput, or operation-wide completion. The SIMD target remains open.

The JSON reports, raw samples, parity reports, and execution attestations in
this directory preserve the selected inputs, sample policy, backend receipts,
and run-to-run variation.

The three candidate benchmark-result artifacts were imported into the local
PerfDB as `c35b75fb73d5`, `1d35fb39bb96`, and `6973a918c8be`. They remain
observations only; no tracking or strict baseline was promoted or changed.

Call-site inspection found the shared RGB gather is also used by Grayscale's
YCbCr SIMD kernel. Its strict parity and two focused full-call guard runs are
recorded in
[`grayscale-ycbcr-getchannel-shared-gather-guard-arm64-20261019`](../../grayscale/grayscale-ycbcr-getchannel-shared-gather-guard-arm64-20261019/README.md).
Those two benchmark-result observations (`1c105809fb03`, `d9344759e099`) were
also imported without baseline promotion. The guard medians remain inconclusive
against the older control, so GetChannel stays a candidate rather than an
accepted shared-helper optimization.
