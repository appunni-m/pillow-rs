# CPU mode and architecture refactor plan

Status: active. The baseline inventory is captured in
[CPU refactor inventory](CPU_REFACTOR_INVENTORY.md). The original CPU and SIMD
trees are preserved under pool_cpu/legacy; per-family mode and kernel contract
review continues from that archive before each maintained implementation is
written.
Baseline: `3f9dfd4036caa061eabcf2255c9f34a745df9c7e`.
Issue: `pillow-rs#3ptb`; related parity findings: `pillow-rs#brg0`.

## Outcome and scope

Transform CPU execution completely into the mode and architecture structure
defined by [Implementation structure](IMPLEMENTATION_STRUCTURE.md). That guide
is the sole placement authority; this plan defines work order and acceptance.

Mode is carried by the flat Pillow-owned `raster::DynamicImage` enum. Execution
context carries Backend and Architecture from routing to implementation.
Scalar/default and existing SIMD kernels become implementations of the CPU
backend. Parallel CPU is a separate subsequent stage and begins only after the
entire CPU implementation compiles and passes its quality and parity gates.

Each operation exposes its typed support contract at the operation entry point.
It composes a mode strategy with an architecture strategy using the injected
context. Support reporting, request admission, and executable preparation use
the same declarations, as defined in Section 3 of the structure guide. Folder
placement isolates these strategies; explicit contracts establish support.

Ownership and operation-level visibility are required acceptance criteria,
not optional follow-up work. Use Section 2.1 of the structure guide to keep
Pillow mode identity separate from codec metadata. From an operation's entry
point, a caller must be able to inspect every mode's declared strategies,
input/output constraints, architecture requirements, and rejection or fallback
reasons without inspecting implementation folders. Generate that view from
the same declarations used to prepare execution; do not maintain a second
support matrix by hand.

The ownership decision is explicit: `pillow-rs` owns the operation-facing
`raster::Mode`. Rust callers use it directly, and Python translates
its strings to it before delegation. Review the support-visibility acceptance
contract in Section 3.3 of the structure guide through each operation's entry
point, including deferred guards for facts unavailable before materialization.

This is a structural refactor. Preserve every Pillow-defined algorithm,
arithmetic order, supported and rejected mode behavior, error contract, pixel
representation, fusion path, allocation boundary, and public API behavior.
Core support may intentionally exceed Pillow: add a declared native-mode
strategy where useful, mark it beside its implementation with
`// NO_PILLOW_SUPPORT: <reason>`, and report it separately from Pillow parity.
Do not introduce new SIMD algorithms, optimize an algorithm, or replace native
modes with RGBA as an implementation shortcut.

Implementation starts from this baseline with Stage 0. The user has authorized
the ordered stages below. Do not repeat the reset or discard work created after
the baseline. Keep `pillow-rs-bad` as reference.

## Stage 0: freeze the inventory and execution contract

Perform source inspection only; do not run builds or validation campaigns.

1. Confirm the active checkout, current Git status, baseline, user changes,
   attached worktrees, and recovery refs. Preserve unrelated work and refs.
2. Use Kata to maintain `pillow-rs#3ptb` and record the revised scope. Earlier
   evidence from the abandoned implementation does not verify the new source.
3. Inventory the baseline CPU tree: 98 tracked files and the 84 `PipelineOp`
   variants currently declared in `pillow-rs/src/pipeline.rs`. Record every
   source-to-target move, exported function, caller, mode, kernel, and helper
   in `docs/CPU_REFACTOR_INVENTORY.md`; the baseline file paths, visible
   function declarations, mode candidates, cross-pool references, instruction
   sites, and verification denominator are frozen there. Confirm each family's
   exact mode outcomes and function destinations from the archived source
   before writing its maintained implementation.
   The source enum and existing public manifest remain the authorities.
4. Include direct/eager CPU entry points, constructors, decode adapters,
   materialization, lazy graphs, mutation, scalar samples, palette operations,
   and public calls that do not become a `PipelineOp`.
5. Inventory the existing SIMD kernels and their exact feature requirements.
   Include SIMD work outside `pool_simd`, such as specialized transpose or
   resize paths. Identify mixed serial/Rayon helpers and preserve both original
   implementations for their eventual owners.
6. Record source-owned image modes and storage representations, output-mode
   changes, secondary-image/mask contracts, defaults, and unsupported cases.
   Every affected mode/operation combination needs an explicit destination or
   preserved error outcome. Record fallback as a destination, not as a gap.
   For each operation, classify all 25 identities and their input combinations,
   architecture requirements, build availability, eligibility guards, and
   fallback/rejection reasons before declaring its strategy support.
7. Identify source locations requiring registry, pipeline, binding, telemetry,
   feature, Makefile, source-map, or documentation wiring changes. GPU changes
   are limited to compatibility required by shared types and CPU fallback.
8. Freeze the complete CPU-applicable parity denominator from the existing
   manifest: IDs, count, hash, oracle, source/build identity, target features,
   materialization contract, and output locations. Record platform and
   non-CPU-only contract exclusions explicitly. No sampling or surface filters.
9. Freeze existing CPU benchmark workload IDs and compatible baseline evidence.
   Do not run a baseline campaign during implementation. If suitable evidence
   is absent, record that limitation and measure the preserved baseline only
   after the CPU validation gate.

Deliverable: an exhaustive baseline source/path inventory and a fixed
verification scope, tracked with the issue. Record each family's exact
mode/function route and behavior as part of its source review before writing
that family's maintained implementation. Do not replace this inventory with
a claim that a few representative operations cover the entire backend.

## Stage 1: preserve legacy and establish the approved tree

1. Preserve the existing CPU source in its inert legacy location, preserving
   file contents and relative paths. Record baseline provenance for each file.
   Completed before rebuilding: 98 CPU files are under `pool_cpu/legacy/` and
   182 SIMD files are under `pool_cpu/legacy/pool_simd/`.
2. Prepare the maintained tree using the guide's family, mode, and architecture
   mapping. Create simple module exports and orchestration boundaries without
   importing legacy or copying the old family layout back into active code.
3. Preserve existing SIMD source in the specified legacy namespace before
   folding it into CPU. Completed; trace every retained or retired
   implementation against this archive.
4. Record the actual directory tree alongside its inventory. Confirm that mode
   and architecture levels exist and that `mod.rs` files remain simple.
5. Keep the architecture unchanged while filling in implementations. A need to
   alter the agreed design must be discussed before replacing the tree.

Deliverable: preserved reference source and the correct maintained module tree.
The temporary migration may be uncompilable; no compiler or lint runs here.

## Stage 2: make DynamicImage authoritative for mode

1. Expand the existing raster enum into distinct mode-bearing variants while
   preserving its typed pixel buffers and existing high-depth representations.
   Use the guide's exact 25 mode identities and folder mapping. Give the
   payload-free `raster::Mode` selector the same variant identities, and read
   the materialized mode directly from the `DynamicImage` variant tag.
   Both types belong to pillow-rs. Keep image-slash-star codec types and mode
   descriptors independent, following the guide's ownership boundary.
   All backends consume this same Mode type; do not create backend-local enums.
   Representability in Mode does not imply support by any particular operation.
2. Give construction, decoding, raw-byte import, and conversion explicit output
   variants. Validate required sample shape, palette, and alpha invariants at
   those boundaries while retaining existing pixel-value behavior.
3. Preserve identity through clone, crop, transpose, resize, and every other
   mode-preserving result. Audit mode-changing operations and select their
   resulting variants explicitly; do not blindly inherit the source mode.
4. Keep lazy graph mode information consistent with materialized output.
   Resolve planned transitions when the graph changes. Python parses Pillow
   strings before calling typed Rust operation APIs; bindings and explicit
   I/O adapters own string parsing/formatting. Keep core mode parameters and
   strategy declarations typed, including internal high-depth identities.
5. Reconcile `ColorMode`, `PixelMode`, decoded metadata, `explicit_mode`, and
   pipeline caches. Retain genuinely distinct parameter roles, but remove
   duplicated image identity and channel-based inference from active execution.
6. Update binary inputs, masks, merge bands, pasted images, and fused outputs so
   their own exact modes travel with them. Preserve palette and source metadata
   through the appropriate Pillow wrapper dependencies.
7. Adapt callers and exports across core and bindings as required to compile
   against the new type. Map decoded codec metadata once in the Pillow-owned
   decode adapter and translate back at the encoding boundary. Preserve
   palette, sample, alpha, byte-order, conversion, and error behavior without
   adding Pillow execution metadata to image-slash-star types.

Deliverable: mode is determined once at admission or a mode-changing operation;
maintained CPU code does not recover it from strings or storage layout.

## Stage 3: inject Backend and Architecture

1. Define immutable execution context and the centralized selection policy.
   CPU with Default architecture uses the serial default implementation.
2. Respect existing explicit/backend-auto-routing scenarios. Resolve architecture
   from guaranteed compilation features or the one-time portable capability
   policy described in the guide. Do not repeatedly detect it in operations.
3. Thread the context through the backend interface, registry function types,
   eager execution, lazy materialization, pipeline batches, admission, fusion,
   nested operations, and fallback calls. Audit every call edge, not just the
   top-level pool method.
4. Eliminate fresh default contexts inside helpers and context-free escape
   routes. Geometry helpers calling resize, thumbnail calling reduce/resize,
   filters calling shared kernels, and multi-image operations keep the supplied
   selection or an explicit validated fallback derived from it.
5. Replace independent SIMD registry and pool selection with CPU architecture
   selection. Adapt legacy public SIMD requests at admission if compatibility
   requires it. Keep actual backend, requested architecture, selected
   implementation, and fallback reason observable.
6. Remove feature-dependent Rayon execution from maintained CPU paths. Enabling
   the parallel feature must not change CPU into a parallel executor.
7. Add the shared operation-support contract and registry delegation described
   in the guide. Expose the complete contract from each operation's identity
   and `PipelineOp`, including direct operations outside the graph. Separate
   build availability, request assessment, and actual prepared selection.
8. Route admission and execution through a prepared strategy decision. Inject
   validated inputs and dependencies into the plan; retain requested/selected
   backend, architecture, implementation identity, and fallback reason. Keep
   unresolved lazy guards deferred until they can actually be evaluated.

Deliverable: every maintained CPU execution route receives its selection from
the injected context, and every image supplies its exact mode.

## Stage 4: implement every CPU family and fold SIMD

Complete every family assigned by the structure guide, including all 84
pipeline variants and the direct CPU entry points found in Stage 0.

For each family:

1. Read the preserved implementation and all callers. Record mode-specific
   arithmetic, output identity, rejection behavior, and special fast paths.
2. Move the serial algorithm substantially as-is into the mode's Default owner.
   Split mixed mode branches into their correct implementations without
   changing arithmetic, rounding, clipping, border handling, or sample layout.
3. Move existing vector kernels into the corresponding architecture owners.
   Preserve instruction requirements, scalar tails, dimensions, alignment,
   and precision guards. Do not add empty architecture implementations.
4. Define each operation's explicit support declaration at its entry point.
   Compose its mode and architecture strategies with small exhaustive
   orchestration and concrete prepared plans. The same linked declarations
   drive support queries, eligibility, and executable selection; do not add a
   separate support table that can disagree with dispatch. Use context to
   select an eligible specialization or the same mode's Default strategy.
5. Place real shared helpers according to the guide. Reuse identical algorithms
   with direct calls and injected dependencies; preserve specialized hot loops.
   Do not duplicate kernels merely to populate folders. Do not cast, convert,
   or retag an image to another mode merely to reuse its implementation; each
   mode strategy processes its native representation. Only conversions defined
   by the operation's Pillow semantics may change mode, and the support/output
   contract must state that transition.
6. Preserve output mode, palettes, alpha conventions, and metadata. Unsupported
   modes retain their existing public exception behavior rather than silently
   converting to a supported mode. Additional core-only mode support is
   allowed when it has a native representation and explicit strategy; mark
   each implementation `NO_PILLOW_SUPPORT` and keep it outside Pillow parity.
7. Rewire every caller and remove superseded active functions and exports.
   Retire maintained `pool_simd` code only after every implementation and
   public compatibility route has an accounted-for CPU destination.
8. Update the inventory and mark source review complete. Keep arch-specific
   reachability findings explicit. Record strategy identities and guards for
   every operation independently, including siblings in the same family.
   Do not execute tests for individual families.

Deliverable: the entire CPU backend is written in the required structure.
There are no placeholders, runtime legacy dependencies, unclassified operation
paths, or hidden serial/parallel alternatives inside CPU code.

## Stage 5: finish all source work before validation

Complete a source review of the entire migration before running any compiler,
formatter, lint, test, parity, benchmark, or coverage command.

- Account for every legacy export, pipeline variant, direct call, mode,
  specialization, and caller edge in the inventory.
- Any implementation route left for later must have an actionable `TODO` in
  its owning maintained source module and a matching inventory entry. Name the
  operation or call edge and the next concrete migration step; a broad plan
  statement alone does not count. Keep these TODOs until the route is migrated
  and reviewed, and do not treat their presence as completion evidence.
- Confirm exact mode identity and output transitions, including `1`, palette
  modes, premultiplied alpha, LAB, numeric samples, byte order, and high depth.
- Confirm pillow-rs owns Mode and its mode-bearing DynamicImage, with codec
  translation confined to the decode/encode boundaries. Python parses strings
  before typed core operation calls; image-slash-star retains its own types.
  Inspect operation arguments, support descriptors, and prepared strategies
  together: all use the same Pillow-owned enum, with no string or codec-type
  translation inside compute.
- Confirm Backend and Architecture flow from routing through nested operations.
  Default is the fallback for every supported mode missing a specialization.
- Inspect every operation's top-level support contract and its executable
  bindings. All 25 modes have explicit outcomes; multi-input constraints,
  output identity, architecture guards, and fallback reasons are visible.
  Include rejected modes without implementation folders, and distinguish
  declared support, request eligibility, actual selection, and verification.
  Admission and execution consume the same prepared choice. Confirm support
  views are derived from registrations rather than directory inspection or
  an independent boolean registry. Keep evidence status distinct from support.
- Confirm maintained CPU execution contains no Rayon scheduling, parallel
  flags, repeated CPU detection, or separate SIMD executor.
- Inspect the actual tree against the guide. Keep orchestration out of
  `mod.rs` and retain ordinary named implementation files in architecture owners.
- Finish compatibility edits, meaningful type/context contract tests when
  needed, path-classification rules, source crosswalks, maps, and documentation.
  Preserve existing parity inputs, expected outputs, and assertions; only
  mechanical test import/type wiring may follow the new API.
- Review file and function size by reading/splitting source. Target 1,000
  physical lines per Rust file and 100 Clippy-counted body lines per function.
  The exact automated function-size proof belongs to the next stage.
- Remove dead maintained code and temporary scaffolding after accounting for
  all feature and architecture configurations. Leave the reference archive inert.

Record this milestone honestly: source review is complete, but compilation,
parity, and performance are not yet proved. Once all items above are complete,
proceed directly to validation without an extra approval request.

## Stage 6: compile and run static quality gates

Only now run the existing Makefile gates. Fix failures without weakening rules,
adding blanket exclusions, suppressing warnings, or inflating size baselines.

1. Refresh the source map and ownership inventories using the maintained
   Makefile targets. Ensure new paths and the inert archive are classified
   accurately. Preserve checks for every maintained file.
2. Run `make quality`, the consolidated static gate containing formatting,
   checkstyle/Clippy, documentation, and repository checks. Do not separately
   repeat the same successful gate through its `lint` alias.
3. Run the structure and crosswalk contract checks affected by their changes,
   plus meaningful affected Rust tests through existing targets or focused
   native commands when no appropriate target exists. Include focused strategy
   contracts for exhaustive support classification, assessment/preparation
   agreement, unavailable/ineligible architecture fallbacks, preserved errors,
   and actual implementation selection without hidden mode conversions.
4. Build the CPU parity target with `make build-parity`. Keep Rayon opt-in and
   disabled for this CPU build. This target avoids installing the replacement
   `PIL` over the live Pillow oracle.
5. Check feature/target configurations affected by the enum and context changes.
   Record which architectures can be compiled and executed on available hosts.
   Do not claim an unavailable architecture was runtime-verified.

Useful maintained inventory targets are:

```sh
make implementation-structure-audit implementation-structure-inventory
make repo-map-update repo-map-check
make operation-source-crosswalk operation-source-crosswalk-test
```

After a repair, rerun failed or directly affected checks. Do not rerun successful
unaffected campaigns. Coverage collection is outside this migration campaign.

## Stage 7: run the entire CPU parity suite

Run the complete fixed CPU-applicable denominator after Stage 6 succeeds.
The existing full-suite invocation is:

```sh
MIGRATION_TARGET_BACKEND=cpu \
MIGRATION_PARITY_EXECUTION_OUTPUT=build/migration-parity/cpu-refactor.execution.json \
make migration-parity-test \
  MIGRATION_PARITY_OUTPUT=build/migration-parity/cpu-refactor.parity.json
```

The command selects the full manifest because no case, surface, or limit filter
is supplied. Confirm the frozen selection and CPU profile identities in the
result. Keep oracle and target processes isolated. This command is documented
from the baseline Makefile; it has not been executed for this plan.

The architecture-aware CPU runner must make its chosen architecture explicit
in evidence, using the prepared plan's actual selection and fallback reason.
Run the complete suite for Default and each independently selected
CPU architecture route executable on the available host; unsupported
specializations use the documented Default fallback. Adapt architecture
selection plumbing as needed, without changing test cases or their assertions.
Cross-compilation is not a substitute for execution on another architecture.

Success requires all selected CPU cases to pass, zero implementation failures,
zero infrastructure errors, and complete applicable execution evidence. Report
legitimate non-CPU-only contracts and platform exclusions separately. Do not
shrink the denominator, hide failures as skips, or accept output parity alone
as proof that a requested specialization ran.

Do not run GPU, WASM, the old independent SIMD backend, or Parallel CPU parity
in this stage. SIMD kernels are exercised as CPU architecture implementations.
Fix implementation failures; rerun failures and directly affected guards. Reuse
passing results only when their input, oracle, build, producer, and executed
path identities still match. If a repair changes all CPU paths, broader
revalidation is justified and its reason must be recorded.

## Stage 8: CPU performance checkpoint and handoff

Preserve the earlier performance requirement after parity passes: compare the
frozen CPU workload IDs with compatible pre-refactor evidence under quiet
conditions. Keep scalar CPU and architecture-specific CPU results identifiable.
Do not mix in Parallel CPU or GPU results, reduce samples, change workloads,
or hide a per-operation regression in an aggregate.

Use the maintained benchmark runner with explicit CPU-only backend selection;
the default benchmark target includes other backends and must not be run
unfiltered. Discover the current runner's selectors from source/help before
execution rather than inventing flags. Accept at most a 5% slowdown per
comparable operation/mode pair, with 1% or less the preferred outcome.
Allow at most one confirmation repeat of questionable pairs and record its
reason. Earlier repeats from a different implementation do not become fresh
evidence; do not exceed the user's repetition allowance without discussion.

Record a CPU completion report containing:

- The actual maintained tree and source-to-target inventory, with no missing
  pipeline or direct CPU paths.
- The mode-bearing enum, mode-transition review, context call edges,
  specialization owners, and explicit Default fallback paths.
- A top-level per-operation support view derived from the typed strategy
  declarations, including rejected combinations, build availability, guards,
  implementation identities, and selection/fallback receipts.
- Quality, compilation, affected-test, full CPU parity, execution-receipt,
  and benchmark results bound to source/build identities.
- All unresolved issues, unavailable architectures, and validation limitations.
  CPU implementation, compilation, quality, or parity failures block handoff.
  Performance failures must be resolved or explicitly accepted before calling
  the CPU stage complete.

Only after the CPU gate passes may work begin on `pool_parallel`. Give it its
own backend context and scheduling, reuse CPU implementations by composition,
and complete its code before its own static checks and separate Parallel CPU
parity campaign. CPU evidence remains a fixed checkpoint unless a subsequent
change actually affects a CPU path. GPU migration follows as separate work.

Keep Kata issues open until their exact work is complete and verified. Old
SIMD/GPU findings in `pillow-rs#brg0` are not closed by CPU parity. Commit only
completed, verified work at the agreed checkpoint; do not mix unfinished
experiments into that commit or force-push history as part of this goal.

## Goal prompt

Run the following to continue the current `pillow-rs` work. Starting the goal
authorizes its implementation and ordered validation stages; it does not
authorize another reset, changing the agreed structure, or relaxing tests.

```text
/goal In /Users/lazytrot/work/pillow-rs, continue docs/CPU_REFACTOR_PLAN.md
under the strict docs/IMPLEMENTATION_STRUCTURE.md contract. Preserve current
work and the baseline 3f9dfd403 backups; do not reset or repeat completed
archive moves. Use pool_cpu/legacy as reference only, with no active imports.

Finish CPU first. pillow-rs owns the flat 25-identity raster::DynamicImage
and matching payload-free Mode selector. DynamicImage::mode() reads the enum
variant; do not store an independently mutable mode/storage pair or infer
mode from buffers. Keep image-slash-star codec types independent; translate
metadata in Pillow-owned decode/encode adapters. Rust mode inputs use the
Pillow-owned enum; Python parses strings before typed Rust operation calls.
Codec pixel-format descriptors must not become a second mode authority.
Use that same enum in operation arguments, support descriptors, and prepared
strategies; keep string parsing and codec translation outside compute.
All backends use this same Mode type, with no backend-local copies. Its 25
identities describe representable rasters; each operation explicitly declares
which it accepts or rejects.

Expose each operation's complete typed support contract at its entry point.
Make it accessible through its OperationId and PipelineOp where applicable,
including direct operations outside PipelineOp. Folder presence is not proof
of support.
Explicitly classify all 25 modes and relevant multi-input combinations,
outputs, architecture requirements, build availability, request guards,
and rejection or fallback reasons. Compose a Mode Strategy with an
Architecture Strategy using concrete types and closed enums. The same linked
declarations must drive support reporting, admission, and prepared execution.
Include rejected identities in the top-level view even without mode folders;
keep declarations, request eligibility, actual selection, and evidence distinct.
Make requested and actual implementation selections inspectable.
Allow support enumeration without materializing pixels; expose unresolved
content requirements as deferred guards, and consume the prepared selection
without independently selecting again during execution.

Inject Backend and Architecture end to end; resolve capabilities centrally
once, with no leaf hardware probes or mode inference. Fold SIMD into CPU at
pool_cpu/ops/<family>/<mode>/<arch>/<files>, retain same-mode serial Default
fallbacks, keep mod.rs limited to declarations/exports, and keep Rayon out of
CPU. Reuse proven shared helpers by composition without duplicating kernels.
Preserve algorithms, errors, representations, specialized hot loops, and
test/benchmark contracts.

Finish all CPU code and source review, including the 100-line function and
1,000-line file limits, before running any build, formatter, checkstyle,
lint, test, parity, or benchmark. Then run the planned static/compilation
gates, entire frozen CPU parity suite, and CPU performance comparison with
the documented 1-5% allowance and at most one confirmation repeat. Run no
coverage. Prove CPU completion before proceeding to pool_parallel and its
separate context, scheduling, and parity; other backends follow afterward.

Use kata to track tasks under pillow-rs#3ptb and related pillow-rs#brg0.
Keep status and remaining work current; close only verified completed work
and commit completed, verified checkpoints. Discuss necessary design changes
or genuine blockers before deviating; continue routine work autonomously.
```
