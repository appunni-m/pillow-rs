# pillow-rs-image: Zero-Dependency Pixel-Perfect Image Codec Crate

**Status:** Approved | **Date:** 2026-06-17

## 1. Motivation

pillow-rs currently depends on the `image` crate (v0.25) which uses `zune-jpeg` for JPEG decoding. `zune-jpeg` explicitly states pixel-identical output with libjpeg is a **non-goal**. This causes JPEG-loaded reference images to differ by ±1-3 pixels from PIL's libjpeg output, breaking parity tests for EVERY operation that starts from a loaded image.

Additionally, `zune-jpeg` cannot be used in WASM (relies on platform-specific SIMD intrinsics). We need a pure-Rust, WASM-compatible, pixel-perfect decoder.

## 2. Goals

| Goal | Detail |
|------|--------|
| Remove `image` crate dependency | Own all types, decode, encode |
| Pixel-perfect JPEG output | IJG `DCT_ISLOW` IDCT matching libjpeg exactly |
| Pixel-perfect PNG output | Deflate (miniz_oxide) + PNG filter matching libpng |
| Zero C dependency | Works on WASM, pure Rust throughout |
| MIT license | All deps MIT-compatible |
| 100+ tests per format | Verify against libjpeg/libpng reference output |
| Backward-compatible API | Same `DynamicImage`, `GrayImage`, `RgbImage`, `RgbaImage` types |

## 3. Architecture

```
pillow-rs-image/
├── Cargo.toml           Zero external C deps, only pure-Rust MIT crates
├── src/
│   ├── lib.rs           detect_format(), decode(), encode(), ColorType, ImageFormat
│   ├── types/
│   │   ├── mod.rs       Re-exports
│   │   ├── traits.rs    Pixel, GenericImageView traits (matches image crate API)
│   │   ├── buffer.rs    ImageBuffer<P, Container>
│   │   ├── dynamic.rs   DynamicImage enum
│   │   └── color.rs     Luma, Rgb, Rgba, LumaA pixel types
│   ├── decode/
│   │   ├── mod.rs       Unified decode(&[u8]) → Result<DynamicImage>
│   │   ├── jpeg.rs      IJG DCT_ISLOW + Huffman + YCbCr→RGB (own impl)
│   │   ├── png.rs       Wrap `png` crate, verify libpng parity
│   │   ├── gif.rs       Wrap `gif` crate
│   │   ├── bmp.rs       Direct decoder (~100 LOC)
│   │   ├── tiff.rs      Wrap `tiff` crate with miniz_oxide backend
│   │   ├── webp.rs      Own VP8/VP8L decode (~2k LOC)
│   │   ├── ico.rs       Wrap BMP/PNG decode
│   │   └── avif.rs      Wrap `zenavif` (rav1d-based, pure Rust)
│   ├── encode/
│   │   ├── mod.rs       Unified encode(&DynamicImage, ImageFormat) → Vec<u8>
│   │   ├── jpeg.rs      Wrap `jpeg-encoder` or own baseline DCT
│   │   ├── png.rs       Wrap `png` crate encoder
│   │   ├── gif.rs       Wrap `gif` crate encoder
│   │   ├── bmp.rs       Direct encoder (~100 LOC)
│   │   ├── tiff.rs      Wrap `tiff` crate encoder
│   │   ├── webp.rs      Own VP8 encode
│   │   ├── ico.rs       Wrap BMP/PNG encode
│   │   └── avif.rs      Wrap `ravif` encoder
│   └── tests/
│       ├── jpeg/        libjpeg reference images + pixel comparison
│       ├── png/         libpng reference images
│       ├── gif/         reference GIFs
│       ├── bmp/         reference BMPs
│       ├── tiff/        reference TIFFs
│       ├── webp/        reference WebPs
│       └── avif/        reference AVIFs
├── test-assets/         Committed test images (small, public domain)
└── README.md
```

## 4. Type System

### 4.1 Types owned by pillow-rs-image (matching image crate API)

```rust
// Pixel types
pub struct Luma<T>(pub [T; 1]);       // grayscale pixel
pub struct LumaA<T>(pub [T; 2]);      // grayscale + alpha
pub struct Rgb<T>(pub [T; 3]);        // RGB pixel
pub struct Rgba<T>(pub [T; 4]);       // RGBA pixel

// Image buffer — same as image::ImageBuffer
pub struct ImageBuffer<P, Container> {
    width: u32,
    height: u32,
    _phantom: PhantomData<P>,
    data: Container,
}

// Concrete type aliases
pub type GrayImage = ImageBuffer<Luma<u8>, Vec<u8>>;        // 1 byte/pixel
pub type GrayAlphaImage = ImageBuffer<LumaA<u8>, Vec<u8>>;  // 2 bytes/pixel
pub type RgbImage = ImageBuffer<Rgb<u8>, Vec<u8>>;          // 3 bytes/pixel
pub type RgbaImage = ImageBuffer<Rgba<u8>, Vec<u8>>;        // 4 bytes/pixel

// Dynamic image enum
pub enum DynamicImage {
    ImageLuma8(GrayImage),
    ImageLumaA8(GrayAlphaImage),
    ImageRgb8(RgbImage),
    ImageRgba8(RgbaImage),
    // ... other variants as needed
}

// Trait — same as image::GenericImageView
pub trait GenericImageView {
    type Pixel: Pixel;
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn dimensions(&self) -> (u32, u32);
    fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel;
    fn pixels(&self) -> Pixels<Self>;
}

// Pixel trait
pub trait Pixel: Copy + Clone {
    type Subpixel: Primitive;
    const CHANNEL_COUNT: u8;
    fn channels(&self) -> &[Self::Subpixel];
    fn from_slice(slice: &[Self::Subpixel]) -> &Self;
}
```

### 4.2 Color types

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorType {
    L8, La8, Rgb8, Rgba8,
    L16, La16, Rgb16, Rgba16,
}

impl ColorType {
    pub fn bits_per_pixel(&self) -> u16 { .. }
    pub fn channels(&self) -> u8 { .. }
}
```

## 5. Format Coverage

### 5.1 Decode (pixel-perfect with C libraries)

| Format | Approach | Test Strategy |
|--------|----------|---------------|
| JPEG | Own IJG DCT_ISLOW IDCT + Huffman decoder | 100+ libjpeg reference images, per-pixel comparison |
| PNG | `png` crate (miniz_oxide backend) | libpng reference images, all color types + interlaced |
| GIF | `gif` crate (weezl LZW) | Reference GIFs, animated + static |
| BMP | Direct impl | All BMP subtypes (1/4/8/16/24/32 bit) |
| TIFF | `tiff` crate (miniz_oxide, no C) | Baseline TIFFs, LZW + deflate compression |
| WebP | Own VP8/VP8L decoder | libwebp reference images |
| ICO | Wrap BMP/PNG | Standard ICO files |
| AVIF | `zenavif` (rav1d, optional feature) | Reference AVIFs |

### 5.2 Encode

| Format | Approach |
|--------|----------|
| JPEG | `jpeg-encoder` crate (pure Rust) or own baseline DCT |
| PNG | `png` crate encoder (miniz_oxide deflate) |
| GIF | `gif` crate encoder (weezl LZW) |
| BMP | Direct impl |
| TIFF | `tiff` crate encoder |
| WebP | Own VP8 encoder |
| ICO | Wrap BMP/PNG encode |
| AVIF | `ravif` encoder (optional feature) |

## 6. JPEG IDCT Implementation

### 6.1 Algorithm

IJG `DCT_ISLOW` ("slow-but-accurate") from `jidctint.c`:

- **CONST_BITS** = 13 (fixed-point precision)
- **PASS1_BITS** = 2 (column pass extra precision)
- **12 multiplies, 32 adds** per 1-D IDCT
- **FIX(c)** = (i32)(c * (1 << CONST_BITS) + 0.5)
- **Scaling**: √2 factors absorbed into constants, final right-shift corrects

### 6.2 Key constants (verified against jidctint.c)

```
FIX_0_298631336 = 2446    FIX_0_390180644 = 3196
FIX_0_541196100 = 4433    FIX_0_765366865 = 6270
FIX_0_899976223 = 7373    FIX_1_175875602 = 9633
FIX_1_501321110 = 12299   FIX_1_847759065 = 15137
FIX_1_961570560 = 16069   FIX_2_053119869 = 16819
FIX_2_562915447 = 20995   FIX_3_072711026 = 25172
```

### 6.3 Decode pipeline

```
JPEG bytes → Parse markers (SOI, DQT, SOF0, DHT, SOS)
          → Entropy decode (Huffman)
          → Dequantize 8×8 blocks
          → IDCT (jpeg_idct_islow) per block
          → YCbCr → RGB (matching libjpeg jdcolor.c)
          → Upsample chroma for 4:2:0
          → DynamicImage
```

## 7. Testing Strategy

### 7.1 Per-format tests

Each format gets a dedicated test directory with:
- `test-assets/` — small reference images (public domain, committed to repo)
- Generated reference pixels from C libraries (libjpeg, libpng, etc.)
- Per-pixel comparison against reference

### 7.2 Reference generation

```bash
# Generate JPEG reference from libjpeg
python3 -c "
from PIL import Image
import numpy as np
img = Image.open('test.jpg')
pixels = np.array(img).tobytes()
open('reference.bin', 'wb').write(pixels)
"
```

### 7.3 Test pattern

```rust
#[test]
fn test_jpeg_decode_baseline() {
    let data = include_bytes!("../test-assets/baseline.jpg");
    let ref_bytes = include_bytes!("../test-assets/baseline_ref.bin");
    let decoded = pillow_rs_decode::decode(data).unwrap();
    assert_eq!(decoded.as_bytes(), ref_bytes);
}
```

### 7.4 Coverage targets

- 100+ test images per format
- All color subsampling modes (JPEG: 4:4:4, 4:2:2, 4:2:0)
- All color types (PNG: L, LA, RGB, RGBA, indexed, grayscale-alpha)
- Interlaced vs progressive
- Edge cases: 1×1, maximum dimensions, corrupt input

## 8. Integration with pillow-rs-core

### 8.1 Changes required

```diff
# Cargo.toml
- image = { version = "0.25", ... }
+ pillow-rs-image = { path = "../pillow-rs-image" }

# All files
- use image::DynamicImage;
+ use pillow_rs_decode::DynamicImage;
- use image::GrayImage;
+ use pillow_rs_decode::GrayImage;
- use image::RgbImage;
+ use pillow_rs_decode::RgbImage;
- use image::RgbaImage;
+ use pillow_rs_decode::RgbaImage;
- use image::GenericImageView;
+ use pillow_rs_decode::GenericImageView;
```

### 8.2 Materialize path changes

```rust
// Before (image.rs)
Image::Path { path, .. } => {
    let img = image::open(path).map_err(PilError::ImageError)?;
    Ok(img)
}

// After
Image::Path { path, .. } => {
    let data = std::fs::read(path)?;
    let decoded = pillow_rs_decode::decode(&data)
        .ok_or(PilError::DecodeError("unrecognized format".into()))?;
    Ok(decoded)
}
```

### 8.3 Save path changes

```rust
// Before
img.save_with_format(path, format)

// After
let bytes = pillow_rs_decode::encode(&dynamic_image, format)?;
std::fs::write(path, bytes)?;
```

## 9. Dependency Tree

```
pillow-rs-image
├── (none)              JPEG, BMP, ICO, WebP — own impl
├── png = "0.18"        PNG decode/encode (MIT)
│   └── miniz_oxide     Pure Rust deflate
├── gif = "0.13"        GIF decode/encode (MIT)
│   └── weezl           Pure Rust LZW
├── tiff = "0.9"        TIFF decode/encode (MIT)
│   ├── miniz_oxide     Pure Rust deflate (via flate2 rust_backend)
│   └── weezl           Pure Rust LZW
├── jpeg-encoder = "0.6" JPEG encode (MIT, optional)
├── zenavif = "0.1"     AVIF decode (MIT, optional, feature-gated)
│   └── rav1d           Pure Rust AV1 decoder
└── ravif = "0.11"      AVIF encode (MIT, optional, feature-gated)
    └── rav1e           Pure Rust AV1 encoder

Zero C dependencies. All MIT or MIT-compatible.
```

## 10. Implementation Phases

### Phase 1: Types + JPEG decode (week 1)
- Implement all types in `pillow-rs-image/src/types/`
- Complete JPEG decoder: Huffman + IDCT + YCbCr→RGB
- 100+ JPEG tests against libjpeg reference
- Replace `image::open` in pillow-rs-core

### Phase 2: PNG + GIF + BMP (week 2)
- Wire up `png`, `gif` crates
- Direct BMP implementation
- Tests for each format
- Replace remaining image crate usage

### Phase 3: TIFF + WebP + ICO + AVIF (week 3)
- Wire up `tiff` crate
- Own WebP VP8 decoder
- ICO wrapper, AVIF feature
- Encode pipeline for all formats

### Phase 4: Encode + full integration (week 4)
- Complete encode for all formats
- Remove `image` crate from Cargo.toml
- Full parity test suite passes with pillow-rs-image
- WASM build verified

## 11. Success Criteria

- [ ] `image` crate removed from pillow-rs-core Cargo.toml
- [ ] All 778 suite0 tests pass with new decoder
- [ ] All 1148 total tests pass (suite0 + suite1)
- [ ] 100+ tests per format in pillow-rs-image
- [ ] WASM build succeeds (`wasm-pack build` in pillow-rs-js)
- [ ] Pixel-perfect JPEG output verified against libjpeg
- [ ] Zero C dependencies in dependency tree
- [ ] All dependencies MIT licensed

## ADDENDUM: Test Architecture & manifest.yaml

### manifest.yaml — Single Source of Truth

`pillow-rs-image/manifest.yaml` lists every format, every edge case, and
every test asset. Tests are auto-discovered from the manifest. See the file
for the complete catalog (165+ edge cases across 8 formats).

### Test pattern

```
Input: test-assets/input/jpeg/baseline.jpg  (committed .jpg)
Reference: test-assets/reference/jpeg/baseline.bin  (pre-generated from libjpeg)
Test: decode(input) → assert_eq!(pixels, reference)
```

### Reference generation

`scripts/generate_decode_refs.py` — reads manifest, opens each asset via PIL
(libjpeg/libpng), calls `image.tobytes()` for raw pixels, writes `.bin` files.

### Coverage

| Format | Edge Cases | Test Files |
|--------|-----------|------------|
| JPEG   | 30        | 30+        |
| PNG    | 27        | 27+        |
| GIF    | 9         | 9+         |
| BMP    | 15        | 15+        |
| TIFF   | 18        | 18+        |
| WebP   | 12        | 12+        |
| ICO    | 6         | 6+         |
| AVIF   | 6         | 6+         |
| Cross  | 5         | 5+         |
| **Total** | **128+** | **128+** |
