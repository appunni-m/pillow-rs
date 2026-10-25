# Unreachable, unused, and legacy code audit

**Status: in progress.** This ledger records evidence for investigation; it does
not authorize code deletion. A static reference gap is not proof that a path is
dead. Track the complete audit in Kata issue `pillow-rs#mmqd`.

## Method and scope

Raw run receipts cited in historical entries are not tracked in Git. Locally
retained copies live under ignored `target/evidence/raw/`; a fresh checkout may
contain only the findings summarized here, not the original samples or
attestations.

The generated operation/source crosswalk links the parity manifest and its
declared parity and benchmark inputs to Python bindings, PyO3 registration,
pipeline variants, Rust module declarations, re-exports, and literal include
references. Generate it with
`make operation-source-crosswalk`; its ignored report is
`target/operation-source-crosswalk.json`. The generator's source parser and
Rust module inventory are implemented in `scripts/source_crosswalk/`.

The crosswalk is a static index. It does not evaluate `cfg` or Cargo feature
sets, expand macros or procedural macros, inspect build output, establish
runtime loading, or prove that a test or parity case executes a specific
implementation. Operation mappings establish discoverability, not coverage or
execution. Items below remain review candidates until those boundaries are
traced. This audit uses no coverage data and has not deleted or simplified
code. It does not add unit tests. Behavioral gaps and public-route claims are
checked with the same generated fixture and requirement IDs while the runner
selects CPU, SIMD, or GPU. Ordinary parity inputs and operation parity policies
must not select a backend. Strict backend runs and their execution receipts
establish whether a pipeline actually used the selected backend; parity alone
does not.

## Current operation/source and fixture snapshot — 2026-10-24

The schema-10 crosswalk was regenerated at revision
`c18fee48be020689ed9da4ab61e9e6594b6dbbb1`. Its dirty marker reflects the
in-progress audit and preserved untracked workspace artifacts. It read 837
source and fixture inputs with fingerprint
`7413a70a77005338df3eae9259284f26f938c53536513c16952ec3dddea1b2d4`. The
ignored report `target/operation-source-crosswalk.json` has SHA-256
`faa3c5fa5ab4ba68ee176c4fab47a3afb4a40e865d929a9bc0bdb9301a0b346f`.

| Generated measure | Count |
| --- | ---: |
| Manifest operations mapped to Python | 214 / 214 |
| Indexed parity-input cases, including 1 fault contract | 17,339 across all 214 |
| Declared benchmark workloads across public operations | 1,032 across 209 |
| Python native calls mapped to registered PyO3 declarations | 157 / 157 |
| Direct PyO3 core API calls mapped | 103 / 103 |
| Direct PyO3 receiver calls mapped | 98 / 98 |
| Pipeline variants / variant-key edges | 84 / 84 |
| Registry keys / SIMD registrations | 81 / 81 |
| Core methods reached from PyO3 | 207 |
| Rust source files / files with static public-operation routes | 594 / 97 |
| Static graph edges without full route / files without graph edge | 24 / 473 |
| Operation-graph source review candidates | 497 |
| Files with a possible dynamic-callback route | 11 |
| Family/layout findings / shader candidates | 34 / 4 |
| Unresolved module edges / unmapped fixture operation references | 0 / 0 |

The five operations without an indexed benchmark workload are
`PIL.GpuBatchExecutor`, `PIL.GpuBatchExecutor.submit`,
`PIL.Image.Image.toqimage`, `PIL.Image.Image.toqpixmap`, and
`PIL.ImageFont.MAX_STRING_LENGTH`. The two GPU batch API operations are
target-only and have no Pillow parity equivalent. Parity and benchmark counts
are declarations; they do not prove case selection, execution, exact parity,
or backend receipts. All static route counts are review evidence, not runtime
reachability claims.

The earlier crosswalk parser expected registry entries inside one `register_all`
body and missed class registration when PyO3 methods were split into child
modules. It now indexes registration modules and resolves `PyImage` methods
through the separately registered class and its `inner: pillow_rs::Image`
field declaration. The core receiver index now scans the `image/` and `draw/`
module trees as well as operation modules, resolving all 98 direct PyO3
receiver calls. No unit tests, benchmarks, or coverage were run for these
static inventory-tool changes. The selected parity runs below verify backend
execution for one shared operation slice; source mappings and fixture
declarations still do not prove general runtime reachability.

## Backend-neutral parity selection and backend execution audit — 2026-10-24

The active manifest indexes 27 parity input files containing 17,338 ordinary
oracle-parity cases and one target-only fault contract. None of those cases has
a `target_profiles` field, and none of the 214 operation-level parity policies
declares `target_profiles`. The parity policy requires 212 operations; the two
GPU batch API operations are explicitly not-applicable to Pillow. The fault
contract is selected through its dedicated profile and command; it is not an
ordinary oracle-parity case.

`scripts/run_migration_parity.py::select_cases_for_profile` appends every
ordinary parity case for the CPU, SIMD, GPU, and Parallel CPU runtime profiles.
It excludes ordinary parity only under the separate GPU fault-contract profile;
the profile filter otherwise applies only to target-only fault contracts. The
same case IDs can therefore be passed to
`make migration-parity-test-simd-strict` or
`make migration-parity-test-gpu-strict`; the backend is a runner option. The
strict runner activates one backend and rejects fallback or missing terminal
proof for backend-relevant deferred pipelines.

`MIGRATION_TARGET_BACKEND=simd` or `gpu` by itself does not require that
backend to execute; normal parity permits the documented CPU fallback. Use
the strict target or inspect the execution sidecar before crediting a backend.
`scripts/run_all_backend_tests.py` records receipts for its full SIMD and GPU
lanes. Its full-corpus lanes permit fallback so the parity result remains
available while unsupported routes are inventoried. The artifact distinguishes
`passed_with_backend_gaps`, and the all-backends command now exits nonzero for
that status or when full GPU parity is skipped. Case-filtered SIMD/GPU lanes
are strict and reject fallback or missing terminal proof.

`scripts/run_all_backend_tests.py::public_case_ids` also builds one shared
ordinary-parity denominator and forwards an explicit case filter to CPU, SIMD,
and the optional full GPU lane. Its separate GPU smoke gate always selects the
fixed `PIL.ImageFilter.GaussianBlur.nuanced.backend-noise-l-65x47-radius-2`
case, including when the full-lane filter omits it. The all-backend SIMD and
full GPU lanes intentionally allow documented fallback and collect receipts;
fallback or incomplete receipt coverage changes the result to
`passed_with_backend_gaps` and returns a nonzero exit code. Setting
`MIGRATION_GPU_FULL=0` runs only the smoke gate, reports the
full GPU lane as skipped, and returns nonzero. The JS/WASM runner separately
excludes target-only fault contracts and
native platform-viewer workflows at runner level; those exclusions do not alter
the CPU/SIMD/GPU fixture set.

The canonical fixture generator rebuilt 27 indexed parity JSON files and 129
binary assets on 2026-10-25. `scripts/check_migration_parity_inputs.py`
confirmed the generated inputs reproduce exactly; these generated payloads
are ignored and are not committed. Five older profile-tagged trial inputs
(`getbbox-i-rowor-20261018.json`, `getbbox-f-filled-1024x768.json`,
`convolution-uniform-color-trial-20261014.json`, `convert-cmyk-trial.json`, and
`getdata-hsv-band2-x86-20261014.json`) are outside the canonical input index
and absent from a fresh checkout. Four standalone slice manifests remain as
historical selection records; the table below documents their old profile
scopes, not active runner inputs. The GetBBox F row-scan evidence snapshot was
moved to ignored local output and purged from published history.

| Historical manifest | Legacy parity profile scope |
| --- | --- |
| `pillow-rs/tests/fixtures/manifest.convolution-uniform-color-trial-20261014.yaml` | 10 operation policies select CPU; 10 cases select CPU except one SIMD-only case. |
| `pillow-rs/tests/fixtures/manifest-getbbox-i-rowor-20261018.yaml` | 4 operation policies and all 10 cases select CPU. |
| `pillow-rs/tests/fixtures/manifest.convert-cmyk-trial.yaml` | 212 operation policies select CPU, with Convert also tagged SIMD/GPU; its one case selects CPU, SIMD, and GPU. |
| `pillow-rs/tests/fixtures/manifest.getdata-x86-run.yaml` | 2 operation policies select CPU; its one case selects CPU, SIMD, and GPU. |
| GetBBox F row-scan historical evidence manifest (moved to ignored local output) | 2 operation policies and its one case select CPU. |

The evidence YAML search found 26 snapshots with operation-level parity
profile tags; five of those also index profile-tagged cases. These snapshots
preserve older experiment and PerfDB identities and are not the active parity
specification. Do not rewrite their digests in place. The current
`validate_migration_parity_contract.py` rejects `target_profiles` on parity
operation policies, `build_migration_parity_inputs.py` rejects generated
parity cases with profile tags, and `run_migration_parity.py::load_manifest`
uses that validator before running a custom manifest. The aggregator's
`requirements_for_lane` also ignores target profiles for parity, so legacy
profile tags are not an executable case filter. New cross-backend parity work
must use the active manifest and one shared fixture set.

The same nine active `PIL.ImageChops.screen` case IDs were run through the
strict SIMD and strict GPU targets at the current revision. Both parity results
passed 9/9. They were re-run through `run_all_backend_tests.py` with the same
explicit filter across CPU, strict SIMD, strict GPU, Node WASM, and browser
WASM. Every lane passed 9/9 with the same selected-case digest,
`80c08204b4a49847b0843c9e3763246afd48721243ce3e66a01160771642d4eb`.
The CPU, SIMD, and GPU parity run IDs are
`migration-parity-0a2f6f904af742eaa909f82a23f303ec`,
`migration-parity-62115dd8b75f45efa9c879628e5514d4`, and
`migration-parity-00bfd8664b344922995faa4825dca582`. Each execution sidecar
reports nine pipeline-applicable cases, nine terminal-complete receipts, the
requested backend for all nine cases, zero fallback reasons, and zero missing
or partial receipts.
The selected cases are the L/LA/RGB/RGBA 5×5 tails, the L/LA/RGB/RGBA
large-wide workloads, and the materialized RGB-noise 1024×768 workload.
SIMD reports 314,204 vector blocks and zero scalar tails. The GPU WGSL sidecar
records 13 dispatches across those cases (nine `Screen` and four input
`PutPixel` dispatches) over 11,022 workgroups. Node and browser WASM report CPU
execution; neither is credited as GPU execution. The current parity outputs,
execution sidecars, and WGSL receipt are in
`build/migration-parity/backend-bypass-audit-20261024/all-backends/`.
This proves the selected cases use both backends; it is not a claim that every
ordinary operation has a SIMD or GPU implementation.

The shared ImageChops fixture now also contains 22 offset-coercion cases: 11
input forms each for `add` and `subtract`. The fixture and public requirement
IDs contain no backend tags. `scripts/test_chops_affine_parity.py` selects those
IDs and sends the same fixture inputs through the canonical Python parity
runner with CPU, strict SIMD, and strict GPU selected at the runner boundary,
then through the Node and browser WASM parity runners. All five lanes pass
22/22 with case digest
`9439a86dcb06f80e00674f7715828151046912031bc50e91f994b2977d64b157` and parity
input digest
`303d277ea91260c8ad065375747191c50fff2abd44e88427c8c9ebd421ac7d72`. The
Pillow 12.2.0 source identity is the same in every lane.

Each Python execution sidecar classifies 14 argument-validation cases as not
pipeline-applicable and records eight complete receipts. CPU reports eight
`actual_backend=cpu` receipts. SIMD reports eight `actual_backend=simd`
vector-path receipts (16 vector blocks, no scalar tails); GPU reports eight
completed dispatches with `actual_backend=gpu`. SIMD and GPU have no fallback
reason, missing receipt, or partial receipt. Node and browser WASM each report
22/22 with no pending cases. Their eight static host-value hints are
diagnostics only and do not filter execution.

Run IDs: CPU `migration-parity-bf2267ae4d0f45ffb6e6446e5ff4a6c8`, SIMD
`migration-parity-a713f04b14184ba195bb8d8ff1741692`, and GPU
`migration-parity-ebaaf605ed9341daa9a58f57aea4fba3`. Parity outputs and
execution sidecars are under `build/migration-parity/chops-affine/`.

The initial WASM run exposed differences in float, `None`, string, and integer
overflow handling. The JS host adapter now preserves Python numeric type for
the declared ImageChops offset input, while the public WASM binding performs
the integer and range validation. The generic input-only
`invalid-argument-type` and `public-index` protocols remain backend-neutral
and contain no expected outputs. No fixture selection or expected result is
special-cased by target or backend.

The fixture-backed portions of the standalone font, getdata, and putdata
parity routes were checked with one nine-case filter through
`run_all_backend_tests.py`: `font_variant.behavior.default`,
`getdata.nuanced.retained-view-mutation-and-thumbnail`, and seven packed or
byte-oriented putdata cases spanning `1`, `L`, `LA`, `P`, `RGB`, and `RGBA`.
CPU, strict SIMD, strict GPU, Node WASM, and browser WASM each passed 9/9 with
case digest
`bc7554d1e2e077ab8fe9f5a80fb4f23417bd2d73c8bb71ad6e5a061a6e32a24b`.
Each native Python lane recorded eight terminal-complete pipeline receipts:
CPU reported eight CPU executions, SIMD eight SIMD executions, and GPU eight
GPU executions, all with no fallback reason. The font-variant case was
classified not applicable because it has no deferred image pipeline. The GPU
smoke case and filtered full GPU lane also passed. Artifacts are under
`build/migration-parity/backend-bypass-audit-20261024/all-backends/`.
These are shared-fixture cases only; the extra mutable font state, sequence
callbacks, and custom sequence protocol probes in the standalone scripts
remain outside the active fixture matrix and are not covered by this run.

## Standalone parity routes and bypass inventory — 2026-10-24

The active fixture/runner path is `run_migration_parity.py`, with
`run_all_backend_tests.py` as the multi-lane orchestrator. A source scan for
standalone live-Pillow comparisons and direct backend activation found the
following exceptions. “Outside fixtures” means the inputs or expected
observations are constructed in the script rather than selected by active
fixture IDs; it does not mean their current code necessarily has a backend
fallback bug.

| Script | Current route | Bypass and disposition |
| --- | --- | --- |
| `scripts/test_chops_affine_parity.py` | **Repaired.** Selects active ImageChops fixture IDs and sends the identical case set to the standard Python runner for CPU, strict SIMD, and strict GPU, then Node and browser WASM runners. | Previously made direct `enable_backend()` calls and compared results without recording the actual backend. All five lanes now pass 22/22; SIMD/GPU receipts prove native execution for the eight pipeline-applicable cases. |
| `scripts/run_all_backend_tests.py` | Multi-lane orchestrator launches CPU, SIMD, and GPU Python parity runs plus Node/browser WASM against one case filter. | It clears inherited profile/benchmark overrides. Case-filtered SIMD/GPU runs are strict; full-corpus runs record fallback gaps, and now return nonzero if backend proof is incomplete or full GPU parity is skipped. The nine-case Screen audit passed every lane with actual SIMD/GPU receipts. |
| `scripts/test_grayscale_rgb_domain.py` | Custom exhaustive Pillow/target diagnostic enumerating all 16,777,216 RGB triples in 256 blocks. It uses `lock_target_image_pipeline`, strict backend settings, checks one native receipt per block, and compares exact output bytes. | Inputs and output comparisons bypass the active fixture matrix, though backend locking/receipts are strict. Keep it classified as supplemental exhaustive evidence, not fixture parity or benchmark evidence. On 2026-10-24, Pillow 12.2.0, CPU, SIMD, and GPU produced exact output; each target backend recorded 256 native receipts. SIMD reported 1,048,576 vector blocks with no scalar tails; GPU reported 256 dispatches. Artifacts are under `build/migration-parity/backend-bypass-audit-20261024/grayscale-rgb-domain/`. |
| `scripts/test_font_variant_parity.py` | Isolated source and target subprocesses compare font creation, mutable source/path settings, variants, and rendered observations. | It uses a fixture font asset but owns the scenarios and result dictionary; it has no active parity case IDs, backend option, or execution receipts, so it proves only the default Python route. It is not wired to a Make target. |
| `scripts/test_getdata_sequence_parity.py` | Inline Pillow/target probes compare indexes, slices, banded modes, palette snapshots, and errors. | It has no generated case IDs or backend option/receipt and proves only the default Python route. It is not wired to a Make target. |
| `scripts/test_putdata_sequence_parity.py` | Inline live-Pillow probes exercise mutable callbacks, custom sequences, bytes subclasses, coercion order, and partial writes. | It has no generated case IDs or backend option/receipt and proves only the default Python route. Existing declarative sequence protocols cover ordinary values but cannot express the callback side effects exercised here. It is not wired to a Make target. |
| `scripts/test_gpu_batch.py` | A Make-wired, explicit `GpuBatchExecutor` oracle comparison creates image graphs from a local generator and compares terminal results with live Pillow; its `--contracts` mode checks queue/resource behavior. | This is a distinct target-only batching API and is not the normal CPU/SIMD/GPU image backend runner. Its graph inputs bypass active fixture IDs. Keep its admission, identity, and queue contracts in the dedicated GPU-batch lane; do not cite it as ordinary GPU parity. |

`scripts/benchmark_gpu_batch.py` and the other backend-specific benchmark
drivers are benchmark workloads rather than parity runners, so their profile
selection belongs to the benchmark runner and is not counted as a parity
bypass here. Coverage/native probes also do not compare against live Pillow
and are outside this inventory. The standalone parity scripts above should be
migrated only when their input behavior can be represented by generated
fixtures without losing callbacks, state lifetime, or the specialized
exhaustive/batch contract. No such migration is performed by this audit.

The active manifest already contains ordinary `font_variant`, `getdata`, and
`putdata` cases, and those shared IDs are selected by every Python backend
profile. The standalone font and sequence scripts add mutable object state,
sequence indexing, and callback side effects that their current fixture
protocol cannot express; those extra scenarios remain outside the matrix.
These API calls do not enqueue compute `PipelineOp`s, so their SIMD/GPU profile
runs are parity checks with backend execution classified not applicable. The
strict backend claim applies to actual deferred image work, not to forcing a
GPU dispatch for a host-side sequence or font operation.

## Historical operation-to-family and fixture snapshot — 2026-10-21

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

## Historical family/layout review — 2026-10-22

The 2026-10-20 crosswalk emitted seven `rust_source_family_layout_review`
findings. At the 2026-10-22 checkpoint, after the completed family moves, the
crosswalk reported three: the CPU allocation test module and two Convert
children. These are path-role signals from the shared structure classifier,
not dead-code findings. The
table also records the moved Thumbnail and Resize routes so their ownership
stays traceable:

| Path | Reachability evidence | Classification and disposition |
| --- | --- | --- |
| `pillow-rs/src/compute/pool_simd/ops/adapters/cmyk_packing_tests.rs` | Included only under `#[cfg(all(test, target_arch = "x86_64"))]` by `adapters.rs:19-20`. It checks the CMYK conversion helper used by `adapters/convert.rs:52` and the SSSE3 packer. | Test-only x86 guard, not a production path. Keep the coverage; review whether it can live under the Convert family's test tree without obscuring private helper ownership (`rxcj`). |
| `pillow-rs/src/compute/pool_cpu/ops/color_convert_allocation_tests.rs` | Declared through `#[cfg(test)]` and `#[path]` by `pool_cpu/ops/color.rs:190-192`; its tests exercise native LA conversion allocations and alpha semantics. | Test-only CPU conversion contract, not a production path. Keep the assertions; move the module beside a family only when conversion ownership is split. |
| `pillow-rs/src/compute/pool_simd/ops/adapters/thumbnail.rs` | Declared/re-exported by `adapters.rs:32,48-51`; the registry binds Thumbnail to `adapters::simd_thumbnail` at `compute/registry.rs:2939`. | Live family adapter for standalone Thumbnail dispatch and support checks. Keep its route; the family layout move is recorded under `rxcj`. |
| `pillow-rs/src/compute/pool_simd/ops/adapters/thumbnail/kernels/fusion.rs` | Declared by `thumbnail.rs:21-26` and called by `pool_simd/put_pixel_batch.rs:57-82` for eligible RGB PutPixel prefixes followed by Thumbnail. | Live fused RGB execution path. Keep it under the Thumbnail kernel layer and preserve the fused receipt. |
| `pillow-rs/src/compute/pool_simd/ops/adapters/thumbnail/kernels/float.rs` | Declared by `thumbnail.rs:17`; its F-mode reducer is selected by `simd_thumbnail` at `thumbnail.rs:283`. | Live F-mode kernel on the SIMD Thumbnail path. Keep it under the family's explicit kernel layer. |
| `pillow-rs/src/compute/pool_simd/ops/adapters/thumbnail/kernels/integer.rs` | Declared by `thumbnail.rs:19`; I-mode reduction and resampling are selected by `simd_thumbnail` at `thumbnail.rs:272,285`. | Live I-mode kernel on the SIMD Thumbnail path. Keep it under the family's explicit kernel layer. |
| `pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/i32.rs` | Declared/re-exported by `resize.rs:3-12`; Thumbnail calls its I-mode resize helpers from `thumbnail.rs:367-380`. | Live mode-specific Resize implementation shared with Thumbnail. Keep it with Resize and preserve both direct routes. |
| `pillow-rs/src/compute/pool_simd/ops/adapters/resize_i32/neon.rs` | The parent declares it only for `target_arch = "aarch64"` plus `target_feature = "neon"` (`resize_i32.rs:20-21`); the I32 horizontal planner selects its NEON path under that cfg. | Supported architecture-specific implementation, not an x86-host dead path. Keep it; move it with the I-mode Resize family when its exact cfg and SIMD receipt can be preserved. |

This pass traces six production adapters/kernels and identifies two test-only
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

The current crosswalk has no unresolved direct PyO3 receiver calls. It still
has 24 files with static graph edges but no complete public-operation route,
473 files without a static operation-graph edge, and 34 family/layout findings.
The 11 callback-route files remain possible routes, not proven runtime
execution. Triage these queues by module ownership,
callers, cfg/features, macro/generated paths, and test role; do not infer
unreachability from their counts.

The four shader candidates have source-loader evidence but still need their
history and a separate removal decision. The audit also needs supported-target
and feature reviews, compiler-backed dead-item evidence where justified, private
caller-chain review, refactor duplicate review, and fixture selection checks.
For behavior, reuse generated fixture parity and fault-contract IDs and select
the backend at the runner. Use strict SIMD/GPU lanes when actual backend
execution is the claim. The static crosswalk does not prove that a runner
selects or executes a declared case.

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
registered family entry calls this local dispatcher. General byte and float
pixel kernels and the Luma16 kernel remained in `adapters.rs` at that
checkpoint; later moves are recorded below. This changes file ownership only;
it does not alter the historical crosswalk counts or infer reachability beyond
the selected parity and benchmark paths.

## SIMD Resize native-byte convolution follow-up — 2026-10-22

The direct separable native-byte convolution entry points now live in
`pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8.rs`. Direct
filtered Resize and native Thumbnail continue to enter through their existing
family routes. Shared coefficient and row helpers remain in `adapters.rs`
because boxed byte convolution and other Resize paths still call them. This
relocates the existing implementation without changing its algorithms or
registered operation routes; static reachability counts remain unchanged.

## SIMD Resize native-byte nearest follow-up — 2026-10-22

The direct and boxed native-byte nearest kernels, plus their byte gather
helper, originally moved into `pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8.rs`.
They now live in the focused `resize/kernels/u8_nearest.rs` module. Resize and
native Thumbnail continue to use the same routes. Shared coordinate builders
remain in `adapters.rs` because the float and I32 Resize paths also call them.
The ownership moves do not change the operation graph or add a static
reachability claim.

## SIMD Resize boxed native-byte convolution follow-up — 2026-10-22

The boxed native-byte convolution kernel now lives in
`pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8_boxed_convolution.rs`.
Resize and `ImageOps.fit` keep their existing callers through the `resize`
module re-export. Shared coefficient and row helpers remain in `adapters.rs`
because other Resize kernels still use them. This is an ownership-only move;
the operation graph and static reachability counts are unchanged.

## SIMD Resize F-mode kernel follow-up — 2026-10-22

The direct and boxed F-mode Resize kernels now live in
`pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/f32.rs`. Resize,
Fit, and aspect-resize callers retain the same function routes through the
family re-exports. The Luma16 kernel remained in `adapters.rs` at that
checkpoint; its later move is recorded below. Shared byte coefficient,
coordinate, and row helpers remain there for their existing callers. This is
a structural move only; no runtime reachability or performance claim is added.

## SIMD Resize Luma16 kernel follow-up — 2026-10-22

The `I;16*` SIMD Resize kernel now lives in
`pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/luma16.rs`. The
Resize family's typed-mode gate still selects it through the same local
dispatcher. Its endian handling and shared scalar-compatible resampling
helpers remain unchanged. This ownership move adds no runtime reachability or
performance claim.

## SIMD Resize composite adapter follow-up — 2026-10-22

Scale, Contain, Cover, and Fit adapters and their aspect-resize helper now live
in `pillow-rs/src/compute/pool_simd/ops/adapters/resize.rs`. The registry keeps
the same `adapters::simd_*` names through direct family re-exports. No dispatch
or runtime reachability claim changed; this extraction adds no performance
claim.

## SIMD Offset family path follow-up — 2026-10-22

The registered Offset adapter remains `adapters::simd_offset`; its family
adapter now lives in `pillow-rs/src/compute/pool_simd/ops/adapters/offset.rs`.
The native byte loops and the Pillow-compatible byte-oriented `I;16*` loops
live under that family's `kernels/` directory. Native-copy layout, empty-image,
and shared byte-conversion helpers remain in `adapters.rs` because Flip,
Mirror, and Crop still call them. The selected 14-case CPU, strict-SIMD, and
GPU-requested parity lanes passed before and after extraction. This changes
ownership only; no operation route or reachability count changed. Receipts are
preserved in `docs/evidence/geometry/offset-simd-family-layout-arm64-20261022/`.

## Filter scalar and GPU route follow-up — 2026-10-24

### Private packed scalar filter helpers

The module split in `77fe11803` moved the packed filter helpers to
`pillow-rs/src/compute/pool_simd/ops/scalar/filters.rs`. A read-only scanner
under `/tmp` inspected 590 Rust files across core, Rust tests, and the Python
and JavaScript bindings, including cfg and macro text. In the SIMD ops tree,
the only module hit is the private `mod scalar` declaration in `ops/mod.rs`;
no current import or call reaches the filter functions. Bare
function names collide with public `Image` methods and separate CPU helpers,
so they are not evidence of a caller. `#[allow(dead_code)]` suppresses
compiler warnings, and this source scan is not a compiler proof.

The helpers were an active fallback before `3501013f3` replaced their adapter
calls with strict native-layout implementations. Current admission rejects
unsupported inputs before SIMD adapter execution; automatic planning selects
another backend where supported, while a strict SIMD lock reports the
unsupported operation. The packed helpers are not the present fallback.
Disposition: review removal of only `scalar/filters.rs` and its re-exports in
a separate focused change; update the stale fallback comments. Keep unrelated
scalar operation families. The filter-only recommendation and route evidence
are tracked in `pillow-rs#aqsq`; no source was deleted here.

Generated public parity fixtures remain the behavior contract, though they do
not reach the now-unreferenced scalar module. Relevant selectors include
`PIL.Image.Image.filter.nuanced.l-mode-detail-fused-row`,
`PIL.Image.Image.filter.nuanced.l-mode-kernel-5x5-noise-1024x768`,
`PIL.ImageFilter.MedianFilter.nuanced.coverage-batch-filter-l-size-3`,
`PIL.ImageFilter.MaxFilter.nuanced.coverage-batch-filter-rgb-size-5`,
`PIL.ImageFilter.MinFilter.nuanced.coverage-batch-filter-la-size-3`,
`PIL.ImageFilter.RankFilter.nuanced.coverage-batch-rank-rgb-size-9-rank-40`,
`PIL.ImageFilter.BoxBlur.nuanced.coverage-batch-filter-la-radius-1`, and
`PIL.ImageEnhance.Sharpness.enhance.nuanced.parity-blend-rounding-factor-0-3-border-rgb-3x3`.
Preserve the existing fault-contract fixtures
`PIL.ImageFilter.MaxFilter.nuanced.coverage-batch-filter-invalid-even-size`,
`PIL.ImageFilter.MinFilter.nuanced.coverage-batch-filter-invalid-even-size`,
`PIL.ImageFilter.MedianFilter.nuanced.coverage-batch-filter-invalid-even-size`,
`PIL.ImageFilter.RankFilter.nuanced.coverage-batch-filter-invalid-even-size`,
`PIL.ImageFilter.RankFilter.nuanced.palette-rejected`, and
`PIL.ImageFilter.BoxBlur.nuanced.negative-radius-error`. Do not add unit tests.
For a behavior change, run the affected generated-fixture IDs with the backend
selected by the runner; use strict SIMD/GPU when the claim is that those
backends execute the operation.

The five active F-mode order-statistic fixtures are backend-neutral; none has a
`target_profiles` field:
`PIL.Image.Image.filter.nuanced.f-mode-max-filter`,
`PIL.Image.Image.filter.nuanced.f-mode-min-filter`,
`PIL.Image.Image.filter.nuanced.f-mode-median-filter`,
`PIL.Image.Image.filter.nuanced.f-mode-rank-filter`, and
`PIL.Image.Image.filter.nuanced.f-mode-large-rank-filter`. A strict-SIMD run
selected all five IDs and passed 5/5 with zero infrastructure errors at
revision `374a3b7bd`; that result is historical and does not make the fixtures
SIMD-specific. The current requirement is to keep one shared ID set while
selecting each backend in the runner. The earlier case-count/profile discussion
is tracked in `pillow-rs#8nq8`.

```sh
MIGRATION_PARITY_CASE_IDS='PIL.Image.Image.filter.nuanced.f-mode-max-filter,PIL.Image.Image.filter.nuanced.f-mode-min-filter,PIL.Image.Image.filter.nuanced.f-mode-median-filter,PIL.Image.Image.filter.nuanced.f-mode-rank-filter,PIL.Image.Image.filter.nuanced.f-mode-large-rank-filter' \
  make migration-parity-test-simd-strict
```

The old result was written to
`build/migration-parity/simd-strict-parity-result.json`. Current active cases
remain backend-neutral; do not add per-case profile credit to parity inputs.

### GPU projection backend-neutral case

The public GPU projection path currently compiles
`getprojection_atomic.wgsl`; the CMYK terminal checks are separate admission
and validation code, and strict GPU reports CMYK projection unsupported. The
old `getprojection_cmyk.wgsl` has no current source loader/reference.
`PIL.Image.Image.getprojection.parity.material-cmyk-1024x768` is an active,
backend-neutral case. The earlier statement that it declared `python-gpu` was
stale. Keep one shared case and use the strict GPU runner to expose the current
unsupported route; a normal GPU parity pass can include CPU fallback and does
not prove GPU execution. GPU benchmark applicability is a separate operation
policy and does not tag the parity case. The source/history evidence is
recorded under `pillow-rs#h2sn`.
