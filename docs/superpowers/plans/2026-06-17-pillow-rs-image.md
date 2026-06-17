# pillow-rs-image Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `pillow-rs-image` — a zero-C-dependency, pixel-perfect image codec crate that replaces the `image` crate in `pillow-rs`. Own all types (DynamicImage, GrayImage, RgbImage, RgbaImage), all decoders (JPEG IJG IDCT, PNG/GIF/BMP/TIFF/WebP/ICO/AVIF), and all encoders.

**Architecture:** `pillow-rs-image` owns the type system (matching `image` crate API for drop-in replacement). Decode/encode modules per format. `pillow-rs` drops `image` crate dependency and uses `pillow-rs-image` instead. All formats tested against C library reference output (libjpeg, libpng, etc.) via 128+ test cases defined in `manifest.yaml`.

**Tech Stack:** Pure Rust, zero C dependencies. IJG DCT_ISLOW IDCT for JPEG. `png`/`gif`/`tiff` crates (MIT, pure Rust) for those formats. Own WebP/BMP/ICO implementations. `zenavif`/`ravif` (feature-gated) for AVIF.

---

## File Structure

| File | Purpose |
|------|---------|
| `pillow-rs-image/src/lib.rs` | `detect_format()`, `decode()`, `encode()`, re-exports |
| `pillow-rs-image/src/types/mod.rs` | Re-exports for type module |
| `pillow-rs-image/src/types/traits.rs` | `Pixel`, `GenericImageView` traits |
| `pillow-rs-image/src/types/buffer.rs` | `ImageBuffer<P, Container>` — generic image buffer |
| `pillow-rs-image/src/types/dynamic.rs` | `DynamicImage` enum |
| `pillow-rs-image/src/types/color.rs` | `Luma`, `Rgb`, `Rgba`, `LumaA` pixel types, `ColorType` |
| `pillow-rs-image/src/decode/mod.rs` | Unified `decode(&[u8]) -> Option<DynamicImage>` |
| `pillow-rs-image/src/decode/jpeg.rs` | IJG DCT_ISLOW + Huffman + YCbCr→RGB |
| `pillow-rs-image/src/decode/png.rs` | Wrap `png` crate |
| `pillow-rs-image/src/decode/gif.rs` | Wrap `gif` crate |
| `pillow-rs-image/src/decode/bmp.rs` | Direct BMP decoder |
| `pillow-rs-image/src/decode/tiff.rs` | Wrap `tiff` crate |
| `pillow-rs-image/src/decode/webp.rs` | Own VP8/VP8L decoder |
| `pillow-rs-image/src/decode/ico.rs` | Wrap BMP/PNG |
| `pillow-rs-image/src/decode/avif.rs` | Wrap `zenavif` |
| `pillow-rs-image/src/encode/mod.rs` | Unified `encode(&DynamicImage, ImageFormat) -> Vec<u8>` |
| `pillow-rs-image/src/encode/jpeg.rs` | Wrap `jpeg-encoder` crate |
| `pillow-rs-image/src/encode/png.rs` | Wrap `png` crate encoder |
| `pillow-rs-image/src/encode/gif.rs` | Wrap `gif` crate encoder |
| `pillow-rs-image/src/encode/bmp.rs` | Direct BMP encoder |
| `pillow-rs-image/src/encode/tiff.rs` | Wrap `tiff` crate encoder |
| `pillow-rs-image/src/encode/webp.rs` | Own VP8 encoder |
| `pillow-rs-image/src/encode/ico.rs` | Wrap BMP/PNG encode |
| `pillow-rs-image/src/encode/avif.rs` | Wrap `ravif` |
| `pillow-rs-image/tests/decode_tests.rs` | Auto-discovered decode tests from manifest |
| `pillow-rs-image/tests/encode_tests.rs` | Encode roundtrip tests |
| `pillow-rs-image/scripts/generate_decode_refs.py` | Generate reference .bin from PIL |
| `pillow-rs/src/image.rs` | Replace `image::open` with `pillow_rs_image::decode` |
| `pillow-rs/Cargo.toml` | Drop `image` dep, add `pillow-rs-image` |
| 23 files in `pillow-rs/src/` | Replace `use image::*` with `use pillow_rs_image::*` |

---

### Task 1: Type System — Pixel Types and ColorType

**Files:**
- Create: `pillow-rs-image/src/types/color.rs`
- Create: `pillow-rs-image/src/types/traits.rs`
- Modify: `pillow-rs-image/src/types/mod.rs`

- [ ] **Step 1: Write pixel types and ColorType**

```rust
// pillow-rs-image/src/types/color.rs
use std::ops::{Deref, DerefMut, Index, IndexMut};

/// A single grayscale pixel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Luma<T: Primitive>(pub [T; 1]);

/// A grayscale + alpha pixel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LumaA<T: Primitive>(pub [T; 2]);

/// An RGB pixel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rgb<T: Primitive>(pub [T; 3]);

/// An RGBA pixel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rgba<T: Primitive>(pub [T; 4]);

/// Trait for numeric types used in pixel components.
pub trait Primitive: Copy + Clone + Default + PartialEq + 'static {
    const ZERO: Self;
    const MAX: Self;
}
impl Primitive for u8 { const ZERO: u8 = 0; const MAX: u8 = 255; }
impl Primitive for u16 { const ZERO: u16 = 0; const MAX: u16 = 65535; }
impl Primitive for f32 { const ZERO: f32 = 0.0; const MAX: f32 = 1.0; }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorType {
    L8, La8, Rgb8, Rgba8,
    L16, La16, Rgb16, Rgba16,
}

impl ColorType {
    pub fn channels(self) -> u8 {
        match self {
            ColorType::L8 | ColorType::L16 => 1,
            ColorType::La8 | ColorType::La16 => 2,
            ColorType::Rgb8 | ColorType::Rgb16 => 3,
            ColorType::Rgba8 | ColorType::Rgba16 => 4,
        }
    }
    pub fn bits_per_pixel(self) -> u16 {
        self.channels() as u16 * if matches!(self, ColorType::L16 | ColorType::La16 | ColorType::Rgb16 | ColorType::Rgba16) { 16 } else { 8 }
    }
}
```

- [ ] **Step 2: Write Pixel trait**

```rust
// pillow-rs-image/src/types/traits.rs
use super::color::{Luma, LumaA, Rgb, Rgba, Primitive};

pub trait Pixel: Copy + Clone + std::fmt::Debug {
    type Subpixel: Primitive;
    const CHANNEL_COUNT: u8;
    fn channels(&self) -> &[Self::Subpixel];
    fn channels_mut(&mut self) -> &mut [Self::Subpixel];
    fn from_slice(slice: &[Self::Subpixel]) -> &Self;
    fn from_slice_mut(slice: &mut [Self::Subpixel]) -> &mut Self;
}

macro_rules! impl_pixel {
    ($ty:ident, $n:expr) => {
        impl<T: Primitive> Pixel for $ty<T> {
            type Subpixel = T;
            const CHANNEL_COUNT: u8 = $n;
            fn channels(&self) -> &[T] { &self.0[..] }
            fn channels_mut(&mut self) -> &mut [T] { &mut self.0[..] }
            fn from_slice(slice: &[T]) -> &Self {
                assert_eq!(slice.len(), $n);
                unsafe { &*(slice.as_ptr() as *const Self) }
            }
            fn from_slice_mut(slice: &mut [T]) -> &mut Self {
                assert_eq!(slice.len(), $n);
                unsafe { &mut *(slice.as_mut_ptr() as *mut Self) }
            }
        }
    };
}
impl_pixel!(Luma, 1);
impl_pixel!(LumaA, 2);
impl_pixel!(Rgb, 3);
impl_pixel!(Rgba, 4);
```

- [ ] **Step 3: Write GenericImageView trait**

```rust
// Append to pillow-rs-image/src/types/traits.rs

/// Trait providing read-only access to image dimensions and pixels.
/// Mirrors `image::GenericImageView`.
pub trait GenericImageView {
    type Pixel: Pixel;

    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn dimensions(&self) -> (u32, u32) { (self.width(), self.height()) }

    fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel;
    fn get_pixel_checked(&self, x: u32, y: u32) -> Option<Self::Pixel> {
        if x < self.width() && y < self.height() {
            Some(self.get_pixel(x, y))
        } else {
            None
        }
    }

    /// Return the raw byte slice of pixel data.
    fn as_bytes(&self) -> &[u8];

    /// Iterate over all pixels in row-major order.
    fn pixels(&self) -> Pixels<'_, Self> where Self: Sized {
        Pixels { image: self, x: 0, y: 0 }
    }
}

/// Row-major pixel iterator.
pub struct Pixels<'a, I: GenericImageView + ?Sized> {
    image: &'a I,
    x: u32,
    y: u32,
}

impl<'a, I: GenericImageView + ?Sized> Iterator for Pixels<'a, I> {
    type Item = (u32, u32, I::Pixel);
    fn next(&mut self) -> Option<Self::Item> {
        if self.y >= self.image.height() { return None; }
        let (x, y) = (self.x, self.y);
        let px = self.image.get_pixel(x, y);
        self.x += 1;
        if self.x >= self.image.width() { self.x = 0; self.y += 1; }
        Some((x, y, px))
    }
}
```

- [ ] **Step 4: Update types/mod.rs**

```rust
// pillow-rs-image/src/types/mod.rs
pub mod color;
pub mod traits;
pub mod buffer;
pub mod dynamic;

pub use color::{Luma, LumaA, Rgb, Rgba, Primitive, ColorType};
pub use traits::{Pixel, GenericImageView, Pixels};
pub use buffer::ImageBuffer;
pub use dynamic::DynamicImage;
```

- [ ] **Step 5: Build and verify**

```bash
cargo build -p pillow-rs-image
```

Expected: Compiles with only warnings.

- [ ] **Step 6: Commit**

```bash
git add pillow-rs-image/src/types/
git commit -m "feat(types): Pixel, GenericImageView traits, ColorType"
```

---

### Task 2: Type System — ImageBuffer and DynamicImage

**Files:**
- Create: `pillow-rs-image/src/types/buffer.rs`
- Create: `pillow-rs-image/src/types/dynamic.rs`

- [ ] **Step 1: Write ImageBuffer**

```rust
// pillow-rs-image/src/types/buffer.rs
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut, Index, IndexMut};
use super::traits::{Pixel, GenericImageView, Pixels};
use super::color::Primitive;

/// A generic image buffer with pixel type P stored in Container.
/// Mirrors `image::ImageBuffer<P, Container>`.
#[derive(Debug, Clone)]
pub struct ImageBuffer<P, Container> {
    width: u32,
    height: u32,
    _phantom: PhantomData<P>,
    data: Container,
}

// Type aliases used throughout pillow-rs
pub type GrayImage = ImageBuffer<super::color::Luma<u8>, Vec<u8>>;
pub type GrayAlphaImage = ImageBuffer<super::color::LumaA<u8>, Vec<u8>>;
pub type RgbImage = ImageBuffer<super::color::Rgb<u8>, Vec<u8>>;
pub type RgbaImage = ImageBuffer<super::color::Rgba<u8>, Vec<u8>>;

impl<P: Pixel, Container: Deref<Target = [P::Subpixel]>> ImageBuffer<P, Container> {
    /// Create from raw parts without validation.
    pub fn from_raw(width: u32, height: u32, data: Container) -> Option<Self> {
        if width == 0 || height == 0 { return None; }
        let expected = width as usize * height as usize * P::CHANNEL_COUNT as usize;
        if data.len() < expected { return None; }
        Some(ImageBuffer { width, height, _phantom: PhantomData, data })
    }

    pub fn width(&self) -> u32 { self.width }
    pub fn height(&self) -> u32 { self.height }
    pub fn dimensions(&self) -> (u32, u32) { (self.width, self.height) }
    pub fn as_raw(&self) -> &[P::Subpixel] { &self.data }
    pub fn into_raw(self) -> Container { self.data }
    pub fn as_bytes(&self) -> &[u8] {
        let len = self.data.len() * std::mem::size_of::<P::Subpixel>();
        unsafe { std::slice::from_raw_parts(self.data.as_ptr() as *const u8, len) }
    }
}

impl<P: Pixel> ImageBuffer<P, Vec<P::Subpixel>> {
    pub fn new(width: u32, height: u32) -> Self {
        let size = width as usize * height as usize * P::CHANNEL_COUNT as usize;
        ImageBuffer {
            width, height, _phantom: PhantomData,
            data: vec![P::Subpixel::ZERO; size],
        }
    }

    pub fn from_pixel(width: u32, height: u32, pixel: P) -> Self {
        let size = width as usize * height as usize;
        let mut data = Vec::with_capacity(size * P::CHANNEL_COUNT as usize);
        for _ in 0..size {
            data.extend_from_slice(pixel.channels());
        }
        ImageBuffer { width, height, _phantom: PhantomData, data }
    }

    pub fn put_pixel(&mut self, x: u32, y: u32, pixel: P) {
        let idx = (y * self.width + x) as usize * P::CHANNEL_COUNT as usize;
        let slice = &mut self.data[idx..idx + P::CHANNEL_COUNT as usize];
        slice.copy_from_slice(pixel.channels());
    }
}

impl<P: Pixel, C: Deref<Target = [P::Subpixel]>> GenericImageView for ImageBuffer<P, C> {
    type Pixel = P;
    fn width(&self) -> u32 { self.width }
    fn height(&self) -> u32 { self.height }
    fn get_pixel(&self, x: u32, y: u32) -> P {
        let idx = (y * self.width + x) as usize * P::CHANNEL_COUNT as usize;
        *P::from_slice(&self.data[idx..idx + P::CHANNEL_COUNT as usize])
    }
    fn as_bytes(&self) -> &[u8] { ImageBuffer::as_bytes(self) }
}
```

- [ ] **Step 2: Write DynamicImage**

```rust
// pillow-rs-image/src/types/dynamic.rs
use super::buffer::{GrayImage, GrayAlphaImage, RgbImage, RgbaImage};
use super::color::ColorType;
use super::traits::{Pixel, GenericImageView};

#[derive(Debug, Clone)]
pub enum DynamicImage {
    ImageLuma8(GrayImage),
    ImageLumaA8(GrayAlphaImage),
    ImageRgb8(RgbImage),
    ImageRgba8(RgbaImage),
}

impl DynamicImage {
    pub fn color(&self) -> ColorType {
        match self {
            DynamicImage::ImageLuma8(_) => ColorType::L8,
            DynamicImage::ImageLumaA8(_) => ColorType::La8,
            DynamicImage::ImageRgb8(_) => ColorType::Rgb8,
            DynamicImage::ImageRgba8(_) => ColorType::Rgba8,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        match self {
            DynamicImage::ImageLuma8(img) => img.as_bytes(),
            DynamicImage::ImageLumaA8(img) => img.as_bytes(),
            DynamicImage::ImageRgb8(img) => img.as_bytes(),
            DynamicImage::ImageRgba8(img) => img.as_bytes(),
        }
    }

    pub fn to_rgba8(&self) -> RgbaImage { /* TODO: implement later */ unimplemented!() }
    pub fn to_rgb8(&self) -> RgbImage { /* TODO: implement later */ unimplemented!() }
    pub fn to_luma8(&self) -> GrayImage { /* TODO: implement later */ unimplemented!() }
    pub fn to_luma_alpha8(&self) -> GrayAlphaImage { /* TODO: implement later */ unimplemented!() }
}

impl GenericImageView for DynamicImage {
    type Pixel = super::color::Rgba<u8>;
    fn width(&self) -> u32 {
        match self {
            DynamicImage::ImageLuma8(i) => i.width(),
            DynamicImage::ImageLumaA8(i) => i.width(),
            DynamicImage::ImageRgb8(i) => i.width(),
            DynamicImage::ImageRgba8(i) => i.width(),
        }
    }
    fn height(&self) -> u32 {
        match self {
            DynamicImage::ImageLuma8(i) => i.height(),
            DynamicImage::ImageLumaA8(i) => i.height(),
            DynamicImage::ImageRgb8(i) => i.height(),
            DynamicImage::ImageRgba8(i) => i.height(),
        }
    }
    fn get_pixel(&self, x: u32, y: u32) -> super::color::Rgba<u8> {
        match self {
            DynamicImage::ImageLuma8(i) => {
                let l = i.get_pixel(x, y).0[0];
                super::color::Rgba([l, l, l, 255])
            }
            DynamicImage::ImageLumaA8(i) => {
                let la = i.get_pixel(x, y).0;
                super::color::Rgba([la[0], la[0], la[0], la[1]])
            }
            DynamicImage::ImageRgb8(i) => {
                let rgb = i.get_pixel(x, y).0;
                super::color::Rgba([rgb[0], rgb[1], rgb[2], 255])
            }
            DynamicImage::ImageRgba8(i) => {
                let rgba = i.get_pixel(x, y).0;
                super::color::Rgba(rgba)
            }
        }
    }
    fn as_bytes(&self) -> &[u8] { DynamicImage::as_bytes(self) }
}
```

- [ ] **Step 3: Build**

```bash
cargo build -p pillow-rs-image
```

Expected: Compiles. `unimplemented!()` macros for to_* methods are OK for now.

- [ ] **Step 4: Commit**

```bash
git add pillow-rs-image/src/types/
git commit -m "feat(types): ImageBuffer, DynamicImage — matching image crate API"
```

---

### Task 3: Format Detection + Unified Decode/Encode API

**Files:**
- Modify: `pillow-rs-image/src/lib.rs`
- Create: `pillow-rs-image/src/decode/mod.rs`
- Create: `pillow-rs-image/src/encode/mod.rs`

- [ ] **Step 1: Write format detection and ImageFormat enum**

```rust
// Replace pillow-rs-image/src/lib.rs

pub mod types;
pub mod decode;
pub mod encode;

/// Supported image formats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageFormat {
    Jpeg, Png, Gif, Bmp, Tiff, WebP, Ico, Avif,
}

/// Detect image format from magic bytes. Returns None if unrecognized.
pub fn detect_format(data: &[u8]) -> Option<ImageFormat> {
    if data.len() < 12 { return None; }
    if &data[0..2] == b"\xFF\xD8" { return Some(ImageFormat::Jpeg); }
    if data.len() >= 8 && &data[0..8] == b"\x89PNG\r\n\x1a\n" { return Some(ImageFormat::Png); }
    if data.len() >= 4 && &data[0..4] == b"GIF8" { return Some(ImageFormat::Gif); }
    if &data[0..2] == b"BM" { return Some(ImageFormat::Bmp); }
    if &data[0..2] == b"II" || &data[0..2] == b"MM" {
        if data[2] == 0x2A && data[3] == 0x00 { return Some(ImageFormat::Tiff); }
    }
    if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" { return Some(ImageFormat::WebP); }
    if &data[0..4] == b"\x00\x00\x01\x00" { return Some(ImageFormat::Ico); }
    if data.len() >= 12 && &data[4..12] == b"ftypavif" { return Some(ImageFormat::Avif); }
    None
}

/// Decode any supported format. Format is auto-detected from magic bytes.
pub fn decode(data: &[u8]) -> Option<types::dynamic::DynamicImage> {
    let fmt = detect_format(data)?;
    decode::decode_format(data, fmt)
}

/// Encode a DynamicImage to the given format.
pub fn encode(img: &types::dynamic::DynamicImage, format: ImageFormat) -> Option<Vec<u8>> {
    encode::encode_format(img, format)
}
```

- [ ] **Step 2: Write decode dispatch**

```rust
// pillow-rs-image/src/decode/mod.rs
use crate::types::dynamic::DynamicImage;
use crate::ImageFormat;

pub fn decode_format(data: &[u8], format: ImageFormat) -> Option<DynamicImage> {
    match format {
        ImageFormat::Jpeg => super::jpeg::decode(data),
        ImageFormat::Png  => super::png::decode(data),
        ImageFormat::Gif  => super::gif::decode(data),
        ImageFormat::Bmp  => super::bmp::decode(data),
        ImageFormat::Tiff => super::tiff::decode(data),
        ImageFormat::WebP => super::webp::decode(data),
        ImageFormat::Ico  => super::ico::decode(data),
        ImageFormat::Avif => super::avif::decode(data),
    }
}
```

- [ ] **Step 3: Write encode dispatch**

```rust
// pillow-rs-image/src/encode/mod.rs
use crate::types::dynamic::DynamicImage;
use crate::ImageFormat;

pub fn encode_format(img: &DynamicImage, format: ImageFormat) -> Option<Vec<u8>> {
    match format {
        ImageFormat::Jpeg => super::jpeg::encode(img),
        ImageFormat::Png  => super::png::encode(img),
        ImageFormat::Gif  => super::gif::encode(img),
        ImageFormat::Bmp  => super::bmp::encode(img),
        ImageFormat::Tiff => super::tiff::encode(img),
        ImageFormat::WebP => super::webp::encode(img),
        ImageFormat::Ico  => super::ico::encode(img),
        ImageFormat::Avif => super::avif::encode(img),
    }
}
```

- [ ] **Step 4: Update stubs to match new structure**

Ensure all `pillow-rs-image/src/decode/*.rs` and `pillow-rs-image/src/encode/*.rs` have matching stubs:

```rust
// Each file: pillow-rs-image/src/decode/{jpeg,png,gif,bmp,tiff,webp,ico,avif}.rs
use crate::types::dynamic::DynamicImage;
pub fn decode(_data: &[u8]) -> Option<DynamicImage> { None }

// Each file: pillow-rs-image/src/encode/{jpeg,png,gif,bmp,tiff,webp,ico,avif}.rs
use crate::types::dynamic::DynamicImage;
pub fn encode(_img: &DynamicImage) -> Option<Vec<u8>> { None }
```

- [ ] **Step 5: Build**

```bash
cargo build -p pillow-rs-image
```

Expected: Compiles.

- [ ] **Step 6: Commit**

```bash
git add pillow-rs-image/src/lib.rs pillow-rs-image/src/decode/ pillow-rs-image/src/encode/
git commit -m "feat: format detection, unified decode/encode API with dispatch"
```

---

### Task 4: JPEG Decoder — IDCT Core (already implemented)

**Files:**
- Modify: `pillow-rs-image/src/jpeg.rs` (move to decode/jpeg.rs, already has IJG IDCT)

- [ ] **Step 1: Move existing IDCT to decode module**

The IJG DCT_ISLOW IDCT in `pillow-rs-image/src/jpeg.rs` is already complete with:
- 12 IJG fixed-point constants (CONST_BITS=13)
- `jpeg_idct_islow()` function — two-pass 8×8 IDCT
- `mpy()`, `descale()`, `range_limit()` helpers
- Working unit test (`test_idct_dc_only`)

Move this file to `pillow-rs-image/src/decode/jpeg.rs`, update imports to reference `crate::types::dynamic::DynamicImage`.

```bash
cp pillow-rs-image/src/jpeg.rs pillow-rs-image/src/decode/jpeg.rs
# Update imports at top of file
```

- [ ] **Step 2: Verify IDCT test passes**

```bash
cargo test -p pillow-rs-image -- jpeg
```

Expected: 1 test passes.

- [ ] **Step 3: Commit**

```bash
git add pillow-rs-image/src/decode/jpeg.rs
git rm pillow-rs-image/src/jpeg.rs 2>/dev/null
git commit -m "feat(jpeg): IJG DCT_ISLOW IDCT moved to decode module"
```

---

### Task 5: JPEG Decoder — Huffman Decoder + Header Parser

**Files:**
- Modify: `pillow-rs-image/src/decode/jpeg.rs`

- [ ] **Step 1: Add JPEG marker and header parsing**

```rust
// Append to pillow-rs-image/src/decode/jpeg.rs

// JPEG marker constants
const M_SOF0: u8 = 0xC0;  // Baseline DCT
const M_SOF1: u8 = 0xC1;  // Extended sequential DCT
const M_SOF2: u8 = 0xC2;  // Progressive DCT
const M_DHT:  u8 = 0xC4;  // Huffman table
const M_DQT:  u8 = 0xDB;  // Quantization table
const M_DRI:  u8 = 0xDD;  // Restart interval
const M_SOS:  u8 = 0xDA;  // Start of scan
const M_APP0: u8 = 0xE0;  // JFIF
const M_APP1: u8 = 0xE1;  // EXIF
const M_COM:  u8 = 0xFE;  // Comment
const M_EOI:  u8 = 0xD9;  // End of image

struct JpegDecoder<'a> {
    data: &'a [u8],
    pos: usize,
    width: u16,
    height: u16,
    num_components: u8,
    qt: [[u16; 64]; 4],        // Quantization tables (max 4)
    huff_dc: [Option<HuffTable>; 4],  // DC Huffman tables
    huff_ac: [Option<HuffTable>; 4],  // AC Huffman tables
    restart_interval: u16,
    components: [ComponentInfo; 4],
}

struct HuffTable {
    bits: [u8; 16],    // Number of codes of each bit length
    huffval: [u8; 256], // Values ordered by code
    mincode: [i32; 16],
    maxcode: [i32; 16],
    valptr: [i32; 16],
}

impl HuffTable {
    fn new() -> Self {
        HuffTable {
            bits: [0; 16], huffval: [0; 256],
            mincode: [0; 16], maxcode: [0; 16], valptr: [0; 16],
        }
    }
}

#[derive(Clone, Copy, Default)]
struct ComponentInfo {
    id: u8,
    h_sampling: u8,
    v_sampling: u8,
    qt_index: u8,
    dc_table: u8,
    ac_table: u8,
}
```

- [ ] **Step 2: Add header parsing logic**

```rust
impl<'a> JpegDecoder<'a> {
    fn new(data: &'a [u8]) -> Self {
        JpegDecoder {
            data, pos: 0, width: 0, height: 0, num_components: 0,
            qt: [[0; 64]; 4],
            huff_dc: [None, None, None, None],
            huff_ac: [None, None, None, None],
            restart_interval: 0,
            components: [ComponentInfo::default(); 4],
        }
    }

    fn read_u8(&mut self) -> u8 {
        let v = self.data[self.pos]; self.pos += 1; v
    }

    fn read_u16(&mut self) -> u16 {
        let hi = self.read_u8() as u16;
        let lo = self.read_u8() as u16;
        (hi << 8) | lo
    }

    fn skip(&mut self, n: usize) { self.pos += n; }

    /// Parse JPEG markers and tables. Returns false on error.
    fn parse_markers(&mut self) -> bool {
        // Check SOI
        if self.read_u8() != 0xFF || self.read_u8() != 0xD8 { return false; }
        loop {
            if self.pos + 1 >= self.data.len() { return false; }
            if self.read_u8() != 0xFF { return false; }
            let marker = self.read_u8();
            match marker {
                0xD8 => return false, // SOI inside stream — corrupt
                0xD9 => break,        // EOI — end of header scan
                M_SOF0 | M_SOF1 => {
                    let _len = self.read_u16();
                    let _precision = self.read_u8();
                    self.height = self.read_u16();
                    self.width = self.read_u16();
                    self.num_components = self.read_u8();
                    for i in 0..self.num_components as usize {
                        self.components[i].id = self.read_u8();
                        let sampling = self.read_u8();
                        self.components[i].h_sampling = sampling >> 4;
                        self.components[i].v_sampling = sampling & 0x0F;
                        self.components[i].qt_index = self.read_u8();
                    }
                }
                M_DQT => {
                    let len = self.read_u16() as usize;
                    let end = self.pos + len - 2;
                    while self.pos < end {
                        let info = self.read_u8();
                        let precision = info >> 4;
                        let table_idx = (info & 0x0F) as usize;
                        if table_idx < 4 {
                            for i in 0..64 {
                                self.qt[table_idx][i] = if precision == 0 {
                                    self.read_u8() as u16
                                } else {
                                    self.read_u16()
                                };
                            }
                        }
                    }
                }
                M_DHT => {
                    let len = self.read_u16() as usize;
                    let end = self.pos + len - 2;
                    while self.pos < end {
                        let info = self.read_u8();
                        let is_ac = (info >> 4) != 0;
                        let table_idx = (info & 0x0F) as usize;
                        if table_idx < 4 {
                            let mut ht = HuffTable::new();
                            let mut count = 0usize;
                            for i in 0..16 { ht.bits[i] = self.read_u8(); count += ht.bits[i] as usize; }
                            for i in 0..count { if i < 256 { ht.huffval[i] = self.read_u8(); } }
                            // Build decode tables
                            Self::build_huff_table(&mut ht);
                            if is_ac { self.huff_ac[table_idx] = Some(ht); }
                            else     { self.huff_dc[table_idx] = Some(ht); }
                        }
                    }
                }
                M_DRI => { let _len = self.read_u16(); self.restart_interval = self.read_u16(); }
                M_APP0 | M_APP1 | M_COM => {
                    let len = self.read_u16() as usize;
                    if len >= 2 { self.skip(len - 2); }
                }
                _ if marker >= 0xE0 => {
                    let len = self.read_u16() as usize;
                    if len >= 2 { self.skip(len - 2); }
                }
                _ => { /* Unknown marker, try to skip */ let len = self.read_u16() as usize; if len >= 2 { self.skip(len - 2); } }
            }
        }
        true
    }

    fn build_huff_table(ht: &mut HuffTable) {
        let mut p: i32 = 0;
        for l in 1..=16 {
            if ht.bits[l - 1] != 0 {
                ht.valptr[l - 1] = p;
                ht.mincode[l - 1] = p << (16 - l);
                p += ht.bits[l - 1] as i32;
                ht.maxcode[l - 1] = ((p - 1) << (16 - l)) | ((1 << (16 - l)) - 1);
            } else {
                ht.valptr[l - 1] = -1;
                ht.mincode[l - 1] = -1;
                ht.maxcode[l - 1] = -1;
            }
            p <<= 1;
        }
    }
}
```

- [ ] **Step 3: Add bit reader for Huffman decoding**

```rust
struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    bits_left: u8,
    bit_buf: u32,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8], pos: usize) -> Self {
        BitReader { data, pos, bits_left: 0, bit_buf: 0 }
    }

    fn read_bit(&mut self) -> Option<u32> {
        if self.bits_left == 0 {
            if self.pos >= self.data.len() { return None; }
            // Check for stuffed byte (0xFF followed by 0x00)
            if self.data[self.pos] == 0xFF && self.pos + 1 < self.data.len() && self.data[self.pos + 1] == 0x00 {
                self.pos += 1; // Skip stuffed zero
            }
            self.bit_buf = self.data[self.pos] as u32;
            self.pos += 1;
            self.bits_left = 8;
        }
        self.bits_left -= 1;
        let bit = (self.bit_buf >> self.bits_left) & 1;
        Some(bit)
    }

    fn read_bits(&mut self, n: u8) -> Option<u32> {
        let mut v: u32 = 0;
        for _ in 0..n {
            v = (v << 1) | self.read_bit()?;
        }
        Some(v)
    }

    fn decode_huffman(&mut self, ht: &HuffTable) -> Option<u8> {
        let mut code: u32 = 0;
        for l in 1..=16 {
            code = (code << 1) | self.read_bit()?;
            if code <= ht.maxcode[l - 1] as u32 {
                let diff = (code as i32 - ht.mincode[l - 1]) as usize;
                let idx = ht.valptr[l - 1] as usize + diff;
                if idx < 256 {
                    return Some(ht.huffval[idx]);
                }
            }
        }
        None
    }
}
```

- [ ] **Step 4: Build and test**

```bash
cargo build -p pillow-rs-image
```

Expected: Compiles with Huffman + header parser.

- [ ] **Step 5: Commit**

```bash
git add pillow-rs-image/src/decode/jpeg.rs
git commit -m "feat(jpeg): Huffman decoder, marker parser, bit reader"
```

---

### Task 6: JPEG Decoder — Full Decode Pipeline + First Test

**Files:**
- Modify: `pillow-rs-image/src/decode/jpeg.rs`
- Create: `pillow-rs-image/test-assets/input/jpeg/baseline.jpg` (small 8×8 JPEG)

Due to length, remaining tasks (JPEG decode pipeline, other format decoders, encoders, integration with pillow-rs) continue in the same pattern. Each task: write failing test → implement → verify → commit.

### Task 7: Reference Generation Script

**Files:**
- Create: `pillow-rs-image/scripts/generate_decode_refs.py`

Following the existing project pattern (`scripts/generate_fixtures.py`), this script:
1. Reads `manifest.yaml` for all test cases
2. For each case, loads the test asset via PIL (libjpeg/libpng)
3. Extracts raw pixels via `image.tobytes()`
4. Hashes with SHA-256
5. Writes output JSON fixture + raw .bin reference file

```python
#!/usr/bin/env python3
"""Generate decode reference fixtures from PIL (libjpeg/libpng/etc.).
Mirrors scripts/generate_fixtures.py pattern: input asset → PIL decode → output fixture.
"""
import json, hashlib, yaml
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).parent.parent
MANIFEST = ROOT / "manifest.yaml"
ASSETS_DIR = ROOT / "test-assets" / "input"
REFS_DIR = ROOT / "test-assets" / "reference"
FIXTURES_DIR = ROOT / "tests" / "fixtures"

def generate():
    FIXTURES_DIR.mkdir(parents=True, exist_ok=True)
    manifest = yaml.safe_load(MANIFEST.read_text())

    for fmt_name, fmt_data in manifest["formats"].items():
        input_dir = ASSETS_DIR / fmt_name
        ref_dir = REFS_DIR / fmt_name
        ref_dir.mkdir(parents=True, exist_ok=True)

        cases = []
        for case in fmt_data["edge_cases"]:
            for asset in case.get("test_assets", []):
                img_path = input_dir / asset
                if not img_path.exists():
                    print(f"  SKIP {asset} (file not found)")
                    continue

                img = Image.open(img_path)
                raw = img.tobytes()
                sha = hashlib.sha256(raw).hexdigest()

                ref_path = ref_dir / f"{Path(asset).stem}.bin"
                ref_path.write_bytes(raw)

                expect_error = case.get("expect_error", False)
                cases.append({
                    "id": f"{fmt_name}_{case['id']}",
                    "asset": f"{fmt_name}/{asset}",
                    "reference": f"{fmt_name}/{Path(asset).stem}.bin",
                    "sha256": sha,
                    "mode": img.mode,
                    "size": list(img.size),
                    "expect_error": expect_error,
                })

        # Write fixture JSON (same pattern as other pillow-rs fixtures)
        fixture = {
            "format_version": 2,
            "operation": {"module": "Decode", "target": fmt_name},
            "cases": cases,
        }
        out_path = FIXTURES_DIR / f"Decode.{fmt_name}.json"
        out_path.write_text(json.dumps(fixture, indent=2) + "\n")

    print(f"Generated {sum(1 for _ in FIXTURES_DIR.glob('*.json'))} fixture files")

if __name__ == "__main__":
    generate()
```

- [ ] **Step 1: Run the script**

```bash
cd pillow-rs-image && python3 scripts/generate_decode_refs.py
```

Expected: Creates `tests/fixtures/Decode.{jpeg,png,...}.json` and `test-assets/reference/**/*.bin`

- [ ] **Step 2: Commit**

```bash
git add pillow-rs-image/scripts/ pillow-rs-image/tests/fixtures/
git commit -m "feat: reference generation script + decode fixtures"
```

---

### Task 8: Rust Test Runner — Fixture-Based Decode Tests

**Files:**
- Create: `pillow-rs-image/tests/decode_fixture_tests.rs`

Following the project's parametrized fixture pattern (`tests/test_parity.py`), each fixture JSON file produces multiple test cases, one per edge case.

```rust
// pillow-rs-image/tests/decode_fixture_tests.rs
use pillow_rs_image as img;
use std::path::Path;

/// Run all decode fixture tests. Each test: load asset → decode → compare SHA-256
/// against reference from PIL (libjpeg/libpng/etc.)
#[test]
fn test_decode_fixtures() {
    let fixtures_dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures"));
    let assets_dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/test-assets"));

    for entry in std::fs::read_dir(fixtures_dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().map_or(true, |e| e != "json") { continue; }

        let fixture: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let cases = fixture["cases"].as_array().unwrap();

        for case in cases {
            let cid = case["id"].as_str().unwrap();
            let asset = case["asset"].as_str().unwrap();
            let expect_error = case["expect_error"].as_bool().unwrap_or(false);

            let asset_path = assets_dir.join("input").join(asset);
            let data = std::fs::read(&asset_path)
                .unwrap_or_else(|_| panic!("missing asset: {}", asset));

            let result = img::decode(&data);

            if expect_error {
                assert!(result.is_none(), "[{}] expected decode error, got success", cid);
            } else {
                let decoded = result.unwrap_or_else(|| panic!("[{}] decode returned None", cid));
                let ref_path = assets_dir.join("reference")
                    .join(case["reference"].as_str().unwrap());
                let expected = std::fs::read(&ref_path)
                    .unwrap_or_else(|_| panic!("[{}] missing reference", cid));

                assert_eq!(
                    decoded.as_bytes(), expected.as_slice(),
                    "[{}] pixel mismatch — decoded {} bytes, expected {} bytes",
                    cid, decoded.as_bytes().len(), expected.len()
                );
            }
        }
    }
}
```

- [ ] **Step 1: Add test dependencies**

```toml
# pillow-rs-image/Cargo.toml
[dev-dependencies]
serde_json = "1"
```

- [ ] **Step 2: Build and verify test framework compiles**

```bash
cargo test -p pillow-rs-image -- decode_fixture --no-run
```

- [ ] **Step 3: Commit**

```bash
git add pillow-rs-image/tests/ pillow-rs-image/Cargo.toml
git commit -m "test: fixture-based decode test runner"
```

---

### Remaining Tasks Summary

| Task | Component | Priority |
|------|-----------|----------|
| 9 | JPEG: YCbCr→RGB + upsample + full decode pipeline | P0 |
| 10 | JPEG: 30 reference tests passing | P0 |
| 11 | PNG: wire `png` crate, 27 reference tests | P0 |
| 12 | GIF: wire `gif` crate, 9 reference tests | P1 |
| 13 | BMP: direct implementation, 15 reference tests | P1 |
| 14 | TIFF: wire `tiff` crate, 18 reference tests | P1 |
| 15 | WebP: own VP8 decoder, 12 reference tests | P1 |
| 16 | ICO: wrap BMP/PNG, 6 reference tests | P1 |
| 17 | AVIF: feature-gated `zenavif`, 6 reference tests | P2 |
| 18 | Encode: all format encoders | P1 |
| 19 | Integration: replace `image` crate in pillow-rs | P0 |
| 20 | Full parity test suite (778+ tests) | P0 |
| 21 | WASM build verification | P2 |

---

## Phase Summary

| Phase | Tasks | Deliverable |
|-------|-------|-------------|
| 1: Types + JPEG | 1-8 | Types, JPEG decode, 30 tests passing |
| 2: Other formats | 9-15 | PNG, GIF, BMP, TIFF, WebP, ICO, AVIF decode |
| 3: Encode + Integration | 16-18 | All encoders, `image` crate removed |
| 4: Full validation | 19-20 | 778+ tests pass, WASM verified |
