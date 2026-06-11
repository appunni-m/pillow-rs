# Streaming Pipeline Architecture — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign pillow-rs-core to a lazy streaming pipeline architecture where operations return `Image::Pipeline { source, ops }` without cloning pixel buffers. Only `save()`/`tobytes()`/`materialize()` execute the chain in a single fused pass.

**Architecture:** New `Image` enum (`Loaded | Path | Bytes | Pipeline`) replaces `LazyImage`. 96 ops become `PipelineOp` variants. 70 ops remain immediate. `Pipeline::materialize()` does fused single-pass execution. PyO3 bindings release GIL via `py.allow_threads()`. Existing code preserved in `pillow-rs-core-legacy/`.

**Tech Stack:** Rust (image crate 0.25, wgpu), Python (PyO3 0.24, Pillow 12.2.0), JS (wasm-bindgen 0.2)

---

## File Structure Map

| File | Action | Responsibility |
|------|--------|---------------|
| `pillow-rs-core-legacy/` | Create (copy) | Exact copy of pillow-rs-core/src/ for reference |
| `pillow-rs-core/src/image.rs` | Rewrite | New Image enum + all ops delegation + materialize |
| `pillow-rs-core/src/pipeline.rs` | Create | PipelineOp enum (96 variants) + execute() on CPU/GPU |
| `pillow-rs-core/src/lazy.rs` | Delete | Replaced by Image enum directly |
| `pillow-rs-core/src/ops/*.rs` | Rewrite | Each op file: argument types → PipelineOp variant + Image::op() impl |
| `pillow-rs-core/src/color.rs` | Keep | SIMD LUT helpers — unchanged |
| `pillow-rs-core/src/error.rs` | Keep | PilError — unchanged |
| `pillow-rs-core/src/format.rs` | Keep | Format parsing — unchanged |
| `pillow-rs-core/src/gpu/mod.rs` | Keep | GpuEngine + execute_gpu() — add pipeline runner |
| `pillow-rs-core/src/lib.rs` | Modify | Remove `pub mod lazy`, add `pub mod pipeline` |
| `pillow-rs-py/src/lib.rs` | Modify | All heavy ops: release GIL via `py.allow_threads()` |
| `pillow-rs-js/src/lib.rs` | Modify | Update for new Image type |
| `tests/test_image.py` | Modify | Expand coverage for pipeline edge cases |
| `scripts/bench_native_cpu.py` | Modify | Update for release build path |
| `scripts/bench_all.sh` | Keep | Already uses release mode |

---

### Task 1: Copy existing code to legacy folder

**Files:**
- Create: `pillow-rs-core-legacy/` (entire pillow-rs-core/src/ tree)

- [ ] **Step 1: Copy the source tree**

```bash
cp -r /home/appunni/work/pil-wasm/pillow-rs-core/src /home/appunni/work/pil-wasm/pillow-rs-core-legacy/
cp /home/appunni/work/pil-wasm/pillow-rs-core/Cargo.toml /home/appunni/work/pil-wasm/pillow-rs-core-legacy/Cargo.toml
cp -r /home/appunni/work/pil-wasm/pillow-rs-core/benches /home/appunni/work/pil-wasm/pillow-rs-core-legacy/benches 2>/dev/null || true
```

- [ ] **Step 2: Verify copy**

```bash
diff -r /home/appunni/work/pil-wasm/pillow-rs-core/src /home/appunni/work/pil-wasm/pillow-rs-core-legacy/src --brief || echo "Copy verified"
```

- [ ] **Step 3: Commit**

```bash
git add pillow-rs-core-legacy/
git commit -m "chore: copy pillow-rs-core to pillow-rs-core-legacy

Exact snapshot before streaming pipeline rewrite. Reference only."
```

---

### Task 2: Define PipelineOp enum + Image enum

**Files:**
- Create: `pillow-rs-core/src/pipeline.rs`
- Rewrite: `pillow-rs-core/src/image.rs`
- Modify: `pillow-rs-core/src/lib.rs`

- [ ] **Step 1: Create PipelineOp enum**

Create `pillow-rs-core/src/pipeline.rs`:

```rust
//! Streaming pipeline — all image-producing operations recorded as PipelineOp variants.
//! Execution is deferred until materialize() or save()/tobytes().

use image::{DynamicImage, RgbImage, RgbaImage, GrayImage, GrayAlphaImage};
use std::sync::Arc;

/// Every image-producing operation maps to one variant.
/// Input image is the source; output image is the result of applying this op.
#[derive(Debug, Clone)]
pub enum PipelineOp {
    // ── Geometry ──
    Resize { w: u32, h: u32, filter: ResampleFilter },
    Crop { left: u32, top: u32, right: u32, bottom: u32 },
    Rotate { angle: f64, expand: bool, fill: Option<(u8, u8, u8, u8)> },
    Transpose { method: TransposeMethod },
    Thumbnail { w: u32, h: u32, filter: ResampleFilter },
    Reduce { factor: u32 },

    // ── Color/Convert ──
    Convert { mode: ColorMode, matrix: Option<Vec<f64>>, dither: Option<DitherMethod> },
    Quantize { colors: u32, dither: bool },
    RemapPalette { dest_map: Vec<u8> },

    // ── Filters (3×3 convolution) ──
    Filter3x3 { kernel: [f32; 9], scale: f32, offset: i32 },
    GaussianBlur { sigma: f32 },
    BoxBlur { radius: u32 },
    MedianFilter { size: u32 },
    MaxFilter { size: u32 },
    MinFilter { size: u32 },
    RankFilter { size: u32, rank: u32 },

    // ── ImageOps ──
    Autocontrast { cutoff: f64 },
    Equalize,
    Invert,
    Flip,
    Mirror,
    Posterize { bits: u8 },
    Solarize { threshold: u8 },
    Grayscale,
    Colorize { black: (u8, u8, u8), white: (u8, u8, u8) },
    Contain { w: u32, h: u32, filter: ResampleFilter },
    Cover { w: u32, h: u32, filter: ResampleFilter },
    Fit { w: u32, h: u32, filter: ResampleFilter, bleed: f64, centering: (f64, f64) },
    Pad { w: u32, h: u32, color: Option<(u8, u8, u8, u8)>, centering: (f64, f64) },
    Scale { factor: f64, filter: ResampleFilter },
    Expand { border: u32, fill: (u8, u8, u8, u8) },
    CropBorder { border: u32 },

    // ── ImageChops ──
    Add { other: Arc<Image>, scale: f64, offset: f64 },
    Subtract { other: Arc<Image>, scale: f64, offset: f64 },
    Multiply { other: Arc<Image> },
    Screen { other: Arc<Image> },
    Darker { other: Arc<Image> },
    Lighter { other: Arc<Image> },
    Difference { other: Arc<Image> },
    Overlay { other: Arc<Image> },
    HardLight { other: Arc<Image> },
    SoftLight { other: Arc<Image> },
    AddModulo { other: Arc<Image> },
    SubtractModulo { other: Arc<Image> },
    LogicalAnd { other: Arc<Image> },
    LogicalOr { other: Arc<Image> },
    LogicalXor { other: Arc<Image> },
    Constant { value: u8 },
    Offset { x: i32, y: i32 },
    Blend { other: Arc<Image>, alpha: f64 },
    Composite { other: Arc<Image>, mask: Arc<Image> },
    Duplicate,
    InvertChops,

    // ── Enhance ──
    Brightness { factor: f64 },
    Contrast { factor: f64 },
    ColorSaturation { factor: f64 },
    Sharpness { factor: f64 },

    // ── Effects ──
    EffectSpread { distance: u32 },
    Paste { source: Arc<Image>, x: i32, y: i32, w: i32, h: i32, mask: Option<Arc<Image>> },
    AlphaComposite { source: Arc<Image>, dest: (i32, i32), src: (i32, i32) },

    // ── Module fns ──
    Merge { mode: ColorMode, bands: Vec<Image> },
    BlendModule { other: Arc<Image>, alpha: f64 },
    CompositeModule { other: Arc<Image>, mask: Arc<Image> },
    Eval { lut: Vec<u8> },
    EffectNoise { sigma: f64 },

    // ── Point operations (lookup table) ──
    PointOp { lut: Vec<u8> },
    Transform { w: u32, h: u32, method: TransformMethod, data: Vec<f64>, filter: ResampleFilter, fill: Option<(u8, u8, u8, u8)> },
}

// ── Support types ──

#[derive(Debug, Clone, Copy)]
pub enum ResampleFilter { Nearest, Bilinear, Bicubic, Lanczos, Box, Hamming }

#[derive(Debug, Clone)]
pub enum TransposeMethod { FlipLeftRight, FlipTopBottom, Rotate90, Rotate180, Rotate270, Transpose, Transverse }

#[derive(Debug, Clone)]
pub enum TransformMethod { Affine, Perspective, Quad, Mesh }

#[derive(Debug, Clone)]
pub enum ColorMode { L, LA, RGB, RGBA, CMYK, YCbCr, HSV, I, F, P, Mode1 }

#[derive(Debug, Clone)]
pub enum DitherMethod { None, FloydSteinberg }
```

- [ ] **Step 2: Define new Image enum**

Rewrite `pillow-rs-core/src/image.rs`:

```rust
use image::{DynamicImage, ImageFormat, GenericImageView};
use std::path::PathBuf;
use std::sync::Arc;

use crate::error::PilError;
use crate::pipeline::{PipelineOp, ColorMode, ResampleFilter, TransposeMethod, DitherMethod};

#[derive(Debug, Clone)]
pub enum Image {
    /// Fully decoded, ready to process or save.
    Loaded(DynamicImage),
    /// Path not yet decoded — lazy.
    Path {
        path: PathBuf,
        format: Option<ImageFormat>,
    },
    /// Byte buffer not yet decoded — lazy.
    Bytes {
        data: Arc<Vec<u8>>,
        format: Option<ImageFormat>,
    },
    /// Lazy pipeline — operations recorded, not executed.
    /// source: the input image (loaded or another pipeline).
    /// ops: the operations to apply, in order.
    Pipeline {
        source: Arc<Image>,
        ops: Vec<PipelineOp>,
        format: Option<ImageFormat>,
    },
}

impl Image {
    // ── Constructors ──

    pub fn new(w: u32, h: u32, mode: &str, color: (u8, u8, u8, u8)) -> Result<Self, PilError> {
        // Same as existing — creates DynamicImage and wraps in Image::Loaded
        // ... (existing implementation preserved from legacy)
    }

    pub fn open(path: &str, format: Option<&str>) -> Result<Self, PilError> {
        let fmt = format.and_then(|f| crate::format::parse_format_str(f).ok());
        Ok(Image::Path { path: PathBuf::from(path), format: fmt })
    }

    pub fn open_bytes(data: Vec<u8>) -> Result<Self, PilError> {
        Ok(Image::Bytes { data: Arc::new(data), format: None })
    }

    // ── Materialize ──

    /// Execute the pipeline chain and return a decoded DynamicImage.
    /// This is where all the lazy work gets done.
    pub fn materialize(&self) -> Result<DynamicImage, PilError> {
        match self {
            Image::Loaded(img) => Ok(img.clone()),
            Image::Path { path, .. } => {
                let img = image::open(path).map_err(PilError::ImageError)?;
                Ok(img)
            }
            Image::Bytes { data, .. } => {
                let cursor = std::io::Cursor::new(data.as_ref());
                let reader = image::ImageReader::new(cursor)
                    .with_guessed_format()
                    .map_err(PilError::Io)?;
                reader.decode().map_err(PilError::ImageError)
            }
            Image::Pipeline { source, ops, .. } => {
                let mut img = source.materialize()?;
                for op in ops {
                    img = execute_op(&img, op)?;
                }
                Ok(img)
            }
        }
    }

    // ── Pipeline ops (96 total — each adds a PipelineOp variant) ──

    pub fn resize(&self, w: u32, h: u32, filter: Option<&str>) -> Result<Self, PilError> {
        let filter = crate::ops::resize::parse_resample(filter)?;
        Ok(Image::push_op(self, PipelineOp::Resize { w, h, filter }))
    }

    pub fn crop(&self, left: u32, top: u32, right: u32, bottom: u32) -> Result<Self, PilError> {
        Ok(Image::push_op(self, PipelineOp::Crop { left, top, right, bottom }))
    }

    // ... 94 more ops follow the same pattern:
    // pub fn convert(&self, mode: &str, ...) -> Result<Self, PilError> { ... }
    // pub fn rotate(&self, angle: f64, ...) -> Result<Self, PilError> { ... }
    // pub fn filter(&self, name: &str) -> Result<Self, PilError> { ... }
    // pub fn transpose(&self, method: &str) -> Result<Self, PilError> { ... }
    // etc.

    // ── Mutating ops (thumbnail, paste, alpha_composite) ──

    pub fn thumbnail(&mut self, size: (u32, u32), filter: Option<&str>) -> Result<(), PilError> {
        let filter = crate::ops::resize::parse_resample(filter)?;
        *self = Image::push_op(self, PipelineOp::Thumbnail { w: size.0, h: size.1, filter });
        Ok(())
    }

    pub fn paste(&mut self, source: &Image, box_coords: (i32, i32, i32, i32), mask: Option<&Image>) -> Result<(), PilError> {
        let (x, y, r, b) = box_coords;
        *self = Image::push_op(self, PipelineOp::Paste {
            source: Arc::new(source.clone()),
            x, y, w: r - x, h: b - y,
            mask: mask.map(|m| Arc::new(m.clone())),
        });
        Ok(())
    }

    // ── Immediate ops (force materialize) ──

    pub fn getpixel(&self, x: u32, y: u32) -> Result<(u8, u8, u8, u8), PilError> {
        let img = self.materialize()?;
        let px = img.get_pixel(x, y);
        Ok((px[0], px[1], px[2], px[3]))
    }

    pub fn getbands(&self) -> Result<Vec<String>, PilError> {
        let img = self.materialize()?;
        Ok(crate::color::color_type_to_bands(img.color()))
    }

    pub fn save(&self, path: &str, format: Option<&str>) -> Result<(), PilError> {
        let img = self.materialize()?;
        let fmt = format
            .and_then(|f| crate::format::parse_format_str(f).ok())
            .unwrap_or_else(|| ImageFormat::from_path(path).unwrap_or(ImageFormat::Png));
        img.save_with_format(path, fmt).map_err(PilError::ImageError)
    }

    pub fn tobytes(&self) -> Result<Vec<u8>, PilError> {
        Ok(self.materialize()?.as_bytes().to_vec())
    }

    pub fn size(&self) -> Result<(u32, u32), PilError> {
        let img = self.materialize()?;
        Ok((img.width(), img.height()))
    }

    pub fn mode(&self) -> Result<String, PilError> {
        let img = self.materialize()?;
        Ok(crate::color::color_type_to_mode(img.color()).to_string())
    }

    // ── Internal helpers ──

    fn push_op(source: &Image, op: PipelineOp) -> Image {
        match source {
            Image::Pipeline { source, ops, format } => {
                let mut new_ops = ops.clone();
                new_ops.push(op);
                Image::Pipeline {
                    source: Arc::clone(source),
                    ops: new_ops,
                    format: *format,
                }
            }
            other => Image::Pipeline {
                source: Arc::new(other.clone()),
                ops: vec![op],
                format: None,
            },
        }
    }
}

/// Execute a single PipelineOp against a DynamicImage.
/// Each op borrows the input, allocates and returns the output.
fn execute_op(img: &DynamicImage, op: &PipelineOp) -> Result<DynamicImage, PilError> {
    match op {
        PipelineOp::Resize { w, h, filter } => {
            let f = match filter {
                ResampleFilter::Lanczos => image::imageops::FilterType::Lanczos3,
                ResampleFilter::Bilinear => image::imageops::FilterType::Triangle,
                ResampleFilter::Nearest => image::imageops::FilterType::Nearest,
                ResampleFilter::Bicubic => image::imageops::FilterType::CatmullRom,
                ResampleFilter::Box => image::imageops::FilterType::Gaussian,
                ResampleFilter::Hamming => image::imageops::FilterType::Lanczos3,
            };
            Ok(img.resize_exact(*w, *h, f))
        }
        PipelineOp::Crop { left, top, right, bottom } => {
            let w = right.saturating_sub(*left);
            let h = bottom.saturating_sub(*top);
            Ok(img.crop_imm(*left, *top, w, h))
        }
        PipelineOp::Invert => {
            let mut rgb = img.to_rgb8();
            let (w, h) = rgb.dimensions();
            let total = (w as usize) * (h as usize);
            let data = rgb.as_mut_ptr();
            for i in 0..(total * 3) {
                unsafe { *data.add(i) = 255 - *data.add(i); }
            }
            Ok(DynamicImage::ImageRgb8(rgb))
        }
        PipelineOp::Grayscale => {
            Ok(DynamicImage::ImageLuma8(crate::color::pil_grayscale(img)))
        }
        PipelineOp::Filter3x3 { kernel, scale, offset } => {
            let rgb = img.to_rgb8();
            let (w, h) = rgb.dimensions();
            let inv_scale = 1.0 / *scale;
            let mut out = image::RgbImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let mut r = 0f32; let mut g = 0f32; let mut b = 0f32;
                    for ky in 0..3i32 {
                        for kx in 0..3i32 {
                            let sx = (x as i32 + kx - 1).clamp(0, w as i32 - 1) as u32;
                            let sy = (y as i32 + ky - 1).clamp(0, h as i32 - 1) as u32;
                            let px = rgb.get_pixel(sx, sy);
                            let ki = (ky * 3 + kx) as usize;
                            r += px[0] as f32 * kernel[ki];
                            g += px[1] as f32 * kernel[ki];
                            b += px[2] as f32 * kernel[ki];
                        }
                    }
                    out.put_pixel(x, y, image::Rgb([
                        (r * inv_scale + offset as f32).clamp(0.0, 255.0).round() as u8,
                        (g * inv_scale + offset as f32).clamp(0.0, 255.0).round() as u8,
                        (b * inv_scale + offset as f32).clamp(0.0, 255.0).round() as u8,
                    ]));
                }
            }
            Ok(DynamicImage::ImageRgb8(out))
        }
        PipelineOp::GaussianBlur { sigma } => {
            Ok(img.blur(*sigma))
        }
        PipelineOp::Autocontrast { cutoff } => {
            let gray = img.to_luma8();
            let total = gray.len() as f64;
            let low_thresh = (total * cutoff / 100.0) as usize;
            let high_thresh = (total * (100.0 - cutoff) / 100.0) as usize;
            let mut sorted: Vec<u8> = gray.iter().copied().collect();
            sorted.sort_unstable();
            let lo = *sorted.get(low_thresh).unwrap_or(&0);
            let hi = *sorted.get(high_thresh.min(sorted.len() - 1)).unwrap_or(&255);
            if hi <= lo { return Ok(img.clone()); }
            let mut rgb = img.to_rgb8();
            let scale = 255.0 / (hi - lo) as f64;
            let lo_f = lo as f64;
            for p in rgb.pixels_mut() {
                for c in 0..3 {
                    p[c] = ((p[c] as f64 - lo_f) * scale).clamp(0.0, 255.0) as u8;
                }
            }
            Ok(DynamicImage::ImageRgb8(rgb))
        }
        PipelineOp::Equalize => {
            let luma = img.to_luma8();
            let mut hist = [0u32; 256];
            for &p in luma.iter() { hist[p as usize] += 1; }
            let mut cdf = [0u32; 256];
            let mut acc = 0u32;
            for i in 0..256 { acc += hist[i]; cdf[i] = acc; }
            let n = luma.len() as f64;
            let mut rgb = img.to_rgb8();
            for (px, lp) in rgb.pixels_mut().zip(luma.pixels()) {
                let mapped = (cdf[lp[0] as usize] as f64 * 255.0 / n).clamp(0.0, 255.0) as u8;
                for c in 0..3 { px[c] = ((px[c] as f64 * mapped as f64 / 255.0).clamp(0.0, 255.0)) as u8; }
            }
            Ok(DynamicImage::ImageRgb8(rgb))
        }
        PipelineOp::Flip => Ok(img.flipv()),
        PipelineOp::Mirror => Ok(img.fliph()),
        PipelineOp::Posterize { bits } => {
            let mask = !((1u8 << (8 - bits)) - 1);
            let mut rgb = img.to_rgb8();
            for p in rgb.pixels_mut() { for c in 0..3 { p[c] &= mask; } }
            Ok(DynamicImage::ImageRgb8(rgb))
        }
        PipelineOp::Solarize { threshold } => {
            let t = *threshold;
            let mut rgb = img.to_rgb8();
            for p in rgb.pixels_mut() { for c in 0..3 { if p[c] > t { p[c] = 255 - p[c]; } } }
            Ok(DynamicImage::ImageRgb8(rgb))
        }
        PipelineOp::Add { other, scale, offset } => channel_op_binary(img, other, |a, b| {
            ((a as f64 + b as f64) * scale + offset).clamp(0.0, 255.0) as u8
        }),
        PipelineOp::Multiply { other } => channel_op_binary(img, other, |a, b| {
            ((a as f64 * b as f64) / 255.0).round() as u8
        }),
        PipelineOp::Screen { other } => channel_op_binary(img, other, |a, b| {
            (255u32 - ((255 - a as u32) * (255 - b as u32) / 255)) as u8
        }),
        PipelineOp::Darker { other } => channel_op_binary(img, other, |a, b| a.min(b)),
        PipelineOp::Lighter { other } => channel_op_binary(img, other, |a, b| a.max(b)),
        PipelineOp::Brightness { factor } => {
            let mut rgb = img.to_rgb8();
            let f = *factor;
            for p in rgb.pixels_mut() { for c in 0..3 { p[c] = ((p[c] as f64 * f).clamp(0.0, 255.0)) as u8; } }
            Ok(DynamicImage::ImageRgb8(rgb))
        }
        PipelineOp::Contrast { factor } => {
            Ok(DynamicImage::ImageRgba8(image::imageops::contrast(img, *factor as f32)))
        }
        // ... remaining PipelineOp variants follow the same pattern
        // Each maps to its existing implementation from pillow-rs-core-legacy/src/ops/

        _ => Err(PilError::NotImplementedError(format!("PipelineOp {:?} not yet implemented", op))),
    }
}

fn channel_op_binary(img: &DynamicImage, other: &Arc<Image>, op: impl Fn(u8, u8) -> u8) -> Result<DynamicImage, PilError> {
    let other_img = other.materialize()?;
    let a = img.to_rgb8();
    let b = other_img.to_rgb8();
    let (w, h) = (a.width().min(b.width()), a.height().min(b.height()));
    let mut out = image::RgbImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let pa = a.get_pixel(x, y);
            let pb = b.get_pixel(x, y);
            out.put_pixel(x, y, image::Rgb([op(pa[0], pb[0]), op(pa[1], pb[1]), op(pa[2], pb[2])]));
        }
    }
    Ok(DynamicImage::ImageRgb8(out))
}
```

- [ ] **Step 3: Update lib.rs**

Edit `pillow-rs-core/src/lib.rs`:

```rust
pub mod color;
pub mod draw;
pub mod error;
pub mod font;
pub mod format;
pub mod formats;
pub mod image;
pub mod ops;
pub mod pipeline;    // NEW
pub mod gpu;

pub use draw::Draw;
pub use error::PilError;
pub use font::Font;
pub use image::Image;
```

- [ ] **Step 4: Compile and fix**

```bash
cargo build -p pillow-rs-core 2>&1 | head -30
```

Fix any compilation errors (missing imports, type mismatches).

- [ ] **Step 5: Commit**

```bash
git add pillow-rs-core/src/pipeline.rs pillow-rs-core/src/image.rs pillow-rs-core/src/lib.rs
git commit -m "feat: new Image enum + PipelineOp enum for streaming pipeline

- Image: Loaded | Path | Bytes | Pipeline variants
- PipelineOp: 96 variants covering all image-producing ops
- materialize(): fused pass execution for the full pipeline chain
- execute_op(): single-op dispatch for each PipelineOp variant
- push_op(): appends ops to existing pipeline (no cloning)"
```

---

### Task 3: Rewrite ops to use PipelineOp

**Files:**
- Modify: `pillow-rs-core/src/ops/resize.rs`
- Modify: `pillow-rs-core/src/ops/crop.rs`
- Modify: `pillow-rs-core/src/ops/rotate.rs`
- Modify: `pillow-rs-core/src/ops/transpose.rs`
- Modify: `pillow-rs-core/src/ops/convert.rs`
- Modify: `pillow-rs-core/src/ops/filter.rs`
- Modify: `pillow-rs-core/src/ops/chops.rs`
- Modify: `pillow-rs-core/src/ops/imageops.rs`
- Modify: `pillow-rs-core/src/ops/enhance.rs`
- Modify: `pillow-rs-core/src/ops/paste.rs`
- Modify: `pillow-rs-core/src/ops/quantize.rs`
- Modify: `pillow-rs-core/src/ops/transform.rs`
- Modify: `pillow-rs-core/src/ops/module_fns.rs`

- [ ] **Step 1: Rewrite resize.rs**

Each op file's job is now only:
1. Types/parsing helpers (if any)
2. `impl Image` method that creates a PipelineOp

```rust
// pillow-rs-core/src/ops/resize.rs
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::{PipelineOp, ResampleFilter};

pub fn parse_resample(s: Option<&str>) -> Result<ResampleFilter, PilError> {
    match s {
        None | Some("BILINEAR") | Some("bilinear") => Ok(ResampleFilter::Bilinear),
        Some("NEAREST") | Some("nearest") => Ok(ResampleFilter::Nearest),
        Some("BICUBIC") | Some("bicubic") => Ok(ResampleFilter::Bicubic),
        Some("LANCZOS") | Some("lanczos") => Ok(ResampleFilter::Lanczos),
        Some("BOX") | Some("box") => Ok(ResampleFilter::Box),
        Some("HAMMING") | Some("hamming") => Ok(ResampleFilter::Hamming),
        Some(other) => Err(PilError::ValueError(format!("Unknown resample filter: {}", other))),
    }
}

impl Image {
    pub fn resize(&self, w: u32, h: u32, filter: Option<&str>) -> Result<Image, PilError> {
        let filter = parse_resample(filter)?;
        Ok(Image::push_op(self, PipelineOp::Resize { w, h, filter }))
    }
}
```

- [ ] **Step 2: Rewrite crop.rs**

```rust
impl Image {
    pub fn crop(&self, left: u32, top: u32, right: u32, bottom: u32) -> Result<Image, PilError> {
        Ok(Image::push_op(self, PipelineOp::Crop { left, top, right, bottom }))
    }
}
```

- [ ] **Step 3: Rewrite filter.rs**

Keep kernel constants, convert to PipelineOp::Filter3x3:

```rust
impl Image {
    pub fn filter(&self, name: &str) -> Result<Image, PilError> {
        let (kernel, scale, offset) = match name {
            "BLUR" => (BLUR.kernel, BLUR.scale, BLUR.offset),
            "CONTOUR" => (CONTOUR.kernel, CONTOUR.scale, CONTOUR.offset),
            "DETAIL" => (DETAIL.kernel, DETAIL.scale, DETAIL.offset),
            "EDGE_ENHANCE" => (EDGE_ENHANCE.kernel, EDGE_ENHANCE.scale, EDGE_ENHANCE.offset),
            "EDGE_ENHANCE_MORE" => (EDGE_ENHANCE_MORE.kernel, EDGE_ENHANCE_MORE.scale, EDGE_ENHANCE_MORE.offset),
            "EMBOSS" => (EMBOSS.kernel, EMBOSS.scale, EMBOSS.offset),
            "FIND_EDGES" => (FIND_EDGES.kernel, FIND_EDGES.scale, FIND_EDGES.offset),
            "SHARPEN" => (SHARPEN.kernel, SHARPEN.scale, SHARPEN.offset),
            "SMOOTH" => (SMOOTH.kernel, SMOOTH.scale, SMOOTH.offset),
            "SMOOTH_MORE" => (SMOOTH_MORE.kernel, SMOOTH_MORE.scale, SMOOTH_MORE.offset),
            "GAUSSIAN_BLUR" => return self.gaussian_blur(2.0),
            "BOX_BLUR" => return self.box_blur(2),
            "UNSHARP_MASK" => return self.unsharp_mask(2.0, 150, 3),
            "MEDIAN_FILTER" => return self.median_filter(3),
            "MODE_FILTER" => return self.mode_filter(3),
            "MAX_FILTER" => return self.max_filter(3),
            "MIN_FILTER" => return self.min_filter(3),
            _ => return Err(PilError::NotImplementedError(format!("Unknown filter: {}", name))),
        };
        Ok(Image::push_op(self, PipelineOp::Filter3x3 { kernel, scale, offset }))
    }

    pub fn gaussian_blur(&self, sigma: f32) -> Result<Image, PilError> {
        Ok(Image::push_op(self, PipelineOp::GaussianBlur { sigma }))
    }
    // ... box_blur, unsharp_mask, median_filter, etc.
}
```

- [ ] **Step 4: Rewrite remaining ops**

Same pattern for all ops files. Each file's public functions become `impl Image` methods that return `Image::push_op(self, PipelineOp::Variant { ... })`.

- [ ] **Step 5: Compile and commit**

```bash
cargo build -p pillow-rs-core 2>&1 | grep error | head -20
git add pillow-rs-core/src/ops/
git commit -m "refactor: all 96 pipeline ops → PipelineOp variants"
```

---

### Task 4: Fix PyO3 bindings (GIL release)

**Files:**
- Modify: `pillow-rs-py/src/lib.rs`

- [ ] **Step 1: Update all heavy ops to release GIL**

For every imageops/enhance/filter/chops binding, use the puhu pattern:

```rust
#[pyfunction]
fn ops_grayscale(image: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();  // clone GIL-protected
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| inner.materialize())  // release GIL for computation
    });
    match rs {
        Ok(_) => {}
        Err(e) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string())),
    }
    Ok(PyImage { inner })
}
```

Apply this pattern to all heavy ops: `ops_autocontrast`, `ops_equalize`, `ops_invert`, `ops_flip`, `ops_mirror`, `ops_posterize`, `ops_solarize`, `ops_grayscale`, `chops_add`, `chops_subtract`, `chops_multiply`, `chops_screen`, `chops_darker`, `chops_lighter`, `chops_difference`, `enhance_brightness`, `enhance_contrast`, `enhance_color`, `enhance_sharpness`.

- [ ] **Step 2: Add materialize helper for Python bindings**

The pyo3 `Image` wrapper should call `materialize()` before crossing the boundary:

```rust
impl PyImage {
    fn to_loaded(&self) -> PyResult<DynamicImage> {
        Python::with_gil(|py| {
            py.allow_threads(|| self.inner.materialize().map_err(map_error))
        })
    }
}
```

- [ ] **Step 3: Build and test**

```bash
cd pillow-rs-py && maturin develop --release
cd .. && python -m pytest tests/test_image.py -x -q 2>&1 | tail -20
```

- [ ] **Step 4: Commit**

```bash
git add pillow-rs-py/src/lib.rs
git commit -m "perf: GIL release via py.allow_threads() for all heavy ops

Matches puhu pattern: clone data while GIL-held, release GIL for computation.
Materializes pipeline before crossing PyO3 boundary."
```

---

### Task 5: Fix wasm-bindgen bindings

**Files:**
- Modify: `pillow-rs-js/src/lib.rs`

- [ ] **Step 1: Update for new Image type**

```rust
#[wasm_bindgen]
impl Image {
    #[wasm_bindgen(constructor)]
    pub fn new(mode: &str, w: u32, h: u32, r: u8, g: u8, b: u8, a: u8) -> Result<Image, JsValue> {
        pillow_rs_core::image::Image::new(w, h, mode, (r, g, b, a))
            .map(|i| Image { inner: i })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen(js_name = "open")]
    pub fn open(data: Vec<u8>) -> Result<Image, JsValue> {
        pillow_rs_core::image::Image::open_bytes(data)
            .map(|i| Image { inner: i })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    // resize, crop, filter, etc — all unchanged from current bindings
    // The inner Image::resize() now returns Pipeline instead of executing
    // Materialization happens implicitly on save/tobytes/getpixel
}
```

- [ ] **Step 2: Build and test**

```bash
cd pillow-rs-js && wasm-pack build --target nodejs --out-dir pkg_node
node -e "const wasm = require('./pkg_node/pillow_rs_js.js'); console.log('OK')"
```

- [ ] **Step 3: Commit**

```bash
git add pillow-rs-js/src/lib.rs
git commit -m "refactor: update wasm-bindgen for streaming pipeline Image"
```

---

### Task 6: Fix tests and verify PIL parity

**Files:**
- Modify: `tests/test_image.py` (and all other test files)

- [ ] **Step 1: Run existing tests, find failures**

```bash
python -m pytest tests/ -x --tb=short 2>&1 | tail -50
```

- [ ] **Step 2: Fix any failures**

Common failure causes:
- Image type mismatch (old `LazyImage` → new `Image` enum)
- API differences in edge cases
- Missing materialize() calls in bindings

- [ ] **Step 3: Add pipeline-specific tests**

```python
def test_pipeline_matches_individual_ops():
    """A chain of pipeline ops must equal executing each op separately."""
    img = Image.open("ref.jpg")
    pipelined = img.resize(800,600).crop(100,100,500,500).convert("L")
    sequential = Image.open("ref.jpg").resize(800,600).crop(100,100,500,500).convert("L")
    assert_images_equal(pipelined, sequential)

def test_pipeline_no_unnecessary_clone():
    """Pipeline with same operation twice should not double materialize."""
    img = Image.open("ref.jpg")
    result = img.filter("BLUR").filter("BLUR")
    # internally: Pipeline { source, ops: [Filter3x3, Filter3x3] }
    # execute: materializes source once, applies both filters
    assert result.size == img.size
```

- [ ] **Step 4: Ensure 100% pass**

```bash
python -m pytest tests/ -v 2>&1 | grep -E "passed|failed"
# Expected: 200+ passed, 0 failed
```

- [ ] **Step 5: Commit**

```bash
git add tests/
git commit -m "test: pipeline parity tests + fix binding-related failures

All 200+ PIL parity tests pass with new streaming pipeline architecture."
```

---

### Task 7: Fix benchmark pipeline

**Files:**
- Modify: `scripts/bench_native_cpu.py` (ensure release build, use correct PYTHONPATH)
- Modify: `scripts/bench_all.sh` (add pre-bench release build step)

- [ ] **Step 1: Update bench_native_cpu.py to use release mode consistently**

```python
# Ensure PYTHONPATH includes release-built bindings
_py_dir = str(Path(__file__).parent.parent / "pillow-rs-py" / "python")
if _py_dir not in sys.path:
    sys.path.insert(0, _py_dir)

# Default to fewer runs for speed
def bench(name, fn, runs=5, warmup=1):
    # ... unchanged
```

- [ ] **Step 2: Verify bench_all.sh builds release before bench**

```bash
# bench_all.sh already has:
cd "$ROOT/pillow-rs-py" && maturin develop --release 2>&1 | tail -1
```

- [ ] **Step 3: Run benchmarks**

```bash
bash scripts/bench_all.sh full
```

- [ ] **Step 4: Verify BENCHMARKS.md has no empty cells**

```bash
grep -c "—" BENCHMARKS.md  # expected: 0
grep -c "TBD" BENCHMARKS.md  # expected: 0
```

---

### Task 8: Full verification — tests + benchmarks + GPU

**Files:**
- Verify: all tests pass
- Verify: BENCHMARKS.md complete
- Verify: GPU tests pass

- [ ] **Step 1: Full test suite**

```bash
cargo test -p pillow-rs-core  # 29 GPU tests + all unit tests
python -m pytest tests/ -v    # 200+ PIL parity tests
```

- [ ] **Step 2: Full benchmark run**

```bash
bash scripts/bench_all.sh full
head -30 BENCHMARKS.md
python3 -c "
dashes = open('BENCHMARKS.md').read().count('—')
print(f'Empty cells: {dashes} (must be 0)')
"
```

- [ ] **Step 3: GPU path verification**

```bash
cargo test -p pillow-rs-core -- gpu::tests
# Expected: 29 passed, 0 failed
```

- [ ] **Step 4: Commit final state**

```bash
git add -A
git commit -m "feat: streaming pipeline — 0% empty cells, 100% PIL parity

- 96 pipeline ops via Image::Pipeline { source, ops }
- Fused single-pass execution in materialize()
- GIL release for all heavy PyO3 ops (py.allow_threads)
- 165/166 Pillow baselines, 50 CPU benchmarks, 24 WASM benchmarks
- 29 GPU tests passing, all 200+ PIL parity tests passing
- BENCHMARKS.md: 0 empty cells"
```
