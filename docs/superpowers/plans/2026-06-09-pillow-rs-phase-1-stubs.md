# pillow-rs Phase 1: Stub Everything — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create the full workspace with every Pillow function exposed as a stub, working Python package (`from RSPIL import Image`), working WASM build, manifest-driven coverage reporting, test infrastructure, and CI pipeline. All functions exist with correct signatures but raise `NotImplementedError` until Phase 2.

**Architecture:** Workspace with three crates — `pillow-rs` (pure Rust, all image logic, zero binding deps), `pillow-rs-py` (PyO3 thin wrappers), `pillow-rs-js` (wasm-bindgen thin wrappers). `manifest.yaml` is the single source of truth defining every Pillow function, its signature, supported modes, parameter variants, and edge cases. Tests drive coverage computation.

**Tech Stack:** Rust 2021 edition, `image` crate 0.25, `pyo3` 0.24, `wasm-bindgen` 0.2, `thiserror`, `rayon`, maturin, wasm-pack, pytest

---

### Task 1: Workspace Root Setup

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `.gitignore`
- Create: `rust-toolchain.toml`

- [ ] **Step 1: Create workspace Cargo.toml**

```toml
[workspace]
members = [
    "pillow-rs",
    "pillow-rs-py",
    "pillow-rs-js",
]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
repository = "https://github.com/user/pillow-rs"

[workspace.lints.clippy]
redundant_clone = "deny"
large_enum_variant = "deny"
needless_collect = "deny"
unwrap_used = "deny"
expect_used = "deny"

[workspace.profile.release]
lto = true
codegen-units = 1
opt-level = 3
panic = "abort"

[workspace.profile.dev]
opt-level = 1
```

- [ ] **Step 2: Create .gitignore**

```
/target/
**/*.rs.bk
*.pyc
__pycache__/
.pytest_cache/
dist/
*.so
*.dylib
*.dll
*.wasm
/pkg/
/node_modules/
coverage/report.json
.python-version
venv/
.venv/
```

- [ ] **Step 3: Create rust-toolchain.toml**

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
targets = ["wasm32-unknown-unknown"]
```

- [ ] **Step 4: Verify workspace**

Run: `cargo check 2>&1 || true`
Expected: "no members defined in the manifest" is fine — we'll add crates next.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml .gitignore rust-toolchain.toml
git commit -m "feat: initialize cargo workspace"
```

---

### Task 2: pillow-rs Crate Scaffolding

**Files:**
- Create: `pillow-rs/Cargo.toml`
- Create: `pillow-rs/src/lib.rs`
- Create: `pillow-rs/src/error.rs`

- [ ] **Step 1: Create pillow-rs/Cargo.toml**

```toml
[package]
name = "pillow-rs"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Pure Rust image processing — Pillow reimplementation core"

[lib]
name = "pillow_rs"

[dependencies]
image = { version = "0.25", default-features = false, features = ["jpeg", "png", "gif", "bmp", "tiff", "webp"] }
thiserror = "1"
csscolorparser = "0.8"
color_quant = "1.1"

[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
rayon = "1.7"

[features]
default = []
```

- [ ] **Step 2: Create pillow-rs/src/error.rs**

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PilError {
    #[error("{0}")]
    IOError(String),

    #[error("cannot identify image file '{0}'")]
    UnidentifiedImageError(String),

    #[error("{0}")]
    ValueError(String),

    #[error("{0}")]
    TypeError(String),

    #[error("image processing error: {0}")]
    ImageError(#[from] image::ImageError),

    #[error("{0}")]
    NotImplementedError(String),

    #[error("unknown format: {0}")]
    UnknownFormat(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

- [ ] **Step 3: Create pillow-rs/src/lib.rs (minimal)**

```rust
pub mod error;
pub use error::PilError;
```

- [ ] **Step 4: Build and test core**

Run: `cargo build -p pillow-rs`
Expected: Compiles successfully.

Run: `cargo test -p pillow-rs`
Expected: 0 tests run, OK.

- [ ] **Step 5: Commit**

```bash
git add pillow-rs/
git commit -m "feat: scaffold pillow-rs with error types"
```

---

### Task 3: Core Image Types (color, lazy, image)

**Files:**
- Create: `pillow-rs/src/color.rs`
- Create: `pillow-rs/src/lazy.rs`
- Create: `pillow-rs/src/image.rs`
- Modify: `pillow-rs/src/lib.rs`

- [ ] **Step 1: Create pillow-rs/src/color.rs**

```rust
use image::ColorType;

/// Map image::ColorType to Pillow mode string
pub fn color_type_to_mode(ct: ColorType) -> &'static str {
    match ct {
        ColorType::L8 => "L",
        ColorType::La8 => "LA",
        ColorType::Rgb8 => "RGB",
        ColorType::Rgba8 => "RGBA",
        ColorType::L16 => "I;16",
        ColorType::Rgb16 => "I;16",
        ColorType::Rgba16 => "I;16",
        _ => "RGB",
    }
}

/// Parse a CSS color string, integer, or tuple into RGBA u8 values
pub fn parse_color_str(s: &str) -> Result<(u8, u8, u8, u8), crate::error::PilError> {
    let c = csscolorparser::parse(s)
        .map_err(|e| crate::error::PilError::ValueError(format!("Invalid color string: {}", e)))?;
    let rgba = c.to_rgba8();
    Ok((rgba[0], rgba[1], rgba[2], rgba[3]))
}
```

- [ ] **Step 2: Create pillow-rs/src/lazy.rs**

```rust
use image::{DynamicImage, ImageFormat};
use std::io::Cursor;
use std::path::PathBuf;

/// Deferred-load image — decode only on first operation.
/// Pattern adopted from Puhu's LazyImage.
#[derive(Clone)]
pub enum LazyImage {
    Loaded(DynamicImage),
    Path { path: PathBuf, format: Option<ImageFormat> },
    Bytes { data: Vec<u8>, format: Option<ImageFormat> },
}

impl LazyImage {
    pub fn ensure_loaded(&mut self) -> Result<&DynamicImage, crate::error::PilError> {
        match self {
            LazyImage::Loaded(img) => Ok(img),
            LazyImage::Path { path, format: _ } => {
                let img = image::open(path)
                    .map_err(|e| crate::error::PilError::ImageError(e))?;
                *self = LazyImage::Loaded(img);
                match self {
                    LazyImage::Loaded(img) => Ok(img),
                    _ => unreachable!(),
                }
            }
            LazyImage::Bytes { data, format: _ } => {
                let cursor = Cursor::new(data);
                let reader = image::io::Reader::new(cursor)
                    .with_guessed_format()
                    .map_err(|e| crate::error::PilError::Io(e))?;
                let img = reader.decode()
                    .map_err(|e| crate::error::PilError::ImageError(e))?;
                *self = LazyImage::Loaded(img);
                match self {
                    LazyImage::Loaded(img) => Ok(img),
                    _ => unreachable!(),
                }
            }
        }
    }
}
```

- [ ] **Step 3: Create pillow-rs/src/image.rs (skeleton)**

```rust
use image::{DynamicImage, ImageFormat};
use std::path::PathBuf;
use crate::color::color_type_to_mode;
use crate::error::PilError;
use crate::lazy::LazyImage;

#[derive(Clone)]
pub struct Image {
    pub(crate) inner: LazyImage,
    pub(crate) format: Option<ImageFormat>,
}

impl Image {
    pub fn new(width: u32, height: u32, mode: &str, color: (u8, u8, u8, u8)) -> Result<Self, PilError> {
        let img = match mode {
            "RGB" => DynamicImage::ImageRgb8(
                image::RgbImage::from_pixel(width, height, image::Rgb([color.0, color.1, color.2]))
            ),
            "RGBA" => DynamicImage::ImageRgba8(
                image::RgbaImage::from_pixel(width, height, image::Rgba([color.0, color.1, color.2, color.3]))
            ),
            "L" => DynamicImage::ImageLuma8(
                image::GrayImage::from_pixel(width, height, image::Luma([color.0]))
            ),
            "LA" => DynamicImage::ImageLumaA8(
                image::GrayAlphaImage::from_pixel(width, height, image::LumaA([color.0, color.3]))
            ),
            _ => return Err(PilError::ValueError(format!("Unsupported mode: {}", mode))),
        };
        Ok(Image { inner: LazyImage::Loaded(img), format: None })
    }

    pub fn open_path(path: &str) -> Result<Self, PilError> {
        let path_buf = PathBuf::from(path);
        let format = ImageFormat::from_path(&path_buf).ok();
        Ok(Image { inner: LazyImage::Path { path: path_buf, format }, format })
    }

    pub fn open_bytes(data: Vec<u8>) -> Result<Self, PilError> {
        let format = {
            let cursor = std::io::Cursor::new(&data);
            image::io::Reader::new(cursor)
                .with_guessed_format()
                .ok()
                .and_then(|r| r.format())
        };
        Ok(Image { inner: LazyImage::Bytes { data, format }, format })
    }

    pub fn ensure_loaded(&mut self) -> Result<&DynamicImage, PilError> {
        self.inner.ensure_loaded()
    }

    pub fn size(&mut self) -> Result<(u32, u32), PilError> {
        let img = self.ensure_loaded()?;
        Ok((img.width(), img.height()))
    }

    pub fn mode(&mut self) -> Result<String, PilError> {
        let img = self.ensure_loaded()?;
        Ok(color_type_to_mode(img.color()).to_string())
    }

    pub fn format_name(&self) -> Option<String> {
        self.format.map(|f| format!("{:?}", f).to_uppercase())
    }

    pub fn to_bytes(&mut self) -> Result<Vec<u8>, PilError> {
        let img = self.ensure_loaded()?;
        Ok(img.as_bytes().to_vec())
    }

    pub fn copy(&self) -> Self {
        self.clone()
    }
}
```

- [ ] **Step 4: Update pillow-rs/src/lib.rs**

```rust
pub mod error;
pub mod color;
pub mod lazy;
pub mod image;
pub mod ops;
pub mod formats;

pub use error::PilError;
pub use image::Image;
```

- [ ] **Step 5: Create placeholder mod files**

```bash
mkdir -p pillow-rs/src/ops pillow-rs/src/formats pillow-rs/src/draw pillow-rs/src/font
```

Create `pillow-rs/src/ops/mod.rs`:
```rust
pub mod resize;
pub mod crop;
pub mod rotate;
pub mod transpose;
pub mod convert;
pub mod paste;
pub mod split;
pub mod filter;
pub mod enhance;
```

Create `pillow-rs/src/formats/mod.rs`:
```rust
// Format handling stubs — implemented in Phase 2
```

- [ ] **Step 6: Build and verify**

Run: `cargo build -p pillow-rs`
Expected: Compiles successfully.

- [ ] **Step 7: Commit**

```bash
git add pillow-rs/
git commit -m "feat: core image types — Image, LazyImage, color parsing"
```

---

### Task 4: Core Operation Stubs

**Files:**
- Create: `pillow-rs/src/ops/resize.rs`
- Create: `pillow-rs/src/ops/crop.rs`
- Create: `pillow-rs/src/ops/rotate.rs`
- Create: `pillow-rs/src/ops/transpose.rs`
- Create: `pillow-rs/src/ops/convert.rs`
- Create: `pillow-rs/src/ops/paste.rs`
- Create: `pillow-rs/src/ops/split.rs`
- Create: `pillow-rs/src/ops/filter.rs`
- Create: `pillow-rs/src/ops/enhance.rs`
- Modify: `pillow-rs/src/image.rs` (add delegation methods)

- [ ] **Step 1: Create resize stub**

`pillow-rs/src/ops/resize.rs`:
```rust
use crate::error::PilError;
use crate::image::Image;

pub enum ResampleFilter {
    Nearest,
    Bilinear,
    Bicubic,
    Lanczos,
}

impl ResampleFilter {
    pub fn from_str(s: Option<&str>) -> Result<image::imageops::FilterType, PilError> {
        match s {
            None | Some("BILINEAR") | Some("bilinear") => Ok(image::imageops::FilterType::Triangle),
            Some("NEAREST") | Some("nearest") => Ok(image::imageops::FilterType::Nearest),
            Some("BICUBIC") | Some("bicubic") => Ok(image::imageops::FilterType::CatmullRom),
            Some("LANCZOS") | Some("lanczos") => Ok(image::imageops::FilterType::Lanczos3),
            Some(other) => Err(PilError::ValueError(format!("Unknown resample filter: {}", other))),
        }
    }
}

impl Image {
    pub fn resize(&self, _size: (u32, u32), _filter: Option<&str>) -> Result<Image, PilError> {
        Err(PilError::NotImplementedError("Image.resize".into()))
    }
}
```

- [ ] **Step 2: Create crop stub**

`pillow-rs/src/ops/crop.rs`:
```rust
use crate::error::PilError;
use crate::image::Image;

impl Image {
    /// box_coords: (left, top, right, bottom) — Pillow-style
    pub fn crop(&self, _box_coords: (u32, u32, u32, u32)) -> Result<Image, PilError> {
        Err(PilError::NotImplementedError("Image.crop".into()))
    }
}
```

- [ ] **Step 3: Create rotate stub**

`pillow-rs/src/ops/rotate.rs`:
```rust
use crate::error::PilError;
use crate::image::Image;

impl Image {
    pub fn rotate(&self, _angle: f64, _expand: bool, _fillcolor: Option<(u8,u8,u8,u8)>) -> Result<Image, PilError> {
        Err(PilError::NotImplementedError("Image.rotate".into()))
    }
}
```

- [ ] **Step 4: Create transpose stub**

`pillow-rs/src/ops/transpose.rs`:
```rust
use crate::error::PilError;
use crate::image::Image;

impl Image {
    pub fn transpose(&self, _method: &str) -> Result<Image, PilError> {
        Err(PilError::NotImplementedError("Image.transpose".into()))
    }
}
```

- [ ] **Step 5: Create convert stub**

`pillow-rs/src/ops/convert.rs`:
```rust
use crate::error::PilError;
use crate::image::Image;

impl Image {
    pub fn convert(
        &self,
        _mode: &str,
        _matrix: Option<Vec<f64>>,
        _dither: Option<&str>,
        _palette: Option<&str>,
        _colors: Option<u32>,
    ) -> Result<Image, PilError> {
        Err(PilError::NotImplementedError("Image.convert".into()))
    }
}
```

- [ ] **Step 6: Create paste stub**

`pillow-rs/src/ops/paste.rs`:
```rust
use crate::error::PilError;
use crate::image::Image;

pub enum PasteSource {
    Image(Image),
    Color((u8, u8, u8, u8)),
}

impl Image {
    /// Paste mutates self (matching Pillow semantics)
    pub fn paste(
        &mut self,
        _source: PasteSource,
        _box_coords: Option<(i32, i32, i32, i32)>,
        _mask: Option<&Image>,
    ) -> Result<(), PilError> {
        Err(PilError::NotImplementedError("Image.paste".into()))
    }
}
```

- [ ] **Step 7: Create split stub**

`pillow-rs/src/ops/split.rs`:
```rust
use crate::error::PilError;
use crate::image::Image;

impl Image {
    pub fn split(&self) -> Result<Vec<Image>, PilError> {
        Err(PilError::NotImplementedError("Image.split".into()))
    }

    pub fn getbands(&self) -> Result<Vec<String>, PilError> {
        Err(PilError::NotImplementedError("Image.getbands".into()))
    }
}
```

- [ ] **Step 8: Create filter stub**

`pillow-rs/src/ops/filter.rs`:
```rust
use crate::error::PilError;
use crate::image::Image;

impl Image {
    pub fn filter(&self, _filter_type: &str) -> Result<Image, PilError> {
        Err(PilError::NotImplementedError("Image.filter".into()))
    }
}
```

- [ ] **Step 9: Create enhance stub**

`pillow-rs/src/ops/enhance.rs`:
```rust
use crate::error::PilError;
use crate::image::Image;

impl Image {
    pub fn enhance_brightness(&self, _factor: f64) -> Result<Image, PilError> {
        Err(PilError::NotImplementedError("ImageEnhance.Brightness".into()))
    }

    pub fn enhance_contrast(&self, _factor: f64) -> Result<Image, PilError> {
        Err(PilError::NotImplementedError("ImageEnhance.Contrast".into()))
    }

    pub fn enhance_color(&self, _factor: f64) -> Result<Image, PilError> {
        Err(PilError::NotImplementedError("ImageEnhance.Color".into()))
    }

    pub fn enhance_sharpness(&self, _factor: f64) -> Result<Image, PilError> {
        Err(PilError::NotImplementedError("ImageEnhance.Sharpness".into()))
    }
}
```

- [ ] **Step 10: Add save and thumbnail methods to Image**

`pillow-rs/src/image.rs` — append:
```rust
impl Image {
    pub fn save(&mut self, _path: &str, _format: Option<&str>) -> Result<(), PilError> {
        Err(PilError::NotImplementedError("Image.save".into()))
    }

    pub fn thumbnail(&mut self, _size: (u32, u32), _filter: Option<&str>) -> Result<(), PilError> {
        Err(PilError::NotImplementedError("Image.thumbnail".into()))
    }
}
```

- [ ] **Step 11: Build and verify all stubs compile**

Run: `cargo build -p pillow-rs`
Expected: Compiles successfully.

- [ ] **Step 12: Commit**

```bash
git add pillow-rs/src/ops/ pillow-rs/src/image.rs
git commit -m "feat: add core operation stubs (resize, crop, rotate, transpose, convert, paste, split, filter, enhance)"
```

---

### Task 5: pillow-rs-py — PyO3 Binding Crate

**Files:**
- Create: `pillow-rs-py/Cargo.toml`
- Create: `pillow-rs-py/pyproject.toml`
- Create: `pillow-rs-py/src/lib.rs`
- Create: `pillow-rs-py/python/pillow_rs/__init__.py`
- Create: `pillow-rs-py/python/pillow_rs/enums.py`
- Create: `pillow-rs-py/python/pillow_rs/image.py`
- Create: `pillow-rs-py/python/pillow_rs/operations.py`

- [ ] **Step 1: Create pillow-rs-py/Cargo.toml**

```toml
[package]
name = "pillow-rs-py"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "PyO3 bindings for pillow-rs — Pillow drop-in replacement"

[lib]
name = "_core"
crate-type = ["cdylib"]

[dependencies]
pillow-rs = { path = "../pillow-rs" }
pyo3 = { version = "0.24", features = ["extension-module", "abi3", "abi3-py38"] }
```

- [ ] **Step 2: Create pillow-rs-py/pyproject.toml**

```toml
[build-system]
requires = ["maturin>=1.0,<2.0"]
build-backend = "maturin"

[project]
name = "pillow-rs"
version = "0.1.0"
description = "Pillow drop-in replacement powered by Rust"
requires-python = ">=3.8"
classifiers = [
    "Programming Language :: Python :: 3",
    "Programming Language :: Rust",
]

[project.optional-dependencies]
dev = ["pytest>=7.0", "pytest-benchmark>=4.0", "pytest-json-report"]

[tool.maturin]
python-source = "python"
module-name = "pillow_rs._core"
features = ["pyo3/extension-module"]
```

- [ ] **Step 3: Create pillow-rs-py/src/lib.rs**

```rust
use pyo3::prelude::*;
use pillow_rs::image::Image as RsImage;
use pillow_rs::error::PilError;
use pillow_rs::ops;

#[pyclass(name = "Image")]
pub struct PyImage {
    inner: RsImage,
}

#[pymethods]
impl PyImage {
    #[new]
    fn new(mode: &str, size: (u32, u32), color: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        // Delegate color parsing to Python layer — core only receives parsed RGBA
        let c = if let Some(val) = color {
            if let Ok(hex_str) = val.extract::<String>() {
                pillow_rs::color::parse_color_str(&hex_str)
                    .map_err(|e| map_error(e))?
            } else if let Ok(i) = val.extract::<u8>() {
                (i, i, i, 255)
            } else if let Ok((r, g, b)) = val.extract::<(u8, u8, u8)>() {
                (r, g, b, 255)
            } else if let Ok((r, g, b, a)) = val.extract::<(u8, u8, u8, u8)>() {
                (r, g, b, a)
            } else {
                return Err(pyo3::exceptions::PyTypeError::new_err(
                    "color must be int, tuple, or string"
                ));
            }
        } else {
            (0, 0, 0, 0)
        };
        let img = RsImage::new(size.0, size.1, mode, c)
            .map_err(|e| map_error(e))?;
        Ok(PyImage { inner: img })
    }

    #[classmethod]
    fn open(_cls: &Bound<'_, PyType>, fp: &Bound<'_, PyAny>) -> PyResult<Self> {
        if let Ok(path) = fp.extract::<String>() {
            let img = RsImage::open_path(&path).map_err(|e| map_error(e))?;
            Ok(PyImage { inner: img })
        } else if let Ok(bytes) = fp.extract::<Vec<u8>>() {
            let img = RsImage::open_bytes(bytes).map_err(|e| map_error(e))?;
            Ok(PyImage { inner: img })
        } else {
            Err(pyo3::exceptions::PyTypeError::new_err("Expected str or bytes"))
        }
    }

    fn save(&mut self, fp: &str, format: Option<String>) -> PyResult<()> {
        self.inner.save(fp, format.as_deref()).map_err(|e| map_error(e))
    }

    fn resize(&self, size: (u32, u32), resample: Option<String>) -> PyResult<PyImage> {
        let rs = self.inner.resize(size, resample.as_deref()).map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    fn crop(&self, box_coords: (u32, u32, u32, u32)) -> PyResult<PyImage> {
        let rs = self.inner.crop(box_coords).map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    fn rotate(&self, angle: f64, expand: Option<bool>, fillcolor: Option<&Bound<'_, PyAny>>) -> PyResult<PyImage> {
        // fillcolor parsing skipped for stub — will be implemented in Phase 2
        let rs = self.inner.rotate(angle, expand.unwrap_or(false), None)
            .map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    fn transpose(&self, method: &str) -> PyResult<PyImage> {
        let rs = self.inner.transpose(method).map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (mode, matrix=None, dither=None, palette=None, colors=None))]
    fn convert(&self, mode: &str, matrix: Option<Vec<f64>>, dither: Option<String>,
               palette: Option<String>, colors: Option<u32>) -> PyResult<PyImage> {
        let rs = self.inner.convert(mode, matrix, dither.as_deref(), palette.as_deref(), colors)
            .map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (im, box_coords=None, mask=None))]
    fn paste(&mut self, im: &Bound<'_, PyAny>, box_coords: Option<&Bound<'_, PyAny>>,
             mask: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err("Image.paste"))
    }

    fn split(&self) -> PyResult<Vec<PyImage>> {
        let bands = self.inner.split().map_err(|e| map_error(e))?;
        Ok(bands.into_iter().map(|img| PyImage { inner: img }).collect())
    }

    fn filter(&self, filter_type: &str) -> PyResult<PyImage> {
        let rs = self.inner.filter(filter_type).map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    fn copy(&self) -> PyImage {
        PyImage { inner: self.inner.copy() }
    }

    fn to_bytes(&mut self) -> PyResult<Vec<u8>> {
        self.inner.to_bytes().map_err(|e| map_error(e))
    }

    fn thumbnail(&mut self, size: (u32, u32), resample: Option<String>) -> PyResult<()> {
        self.inner.thumbnail(size, resample.as_deref()).map_err(|e| map_error(e))
    }

    #[getter]
    fn size(&mut self) -> PyResult<(u32, u32)> {
        self.inner.size().map_err(|e| map_error(e))
    }

    #[getter]
    fn width(&mut self) -> PyResult<u32> {
        let (w, _) = self.inner.size().map_err(|e| map_error(e))?;
        Ok(w)
    }

    #[getter]
    fn height(&mut self) -> PyResult<u32> {
        let (_, h) = self.inner.size().map_err(|e| map_error(e))?;
        Ok(h)
    }

    #[getter]
    fn mode(&mut self) -> PyResult<String> {
        self.inner.mode().map_err(|e| map_error(e))
    }

    #[getter]
    fn format(&self) -> Option<String> {
        self.inner.format_name()
    }

    fn __repr__(&mut self) -> String {
        match self.inner.size() {
            Ok((w, h)) => {
                let mode = self.inner.mode().unwrap_or_else(|_| "?".into());
                let fmt = self.inner.format_name().unwrap_or_else(|| "Unknown".into());
                format!("<Image size={}x{} mode={} format={}>", w, h, mode, fmt)
            }
            Err(_) => "<Image [error loading]>".into(),
        }
    }
}

fn map_error(e: PilError) -> PyErr {
    match e {
        PilError::IOError(msg) => pyo3::exceptions::PyOSError::new_err(msg),
        PilError::UnidentifiedImageError(msg) => pyo3::exceptions::PyValueError::new_err(msg),
        PilError::ValueError(msg) => pyo3::exceptions::PyValueError::new_err(msg),
        PilError::TypeError(msg) => pyo3::exceptions::PyTypeError::new_err(msg),
        PilError::ImageError(err) => pyo3::exceptions::PyException::new_err(err.to_string()),
        PilError::NotImplementedError(msg) => pyo3::exceptions::PyNotImplementedError::new_err(msg),
        PilError::UnknownFormat(msg) => pyo3::exceptions::PyValueError::new_err(msg),
        PilError::Io(err) => pyo3::exceptions::PyOSError::new_err(err.to_string()),
    }
}

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyImage>()?;
    Ok(())
}
```

- [ ] **Step 4: Create Python wrapper — pillow_rs/__init__.py**

```python
"""
pillow-rs — Pillow drop-in replacement powered by Rust.
Import as: from RSPIL import Image
"""

from .enums import Dither, ImageFormat, ImageMode, Palette, Resampling, Transpose
from .image import Image
from .operations import convert, crop, new, open, resize, rotate, save

__version__ = "0.1.0"

__all__ = [
    "Image",
    "ImageMode",
    "ImageFormat",
    "Resampling",
    "Transpose",
    "Dither",
    "Palette",
    "open",
    "new",
    "save",
    "resize",
    "crop",
    "rotate",
    "convert",
]
```

- [ ] **Step 5: Create Python enums**

`pillow_rs/enums.py` — copy from `puhu/python/puhu/enums.py` (already complete and matches Pillow).

- [ ] **Step 6: Create Python Image wrapper**

`pillow_rs/image.py` — mirror Puhu's `Image` class at `puhu/python/puhu/image.py` but pointing to `._core`.

- [ ] **Step 7: Create operations.py**

`pillow_rs/operations.py` — module-level functional API mirroring Puhu's version.

- [ ] **Step 8: Build Python package**

Run: `cd pillow-rs-py && pip install maturin && maturin develop`
Expected: Builds and installs successfully. `python -c "from pillow_rs._core import Image; print(Image)"` works.

- [ ] **Step 9: Verify import works**

Run: `python -c "from pillow_rs import Image; img = Image.new('RGB', (10, 10), (255,0,0)); print(img)"`
Expected: `<Image size=10x10 mode=RGB format=Unknown>`

- [ ] **Step 10: Commit**

```bash
git add pillow-rs-py/
git commit -m "feat: PyO3 bindings — Python package with stub operations"
```

---

### Task 6: pillow-rs-js — wasm-bindgen Crate

**Files:**
- Create: `pillow-rs-js/Cargo.toml`
- Create: `pillow-rs-js/src/lib.rs`
- Create: `pillow-rs-js/package.json`

- [ ] **Step 1: Create pillow-rs-js/Cargo.toml**

```toml
[package]
name = "pillow-rs-js"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "wasm-bindgen bindings for pillow-rs — Pillow in the browser"

[lib]
crate-type = ["cdylib"]

[dependencies]
pillow-rs = { path = "../pillow-rs" }
wasm-bindgen = "0.2"
console_error_panic_hook = "0.1"
js-sys = "0.3"

[profile.release]
lto = true
opt-level = "s"
```

- [ ] **Step 2: Create pillow-rs-js/src/lib.rs**

```rust
use wasm_bindgen::prelude::*;
use pillow_rs::image::Image as RsImage;
use pillow_rs::error::PilError;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn error(s: &str);
}

#[wasm_bindgen]
pub struct Image {
    inner: RsImage,
}

#[wasm_bindgen]
impl Image {
    #[wasm_bindgen(constructor)]
    pub fn new(mode: &str, width: u32, height: u32, r: u8, g: u8, b: u8, a: u8) -> Result<Image, JsValue> {
        console_error_panic_hook::set_once();
        let img = RsImage::new(width, height, mode, (r, g, b, a))
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Image { inner: img })
    }

    #[wasm_bindgen(js_name = "open")]
    pub fn open(data: Vec<u8>) -> Result<Image, JsValue> {
        let img = RsImage::open_bytes(data)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Image { inner: img })
    }

    #[wasm_bindgen(js_name = "resize")]
    pub fn resize(&self, width: u32, height: u32, filter: Option<String>) -> Result<Image, JsValue> {
        let rs = self.inner.resize((width, height), filter.as_deref())
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Image { inner: rs })
    }

    #[wasm_bindgen(js_name = "crop")]
    pub fn crop(&self, left: u32, top: u32, right: u32, bottom: u32) -> Result<Image, JsValue> {
        let rs = self.inner.crop((left, top, right, bottom))
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Image { inner: rs })
    }

    #[wasm_bindgen(js_name = "rotate")]
    pub fn rotate(&self, angle: f64) -> Result<Image, JsValue> {
        let rs = self.inner.rotate(angle, false, None)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Image { inner: rs })
    }

    #[wasm_bindgen(js_name = "convert")]
    pub fn convert(&self, mode: &str) -> Result<Image, JsValue> {
        let rs = self.inner.convert(mode, None, None, None, None)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Image { inner: rs })
    }

    #[wasm_bindgen(js_name = "toBytes")]
    pub fn to_bytes(&mut self) -> Result<Vec<u8>, JsValue> {
        self.inner.to_bytes().map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen(js_name = "copy")]
    pub fn copy(&self) -> Image {
        Image { inner: self.inner.copy() }
    }

    #[wasm_bindgen(getter)]
    pub fn width(&mut self) -> Result<u32, JsValue> {
        let (w, _) = self.inner.size().map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(w)
    }

    #[wasm_bindgen(getter)]
    pub fn height(&mut self) -> Result<u32, JsValue> {
        let (_, h) = self.inner.size().map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(h)
    }

    #[wasm_bindgen(getter)]
    pub fn mode(&mut self) -> Result<String, JsValue> {
        self.inner.mode().map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn size(&mut self) -> Result<Vec<u32>, JsValue> {
        let (w, h) = self.inner.size().map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(vec![w, h])
    }

    pub fn repr(&mut self) -> Result<String, JsValue> {
        let (w, h) = self.inner.size().map_err(|e| JsValue::from_str(&e.to_string()))?;
        let mode = self.inner.mode().map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(format!("<Image size={}x{} mode={}>", w, h, mode))
    }
}
```

- [ ] **Step 3: Create pillow-rs-js/package.json**

```json
{
  "name": "pillow-rs",
  "version": "0.1.0",
  "description": "Pillow drop-in replacement for the browser, powered by Rust/WASM",
  "main": "pkg/pillow_rs_js.js",
  "module": "pkg/pillow_rs_js.js",
  "types": "pkg/pillow_rs_js.d.ts",
  "files": ["pkg/"],
  "scripts": {
    "build": "wasm-pack build --target web",
    "build:release": "wasm-pack build --target web --release"
  },
  "keywords": ["image", "pillow", "wasm", "rust"],
  "license": "MIT"
}
```

- [ ] **Step 4: Build WASM package**

Run: `which wasm-pack || cargo install wasm-pack`
Run: `cd pillow-rs-js && wasm-pack build --target web --dev`
Expected: Compiles, generates `pkg/` with `.js` + `.wasm` + `.d.ts`.

- [ ] **Step 5: Commit**

```bash
git add pillow-rs-js/
git commit -m "feat: wasm-bindgen bindings — JS Image class with stubs"
```

---

### Task 7: manifest.yaml — Pillow API Surface

**Files:**
- Create: `manifest.yaml`

- [ ] **Step 1: Create manifest.yaml with full Pillow API surface**

```yaml
# pillow-rs API surface — single source of truth
# Each entry defines what exists in Pillow and drives:
#   1. Stub generation  (scripts/generate_stubs.py)
#   2. Test discovery    (tests/conftest.py markers)
#   3. Coverage reports  (scripts/compute_coverage.py)

version: "0.1.0"
pillow_version: "11.0.0"

modules:
  Image:
    class_methods:
      - name: open
        signature: "open(fp: str|bytes|Path, mode: str='r', formats: list|None=None) -> Image"
        supported_modes: [L, LA, RGB, RGBA]
        supported_formats: [PNG, JPEG, GIF, BMP, TIFF, WEBP, ICO]
        param_variants:
          - {fp: path}
          - {fp: bytes}
          - {fp: path, mode: "r"}
        edge_cases:
          - nonexistent_file
          - invalid_bytes
          - empty_bytes
        pillow_since: "1.0"

      - name: new
        signature: "new(mode: str, size: tuple[int,int], color: int|tuple|str=None) -> Image"
        supported_modes: [L, LA, RGB, RGBA, 1]
        param_variants:
          - {mode: RGB, size: [100,100]}
          - {mode: L, size: [100,100]}
          - {mode: RGBA, size: [100,100]}
          - {mode: RGB, size: [100,100], color: int}
          - {mode: RGB, size: [100,100], color: hex_string}
          - {mode: RGB, size: [100,100], color: rgb_tuple}
          - {mode: LA, size: [100,100], color: la_tuple}
          - {mode: RGB, size: [100,100], color: rgba_tuple}
        edge_cases:
          - zero_width
          - zero_height
          - invalid_mode
        pillow_since: "1.0"

    methods:
      - name: save
        signature: "save(self, fp: str|Path, format: str|None=None, **params) -> None"
        supported_formats: [PNG, JPEG, GIF, BMP, TIFF, WEBP]
        param_variants:
          - {fp: path}
          - {fp: path, format: PNG}
          - {fp: path, format: JPEG}
          - {fp: path, format: GIF}
          - {fp: path, format: WEBP}
        edge_cases:
          - invalid_path
          - unsupported_format
          - read_only_directory
        pillow_since: "1.0"

      - name: resize
        signature: "resize(self, size: tuple[int,int], resample: int=BILINEAR, box: tuple|None=None, reducing_gap: float|None=None) -> Image"
        supported_modes: [L, LA, RGB, RGBA, 1, P]
        param_variants:
          - {size: [100,100]}
          - {size: [50,50], resample: NEAREST}
          - {size: [200,200], resample: BILINEAR}
          - {size: [200,200], resample: BICUBIC}
          - {size: [200,200], resample: LANCZOS}
        edge_cases:
          - same_size_noop
          - zero_dimension
          - upscale
          - downscale
          - aspect_ratio_change
        pillow_since: "1.0"

      - name: crop
        signature: "crop(self, box: tuple[int,int,int,int]) -> Image"
        supported_modes: [L, LA, RGB, RGBA, 1, P]
        param_variants:
          - {box: [0,0,50,50]}
          - {box: [10,10,90,90]}
        edge_cases:
          - out_of_bounds
          - zero_size_crop
          - full_image_crop
          - negative_coords
        pillow_since: "1.0"

      - name: rotate
        signature: "rotate(self, angle: float, resample: int=NEAREST, expand: bool=False, center: tuple|None=None, translate: tuple|None=None, fillcolor: int|tuple|None=None) -> Image"
        supported_modes: [L, LA, RGB, RGBA]
        param_variants:
          - {angle: 90}
          - {angle: 180}
          - {angle: 270}
          - {angle: 45}
          - {angle: 90, expand: true}
        edge_cases:
          - angle_zero
          - angle_negative
          - angle_360
        pillow_since: "1.0"

      - name: transpose
        signature: "transpose(self, method: int|str) -> Image"
        supported_modes: [L, LA, RGB, RGBA, 1, P]
        param_variants:
          - {method: FLIP_LEFT_RIGHT}
          - {method: FLIP_TOP_BOTTOM}
          - {method: ROTATE_90}
          - {method: ROTATE_180}
          - {method: ROTATE_270}
          - {method: TRANSPOSE}
          - {method: TRANSVERSE}
        edge_cases: []
        pillow_since: "1.0"

      - name: convert
        signature: "convert(self, mode: str|None=None, matrix: tuple|None=None, dither: int|None=None, palette: int=WEB, colors: int=256) -> Image"
        supported_modes: [L, LA, RGB, RGBA, 1, P, CMYK, YCbCr, HSV, I, F]
        param_variants:
          - {mode: L}
          - {mode: LA}
          - {mode: RGB}
          - {mode: RGBA}
          - {mode: "1"}
          - {mode: "1", dither: NONE}
          - {mode: "1", dither: FLOYDSTEINBERG}
          - {mode: P}
          - {mode: P, palette: WEB}
          - {mode: P, palette: ADAPTIVE, colors: 128}
          - {mode: RGB, matrix: 12tuple}
          - {mode: RGB, matrix: 4tuple}
        edge_cases:
          - same_mode_noop
          - invalid_mode
          - invalid_matrix_size
        pillow_since: "1.0"

      - name: paste
        signature: "paste(self, im: Image|tuple|int, box: tuple|None=None, mask: Image|None=None) -> None"
        supported_modes: [L, LA, RGB, RGBA]
        param_variants:
          - {im: image, box: [0,0]}
          - {im: image, box: [10,10,60,60]}
          - {im: color_tuple}
          - {im: image, mask: image}
          - {im: image, box: [0,0], mask: image}
        edge_cases:
          - source_larger_than_dest
          - negative_coords
          - mode_mismatch
          - mask_size_mismatch
        pillow_since: "1.0"

      - name: split
        signature: "split(self) -> tuple[Image,...]"
        supported_modes: [L, LA, RGB, RGBA]
        param_variants:
          - {}
        edge_cases:
          - single_band_image
        pillow_since: "1.0"

      - name: getbands
        signature: "getbands(self) -> tuple[str,...]"
        supported_modes: [L, LA, RGB, RGBA]
        param_variants:
          - {}
        edge_cases: []
        pillow_since: "1.0"

      - name: filter
        signature: "filter(self, filter: Filter|type) -> Image"
        supported_modes: [L, LA, RGB, RGBA]
        param_variants:
          - {filter: BLUR}
          - {filter: CONTOUR}
          - {filter: DETAIL}
          - {filter: EDGE_ENHANCE}
          - {filter: EDGE_ENHANCE_MORE}
          - {filter: EMBOSS}
          - {filter: FIND_EDGES}
          - {filter: SHARPEN}
          - {filter: SMOOTH}
          - {filter: SMOOTH_MORE}
        edge_cases: []
        pillow_since: "1.0"

      - name: copy
        signature: "copy(self) -> Image"
        supported_modes: [L, LA, RGB, RGBA, 1, P]
        param_variants:
          - {}
        edge_cases: []
        pillow_since: "1.0"

      - name: thumbnail
        signature: "thumbnail(self, size: tuple[int,int], resample: int=BICUBIC, reducing_gap: float=2.0) -> None"
        supported_modes: [L, LA, RGB, RGBA]
        param_variants:
          - {size: [100,100]}
          - {size: [50,50], resample: LANCZOS}
        edge_cases:
          - larger_than_image
          - zero_size
        pillow_since: "1.0"

      - name: tobytes
        signature: "tobytes(self, encoder_name: str='raw', *args) -> bytes"
        supported_modes: [L, LA, RGB, RGBA]
        param_variants:
          - {}
          - {encoder_name: raw}
        edge_cases: []
        pillow_since: "1.0"

      - name: load
        signature: "load(self) -> PixelAccess"
        supported_modes: [L, LA, RGB, RGBA]
        param_variants:
          - {}
        edge_cases: []
        pillow_since: "1.0"

      - name: close
        signature: "close(self) -> None"
        supported_modes: [L, LA, RGB, RGBA, 1, P]
        param_variants:
          - {}
        edge_cases: []
        pillow_since: "1.0"

      - name: getpixel
        signature: "getpixel(self, xy: tuple[int,int]) -> tuple|int"
        supported_modes: [L, LA, RGB, RGBA, 1, P]
        param_variants:
          - {xy: [0,0]}
        edge_cases:
          - out_of_bounds
        pillow_since: "1.0"

      - name: putpixel
        signature: "putpixel(self, xy: tuple[int,int], value: tuple|int) -> None"
        supported_modes: [L, LA, RGB, RGBA, 1, P]
        param_variants:
          - {xy: [0,0], value: tuple}
        edge_cases:
          - out_of_bounds
        pillow_since: "1.0"

      - name: quantize
        signature: "quantize(self, colors: int=256, method: int=MEDIANCUT, kmeans: int=0, palette: Image|None=None, dither: int=1) -> Image"
        supported_modes: [RGB, RGBA]
        param_variants:
          - {colors: 256}
          - {colors: 16}
        edge_cases: []
        pillow_since: "1.0"

    properties:
      - name: size
        type: "tuple[int,int]"
        modes: [L, LA, RGB, RGBA, 1, P]
      - name: width
        type: int
        modes: [L, LA, RGB, RGBA, 1, P]
      - name: height
        type: int
        modes: [L, LA, RGB, RGBA, 1, P]
      - name: mode
        type: str
        modes: [L, LA, RGB, RGBA, 1, P]
      - name: format
        type: "str|None"
        modes: [L, LA, RGB, RGBA, 1, P]
      - name: info
        type: dict
        modes: [L, LA, RGB, RGBA, 1, P]

  ImageDraw:
    methods:
      - name: line
        signature: "line(self, xy: list, fill=None, width: int=0, joint: str=None) -> None"
        supported_modes: [RGB, RGBA, L]
        param_variants: [{xy: line}, {xy: line, width: 5}]
        edge_cases: []
        pillow_since: "1.0"
      - name: rectangle
        signature: "rectangle(self, xy: list, fill=None, outline=None, width: int=1) -> None"
        supported_modes: [RGB, RGBA, L]
        param_variants: [{xy: box}, {xy: box, fill: color, outline: color}]
        edge_cases: []
        pillow_since: "1.0"
      - name: ellipse
        signature: "ellipse(self, xy: list, fill=None, outline=None, width: int=1) -> None"
        supported_modes: [RGB, RGBA, L]
        param_variants: [{xy: box}]
        edge_cases: []
        pillow_since: "1.0"
      - name: text
        signature: "text(self, xy: tuple, text: str, fill=None, font=None, anchor=None, spacing: int=4, align: str='left', direction: str=None, features: list=None, language: str=None, stroke_width: int=0, stroke_fill=None, embedded_color: bool=False) -> None"
        supported_modes: [RGB, RGBA, L]
        param_variants: [{xy: [0,0], text: "hello"}]
        edge_cases: []
        pillow_since: "1.0"

  ImageFilter:
    functions:
      - name: BLUR
        signature: "ImageFilter.BLUR"
      - name: CONTOUR
        signature: "ImageFilter.CONTOUR"
      - name: DETAIL
        signature: "ImageFilter.DETAIL"
      - name: EDGE_ENHANCE
        signature: "ImageFilter.EDGE_ENHANCE"
      - name: EDGE_ENHANCE_MORE
        signature: "ImageFilter.EDGE_ENHANCE_MORE"
      - name: EMBOSS
        signature: "ImageFilter.EMBOSS"
      - name: FIND_EDGES
        signature: "ImageFilter.FIND_EDGES"
      - name: SHARPEN
        signature: "ImageFilter.SHARPEN"
      - name: SMOOTH
        signature: "ImageFilter.SMOOTH"
      - name: SMOOTH_MORE
        signature: "ImageFilter.SMOOTH_MORE"
      - name: GaussianBlur
        signature: "ImageFilter.GaussianBlur(radius: float=2)"
      - name: BoxBlur
        signature: "ImageFilter.BoxBlur(radius: float)"
      - name: UnsharpMask
        signature: "ImageFilter.UnsharpMask(radius: float=2, percent: int=150, threshold: int=3)"
      - name: MaxFilter
        signature: "ImageFilter.MaxFilter(size: int=3)"
      - name: MinFilter
        signature: "ImageFilter.MinFilter(size: int=3)"
      - name: MedianFilter
        signature: "ImageFilter.MedianFilter(size: int=3)"
      - name: ModeFilter
        signature: "ImageFilter.ModeFilter(size: int=3)"

  ImageEnhance:
    methods:
      - name: enhance
        signature: "enhance(self, factor: float) -> Image"
        supported_modes: [L, LA, RGB, RGBA]
        param_variants: [{factor: 1.0}, {factor: 0.5}, {factor: 2.0}]
        edge_cases: [factor_zero, factor_negative]
        pillow_since: "1.0"
```

- [ ] **Step 2: Validate manifest is valid YAML**

Run: `python -c "import yaml; yaml.safe_load(open('manifest.yaml')); print('OK')"`
Expected: `OK`

- [ ] **Step 3: Commit**

```bash
git add manifest.yaml
git commit -m "feat: manifest.yaml — complete Pillow API surface definition"
```

---

### Task 8: Test Infrastructure

**Files:**
- Create: `tests/conftest.py`
- Create: `tests/benchmarks/conftest.py`

- [ ] **Step 1: Create tests/conftest.py**

```python
"""
Test configuration for pillow-rs.
Tests are written against Pillow's API and can run against:
  1. pillow-rs (RSPIL) — default
  2. PIL (Pillow)      — for parity comparison via --pil flag

Usage:
    pytest tests/                          # test pillow-rs
    pytest tests/ --pil                    # test against Pillow (parity check)
    pytest tests/ --json-report            # generate coverage input
"""
import pytest
import yaml
from pathlib import Path


def pytest_addoption(parser):
    parser.addoption("--pil", action="store_true", default=False,
                     help="Run tests against Pillow instead of pillow-rs")
    parser.addoption("--manifest", action="store", default="manifest.yaml",
                     help="Path to manifest.yaml")


@pytest.fixture(scope="session")
def manifest(request):
    manifest_path = Path(request.config.getoption("--manifest"))
    with open(manifest_path) as f:
        return yaml.safe_load(f)


@pytest.fixture(scope="session")
def use_pillow(request):
    return request.config.getoption("--pil", False)


@pytest.fixture(scope="session")
def ImageModule(use_pillow):
    """Import Image from RSPIL or PIL based on --pil flag."""
    if use_pillow:
        from PIL import Image as PILImage
        return PILImage
    else:
        from pillow_rs import Image
        return Image


@pytest.fixture
def Image(ImageModule):
    return ImageModule


def pytest_collection_modifyitems(config, items):
    """If --pil is set, skip tests marked with rs_only."""
    if config.getoption("--pil", False):
        skip_rs = pytest.mark.skip(reason="requires pillow-rs")
        for item in items:
            if "rs_only" in item.keywords:
                item.add_marker(skip_rs)


def pytest_configure(config):
    config.addinivalue_line("markers", "covers(func, mode=None, variant=None): mark test as covering a manifest entry")
    config.addinivalue_line("markers", "rs_only: test only applies to pillow-rs (not Pillow)")
```

- [ ] **Step 2: Create tests/benchmarks/conftest.py**

```python
"""Benchmark fixtures — large test images at various sizes."""
import pytest
from pillow_rs import Image


@pytest.fixture(scope="module")
def rgb_small_image():
    return Image.new("RGB", (100, 100), (128, 64, 32))


@pytest.fixture(scope="module")
def rgb_medium_image():
    return Image.new("RGB", (800, 600), (255, 128, 0))


@pytest.fixture(scope="module")
def rgb_large_image():
    return Image.new("RGB", (4000, 3000), (64, 128, 255))


@pytest.fixture(scope="module")
def rgba_image():
    return Image.new("RGBA", (500, 500), (255, 0, 0, 128))


@pytest.fixture(scope="module")
def grayscale_image():
    return Image.new("L", (500, 500), 128)
```

- [ ] **Step 3: Verify test infrastructure**

Run: `cd pillow-rs-py && pip install -e ".[dev]" && python -c "from pillow_rs import Image; print('OK')"`
Expected: `OK`

- [ ] **Step 4: Commit**

```bash
git add tests/
git commit -m "feat: test infrastructure — conftest, fixtures, Pillow parity mode"
```

---

### Task 9: Core Test Suite

**Files:**
- Create: `tests/test_image_new.py`
- Create: `tests/test_image_open.py`
- Create: `tests/test_image_resize.py`
- Create: `tests/test_image_crop.py`
- Create: `tests/test_image_rotate.py`
- Create: `tests/test_image_convert.py`
- Create: `tests/test_image_paste.py`
- Create: `tests/test_image_split.py`
- Create: `tests/test_image_properties.py`
- Create: `tests/test_image_filter.py`

- [ ] **Step 1: Create tests/test_image_new.py**

```python
"""Tests for Image.new() — creation with various modes and colors."""
import pytest
from pillow_rs import Image


class TestImageNew:
    @pytest.mark.covers("Image.new", mode="RGB", variant="default")
    def test_new_rgb_default(self):
        img = Image.new("RGB", (100, 100))
        assert img.size == (100, 100)
        assert img.mode == "RGB"

    @pytest.mark.covers("Image.new", mode="RGB", variant="color_int")
    def test_new_rgb_with_int_color(self):
        img = Image.new("RGB", (50, 50), 128)
        assert img.size == (50, 50)

    @pytest.mark.covers("Image.new", mode="RGB", variant="color_hex")
    def test_new_rgb_with_hex_color(self):
        img = Image.new("RGB", (10, 10), "#FF0000")
        data = img.tobytes()
        assert data[0] == 255  # R
        assert data[1] == 0    # G

    @pytest.mark.covers("Image.new", mode="RGB", variant="color_rgb_tuple")
    def test_new_rgb_with_rgb_tuple(self):
        img = Image.new("RGB", (25, 25), (255, 0, 0))
        assert img.mode == "RGB"

    @pytest.mark.covers("Image.new", mode="RGBA", variant="default")
    def test_new_rgba(self):
        img = Image.new("RGBA", (30, 30))
        assert img.mode == "RGBA"

    @pytest.mark.covers("Image.new", mode="L", variant="default")
    def test_new_grayscale(self):
        img = Image.new("L", (10, 10), 128)
        assert img.mode == "L"
        data = img.tobytes()
        assert data[0] == 128

    @pytest.mark.covers("Image.new", edge_case="zero_width")
    def test_new_zero_width_raises(self):
        with pytest.raises(Exception):
            Image.new("RGB", (0, 100))

    @pytest.mark.covers("Image.new", edge_case="invalid_mode")
    def test_new_invalid_mode_raises(self):
        with pytest.raises(Exception):
            Image.new("INVALID", (100, 100))
```

- [ ] **Step 2: Create tests/test_image_properties.py**

```python
"""Tests for Image properties — size, width, height, mode, format."""
import pytest
from pillow_rs import Image


class TestImageProperties:
    def test_size(self):
        img = Image.new("RGB", (150, 75))
        assert img.size == (150, 75)

    def test_width_height(self):
        img = Image.new("RGB", (200, 100))
        assert img.width == 200
        assert img.height == 100

    def test_mode(self):
        img = Image.new("RGBA", (10, 10))
        assert img.mode == "RGBA"

    def test_format_none_for_new(self):
        img = Image.new("RGB", (10, 10))
        assert img.format is None

    def test_repr(self):
        img = Image.new("RGB", (20, 30))
        r = repr(img)
        assert "Image" in r
        assert "20" in r
        assert "30" in r
```

- [ ] **Step 3: Create tests/test_image_copy.py**

```python
"""Tests for Image.copy()."""
import pytest
from pillow_rs import Image


class TestImageCopy:
    def test_copy_is_independent(self):
        img = Image.new("RGB", (50, 50), (255, 0, 0))
        copied = img.copy()
        assert copied.size == img.size
        assert copied.mode == img.mode
        assert copied is not img
```

- [ ] **Step 4: Create tests/test_image_resize.py (stub verification)**

```python
"""Tests for Image.resize() — verifies signature and error on stubs."""
import pytest
from pillow_rs import Image


class TestImageResize:
    @pytest.mark.covers("Image.resize", mode="RGB", variant="default")
    def test_resize_signature_exists(self):
        img = Image.new("RGB", (100, 100))
        with pytest.raises(NotImplementedError, match="Image.resize"):
            img.resize((50, 50))

    @pytest.mark.covers("Image.resize", mode="RGB", variant="nearest")
    def test_resize_with_nearest(self):
        img = Image.new("RGB", (100, 100))
        with pytest.raises(NotImplementedError, match="Image.resize"):
            img.resize((50, 50), resample=0)

    @pytest.mark.covers("Image.resize", mode="RGB", variant="lanczos")
    def test_resize_with_lanczos(self):
        img = Image.new("RGB", (100, 100))
        with pytest.raises(NotImplementedError, match="Image.resize"):
            img.resize((200, 200), resample=3)
```

- [ ] **Step 5: Create remaining test files following the same pattern**

`tests/test_image_crop.py`, `tests/test_image_rotate.py`, `tests/test_image_convert.py`, `tests/test_image_paste.py`, `tests/test_image_split.py`, `tests/test_image_filter.py` — each verifies the method exists and raises `NotImplementedError` with the correct method name.

- [ ] **Step 6: Run test suite**

Run: `cd pillow-rs-py && pip install -e ".[dev]" && python -m pytest ../tests/ -v`
Expected: All tests pass (signature verification). Failures if any stub is missing.

- [ ] **Step 7: Commit**

```bash
git add tests/
git commit -m "test: core test suite — signature verification for all stubs"
```

---

### Task 10: Coverage Computation Script

**Files:**
- Create: `scripts/compute_coverage.py`

- [ ] **Step 1: Create scripts/compute_coverage.py**

```python
#!/usr/bin/env python3
"""
Compute coverage from manifest.yaml + pytest json report.

Reads:
  - manifest.yaml      → expected API surface
  - report.json        → pytest --json-report output

Produces:
  - coverage/report.json → per-function, per-module, overall scores
  - stdout summary table

Coverage formula per function:
  signature × 0.10 + params × 0.20 + modes × 0.35 + edges × 0.15 + formats × 0.10 + parity × 0.10
"""
import json
import sys
import yaml
from pathlib import Path
from collections import defaultdict

WEIGHTS = {
    "signature": 0.10,
    "params": 0.20,
    "modes": 0.35,
    "edges": 0.15,
    "formats": 0.10,
    "parity": 0.10,
}


def load_manifest(path: str) -> dict:
    with open(path) as f:
        return yaml.safe_load(f)


def load_test_results(path: str) -> dict:
    """Load pytest --json-report output."""
    with open(path) as f:
        return json.load(f)


def extract_covered_cells(tests: dict) -> dict:
    """
    Extract coverage from test markers.
    Returns: { "Image.resize": { "modes": {"RGB"}, "variants": {"default"}, ... } }
    """
    covered = defaultdict(lambda: {
        "signature_tested": False,
        "modes": set(),
        "variants": set(),
        "edges": set(),
        "formats": set(),
        "parity_tests": 0,
        "parity_passes": 0,
    })

    for test in tests.get("tests", []):
        markers = test.get("markers", [])
        func_name = None
        mode = None
        variant = None
        edge = None
        fmt = None

        for marker in markers:
            if marker.startswith("covers("):
                # Parse: covers("Image.resize", mode="RGB", variant="default")
                args_str = marker[7:-1]  # strip "covers(" and ")"
                # Simple parse — handles our format
                parts = [p.strip() for p in args_str.split(",")]
                func_name = parts[0].strip('"').strip("'")
                for p in parts[1:]:
                    if "=" in p:
                        k, v = p.split("=", 1)
                        k = k.strip()
                        v = v.strip().strip('"').strip("'")
                        if k == "mode":
                            mode = v
                        elif k == "variant":
                            variant = v
                        elif k == "edge_case":
                            edge = v
                        elif k == "format":
                            fmt = v

        if func_name is None:
            continue

        cell = covered[func_name]
        cell["signature_tested"] = True
        if mode:
            cell["modes"].add(mode)
        if variant:
            cell["variants"].add(variant)
        if edge:
            cell["edges"].add(edge)
        if fmt:
            cell["formats"].add(fmt)

        outcome = test.get("outcome", "failed")
        if outcome == "passed":
            cell["parity_tests"] += 1
            cell["parity_passes"] += 1
        elif outcome in ("failed", "error", "xpassed", "xfailed"):
            cell["parity_tests"] += 1

    return dict(covered)


def compute_function_coverage(func_def: dict, cells: dict, func_key: str) -> dict:
    """Compute coverage score for a single function."""
    cell = cells.get(func_key, {})
    test_count = 0
    pass_count = 0

    # Signature: 1 if any test covers this function
    sig_score = 1.0 if cell.get("signature_tested") else 0.0

    # Parameters: % of expected variants that have tests
    expected_modes = set(func_def.get("supported_modes", []))
    expected_variants = [v for v in func_def.get("param_variants", [])]
    expected_edges = func_def.get("edge_cases", [])
    expected_formats = func_def.get("supported_formats", [])

    tested_modes = cell.get("modes", set())
    tested_variants = cell.get("variants", set())

    # Each param_variant is a dict; we use its index as identifier
    n_expected_variants = len(expected_variants)
    # Count which variants were tested (by the variant name in markers)
    n_tested_variants = min(len(tested_variants), n_expected_variants) if n_expected_variants > 0 else 0
    param_score = n_tested_variants / max(n_expected_variants, 1)

    # Mode × variant matrix
    total_mode_cells = max(len(expected_modes) * max(n_expected_variants, 1), 1)
    covered_mode_cells = len(tested_modes & expected_modes) * max(n_tested_variants, 1)
    mode_score = min(covered_mode_cells / total_mode_cells, 1.0)

    # Edge cases
    n_expected_edges = len(expected_edges)
    n_tested_edges = len(cell.get("edges", set()) & set(expected_edges))
    edge_score = n_tested_edges / max(n_expected_edges, 1)

    # Formats
    n_expected_fmts = len(expected_formats)
    n_tested_fmts = len(cell.get("formats", set()) & set(expected_formats))
    format_score = n_tested_fmts / max(n_expected_fmts, 1) if n_expected_fmts > 0 else 1.0

    # Parity
    parity_total = cell.get("parity_tests", 0)
    parity_passes = cell.get("parity_passes", 0)
    parity_score = parity_passes / max(parity_total, 1)

    total = (
        WEIGHTS["signature"] * sig_score
        + WEIGHTS["params"] * param_score
        + WEIGHTS["modes"] * mode_score
        + WEIGHTS["edges"] * edge_score
        + WEIGHTS["formats"] * format_score
        + WEIGHTS["parity"] * parity_score
    )

    return {
        "function": func_key,
        "signature_score": sig_score,
        "param_score": round(param_score, 3),
        "mode_score": round(mode_score, 3),
        "edge_score": round(edge_score, 3),
        "format_score": round(format_score, 3),
        "parity_score": round(parity_score, 3),
        "total": round(total, 3),
        "mode_coverage": f"{covered_mode_cells}/{total_mode_cells}",
        "variant_coverage": f"{n_tested_variants}/{max(n_expected_variants, 1)}",
    }


def extract_all_functions(manifest: dict) -> dict:
    """Walk manifest and return {full_key: definition} for every function/method."""
    funcs = {}
    for module_name, module_def in manifest.get("modules", {}).items():
        # Class methods
        for method in module_def.get("class_methods", []):
            key = f"{module_name}.{method['name']}"
            funcs[key] = method
        for method in module_def.get("methods", []):
            key = f"{module_name}.{method['name']}"
            funcs[key] = method
        for func in module_def.get("functions", []):
            key = f"{module_name}.{func['name']}"
            funcs[key] = func
    return funcs


def main():
    manifest_path = sys.argv[1] if len(sys.argv) > 1 else "manifest.yaml"
    report_path = sys.argv[2] if len(sys.argv) > 2 else "report.json"

    manifest = load_manifest(manifest_path)
    tests = load_test_results(report_path) if Path(report_path).exists() else {"tests": []}

    cells = extract_covered_cells(tests)
    funcs = extract_all_functions(manifest)

    results = []
    for key, func_def in sorted(funcs.items()):
        score = compute_function_coverage(func_def, cells, key)
        results.append(score)

    # Module summaries
    module_scores = defaultdict(list)
    for r in results:
        parts = r["function"].split(".")
        mod = parts[0] if len(parts) > 0 else "unknown"
        module_scores[mod].append(r["total"])

    modules = {}
    for mod, scores in sorted(module_scores.items()):
        modules[mod] = {
            "function_count": len(scores),
            "average": round(sum(scores) / len(scores), 3),
        }

    overall = round(sum(r["total"] for r in results) / max(len(results), 1), 3)

    report = {
        "version": manifest.get("version", "unknown"),
        "pillow_version": manifest.get("pillow_version", "unknown"),
        "overall_coverage": overall,
        "modules": modules,
        "functions": results,
    }

    # Write report
    Path("coverage").mkdir(exist_ok=True)
    with open("coverage/report.json", "w") as f:
        json.dump(report, f, indent=2)

    # Print summary
    print(f"\n{'='*60}")
    print(f"  pillow-rs Coverage Report")
    print(f"  Overall: {overall*100:.1f}%")
    print(f"{'='*60}")
    print(f"  {'Module':<20} {'Funcs':<8} {'Coverage':<10}")
    print(f"  {'-'*38}")
    for mod, info in sorted(modules.items()):
        print(f"  {mod:<20} {info['function_count']:<8} {info['average']*100:.1f}%")
    print(f"{'='*60}\n")

    return report


if __name__ == "__main__":
    main()
```

- [ ] **Step 2: Test coverage script with empty results**

Run: `python scripts/compute_coverage.py manifest.yaml /dev/null 2>&1 || echo '[]' > /tmp/empty.json && python scripts/compute_coverage.py manifest.yaml /tmp/empty.json`
Expected: Prints coverage table with 0% for all functions.

- [ ] **Step 3: Commit**

```bash
git add scripts/compute_coverage.py
git commit -m "feat: coverage computation script — reads manifest + pytest json"
```

---

### Task 11: Stub Generation Script

**Files:**
- Create: `scripts/generate_stubs.py`

- [ ] **Step 1: Create scripts/generate_stubs.py**

```python
#!/usr/bin/env python3
"""
Generate Rust stub functions from manifest.yaml.
Reads the manifest and produces Rust code with unimplemented!() bodies
for every function that doesn't yet exist in pillow-rs/src/ops/.
"""
import yaml
import sys
from pathlib import Path
from collections import defaultdict


def load_manifest(path: str) -> dict:
    with open(path) as f:
        return yaml.safe_load(f)


def generate_rust_stub(func_name: str, func_def: dict, module: str) -> str:
    """Generate a Rust stub method on Image."""
    return f"""    pub fn {func_name}(&self) -> Result<(), crate::error::PilError> {{
        Err(crate::error::PilError::NotImplementedError("{module}.{func_name}".into()))
    }}
"""


def main():
    manifest_path = sys.argv[1] if len(sys.argv) > 1 else "manifest.yaml"
    manifest = load_manifest(manifest_path)

    ops_dir = Path("pillow-rs/src/ops")
    existing = set()
    # Find existing impl blocks per file
    for rs_file in ops_dir.glob("*.rs"):
        content = rs_file.read_text()
        for line in content.split("\n"):
            if "pub fn " in line:
                # Extract function name
                name = line.split("pub fn ")[1].split("(")[0].strip()
                existing.add(name)

    missing = []
    for mod_name, mod_def in manifest.get("modules", {}).items():
        for method in mod_def.get("methods", []):
            if method["name"] not in existing:
                missing.append((mod_name, method["name"], method))

    if missing:
        print(f"Missing stubs ({len(missing)}):")
        for mod, name, _ in missing:
            print(f"  {mod}.{name}")
    else:
        print("All manifest entries have stubs.")


if __name__ == "__main__":
    main()
```

- [ ] **Step 2: Run stub checker**

Run: `python scripts/generate_stubs.py manifest.yaml`
Expected: Lists any missing stubs (should be 0 after manual stub creation in Task 4).

- [ ] **Step 3: Commit**

```bash
git add scripts/generate_stubs.py
git commit -m "feat: stub generation checker — validates manifest coverage"
```

---

### Task 12: Benchmark Test Suite

**Files:**
- Create: `tests/benchmarks/test_bench_resize.py`
- Create: `tests/benchmarks/test_bench_convert.py`
- Create: `scripts/compare_benchmarks.py`

- [ ] **Step 1: Create tests/benchmarks/test_bench_resize.py**

```python
"""Benchmarks for Image.resize with pytest-benchmark.

Run: pytest tests/benchmarks/ -m benchmark --benchmark-only
"""
import pytest
from pillow_rs import Image, Resampling


@pytest.mark.benchmark
class TestBenchResize:
    def test_bench_resize_rgb_small(self, benchmark, rgb_small_image):
        result = benchmark(lambda: rgb_small_image.resize((50, 50), Resampling.BILINEAR))
        assert result.size == (50, 50)

    def test_bench_resize_rgb_medium(self, benchmark, rgb_medium_image):
        result = benchmark(lambda: rgb_medium_image.resize((400, 300), Resampling.BILINEAR))
        assert result.size == (400, 300)

    def test_bench_resize_rgb_large_lanczos(self, benchmark, rgb_large_image):
        result = benchmark(lambda: rgb_large_image.resize((800, 600), Resampling.LANCZOS))
        assert result.size == (800, 600)

    def test_bench_resize_rgba(self, benchmark, rgba_image):
        result = benchmark(lambda: rgba_image.resize((250, 250), Resampling.BICUBIC))
        assert result.size == (250, 250)

    def test_bench_resize_grayscale(self, benchmark, grayscale_image):
        result = benchmark(lambda: grayscale_image.resize((250, 250), Resampling.NEAREST))
        assert result.size == (250, 250)
```

- [ ] **Step 2: Create tests/benchmarks/test_bench_convert.py**

```python
"""Benchmarks for Image.convert."""
import pytest
from pillow_rs import Image


@pytest.mark.benchmark
class TestBenchConvert:
    def test_bench_rgb_to_l(self, benchmark, rgb_medium_image):
        result = benchmark(lambda: rgb_medium_image.convert("L"))
        assert result.mode == "L"

    def test_bench_rgb_to_rgba(self, benchmark, rgb_medium_image):
        result = benchmark(lambda: rgb_medium_image.convert("RGBA"))
        assert result.mode == "RGBA"

    def test_bench_rgba_to_rgb(self, benchmark, rgba_image):
        result = benchmark(lambda: rgba_image.convert("RGB"))
        assert result.mode == "RGB"

    def test_bench_rgb_to_1(self, benchmark, rgb_medium_image):
        result = benchmark(lambda: rgb_medium_image.convert("1", dither="FLOYDSTEINBERG"))
        assert result.mode == "L"
```

- [ ] **Step 3: Create scripts/compare_benchmarks.py (skeleton)**

```python
#!/usr/bin/env python3
"""
Compare pillow-rs benchmark results against Pillow baseline.
Generates comparison table matching Puhu's BENCHMARKS.md format.

Usage: python scripts/compare_benchmarks.py <pillow-rs-bench.json> <pillow-bench.json>
"""
import json
import sys


def load_bench(path: str) -> dict:
    with open(path) as f:
        return json.load(f)


def compare(pillow_rs_data: dict, pillow_data: dict) -> list:
    """Compute speedup (pillow_time / pillow_rs_time) for each benchmark."""
    results = []
    # TODO: implemented in Phase 2 when benchmarks produce output
    return results


def print_table(results: list) -> None:
    print(f"{'Benchmark':<30} {'Pillow':>10} {'pillow-rs':>10} {'Speedup':>12}")
    print("-" * 62)
    for r in results:
        speedup = r["pillow_ms"] / r["pillow_rs_ms"] if r["pillow_rs_ms"] > 0 else 0
        direction = "faster" if speedup > 1 else "slower"
        print(f"{r['name']:<30} {r['pillow_ms']:>8.1f}ms {r['pillow_rs_ms']:>8.1f}ms {speedup:>7.2f}× {direction}")


if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("Usage: compare_benchmarks.py <pillow-rs-bench.json> <pillow-bench.json>")
        sys.exit(1)
    rs = load_bench(sys.argv[1])
    pil = load_bench(sys.argv[2])
    results = compare(rs, pil)
    print_table(results)
```

- [ ] **Step 4: Commit**

```bash
git add tests/benchmarks/ scripts/compare_benchmarks.py
git commit -m "feat: benchmark suite — resize and convert benchmarks, comparison script"
```

---

### Task 13: CI Pipeline

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: Create .github/workflows/ci.yml**

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  core-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - run: cargo fmt --check
      - run: cargo clippy --all-targets --all-features -- -D warnings
      - run: cargo test -p pillow-rs

  python-tests:
    needs: core-tests
    runs-on: ubuntu-latest
    strategy:
      matrix:
        python-version: ["3.8", "3.10", "3.12"]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: actions/setup-python@v5
        with:
          python-version: ${{ matrix.python-version }}
      - run: pip install maturin
      - working-directory: pillow-rs-py
        run: maturin develop
      - run: pip install -e "pillow-rs-py/[dev]"
      - run: python -m pytest tests/ -v --json-report --json-report-file=report.json
      - run: python scripts/compute_coverage.py manifest.yaml report.json
      - uses: actions/upload-artifact@v4
        with:
          name: coverage-report-py${{ matrix.python-version }}
          path: coverage/report.json

  wasm-build:
    needs: core-tests
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown
      - run: cargo install wasm-pack
      - working-directory: pillow-rs-js
        run: wasm-pack build --target web --dev

  benchmark:
    needs: python-tests
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: actions/setup-python@v5
        with:
          python-version: "3.12"
      - run: pip install maturin
      - working-directory: pillow-rs-py
        run: maturin develop --release
      - run: pip install -e "pillow-rs-py/[dev]"
      - run: python -m pytest tests/benchmarks/ -m benchmark --benchmark-only --benchmark-json=bench-rs.json
      - uses: actions/upload-artifact@v4
        with:
          name: benchmark-results
          path: bench-rs.json
```

- [ ] **Step 2: Commit**

```bash
git add .github/
git commit -m "feat: CI pipeline — core tests, Python matrix, WASM build, benchmarks"
```

---

### Task 14: Final Integration Verification

- [ ] **Step 1: Run the full pipeline locally**

```bash
# 1. Core
cargo test -p pillow-rs
cargo clippy --all-targets --all-features -- -D warnings

# 2. Python
cd pillow-rs-py && maturin develop --release && cd ..
python -c "from pillow_rs import Image; img = Image.new('RGB', (100,100)); print(img)"

# 3. Run tests and generate coverage
python -m pytest tests/ -v --json-report --json-report-file=report.json
python scripts/compute_coverage.py manifest.yaml report.json

# 4. WASM
cd pillow-rs-js && wasm-pack build --target web --dev && cd ..
```

- [ ] **Step 2: Verify coverage report**

Run: `cat coverage/report.json | python -m json.tool | head -30`
Expected: Valid JSON with `overall_coverage: 0.0` (all stubs, no implementations yet).

- [ ] **Step 3: Final commit**

```bash
git add -A
git commit -m "feat: Phase 1 complete — workspace, stubs, tests, coverage, CI"
```
