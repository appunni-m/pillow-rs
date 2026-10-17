# L-mode BoxBlur radius-one CPU row specialization

This matched local trial evaluates a serial CPU-only L row kernel for the
1024×768 radius-one BoxBlur path. It uses the existing Pillow workload and
parity inputs; it does not replace the canonical multi-host operation matrix.

## Frozen scope

- Workload: `pipeline-op.boxblur.material-l-noise-1024x768-radius-1`
- Cases: `PIL.ImageFilter.BoxBlur.nuanced.performance-material-l-noise-1024x768-radius-1`
  and `PIL.ImageFilter.BoxBlur.nuanced.backend-noise-l-65x47-radius-1`
- Backend: strict `python-cpu`
- Boundary: warm `observed_steps`, `apply-filter` + `observe-filter-result`
- Policy: 5 warmups, 20 iterations × 5 samples, concurrency 1
- Oracle: Pillow 12.2.0
- Host: macOS 15.7.7 ARM64; Python 3.12.13
- Source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31` (dirty in all runs)

The sequence was B1 candidate, A1 control, A2 control, B2 candidate. Each
benchmark report contains 100 latency samples for both Pillow and the target.
The same loaded extension bytes were used for both runs within each build:

| Run | Variant | `filter.rs` SHA-256 | `_core.abi3.so` SHA-256 |
| --- | --- | --- | --- |
| B1, B2 | L-only row path | `e34cd28fce861343f7298b9499ac785356da41ac275f89eec803ebe4d8cc0f22` | `46700e668fd73433ee89e0108c383bddacae320e58d2c27e071029c6a8031017` |
| A1, A2 | Existing generic row path | `d7ed7aceec836f4ff65e3b049b4f18d000e4c7bdacaa5079203137a8fb868725` | `c8bdfe53253441a7b31d10e69be1af0be934209e297ec173efa7bb7885123559` |

## Results

| Run | Pillow median | CPU median | Pillow / CPU |
| --- | ---: | ---: | ---: |
| B1 | 1.810875 ms | 0.648792 ms | 2.791× |
| A1 | 1.885125 ms | 0.718396 ms | 2.624× |
| A2 | 1.879771 ms | 0.738625 ms | 2.545× |
| B2 | 1.907063 ms | 0.666688 ms | 2.861× |

The mean of the two candidate medians was 0.657740 ms; the control mean was
0.728510 ms, a 9.71% reduction. The mean of the two per-run Pillow speedups
rose from 2.585× to 2.826×. Every measured target receipt reports actual CPU,
completed terminal materialization, and no fallback.

Exact parity passed both selected cases on B1, A1, and B2 (2/2 each). The
focused Rust row-ring test
`serial_box_blur_radius_one_row_ring_tests::row_ring_matches_fixed_point_two_passes_at_edges_and_tails`
also passed with the candidate enabled. The helper preserves the existing
fixed-point multiply/bias/shift and replicated edges, including widths one and
two. It is compiled only for the serial CPU path; no SIMD or GPU code changed.

This is ARM64 runtime evidence only. Cross-target x86_64 LLVM IR showed the
generic channel switch inside the output loop, which motivates the L-only
dispatch, but no native x86 timing was available. Do not attribute this local
gain to the historical x86 row or update the canonical matrix from this
single-workload trial. SIMD, Parallel CPU, GPU, other modes, and other radii
remain outside this result.

Reusing the first candidate's parity receipt after rebuilding identical
extension bytes was rejected because the runtime identity includes loaded-file
metadata. The two selected parity cases were rerun for the rebuilt candidate;
the guard was left intact. No test defect was found. No full suite, coverage,
commit, or push ran.

The benchmark reports, raw samples, parity reports, producer/execution
attestations, and their child receipts are retained in this directory.
`SHA256SUMS` covers every evidence file except itself.

Kata tracking: `pillow-rs#7gmy`.
