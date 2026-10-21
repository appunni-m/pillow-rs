# GetBBox RGB opposite-corner certificate — no-go

**Decision: reject and revert the candidate.** The exact-parity candidate did
not reduce CPU call latency on either selected workload. This is a diagnostic
experiment only; it was not admitted to a strict PerfDB baseline and did not
change the accepted implementation.

## Hypothesis and guard

For a nonempty RGB image, nonzero logical pixels at both the top-left and
bottom-right corners prove that the bounding box is the full image. The trial
used that certificate to skip the full extrema scan. The bottom-right address
was computed from logical width and height; `ImageBuffer::from_raw` can hold
extra trailing storage, which must not be treated as a logical pixel. A focused
Rust regression test for that case passed during the trial and was removed
with the rejected candidate.

The five selected parity guards passed: corner-pixel, nonzero RGB, green-only
RGB, blue-only RGB, and the existing blank 1024×768 workload. The A1 and A2
measurements each had fresh exact parity for both timed cases. B1 had fresh
exact parity; B2 reused B1 parity against the same candidate build.

## Measurement

ARM64, CPython 3.12.13, Pillow 12.2.0, warm call boundary, 5 warmups, 20
iterations × 5 samples, concurrency 1. Each report completed both selected
workloads. The target receipt reported CPU execution, one terminal operation,
and no fallback. Ratios below are computed from Pillow and CPU medians in the
same report. Times are microseconds.

| Workload | A1 control CPU / Pillow (×) | B1 candidate CPU / Pillow (×) | B2 candidate CPU / Pillow (×) | A2 control CPU / Pillow (×) |
| --- | ---: | ---: | ---: | ---: |
| RGB filled 1024×768 | 1.458 / 2.250 (1.543×) | 1.500 / 2.2915 (1.528×) | 1.542 / 2.250 (1.459×) | 1.500 / 2.375 (1.583×) |
| RGB noise 1024×768 | 1.542 / 2.542 (1.649×) | 1.583 / 2.500 (1.579×) | 1.583 / 2.875 (1.816×) | 1.625 / 3.0625 (1.885×) |

Across the two control reports, the mean of the CPU medians was 1.479 µs for
filled and 1.5835 µs for noise. Across the two candidate reports, it was
1.521 µs for filled (2.8% slower) and 1.583 µs for noise (no meaningful gain).
Pillow medians drifted between reports; B2's better-looking noise ratio came
from a 2.875 µs Pillow median while the CPU median stayed at 1.583 µs. The
candidate therefore has no demonstrated full-call improvement.

| Leg | Benchmark run | Parity evidence |
| --- | --- | --- |
| A1 control | `migration-benchmark-7abf1f1b02944f9f8bfa9c449afa1da5` | `migration-parity-benchmark-gate-1a2b4aa8209f4775b0f4825e1f5b8b77` |
| B1 candidate | `migration-benchmark-6c69950efb11479cb0d4c9d9e13d914d` | Fresh, 2/2 passed |
| B2 candidate | `migration-benchmark-69effb24ddff48ecaaedc0f020faff31` | Reused B1 proof for the same candidate build |
| A2 restored control | `migration-benchmark-327e954a2af74b4a8a49bc7135d92bb3` | Fresh after runtime-identity mismatch; 2/2 passed |

The benchmark reports identify source revision `418d5f0cde51ca64a97cbd007ea731da72d68abf`
with a dirty worktree. These results are not a clean-source/build-bound
baseline. The source candidate and its temporary unit test were reverted;
current `analysis.rs` retains only the separate existing blank-RGB scan work.
No SIMD, Parallel CPU, GPU, or throughput claim was measured here.

## Retained artifacts

`A1/`, `B1/`, `B2/`, and `A2/` contain each benchmark report, raw samples,
execution attestation, producer attestation, and receipt children. A1, B1, and
A2 also retain their parity report and attestations. B2 records its parity
report; the corresponding attestation is retained with B1 because that exact
proof was reused. `guards/` retains the five candidate parity guards.

`SHA256SUMS` covers every retained generated report and receipt. No full image
campaign, full parity campaign, coverage run, or baseline promotion was made.
