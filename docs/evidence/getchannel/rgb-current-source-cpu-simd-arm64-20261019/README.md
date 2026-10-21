# GetChannel RGB current-source CPU/SIMD ARM64 rebaseline — 2026-10-19

## Frozen call

- Public operation: `PIL.Image.Image.getchannel`, RGB 1024×768, green band.
- Workload: `pil-image-image.getchannel.materialized.rgb-1024x768`.
- Exact parity case: `PIL.Image.Image.getchannel.nuanced.performance-rgb-1024x768`.
- Timing boundary: warm public `call;observe-result`, including result materialization; 5 warmups, 20 iterations × 5 samples, concurrency 1.
- Pillow oracle: 12.2.0, CPython 3.12.13. The benchmark and parity used isolated source/target processes.
- Source revision: `418d5f0cde51ca64a97cbd007ea731da72d68abf`; dirty worktree. Build profile: release, default features, Parallel CPU disabled.
- Benchmark run: `migration-benchmark-aa6f9cb2b719417facd2338b1e38600a`. Parity gate: `migration-parity-benchmark-gate-96159bde49594475afdd038383609cf9`; 2/2 profile comparisons passed.

## Results

| Profile | Pillow median | pillow-rs median | Pillow / pillow-rs | Receipt |
| --- | ---: | ---: | ---: | --- |
| Serial CPU | 167.979 µs | 51.812 µs | 3.242× | actual CPU, terminal complete, no fallback |
| SIMD | 167.979 µs | 86.834 µs | 1.934× | actual SIMD, `neon-deinterleave`, 49,152 vector blocks, zero tail, terminal complete, no fallback |

CPU clears Pillow on this ARM64 workload. SIMD is slower than CPU in this run and below the 5× target. This does not prove results for other modes, sizes, x86, Pillow-SIMD, GPU, or Parallel CPU.

Both targets reported one completed `ExtractBand` operation, one mode conversion, two host buffers (3,145,728 bytes total), and one 786,432-byte output allocation. Timing includes the public call and materialization. The report retains 100-sample summary statistics; raw sample values were not emitted by this run.

## Build identity

The extension was built from the current dirty worktree with `make build-parity`. Its SHA-256 and the relevant source hashes are recorded here:

| File | SHA-256 |
| --- | --- |
| `pillow-rs/src/image.rs` | `423f9d54b312afb31fdf10b294309fafcf12e5a375ece9887d2b2e238db547e3` |
| `pillow-rs/src/compute/pool_cpu/ops/color.rs` | `1e0536eeccdde70eac33495bbdc1553749021f4f63a81a5e1d9acf6daa7dc732` |
| `pillow-rs/src/compute/pool_simd/ops/adapters/extract_band.rs` | `981e15d6bcdf8628debd9a552212afe1590f4bd20e2348a895bc1b156c11ab33` |
| `pillow-rs/src/compute/pool_simd/ops/native_channel.rs` | `4778f65815df404e7de5a42ca11be07827bf88afcf09ea47a9f15ec9a0f68312` |
| `pillow-rs-py/src/lib.rs` | `068c22d3b7964dd917584118860de0034623b7f19683e36a87d997b8e8b3ca24` |
| `pillow-rs-py/python/pillow_rs/image.py` | `16d7f78048f912b736d56071b5158903d962113cf2d6c1cbe305c5e69762565d` |
| `pillow-rs-py/python/pillow_rs/_core.abi3.so` | `400c2ccc2b3644d4a87c9c604b112189cee53aa82b02426491cd77d623bb33e3` |

`benchmark.json` is the imported PerfDB observation. `parity.json.gz` is the lossless compressed parity report. No source algorithm changed during this rebaseline.
