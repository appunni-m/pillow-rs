# Rayon parallel execution audit

Audit date: 2026-09-30. This is a point-in-time record of verified Rayon/backend
findings, fixes, and remaining questions. Check the current source before using
its call-site references as present-day status. The active feature decisions are
kept in [CLAUDE.md](../CLAUDE.md).

## Required execution identities

- **CPU** is the serial CPU execution profile.
- **Parallel CPU** is CPU work scheduled by Rayon and must be selected and
  benchmarked as its own opt-in profile.
- **SIMD** is vector-kernel execution without Rayon scheduling. The current
  adapters use portable `wide` vectors; source-authored ISA-specific kernels
  have not been found, so do not claim architecture-specific specialization.
- **GPU** performs device work without Rayon in its GPU execution path,
  including host staging and result materialization.

Every performance record must say which build features and actual backend ran.
Parity remains identical across these execution identities.

## Confirmed problems

### Default feature propagation mixed all normal Python work with Rayon

Before this audit, `pillow-rs/Cargo.toml` listed `parallel` in core defaults.
`pillow-rs-py/Cargo.toml` depended on core with default features, so the ordinary
Python extension enabled Rayon. The runtime called that execution **CPU** or
**SIMD**, neither of which exposed whether Rayon ran. JavaScript already disabled
core defaults and did not enable `parallel`.

The defaults are now `gpu` and `image-codecs-all`; `parallel` has been removed.
The Python crate now has an empty default feature set and an explicit
`parallel = ["pillow-rs/parallel"]` opt-in. This fixes the default propagation.

### SIMD execution schedules Rayon work

Before the backend split, `pillow-rs/src/compute/pool_simd/ops/adapters.rs`
contained 39 production Rayon expansions (36 row helpers and 3 block
collectors). They are now replaced by local serial row/block helpers, so those
adapters do not schedule Rayon even when `parallel` is compiled. Their CPU
counterparts retain Rayon scheduling in the CPU operation modules. The affected
operation families are:

| Operation family | Rayon call sites / implementation areas |
| --- | --- |
| Paste and channel expansion | `native_paste_apply` (four mode-specific paths), `simd_paste_rgb_to_rgba`, `native_rgb_to_rgba_bytes` |
| Point transforms and LUTs | `apply_native_rows`, `native_lut_map_rows`, `native_cmyk_grayscale_bytes` |
| Chops and blends | native Multiply/Screen blend rows, SoftLight, fused Multiply→Screen, Sharpness blend, AlphaComposite |
| Layout transforms | tiled and collected transpose paths, including the row-cache block collectors |
| Filters | Filter3x3 byte paths, Filter5x5 byte/I paths, horizontal/vertical rank filters, byte/float order-statistic filters, blur rows and luma vertical blur |
| Resize and reduction | native reduce; SIMD resize and boxed Fit horizontal/vertical passes |
| Alpha writes | constant PutAlpha and mask-based PutAlphaData |

Representative implementations are at `adapters.rs:2165-2486` (paste),
`7955-8427` (native transforms/LUT), `12078-12691` (transpose),
`15213-17583` (filters and blur), `21705-22227` (resize/Fit), and
`27543-28136` (alpha operations). The previous feature-enabled behavior was a
SIMD+threaded hybrid; new SIMD receipts no longer include Rayon scheduling from
these adapters.

The Rayon scheduling branches were replaced with local serial row/block
helpers. That preserves the existing vector-lane calculations while removing
thread scheduling from SIMD receipts. This does not itself prove that a
portable `wide` kernel is architecture-specific; keep that distinction visible
in backend claims and benchmark labels.

The source-level architecture audit found no `std::arch`/`core::arch`
intrinsics, architecture-specific runtime dispatch, or `#[target_feature]`
functions in the adapters. They use portable `wide` vector types. There are
compile-time FMA/NEON gates around a few exact arithmetic helpers (for example
`adapters.rs:10347`, `13624`, and `26189`), but these do not provide separate
architecture-specific kernels for the listed operations. Under the requested
rule, retain a SIMD label only where source-level architecture-specific vector
code is actually present; otherwise classify the operation as CPU, and any
Rayon-scheduled variant as Parallel CPU.

The transpose collectors were a special case: they combined Rayon scheduling
with a row-cache algorithm that disappeared when `parallel` was disabled. The
block collector now runs serially in the SIMD adapter when that feature is
compiled. Compare it against CPU Parallel CPU transpose at representative
geometries before deciding whether the cache algorithm belongs in CPU code.

### GPU readback used host Rayon

Before this audit, `pool_gpu::rgb_from_packed_readback` used `par_rows_mut!` to
discard the alpha byte from mapped RGBA transport for images at or above
256 Ki pixels. The helper is called from GPU readback, so a GPU request could
schedule host Rayon work and include it in reported GPU latency. This Rayon path
has now been removed; RGB materialization walks the mapped rows serially. Keep
the GPU path Rayon-free. If host materialization is later vectorized, measure it
without introducing thread scheduling into the GPU profile.

### CPU masked paste bypassed the approved Rayon helpers

`pool_cpu/ops/effects.rs::paste_native_masked` called `rayon::par_chunks_mut`
directly, despite `par.rs` declaring the row macros to be the approved path.
The loop scheduled every destination row and then returned early for rows
outside the clipped paste. It now uses `par_rows_mut!` over the affected row
slice only and maps each local row to its destination coordinate. This removes
the raw Rayon exception and avoids scheduling untouched rows.

### Backend names and benchmark profiles did not expose threading

`compute::Backend` has only `Cpu`, `Simd`, and `Gpu`; its CPU variant is
described as scalar even though feature-enabled CPU kernels use Rayon. Keep the
runtime executor name as `cpu`; benchmark profile identity now separates
`python-cpu` from `python-parallel-cpu`. The added
`make migration-parity-benchmark-parallel-cpu` target builds the extension with
the explicit Rayon feature, verifies that feature at runtime, requests only
CPU, parity-gates CPU-applicable workloads, and writes separate
feature-stamped results. The default CPU/SIMD/GPU campaign remains on default
features.

The benchmark, parity receipt, and generated manifest used to hard-code
`features: ["all-features"]`, although `make build-parity` invokes ordinary
`maturin develop` without `--all-features`. They now name the Python and core
default feature profiles. The manifest and result identities now declare the
separate Parallel CPU feature set.
The separate coverage receipt generator also has an `all-features` label; it
was intentionally left untouched because this pass does not run or modify
coverage. Verify that label against the coverage build before using any such
receipt.

### Rayon policy comments claimed an absent CI check

`par.rs` used to say that CI ran `scripts/check_parallelism.sh`; no such script
exists and no workflow invokes it. The false enforcement claim was removed.
Clippy's `--all-features` lane does not prove backend separation. The Makefile
now has explicit default and Parallel CPU builds, and the benchmark verifies
that the selected profile matches the compiled feature. A source-level CI guard
against adding Rayon to SIMD or GPU code remains useful follow-up work.

### Parallel helper macro surface contains unused/incomplete helpers

`par_pixels!` and `par_tiles!` have no production call sites. `par_pixels!`
documents `$idx` as a byte offset but computes `row_start + x`; for multichannel
pixels the byte offset must also multiply `x` by the channel count. The example
can therefore produce overlapping pixel addresses if used as written.
`par_tiles!` accepts `$data` but does not use it, and its closure receives only
coordinates and a unit value rather than a bounded tile view. It cannot by
itself provide the safe tile ownership its documentation promises. Either
implement and test these contracts before use or remove the unused helpers.

The macros are `#[macro_export]` while expanding to unqualified `use rayon::...`.
If consumers invoke these macros, they need a direct Rayon dependency in their
own crate. Treat them as internal implementation details or provide a deliberate
crate-anchored public API before advertising them downstream.

### Performance guidance mixes vectorization with thread scheduling

`docs/PERFORMANCE_SIZE_CUTOFF_AUDIT.md` groups CPU and SIMD serial/parallel
cutoffs, recommends a shared CPU/SIMD scheduling helper, and discusses SIMD
transpose collectors as SIMD policy. Rewrite this around CPU scheduling versus
SIMD kernel work: thresholds should estimate the actual CPU work and memory
traffic, not vector width. Historical entries in
`docs/PERFORMANCE_CAMPAIGN.md` that measured the feature-enabled SIMD paths need
to be labeled as threaded SIMD hybrids unless their exact feature receipt proves
otherwise. Do not relabel old timing as serial SIMD without rerunning it.

## Remaining decisions

1. Benchmark transpose and other large SIMD adapter families against both
   serial CPU and Parallel CPU with identical inputs. Keep only measured wins;
   do not infer speed from Rayon call counts or module names.
2. Decide whether portable `wide` kernels satisfy the project's SIMD naming
   contract. If the contract requires authored ISA-specific dispatch, migrate
   unqualified kernels to CPU and retain a SIMD route only for specialized
   implementations.
3. Add a source-level CI guard that prevents Rayon from reappearing under
   `pool_simd` or `pool_gpu`.
4. Audit unused public Rayon helper macros (`par_pixels!`, `par_tiles!`) and
   either correct their documented ownership/indexing contracts or remove
   them before external use.
5. Run the default and explicit Parallel CPU parity profiles separately and
   keep their feature identities in every receipt. GPU timing must include
   device dispatch, transfers, and host materialization, without Rayon.

## Checkpoint at the audit date

Core and Python defaults are Rayon-free. SIMD adapters contain no Rayon row or
block scheduling, GPU RGB readback is serial, and CPU masked paste uses the
approved helper over only the copied rows. The Python binding exposes the
compiled `parallel` feature; the Parallel CPU target builds that feature, runs
CPU-only parity-gated workloads, and writes a separate `python-parallel-cpu`
benchmark identity. The quick benchmark measured four RGB pipeline workloads
against Pillow; its latency/throughput ratios are evidence for that cohort only,
not a claim about every operation. Full affected backend parity and a CI guard
remain release gates before treating this audit as fully closed.
