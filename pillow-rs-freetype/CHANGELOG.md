# Changelog

## 0.1.0 (unreleased)

### Added
- Initial crate: pure-Rust FreeType 2.14.1 port
- TrueType table parsers: cmap, glyf, head, hhea, hmtx, maxp, name, OS/2, loca
- Latin script auto-hinter: reload, segment detection, edge grouping, 4-phase hinting, IUP
- Smooth anti-aliased rasterizer (FT_INT64 DDA path from ftgrays.c)
- `BitmapBackend::FreeType` and `BitmapBackend::PIL` backends
- Harness-first parity tracking: exact gates, threshold baselines, incomplete executed baselines, and unexecuted fixture debt are reported separately.
- Runtime FFI guard: `tests/no_runtime_ffi.rs` prevents reintroducing FreeType C linking in crate runtime files.
- Fixture generator contract: `doc/GENERATOR_SYSTEM.md` and `tests/generator_contract.rs` make C-oracle fixture reproduction part of the maintained harness.
- Unified runner now executes `render_mono_matrix.json`, `render_lcd_matrix.json`, `metrics_only_matrix.json`, `no_hinting_matrix.json`, and `outline_cbox_matrix.json`; current failures are exposed as implementation gaps instead of hidden fixture debt.

### Fixed
- Removed the runtime native FreeType bridge (`build.rs`, `src/native_ft.rs`, `src/native_ft.c`); `BitmapBackend::PIL` now routes through the Rust scaler, TrueType hinting, and rasterizer path.
- `1ecd364`: WEAK_INTERPOLATION classification — 18→9 failures. The "both-None" case
  XOR check and `corner_is_flat` must run sequentially (not OR'd) because
  `corner_is_flat` updates direction-chain deltas that downstream classifications
  depend on. Spike detection must be unconditional (not gated on `AF_FLAG_NEAR`).
- `04975f8`: pp1.x phantom-point translation — 309→18 failures. C's TT_Load_Glyph
  shifts contour X coords by `pp1.x = glyf_header.xMin - hmtx_lsb`. Must use glyf
  HEADER xMin (not computed min) and shift both raw outline AND scaled coords.
- `cbbdcba`: getmetrics `f32` precision → `FT_MulFix + FT_PIX_CEIL`
- `cf19f9e`: getlength from Python hmtx instead of C `FT_LOAD_DEFAULT`
- `887070a`: walk_contour conic wrap when `first==0`
