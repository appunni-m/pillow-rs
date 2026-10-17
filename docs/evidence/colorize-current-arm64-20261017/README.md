# Colorize L-noise current-source A1 baseline

This focused baseline measures the existing public `PIL.ImageOps.colorize` call with terminal materialization. It establishes current CPU performance only; no runtime optimization or operation completion is claimed.

## Frozen workload and identity

- Workload: `pil-imageops.colorize.materialized-noise-l-1024x768`.
- Exact parity case: `PIL.ImageOps.colorize.nuanced.performance-large-l-noise-1024x768`; L noise input asset SHA-256 `bbeddf3f3f523d7a7bc4910bb56f7aa5ff8ec4f178048b8c09bd7037be9f2edc`.
- Boundary: `call` + `observe-result` (`tobytes`); warm release, 5 warmups, 20 iterations × 5 samples, concurrency 1.
- Oracle: Pillow 12.2.0 / CPython 3.12.13. Target: serial CPU, default Cargo features, Parallel CPU disabled.
- Host: macOS-15.7.7-arm64-arm-64bit; CPU reported as `arm`.
- Benchmark run: `migration-benchmark-eca25e83263044529fe4bae700c996b0`; exact parity gate: `migration-parity-benchmark-gate-9f965a1245f244d4940c0a6e0f3135f1` (1/1 passed).
- Dirty source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`.
- Benchmark input SHA-256: `7edbefa95fefc7bc7daa664c51e9782b84bf479b4a30ae3dffdc0ce7c559d57f`; parity input SHA-256: `f9bc243391b4f5b3129319d8c792c59a12691df3363ff1aedb02ffb005923b2e`.
- Native extension `_core.abi3.so` SHA-256: `3a16bed165a0133179e8b9f594572520cf3489753b54c8c969e4315dffbf6d90`.
- CPU implementation source hashes: `pool_cpu/ops/imageops.rs` `68f644ef3d0da1398ec70131f684ddcc9830e7266a5c35cc3a085b97cacb4bb8`; `registry.rs` `1d37d30579e8a2563c804ae5271e1ca118ade42ca6e4c3720ba45bb0d010690f`; Python wrapper `imageops.py` `9118e1b66382e9867f54be37745723249361987e4e251a331d4394cfa1d9c4c1`.

## Result and disposition

| Subject | Median | P95 | Pillow / target |
| --- | ---: | ---: | ---: |
| Pillow | 1.949000 ms | 15.909583 ms | 1.000× |
| Serial CPU | 0.812125 ms | 2.074500 ms | 2.400× |

The target receipt reports actual CPU execution, terminal completion, no fallback, one `Colorize` operation, and the expected L→RGB materialization. The target median is above the 2× CPU checkpoint. Pillow had a high p95 tail in this run, so retain this as a screening baseline and avoid inferring a tail-latency guarantee from the median. No CPU optimization is selected from this result; proceed to a different current CPU gap.

## Artifacts

The parity report is gzip-compressed to keep the retained evidence compact. Decompressing `parity.json.gz` reproduces the run's raw parity report; its uncompressed SHA-256 is `59091ed97157335aa3c0efd4692d196b7294f9696013380b7244c40d6cce8f2b`. Verify retained artifact bytes with:

```sh
shasum -a 256 -c SHA256SUMS.txt
```

No broad campaign, coverage, commit, or push was run.
