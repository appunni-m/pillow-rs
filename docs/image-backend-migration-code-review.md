# `pillow-rs` Image Backend Migration Code Review

Date: 2026-07-23

## Decision

The downstream migration uses the correct architectural boundary:
`image-slash-star` owns encoded-format behavior, while `pillow-rs` owns
Pillow-facing state, lazy operations, modes, palettes, and compute routing.
Source-format auto-detection is consumed from the canonical backend instead of
being reimplemented here.

Three confirmed runtime issues block acceptance of the lazy-loading slice:

1. extending a pipeline can discard an explicitly selected compute backend;
2. pipeline `verify()` changes observable materialization state despite its
   non-mutating contract;
3. every read of a loaded P-mode image clones the entire index buffer.

The other findings cover public invariants, palette proof, memory behavior,
feature forwarding, compatibility, tests, documentation, and commit hygiene.
They remain within the approved backend migration.

Upstream codec/source findings are owned by
`docs/image-slash-star-code-review.md` in the `image-slash-star` repository.

## Scope reviewed

- image codec feature forwarding and removal of the historical format handler;
- path and byte source construction, retained metadata, and auto-detection;
- lazy source and pipeline caches, load, verify, copy, and mutation behavior;
- exact decoded mode, palette, palette-alpha, and source-format retention;
- palette-safe operation classification and Pillow-oracle fixtures;
- structured error propagation and public compatibility;
- migration tests, fixture generation, documentation, and worktree hygiene.

Unrelated compute features, new Pillow operations, codec algorithm changes, and
the existing FreeType scalar failures are outside this review.

## Validation evidence

- The complete worktree diff and relevant open/materialize/load/verify/copy/
  pipeline/palette call paths were inspected.
- Feature declarations, Makefile lanes, fixture manifests, generators, and Rust
  tests were cross-checked.
- `git diff --check` passed.
- Per project direction, Coverage MCP was not used for `pillow-rs`; the backend
  migration tests are manual Cargo/Makefile lanes.
- `cargo test -p pillow-rs --all-features --test image_backend_migration
  --locked` passes all 8 tests after the palette-write slice. The lane compares
  exact Pillow 12.2.0 indices, palette bytes, palette alpha, encoded PNG bytes,
  persistent-load state, and structured operation errors.
- Strict Clippy currently fails with more than two thousand primarily existing
  arithmetic/cast diagnostics, so it is not yet a useful migration gate.

## Acceptance blockers

### P1 — Extending a pipeline discards its backend lock

Severity: high, confirmed correctness defect.

`pillow-rs/src/image.rs:726-743` destructures an existing `Image::Pipeline`,
appends the new operation, and builds a replacement pipeline. Its existing
`backend` field is ignored. The replacement uses `source.backend()`, where the
pattern binding named `source` is the underlying input node rather than the
pipeline being extended. A normal Path, Bytes, Loaded, or Paletted source
returns `None`.

Thus `pipeline.use_backend(Cpu)` followed by any additional operation silently
restores automatic routing. Materialization can then select a different
backend, including the separate palette-safe routing branch.

Recommendation:

- bind and retain the existing pipeline's `backend` when flattening it;
- rename the inner binding to `pipeline_source` to prevent shadowing;
- test a locked pipeline before and after one and multiple appended operations;
- cover clones, an unlocked pipeline, and a palette-safe pipeline;
- use a focused Rust invariant test because Pillow fixtures cannot observe
  internal backend selection.

### P2 — Pipeline verification mutates observable lazy state

Severity: high, confirmed contract defect.

`pillow-rs/src/image.rs:1552-1567` promises verification without changing
state. Path and byte variants independently call the canonical source's
`verify()`, but all other variants call `materialized_shared()`. For a Pipeline,
this initializes its `OnceLock`, changing `is_materialized()` from false to
true.

Recommendation:

- execute a pipeline independently during verification and discard the output
  rather than publishing it to the normal cache;
- assert non-materialization before and after successful and failing pipeline
  verification;
- repeat through a clone to prove the shared cache remains untouched;
- preserve a single non-mutating contract across source and pipeline variants.

The definition of whether encoded verification covers only the primary image
or every frame belongs to the upstream review.

### P3 — P-mode reads perform an unbounded hidden copy

Severity: high, confirmed performance and lifecycle-contract defect.

`pillow-rs/src/image.rs:584-588` returns an `Arc` clone for Loaded images but
constructs a new `DynamicImage` from `data.indices.clone()` for every Paletted
read. Repeated `tobytes`, `getpixel`, save, statistics, and similar operations
copy the complete index plane, including after `load()` has made the image
persistent.

This contradicts the documented rule that read-only access shares cached pixel
storage and full copies occur only for ownership-promising or copy-on-write
operations.

Recommendation:

- share immutable paletted indices or cache an `Arc<DynamicImage>` index view;
- keep palette mutation copy-on-write so clones remain isolated;
- add allocation or pointer-identity tests for repeated reads of directly
  loaded and pipeline-produced P images;
- retain exact indices, palette, alpha, and encoded-byte oracle assertions.

## Correctness and API findings

### P4 — Public enum fields make invalid lazy states constructible

Severity: medium-high API risk.

`LoadedData`, every field of `Image::{Path,Bytes,Pipeline}`, and
`MaterializationCache` are public. External code can pair a source with a
different format or `ImageInfo`, reuse a cache from another image, construct
incoherent mode/palette state, or assemble arbitrary pipeline graphs. Internal
constructors are coherent; the public representation breaks that guarantee.

Recommendation:

- make node representation internal and expose read-only accessors and checked
  constructors;
- if immediate privacy is too disruptive, mark the enum non-exhaustive and
  document invariants before encapsulating it in the same breaking release;
- do not silently make this change in a patch release because consumers may
  pattern-match or construct variants.

### P5 — Palette safety classifies tuple `putpixel` more broadly than proved

Severity: medium.

Every `PipelineOp::PutPixel` is classified as index-preserving. The operation
stores an RGBA tuple, while P-mode execution writes only `color.0` as an index.
Oracle fixtures prove scalar `putpixel_mode(..., "P")`, including a chained
case, but do not prove tuple-based public `Image::putpixel()` behavior on P
images.

Recommendation:

- distinguish an explicit palette-index operation from a color-tuple write, or
  reject/convert tuples according to Pillow behavior;
- classify only the proven index form as palette-safe until a tuple oracle row
  exists;
- cover both binding paths with exact success/error fixtures.

Implemented (July 2026):

- `PipelineOp::PutPixel` records whether its value is a proven palette index;
- scalar P-mode writes remain index-preserving;
- RGB tuple writes reuse or allocate a Pillow-compatible palette entry and
  retain the updated palette through lazy execution and persistent `load()`;
- non-opaque RGBA returns the exact Pillow `ValueError` message;
- the pinned operation manifest contains exact success and error rows, including
  exact PNG bytes before and after `load()`.

### P6 — Downstream retains duplicate full decoded pixel buffers

Severity: medium, measurable resource risk.

The canonical source retains encoded bytes and its cached `DecodedImage`.
`decoded_to_dynamic()` clones that pixel vector into another full buffer, while
the canonical decoded buffer remains reachable. P-mode load can add a further
conversion copy. Correctness is preserved, but large native and WASM images can
retain roughly two decoded buffers plus encoded input.

Recommendation:

- measure open, first read, repeated read, clone, mutation, and drop memory on
  native and WASM before changing ownership;
- then prefer a shared/owned handoff or an operation representation compatible
  with the backend buffer;
- preserve immutable clone semantics and cached deterministic failures.

## Feature and compatibility findings

### P7 — Feature forwarding is not proved one codec at a time

Severity: medium-high test gap.

The downstream test proves exact errors with no default features, and the
migration suite runs with all features. Those endpoints cannot detect an
individual feature wired to the wrong upstream feature because all-features
masks it and no-default enables neither side.

Recommendation:

- add an isolated lane for `image-jpeg`, `image-png`, `image-gif`, `image-bmp`,
  `image-tiff`, `image-webp`, `image-ico`, and `image-avif`;
- prove the selected format succeeds and unrelated formats return exact
  disabled-feature errors;
- explicitly assert ICO's transitive PNG/BMP behavior;
- retain the no-default and all-feature lanes.

### P8 — The migration contains public breaking changes

Severity: medium-high release risk.

The worktree changes `PilError::Io` from `std::io::Error` to
`Arc<std::io::Error>`, changes `LoadedData.image` to `Arc<DynamicImage>`, changes
public Image variant fields, and removes the old direct format-handler modules.
These choices support persistent shared state but break consumers that match or
construct those public values.

Recommendation:

- use an explicitly breaking version boundary;
- add a migration guide for error matching, pixel access, construction, codec
  features, and removed handlers;
- distinguish semantic behavior changes from mechanical `Arc` dereferencing;
- do not restore compatibility aliases named `pillow-rs-image`; the dependency
  and package remain `image-slash-star`.

## Test and fixture findings

### P9 — Palette-safe parameter boundaries are under-sampled

Severity: medium test-strength gap.

The 19 exact Pillow-oracle rows are valuable. Some safe classifications cover
more parameters than their proof: arbitrary rotation angle/expand combinations,
nearest output dimensions, affine matrices/fills, and boundary coordinates.

Recommendation:

- add a compact boundary matrix: zero/right-angle/negative/non-right-angle
  rotation, expand on/off, one-pixel resize/thumbnail, affine identity/
  translation/out-of-bounds, and first/last pixel writes;
- generate every row with the pinned Pillow oracle;
- compare exact mode, size, indices, palette, alpha, and encoded bytes;
- keep intentionally failing rows explicit in the manifest while debugging.

### P10 — The operation generator can leave stale fixtures

Severity: low-medium.

`scripts/generate_image_backend_operation_fixtures.py` writes current rows but
does not enforce a bijection between manifest entries and files in
`outputs/operations`. Renaming or removing a row can leave obsolete artifacts.

Implemented (July 2026): the pinned generator now computes the complete expected
`.bin`/`.png` set and removes stale operation artifacts before writing the
authoritative manifest.

Recommendation:

- compute the expected output set and reject or remove stale files only inside
  that exact generated directory;
- add a manifest-to-files completeness assertion;
- retain deterministic generation under the pinned Pillow version.

### P11 — Temporary path tests are not panic-safe

Severity: low.

The path-backed test uses a process-derived filename and manual cleanup. A
panic leaves it behind, and repeated cases in one process can collide.

Recommendation: use an RAII guard and unique suffix without adding a production
dependency solely for test convenience.

## Documentation and repository findings

### P12 — Migration documents overstate current guarantees

Severity: medium.

Current documents mark non-mutating verification, no hidden read copies,
cycle-safe construction, and feature forwarding complete. P2, P3, P4, and P7
show that these claims are stronger than current code or evidence.

Recommendation:

- mark affected rows partial until fixed;
- record the exact test lane supporting each completed contract;
- reconcile old Coverage MCP checkpoint wording with the decision not to use
  Coverage MCP for this repository.

### P13 — Strict Clippy is documented but not enforceable

Severity: medium process risk.

The documented strict command currently fails with more than two thousand
primarily existing diagnostics. A permanently red gate cannot identify new
migration regressions.

Recommendation:

- baseline existing lint debt and reject new diagnostics in touched code;
- burn down existing debt separately;
- do not weaken correctness lints or expand this migration into a repository-
  wide lint rewrite.

### P14 — Coverage tool state must not enter the commit

Severity: low-medium.

The worktree contains an untracked `.coverage-mcp/` directory, while the root
ignore file does not exclude it. It is unnecessary generated state and this
repository is not using Coverage MCP for this migration.

Recommendation:

- add `/.coverage-mcp/` to the root ignore rules in a hygiene change;
- ensure its database and reports are never staged;
- do not delete user tool data merely to prepare the migration commit.

### P15 — A one-off palette analysis script remains untracked

Severity: low.

`scripts/analyze_palette_rotate.py` is not referenced by a maintained test or
generator target. Repository instructions say one-off debugging scripts should
not remain.

Recommendation: exclude it from the migration commit, or promote it only if it
becomes a documented deterministic diagnostic. The permanent oracle fixture
generator should be retained.

### P16 — Unrelated FreeType formatting noise should remain separate

Severity: low.

`pillow-rs/tests/imagingft_matrix_tests.rs` contains formatting-only changes
unrelated to the image backend migration.

Recommendation: exclude it from the migration commit unless it belongs to a
separate repository-wide formatting commit. Preserve the existing unrelated
FreeType test state.

## Positive findings

- Auto-detection and codec behavior are consumed from `image-slash-star` rather
  than duplicated here.
- Path opening snapshots bytes, so later filesystem replacement cannot change
  the lazy source.
- Source and pipeline caches use shared, once-initialized, immutable results.
- Source format, exact mode, metadata, palette, and alpha survive the main
  decode/load path.
- Structured codec errors propagate through `PilError` without duplicate
  `try_*` APIs.
- Codec feature forwarding is explicit and default features correctly exclude
  optional native AVIF.
- Exact fixtures compare values and bytes, not merely output sizes.
- No unsafe Rust or new FFI boundary was introduced by this migration.
- `git diff --check` is clean.

## Resolution order

1. Fix P1, P2, and P3 with focused lifecycle/invariant tests.
2. Narrow and prove palette writes in P5 and add the P9 boundary matrix.
3. Add isolated feature lanes from P7.
4. Make the public representation/release decision in P4 and P8.
5. Measure P6 before changing buffer ownership.
6. Align documentation and gates under P12 and P13.
7. Prepare the commit using P10, P11, P14, P15, and P16.
8. Run formatting, exact migration tests, and every isolated feature lane before
   declaring the downstream slice complete.

## Review completion rule

Each finding is complete when fixed with the named evidence or explicitly
accepted as a documented compatibility or release tradeoff. This review does
not add a codec, operation, dependency, or Coverage MCP requirement.
