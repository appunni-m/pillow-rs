# RGB 5×5 BLUR ring current-source rebaseline

This focused run rechecks the retained serial CPU RGB ring specialization on
the exact 1024×768 noisy BLUR workload using the current dirty checkout and
loaded extension. It covers one workload and does not complete
`PIL.Image.Image.filter`.

## Frozen workload and identity

- Workload: `pipeline-matrix.expanded.filter5x5.1024x768`
- Parity case: `PIL.Image.Image.filter.nuanced.rgb-blur-5x5-noise-1024x768`
- Input: RGB noise, 1024×768; asset SHA-256
  `dd83fcb3d95919b5500c8a30b0760cae0f0d5d321e1b0a8f3fd84f267d976456`
- Boundary: warm whole workflow including setup, `filter`, and terminal
  `tobytes`; 1 warmup, 3 iterations × 2 samples, concurrency 1.
- Oracle: Pillow 12.2.0 on CPython 3.12.13; target: default serial CPU
  features, with the Parallel CPU feature disabled.
- Host: macOS 15.7.7 ARM64.
- Benchmark run: `migration-benchmark-5b06787af3e94a1d93b8097fc4975fb1`.
- Exact parity gate: `migration-parity-benchmark-gate-689d06dc72824537a89b3d6eeccd79c6` (1/1 passed).
- Dirty source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`.
- Current `filter.rs` SHA-256:
  `6b063307eba475abeaad72f0c4bb55394ff0d57f204487748e933f26d36e139c`.
- Loaded `pillow_rs._core` SHA-256:
  `3a16bed165a0133179e8b9f594572520cf3489753b54c8c969e4315dffbf6d90`.
- Benchmark input manifest SHA-256:
  `97a4ec9b8fa1704fb77518abae0dbb52e9afd4bf1e8fd459d40c64580fc74966`.
- Parity input manifest SHA-256:
  `c9cd37e39695fa309230dcbd497e3a7a47d2706378d756ec08fb3aad81e67d6d`.

## Result

| Subject | Median | P95 | Pillow / target |
| --- | ---: | ---: | ---: |
| Pillow | 11.476750 ms | 11.615875 ms | 1.000× |
| Serial CPU | 2.445542 ms | 2.496042 ms | 4.693× |

The target receipt reports actual CPU execution, one `Filter5x5` operation,
terminal completion, and no fallback. The exact parity gate passed. This
rebaseline demonstrates this one current-source workload above the 2× CPU
checkpoint; it does not cover other Filter modes, sizes, SIMD, Parallel CPU,
GPU latency, or sustained throughput.

The current source retains the RGB ring dispatch and helper. The earlier
source-matched A–B–B–A evidence is in the adjacent
`rgb-blur-ring-source-matched-abba-arm64-20261016` directory; the present run
is a current-worktree rebaseline, not a new candidate trial.

## Artifacts

`benchmark.json`, timing samples, producer and execution attestations, and
the compressed exact parity report are retained here. The attestation child
receipts are stored in their respective `.children` directories. Verify the
retained artifact bytes with:

```sh
shasum -a 256 -c SHA256SUMS.txt
```

No full parity or benchmark campaign, coverage, commit, or push was run.
