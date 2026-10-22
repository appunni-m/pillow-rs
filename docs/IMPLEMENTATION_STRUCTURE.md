# Repository-wide file structure standard

This is the repository-wide standard for first-party maintained files in
every area: Rust and binding source, tests, scripts and tools, documentation,
benchmark evidence, fixtures, generated inputs, shaders, and workflows.
It is the single source for repository path and file-placement rules. Other
guides may explain ownership, behavior, or commands and link here, but must not
define a competing layout. Before introducing a path this standard does not
cover, update the standard first; record a concrete, scoped exception when a
real package boundary requires one.
Ignored build outputs, virtual environments, dependency caches, and vendored
third-party trees are outside the layout contract. Keep real package and
lifecycle boundaries at the top level, then organize each owner's contents by
domain, coherent family or concern, and concrete responsibility. A mode,
architecture, or other variant is a final level only when it represents a
distinct supported path. Root manifests, project-wide configuration, and
genuinely standalone entry points stay at the root because they govern or
start the whole owner.

The standard keeps related work together without creating a file for every
helper or collecting unrelated work in large catch-all modules. Every new file
and every directory touched during structural work follows the same order.
This applies to every new maintained file and every directory created or
reshaped. Existing paths that do not yet fit are migration debt, not alternate
standards: align a touched area when the move preserves behavior and makes
ownership clearer. Record a concrete reason when it must stay put. Do not
mass-move files only to make the tree look uniform.

## The repository path standard

Use this order for maintained files and directories:

```text
<owner>/<domain-or-layer>/<family-or-concern>/<responsibility>/<mode-or-variant>
```

Before adding or moving a file, identify its owner, domain, family or concern,
and responsibility. Add a final mode or architecture level only if it selects
a distinct supported implementation. Omit levels with no real meaning; do not
compensate by using generic buckets such as `common/`, `misc/`, or `helpers/`.
The path must make clear both who owns the file and what role it plays.

The path segments have stable meanings across the tree:

- **Owner** says which crate, tool, or maintained subtree owns behavior, such as `pillow-rs`,
  `pillow-rs-py`, `pillow-rs-js`, `pillow-rs-parallel`, or a repository tool.
- **Domain or layer** says what the code does there, such as `ops`, `adapters`,
  `kernels`, `shaders`, `tests`, `inputs`, or `evidence`. Existing package and
  backend segments may be nested here when they represent real ownership.
- **Family or concern** uses one stable lowercase `snake_case` token for the
  coherent operation family, tool, document topic, test suite, or workflow
  group.
- **Responsibility** names a concrete role, such as an orchestrator, kernel,
  test, parity case, benchmark workload, or immutable result.
- **Mode or architecture** appears only when it selects a distinct supported
  path, such as `ycbcr`, `rgba`, `avx2`, or `neon`.

The hierarchy is uniform; filenames follow the conventions of their owning
language or artifact format. Rust and Python module stems use `snake_case`,
JavaScript follows the package's established module convention, documentation
uses the established Markdown naming convention, and stable case/workload IDs
remain governed by the migration manifest. Do not encode a second owner or
layer in a filename when the directory already states it. Keep the same family
identity traceable across these conventions instead of forcing every artifact
to have an identical spelling.

Do not reverse this order, make a second generic bucket in a package, or use a
file's location as a substitute for an explicit backend/profile selection. If
a concern is too small to need a directory, keep its focused module flat; once
it has distinct child roles, add the concern directory below its owner and keep
its name. Omit a level only when it has no real responsibility in that area;
never invent empty directories to force equal path depth. In Rust this is the
same module-tree pattern used by `grayscale.rs` and
`grayscale/kernels/ycbcr.rs`.

## One family name across operation layers

Use the same stable family name at every layer. A family groups public
operations that share a coherent algorithm and implementation contract. The
name should let a maintainer trace from the Python/JS API through core semantics,
backend selection, a mode/architecture kernel, and the parity/benchmark evidence
that covers it. Keep operation IDs and workload IDs authoritative in the
migration manifest; source layout is a traceability aid, not another API list.

The repository-level shape for image operations is:

```text
pillow-rs/src/ops/<family>.rs                         # core Pillow semantics
pillow-rs/src/compute/pool_<backend>/ops/adapters.rs  # backend facade
pillow-rs/src/compute/pool_<backend>/ops/adapters/
  <family>.rs                                         # family orchestrator
  <family>/kernels/<mode>[_<architecture>].rs        # focused hot loops
pillow-rs/src/compute/pool_gpu/shaders/<family>/      # matching GPU shaders
pillow-rs-py/src/lib.rs                               # native binding entry
pillow-rs-py/python/pillow_rs/<family>.py              # Python wrapper, if split
pillow-rs-js/src/lib.rs                               # JavaScript/WASM entry
pillow-rs-js/src/<family>.rs                          # binding module, if split
pillow-rs/tests/fixtures/inputs/<domain>/              # maintained parity/benchmark inputs
build/migration-parity/                               # ignored run products; outside this contract
docs/evidence/<family>/<experiment>-<backend>-<arch>-<date>/
```

## Applying the shared standard across repository owners

Top-level directories continue to mark real ownership boundaries such as the
Rust core, Python package, JavaScript package, migration tooling, docs, and
generated evidence. Inside those boundaries, keep the owner, domain/layer,
family, responsibility, and optional mode/architecture in the same order.
File depth follows the layer's real responsibilities, while ownership and
family identity stay consistent. A small family stays in one focused module;
non-image code follows the same order, such as `scripts/source_crosswalk/` for
one tool's parser stages or `pillow-rs/src/raster/traits/` for raster
contracts. Skip a level when it has no distinct work; do not invent empty
directories to make paths equally deep.

At repository root, preserve package and workflow ownership boundaries:

| Owner | Contents and internal rhythm |
| --- | --- |
| `pillow-rs/` | Image semantics in `src/ops/<family>.rs`; data/model domains such as `raster/`, `draw/`, and `font/`; backend implementations below `src/compute/pool_<profile>/`; tests beside internal code or in `tests/<suite>/`. |
| `pillow-rs-py/`, `pillow-rs-js/` | Host conversion and delegation by family where a module boundary exists; keep crate-root registration together until family-specific work warrants a module. |
| `pillow-rs-parallel/` | Companion-wheel packaging and release metadata for the optional Parallel CPU binding; it owns no image algorithms. |
| `scripts/` | Small standalone commands can remain one file; multi-stage workflows live in `scripts/<workflow>/` with an entry point and focused stage modules. Keep test modules beside the workflow they exercise. |
| `tools/` | Standalone maintained tools stay under a tool-specific owner and concern; ignored build products do not define source layout. |
| `.claude/` | Repository-local agents and skills stay grouped by workflow or domain, with examples and references beneath the owning skill. |
| `site-assets/` | Documentation-site assets stay grouped by site concern when a concern has multiple maintained files; small shared assets may remain at this owner level. |
| `pillow-rs/tests/fixtures/` | Public fixture manifest and input payloads grouped by contract/domain with stable IDs; generated outputs remain under ignored build directories. |
| `docs/evidence/` | Immutable results grouped by family and experiment identity; never replace older evidence. |
| `.github/workflows/` | One workflow per lifecycle purpose (CI, docs, benchmarks, release); shared behavior belongs in maintained scripts or composite actions rather than copied workflow bodies. |

Cargo examples stay as named targets in `pillow-rs/examples/`. Embedded assets
stay with their owning domain (`src/font/`, `src/data/`) or operation family
(`src/ops/lut_<family>.*`); do not create a generic asset bucket. Package
manifests and lifecycle metadata stay at their package roots.

Root manifests and project-wide configuration remain at the root because they
govern multiple owners. Ignored `build/`, `target/`, and cache products are not
maintained source; if a generated result is retained as evidence, group it by
its owning family and experiment without changing its stable identity.

Existing flat evidence bundles such as `docs/evidence/<family>-<experiment>-.../`
retain their immutable paths because campaign notes and local ledgers refer to
them. They are migration debt, not a second convention. Put all new evidence
under `docs/evidence/<family>/<experiment>-<backend>-<arch>-<date>/`; move an
old bundle only when its references are updated together.
The cross-family campaign ledger
`docs/evidence/performance-optimization-local-runs.csv` stays at the evidence
root as a shared index; it is not an experiment bundle.

The root ownership map remains the one in `docs/REPO_MAP.md`; this document
defines the internal path rhythm. Do not introduce a second top-level bucket
for operations or a parallel hand-maintained list of families.

| Layer | Placement rule |
| --- | --- |
| Core semantics | `pillow-rs/src/ops/<family>.rs`; family-only child modules live under `ops/<family>/` when needed. |
| Serial CPU and SIMD execution | Stay in the selected `compute/pool_cpu/ops/` or `compute/pool_simd/ops/` owner. Route through the pool's family entry point; put distinct mode/architecture loops under that family's `kernels/`. |
| Parallel CPU execution | Keep Rayon scheduling at the CPU family call site under `pool_cpu/ops/<family>.rs` and shared row/tile scheduling helpers in `pillow-rs/src/par.rs`. This is a separate execution profile, not a duplicate source owner: do not add `pool_parallel/` or copy kernels unless a distinct algorithm contract requires it. Keep the feature gate and profile identity explicit. |
| GPU execution | Keep family admission/dispatch with the GPU operation path and shaders under `pool_gpu/shaders/<family>/`. Keep pipeline selection and receipts explicit. |
| Python and JavaScript | Keep conversion and delegation in their package owners. Use the same family name for a dedicated wrapper/module; existing multi-family user-facing modules and crate-root registration remain valid when they are the natural boundary. |
| Parity and benchmark inputs | Keep the existing migration manifest as the operation/workload authority. Add cases and workloads through its established input tree and preserve stable IDs; do not create parallel per-layer operation lists. |
| Tests and evidence | Keep unit tests beside the implementation they prove; put substantial contract tests in a focused family test module. Keep public parity in the migration workflow and immutable measured artifacts under `docs/evidence/<family>/<experiment>-<backend>-<arch>-<date>/`. |
| Generators and inventory | Keep tools in their owning `scripts/` or `tools/` area, grouped by concern when a workflow has multiple files. Generate operation/source mappings from authoritative manifests and registration code. |

Shared low-level code belongs in the nearest existing owner whose callers use
the same contract. A directory should represent a meaningful package, layer,
family, or tool concern; it should not be added merely to make every path the
same length. Facades contain registration, routing, or re-exports, while
executable work stays in the family implementation. The path grammar and
family token stay stable even when a package has no module for a particular
role.

Bindings stay thin: they own host conversion, I/O, validation, and delegation;
the core owns image algorithms. A binding family module is created when it has
real family-specific conversion or delegation work. It is not a second home
for image semantics. One-file binding crates can keep their registration glue
at the crate root until a real module boundary is needed.

When a backend has a real dispatch facade and family orchestrators, use this
shape for its SIMD implementation:

```text
pillow-rs/src/compute/pool_<backend>/ops/
  adapters.rs                         # dispatch and re-exports only
  adapters/
    <family>.rs                       # family validation, mode/backend choice,
                                      # receipt and result materialization
    <family>/
      kernels.rs                      # optional inline kernel-family entry
      kernels/
        <mode>.rs                      # mode-specific hot loop, when distinct
        <mode>_<architecture>.rs       # architecture-specific path, when real
```

Every layer uses the same family name and keeps its responsibilities in the
same order; adapters and kernel directories appear only when those are real
roles. In Rust, `kernels.rs` is a narrow inline family entry when sibling files
under `kernels/` implement distinct paths. Keep it to static forwarding or
re-exports; do not add runtime dispatch to a pixel loop. For example,
`pool_simd/ops/adapters/grayscale.rs` orchestrates the family and
`pool_simd/ops/adapters/grayscale/kernels/ycbcr.rs` owns its YCbCr vector loop.
The family file and its same-named directory are one Rust module tree: the
file is the orchestrator, and the directory holds its focused children. A
direct serial implementation can live at
`pool_cpu/ops/grayscale.rs`; it does not need a forwarding adapter. A GPU family
keeps its admission and dispatch in the GPU operation owner and its shader
stages under `pool_gpu/shaders/<family>/`. Keep the core semantic module at
`ops/<family>.rs`; do not move a hot loop into core just because a backend
adapter can call it or add an empty adapter to equalize path depth.

GPU shader sources are grouped below `pool_gpu/shaders/<family>/`; keep shader
selection and dispatch in the matching family adapter. A CPU implementation
must not be labelled SIMD because it shares an adapter, and Rayon work belongs
to the separate Parallel CPU profile. The profile name never determines source
placement: architecture-specific code belongs with the family kernel that
executes it.

Existing code does not all have this shape yet. Treat it as the standard for
all new work and the destination for incremental moves when a family is changed.
Do not mass-move files just to make the tree look uniform: each move must keep
the executed backend and hot loops clear, preserve parity, and compare full
public-call latency on the affected workloads. Until a family is migrated, its
current entry path must remain explicit and be recorded as a migration
candidate in generated inventory or a documented exception. The generated
source crosswalk records that path and marks uncertain edges for review; do
not maintain a second hand-written operation inventory to stand in for it.

The generated crosswalk is available with
`make operation-source-crosswalk`. It writes the ignored
`target/operation-source-crosswalk.json`; run
`make operation-source-crosswalk-test` for the focused parser and completeness
contracts. The CLI shim stays in `scripts/build_operation_source_crosswalk.py`;
its parser and graph stages live in the focused `scripts/source_crosswalk/`
package. Keep a tool that has multiple source domains organized this way: one
builder/orchestrator and separate modules for each source language or
registration layer, instead of growing one catch-all file.

Schema `pillow-rs/operation-source-crosswalk@8` maps manifest public IDs to
Python definitions, explicit Python `_core` and Rust-image callsites to
registered PyO3 declarations, and direct PyO3 crate-root function calls through
literal Rust public re-exports or root function definitions to their owning
core module and directly referenced `PipelineOp` variants in those function
bodies to their registered pipeline route. It follows `PyImage.inner` and
`PyDraw.draw` calls only when the field type resolves to a supported `pillow_rs`
type and one inherent method matches. It follows direct `self.method()` calls
from those methods, clone-derived local aliases with an exact initializer, and
unique unqualified helper calls within the same core module. Literal
`PipelineOp` references in reached methods and helper functions connect to the
registered pipeline route. It also maps imported CPU operation symbols in
registry expressions to function definitions, SIMD keys to adapter definitions
or macro invocations, and registry defaults to GPU shaders. Each Rust file
includes its public-operation route status, a family ID where its path
identifies one, and the path-role assessment from this guide.
Shader assets without a discovered literal reference and source files with no
complete static public-operation route remain investigation candidates. The
generated `review_queues.operation_graph_source_without_public_route` list
makes those source paths directly discoverable while retaining their distinct
status and per-file evidence. Candidate Python callback routes are also listed
at `review_queues.operation_graph_possible_dynamic_callback_routes`; they remain
uncertain and do not count as statically reached code. These entries are review
signals only; they are not dead-code findings and do not authorize removal. The
inventory covers Rust source files in the three workspace crates, Cargo target roots,
statically resolved `mod` and `#[path]` edges, cfg attributes, re-export
declarations, and literal `include!`, `include_str!`, and `include_bytes!`
references. A literal PyO3 callback method name records checked-in Python
implementations as dynamic candidates; those edges do not count as a public
static route because the runtime object is not fixed. Static links do not prove
runtime execution or backend selection. Local name shadowing, complete helper
call resolution, `PipelineOp` admission, cfg effects, macro expansion, build
output, and runtime loading remain outside the current call-graph proof.
Missing routes are classified for investigation, never as unused code.

The ignored report includes `source.crosswalk_input_sha256`, a fingerprint of
the source, shared path classifier, and generator files used to build it; a
dirty Git flag alone is not enough to identify a generated snapshot. Run
`make operation-source-crosswalk` to refresh the report. Static operation
mappings and module edges help locate ownership and unmapped files, but do not
prove dynamic reachability, Python operations with no static route,
`PipelineOp` admission, runtime capability, actual backend execution, parity,
or performance. The generated report states those
limits; findings never instruct automatic deletion. Keep adding explicit
source edges and conditional-path review flags without duplicating the
manifest's operation denominator.

## Structure and reachability review

An implementation file is not unused merely because one architecture cannot
compile or execute it. Architecture gates, feature selection, macro-generated
references, runtime registration, and public re-exports can all make a valid
path hard to see from a simple text search. Inventory tooling should report
findings with their evidence and classify them for investigation, including:

- public operations with no mapped core/backend implementation;
- adapters with no public, pipeline, or declared test entry point;
- kernels with no reachable adapter path for their declared mode or target
  architecture;
- source files that have no confirmed operation, support, test, generated, or
  platform role;
- paths that are architecture- or feature-specific and still need a native
  runner to prove reachability.

These are review flags, not deletion instructions. Preserve the finding, the
reference search or build evidence, and its architecture/feature scope. Remove
code only after proving there is no supported public route, generated
registration, supported target, or retained compatibility contract that needs
it.

The operation/workload/parity manifest remains the behavior denominator. A
generated source crosswalk may reference it, but it must not create a second,
independently maintained public-operation inventory. Baseline evidence proves
what was measured; it does not by itself prove that a source file is reachable
or useful.

## Initial structural migration candidates

A first-pass source-tree review found these paths that should converge toward
the standard as their families are next changed. This list records migration
work, not permission to move files mechanically:

| Current shape | Standard destination | Migration guard |
| --- | --- | --- |
| `pillow-rs/src/compute/pool_simd/ops/adapters.rs` contains dispatch plus many family implementations; SIMD Offset now lives under `adapters/offset.rs` with byte and Luma16 kernels under `offset/kernels/`. | A re-export/dispatch facade with family orchestrators under `ops/adapters/<family>.rs` and distinct loops under each family's `kernels/`. | Move one family at a time; prove the same SIMD implementation executes and compare full public-call latency and affected parity. |
| `pillow-rs/src/compute/pool_simd/ops/native_filter.rs` holds a byte-block loader used by Filter3x3, Filter5x5, and Sharpness; an LA binomial row loop used by serial AArch64 CPU and SIMD; and an I32 row/vector loop used by SIMD production code (the CPU reference is test-only). | Keep the shared loader and LA loop single-sourced in the current native filter owner. Move the SIMD-only I32 row/vector implementation to `pool_simd/ops/adapters/filter/kernels/i32.rs`, re-export it through that family's `kernels/mod.rs`, and point the CPU differential test at the same implementation. | Preserve the direct SIMD call, vector receipt, and zero-fallback behavior. Use the exact Filter5x5 parity cases and materialized L/LA/I workloads listed below on CPU and SIMD, including terminal materialization. |
| `pool_simd/ops/adapters/cmyk_packing_tests.rs` is an x86-only Convert packing test declared directly under the adapters facade. | `convert/tests/cmyk_packing.rs`, if the test can retain clear access to the private conversion and SSSE3 packing helpers; otherwise record a narrow exception at the existing test owner. | Preserve the x86 cfg and channel-order/layout assertions. This is a test-placement review only; the test is not a production implementation. |
| `pillow-rs/src/compute/pool_gpu/mod.rs` combines GPU pool lifecycle, admission, operation routing, and shader work. | Keep pool lifecycle in the pool owner; route each operation through a family module and put shader assets under `shaders/<family>/`. | Preserve admission, receipts, resource lifetimes, batching, and fallback rules; compare full GPU latency and sustained throughput. |
| GPU shader assets are mostly direct children of `pillow-rs/src/compute/pool_gpu/shaders/`. | `pool_gpu/shaders/<family>/<stage-or-mode>.wgsl`. | Update include/registration paths with their owning family and verify the exact GPU receipt; retain shader files whose references are macro- or build-generated. |
| CPU implementation modules such as `pillow-rs/src/compute/pool_cpu/ops/geometry.rs`, `pillow-rs/src/compute/pool_cpu/ops/effects.rs`, `pillow-rs/src/compute/pool_cpu/ops/imageops.rs`, and `pillow-rs/src/compute/pool_cpu/ops/filter.rs` group multiple families. | Family orchestrators and focused child modules within `pool_cpu/ops/`. | Keep shared low-level primitives in their existing owner; measure any executed-path change and preserve each mode's semantics. |
| `pillow-rs/src/compute/pool_cpu/ops/filter/rgb_blur_ring.rs` and `filter/uniform_box.rs` contain executable filter loops beside `filter/kernels/i32_cross_smooth.rs`. | Decide with call-site evidence whether each loop is a mode-specific kernel under `filter/kernels/` or a family-level shared helper; use one consistent placement for executable filter kernels. | When Filter is next structurally changed, trace CPU and SIMD callers first; preserve the single shared implementation, exact parity, receipts, and full-call CPU/SIMD latency. Do not duplicate a loop to satisfy path shape. |
| `pillow-rs-js/scripts/run_parity.mjs`, `run_browser_parity.mjs`, `parity_workflow.mjs`, and `browser_parity_page.mjs` form a multi-file parity workflow directly under `scripts/`. | Group the parity entry points, shared workflow, and browser page under `pillow-rs-js/scripts/parity/`. | Move as one workflow when it is next changed; retain the same Node/browser entry paths and run the package-local parity checks. |
| `pillow-rs/src/ops/imageops.rs` combines public ImageOps functions for histogram/effects, geometry, resize composition, and EXIF handling. | Keep `imageops.rs` as the public orchestration facade where needed; place distinct family implementations under `ops/imageops/<family>.rs` when their contracts have a clear boundary. | Trace the public exports and call paths first; split incrementally, preserving Pillow error ordering, laziness, exact parity, and affected public-call latency. |
| `pillow-rs/src/compute/pool_cpu/ops/color.rs` combines conversion, LAB conversion, palette remapping, and band extraction. | Keep only shared color dispatch in the family entry point; move distinct operation contracts to focused `pool_cpu/ops/<family>.rs` modules when their ownership is confirmed. | Use the generated source crosswalk to preserve registrations and route each move through the affected parity cases and full-call latency guards. |
| SIMD Resize now owns direct and boxed admission, mode dispatch, zero-source handling, F-mode direct and boxed kernels in `pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/f32.rs`, I32 mode kernels in `pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/i32.rs`, Luma16 kernels in `pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/luma16.rs`, separable native-byte convolution in `pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8.rs`, direct and boxed native-byte nearest kernels in `pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8_nearest.rs`, and boxed native-byte convolution in `pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8_boxed_convolution.rs`; the I32 NEON kernel remains in `pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/i32/neon.rs`. Shared byte coefficient, coordinate, and row helpers still reside in `adapters.rs`. | Continue tracing the remaining Resize paths and move their coherent kernel groups into mode- and algorithm-specific modules. Keep helpers shared with unextracted paths in place until their callers move together. | Preserve direct Resize and Thumbnail routes, exact parity, actual SIMD/NEON receipts, and full-call latency for every moved path. |

Completed under `pillow-rs#rxcj`: SIMD Thumbnail now has a family adapter at
`pool_simd/ops/adapters/thumbnail.rs`, with its fused RGB PutPixel→Thumbnail
path and F32/I32 reduction kernels under `thumbnail/kernels/`. SIMD Offset's
byte and Luma16 kernels are under `pool_simd/ops/adapters/offset/kernels/`.
BoxBlur owns `box_blur.rs` and its Luma, AVX2, and NEON kernels; Transpose owns
`transpose.rs` with its RGBA AVX2 kernel; GaussianBlur owns
`gaussian_blur.rs` with its RGBA uniform kernel.

Refresh this candidate list as structural work lands. The source crosswalk tracks
implementation references; it cannot by itself declare a path misplaced or
unused. The full-tree structure inventory classifies every maintained path;
the changed-path audit reports new filesystem candidates with reasons. Both
preserve architecture, feature, and macro-generated paths for review.

`make implementation-structure-inventory` writes the ignored
`target/implementation-structure-inventory.json` for every tracked and
non-ignored untracked maintained path. It reports paths with a recognized
shape, an explicit supported root role, documented migration debt, a review
candidate, or no specific layout rule yet. An unclassified path is not silently
considered compliant: this target writes the full report, then fails if any
maintained path has no layout rule. Resolve it into the shared standard or
document a concrete exception and migration trigger. Documented migration debt
and review candidates remain visible but non-blocking because their evidence
may refer to immutable paths or need ownership review. Keep their totals visible
when tracking repo-wide convergence. Findings are never move or delete
instructions.

`make implementation-structure-audit` remains the pre-change view for added,
copied, renamed, and untracked maintained paths relative to `HEAD`; it writes
`target/implementation-structure-review.json`. Use it to catch new drift while
the full inventory records existing migration debt. Both commands use this
single guide and produce ignored reports. Run
`make implementation-structure-audit-test` for the focused path-contract and
inventory-scope checks. `make docs-check` runs those contracts and generates
the whole-tree inventory in CI alongside the repository map check. This keeps
the guide and its inventory rules on the regular contributor validation path.

The SIMD Filter5x5 seam now follows the family pattern: `adapters.rs` re-exports
the family entry, `adapters/filter.rs` owns admission, mode routing, receipts,
and materialization, and SIMD-only mode kernels live under
`adapters/filter/kernels/`. The byte loader is shared across filter families;
the LA-binomial row loop has serial AArch64 CPU and SIMD production callers.
The I32 row/vector loop has a SIMD production caller, while CPU reaches it only
from a target-gated differential test, so it is a candidate to move into the
Filter family tree after the listed guards are run. ARM64 and native x86 layout
evidence is in `docs/evidence/filter/filter5x5-family-layout-20261019/`.

Before moving the SIMD-only I32 row/vector helper out of `native_filter.rs`,
freeze these exact parity cases: `PIL.Image.Image.filter.nuanced.rgb-blur-5x5-noise-1024x768`,
`PIL.Image.Image.filter.nuanced.l-mode-kernel-5x5-noise-1024x768`,
`PIL.Image.Image.filter.nuanced.la-mode-kernel-5x5-noise-1024x768`, and
`PIL.Image.Image.filter.nuanced.i-mode-kernel-5x5-noise-1024x768`. Compare the
CPU and SIMD profiles for `pipeline-matrix.expanded.filter5x5.1024x768`,
`pipeline-op.filter.material-l-noise-5x5-1024x768`,
`pipeline-op.filter.material-la-noise-5x5-1024x768`, and
`pipeline-op.filter.material-i-noise-5x5-1024x768`. Require the actual SIMD
vector receipt, zero fallback, and full-call timing through terminal
materialization. These guards are for the proposed move; this documentation
change did not move or benchmark the kernel.

## Performance-preserving changes

Changing files or adapter boundaries does not establish performance. Before
accepting a reuse or layout change that affects execution, retain the exact
parity result and compare the full public call on the same workload, mode,
backend, runner, and source/build identity. Include allocation, transfers,
synchronization, and terminal materialization in the declared timing boundary.
Verify the actual backend receipt and reject any fallback. Keep CPU, SIMD,
Parallel CPU, and GPU results separate. Do not average away a per-operation or
per-mode regression.

The RGBA PutPixel batch now follows the family shape: `adapters.rs` re-exports
`adapters/put_pixel.rs`, which owns admission, output materialization, and
receipts; `put_pixel/kernels/rgba_batch.rs` retains the ordered masked stores.
The selected ARM64 whole-workflow A1/B1 runs both passed exact parity on the
1024×768 and 16×16 RGBA seeded-corners workloads. The 1024×768 SIMD median was
1.185 ms before and 0.958 ms after; same-run Pillow medians were 1.785 ms and
1.570 ms. The small SIMD guard was 0.119 ms before and 0.116 ms after, against
Pillow at 0.115 ms and 0.114 ms. This single pair does not establish a speedup;
it found no latency regression. Receipts showed actual SIMD, NEON deinterleave,
terminal completion, and no fallback. The focused `SimdPool` test also verified
all four PutPixel vector receipts, ordered output equality, and fused-operation
accounting. Full reports are under
`docs/evidence/put_pixel/family-layout-arm64-20261020/`.

SIMD Offset now follows the same family shape: `adapters.rs` re-exports
`adapters/offset.rs`, which owns operation admission and receipts;
`offset/kernels/byte.rs` contains the native byte path, and
`offset/kernels/luma16.rs` preserves Pillow's byte-oriented `I;16*` behavior.
Shared native-copy layout and byte-conversion helpers remain in `adapters.rs`
because Flip, Mirror, and Crop still use them. The selected CPU, strict-SIMD,
and GPU-requested parity cases passed before and after the move. Full-call
benchmark receipts and the focused GPU repeat are recorded in
`docs/evidence/geometry/offset-simd-family-layout-arm64-20261022/`.
