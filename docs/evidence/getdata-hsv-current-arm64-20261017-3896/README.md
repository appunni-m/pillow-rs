# GetData HSV band-2 current-worktree rebaseline — 2026-10-17

## Scope

This is one focused ARM64 recheck of `PIL.Image.Image.getdata` band 2 on a 1024×768 HSV image. Workload: `pipeline-chain.terminal-read.getdata.hsv-band2-1024x768`. Exact parity case: `PIL.Image.Image.getdata.performance.material-hsv-band2-1024x768`. The benchmark selected only Pillow and serial `python-cpu`; it measured the existing warm `band-data` step, including the returned 786,432-byte Python bytes result. Setup stayed outside the timed boundary.

The run used Pillow 12.2.0, CPython 3.12.13, release mode, 5 warmups, 20 iterations × 5 samples, concurrency 1. Parity passed 1/1. The target receipt records actual CPU, terminal completion, one GetData operation, and no fallback; Parallel CPU was disabled. The direct HSV band-2 path returns stored V bytes into the result buffer with no mode conversion.

## Result

| Subject | Median | P95 | Median throughput |
|---|---:|---:|---:|
| Pillow | 5.426583 ms | 10.126250 ms | 184.278 operations/s |
| Serial CPU | 2.536042 ms | 5.636625 ms | 394.315 operations/s |

The current dirty checkout measured 2.140× Pillow for this workload and clears the 1× and 2× CPU checkpoints on this host. P95 values had substantial outliers, so this row supports a median comparison only. It does not retire the older clean published macOS row (0.765807×), because the source/build/input identity and host cohort differ.

## Identity

- Run: `migration-benchmark-157a94fdf79443149f61b0fdf5593ea4`
- Parity gate: `migration-parity-benchmark-gate-33cd477138d84b429b573ebed5161d8b`
- Source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31` (dirty)
- Loaded `_core.abi3.so`: `3896b49bbf7375c727f1b3d65af7b527ba4f08d6ba7c5e8e078913fbd3e619ff`
- `pillow-rs/src/ops/analysis.rs`: `016670bd1efa61429332de655c0289f019837a2082ad096f84352625cc86dc13`
- `pillow-rs/src/image.rs`: `dcd38ebeeba2b76b0c65ee1430b109833af158bc11a932c451716f3417cd07c8`
- PyO3 binding `pillow-rs-py/src/lib.rs`: `068c22d3b7964dd917584118860de0034623b7f19683e36a87d997b8e8b3ca24`
- Python wrapper `pillow-rs-py/python/pillow_rs/image.py`: `16d7f78048f912b736d56071b5158903d962113cf2d6c1cbe305c5e69762565d`
- Manifest SHA-256: `ff0d8fa434e60ed844d9d3606396919e28e8b59bdcc590c80d0d73564ec5d885`
- Benchmark input SHA-256: `97a4ec9b8fa1704fb77518abae0dbb52e9afd4bf1e8fd459d40c64580fc74966`
- Parity input SHA-256: `c9cd37e39695fa309230dcbd497e3a7a47d2706378d756ec08fb3aad81e67d6d`
- Pillow `_imaging` SHA-256: `ed534830281dd092be3755632eac8422cdee53d2d8f045ff1779a7d795414d26`
- Host: macOS-15.7.7-arm64-arm-64bit; Pillow 12.2.0 and CPython 3.12.13.

## Reproduction

```sh
make build-parity
MIGRATION_BENCHMARK_BACKENDS=cpu ./.venv/bin/python scripts/run_migration_benchmark.py \
  --workload-id pipeline-chain.terminal-read.getdata.hsv-band2-1024x768 \
  --output docs/evidence/getdata-hsv-current-arm64-20261017-3896/benchmark.json \
  --parity-output docs/evidence/getdata-hsv-current-arm64-20261017-3896/parity.json \
  --raw-samples-output docs/evidence/getdata-hsv-current-arm64-20261017-3896/raw-samples.json \
  --attestation-output docs/evidence/getdata-hsv-current-arm64-20261017-3896/attestation.json \
  --producer-attestation-output docs/evidence/getdata-hsv-current-arm64-20261017-3896/producer-attestation.json \
  --parity-attestation-output docs/evidence/getdata-hsv-current-arm64-20261017-3896/parity-attestation.json
```

No kernel or wrapper change was made. The full local run ledger has the two subject rows for this run; the published matrix remains pinned to its existing snapshot until the normal 33-item batch.
