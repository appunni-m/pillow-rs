# pillow-rs-freetype

Pure-Rust font rendering engine — a FreeType-compatible TrueType loader,
Latin auto-hinter, bytecode hinter, and smooth anti-aliased rasterizer.

**Runtime is 100% Rust: zero FreeType FFI, zero unsafe.**

Project goal: exact FreeType C pixel/byte parity produced by Rust code. C is an
oracle for fixtures only. Broad parity matrices must not be reduced to smoke
tests; incomplete threshold baselines are tracked as unfinished parity work.
See `PROJECT_GOALS.md`.

## Quick Start

```rust
use pillow_rs_freetype::{BitmapBackend, Font, FontError};

fn main() -> Result<(), FontError> {
    let font_data = std::fs::read("DejaVuSans.ttf")?;

    // Create a font at 12pt
    let font = Font::truetype(&font_data, 12.0, BitmapBackend::FreeType)?;

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
- **Two backends**: `FreeType` (raw), `PIL` (PIL-compatible padded masks)
- **No runtime FreeType C dependency**: Vendored C source and scripts are offline fixture references only

## Backends

| Backend | Mask behavior | Use case |
|---------|--------------|----------|
| `BitmapBackend::FreeType` | Raw raster output, no padding | Comparing against FreeType C output |
| `BitmapBackend::PIL` | Padded to ascender/descender extent, PIL-compatible | Drop-in PIL replacement |

## API

### `Font::truetype(data, size_pt, backend) -> Result<Font, FontError>`
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
# Run FreeType fixture tests
cargo test -p pillow-rs-freetype --test coverage_matrix_tests -- --nocapture

# Run all tests
cargo test -p pillow-rs-freetype

# Test with output
cargo test -p pillow-rs-freetype -- --nocapture
```

Test fixtures are FreeType-path JSON matrices generated from vendored FreeType C.
2.14.3 C reference output. See `doc/REFERENCES.md` for fixture regeneration.

## License

MIT — see `LICENSE`. The vendored FreeType C source under `freetype/` is
covered by the FreeType License (FTL).
