# GetChannel RGB current-source ARM64 rebaseline (2026-10-17)

## Frozen workload

- Operation: `PIL.Image.Image.getchannel`, RGB 1024×768, green channel.
- Workload: `pil-image-image.getchannel.materialized.rgb-1024x768`.
- Exact parity case: `PIL.Image.Image.getchannel.nuanced.performance-rgb-1024x768`.
- Timed boundary: warm `call;observe-result` (the public call and `tobytes()`), 5 warmups, 20 iterations × 5 samples, concurrency 1. Image setup is outside the observed steps.
- Each profile ran separately with only its requested backend selected. Both benchmark gates passed parity 1/1.

## Results

| Profile | Pillow median | pillow-rs median | Pillow / pillow-rs | Execution proof |
| --- | ---: | ---: | ---: | --- |
| Serial CPU | 174.313 µs | 50.188 µs | 3.473× | requested=actual CPU, `ExtractBand` CPU path, 1 operation, terminal complete, no fallback |
| SIMD | 192.687 µs | 87.146 µs | 2.211× | requested=actual SIMD, `neon-deinterleave`, 49,152 vector blocks, zero scalar tail, 1 operation, terminal complete, no fallback |

Use each profile's same-run Pillow baseline for its ratio. These were separate runs, so the Pillow medians differ; this is a rebaseline, not a paired CPU-versus-SIMD experiment. CPU clears Pillow on this ARM64 row. SIMD executed real vector work but remains below the 5× Pillow target and was slower than the serial CPU result in this unpaired observation.

## Identity

- Source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31` (dirty worktree).
- Build: `make build-parity`, release, default features; `parallel` disabled.
- Loaded target extension in both runs: `pillow-rs-py/python/pillow_rs/_core.abi3.so`, SHA-256 `3896b49bbf7375c727f1b3d65af7b527ba4f08d6ba7c5e8e078913fbd3e619ff`.
- Relevant source hashes:
  - `pillow-rs/src/image.rs`: `dcd38ebeeba2b76b0c65ee1430b109833af158bc11a932c451716f3417cd07c8`
  - `pillow-rs/src/compute/pool_cpu/ops/color.rs`: `761c086118858975cbe5ad93457f97241ffedd5816452b1d9b3303d17e8265ca`
  - `pillow-rs/src/compute/pool_simd/ops/adapters/extract_band.rs`: `981e15d6bcdf8628debd9a552212afe1590f4bd20e2348a895bc1b156c11ab33`
  - `pillow-rs/src/compute/pool_simd/ops/native_channel.rs`: `4778f65815df404e7de5a42ca11be07827bf88afcf09ea47a9f15ec9a0f68312`
  - `pillow-rs-py/src/lib.rs`: `068c22d3b7964dd917584118860de0034623b7f19683e36a87d997b8e8b3ca24`
  - `pillow-rs-py/python/pillow_rs/image.py`: `16d7f78048f912b736d56071b5158903d962113cf2d6c1cbe305c5e69762565d`
- Host: macOS 15.7.7 ARM64, CPython 3.12.13, Pillow 12.2.0.

## Evidence and limits

- CPU benchmark: `migration-benchmark-7a3bf876c3124b48893f138080f04891`; parity gate `migration-parity-benchmark-gate-653cf90c5bac44c89ff53f255d264063`.
- SIMD benchmark: `migration-benchmark-f82e387d531a41be89d90151d71eb8be`; parity gate `migration-parity-benchmark-gate-facfd14be2e741d1932fa5f783b018bc`.
- Both benchmark and parity reports validate against the repository schemas. Raw samples, invocation attestations, producer attestations, and report sidecars are retained here; `SHA256SUMS.txt` covers the artifacts.
- The published native-x86 RGB row (0.760568× Pillow) is not rebaselined by this ARM64 run. A read-only SSH probe to `appunni@192.168.0.221` returned TCP connection refused; no authentication or file transfer occurred.
- No code changed. No full campaign, coverage, or unrelated tests ran. The result does not complete the operation or the 33-item commit batch.
