# Streaming Pipeline Architecture — Design Spec

> 2026-06-11 | pillow-rs | 166 functions | PIL-compatible

## Problem

Every image operation clones the entire pixel buffer:

```rust
pub fn resize(&self, ...) -> Result<Image> {
    let mut clone = self.clone();   // copies 9MB for 2k image
    let img = clone.ensure_loaded()?;
    let resized = img.resize_exact(w, h, filter);
    Ok(Image { inner: LazyImage::Loaded(resized), format })
}
```

A 20-op pipeline does 20 × 9MB = 180MB of unnecessary copying. This prevents SIMD from working at scale because data constantly round-trips through memory.

## Solution

All operations are **lazily recorded** into a pipeline. Only `save()`, `tobytes()`, or `materialize()` execute the chain. Execution is a **single fused pass** that allocates one output buffer and streams pixels through all operations.

## Data Model

```rust
pub enum Image {
    Loaded(DynamicImage),                                        // decoded, ready
    Path { path: PathBuf, format: Option<ImageFormat> },         // not decoded yet
    Bytes { data: Arc<Vec<u8>>, format: Option<ImageFormat> },   // not decoded yet
    Pipeline { source: Arc<Image>, ops: Vec<PipelineOp> },       // lazy chain
}
```

### PipelineOp Enum (96 variants — covers all image-producing operations)

Every operation that returns a new Image or mutates in-place becomes a PipelineOp:

| Category | Variants | Count |
|----------|----------|-------|
| Geometry | Resize, Crop, Rotate, Transpose, Transform, Reduce, Thumbnail | 7 |
| Color | Convert, Quantize, Point, RemapPalette | 4 |
| Filter | Filter3x3, GaussianBlur, BoxBlur, MedianFilter, MaxFilter, MinFilter, RankFilter | 7 |
| ImageOps | Autocontrast, Equalize, Invert, Flip, Mirror, Posterize, Solarize, Grayscale, Colorize, Contain, Cover, Fit, Pad, Scale, Expand, CropBorder | 16 |
| ImageChops | Add, Subtract, Multiply, Screen, Darker, Lighter, Difference, Overlay, HardLight, SoftLight, AddModulo, SubtractModulo, LogicalAnd, LogicalOr, LogicalXor, Constant, Offset, Blend, Composite, Duplicate, InvertChops | 21 |
| Enhance | Brightness, Contrast, ColorSaturation, Sharpness | 4 |
| ModuleFns | Merge, BlendModule, CompositeModule, Eval, EffectNoise | 5 |
| Effect | EffectSpread, Paste, AlphaComposite | 3 |
| **Pipeline-capable** | | **96** |

### Complete Function List (all 166 from manifest.yaml)

#### Pipeline Operations (96)

**Image (22):** resize, crop, rotate, transpose, convert, filter, quantize, reduce, effect_spread, point, transform, thumbnail, remap_palette, tobitmap, draft, alpha_composite, paste, pasteColor, frombytes, apply_transparency, tobytes, save

**ImageOps (16):** autocontrast, equalize, invert, flip, mirror, posterize, solarize, grayscale, colorize, contain, cover, fit, pad, scale, expand, crop

**ImageChops (21):** add, subtract, multiply, screen, darker, lighter, difference, overlay, hard_light, soft_light, add_modulo, subtract_modulo, logical_and, logical_or, logical_xor, constant, offset, blend, composite, duplicate, invert

**ImageEnhance (4):** Brightness, Color, Contrast, Sharpness

**ImageFilter (20):** BLUR, CONTOUR, DETAIL, EDGE_ENHANCE, EDGE_ENHANCE_MORE, EMBOSS, FIND_EDGES, SHARPEN, SMOOTH, SMOOTH_MORE, GaussianBlur, BoxBlur, UnsharpMask, MaxFilter, MinFilter, MedianFilter, ModeFilter, RankFilter, Kernel, Color3DLUT

**ImageModule (8):** merge, blend, composite, eval, effect_noise, fromarray, frombytes, new

**ImageDraw (1):** point

**ImageEffect (1):** effect_spread

**Mutating pipeline (3):** paste, thumbnail, alpha_composite

#### Non-Pipeline Operations (70 — execute immediately)

**Image analysis (17):** getpixel, getbands, getbbox, getcolors, getdata, getextrema, getprojection, histogram, entropy, getchannel, getexif, getim, getpalette, getxmp, get_child_images, get_flattened_data, load

**Image IO/mutation (14):** open, new, copy, split, putalpha, putpixel, putdata, putpalette, show, close, seek, tell, verify, draft

**ImageColor (2):** getcolor, getrgb

**ImageDraw (17):** arc, bitmap, chord, circle, ellipse, getfont, line, multiline_text, multiline_textbbox, pieslice, polygon, rectangle, regular_polygon, rounded_rectangle, text, textbbox, textlength

**ImageFont (15):** FreeTypeFont, ImageFont, load, load_default, load_default_imagefont, load_path, truetype, getbbox(FreeTypeFont), getlength(FreeTypeFont), getmask(FreeTypeFont), getmetrics, getname, getbbox(ImageFont), getlength(ImageFont), getmask(ImageFont)

**ImagePalette (5):** copy, getcolor, getdata, save, tobytes

**ImageSequence (1):** Iterator

**ImageStat (1):** Stat

- **Getters**: getpixel, getbands, getbbox, getcolors, getdata, getextrema, getprojection, histogram, entropy, getchannel, getexif, getim, getpalette, getxmp, get_child_images, get_flattened_data, load, getrgb, getcolor
- **IO**: new, open, frombytes, fromarray, save, tobytes, tobitmap, seek, tell, verify, show, close, copy, split
- **Mutation**: putalpha, putpixel, putdata, putpalette, alpha_composite (in-place variant)
- **Draw** (18 ops): arc, line, rectangle, ellipse, polygon, text, multiline_text, circle, rounded_rectangle, regular_polygon, chord, pieslice, bitmap, point, textbbox, multiline_textbbox, textlength, getfont
- **Font** (15 ops): truetype, load, load_default, load_default_imagefont, load_path, getbbox×2, getlength×2, getmask×2, getmetrics, getname, FreeTypeFont, ImageFont
- **Palette** (5 ops): copy, getcolor, getdata, save, tobytes
- **Stat** (1: Stat), **Sequence** (1: Iterator)

## API

```rust
impl Image {
    // Pipeline ops — return Image lazily (96 ops)
    pub fn resize(&self, w: u32, h: u32, filter: Option<&str>) -> Result<Image> {
        Ok(Image::Pipeline {
            source: Arc::new(self.clone()),
            ops: vec![PipelineOp::Resize { w, h, filter: parse_resample(filter)? }],
        })
    }
    pub fn crop(&self, left: u32, top: u32, right: u32, bottom: u32) -> Result<Image> { ... }
    pub fn convert(&self, mode: &str, ...) -> Result<Image> { ... }
    // ... 93 more — all same pattern

    // Mutating ops — update self to Pipeline (thumbnail, paste, etc.)
    pub fn thumbnail(&mut self, size: (u32, u32), filter: Option<&str>) -> Result<()> {
        let (tw, th) = self.compute_thumbnail_size(size)?;
        *self = Image::Pipeline {
            source: Arc::new(self.clone()),
            ops: vec![PipelineOp::Thumbnail { size: (tw, th), filter }],
        };
        Ok(())  // Returns None to Python — PIL compatible
    }
    pub fn paste(&mut self, im: &Image, box: (i32,i32,i32,i32), mask: Option<&Image>) -> Result<()> { ... }

    // Immediate ops — force materialize first
    pub fn getpixel(&self, x: u32, y: u32) -> Result<(u8,u8,u8,u8)> {
        self.materialize()?.get_pixel(x, y)
    }
    pub fn save(&self, path: &str, format: Option<&str>) -> Result<()> { ... }
    pub fn tobytes(&self) -> Result<Vec<u8>> { ... }

    // Materialization
    pub fn materialize(&self) -> Result<DynamicImage>;
    pub fn execute_gpu(&self, engine: &GpuEngine) -> Result<DynamicImage>;
}
```

## PIL Compliance

| PIL behavior | Implementation | Status |
|---|---|---|
| resize() returns new Image | Returns Image::Pipeline | ✅ |
| thumbnail() mutates in-place, returns None | PipelineOp mutates self | ✅ |
| paste() mutates in-place, returns None | PipelineOp mutates self | ✅ |
| copy() returns independent clone | Materializes + clones pixel buffer | ✅ |
| All getters return scalars/vectors | Materialize first, then read | ✅ |
| Pipeline never visible from Python | materialize() before crossing PyO3 boundary | ✅ |
| Zero API differences from PIL | All 166 functions match PIL signatures | ✅ |

## Execution Engine

### CPU execution (fused pass)

```
materialize():
  1. resolve: if Path/Bytes → decode to DynamicImage
  2. walk ops to determine final dimensions and pixel format
  3. allocate output buffer (single allocation)
  4. for each pixel position in output:
       walk ops in reverse to find source pixels
       apply transforms in order
       write final pixel to output buffer
  5. return DynamicImage from output buffer
```

### GPU execution (compute shader chain)

```
execute_gpu(engine):
  1. resolve source
  2. upload to GPU texture
  3. for each PipelineOp:
       select matching WGSL shader
       bind input texture, output texture
       dispatch compute workgroups
       swap input ← output
  4. download final texture
  5. return DynamicImage
```

## File Structure

```
pillow-rs-core/
├── src/
│   ├── image.rs          ← Image enum + all op delegations (thin)
│   ├── pipeline.rs       ← PipelineOp enum + materialize() + execute_gpu()
│   ├── error.rs          ← UNCHANGED (PilError)
│   ├── color.rs          ← KEEP (LUT helpers, SIMD)
│   ├── format.rs         ← KEEP (format parsing)
│   ├── ops/              ← REFACTOR: each op file adds a PipelineOp variant
│   │   ├── resize.rs     ← Resize variant + parse_resample()
│   │   ├── crop.rs       ← Crop variant
│   │   ├── ...etc        ← same pattern for all 96 pipeline ops
│   └── gpu/
│       ├── mod.rs         ← KEEP (GpuEngine + execute_gpu impl)
│       └── shaders/       ← KEEP (existing WGSL)
├── tests/                 ← ALL existing PIL parity tests MUST pass
└── benches/               ← criterion benchmarks

pillow-rs-core-legacy/     ← EXISTING CODE COPIED (not deleted)
```

## What Gets Replaced

- `lazy.rs` — Image enum handles lazy loading directly
- All `let mut clone = self.clone();` patterns in ops
- `pub(crate) inner: LazyImage` — replaced by `Image` enum variants

## What Stays

- `color.rs` — SIMD LUT helpers
- `error.rs` — PilError types
- `format.rs` — format parsing
- `gpu/` — shaders and GpuEngine
- All 166 PIL parity tests
- All benchmark scripts and harnesses
- PyO3 and wasm-bindgen binding layers (thin)

## Implementation Sequence

1. Copy existing `pillow-rs-core/src/` → `pillow-rs-core-legacy/`
2. Define `PipelineOp` enum with all 96 variants
3. Define `Image` enum with `Pipeline` variant
4. Implement `materialize()` fused execution for CPU
5. Rewrite 96 pipeline ops to return `Image::Pipeline`
6. Rewrite 70 immediate ops to call `materialize()` first
7. Fix bindings (PyO3, wasm-bindgen) for new Image type
8. Run all tests — must pass 166/166
9. Implement `execute_gpu()` with WGSL dispatches
10. Run benchmarks — should show significant improvement
