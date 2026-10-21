# YCbCr grayscale CPU baseline evidence (2026-10-19)

This folder retains one focused public-call comparison for grayscale on a 1024×768 YCbCr noise image. It is scoped to the ARM64 macOS runner and the source snapshot named below.

- Workload: `pipeline-op.grayscale.material-ycbcr-noise-1024x768`
- Parity case: `PIL.ImageOps.grayscale.nuanced.performance-material-ycbcr-noise-1024x768`
- Oracle: Pillow 12.2.0, CPython 3.12.13
- Target: pillow-rs 12.2.2, `python-cpu`, source revision `418d5f0cde51ca64a97cbd007ea731da72d68abf` (working tree was dirty)
- Host: macOS 15.7.7, ARM64, runner ID `lazys-macbook-pro-arm64`
- Boundary: warm `call` + `observe-result`, with materialized output; 100 timing samples per subject
- Median: Pillow 0.3502705 ms; pillow-rs CPU 0.1701665 ms; 2.06× Pillow
- Execution receipt: one `Grayscale` operation on CPU, zero fallback, one mode conversion, one host allocation (786432 bytes)
- Parity: the one selected comparison passed
- Build receipt: `build-witness-v3.json` completed with both workspace crates fresh; artifact SHA-256 `eb13c90ddbfea514b1c07a4503b2f32b1499cbe9de1ec8f55d3d7478c0431b8f`. `RUSTFLAGS=-C link-arg=-undefined -C link-arg=dynamic_lookup` supplies PyO3's required macOS extension linker arguments for this direct Cargo witness build.

`direct-operation-link.json` binds the baseline benchmark, parity, report attestations, producer companions, manifest, selected inputs, and build witness for generated-baseline admission. `direct-operation-candidate-link.json` binds a fresh repeat used to exercise the baseline checker. The checker accepted that distinct candidate under strict policy: CPU median 170.166 → 64.542 µs (−62.071%), p95 342.083 → 265.750 µs (−22.314%), with the same verified runtime, producer, build, and workload identities. Pillow co-measured at 350.271 µs for the baseline and 140.209 µs for the candidate, leaving relative speedups stable at 2.06× and 2.17×. Build attempts `build-witness.json` and `build-witness-v2.json` are retained as failed diagnostics; they are not used as proof.

The generated DuckDB channel uses strict target policy and is tied to a dirty working tree; it is not evidence for other modes, sizes, architectures, SIMD, or GPU. The result follows the repository evidence layout `docs/evidence/<operation-family>/<workload>-<profile>-<architecture>-<date>/`.
