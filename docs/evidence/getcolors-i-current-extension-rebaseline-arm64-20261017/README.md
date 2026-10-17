# GetColors I current-extension CPU rebaseline

Date: 2026-10-17. This is one focused full-public-call benchmark for the saved
GetColors I high-cardinality miss, on the current pillow-rs extension.

## Frozen input and execution

- Workload: pipeline-chain.terminal-read.getcolors.high-cardinality-signed-run-i-128x64
- Parity case: PIL.Image.Image.getcolors.nuanced.i-high-cardinality-signed-run-fallback-128x64
- Input: I 128x64, 8,192 signed i32 pixels. The parity case observes both
  maxcolors=4096 returning None and maxcolors=4097 returning the ordered list,
  including the late negative value.
- Timed boundary: call-fallback-4097 only; image setup outside the timing.
- Sampling: warm cache, 5 warmups, 20 iterations x 5 samples, concurrency 1;
  100 observations per subject.
- Oracle: Pillow 12.2.0, CPython 3.12. Target: python-cpu, default Cargo
  features, strict CPU request.
- Dirty source revision: 9230f5c55ddfe1eab865c1aa5e05ab7b87241c31.
- Loaded extension SHA-256: 3896b49bbf7375c727f1b3d65af7b527ba4f08d6ba7c5e8e078913fbd3e619ff.
- GetColors source hashes match the earlier source snapshot: core image.rs
  dcd38ebeeba2b76b0c65ee1430b109833af158bc11a932c451716f3417cd07c8; PyO3
  lib.rs 068c22d3b7964dd917584118860de0034623b7f19683e36a87d997b8e8b3ca24;
  Python image.py 16d7f78048f912b736d56071b5158903d962113cf2d6c1cbe305c5e69762565d.

## Result

| Subject | Median latency | Pillow / CPU |
| --- | ---: | ---: |
| Pillow | 0.176729 ms | 1.000x |
| Serial CPU | 0.1813125 ms | 0.9747x |

CPU is 4.5835 microseconds (2.59%) slower than Pillow in this one current-
extension run. Exact parity passed 1/1. The target receipt reports actual CPU,
one GetColors operation, zero dispatches, terminal completion, and no fallback.
The single receipt's backend interval is 25.709 microseconds; this is not a
phase measurement of PyO3 result materialization. No allocation telemetry is
available.

## Limits

This rebaseline uses a different extension hash from the earlier saved
GetColors run, even though the GetColors source hashes match. It is one current
run, not an interleaved A-B-B-A comparison, and the measured gap is small.
Use it to prioritize a focused follow-up; it is not a speedup claim or an
operation completion result. No source change, broad campaign, or coverage ran.

The benchmark, parity report, raw samples, and report-bound producer
attestations are retained in this directory. SHA256SUMS.txt covers the JSON
artifacts.
