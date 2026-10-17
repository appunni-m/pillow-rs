# I-mode Filter 5×5 current-source CPU baseline

This focused A1 run rechecks an older 1.78× CPU result against the current dirty source before selecting an optimization. The old result no longer represents the current implementation. No runtime code changed for this measurement.

## Frozen workload and identity

- Workload: `pipeline-op.filter.material-i-bounded-noise-5x5-1024x768`.
- Exact parity case: `PIL.Image.Image.filter.nuanced.i-mode-kernel-5x5-bounded-noise-1024x768`; I-mode 1024×768, input SHA-256 `d80444d7853adec25fad359c823288473072c811fa553082ce60a5a00e2cb750`.
- Boundary: `call;materialize`; warm release, 5 warmups, 20 iterations × 5 samples, concurrency 1.
- Oracle: Pillow 12.2.0 / CPython 3.12. Target: serial CPU, default Cargo features, Parallel CPU disabled. Only CPU was measured.
- Host: macOS-15.7.7-arm64-arm-64bit; ARM64.
- Benchmark run: `migration-benchmark-27d3a016e71e4966aa2b442d686cdcca`; parity gate: `migration-parity-benchmark-gate-1873ef29846b47708b131739a21b7527` (pass 1/1 passed).
- Dirty source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`.
- Benchmark input SHA-256: `97a4ec9b8fa1704fb77518abae0dbb52e9afd4bf1e8fd459d40c64580fc74966`; parity input SHA-256: `c9cd37e39695fa309230dcbd497e3a7a47d2706378d756ec08fb3aad81e67d6d`.
- Loaded `_core.abi3.so` SHA-256: `3a16bed165a0133179e8b9f594572520cf3489753b54c8c969e4315dffbf6d90`.
- Implementation source hashes: CPU `pool_cpu/ops/filter.rs` `6b063307eba475abeaad72f0c4bb55394ff0d57f204487748e933f26d36e139c`; Rust wrapper `ops/filter.rs` `b9a090099ad4270b8117ce6f53eb5e9d36f82237b109940c7cd3b90b7ac21e71`; Python `imagefilter.py` `221ce032981e8ebf9226399107192653dadae6bb8b0e4862c66813c6f0f3f9c5`; Python `image.py` `16d7f78048f912b736d56071b5158903d962113cf2d6c1cbe305c5e69762565d`.

## Result and disposition

| Subject | Median | P95 | Pillow / target |
| --- | ---: | ---: | ---: |
| Pillow | 5.949375 ms | 6.454250 ms | 1.000× |
| Serial CPU | 1.904937 ms | 2.077083 ms | 3.123× |

The target receipt reports actual CPU execution, completed materialization, no fallback, one `Filter5x5` operation, and no mode conversion. Resource receipt: 2 host buffers / 6,291,456 bytes, peak 6,291,456 bytes; the allocation hook reports zero, so no allocator count is inferred. Exact byte parity passed 1/1. The current median exceeds the 2× CPU checkpoint. The older 1.78× result at source revision `88d414b…` is stale for ranking current CPU gaps. No optimization is selected from it. SIMD, Parallel CPU, and GPU were not measured.

## Artifacts

`parity.json.gz` is the compressed parity result; its uncompressed SHA-256 is `75552facba176021a7ffdf734225293ee8aca4f53095a9eca33a72b322c48f3c`. Verify retained files with:

```sh
shasum -a 256 -c SHA256SUMS.txt
```

No broad campaign, coverage, commit, or push was run.
