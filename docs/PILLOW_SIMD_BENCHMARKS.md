# Pillow-SIMD x86 operation comparison

Pillow-SIMD is an x86-specific Pillow build, so its measurements are kept in a
separate cohort from the Apple ARM benchmark page. It is compared only with the
matching [Pillow 12.1.1](https://pypi.org/project/pillow/12.1.1/) release, on
the same x86 runner, using the Pillow-SIMD default SSE4 build
([12.1.1.post0](https://pypi.org/project/pillow-simd/12.1.1.post0/)). The
published table also includes pillow-rs CPU and SIMD timings from that same
host and exact workload set.

The benchmark currently covers 34 full-size, parity-backed individual
operation cases: all 29 current `pipeline-op` workloads in the benchmark
manifest, the established RGB GaussianBlur case, and getchannel in L, LA, RGB,
and RGBA. Together they cover L, LA, RGB, RGBA, CMYK, F, I, and YCbCr modes.
Pillow and Pillow-SIMD each run in an isolated environment because both
packages provide the `PIL` namespace. Each source must pass the same
exact-output parity cases against pillow-rs before its timing is published.

The [Pillow-SIMD project](https://github.com/uploadcare/pillow-simd) documents
additional accelerated operations, including convolution resizing, alpha
composition, premultiplied-alpha conversion, RGB-to-L, 3x3/5x5 filters, and
split. The current maintained benchmark contract does not
yet provide comparable full-size, parity-backed rows for those operations;
they are not represented by tiny setup-dominated smoke measurements here.
Add full-size parity-backed inputs to the normal operation contract before
including their results in this comparison.

No comparison is made across ARM and x86, across different Pillow versions, or
between SSE4 and AVX2 builds. Pillow-SIMD results appear only after the x86
benchmark and documentation workflows complete successfully.
