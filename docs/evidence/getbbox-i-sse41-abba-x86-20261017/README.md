# GetBBox-I SSE4.1 row screening — A/B/B/A result

This was a focused native x86_64 trial for `PIL.Image.Image.getbbox` on
1024×768 `I` images. The candidate screens four 32-bit samples per SSE4.1
block, then resolves nonzero blocks with the scalar raw-word predicate. F-mode
keeps the original path, including negative-zero behavior.

## Frozen scope and verification

- Workloads: `pil-image-image.getbbox.materialized.i-filled-1024x768`,
  `pil-image-image.getbbox.materialized.i-blank-1024x768`,
  `pil-image-image.getbbox.materialized.i-single-center-negative-1024x768`,
  and
  `pil-image-image.getbbox.materialized.i-single-bottom-right-negative-1024x768`.
- Strict CPU parity cases: the seven corresponding I-mode cases plus
  `PIL.Image.Image.getbbox.nuanced.nonzero-f`,
  `PIL.Image.Image.getbbox.nuanced.blank-f`, and
  `PIL.Image.Image.getbbox.parity.f-negative-zero-is-data`.
- The SSE candidate's strict preflight passed 10/10. B1's benchmark parity
  passed all four selected workload cases; B2 reused that identity-matched
  proof. A2's benchmark gate freshly passed the same four workload cases.
- The selected public-call boundary was `call`; each leg used warm cache,
  5 warmups, 20 iterations × 5 samples, concurrency one, and 100 observations
  per subject. Only serial CPU was selected; output is a small tuple, so there
  is no image materialization step.
- The focused x86 Rust boundary/tail test passed 1/1 for widths 0–24 and all
  single/double-hit pairs. The release Python extension rebuilt successfully.

## A/B/B/A medians

All times are microseconds. `A` is the raw scalar control; `B` is the SSE4.1
candidate. Each ratio compares target and Pillow within the same run.

| Workload | A1 CPU | B1 CPU | B2 CPU | A2 CPU | B mean vs A mean | Pillow mean | B mean / Pillow |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Filled | 3462.952 | 7.360 | 7.680 | 3477.920 | 461.5× faster | 8.018 | 1.066× |
| Blank | 599.198 | 439.537 | 439.407 | 600.737 | 1.365× faster | 292.057 | 0.664× |
| Single center | 599.737 | 440.747 | 441.877 | 601.653 | 1.361× faster | 291.329 | 0.660× |
| Single bottom-right | 598.278 | 439.882 | 439.882 | 602.838 | 1.365× faster | 292.229 | 0.664× |

Every target receipt records actual `cpu`, one completed GetBBox operation,
terminal completion, and no fallback. The candidate is a repeatable improvement
over the scalar control on all four workloads, and it brings the filled case
slightly below Pillow latency. Blank and sparse cases remain about 1.5× slower
than Pillow, so this does **not** meet the operation's CPU target and receives
no operation-completion credit. The candidate source was restored after A2;
the current checkout remains on the baseline while work moves to the next
tracked gap.

## Runtime identities and caveat

Runner: `native-x86-192.168.0.221`, Linux x86_64, CPython 3.12.11, Pillow
12.2.0. Baseline A1/A2 loaded `_core` SHA-256
`5b117f519887f77ec3af54a57f7858e57d522be0edc964f041e5aa41c31bf502`;
candidate B1/B2 loaded `_core` SHA-256
`9c5792d6c0351bf6bee58675a8278057d14c235e08e55584210b81214a95ca3b`.
Benchmark and parity fixture hashes matched across the four legs:
`887a6d9e1123387c8021c9005ab26539f3c5b23f68067e6495952436558d485d` and
`5954ee3edb879096b1e1749827878d11ca136026a805a7eb3f95d1abe090a40b`.

The first A1 Pillow runtime stable ID differed from B1/B2/A2 even though the
Pillow tree SHA-256 and loaded `_imaging` SHA-256 matched. This is file-metadata
identity churn; the compared Pillow code content is identical. The separate
target extension content hashes also establish identical A1/A2 controls and
identical B1/B2 candidates.

## Retained artifacts

- `A1-fresh.json`, parity report and attestations, producer attestation, and
  raw samples.
- `B-preflight-parity.json`, attestation, and producer attestation.
- B1/B2 candidate reports, parity reports, attestations, producer attestations,
  and raw samples.
- A2 control report, fresh parity report and attestations, producer
  attestation, and raw samples.

The local-run CSV records separate Pillow and CPU rows for every A1/B1/B2/A2
workload pair. The native runner retains the original reports under
`/home/appunni/work/pillow-rs-getbbox-i-20261017/build/getbbox-i-sse41-abba-20261017/`;
the copies here are the evidence used for this summary.
