# F→L current-source execution receipt — 2026-10-16

This run adds an execution receipt to the existing eager F→L conversion path. It does not add a new pixel algorithm or establish an attributable before/after speedup.

## Results

- Focused parity: `PIL.Image.Image.convert.nuanced.mode-audit-F-to-L-17x3`, CPU, 1 selected and 1 passed (`migration-parity-49eb91b2d92b40be82bed12d08fe38fb`). This varied 17×3 edge case verifies the operation contract but is not the benchmark's input.
- Benchmark: `pipeline-op.convert-mode-f-to-l-1024x768`, CPU only, warm release, whole workflow (image setup, `convert("L")`, and `tobytes()`), 5 warmups and 100 samples at concurrency 1 (`migration-benchmark-b45f29655f634d33ae933884a765fba4`). Pillow median was 0.717896 ms and target CPU median was 0.256667 ms (2.80×); means were 0.725286 ms and 0.263087 ms (2.76×).
- The benchmark workload's correctness gate is `successful_execution`; its generated parity report selected zero cases. The separate 17×3 parity case does not establish byte parity for the benchmark's constant 1024×768 input. Keep this as a current-state diagnostic, not parity-gated threshold credit.
- The target receipt reports requested and actual backend `cpu`, terminal completion, no fallback, one `Convert` operation, and one output allocation of 786,432 bytes. The receipt is captured separately from the 100 timing samples. The measured workflow includes Python `tobytes()` materialization.
- Pillow/CPU subject rows are recorded in `docs/evidence/performance-optimization-local-runs.csv` (7,956 data rows). The clean published operation matrix remains unchanged.

## Identity

- Host: Apple ARM64, macOS; Pillow 12.2.0; Python 3.12.
- Git revision `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`, dirty worktree; default release build produced the target extension.
- Target extension SHA-256: `7bfbf7774bcf5d1d72413c5d74395154f7d50457abe13d4b780141fdc9e18afb`.
- `pillow-rs/src/ops/convert.rs` SHA-256: `22452c29cff4630cc5ef8022ce74501457bfefb0162b3cfc801387b67664d7c7`.
- Benchmark report SHA-256: `8b690caf8341812e9d6868b3a71d0a58c1353e731f467fb3ce6d403d513b3ca8`.
- Parity report SHA-256: `aa8698f636e477604d9c4023cf6748a2a13ed23b7f661a9c8f360bf8bc40ecaa`.

## Interpretation

The eager path shares the materialized F input and converts directly to an L buffer; it does not enter the deferred CPU adapter. The telemetry wrapper runs only when telemetry is enabled, serial CPU is selected, and the parallel feature is disabled. This makes the executed work visible without changing the normal non-telemetry pixel path. The measured ratio is not attributed to that wrapper or to the pre-existing source-sharing change. No SIMD or GPU result was collected here. See Kata issue `pillow-rs#vdj3` for the x86 audit and cast-only codegen hypothesis.

All reports, execution attestations, producer sidecars, and raw samples are retained beside this note. No full campaign, coverage, commit, or push ran.
