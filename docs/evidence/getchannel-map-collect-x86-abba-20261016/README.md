# RGB GetChannel map/collect x86 trial — identity mismatch — 2026-10-16

**Disposition: diagnostic only; no optimization credit.** The planned native-x86
A1-B1-B2-A2 public-call comparison did not keep one control binary across A1
and A2. A2 also reused a parity proof produced by a different control binary.
The observed timings therefore do not attribute a speedup or regression to the
candidate. The mismatch is recorded so a future trial can pin the imported
extension before timing.

## Frozen selection

- Workload: `pil-image-image.getchannel.materialized.rgb-1024x768`
- Parity case: `PIL.Image.Image.getchannel.nuanced.performance-rgb-1024x768`
- Workload SHA-256: `f0203987ebe336b57fe121e3a4d8a6f2cdb1e787d8c28c2d9192cb1a8d1dd5cf`
- Case SHA-256: `ca5d07d413f695119196f9bdb980f2fc6bfb788173afed973c1f124907790e96`
- Host: AMD Ryzen 5 2600X, Linux x86_64, CPU 4 affinity
- Boundary: warm `call;observe-result`, five warmups, 20 iterations × five
  samples, concurrency one; regular Pillow 12.2.0; serial CPU only; default
  Cargo features with `parallel` disabled
- Manifest SHA-256: `63270b3ab5595564616e4a84351825a56fb67f87b73157e6227cade65b5b29bb`
- Selected input SHA-256 values: benchmark
  `8f735532c7f041ec9a5c79b5c334e7c20aeaa91252362419408fae48eabd7f63`, parity
  `7676c7d62ccdd4a4756b1b1f6f668cf0329721a9ae03e7b2fd6557536470b49a`

## Observed medians

| Position | Loaded core SHA-256 | Pillow ms | Serial CPU ms | Pillow / CPU |
| --- | --- | ---: | ---: | ---: |
| A1 control | `0a5b618df3e87b75d65c5ead9860bb9205d42b30251d28bf15d688bf7b1faa8f` | 0.377907 | 0.311687 | 1.212× |
| B1 map/collect | `845f2a818e8093d22ddcf48a6e054e43cd042ed7e8b155e723c08864367ec516` | 0.345756 | 0.318676 | 1.085× |
| B2 map/collect | `845f2a818e8093d22ddcf48a6e054e43cd042ed7e8b155e723c08864367ec516` | 0.323971 | 0.358631 | 0.904× |
| A2 control | `83d8491c207a6eea16968ab5a30f62df3eb2a887c8dbd131cc17f3e8d2557053` | 0.313840 | 0.298175 | 1.053× |

All four reports show the selected workload and 100 timing observations per
subject. B1 ran the selected exact parity case; B2 reused B1's proof with the
same candidate core hash. Both target reports show one completed `ExtractBand`
on actual CPU, terminal completion, and no fallback. The Pillow median moved
from 0.378 ms at A1 to 0.314 ms at A2; this 17% drift and B1/B2 reversal are
consistent with enough run-to-run variation that these ratios cannot support
source attribution.

## Identity and parity defect

The control source and candidate base were matched at `f2b63f559e91022a85cb43f7264bdf8b586a448d`; the baseline `color.rs` SHA-256 was
`a1ca93c594d0b417df9fc64d8977dd76b09fe9f0c6ca7b5b2456b7f66608cd19`, and the
candidate source SHA-256 was
`761c086118858975cbe5ad93457f97241ffedd5816452b1d9b3303d17e8265ca`.
However, A1 imported the control checkout's `_core.abi3.so` at hash `0a5b…`,
while A2 imported the pre-existing package extension at hash `83d8…`. The
candidate B runs loaded the shared Cargo target artifact `lib_core.so` at
`845f…`. A separate earlier candidate-only report in
[`getchannel-map-collect-x86-20261016`](../getchannel-map-collect-x86-20261016/benchmark.json)
loaded `96beda…`, so it is not folded into this cohort either.

A2's report says `parity_pass`, but its parity proof was reused from the A1
control run and the loaded core hashes differ. Under the frozen identity rule,
that proof is inadmissible for A2. No A2 parity was executed. Do not treat the
reported gate as valid A2 parity evidence.

The setup defect was using a pre-existing package extension for control while
building the candidate through the shared Cargo target. For a repeat, build
control and candidate extensions in isolated source/build locations, pin each
imported path and hash before each run, and reject the run before timing if the
loaded hash does not match the planned variant. Run one parity proof per exact
variant identity. The existing control checkout and its 451 MB target cache
were left in place; no target directory or full fixture corpus was copied.

## Artifacts

This directory keeps the compact B1/B2/A2 reports, raw samples, execution and
producer attestations, plus the B1 parity output. A1 is retained in
[`getchannel-x86-current-source-20261015`](../getchannel-x86-current-source-20261015/benchmark-result-cpu4.json)
with its CPU4 attestation archive and parity proof. Run IDs:

- A1: `migration-benchmark-4346db456fd14a1e8cb940b92346ae3c`
- B1: `migration-benchmark-18b3ef3a87904e80b1fa7fc0cb1ef76b`
- B2: `migration-benchmark-d9ee25a8a67a4ae8b2fb2eeb608ff996`
- A2: `migration-benchmark-e09716c3539a45f48d10c5f070b8c003`

Kata tracking: `pillow-rs#wzjx`. No source implementation was changed, no
full parity or benchmark campaign or coverage ran, and nothing was committed
or pushed.
