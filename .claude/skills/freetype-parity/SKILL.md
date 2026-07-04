---
name: freetype-parity
description: Use for pillow-rs-freetype parity work: pure-Rust FreeType implementation, fixture generation, harness execution, native TrueType, autohinter, rasterizer, metrics, outline bbox/cbox, failure classification, and subagent task splitting.
version: 0.1.0
---

# FreeType Parity

This skill is mandatory for `pillow-rs-freetype` parity work.

The project objective is 100% pure-Rust parity with version-matched C
FreeType. C is an oracle for generation and diagnosis only. Runtime code must
not call C.

## Hard Constraints

- Work only in `pillow-rs-freetype` for FreeType implementation changes.
- Do not use or revive `pillow-rs-font`.
- No runtime FFI: no `freetype-sys`, `bindgen`, `cc`, `extern "C"`, `dlopen`,
  native FreeType calls, or linked C shortcuts.
- Do not modify fixtures, oracle JSON, expected hashes, test thresholds, or
  comparison logic to hide failures.
- Exact lanes stay exact. Pixel bytes, bitmap bytes, metrics, bbox/cbox, and
  26.6 geometry must compare exactly where those lanes define exact parity.
- Temporary C/Rust prints are allowed only for diagnosis and must be removed
  before commit.
- Permanent Rust traces must use the guarded `log::trace!` pattern in
  `CLAUDE.md`.

## What Counts As Parity

Every public FreeType-backed endpoint must be covered by deterministic
comparisons with varied inputs.

- Rendered masks: compare pixel bytes or full bitmap hashes.
- Mono/LCD outputs: compare byte output, placement, stride, and dimensions.
- Metrics endpoints: compare exact metric fields.
- Outline endpoints: compare exact cbox/bbox and point geometry in 26.6 units.
- Error paths: compare the public error classification where applicable.

If a lane is incomplete, it must be visible as a failing parity count or an
explicitly named incomplete baseline. Never treat a baseline guard as parity.

## Harness Workflow

1. Start from clean main and record `git rev-parse --short HEAD`.
2. Run the narrow lane first.
3. Capture failure IDs with `-- --nocapture`.
4. Classify failures before editing code.
5. Pick one bucket and one representative failing glyph/input.
6. Trace C and Rust at pipeline boundaries.
7. Find the first divergence.
8. Fix the minimal Rust root cause.
9. Re-run the narrow lane, then the full matrix.
10. Run no-runtime-FFI, fmt, and clippy.

Useful commands:

```bash
cargo test -p pillow-rs-freetype --test coverage_matrix_tests -- --nocapture
cargo test -p pillow-rs-freetype --test no_runtime_ffi -- --nocapture
cargo fmt --all -- --check
cargo clippy -p pillow-rs-freetype --all-targets --all-features -- -D warnings
```

Use `pillow-rs-freetype/scripts/classify_failure_ids.py` for failure grouping.
If a new generator or classifier is needed, add it under
`pillow-rs-freetype/scripts/` with usage documentation.

## Current Parity Lanes

Track these lanes separately. Do not collapse them into one score.

- `force_autohint_matrix`
- `no_hinting_matrix`
- `render_mono_matrix`
- `render_lcd_matrix`
- `metrics_only_matrix`
- `outline_cbox_matrix`
- `native_tt_default_matrix`

Report before/after counts for every affected lane.

## Failure Buckets

Classify failures by the first differing public surface, then by likely
internal stage.

- `metrics`: advance, bearing, height, vertical metrics, rounding.
- `bbox/cbox`: outline geometry, phantom translation, grid-fitting, 26.6
  bounds.
- `bitmap placement`: left/top/width/height/stride without widespread pixel
  coverage differences.
- `pixel coverage`: bitmap dimensions and placement match, but coverage bytes
  differ.
- `loader/scaler`: unhinted outline points differ before hinting.
- `bytecode`: TrueType instruction state, CVT, storage, round state, DELTA,
  comparison opcodes, phantom points.
- `rasterizer`: hinted outline matches, but gray/mono/LCD bytes differ.

Fix the earliest internal divergence, not the downstream symptom.

## C To Rust Debugging

Use the `systematic-debugging` skill for the detailed method. The short rule:

```text
one input -> dump both sides -> compare stages -> find first divergence -> fix minimal code
```

Do not debug thousands of interleaved fixture rows with broad traces. Isolate
one font, glyph, size, load flags, and endpoint.

Suggested stage order:

- Raw glyph load and unhinted point coordinates.
- Scaled outline before hinting.
- TrueType prep/glyph instruction state.
- CVT/storage/graphics-state mutations.
- Phantom points and advances.
- Final hinted outline.
- Outline bbox/cbox.
- Rasterizer cells/spans/bytes.
- Public placement and metrics.

When C and Rust differ, read the exact C function and trace its branches. Do
not infer behavior from docs alone.

## Subagent Protocol

Subagents are for independent classified buckets. They must work in separate
worktrees and never in `/home/appunni/work/pil-wasm`.

Every subagent task must include:

- Worktree path and branch name.
- Baseline commit and baseline lane counts.
- One owned bucket only.
- Exact test command for the lane.
- No-FFI/no-fixture-edit/no-test-weakening constraints.
- Instruction to use C source and temporary traces for diagnosis.
- Requirement to remove temporary debug prints before commit.
- Requirement to commit only verified improvements.

Subagent final report must include:

- Worktree path and branch.
- Commit hash or "no commit" reason.
- Changed files.
- Before/after counts for all affected lanes.
- Exact verification commands run.
- Whether no-runtime-FFI, fmt, and clippy passed.
- Remaining failure bucket.

Only the orchestrating agent merges to main. Review the diff before merge for:

- FFI or native C shortcuts.
- Fixture/test/threshold weakening.
- Debug prints.
- Unrelated rewrites.
- Legacy `pillow-rs-font` work.

After merge, run the relevant lane, full harness, no-runtime-FFI, fmt, and
clippy. Push only from main after verification.

Archive or remove completed worktrees. Back up dirty diffs and untracked files
to `/tmp/...` before removing a non-clean worktree. Do not report archived
worktrees as active.

## Recent TrueType Lessons

- FreeType v40 backward compatibility can depend on state established by
  `prep`, even for glyphs with no glyph instructions.
- When `backward_compatibility` is active, preserve original horizontal
  phantom points for final outline translation and advance handling.
- CVT scaling during `prep` must match FreeType order: divide stored FWORD*64
  by 64 before applying size scale.
- SROUND/S45ROUND period, phase, and threshold handling affect native TrueType
  pixel parity.
- TrueType stack operand order must match FreeType exactly for comparison and
  DELTA-style opcodes.

## Commit Documentation

Commit messages for parity fixes should state:

- What C produced.
- What Rust produced before the fix.
- The C function/file area compared.
- The first divergence.
- The exact harness count improvement.

At non-obvious fix sites, add a short comment with the C reference and why the
Rust behavior now matches it.
