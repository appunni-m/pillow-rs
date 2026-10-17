# RGBA nearest Rotate current-source control

This focused A1 control measures the existing nonuniform RGBA nearest-rotate
parity input through the public call and terminal materialization. It is a
separate workload from the constant-color whole-workflow Rotate chain and does
not complete `PIL.Image.Image.rotate`.

## Frozen workload and identity

- Workload: `pipeline-op.rotate.material-rgba-nearest-37deg-noise-fill-1024x768`
- Parity case: `PIL.Image.Image.rotate.nuanced.rgba-nearest-37deg-noise-fill-20261016-1024x768`
- Input: RGBA noise, 1024×768; expanded nearest rotate at 37° with custom
  fill `[17, 31, 47, 63]`; noise asset SHA-256
  `b32158748a66d3c511860dc5253757e02c0edb799613e67dfd503ad9f53fa51f`.
- Boundary: observed `call` + `observe-result` (`tobytes`); warm release,
  5 warmups, 20 iterations × 5 samples, concurrency 1.
- Oracle: Pillow 12.2.0 on CPython 3.12.13; target: default serial CPU
  features, with Parallel CPU disabled.
- Host: macOS 15.7.7 ARM64.
- Benchmark run: `migration-benchmark-b8c6bd13b9ab4c3bb5a56ee8d25a7e59`.
- Exact parity gate: `migration-parity-benchmark-gate-51d8f3eb51504a4dab4dc1c4ae4253ad` (1/1 passed).
- Dirty source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`.
- Current `geometry.rs` SHA-256:
  `f6b2603ec1c712f2274d7b02c7e162a30d7cc33d63cb50d7b439a108796721d8`.
- Loaded `pillow_rs._core` SHA-256:
  `3a16bed165a0133179e8b9f594572520cf3489753b54c8c969e4315dffbf6d90`.
- Benchmark input SHA-256:
  `97a4ec9b8fa1704fb77518abae0dbb52e9afd4bf1e8fd459d40c64580fc74966`.
- Parity input SHA-256:
  `c9cd37e39695fa309230dcbd497e3a7a47d2706378d756ec08fb3aad81e67d6d`.

## Result and disposition

| Subject | Median | P95 | Pillow / target |
| --- | ---: | ---: | ---: |
| Pillow | 2.433563 ms | — | 1.000× |
| Serial CPU | 1.105167 ms | 1.412583 ms | 2.202× |

The receipt reports actual CPU execution, `Rotate` telemetry, terminal
completion, and no fallback. This exact nonuniform workload is already above
the 2× CPU checkpoint. No fill-loop candidate was applied; the proposed
16-byte grouped edge fill is deferred unless another required workload still
misses or a distinct measured opportunity appears.

The earlier constant-color workload
`pipeline-chain.geometry-material.rotate-rgba-1024x768` remains unchanged. It
has a different input and `whole_workflow` boundary and cannot substitute for
this exact noise/fill stimulus.

## Artifacts

The benchmark report, exact parity output, timing samples, producer and
execution attestations are retained here. Verify the retained artifact bytes
with:

```sh
shasum -a 256 -c SHA256SUMS.txt
```

No full Rotate campaign, coverage, commit, or push was run.
