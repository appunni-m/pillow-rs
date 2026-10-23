# Pillow-SIMD x86 operation comparison

Pillow-SIMD is an x86-specific Pillow build, so its measurements are kept in a
separate cohort from the Apple ARM benchmark page. It is compared only with the
matching [Pillow 12.1.1](https://pypi.org/project/pillow/12.1.1/) release, on
the same x86 runner, using the Pillow-SIMD default SSE4 build
([12.1.1.post0](https://pypi.org/project/pillow-simd/12.1.1.post0/)). The
published table also includes pillow-rs CPU and SIMD timings from that same
host and exact workload set.

Keep regular Pillow fixed as the conformance oracle for both timed source
cohorts; the timed source is not its own oracle. Pillow and Pillow-SIMD run in
separate environments because both packages provide the `PIL` namespace.
Pillow-SIMD output is checked against regular Pillow on the same workloads.
Rows with incompatible output remain visible, but their speed ratios are
withheld.

The maintained benchmark declares 37 full-size, parity-gated workflow IDs:
32 `pipeline-op` workloads, the established RGB GaussianBlur case, and
getchannel in L, LA, RGB, and RGBA. Together they cover L, LA, RGB, RGBA,
CMYK, F, I, and YCbCr modes. pillow-rs CPU/SIMD output is checked against
regular Pillow; Pillow-SIMD is timed as a separate source.

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
is a completed, clean snapshot from [workflow run 39580258318](https://github.com/appunni-m/pillow-rs/actions/runs/39580258318),
measured 2026-10-21 at revision
`5db7197532eb08b37d748a78fb789545ab8aa577`. It contains 147 rows across 37
workflow IDs: 37 Pillow, 37 `python-cpu`, 36 `python-simd`, and 37 Pillow-SIMD
rows. The missing SIMD row is for
`pipeline-op.rotate.material-rgba-nearest-37deg-noise-fill-1024x768`, whose
workload contract declares only Pillow and `python-cpu`. SIMD is recorded only
when the workload selects that profile and the runner verifies SIMD execution;
a CPU fallback would be reported as CPU, not counted as SIMD. The absent row is
therefore a declared scope difference, not a failed SIMD measurement. The
snapshot's `clean` flag describes source state; it does not mean every
compatibility comparison passed.

Pillow-SIMD matched regular Pillow on 34 workflows. Three rows have different
output and are retained as diagnostics without speed ratios. Compatible rows
are evidence only for their listed workloads; they do not close operations or
modes absent from the cohort and are not comparable with the separate ARM
measurements.
