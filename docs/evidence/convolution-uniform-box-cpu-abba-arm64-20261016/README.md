# Filter5x5 native color column-sum A–B–B–A — ARM64, 2026-10-16

## Decision

The serial CPU candidate materially improves the selected normalized all-ones 5×5 LA/RGB public workflows and did not regress the two selected control paths. Keep it as a measured candidate, with no operation-completion credit yet: the nonmatching generic 5×5 fallback has exact parity coverage but still needs a full-call latency guard before accepting this generic-entry hook. SIMD and GPU were not benchmarked.

## A–B–B–A results

Medians are full public-workflow latency in milliseconds, including setup, image allocation, pipeline operations, and final materialization. “A/B” is the mean CPU control median divided by the mean candidate CPU median. Candidate/Pillow is shown for both candidate runs.

| Workload | Control CPU A1/A2 (ms) | Candidate CPU B1/B2 (ms) | A/B | Candidate/Pillow B1/B2 |
|---|---:|---:|---:|---:|
| `pipeline-chain.convolution.material.rgb-5x5-pad.256x256` | 1.748959 / 2.077521 | 0.197666 / 0.187521 | 9.934× | 4.358× / 4.434× |
| `pipeline-chain.convolution.native.rgb-5x5-pad.1024x768` | 21.219874 / 21.376292 | 2.002250 / 1.847375 | 11.065× | 5.075× / 5.171× |
| `pipeline-chain.convolution.native.la-5x5-mirror.1024x768` | 16.200250 / 16.246834 | 1.541729 / 1.465229 | 10.791× | 4.762× / 4.866× |
| `pipeline-chain.convolution.native.rgba-5x5-invert.1024x768` | 0.740020 / 0.704792 | 0.739958 / 0.603625 | 1.075× | 18.317× / 22.156× |
| `pipeline-chain.convolution.native.l-5x5-scale.1024x768` | 2.348438 / 2.302250 | 2.319542 / 2.250521 | 1.018× | 1.872× / 1.941× |

For every candidate workflow, the timed execution receipt requested and observed serial CPU, completed the terminal materialization, and recorded no fallback. `Filter5x5` and the following `Pad`, `Mirror`, `Invert`, or `Scale` step each appear in operation telemetry as applicable. The candidate reported 1,536 B scratch for RGB 256×256, 6,144 B for RGB 1024×768, and 4,096 B of additional scratch for LA 1024×768; these values come from the pipeline allocation receipt, while latency includes all public call work.

## Exact parity and focused contracts

The control and candidate each passed 10/10 selected CPU comparisons (zero failed, not-run, or infrastructure-error cases). Five cases exactly match the timed workflows on assets, steps, and observations. The other five selections are varied-data and fallback guards:

- `convolution-uniform-color-trial-20261014.workflow.material-rgb-pad-256` — exact timed-workflow projection
- `convolution-uniform-color-trial-20261014.workflow.native-rgb-pad-1024` — exact timed-workflow projection
- `convolution-uniform-color-trial-20261014.workflow.native-la-mirror-1024` — exact timed-workflow projection
- `convolution-uniform-color-trial-20261014.workflow.native-rgba-1024` — exact timed-workflow projection
- `convolution-uniform-color-trial-20261014.workflow.native-l-scale-1024` — exact timed-workflow projection
- `convolution-uniform-color-trial-20261014.varied.la-box-5x5`
- `convolution-uniform-color-trial-20261014.varied.rgb-box-5x5`
- `convolution-uniform-color-trial-20261014.varied.rgba-box-5x5`
- `convolution-uniform-color-trial-20261014.guard.rgb-nonmatching-kernel-513x9` — parity only; performance is still unmeasured
- `convolution-uniform-color-trial-20261014.guard.simd-rgb-1024` — parity guard only; no SIMD timing or acceleration claim

Focused Rust contracts: `cargo test -p pillow-rs --lib uniform_box_tests -- --nocapture` passed 5/5; 591 unrelated tests were filtered. The contracts cover decline-without-mutation, extrema and copied borders, LA/RGB/RGBA tails against ordered filtering, public CPU materialization/source preservation, and all possible 25-byte sums. No broad suite or coverage ran.

## Reproduction identity

- Host: `Lazys-MacBook-Pro.local`, macOS 15.7.7 arm64; CPython 3.12.13; Pillow 12.2.0.
- Target identity in each report: revision `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`, dirty working tree, release extension, default Cargo features, requested/actual backend `cpu`.
- Control loaded extension SHA-256: `b4626ad8bd69a9580dcb0e5e2b44944cc7f258496a951880715e413c01ce61dc`.
- Candidate loaded extension SHA-256: `cf5edefafcee72aa0d2f9163f62f04a46cfd97d82a8d272779cdcaf49166c379`.
- Candidate affected source SHA-256: `filter.rs` `3ef85700fb6411bf7ae9eb1b54f4f86c54af5a0e16eece195cca8e63f9d3d24f`; `uniform_box.rs` `a85c72145aedc17d807ffacf86819d8b29342cf015aada396fb79937dd912406`; focused tests `1c6091ee32db33dc0f00fb4e6dff49561eece3a6ec254c708c3d5515508d14cc`.
- Benchmark policy: whole workflow; warm; one warmup; three iterations × two samples; concurrency one; CPU profile.
- Control parity run: `migration-parity-599296f0dffe4913a1977da8b991a30b`; candidate parity run: `migration-parity-a1a5ad02245545ff85d05e5e416bfdbc`.
- `A1-control` benchmark run: `migration-benchmark-4c7496be93f045acb0715384256d378c`; report `A1-control-benchmark.json`; raw samples `A1-control-raw-samples.json`.
- `A2-control` benchmark run: `migration-benchmark-0175194faf534d30b9172bcbc978d484`; report `A2-control-benchmark.json`; raw samples `A2-control-raw-samples.json`.
- `B1-candidate` benchmark run: `migration-benchmark-86f77d0e564e468ea41e5ce396a03664`; report `B1-candidate-benchmark.json`; raw samples `B1-candidate-raw-samples.json`.
- `B2-candidate` benchmark run: `migration-benchmark-219dc1f82cb3436b82e0993cb7d28203`; report `B2-candidate-benchmark.json`; raw samples `B2-candidate-raw-samples.json`.

The parity JSON reports are stored as gzip streams; decompression reproduces the original report bytes. Their decoded SHA-256 values are:

- Control: `0e2def3fe611cf0b20992cd9631fad127dd4c2998d14e0bdcc94ba755ba72f49` (82964076 bytes).
- Candidate: `8d79c9ea85869f5fe696e3ad26874f1846adeaad03198b53b1fd2d68699f08a6` (82964076 bytes).

The benchmark and parity report/attestation/producer files, child artifacts, raw timing samples, and exact custom manifest/input bytes are retained beside this README. The loaded extension binaries are intentionally not copied. The candidate’s `apply_if_uniform` hook checks channel count, dimensions, input/output lengths, normalized coefficients, and rounding bias; it then uses one `u16` vertical-column row and a sliding horizontal sum, leaving the caller-copied two-pixel border intact. `parallel` builds exclude the candidate route.

This is scoped dirty-source evidence. It does not update the clean published matrix, establish SIMD/GPU targets, close the Filter5x5 operation, or count toward the commit batch. No CI/docs/CD workflow was triggered because no commit or push was made; the current commit batch remains 0/33.
