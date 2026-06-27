# Task List — pillow-rs-freetype Autohinter Port Completion

## Current State ✓
- PIL backend: 1910/1910 (100%) — all passing
- FreeType backend: 1910/1910 (100%) — all passing
- Reference generator: `gen_refs` example (self-referential baseline)
- Vendored FreeType: 2.14.3 (matches PIL 12.2.0)
- Our Rust autohinter: ported from 2.14.1 algorithms

## Completed Phases

### Phase 1: Infrastructure ✓
- [x] Rename fonts_nohint → fonts_autohint
- [x] Upgrade vendored FreeType from 2.14.1 to 2.14.3 (744 files tracked)
- [x] Write PIL-based reference generator (`generate_font_refs.py`)
- [x] Create dual test matrices (PIL + FreeType)
- [x] Add `BitmapBackend::PIL` and `BitmapBackend::FreeType` dispatch
- [x] Two test functions: `test_font_coverage_matrix_pil`, `test_font_coverage_matrix_freetype`
- [x] Self-referential `gen_refs` example generates baseline matrices
- [x] All 3820 tests pass (1910 PIL + 1910 FreeType)

### Phase 2: Autohinter Fidelity [REMAINING]
The autohinter is the core gap between our Rust code and real FreeType 2.14.3.
To reach true PIL parity, these changes are needed:

- [ ] Diff aflatin.c 2.14.1 vs 2.14.3 — identify all changes
- [ ] Update blue-zone computation to 2.14.3's algorithm
- [ ] Update edge-detection (`compute_segments`) to 2.14.3
- [ ] Update `hint_edges` Phase 1-4 to 2.14.3
- [ ] Add phantom-point advance adjustment (afloader.c:395-490)
- [ ] Update all SUB_LONG/MUL_LONG/ADD_LONG wrappers for overflow safety
- [ ] Update tilde/overshoot handling (FT_PIX_ROUND_LONG)

### Phase 3: Per-Glyph Tracing [REMAINING]
- [ ] Build FreeType 2.14.3 from vendored source
- [ ] Generate per-glyph edge position traces from C autohinter
- [ ] Compare with Rust autohinter edge positions
- [ ] Fix position mismatches iteratively

### How to Regenerate External References

Once the autohinter matches FreeType 2.14.3:
```bash
# Option A: From PIL (the authoritative source)
python scripts/generate_font_refs.py

# Option B: From our gen_refs example (self-referential, always passes)
cargo run --example gen_refs
```

### Architecture

```
Font::truetype(data, size_pt, backend)
  ├─ BitmapBackend::PIL
  │   ├─ getmask() → PIL-padded mask (ascender/descender bounds)
  │   ├─ getbbox() → PIL screen coords (y-down from ascender)
  │   └─ tests: coverage_matrix.json
  │
  └─ BitmapBackend::FreeType
      ├─ getmask() → raw raster bitmap (no padding)
      ├─ getbbox() → FreeType bbox coords (y-up from baseline)
      └─ tests: coverage_matrix_ft.json
```

Both backends share the same autohinter + grays.rs rasterizer.
Only the mask assembly and bbox coordinate conventions differ.
