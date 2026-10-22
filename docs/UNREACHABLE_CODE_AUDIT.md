# Unreachable, unused, and legacy code audit

**Status: in progress.** This ledger records evidence for investigation; it does
not authorize code deletion. A static reference gap is not proof that a path is
dead. Track the complete audit in Kata issue `pillow-rs#mmqd`.

## Method and scope

The generated operation/source crosswalk links the parity manifest and its
declared parity and benchmark inputs to Python bindings, PyO3 registration,
pipeline variants, Rust module declarations, re-exports, and literal include
references. Generate it with
`make operation-source-crosswalk`; its ignored report is
`target/operation-source-crosswalk.json`. Run
`make operation-source-crosswalk-test` for the focused parser and completeness
contracts. The generator's source parser and Rust module inventory are
implemented in `scripts/source_crosswalk/`.

The crosswalk is a static index. It does not evaluate `cfg` or Cargo feature
sets, expand macros or procedural macros, inspect build output, establish
runtime loading, or prove that a test or parity case executes a specific
implementation. Operation mappings establish discoverability, not coverage or
execution. Items below remain review candidates until those boundaries are
traced. This audit uses no coverage data and has not deleted or simplified
code.

## Current operation-to-family and fixture snapshot — 2026-10-21

The crosswalk schema is now `pillow-rs/operation-source-crosswalk@10`. It was
generated from dirty revision `418d5f0cde51ca64a97cbd007ea731da72d68abf` using
372 source and fixture inputs. The final input fingerprint is recorded in the ignored
`target/operation-source-crosswalk.json`:
`a2db2571b8edb79bce71484704fe0749ffe91df8e2981cbf52c31b8a0199dd39`.
The generated report SHA-256 is
`0010b74575fedd5b778bc0c0a6d889173de4fbb9393899a67e0208cecfd06ca1`.

| Generated measure | Count | Interpretation |
| --- | ---: | --- |
| Manifest public operations → Python bindings | 212 / 212 | The public manifest denominator remains complete. |
| Declared parity cases → public operations | 17,286 cases cover all 212 operations | These are indexed fixture declarations, not evidence that a runner selected or executed them. |
| Declared benchmark workloads → public operations | 996 workloads cover 209 operations | Three public operations have no indexed benchmark workload and remain review-only: `PIL.Image.Image.toqimage`, `PIL.Image.Image.toqpixmap`, and `PIL.ImageFont.MAX_STRING_LENGTH`. |
| Unmapped fixture operation references / unresolved benchmark parity references / unknown input kinds | 0 / 0 / 0 | Every indexed operation, case reference, requirement reference, and workload input kind resolves under the static mapping rules. |
| Python native calls → registered PyO3 declarations | 157 / 157 | Includes direct checked-in callback implementations and callback argument calls, as well as manifest wrappers. |
| Direct PyO3 crate-root calls → core API routes | 102 / 102 | Literal `pub use` chains and crate-root `pub fn` definitions resolve. |
| PyImage/PyDraw receiver calls → unique core methods | 85 / 85 | Explicit field types, direct calls, and clone-derived local aliases resolve. |
| Reached core methods / direct `PipelineOp` references | 173 / 44 | Direct receiver helper chains connect to the variant → registry → backend graph. |
| Reached core helper functions / helper `PipelineOp` references | 63 / 11 | Unique unqualified same-module call targets are followed. |
| Rust source files reached from public operations | 39 | Directed static links reach core and backend files; this is not runtime proof. |
| Rust source files with graph edges but no static public-operation route | 3 | CPU Filter, SIMD Filter, and SIMD GaussianBlur have possible routes through `Image.filter` callbacks; the runtime callback target is not proven. |
| Rust source files without a static operation-graph edge | 92 | Each file is listed with a review reason; these may be support, conditional code, tests, or indirect/dynamic paths. |
| Source paths in the explicit operation-graph review queue | 95 | Includes the 92 files without a direct edge and the 3 files without a static public route; each retains its distinct status and is review-only. |
| Source paths with possible dynamic-callback routes | 3 | These paths are separately identified as possible routes and remain review-only; they are not promoted to statically reached paths. |
| Rust family/layout findings in the crosswalk | 7 | Five production adapters/kernels and two test modules are flagged for path-role review. |
| Unreferenced GPU shader asset candidates | 4 | Static loader search found no source route; each still requires its documented runtime/build and history review. |
| Unresolved module edges, native-call registrations, and pipeline variants | 0 | A zero static count does not prove runtime reachability. |

For example, `PIL.ImageOps.grayscale` now links through the `ops_grayscale`
PyO3 function and `pillow_rs::imageops_grayscale` to `ops/imageops.rs`, its
direct `PipelineOp::Grayscale` reference, the registry key, and CPU/SIMD family
modules. The receiver routes also connect ImageDraw operations, Brightness and
Sharpness enhancement paths, and `PIL.Image.merge` through its core helper to
the Merge registry route. Every Rust source row carries the shared structure
classifier's role and a family identifier where the path exposes one. PyO3's
literal `_apply` callback call records 11 checked-in ImageFilter methods as
dynamic candidates. Direct calls made by their `rust_image` callback argument
are also indexed as candidate edges. The possible-route closure now identifies
CPU Filter, SIMD Filter, and SIMD GaussianBlur source as reachable only through
those dynamic candidates. These do not count as static public routes because
the runtime filter object is not fixed by the operation source path. No route
proves runtime dispatch, backend execution, parity or performance; no review
flag authorizes deletion.

The generated report exposes the 95 source paths without a statically proven
public operation route at `review_queues.operation_graph_source_without_public_route`.
The queue preserves each path's `no_direct_static_edge`,
`static_graph_edge_without_public_operation_route`, or
`possible_route_via_dynamic_python_callback_candidate` status, family/layout
data, and per-file reason. The 3 possible callback routes also appear in
`review_queues.operation_graph_possible_dynamic_callback_routes`. These are
review queues, not dead-code lists; the source inventory remains authoritative
and no deletion follows from a queue entry.

The source parser previously rejected a valid method whose return type
contained `[u8; 3]`, because it treated the array-length semicolon as a
declaration terminator. It now tracks signature delimiter nesting and has a
focused regression contract for array return types and receiver aliases.

Focused parser/completeness verification passed 31/31 against the 372-input
snapshot above, and the focused structure audit contracts previously passed
17/17. The refreshed 2026-10-21 whole-tree structure inventory has 6,692 paths,
0 unclassified paths, and 11 review candidates (the seven crosswalk family/
layout findings plus four fixture manifests whose domain placement needs
review). Its ignored report is `target/implementation-structure-inventory.json`
with SHA-256 `8241a6404a2506d5036329a670a123aecb45b4904540fdc696bfa5653bd40097`.
One selected ARM64 parity and whole-workflow benchmark pair guarded the PutPixel
family move; no full image campaign or coverage ran, and no source was deleted.

## Historical generated inventory snapshot — 2026-10-19

Generated on 2026-10-19 from revision `418d5f0cde51ca64a97cbd007ea731da72d68abf`
with a dirty worktree. The 307 crosswalk inputs have SHA-256
`a3c5cccef56890f5359968582278083efeb97edb43f268badeb87c2691af48f4`. The
manifest is `pillow-rs/tests/fixtures/manifest.yaml`, SHA-256
`ff0d8fa434e60ed844d9d3606396919e28e8b59bdcc590c80d0d73564ec5d885`. The host
was ARM64 and no Cargo features were explicitly requested. Runtime execution
and Rust module reachability are both reported as unproven.

| Generated measure | Count | Interpretation |
| --- | ---: | --- |
| Manifest public operations | 212 | All 212 map to a Python binding in this snapshot. |
| Discovered Python native calls | 140 | All 140 map to a registered PyO3 declaration. |
| Pipeline variants | 84 | All 84 map to registry keys. |
| Rust source files | 127 | Across the three workspace packages and declared roots. |
| Resolved Rust module edges | 121 | Static module edges; conditional attributes are recorded, not evaluated. |
| Literal include references | 174 | Includes found by the source parser. |
| Files without direct operation edges | 108 | May be support, tests, or conditional paths; not a dead-code count. |
| Static unreached Rust source candidates | 0 | Does not prove runtime reachability. |
| Unresolved module edges | 0 | Does not account for macro expansion or generated paths. |
| Shader asset review candidates | 4 | Listed below; none is classified as dead. |

These counts belong to this dirty source snapshot. Regenerate the ignored report
before relying on them for a later revision.

## Current family/layout review — 2026-10-20

The current crosswalk emits seven `rust_source_family_layout_review` findings.
These are path-role signals from the shared structure classifier, not dead-code
findings. I traced their declarations and callers in the current Rust tree:

| Path | Reachability evidence | Classification and disposition |
| --- | --- | --- |
| `pillow-rs/src/compute/pool_simd/ops/adapters/cmyk_packing_tests.rs` | Included only under `#[cfg(all(test, target_arch = "x86_64"))]` by `adapters.rs:19-20`. It checks the CMYK conversion helper used by `adapters/convert.rs:52` and the SSSE3 packer. | Test-only x86 guard, not a production path. Keep the coverage; review whether it can live under the Convert family's test tree without obscuring private helper ownership (`rxcj`). |
| `pillow-rs/src/compute/pool_cpu/ops/color_convert_allocation_tests.rs` | Declared through `#[cfg(test)]` and `#[path]` by `pool_cpu/ops/color.rs:190-192`; its tests exercise native LA conversion allocations and alpha semantics. | Test-only CPU conversion contract, not a production path. Keep the assertions; move the module beside a family only when conversion ownership is split. |
| `pillow-rs/src/compute/pool_simd/ops/adapters/native_thumbnail.rs` | Declared/re-exported by `adapters.rs:30,45-48`; the registry binds Thumbnail to `adapters::simd_thumbnail` at `compute/registry.rs:2939`. The SIMD pipeline also calls its PutPixel→Thumbnail fusion helpers at `pool_simd/mod.rs:265-290`. | Live family adapter and fusion path. Keep both routes; review the family name and split orchestration from mode kernels under `rxcj`. |
| `pillow-rs/src/compute/pool_simd/ops/adapters/native_thumbnail/float.rs` | Declared by `native_thumbnail.rs:17`; its F-mode reduction is selected by the registered `simd_thumbnail` route at `native_thumbnail.rs:431`. | Live F-mode kernel on the SIMD Thumbnail path. Keep it; place it under the family's explicit kernels/responsibility layer in a measured structural move. |
| `pillow-rs/src/compute/pool_simd/ops/adapters/native_thumbnail/integer.rs` | Declared by `native_thumbnail.rs:18`; I-mode reduction and resampling are selected at `native_thumbnail.rs:420,433`. | Live I-mode kernel on the SIMD Thumbnail path. Keep it; place it under the family's explicit kernels/responsibility layer in a measured structural move. |
| `pillow-rs/src/compute/pool_simd/ops/adapters/resize_i32.rs` | Declared and imported by `adapters.rs:32,52`; the registered Resize adapter selects it for mode I at `adapters.rs:27780`, and Thumbnail reuses it at `native_thumbnail.rs:508-521`. | Live mode-specific Resize implementation shared with Thumbnail. Keep it; align the I mode with the Resize family under `rxcj`, preserving both direct routes. |
| `pillow-rs/src/compute/pool_simd/ops/adapters/resize_i32/neon.rs` | The parent declares it only for `target_arch = "aarch64"` plus `target_feature = "neon"` (`resize_i32.rs:20-21`); the I32 horizontal planner selects its NEON path under that cfg. | Supported architecture-specific implementation, not an x86-host dead path. Keep it; move it with the I-mode Resize family when its exact cfg and SIMD receipt can be preserved. |

This pass traces five production adapters/kernels and identifies two test-only
modules. The earlier CPU Filter path findings for `rgb_blur_ring.rs` and
`uniform_box.rs` no longer appear in the current classifier: both now sit under
the CPU Filter family, and their direct call paths remain live. This does not
prove runtime admission for every input or that a parity/benchmark selector
reaches each branch. The seven rows above remain structural review candidates,
not removal findings. Since this pass, the SIMD GaussianBlur orchestrator moved from the
large `adapters.rs` facade to
`pillow-rs/src/compute/pool_simd/ops/adapters/gaussian_blur.rs`. The module
resolves to the `simd_gaussian_blur` registration and the shared structure
classifier marks it `matches_family_orchestrator_shape`. Its function body and
shared blur kernels were kept intact. The selected 1024×768 RGBA SIMD workload
passed exact parity with an actual SIMD receipt after the move; the run is
recorded under `docs/evidence/gaussian_blur/current-source-simd-arm64-20261020/`.
The new adapter is one of the three source files with a static graph edge but
no complete directed public-operation route; that is a crosswalk review flag,
not evidence of an unused path. The current source crosswalk reports 92 Rust
files without a static operation-graph edge; that is an unmapped-source review
count, not evidence that the files are unused.

The RGBA PutPixel batch is now under `adapters/put_pixel.rs` and
`adapters/put_pixel/kernels/rgba_batch.rs`. Its source caller is visible in
`pool_simd/mod.rs`, and the selected public pipeline plus focused `SimdPool`
test verify its behavior. The crosswalk still does not connect this composed
batch call to a public-operation route, so those family files remain listed as
unmapped review candidates. Extending static tracing across this internal
pipeline dispatcher remains open; the current evidence does not classify the
live kernel as unreachable.

## Findings

### GPU-SHADER-001 — `autocontrast_remap.wgsl`

- **Category:** suspected superseded implementation (3); string/registration
  route still needs review (5).
- **Static evidence:** the generated crosswalk reports
  `unreferenced_shader_asset_candidate`. A repository search found the file
  itself and indirect mentions in another shader's comment, but no Rust
  registration, `include_str!`, or `include_bytes!` reference to this asset.
  Its history starts with the mode-aware shader set (`59c8de980`).
- **Current route:** `pool_gpu/mod.rs` resolves the autocontrast LUT stage
  from `autocontrast_cutoff.wgsl`; the histogram pipeline then selects
  registered packed or generic point shaders. The current Equalize-only
  `equalize_remap_l_packed.wgsl` path is separate. These names differ from
  this asset, but do not prove that dynamic or generated selection is
  impossible.
- **Tests and parity:** operation-level GPU checks include
  `gpu_packed_luma_point_admits_lut_and_lowered_l_autocontrast` and
  `rgb_autocontrast_packed_point_matches_cpu_for_every_rgb_tail`. The manifest
  also has autocontrast mode cases. None is statically linked to this asset;
  they exercise the registered route.
- **Disposition:** investigate whether this is retained history or has a
  platform/build-time loader. Keep it until that check is complete; do not
  treat the parity tests as evidence for this shader.

### GPU-SHADER-002 — `equalize_remap.wgsl`

- **Category:** suspected superseded implementation (3); string/registration
  route still needs review (5).
- **Static evidence:** the generated crosswalk reports
  `unreferenced_shader_asset_candidate`. No Rust registration or literal
  include of this filename was found. The file dates to the mode-aware shader
  set (`59c8de980`).
- **Current route:** `pool_gpu/mod.rs` derives histograms/LUTs with the
  registered equalize histogram and CDF shaders, then selects packed or
  generic point-operation shaders. The old `equalize_remap.wgsl` filename is
  absent from that route.
- **Tests and parity:** GPU operation checks include
  `rgb_equalize_packed_histogram_matches_cpu_for_every_rgb_tail` and
  `luma_equalize_packed_histogram_matches_cpu_for_every_luma_tail`. Manifest
  mode cases cover Equalize behavior, but the crosswalk does not link a case to
  an individual shader file.
- **Disposition:** investigate build/runtime loading and refactor history;
  retain pending that evidence.

### GPU-SHADER-003 — `getprojection_cmyk.wgsl`

- **Category:** suspected superseded implementation (3); GPU operation routing
  requires confirmation (5).
- **Static evidence:** the generated crosswalk reports
  `unreferenced_shader_asset_candidate`. No literal reference to this filename
  was found. Git history shows it was added alongside
  `getprojection_atomic.wgsl` in `88d414bdb`.
- **Current route:** `execute_cmyk_projection_terminal` delegates to the
  projection terminal implementation, which resolves
  `getprojection_atomic.wgsl` (`pool_gpu/mod.rs:23028-23039,
  23279-23281`). This suggests the CMYK shader may have been superseded, but
  the source inventory does not prove that no other loader exists.
- **Tests and parity:** the parity input includes CMYK cases such as
  `PIL.Image.Image.getprojection.parity.material-cmyk-1024x768`,
  `PIL.Image.Image.getprojection.parity.sparse-cmyk-1024x768`, and
  channel-only variants. They exercise the public operation, not this
  unreferenced asset specifically.
- **Disposition:** trace all GPU build/runtime paths and the paired shader's
  history; retain pending review.

### GPU-SHADER-004 — `resize_bilinear.wgsl`

- **Category:** suspected superseded implementation (3); shader-name comments
  and operation dispatch require review (5).
- **Static evidence:** the generated crosswalk reports
  `unreferenced_shader_asset_candidate`. No Rust registration or include of
  this file was found. Search hits outside the file are comments in
  `thumbnail.wgsl`, `contain.wgsl`, and `cover.wgsl` describing related
  sampling behavior; those comments are not references that load this asset.
  Its history starts with the mode-aware shader set (`59c8de980`).
- **Current route:** the registered resize path in `pool_gpu/mod.rs` uses
  `resize_convolution_h.wgsl` and `resize_convolution_v.wgsl` for filtered
  resize work. Other geometry operations have their own shaders. The old
  bilinear asset was not found in a current pipeline registration.
- **Tests and parity:** the GPU resize suite has operation-level tests for
  native typed and filtered paths. The crosswalk does not show any test that
  loads this shader by filename; the candidate is therefore not considered
  test-reachable based on current static evidence.
- **Disposition:** verify feature/target and generated loading paths, then
  classify as retained history or open a separate removal review. Do not delete
  it in this audit.

## Runtime-loader follow-up — 2026-10-19

The current GPU pipeline construction path was inspected after generating the
crosswalk snapshot above. `GpuExecutor::resolve_pipeline` takes a static shader
key, file label, and shader source (`pillow-rs/src/compute/pool_gpu/mod.rs`,
`resolve_pipeline`); it builds the pipeline from the supplied source string.
Current callsites provide that string with `include_str!`. No exact
`include_str!`, `include_bytes!`, shader registration, or source-name reference
to the four candidate filenames exists under `pillow-rs/src/compute/pool_gpu`.
There is no file-system shader loader on this path that could discover those
files by filename at runtime.

| Candidate | Current registered route observed | Assessment |
| --- | --- | --- |
| `autocontrast_remap.wgsl` | Histogram plus `autocontrast_cutoff.wgsl`, then packed or generic point shaders. | Source-only legacy candidate; no current pipeline route references it. |
| `equalize_remap.wgsl` | Histogram plus `equalize_cdf.wgsl`, then packed remap or generic point shaders. | Source-only legacy candidate; no current pipeline route references it. |
| `getprojection_cmyk.wgsl` | `execute_projection_terminal` resolves `getprojection_atomic.wgsl`. | Source-only legacy candidate; CMYK does not select this file on the inspected route. |
| `resize_bilinear.wgsl` | Filtered resize resolves the convolution H/V shaders; thumbnail/contain/cover use their own sources. | Source-only legacy candidate; comments naming it describe sampling semantics and do not load it. |

This closes the current static/runtime-loader question for these four shader
files. They are not in the present registered GPU pipeline path, but this does
not establish why they were retained or authorize removal. Keep them and their
crosswalk findings until a separate, reviewed cleanup decision records their
history and checks all supported build configurations. No code or test was
removed or changed as part of this audit.

## Open audit work

The four static GPU shader candidates have current loader-path evidence above,
and the seven current crosswalk family/layout findings have source routes or a
test-only classification in the bounded review above. Their history and any
separate removal decision remain open. The current snapshot still has 92 Rust
files without a direct operation-graph edge and three files with graph edges but
no static public-operation route; these need classification by module
ownership, callers, cfg/features, macro/generated paths, and test role. The
audit still needs compiler-proven dead items, private caller chains, refactor
duplicates, platform/feature paths, binding and callback routes, and
test/parity workflow selection. Static operation-to-binding edges do not prove
that every production path is selected by a test or benchmark.

No automatic cleanup is supported by this report. Each finding must end as
documented retained code, a focused test-selection repair, a further
investigation with missing evidence, or a separately reviewed removal task.

## SIMD Resize family path follow-up — 2026-10-22

The 2026-10-20 table above records the former I-mode Resize paths. The direct
and boxed I-mode kernels now live at
`pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/i32.rs`, with the
AArch64 NEON module beside them at `resize/kernels/i32/neon.rs`. The Resize
family entry remains registered as `adapters::simd_resize`, and native
Thumbnail continues to call the I-mode direct and boxed routes through the
family. This resolves the two old path-layout findings; it does not change the
historical crosswalk counts or claim runtime reachability for unmeasured cfgs.

## SIMD Resize dispatch follow-up — 2026-10-22

Direct and boxed Resize admission and mode routing, plus the zero-source helper,
now live in `pillow-rs/src/compute/pool_simd/ops/adapters/resize.rs`. The
registered family entry calls this local dispatcher. General byte, float, and
Luma16 pixel kernels remain in `adapters.rs` for later caller-traced moves.
This changes file ownership only; it does not alter the historical crosswalk
counts or infer reachability beyond the selected parity and benchmark paths.
