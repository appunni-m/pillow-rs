# I-mode GetColors current-core CPU A1 baseline

This current-core baseline checks a source-current but previously extension-mismatched GetColors miss. The result confirms a serial CPU/Pillow miss on this exact public call; no code changed for the measurement.

## Frozen workload and identity

- Workload: `pipeline-chain.terminal-read.getcolors.high-cardinality-signed-run-i-128x64`; exact case: `PIL.Image.Image.getcolors.nuanced.i-high-cardinality-signed-run-fallback-128x64`.
- Input: I-mode 128×64, 8,192 signed i32 samples, `maxcolors=4097`; raw pixel bytes SHA-256 `8c8ba91f49b89c75ad21f8662f117bdefaf6303317e348bf8e305600b7da3f29`.
- Boundary: observed step `call-fallback-4097`; warm release, 5 warmups, 20 iterations × 5 samples, concurrency 1. Setup is outside the timed boundary.
- Oracle: Pillow 12.2.0 / CPython 3.12. Target: serial CPU, default Cargo features, Parallel CPU disabled. Only CPU was measured.
- Host: macOS-15.7.7-arm64-arm-64bit; ARM64.
- Benchmark run: `migration-benchmark-99fc85ddc61442208ec81d92b4a4eb97`; exact parity gate: `migration-parity-benchmark-gate-6c2d60935b9e4f96b81a45ff80eea5bd` (1/1 passed).
- Dirty source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`; loaded `_core.abi3.so` SHA-256 `3a16bed165a0133179e8b9f594572520cf3489753b54c8c969e4315dffbf6d90`.
- Benchmark input SHA-256: `97a4ec9b8fa1704fb77518abae0dbb52e9afd4bf1e8fd459d40c64580fc74966`; parity input SHA-256: `c9cd37e39695fa309230dcbd497e3a7a47d2706378d756ec08fb3aad81e67d6d`.
- Implementation source SHA-256: core `image.rs` `dcd38ebeeba2b76b0c65ee1430b109833af158bc11a932c451716f3417cd07c8`; PyO3 binding `lib.rs` `068c22d3b7964dd917584118860de0034623b7f19683e36a87d997b8e8b3ca24`; Python wrapper `image.py` `16d7f78048f912b736d56071b5158903d962113cf2d6c1cbe305c5e69762565d`.

## Result and disposition

| Subject | Median | P95 | Pillow / target |
| --- | ---: | ---: | ---: |
| Pillow | 173.584 µs | 187.333 µs | 1.000× |
| Serial CPU | 184.813 µs | 193.250 µs | 0.939× |

The target receipt reports actual CPU execution, terminal completion, no fallback, one `GetColors` CPU operation, and zero physical mode conversions. Exact output parity passed 1/1. The CPU median is slower than Pillow by 6.5%. Resource/allocation telemetry is absent for this eager operation; no allocation claim is inferred. This is a confirmed current-build CPU miss and the next optimization priority. SIMD, Parallel CPU, and GPU were not measured.

## Artifacts

`parity.json.gz` is the compressed exact parity report; uncompressed SHA-256 `957beba4da8c2f4719959c41099ac5a0e651156f6a3d477b0de72712b754b855`. Verify retained artifacts with:

```sh
shasum -a 256 -c SHA256SUMS.txt
```

No broad campaign, coverage, commit, or push was run.
