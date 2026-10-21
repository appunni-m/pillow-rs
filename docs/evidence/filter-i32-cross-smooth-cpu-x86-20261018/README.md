# Native x86_64 I-mode cross-smooth CPU evidence

Host: supplied native x86_64 runner. The selected workload is
`pipeline-chain.convolution-i.3x3-varied-1024x768`, in release mode with warm
cache, concurrency 1, one warmup, three measured iterations, two samples, and a
whole-workflow boundary through `tobytes()`.

| Variant | Pillow median | CPU median | Pillow / CPU speedup | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| Base | 10.664 ms | 17.442 ms | 0.61× | 6.767 ms | 7.237 ms |
| Typed-view candidate | 9.767 ms | 7.553 ms | 1.29× | 6.004 ms | 5.329 ms |

The typed-view candidate passed exact Pillow parity for
`PIL.Image.Image.filter.nuanced.i-mode-cross-smooth-varied-1024x768` and
`PIL.Image.Image.filter.nuanced.i-mode-cross-smooth-int32-edges`. The benchmark
receipt identifies actual CPU, SIMD, and GPU execution respectively, terminal
completion for Rust profiles, and no fallback reasons.

The two earlier sparse-tap arithmetic candidates also passed the focused parity
cases, but CPU medians remained above Pillow: 12.006 ms vs 10.882 ms for the
first; 10.931 ms vs 10.151 ms and 10.783 ms vs 10.032 ms on a repeat for the
second. They are rejected. The typed-view candidate moved the CPU below Pillow
on this workload; this is one workload, not proof of all modes or sizes.

See the ARM64 report at
`docs/evidence/filter-i32-cross-smooth-cpu-arm64-20261018/README.md` for the
matching architecture result, parity-selector defect, and whole-campaign scope.
