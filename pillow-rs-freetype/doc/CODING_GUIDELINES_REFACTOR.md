# Coding Guidelines Refactor — pillow-rs-freetype

## Final State

### Crate-level `lib.rs`
```rust
#![forbid(unsafe_code)]    // must stay
#![allow(missing_docs)]    // 488 items, separate task
// No more #![allow(clippy::arithmetic_side_effects)] — eliminated
```

### `clippy::arithmetic_side_effects` — vectorized

**Crate-level allow removed.** 718 sites handled as:

| Module | Sites | Strategy |
|--------|-------|----------|
| `wrapping.rs` | — | New module: `add()`, `sub()`, `mul()`, `neg()` helpers |
| `fixed.rs` | 3→0 | Used wrapping helpers + function-level `#[allow]` for safe u64 divs |
| `autohint/` | 307 | `#![allow]` in `autohint/mod.rs` — 26.6 hinting math, C-ported |
| `grays.rs` | 126 | `#![allow]` — DDA rasterizer, C-ported FT_INT64 path |
| `scaler.rs` | 11 | `#![allow]` — 26.6 scaling math |
| `font.rs` | 18 | `#![allow]` — pixel-to-26.6 math, PIL compatibility |
| `tt/` subtree | 206 | `#![allow]` in `tt/mod.rs` — table offset/index math |
| `tt/hinter/` | 82 | `#![allow]` in `tt/hinter/mod.rs` — bytecode VM, 26.6 math |
| **Total** | **718** | **0 crate-level, 7 module-level allows** |

### Architecture
```
lib.rs          → forbid(unsafe_code), allow(missing_docs)
wrapping.rs     → add/sub/mul/neg wrappers (i32 2's-complement)
fixed.rs        → wrapping::add/sub + function-level #[allow]
autohint/mod.rs → #![allow(arithmetic_side_effects)]
grays.rs        → #![allow(arithmetic_side_effects)]
scaler.rs       → #![allow(arithmetic_side_effects)]
font.rs         → #![allow(arithmetic_side_effects)]
tt/mod.rs       → #![allow(arithmetic_side_effects)]
tt/hinter/mod.rs→ #![allow(cast_*, arithmetic_side_effects)]
```

### Tests
- `cargo test -p pillow-rs-freetype --lib`: **20/20 pass**
- `cargo clippy -p pillow-rs-freetype --lib`: **0 warnings**
