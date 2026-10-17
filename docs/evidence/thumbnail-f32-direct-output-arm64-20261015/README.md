# F32 Thumbnail direct-output trial — 2026-10-15

## Decision

Reject the serial CPU candidate and restore the measured control. Writing final
vertical F32 values directly to output bytes did not reduce end-to-end work on
this host: Thumbnail CPU latency increased 17.05%, and the shared F32 Resize
guard increased 8.56%. The trial earns no operation or optimization credit.

This was an Apple ARM64 screen, not native-x86 evidence. The worktree was dirty;
Git revision `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31` is not a build identity.
The candidate `geometry.rs` SHA-256 was
`55e2323a3763d2622d488504e8e4d025b89360348ab3738e1537e41bea9b0c1e`; after
removing it, the restored control file SHA-256 is
`f1089848ee2edea2ca4cd9f8a1fcdbe5c859e343730f6783f1f0e22c05e45486`.
The release extension hashes were candidate
`9f35a7b25dabc17ece0f088d73f7cb2f1defb636c02552498aa8e2747fd23775` and restored
control `8d1a3e18548437fe75044c8ba881bfbc4f5b012256709c5cd608e248086698cb`.

## Frozen measurement

The A–B–B–A cohorts used warm cache, five warmups, 20 iterations × five
samples, concurrency one, and full public-call boundaries:

- Thumbnail: `pipeline-op.thumbnail.native-f32-2048x1536`, 2048×1536 mode F,
  `call;observe-receiver`.
- Shared Resize guard: `pipeline-chain.resize-native-f32.bicubic-noise-1024x768`,
  1024×768 mode F, `call;materialize`.

The table gives the mean of the two control cohort medians and the mean of the
two candidate cohort medians, in milliseconds. A positive change is slower.

| Workload | Subject | Control | Candidate | Change |
| --- | --- | ---: | ---: | ---: |
| Thumbnail | Pillow | 4.402990 | 4.548354 | +3.30% |
| Thumbnail | Serial CPU | 1.266532 | 1.482531 | +17.05% |
| Thumbnail | SIMD | 1.797792 | 1.843656 | +2.55% |
| Thumbnail | Requested GPU | 2.372250 | 2.547365 | +7.38% |
| Resize guard | Pillow | 2.877812 | 2.890917 | +0.46% |
| Resize guard | Serial CPU | 0.922771 | 1.001771 | +8.56% |
| Resize guard | SIMD | 1.419427 | 1.409146 | -0.72% |
| Resize guard | GPU | 4.306157 | 4.266625 | -0.92% |

Thumbnail's requested GPU subject ran on CPU with fallback reason
`exact host semantic control: Thumbnail reducing-gap or typed arithmetic is not
proven`; it is excluded from GPU performance claims. Resize guard GPU receipts
reported actual GPU execution. The exact selected parity gates passed all 6/6
comparisons: control gate
`migration-parity-benchmark-gate-1be55eb953b9432d9345c55c271c4231` and candidate
gate `migration-parity-benchmark-gate-84edc35ed9d94d52a8368fafa3ca4ccc`.
A2/B2 reused their matching A1/B1 proofs.

Focused tests for F Bicubic rounding and convolution signed zero passed. A
broader F-resize test selection stalled in GPU tests after 38 visible
completions and was interrupted; it is not recorded as a complete suite or a
test defect. No assertions or thresholds changed. No coverage ran.

## Retained benchmark artifacts

Four benchmark reports and their raw samples are retained beside this file.
The parity proof JSONs and execution attestations remain in ignored
`build/migration-parity/` to avoid duplicating multi-megabyte proof sidecars.
SHA-256 values:

| Cohort | Report | Report SHA-256 | Raw samples | Samples SHA-256 | Run ID |
| --- | --- | --- | --- | --- | --- |
| A1 control | `thumbnail-f32-direct-bytes-arm64-A1-control.json` | `564c5ce48535572eccf3e2ca91149fd9e567e98ae4447509982f5263c730715d` | `thumbnail-f32-direct-bytes-arm64-A1-control-samples.json` | `92f5c15235690e5365b03abb5c2a9da78a2e9f3058dca65fb5cb9b386c246f29` | `migration-benchmark-1856d41ada864c3883c33c3188b8e8e3` |
| B1 candidate | `thumbnail-f32-direct-bytes-arm64-benchmark.json` | `4b066544d747d2bfe3a13bc897798fd1148f52ab1ba5d08b6109cf7cdd923016` | `thumbnail-f32-direct-bytes-arm64-samples.json` | `9450de629efdde4be666885f23fd0653127c9cf803eff9e4421e76abf9796d82` | `migration-benchmark-35dd4dad2c864bbfb678b70f054c4132` |
| B2 candidate | `thumbnail-f32-direct-bytes-arm64-B2-candidate.json` | `5a2fa734d0d426e1af4776de73c1d3e1ad2f38600dfcb47e3dfa3ca4781762bc` | `thumbnail-f32-direct-bytes-arm64-B2-candidate-samples.json` | `75fd47df127b0166cff5db164289f98711f1433eb3b52a64ebb816ee04364ba5` | `migration-benchmark-f1e896dfec6d4697815ca32ff201eeb6` |
| A2 control | `thumbnail-f32-direct-bytes-arm64-A2-control.json` | `6a79b9428a23b8a693f74d8ebf5c422e20ee559957bab8a4bced9b7d796da629` | `thumbnail-f32-direct-bytes-arm64-A2-control-samples.json` | `24ef3ca628791f8ec5b6cafc597cd6f65eed34d27a390857c9ea759ac11940be` | `migration-benchmark-3c0c4a014da3408fb33c0813c17f023f` |

Parity proof hashes: A1
`0de445504adb9a2836ec0917e94c1f58556dccaff9c04528dbb687b031d6d203`, B1
`85a7b33ec61e95b3fe88660514156768a3a8d6adb215975f122e62600e051120`; the
matched repeat cohorts reused these exact proofs. The four runs add 32 rows to
`docs/evidence/performance-optimization-local-runs.csv` (7,615 data rows).

Kata trial issue `pillow-rs#htfj` is closed as a rejected candidate; parent
Thumbnail issue `pillow-rs#eqsm` remains open. This does not increment the
33-operation commit/push batch.
