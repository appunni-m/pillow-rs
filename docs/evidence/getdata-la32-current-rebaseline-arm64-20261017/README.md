# LA 32×32 GetData current-source rebaseline

This focused current-checkout rebaseline measures the LA alpha-byte `GetData`
path through the public `band-data` observation. It covers one small workload
and does not complete `PIL.Image.Image.getdata`.

## Frozen workload and identity

- Workload: `pipeline-chain.terminal-read.getdata.la-band1-32x32`
- Parity case: `PIL.Image.Image.getdata.performance.material-la-band1-32x32`
- Input: `Image.new("LA", (32, 32), (73, 209))`.
- Boundary: observed step `band-data`; warm release, 5 warmups, 20 iterations
  × 5 samples, concurrency 1.
- Oracle: Pillow 12.2.0 on CPython 3.12.13; target: default serial CPU
  features, with Parallel CPU disabled.
- Host: macOS 15.7.7 ARM64.
- Benchmark run: `migration-benchmark-ec14d0a579724df29a11ba069c0645df`.
- Exact parity gate: `migration-parity-benchmark-gate-90d3365f7fea45d99a99f9090482c1fe` (1/1 passed).
- Dirty source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`.
- Current `image.rs` SHA-256:
  `dcd38ebeeba2b76b0c65ee1430b109833af158bc11a932c451716f3417cd07c8`.
- Current Python wrapper SHA-256:
  `16d7f78048f912b736d56071b5158903d962113cf2d6c1cbe305c5e69762565d`.
- Current PyO3 binding SHA-256:
  `068c22d3b7964dd917584118860de0034623b7f19683e36a87d997b8e8b3ca24`.
- Loaded `pillow_rs._core` SHA-256:
  `3a16bed165a0133179e8b9f594572520cf3489753b54c8c969e4315dffbf6d90`.
- Benchmark input manifest SHA-256:
  `97a4ec9b8fa1704fb77518abae0dbb52e9afd4bf1e8fd459d40c64580fc74966`.
- Parity input manifest SHA-256:
  `c9cd37e39695fa309230dcbd497e3a7a47d2706378d756ec08fb3aad81e67d6d`.

## Result and disposition

| Subject | Median | P95 | Pillow / target |
| --- | ---: | ---: | ---: |
| Pillow | 10.542 µs | 11.041 µs | 1.000× |
| Serial CPU | 5.708 µs | 6.083 µs | 1.847× |

The receipt reports actual CPU execution, one operation, terminal completion,
and no fallback. This current-source workload remains below the 2× CPU
checkpoint. It would need about 0.437 µs (7.7% of target latency) removed to
reach 2×. The current path allocates one immutable output buffer and writes LA
alpha bytes directly into it; the focused audit found no distinct safe shortcut
that removes required snapshot or data movement. Keep the issue open and move to
another ranked item instead of repeating this path's rejected wrapper or scan
trials.

## Artifacts

The benchmark report, exact parity output, raw samples, producer and execution
attestations are retained here. Verify the retained artifact bytes with:

```sh
shasum -a 256 -c SHA256SUMS.txt
```

No full GetData campaign, coverage, commit, or push was run.
