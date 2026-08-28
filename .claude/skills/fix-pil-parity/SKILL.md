---
name: fix-pil-parity
description: This skill should be used when the user asks to "fix more tests", "continue fixing xfailed", "fix the algorithm", "research and fix", "make this test pass", or when working through xfailed_tracker.txt entries. Encodes the proven research→implement→validate cycle for fixing pillow-rs PIL parity test failures.
version: 0.1.0
---

# Fix PIL Parity Test

Encode a proven cycle for fixing pillow-rs PIL parity cases: research PIL's
actual C/Python source code, implement the exact algorithm in Rust, validate
with one stable input comparison, then commit the change.

## When to Use

- The user asks to "fix more tests", "continue fixing xfailed", or "fix the algorithm"
- Any single PIL parity case needs investigation and a Rust-level fix
- A parity case produces hash/value mismatches that require algorithmic correction

## Core Pattern

Three phases per test: research → implement → validate. Never combine multiple tests in one fix cycle.

### Research Phase

To find PIL's actual algorithm:

1. Search for PIL source with WebSearch: `"PIL Pillow <function_name> C source code implementation"`
2. Fetch the actual C or Python source from:
   - `https://raw.githubusercontent.com/python-pillow/Pillow/main/src/libImaging/<file>.c`
   - `https://raw.githubusercontent.com/python-pillow/Pillow/main/src/PIL/<module>.py`
3. Get the EXACT runtime values with Python — documentation is often wrong:
   ```bash
   python3 -c "from PIL import X; print(X.Y.filterargs)" 2>&1
   ```
4. Compare with the current Rust implementation at `pillow-rs/src/`

### Implement Phase

To implement the fix:

1. Read the current Rust code in `pillow-rs/src/image.rs` or `pillow-rs/src/ops/*.rs`
2. Edit to match PIL's algorithm exactly — same math, same rounding, same border handling
3. Build: `make build`

### Validate Phase

To validate the fix:

1. Run only the single public case:
   ```bash
   make migration-parity-case MIGRATION_PARITY_CASE="<case_id>"
   ```
2. Compare the target and Pillow observations directly (see `references/debug-patterns.md`)
3. Regenerate inputs only through `make migration-parity-inputs` when the manifest changes
4. Commit: `git add -A && git commit -m "fix: <description>"`

## Proven Algorithm Patterns

Reference for algorithms already validated:

| Operation | PIL Source | Rust Fix | Key Detail |
|-----------|-----------|----------|------------|
| BoxBlur | `libImaging/BoxBlur.c` | Separable 2-pass, fixed-point 24-bit | `ww = 2^24/window`, `bias = 2^23` |
| Equalize | `ImageOps.py` (Python) | Step-based formula | `step = (sum_nonzero - last_bin)/255` |
| Filter3x3/5x5 | `libImaging/Filter.c` | Bottom-to-top kernel order | `offset+0.5` then `clip8` truncation |
| Blend modes | `ImageChops.c` | 256×256 LUTs from PIL runtime | Use `python3 -c` to build LUT |

## Quick Reference

| Task | Command |
|------|---------|
| Pick next case | Read the parity result or Coverage MCP output for a `case_id` |
| Research | WebSearch + WebFetch PIL source |
| Build | `make build` |
| Single case | `make migration-parity-case MIGRATION_PARITY_CASE="<case_id>"` |
| Debug diffs | See `references/debug-patterns.md` |
| Regen fixture | Use inline python3 script above |
| Full parity | `make migration-parity-test-all-backends` |

## Additional Resources

### Reference Files

- **`references/debug-patterns.md`** — Patterns for debugging hash/value mismatches, including how to compare RSPIL vs PIL output pixel-by-pixel
- **`references/proven-fixes.md`** — Archive of all fixes applied, with PIL source locations and Rust implementations
