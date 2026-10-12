# Pillow-SIMD x86 operation comparison

Pillow-SIMD is an x86-specific Pillow build, so its measurements are kept in a
separate cohort from the Apple ARM benchmark page. It is compared only with the
matching [Pillow 12.1.1](https://pypi.org/project/pillow/12.1.1/) release, on
the same x86 runner, using the Pillow-SIMD default SSE4 build
([12.1.1.post0](https://pypi.org/project/pillow-simd/12.1.1.post0/)). The
published table also includes pillow-rs CPU and SIMD timings from that same
host and exact workload set.

Keep regular Pillow fixed as the conformance oracle for both timed source
cohorts. The timed source is not the oracle. In the 2026-10-10
[Pillow-SIMD workflow run 38055331119](https://github.com/appunni-m/pillow-rs/actions/runs/38055331119),
the runner used each source cohort as `MIGRATION_ORACLE_PYTHON`. That made the
Pillow-SIMD cohort compare its targets against Pillow-SIMD instead of regular
Pillow and produced four failures across the L-mode 5×5 Filter and large RGB
Grayscale cases under CPU and SIMD. This was a benchmark-oracle defect; the
runner now pins regular Pillow for target parity and measures Pillow-SIMD
compatibility separately, withholding ratios for incompatible output. Focused
arm64 CPU and SIMD reproductions pass 2/2; original x86 behavior remains
unverified. Preserve those parity checks when changing cohort setup.

The benchmark currently covers 34 full-size, parity-backed individual
operation cases: all 29 current `pipeline-op` workloads in the benchmark
manifest, the established RGB GaussianBlur case, and getchannel in L, LA, RGB,
and RGBA. Together they cover L, LA, RGB, RGBA, CMYK, F, I, and YCbCr modes.
Pillow and Pillow-SIMD each run in an isolated environment because both
packages provide the `PIL` namespace. pillow-rs CPU/SIMD output is checked
against regular Pillow. Pillow-SIMD is timed as a separate source, and its
output is checked directly against regular Pillow on the same cases. A
Pillow-SIMD row with different output remains visible with its measured time,
but its speed ratio is withheld.

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

The currently published
[`pillow-simd-benchmark.json`](https://appunni-m.github.io/pillow-rs/assets/pillow-simd-benchmark.json)
contains 9 workload IDs, while this repository declares 34. Its rows all pass
their recorded parity and backend checks, but the snapshot is marked dirty and
does not demonstrate the complete declared workload set. Treat those rows as
matched-host diagnostics only; do not use the snapshot to close the full x86
comparison category or to compare against the separate ARM measurements. The
page is refreshed only by a later successful x86 benchmark and documentation
workflow.
