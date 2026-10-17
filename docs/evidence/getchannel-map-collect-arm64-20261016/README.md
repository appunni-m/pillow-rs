# RGB GetChannel `map`/`collect` public-call experiment — 2026-10-16

This is a dirty-tree, single-host, single-workload serial-CPU diagnostic for
the RGB implementation in `pillow-rs/src/compute/pool_cpu/ops/color.rs`. The
candidate replaces the four-pixel `extend_from_slice` gather loop with
`chunks_exact(3).take(pixel_count).map(|pixel| pixel[channel]).collect()`.
It allocates only the output vector and explicitly limits the gather to the
logical pixel count.

The benchmark workload was
`pil-image-image.getchannel.materialized.rgb-1024x768`; the exact parity case
was `PIL.Image.Image.getchannel.nuanced.performance-rgb-1024x768`. The public
boundary was warm `call;observe-result`, with five warmups, 20 iterations × 5
samples, concurrency one, Pillow 12.2.0, and the release build's default
features. It includes call routing, output construction, and result
materialization. Only `python-cpu` was selected.

## Candidate / control / candidate

All values are medians from the same run. Ratios use that run's Pillow timing.

| Cohort | Pillow | Serial CPU | CPU / Pillow | Exact parity | Run |
| --- | ---: | ---: | ---: | ---: | --- |
| B1 candidate | 165.104 µs | 44.084 µs | 3.745× | 1/1 pass | `migration-benchmark-4031e0537b07407bae5b4dec6b23427c` |
| A control | 174.375 µs | 194.187 µs | 0.898× | 1/1 pass | `migration-benchmark-45b26b8858f949599b2782660829f204` |
| B2 candidate | 154.792 µs | 50.313 µs | 3.077× | reused exact B1 proof | `migration-benchmark-f40806ff7eba4e628ea1ad3863f95505` |

The B-A-B ordering brackets one control between two candidate runs. The
candidate loaded extension SHA-256 was
`e7a1ce76dd97149c6a86b0b69db926fee58b2b1a93740cbd60fe0ee6d6f6839e` for both
B runs; the control extension SHA-256 was
`26cb42df2eb53ed8a0c9252fb0f5c8484f4c3ebab24584e4d49a0eea7681ebec`. B2
reused B1 parity only after the source, extension, input, manifest, oracle, and
profile identities matched. The B1 and A parity reports each selected and
passed the single exact case.

Each serial-CPU receipt reports actual `cpu`, completed terminal
materialization, one operation, and no fallback. The resource receipt reports
one 786,432-byte output allocation, one mode conversion, zero upload/readback,
and 3,145,728 peak live host bytes. SIMD, Parallel CPU, and GPU were not run.

## Interpretation and limits

This is a useful source-matched diagnostic: the old control misses Pillow on
this host and case; both candidate cohorts are faster than Pillow by more than
the user's interim 2× CPU sequencing threshold. It supports retaining this
narrow RGB serial-CPU candidate for broader focused evaluation.

It does not complete GetChannel, update the clean canonical matrix, or
establish the target across modes, sizes, architectures, backends, or composed
pipelines. In particular, this is not a SIMD result. The benchmark and parity
reports came from a dirty worktree whose unrelated changes were present; the
full source-file SHA-256 for the candidate state is
`761c086118858975cbe5ad93457f97241ffedd5816452b1d9b3303d17e8265ca`. The
control source variant restored for A had SHA-256
`a1ca93c594d0b417df9fc64d8977dd76b09fe9f0c6ca7b5b2456b7f66608cd19`.

The focused Rust edge test
`rgb_extract_band_ignores_trailing_storage_for_every_tail_and_band` passed
with the candidate (1 passed, 590 filtered). It checks trailing storage and
RGB channel output at widths around vector/group boundaries. No full campaign
or coverage ran. The local-run ledger records both subjects for all three
cohorts; these diagnostic rows do not award operation-completion credit.

## Retained artifacts

| Artifact | SHA-256 |
| --- | --- |
| `benchmark.json` (B1) | `7cc500e61dad924829b8c685af5caff18a5ce0e555e18047bb17368aeb44e49a` |
| `raw-samples.json` (B1) | `e969c84d708f9ba2b1af6ebe6d49236ee463ec92e18d6481e7ee166d4465ccdc` |
| `benchmark-parity.json` (B1) | `b276a0a0bc714bf7590f5c2f7c3103b48417e18822fa0e817ae5d95dc1f246c5` |
| `control-benchmark.json` | `a192ee21a5634f4c53f8b1c04dc94637ec60e8592a8f310521847c30b2400a56` |
| `control-raw-samples.json` | `c5fcc616f5f9f6ef5a60eaaa32dcd17bb1b2dfa729b6483e045851c2d0c2113b` |
| `control-benchmark-parity.json` | `d80071b7be35f068d89b35bc50b5a5f373c96aa4e465a2bc2a7de6bf61b787fa` |
| `candidate-b2-benchmark.json` | `9c50be067a3318af4e0d0bb6131d4cd4290134f9905c439b3f486c91e532a42a` |
| `candidate-b2-raw-samples.json` | `9a29ceac34db5ee3079ca2eb3becd98112b721e3c666e05a327b0a57a1fcacaf` |
| `candidate-b2-benchmark-parity.json` | `b276a0a0bc714bf7590f5c2f7c3103b48417e18822fa0e817ae5d95dc1f246c5` |
