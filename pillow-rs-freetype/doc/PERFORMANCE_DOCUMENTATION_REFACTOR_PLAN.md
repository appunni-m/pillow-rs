# FreeType Performance, Documentation, And Rust Refactor Plan

> Status: project plan.  
> Scope: `pillow-rs-freetype` pure-Rust runtime plus its fixture, benchmark,
> and documentation system.  
> Hard gate: no refactor may reduce any existing parity count or weaken any
> fixture/test threshold.

## Goal

Move `pillow-rs-freetype` from parity-first implementation toward production
quality Rust while preserving exact FreeType-compatible behavior.

The refactor has three equal goals:

1. Performance: measure and improve Rust throughput per public operation
   against a version-matched C FreeType oracle.
2. Documentation: make public and parity-sensitive internals professionally
   documented with units, byte layouts, coordinate systems, invariants, and
   upstream behavior.
3. Rust quality: reduce unnecessary allocation, cloning, and borrow-workaround
   structure while keeping the code vectorization-friendly and test-proven.

## Non-Negotiable Constraints

- Runtime remains pure Rust. No `freetype-sys`, `bindgen`, `extern "C"`,
  `dlopen`, native FreeType calls, or linked C shortcuts in `src/`.
- C FreeType may be used only by fixture generation, live oracle tests,
  benchmark oracle binaries, and trace diagnostics.
- Every fixture row must remain exact: pixels, bitmap bytes, metrics, bbox/cbox,
  offsets, and 26.6 values.
- Refactors must be small enough to review and bisect.
- A performance improvement is accepted only with both parity and measurement.
- Do not replace clear code with unsafe/SIMD tricks unless benchmarks prove the
  need and tests cover the path.

## Current Baseline

Functional parity is currently strong:

- `coverage_matrix_tests`: native `7640/7640`, force-autohint `22168/22168`,
  supplemental lanes `11086/11086`.
- `imagingft_matrix_tests`: every row exact, including historical incomplete
  rows and `7520/7520` large pixel rows.
- `no_runtime_ffi`: runtime crate contains no native FreeType/FFI shortcuts.

Existing benchmark-like coverage:

- `tests/direct_ft_compare.rs` compares pixels against a live C FreeType helper,
  but it is an ignored correctness oracle, not a performance benchmark.
- `doc/FIXTURE_SPEED_BOTTLENECKS.md` documents fixture runner speed issues,
  especially repeated font construction and subprocess spawning.

Missing pieces:

- No stable per-operation Rust-vs-C benchmark framework.
- No checked performance baseline artifact.
- No allocation/clone budget per operation.
- Documentation quality is uneven across public and parity-sensitive internals.
- Some modules still carry C-port structure that is correct but harder for Rust
  to optimize.

## Performance Framework

### Benchmark Targets

Add a deliberate benchmark system under `pillow-rs-freetype/benches/` and
`pillow-rs-freetype/scripts/`.

Operations to benchmark independently:

- Font load: `Font::truetype`, `Font::truetype_face`.
- Scalar metrics: `getname`, `getmetrics`, `getlength`, `getbbox`.
- Glyph metrics: `glyph_metrics`.
- Rendered masks: `getmask` default native TrueType.
- Force autohint rendered masks.
- No-hinting, mono, LCD, outline bbox/cbox fixture lanes.
- Imaging adapter text operations from `pillow-rs`: bbox, length, mask, draw.

Inputs must include:

- Single glyph Latin, non-Latin, empty glyph, composite glyph, and large outline.
- Multi-character strings with kerning (`AV`), negative left bearing (`jQ`),
  and ordinary text (`Hello`).
- Sizes from the existing matrices: small, medium, and large point sizes.
- Fonts from the existing fixture inventory, not hand-picked one-offs.

### Rust Benchmarks

Use `criterion` or Cargo's stable benchmark-compatible harness with explicit
release-mode commands. Preferred initial shape:

```text
pillow-rs-freetype/benches/
  operation_bench.rs       # Rust-only operation timing
  c_oracle_bench.rs        # optional live C timing, ignored unless helper exists
pillow-rs-freetype/scripts/
  bench_freetype.py        # orchestrates Rust and C runs, emits JSON
```

Rust benchmark output must include:

- operation name
- font id
- point size
- codepoint or text
- load mode/render mode
- iterations
- median, mean, p95
- output checksum to prevent dead-code elimination

### C FreeType Benchmark Oracle

Add a C helper separate from runtime code:

```text
pillow-rs-freetype/scripts/bench_ft_ops.c
```

The helper should batch operations in one process. Do not spawn one C process per
glyph in performance comparisons.

Required properties:

- linked only by scripts/build helpers, never by `pillow-rs-freetype` runtime
- same FreeType version as fixture oracle
- same input matrix as Rust benchmark
- same output checksum format as Rust benchmark
- reports timing in machine-readable JSON lines

### Performance Comparison Contract

The benchmark runner should emit a checked artifact:

```text
pillow-rs-freetype/target/freetype-bench/latest.json
```

Each row:

```json
{
  "operation": "getmask",
  "font": "DejaVuSans.ttf",
  "size": 20,
  "input": "AV",
  "rust_ns_per_iter": 12345,
  "c_ns_per_iter": 6789,
  "ratio_rust_to_c": 1.82,
  "output_sha256": "...",
  "oracle_sha256": "..."
}
```

The benchmark must fail if output hashes differ. Performance comparisons are
meaningless unless parity is preserved.

### Performance Gates

Initial gate:

- record baseline only; no ratio failure.

After two stable baselines:

- fail if an operation regresses by more than 10% on median time unless an
  explicit baseline update file explains the reason.
- fail if allocation count increases for hot operations after an allocation
  counter is added.

Mandatory command set:

```bash
cargo test -p pillow-rs-freetype --test coverage_matrix_tests --locked -- --nocapture
cargo test -p pillow-rs --test imagingft_matrix_tests --locked -- --nocapture
cargo test -p pillow-rs-freetype --test no_runtime_ffi --locked -- --nocapture
cargo bench -p pillow-rs-freetype --bench operation_bench --locked
python3 pillow-rs-freetype/scripts/bench_freetype.py --compare-c
```

## Refactor Lanes

### Lane 1: Allocation And Clone Audit

Create a tracked audit:

```text
pillow-rs-freetype/doc/ALLOCATION_CLONE_AUDIT.md
```

Audit commands:

```bash
rg -n '\.clone\(|to_vec\(|Vec::new\(|Vec::with_capacity|collect::<Vec|collect\(\)' pillow-rs-freetype/src
cargo clippy -p pillow-rs-freetype --all-targets --all-features --locked -- -D warnings
```

Classification:

- required ownership boundary
- cache snapshot
- borrow-workaround clone
- avoidable temporary allocation
- hot-loop allocation

Acceptance:

- remove avoidable clones only when parity and benchmarks pass
- replace hot-loop `Vec::new()` with scratch reuse only when code stays clear
- prefer `&[T]`, iterators, and small `Copy` snapshots over owned vectors

### Lane 2: Scratch Buffers And Data Layout

Targets:

- glyph scaling point buffers
- autohint segment/edge scratch
- rasterizer cell storage
- text layout temporary strings in imaging adapter paths

Rules:

- no shared mutable global scratch
- no hidden cross-call state that breaks thread safety
- scratch lifetimes should be owned by `Font`, `FaceGlobals`, or explicit
  operation context types
- data layout changes must include before/after benchmark rows

Preferred direction:

- use flat contiguous buffers for point/edge/cell arrays
- use `clear()` and `reserve()` for reusable buffers
- keep small `Copy` structs compact and cache-friendly
- avoid large enum variants in hot structs

### Lane 3: Operation Contexts

Introduce explicit context types where they reduce repeated setup:

```rust
struct GlyphLoadContext<'font> {
    font: &'font Font,
    load_mode: LoadMode,
}

struct RenderContext<'font> {
    glyph: GlyphLoadContext<'font>,
    render_mode: RenderMode,
}
```

Do not add abstractions that only rename functions. Contexts must remove real
duplication, make ownership clearer, or let scratch buffers be reused safely.

### Lane 4: Fixed-Point And Vectorizable Loops

Targets:

- point scaling loops
- outline translation
- bbox/cbox scans
- raster cell sweep
- TrueType VM vector math

Rules:

- preserve FreeType fixed-point rounding exactly
- keep helper functions small and deterministic
- prefer slice-based loops that LLVM can optimize
- use `chunks_exact`, `zip`, and index-free iteration where it does not obscure
  the FreeType algorithm
- do not introduce SIMD until scalar benchmarks identify a bottleneck

Any vectorization-oriented change must include:

- representative benchmark rows
- parity command output
- comment only when the optimized shape is not obvious

### Lane 5: Public API Cleanup

Follow Rust naming conventions while respecting compatibility:

- new APIs should avoid unnecessary `get_` prefixes
- existing public APIs with Pillow/FreeType-compatible names may remain when
  compatibility is clearer than idiom
- any rename must preserve old API through a documented compatibility wrapper
  until downstream crates migrate

API docs must state units:

- pixels
- 26.6 fixed-point pixels
- font units
- byte rows/stride
- left/top/right/bottom coordinate inclusivity

### Lane 6: Harness And Generator System

Fixture generators are part of the product:

- benchmark input generation must live under `scripts/`
- generated benchmark matrices must be deterministic
- every generator needs usage docs and provenance fields
- no one-off scripts for fixture or benchmark generation

Add a benchmark matrix file:

```text
pillow-rs-freetype/tests/fixtures/perf_operation_matrix.json
```

It should be smaller than parity matrices but cover every operation family and
known regression shape.

## Documentation Plan

### Rustdoc Gates

Add strict documentation checks in phases:

Phase 1:

```bash
RUSTDOCFLAGS="-D warnings" cargo doc -p pillow-rs-freetype --no-deps
cargo test -p pillow-rs-freetype --doc
```

Phase 2:

- enable missing-doc tracking for public API modules
- keep internal-public items documented by contract, not filler
- add doctests only where examples are stable and useful

### Module Docs To Upgrade

Priority order:

1. `src/lib.rs`: crate purpose, runtime purity, FreeType parity scope.
2. `src/font.rs`: public `Font`, load modes, metrics, mask contracts.
3. `src/render.rs` and `src/grays.rs`: bitmap modes, rows, coverage semantics.
4. `src/scaler.rs` and `src/fixed.rs`: fixed-point units and rounding parity.
5. `src/tt/*.rs`: table parser byte layout and supported formats.
6. `src/autohint/*.rs`: pipeline stages and algorithm references.
7. `src/tt/hinter/*.rs`: VM state, stack/storage, graphics state invariants.

### Documentation Quality Bar

Professional docs must include:

- what the item does
- input units and byte layout
- returned units and ownership
- error conditions for `Result`
- parity source when behavior mirrors FreeType C
- invariants for public structs and internal-public helper types
- examples only when they compile and teach real usage

Avoid:

- docs that restate the identifier
- broad claims like "fast" without measurement
- stale TODOs
- unexplained `#[allow(missing_docs)]`

## Review Checklist For Every Refactor PR

Before merge:

- [ ] No runtime FFI shortcuts.
- [ ] No fixture, oracle, threshold, or expected-output weakening.
- [ ] `coverage_matrix_tests` unchanged or improved.
- [ ] `imagingft_matrix_tests` unchanged or improved.
- [ ] `no_runtime_ffi` passes.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy -p pillow-rs-freetype --all-targets --all-features --locked -- -D warnings` passes.
- [ ] Benchmarks run for touched operation family.
- [ ] Performance result recorded when hot code changed.
- [ ] Public docs updated for changed contracts.
- [ ] No temporary debug prints.
- [ ] New abstractions remove measurable duplication or clarify ownership.

## Suggested Execution Order

1. Add benchmark framework skeleton and operation matrix.
2. Add Rust-only benchmark rows and output checksums.
3. Add batched C FreeType benchmark helper and comparison script.
4. Document baseline results.
5. Run allocation/clone audit and classify hot-path clones.
6. Refactor one hot path at a time:
   - measure before
   - refactor
   - run parity
   - measure after
   - document result
7. Upgrade rustdoc module by module.
8. Add stricter lints only after code is ready, not before.

## Initial Candidate Refactors

1. Avoid repeated `String` allocation in text layout paths by exposing a
   codepoint/glyph-slot API in `pillow-rs-freetype`.
2. Make autohint globals lazy for default native TrueType load paths that do
   not need autohint metrics.
3. Convert rasterizer scanline cell storage from many small vectors to a
   reusable flat buffer if benchmarks confirm allocation cost.
4. Replace borrow-workaround clones in autohint passes with small `Copy`
   snapshots or split structs.
5. Split table raw-data ownership from parsed table views where it can remove
   `to_vec()` copies without lifetime complexity leaking into public API.

Each candidate starts with a benchmark and parity baseline. None is accepted on
style grounds alone.
