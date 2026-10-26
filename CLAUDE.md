# pillow-rs agent guide

`AGENTS.md` and `AGENT.md` link to this file. Edit `CLAUDE.md` only.

## Scope and ownership

- `pillow-rs/` owns image algorithms and backend execution in Rust. Do not use
  native Pillow, FreeType, or codec libraries as runtime substitutes.
- `pillow-rs-py/` and `pillow-rs-js/` own host conversion, I/O, validation, and
  delegation. Keep image algorithms in core. The public Python namespace is
  `PIL`; `pillow_rs` contains internal binding modules.
- Draw in the image's native pixel format. Mode conversion must be explicit.
- Keep format-specific image paths format-specific: process supported modes such
  as `1`, `L`, `LA`, `P`, `PA`, `RGB`, and `RGBA` directly where their
  semantics allow it. Do not convert everything to RGBA merely to share an
  implementation.
- Preserve specialized hot loops, inlining, and direct backend selection; do
  not add generic or dynamic dispatch to pixel paths merely to reduce source
  duplication. When a reuse change affects the executed path, compare full
  public-call latency and reject regressions. Follow the shared structure guide
  as the sole authority for adapter, family, and kernel placement and roles.
- Use [the repository implementation structure guide](docs/IMPLEMENTATION_STRUCTURE.md)
  as the single source for placement of maintained files across source, tests,
  tools, docs, evidence, fixtures, assets, and workflows. Other documents may
  map ownership or link to it, but must not define a competing path grammar.
  Treat existing outliers as migration debt and align a touched area only when
  behavior and performance are preserved. Keep uncertain or architecture-
  specific reachability findings reviewable; never delete them automatically.
- The `parallel` Cargo feature is opt-in and must stay out of default features
  in both `pillow-rs` and its bindings. A normal Python build must not silently
  enable Rayon.
- A PyPI extra only selects optional distribution dependencies; it does not
  change Cargo features in an existing wheel. `pillow-rs[parallel]` therefore
  depends on the separately built `pillow-rs-parallel` companion wheel, whose
  `_core_parallel` extension is selected by the Python loader when installed.
  Keep that wheel's version pin, release artifacts, import route, and
  `parallel-wheel` build feature synchronized. Source builds and Parallel CPU
  benchmarks continue using the canonical `_core` module with Cargo's opt-in
  `parallel` feature.
- Rayon is a CPU execution strategy. Rayon-backed work belongs to the separately
  named and benchmarked **Parallel CPU** profile, not SIMD. Reserve SIMD for
  operations with actual architecture-specific vector code; do not create a
  Parallel SIMD profile just to label Rayon work.
- Explicit GPU batching accepts the existing lazy Image/PipelineOp graph,
  preserves result identity independently of order, rejects unsupported pending
  contexts during admission, and keeps resource leases through completion.
  Keep it separate from normal Image routing; do not restore deprecated ImageBatch
  operation wrappers or use host thread pools as GPU batching.
- Keep GPU work on the GPU path. GPU dispatch and kernels must not use Rayon;
  never route GPU work through CPU parallel scheduling. Measure GPU separately
  and optimize its own latency and throughput.
- Report and compare Parallel CPU independently from serial CPU, SIMD, and GPU,
  including its feature configuration and Pillow comparison. Do not fold its
  results into the default CPU profile or claim them as SIMD/GPU performance.
- Preserve exact Pillow parity before accepting any speedup. Compare each
  operation and mode separately: the targets are no serial-CPU operation slower
  than Pillow, SIMD at least 5x faster than Pillow, and GPU latency matching
  SIMD with higher throughput. Do not hide a per-operation regression in an
  aggregate score.
- fontdone and image-slash-star are separate repositories. Follow their own
  instructions when a task includes them. `build/fontdone-src/` may contain
  active standalone work; preserve it and do not reset it to `FONTDONE_REF`.
  Root `fontdone-*` targets require the configured pin; use the standalone
  checkout's Makefile for work on its current revision.
- Preserve unrelated changes and existing safety/lint checks. Keep library
  diagnostics in `log` macros; do not commit temporary prints or traces.

## Development workflow

- Use root `make quality` as the consolidated static quality gate; `make lint`
  is a compatibility alias. `make ci` adds fixture, parity, and coverage
  verification.
- Keep first-party Rust files at or below 1,000 physical lines and functions
  at or below 100 Clippy-counted body lines. Review files from 800 lines.
  Function size has no lower review threshold. Existing violations
  shrink against a checked-in baseline; new over-limit code and growth beyond
  the baseline fail the quality gate. Include first-party tests and use only
  the exact exclusions in the checker. See
  [the Rust size policy and legacy inventory](docs/RUST_SIZE_POLICY.md) for
  counting rules, owners, and reducer issues. After reviewed reductions, use
  `make checkstyle-update-baseline` and inspect the resulting baseline diff.
- Read the affected code and its callers first. Reuse existing functions, types,
  and backend paths before adding new implementations.
- Make the smallest cohesive change that solves the problem. Preserve existing
  APIs unless the task requires changing them. Refactor only as needed for the
  task; avoid broad rewrites, duplicate implementations, unrelated cleanup, and
  speculative abstractions.
- During development, use focused builds, reproductions, or diagnostics when
  they resolve a specific uncertainty. Routine verification belongs at Git push,
  not after each edit or local commit.
- Before parity or benchmark execution, freeze the affected case/workload IDs,
  backend profiles, and necessary guards. Reuse passing evidence for unchanged
  inputs, source/build, producer, oracle, and executed paths. Retry only failures
  or directly affected guards, recording the reason; do not repeat full campaigns
  after each change. A proof-tool change requires its focused contracts, not a
  new image campaign.
- Select explicit workload IDs for benchmarks and case IDs for parity. Run only
  the operation being changed, its affected modes/backends, and justified
  regression guards. Reuse passing parity only for the same input contract and
  verified source/build variant. Do not repeat full campaigns during each
  optimization; broaden verification only when the push affects that scope.
- Fix implementation failures without weakening assertions, thresholds, or the
  selected contract. Do not special-case fixture identities in runtime code.
- For Pillow comparisons, use `make build-parity` and isolated processes or
  environments. `make build` installs the replacement `PIL` namespace and can
  overwrite the oracle in that environment.
- Before benchmarking or parity, select explicit affected workload/case IDs,
  backend profiles, and necessary regression guards. Preserve their workloads,
  thresholds, sample counts, and materialization boundaries. Reuse passing
  parity only when source, extension, input, producer, oracle, and profile
  identities match; dirty commit IDs alone are insufficient. Repeat only
  affected failures or flagged benchmark pairs, with a recorded reason. Do not
  repeatedly run the full benchmark or parity campaign during an operation
  experiment. Run no coverage in the current optimization campaign.
- Record subtle reference behavior beside the implementation. Keep user guides
  separate from contributor procedures, and retain README acknowledgements.

## Git integration

- Default to one completed work item per commit/push checkpoint. Count an item
  after its focused verification; keep separate in-progress experiments out of
  that checkpoint. Increase the batch only when the user explicitly requests it.
- Keep `main` as the integration branch. Fetch `origin/main` before integrating,
  and bring completed, scoped-verified work into `main` at the agreed batch
  boundary so it does not remain only on a topic branch or worktree.
- Before integration, inventory attached worktrees and local refs, including
  recovery refs and branches not reachable from `main`. Preserve them; do not
  prune or delete dangling refs during routine cleanup. Review unique branch
  work individually and merge only validated changes; do not bulk-merge stale
  experiments. Preserve dirty worktrees unless their changes are explicitly
  incorporated.

## Verification before Git push only

Run this checklist immediately before an authorized `git push`, or when the user
explicitly requests verification. Use existing Makefile targets; `make help` and
`make help-all` list them. Direct commands are fine when no suitable target
exists. Add a target only when repeated use justifies maintaining it.

- Documentation changes: `make docs-lint`.
- Site changes: also `make docs-test docs-build`.
- Rust changes: affected tests and `make fmt clippy`.
- Behavior changes: affected parity lanes.
- Added, moved, or removed files:
  `make implementation-structure-audit implementation-structure-inventory
  repo-map-update repo-map-check`.

Check the final changes once; repeat only affected checks after further changes
or failures. Broader campaigns are needed only when the push affects their
scope. Report failures and checks that could not run; do not describe old
results as fresh.

## References

Consult these as needed for the task. This guide controls when verification
runs; linked contributor checklists are not additional development chores.

- [Contributing](CONTRIBUTING.md): setup and change workflow.
- [Command reference](docs/COMMANDS.md): targets, prerequisites, and side effects.
- [Architecture](docs/ARCHITECTURE.md) and [repository map](docs/REPO_MAP.md):
  ownership.
- [Repository implementation structure](docs/IMPLEMENTATION_STRUCTURE.md):
  family, adapter, kernel, binding, parity, and evidence placement rules.
- [Coverage](docs/COVERAGE.md) and [benchmarking](docs/BENCHMARKING.md):
  evidence collection and interpretation.
- [Releasing](RELEASING.md): version synchronization, package checks, and
  tag-triggered publishing.
