# FreeType C API/ABI Dispatch

Baseline: `d76593a2`

This dispatch splits the FreeType C compatibility program into independent
worktree slices. The target is FreeType C headers and behavior, not Servo
`rust-freetype`. Servo is useful only as a reference for how one binding shaped
its API.

## Constraints

- Workers must use their assigned worktree under
  `/home/appunni/work/fontdone-agents/`.
- Workers must not edit `/home/appunni/work/pil-wasm`.
- Workers must not push.
- Runtime code must stay pure Rust: no `freetype-sys`, `bindgen`, `cc`,
  `extern "C"`, `dlopen`, or native FreeType calls in `fontdone`.
- C FreeType is an oracle for headers, fixture generation, and diagnosis only.
- Tests, fixtures, thresholds, and expected outputs must not be weakened.
- Each slice owns one documentation file under
  `pillow-rs-freetype/doc/api-abi-slices/`.

## Completed Slices

| Slice | Worktree | Branch | Output |
| --- | --- | --- | --- |
| Advances and kerning | `/home/appunni/work/fontdone-agents/capi-advances-kerning` | `agent/capi-advances-kerning` | `advances_kerning.md` |
| Cache/modules/exclusions | `/home/appunni/work/fontdone-agents/capi-cache-modules-exclusions` | `agent/capi-cache-modules-exclusions` | `cache_modules_exclusions.md` |
| Charmap and SFNT APIs | `/home/appunni/work/fontdone-agents/capi-charmap-sfnt` | `agent/capi-charmap-sfnt` | `charmap_sfnt.md` |
| Dynamic harness | `/home/appunni/work/fontdone-agents/capi-dynamic-harness` | `agent/capi-dynamic-harness` | `dynamic_harness.md` |
| Face and size APIs | `/home/appunni/work/fontdone-agents/capi-face-size` | `agent/capi-face-size` | `face_size.md` |
| Glyph load/render APIs | `/home/appunni/work/fontdone-agents/capi-glyph-load-render` | `agent/capi-glyph-load-render` | `glyph_load_render.md` |
| Library lifecycle | `/home/appunni/work/fontdone-agents/capi-lifecycle` | `agent/capi-lifecycle` | `library_lifecycle.md` |
| Variations | `/home/appunni/work/fontdone-agents/capi-mm-variations` | `agent/capi-mm-variations` | `mm_variations.md` |
| Outline and bitmap APIs | `/home/appunni/work/fontdone-agents/capi-outline-bitmap` | `agent/capi-outline-bitmap` | `outline_bitmap.md` |
| Records and constants | `/home/appunni/work/fontdone-agents/capi-records-constants` | `agent/capi-records-constants` | `records_constants.md` |

All slices have been reviewed for write scope and integrated into `main`.

## Integration Rule

The orchestrator reviews each branch before merging. Review must check for
runtime FFI shortcuts, fixture/test weakening, unrelated rewrites, and stale
debug output. After merging documentation slices, run:

```bash
make -C pillow-rs-freetype api-abi-audit
cargo test -p fontdone --test no_runtime_ffi --locked -- --nocapture
cargo fmt --all -- --check
```
