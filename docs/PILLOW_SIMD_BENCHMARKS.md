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

The maintained benchmark declares 37 full-size, parity-gated workflow IDs:
32 `pipeline-op` workloads, the established RGB GaussianBlur case, and
getchannel in L, LA, RGB, and RGBA. Together they cover L, LA, RGB, RGBA,
CMYK, F, I, and YCbCr modes. Pillow and Pillow-SIMD each run in an isolated
environment because both packages provide the `PIL` namespace. pillow-rs
CPU/SIMD output is checked against regular Pillow. Pillow-SIMD is timed as a
separate source, and its output is checked directly against regular Pillow on
the same cases. A Pillow-SIMD row with different output remains visible with
its measured time, but its speed ratio is withheld.

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
is a completed, clean snapshot at revision
`600e96997427e9212d3d83023e80463a6f3d6179`, measured 2026-10-17. It contains
147 rows across 37 workflow IDs: 37 Pillow, 37 `python-cpu`, 36 `python-simd`,
and 37 Pillow-SIMD rows. The absent SIMD row is for
`pipeline-op.rotate.material-rgba-nearest-37deg-noise-fill-1024x768`, whose
workload contract declares only Pillow and `python-cpu`. The file-level
`clean` flag describes source state; it does not mean every compatibility
comparison passed.

Pillow-SIMD matched regular Pillow on 34 workflows. Three completed
measurements failed compatibility and are retained as diagnostics only:
`pipeline-op.grayscale.material-rgb-noise-1024x768`,
`pipeline-op.filter.material-l-noise-5x5-1024x768`, and
`pipeline-op.filter.material-la-nonmatching-5x5-1024x768`. Do not use their
Pillow-SIMD timings as speed comparisons. The remaining compatible rows are
matched-host evidence for their listed workloads; they do not close operations
or modes absent from the selected cohort and are not comparable with the
separate ARM measurements.
