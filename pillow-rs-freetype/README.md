# freetype

Pure-Rust font rendering engine — a FreeType-compatible TrueType loader,
Latin auto-hinter, bytecode hinter, and smooth anti-aliased rasterizer.

**Runtime is 100% Rust: zero FreeType FFI, zero unsafe.**

This crate is designed to stand on its own. It can be vendored into larger
workspaces, but build, test, parity, benchmark, fixture, and release workflows
are maintained from this directory.

Project goal: exact FreeType C pixel/byte parity produced by Rust code. C is an
oracle for fixtures only. Broad parity matrices must not be reduced to smoke
tests; incomplete threshold baselines are tracked as unfinished parity work.
See `PROJECT_GOALS.md`.

## Install

```bash
cargo add freetype
```

From source:

```bash
git clone https://github.com/appunni-m/freetype
cd freetype
make test
```

The crate uses the Rust 2024 edition. Minimum supported Rust version: 1.87.
The checked-in toolchain file pins Rust 1.96.1 for local development; CI also
runs a 1.87 MSRV test lane so the public MSRV contract remains enforced.

## Quick Start

```rust
use freetype::{Font, FontError};

fn main() -> Result<(), FontError> {
    let font_data = std::fs::read("DejaVuSans.ttf")?;

    // Create a font at 12pt
    let font = Font::truetype(&font_data, 12.0)?;

    // Get family and style name
    let (family, style) = font.getname();
    println!("Font: {} {}", family, style);

    // Get the 8-bit alpha mask for 'A'
    let mask = font.getmask("A")?;
    println!("Mask: {}×{} pixels, advance: {}", mask.width, mask.height, mask.advance_width);

    // Get bounding box (pixel coordinates)
    let bbox = font.getbbox("A");
    println!("BBox: ({}, {}) → ({}, {})", bbox.0, bbox.1, bbox.2, bbox.3);

    Ok(())
}
```

## Features

- **TrueType outline loading**: Parses `glyf`/`loca` tables, composite glyphs
- **Auto-hinting**: Latin script auto-hinter (FreeType's `af_latin_*` engine)
  — grid-fits edges to pixel boundaries for crisp small-size text
- **Smooth rasterizer**: FT_INT64 DDA path from `ftgrays.c` — 8-bit alpha output
- **Table parsing**: `cmap`, `head`, `hhea`, `hmtx`, `maxp`, `name`, `OS/2`
- **No runtime FreeType C dependency**: Vendored C source and scripts are offline fixture references only

## Current Parity Gates

The default parity runner executes every committed fixture family. Current
exact gates include:

| Gate | Rows |
|------|------|
| `force_autohint_matrix.json` | 22,168 |
| `native_tt_default_matrix.json` | 7,640 |
| `no_hinting_matrix.json` | 11,086 |
| `render_mono_matrix.json` | 11,086 |
| `render_lcd_matrix.json` | 11,086 |
| `metrics_only_matrix.json` | 11,086 |
| `outline_cbox_matrix.json` | 11,086 |

Run:

```bash
make test-parity
```

The no-runtime-FFI guard is mandatory:

```bash
make test-ffi
```

## API

### `Font::truetype(data, size_pt) -> Result<Font, FontError>`
Load a TrueType font from memory. Computes auto-hinter metrics (stem widths,
blue zones) at font creation time.

### `font.getmask(text) -> Result<GlyphMask, FontError>`
Render a glyph to an 8-bit alpha bitmap. Returns width, height, pixel data,
and advance width.

### `font.getbbox(text) -> (i32, i32, i32, i32)`
Bounding box in pixel coordinates (x_min, y_min, x_max, y_max).

### `font.getname() -> (&str, &str)`
Returns `(family_name, style_name)` from the `name` table.

### `font.getlength(text) -> f32`
Advance width of `text` in pixels.

### `font.getmetrics() -> (u32, u32)`
Returns `(ascender, descender)` in pixels.

## Architecture

```
┌──────────┐    ┌───────────┐    ┌────────┐    ┌───────────┐
│ tt/      │    │ scaler.rs │    │ autohint/│   │ grays.rs  │
│ glyf.rs  │───→│           │───→│ latin.rs │───→│           │
│ cmap.rs  │    │ pp1x shift│    │ loader.rs│    │ DDA raster│
│ hmtx.rs  │    │ FU→26.6   │    │ types.rs │    │ sweep     │
│ ...      │    │ bbox      │    │          │    │           │
└──────────┘    └───────────┘    └────────┘    └───────────┘
     ↑                                          │
     │ font.rs: Font::truetype()                ▼
     │ getmask/getbbox/getname/getlength    8-bit alpha bitmap
```

Key modules:

| Module | Purpose |
|--------|---------|
| `tt/` | TrueType table parsers (glyf, cmap, hmtx, head, hhea, maxp, name, OS/2) |
| `scaler` | Glyph scaling (FU → 26.6), pp1.x origin shift, cbox computation |
| `autohint` | Latin auto-hinter: reload, segments, edges, 4-phase snapping, IUP |
| `grays` | Smooth anti-aliased rasterizer (FT_INT64 DDA) |
| `font` | High-level API: `Font::truetype`, `getmask`, `getbbox` |

## Testing

```bash
make fmt
make test
make clippy
```

Or use the standalone Makefile:

```bash
make ci
```

Harness intent:

- `no_runtime_ffi.rs` keeps runtime FreeType C impossible.
- `generator_contract.rs` keeps fixture generation documented and reproducible.
- `harness_contract.rs` locks fixture breadth and gate strength.
- `coverage_matrix_tests.rs` runs exact and incomplete FreeType matrix gates.
- `render_mode_matrix.rs` compares raw render-mode bytes and metadata.
- `fixed_parity.rs` runs mandatory scalar C-oracle parity.
- `interface_coverage.rs` keeps FreeType endpoint status truthful.

Test fixtures are FreeType-path JSON matrices generated from vendored FreeType C
2.14.3 reference output. See `PROJECT_GOALS.md` and
`doc/GENERATOR_SYSTEM.md` before changing fixtures, generators, or gates.

## Benchmarking

```bash
make bench
```

Reports are written to `target/freetype-bench/latest.json` and
`target/freetype-bench/latest.md`. See `doc/PERFORMANCE_BENCHMARKING.md` for
trust labels, timing boundaries, machine metadata, and review rules.

## Contributing

Start with `CONTRIBUTING.md` and `PROJECT_GOALS.md`. The short version:

- keep runtime pure Rust
- keep C FreeType as oracle tooling only
- run the exact parity lane before claiming correctness
- do not weaken fixtures, thresholds, or tests
- document benchmark and fixture changes as project infrastructure

## License

FreeType License (`FTL`) — see `LICENSE` and `FTL.TXT`.

The vendored FreeType C source under `freetype/` is retained as an offline
oracle for fixture generation and diagnosis. Runtime code is pure Rust and does
not link to FreeType C.
