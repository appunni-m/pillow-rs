# RGB 5×5 BLUR ring CPU A–B–B–A

This bundle records a source-matched CPU comparison for the existing serial RGB BLUR ring specialization. It covers one workload and does not complete `PIL.Image.Image.filter`.

## Frozen call and input

- Workload: `pipeline-matrix.expanded.filter5x5.1024x768`
- Parity case: `PIL.Image.Image.filter.nuanced.rgb-blur-5x5-noise-1024x768`
- Input: RGB noise, 1024×768; asset SHA-256 `dd83fcb3d95919b5500c8a30b0760cae0f0d5d321e1b0a8f3fd84f267d976456`
- Input manifest SHA-256 `abfb9fe15ce284f84cf775a62358906810ee50fd4fc6f57fc300d291f99ef18c`; parity input SHA-256 `bf0fc142ef5ebe4c6da85a6b549316a4ee6ac9d365c5eadb486121010702d9f0`
- Pillow 12.2.0; macOS 15.7.7 ARM64; Python 3.12.13; dirty source revision `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`
- `whole_workflow`, setup + filter + terminal `tobytes`; warm release/default features, one warmup, 3 iterations × 2 samples, concurrency 1
- `MIGRATION_BENCHMARK_BACKENDS=cpu` selected only `python-cpu` for each run.

## Results

| Run | Pillow median | CPU median | Pillow / CPU | Parity | Receipt |
| --- | ---: | ---: | ---: | --- | --- |
| A1 control | 10.796646 ms | 19.632021 ms | 0.550× | pass | actual CPU, terminal complete, no fallback |
| B1 ring | 10.633667 ms | 2.213396 ms | 4.803× | pass | actual CPU, terminal complete, no fallback |
| B2 ring | 11.062459 ms | 2.231813 ms | 4.956× | pass | actual CPU, terminal complete, no fallback |
| A2 control | 10.751833 ms | 19.524188 ms | 0.551× | pass | actual CPU, terminal complete, no fallback |

Mean candidate CPU latency was 2.222604 ms versus 19.578104 ms for the controls (8.81× lower). Both candidate legs beat same-run Pillow and exceed the 2× CPU/Pillow checkpoint for this workload. The target receipt reports one `Filter5x5` operation, requested and actual backend `cpu`, completed execution, and an empty fallback map.

## Source and parity identity

- Candidate `filter.rs`: `e224b1e8e3d9ac60df9512a3b011fa64a7fdb056f298651670e4861e2848da68`
- Control `filter.rs`: `1b8083177b4af563b5186f88c1271b167787f9f49b9bcb1a313115947f2510aa`
- The only source change was adding a constant-false guard to the RGB ring dispatch in the control, preserving compilation references while forcing the existing generic path. See `control-dispatch.patch`.
- Control `_core.abi3.so`: `3c9d77f6e125855bd60032ac6154128c0e524644dc3abd0544d4ddea1fa23a28`
- Candidate `_core.abi3.so`: `8b0dda44c94c7c998a511aebd8718c1d244ad60650086ebcb2bb21d26409d749`
- Each control and candidate source/build identity passed the selected parity case. B2 reused B1 parity because source, input, and loaded extension bytes matched. A2 ran one fresh parity preflight: swapping the `.so` changed runtime file-stat identity, so the harness correctly rejected proof reuse even though its bytes matched A1. The failed reuse attempt did not start a benchmark.
- All reports, raw samples, child receipts, execution attestations, producer attestations, and parity artifacts are included. Their digests are listed in `SHA256SUMS.txt`.

## Focused checks and limits

The two `rgb_ring` helper tests and `dispatch_admits_only_pillow_blur_parameters` passed. A public nonmatching-kernel guard exists in an experiment input file, but the default parity manifest does not activate it; an attempted selector therefore returned `no active parity cases selected` before executing a case. It is not counted as parity evidence. The active benchmark parity case and exact dispatch-parameter contract both passed.

This is a dirty local ARM64 serial-CPU result for one RGB 1024×768 kernel. SIMD, Parallel CPU, GPU latency and throughput, other modes/sizes/edges, x86, clean hosted matrix refresh, and full `Image.filter` completion remain open. No coverage, commit, or push was run.
