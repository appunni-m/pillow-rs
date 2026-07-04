# Contributing

## Development

```bash
# Build
cargo build -p pillow-rs-freetype

# Run tests
cargo test -p pillow-rs-freetype

# Full linting
cargo clippy -p pillow-rs-freetype --all-targets -- -D warnings
cargo fmt -p pillow-rs-freetype -- --check
```

## Architecture Rules

- **Core never touches file paths or network.** All I/O lives in `font.rs` at the
  public API boundary. Internal modules take `&[u8]` slices.
- **No unsafe code.** `#![forbid(unsafe_code)]` is enforced at the crate root.
- **All public functions must have doc comments.** Use `#![warn(missing_docs)]`.
  Doc comments follow this pattern:
  ```
  /// One-line summary.
  ///
  /// Key rules/constants (table if helpful).
  ///
  /// # Debug: <symptom>
  /// - [ ] Checkpoint 1
  /// - [ ] Checkpoint 2
  ```
- **Commit messages must describe what changed and why.** Never use "wip" or "fix".
  Include the test pass rate change when fixing parity bugs.

## Parity Debugging

When a glyph renders differently from C FreeType:

1. **Use a standalone binary**, not the test suite. Load exactly ONE font, ONE glyph, ONE size.
2. **Dump every pipeline stage**: reload coords → edges → hint_edges phases → align → IUP → final.
3. **Find the first function where output diverges.** Everything downstream is a consequence.
4. **Compare against C's fprintf traces.** Add matching `fprintf(stderr, ...)` to the
   vendored C source, rebuild with `bash scripts/build_ft.sh`, and run side-by-side.
5. **Fix the root cause, not the symptom.** Never clamp outputs or add per-glyph special cases.

See `src/autohint/loader.rs` for an example of inline debug documentation.

## Adding a New Feature

1. Write a FreeType-path parity test in `tests/`
2. Generate a JSON fixture using `scripts/build_ft_fixture.py`
3. Implement the feature in `src/`
4. Run `cargo test -p pillow-rs-freetype` and verify pass

## Releasing

1. Update version in workspace `Cargo.toml`
2. Update `CHANGELOG.md`
3. `cargo publish -p pillow-rs-freetype`
