# RGBA Transpose CPU current-source refresh — 2026-10-20

## Frozen scope

- Public operation: `PIL.Image.Image.transpose`.
- Workload: `pipeline-matrix.transpose-tiled.rgba-772x768-transpose-mirror.operation-materialized`.
- Input: RGBA, 772×768; `transpose-0;transpose-1;materialize`.
- Boundary: warm end-to-end call including result materialization and Python
  `bytes` output.
- Sampling: release build, 5 warmups, 20 iterations × 5 samples, concurrency 1
  (100 observations per subject).
- Host: native x86_64 Linux, CPython 3.12.3, Pillow 12.2.0.
- Strict CPU parity passed 2/2 cases, including the aligned workload case and
  the odd-size guard:
  `PIL.Image.Image.transpose.nuanced.tiled-aligned-rgba-772x768-methods-5-0`
  and
  `PIL.Image.Image.transpose.nuanced.tiled-odd-rgba-1025x1023-methods-5-0`.

## Result

Ratios use the Pillow median from this same report divided by the target median.
A value below 1 means the target was slower than Pillow.

| Profile | Pillow median | Target median | Target p95 | Pillow / target | Actual execution |
| --- | ---: | ---: | ---: | ---: | --- |
| Pillow | 2.407218 ms | 2.407218 ms | 2.925899 ms | 1.0000× | Pillow oracle |
| Serial CPU | 2.407218 ms | 3.685192 ms | 4.114073 ms | 0.6532× | CPU, 2 operations, terminal complete, no fallback |

The serial CPU call was 53.1% slower by median. Its median throughput was
271.36 calls/s versus Pillow's 415.42 calls/s. The CPU receipt records two
`Transpose` operations composed into one fused pipeline, one 2,371,584-byte
allocation, 4,743,168 peak live host bytes, and no mode conversion. The measured
call includes the final materialization. The backend phase receipt was
2.291117 ms; this phase number is diagnostic and is not substituted for the
public-call latency above.

This is a current-source refresh, not an optimization result. The selected CPU
source closure and input objects matched the main worktree during the run, but
the detached runner has unrelated source differences, so this is not a whole
tree identity claim. The existing transpose kernel experiments are exhausted;
keep this CPU gap in the queue and move on to the next ranked operation.

## Evidence and execution identity

- Benchmark run: `migration-benchmark-c9ff2452790f48638e9f1b1fbf27a8fa`.
- Benchmark report SHA-256:
  `d8d7c7ba377703e7c768ac62ad0bc96276c8b92a5aabc679e1b9b6a7d70b91f3`.
- Benchmark execution attestation SHA-256:
  `c1c7fc1b11b04b856caeba8a4b3977083eadeb2d4bbdfd703b7ed810fca4ee3e`.
- Producer attestation SHA-256:
  `b79367a564cdc9962017fdfc5c04a3e9e3313243a79c0af7e1a0ec567e546fcc`.
- Raw samples SHA-256:
  `2dc7298da2912b2cd346b9221c28061635e0b6368428225fa200c07a9d8327a2`.
- Benchmark's paired parity gate passed for the selected aligned case:
  `migration-parity-benchmark-gate-d3f770f7c84c437ab7e6935ddfe5cc70`.
- Separate strict CPU parity run passed both selected cases:
  `migration-parity-3783b4d7df0547acad8376e0672108f8`.
  Its report SHA-256 is
  `6b34cf108bfc30e59a67700721a3202e639d874dfbd8ec5efa1c36668cab3840`.
- Release extension SHA-256 after rebuilding from the runner checkout:
  `7b7433a32cd1a4c49dd4019d328b673f5cf960c16a6cef2133a6123ce118a066`.
  The rebuilt extension was byte-identical to the runner's prior extension.
- Reported target revision: `418d5f0cde51ca64a97cbd007ea731da72d68abf`,
  dirty worktree.

The compact benchmark report was imported into the local generated PerfDB as
artifact `d8d7c7ba3777…`. PerfDB `history` displays the CPU observation, while
the strict `rank --source-kind benchmark-result --profile python-cpu
--baseline Pillow --below 1.0` query omits it: the report's correctness label
is `source_target_match: pass`, and rank currently admits only
`parity_pass: pass` or `successful_execution: pass`. Treat this as a
diagnostic row until its selected-run proof is admitted; no baseline was
promoted.

The runner rejected one `--reuse-parity` attempt with
`current Pillow oracle identity differs from reused parity`. The parity
attestation and a standalone current-oracle fingerprint both report the same
Pillow stable hash (`f6730182bb8cecfa88f1274937af2f8f1e21021dfe4d9833f3d33fecb3a5cfab`).
A fresh selected preflight then passed and the benchmark completed. Investigate
the reuse guard before relying on reused parity evidence; no comparison or
threshold was bypassed.

Only the two frozen parity cases and this one CPU workload ran. No full campaign,
coverage, or unrelated test suite ran. Raw reports remain on the existing x86
runner; only the compact result was copied to the local PerfDB staging area.
