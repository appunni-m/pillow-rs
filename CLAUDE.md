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
- The `parallel` Cargo feature is opt-in and must stay out of default features
  in both `pillow-rs` and its bindings. A normal Python build must not silently
  enable Rayon.
- Rayon is a CPU execution strategy. Rayon-backed work belongs to the separately
  named and benchmarked **Parallel CPU** profile, not SIMD. Reserve SIMD for
  operations with actual architecture-specific vector code; do not create a
  Parallel SIMD profile just to label Rayon work.
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

- Read the affected code and its callers first. Reuse existing functions, types,
  and backend paths before adding new implementations.
- Make the smallest cohesive change that solves the problem. Preserve existing
  APIs unless the task requires changing them. Refactor only as needed for the
  task; avoid broad rewrites, duplicate implementations, unrelated cleanup, and
  speculative abstractions.
- During development, use focused builds, reproductions, or diagnostics when
  they resolve a specific uncertainty. Routine verification belongs at Git push,
  not after each edit or local commit.
- Fix implementation failures without weakening assertions, thresholds, or the
  selected contract. Do not special-case fixture identities in runtime code.
- For Pillow comparisons, use `make build-parity` and isolated processes or
  environments. `make build` installs the replacement `PIL` namespace and can
  overwrite the oracle in that environment.
- Record subtle reference behavior beside the implementation. Keep user guides
  separate from contributor procedures, and retain README acknowledgements.

## Verification before Git push only

Run this checklist immediately before an authorized `git push`, or when the user
explicitly requests verification. Use existing Makefile targets; `make help` and
`make help-all` list them. Direct commands are fine when no suitable target
exists. Add a target only when repeated use justifies maintaining it.

- Documentation changes: `make docs-lint`.
- Site changes: also `make docs-test docs-build`.
- Rust changes: affected tests and `make fmt clippy`.
- Behavior changes: affected parity lanes.
- Added, moved, or removed files: `make repo-map-update repo-map-check`.

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
- [Coverage](docs/COVERAGE.md) and [benchmarking](docs/BENCHMARKING.md):
  evidence collection and interpretation.
- [Releasing](RELEASING.md): version synchronization, package checks, and
  tag-triggered publishing.
