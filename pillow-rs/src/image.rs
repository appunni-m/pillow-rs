//! Core Pillow-style image object.
//!
//! [`Image`] stores loaded buffers, lazy path/byte inputs, paletted data, and
//! deferred pipelines. Public methods expose Rust equivalents of common
//! `PIL.Image.Image` behavior while keeping I/O and host-language object
//! conversion outside this crate.
//!
//! # Representation
//!
//! `Image` is both a public handle and the crate's lazy representation. Methods
//! such as [`Image::new`], [`Image::open`], and [`Image::open_bytes`] are the
//! stable construction surface. Enum variants are public because binding crates
//! and integration tests need to inspect or carry lazy state, but downstream
//! users should prefer methods over direct variant construction.
//!
//! # Mode And Layout
//!
//! Mode strings follow Pillow names. Raw byte APIs use the current image mode:
//! `L` is one byte per pixel, `RGB` is tightly packed triplets, `RGBA` is
//! tightly packed quadruplets, and `P` returns palette indices. Non-standard
//! modes such as `CMYK`, `HSV`, `YCbCr`, `I`, and `F` may be carried through
//! internal `pillow-rs-image` buffers with an explicit mode tag.
//!
//! # Lazy Execution
//!
//! Operations that can be represented as [`crate::pipeline::PipelineOp`] values
//! may be deferred. Calling [`Image::materialize`], [`Image::save`], or
//! [`Image::tobytes`] forces decoding and pipeline execution.

use pillow_rs_image::{DynamicImage, GenericImageView, ImageFormat};
use std::io::{BufReader, Read, Seek};
use std::path::PathBuf;
use std::sync::Arc;

use crate::checked_dims::CheckedDims;
use crate::color::color_type_to_mode;
use crate::error::PilError;
use crate::format::parse_format_str;
use crate::pipeline::{PipelineOp, ResampleFilter};

/// Default palette matching PIL's web/browser palette.
/// Used for P-mode images without an explicit palette.
/// Matches PIL's `ImagingPaletteNewBrowser`: 6×6×6 color cube entries at indices 10-225,
/// with indices 0-9 and 226-255 set to zero.
pub fn default_palette() -> Vec<u8> {
    let mut pal = vec![0u8; 768]; // 256 * 3 (RGB)
    let mut i = 10; // PIL reserves indices 0-9
    let b_step: [u8; 6] = [0, 51, 102, 153, 204, 255];
    let g_step: [u8; 6] = [0, 51, 102, 153, 204, 255];
    let r_step: [u8; 6] = [0, 51, 102, 153, 204, 255];
    for &b in &b_step {
        for &g in &g_step {
            for &r in &r_step {
                let base = i as usize * 3;
                pal[base] = r;
                pal[base + 1] = g;
                pal[base + 2] = b;
                i += 1;
            }
        }
    }
    // Entries 0-9 and 226-255 remain zero (matching PIL behavior)
    pal
}

/// A decoded P-mode (palette) image.
/// `indices` holds one byte per pixel (the palette index, 0-255).
/// `palette` holds 768 bytes: 256 RGB triples mapping each index to a color.
#[derive(Debug, Clone)]
pub struct PalettedData {
    /// One palette index byte per pixel.
    pub indices: pillow_rs_image::GrayImage,
    /// Palette data as 256 RGB triples.
    pub palette: Vec<u8>,
}

/// Core image value used by Pillow-style operations.
///
/// Values may hold decoded pixels, lazy input references, or a deferred
/// operation pipeline. Call [`Image::materialize`] when a concrete
/// [`DynamicImage`] is required.
#[derive(Debug, Clone)]
pub enum Image {
    /// Fully decoded, ready to process or save. Optional explicit PIL mode.
    Loaded(DynamicImage, Option<String>),
    /// Fully decoded P-mode (palette) image: index bytes + 768-byte palette.
    Paletted(PalettedData),
    /// Path not yet decoded — lazy.
    Path {
        /// Source file path.
        path: PathBuf,
        /// Optional decoded or caller-provided image format.
        format: Option<ImageFormat>,
        /// Whether format probing identified indexed palette storage.
        is_paletted: bool,
    },
    /// Byte buffer not yet decoded — lazy.
    Bytes {
        /// Shared encoded image bytes.
        data: Arc<Vec<u8>>,
        /// Optional detected image format.
        format: Option<ImageFormat>,
        /// Whether format probing identified indexed palette storage.
        is_paletted: bool,
    },
    /// Lazy pipeline — operations recorded, not executed.
    /// source: the input image (loaded or another pipeline).
    /// ops: the operations to apply, in order.
    /// explicit_mode: PIL mode override (e.g. "1", "P") preserved from source.
    Pipeline {
        /// Input image feeding the operation pipeline.
        source: Arc<Image>,
        /// Operations to execute in order.
        ops: Vec<PipelineOp>,
        /// Output format inherited from the source when known.
        format: Option<ImageFormat>,
        /// Explicit Pillow mode preserved across lazy operations.
        explicit_mode: Option<String>,
        /// Locked backend for this pipeline. None = use global active set.
        backend: Option<crate::compute::Backend>,
        /// Quantize palette (RGB triples) — populated after Quantize op materializes.
        palette: Option<Vec<u8>>,
    },
}

/// PIL-compatible statistics result. Scalars for single-band, Vecs for multi-band.
#[derive(Debug, Clone)]
pub struct StatResult {
    /// Number of pixels contributing to each band.
    pub count: StatValue,
    /// Sum of channel values.
    pub sum: StatValue,
    /// Sum of squared channel values.
    pub sum2: StatValue,
    /// Mean channel value.
    pub mean: StatValue,
    /// Median channel value.
    pub median: StatValue,
    /// Root-mean-square channel value.
    pub rms: StatValue,
    /// Variance per channel.
    pub var: StatValue,
    /// Standard deviation per channel.
    pub stddev: StatValue,
    /// Minimum and maximum value per channel.
    pub extrema: StatValue,
}

/// Scalar or per-band statistic value.
#[derive(Debug, Clone)]
pub enum StatValue {
    /// Single integer statistic.
    Int(i64),
    /// Single floating-point statistic.
    Float(f64),
    /// Per-band integer statistics.
    IntList(Vec<i64>),
    /// Per-band floating-point statistics.
    FloatList(Vec<f64>),
    /// Single-band extrema pair.
    ExtremaSingle((i64, i64)),
    /// Per-band extrema pairs.
    ExtremaList(Vec<(i64, i64)>),
}

impl StatResult {
    fn from_bands(bands: &[Vec<f64>]) -> Self {
        let n = bands.len();
        let single = n == 1;
        let fi = |idx: usize| -> StatValue {
            if single {
                StatValue::Int(bands[0][idx] as i64)
            } else {
                StatValue::IntList(bands.iter().map(|b| b[idx] as i64).collect())
            }
        };
        let ff = |idx: usize| -> StatValue {
            if single {
                StatValue::Float(bands[0][idx])
            } else {
                StatValue::FloatList(bands.iter().map(|b| b[idx]).collect())
            }
        };
        let extrema = |min_idx, max_idx| -> StatValue {
            // Always use list format for extrema: [[min, max]] for single, [[min,max], ...] for multi
            StatValue::ExtremaList(
                bands
                    .iter()
                    .map(|b| (b[min_idx] as i64, b[max_idx] as i64))
                    .collect(),
            )
        };
        StatResult {
            count: fi(0),
            sum: ff(1),
            sum2: ff(2),
            mean: ff(3),
            median: fi(4),
            rms: ff(5),
            var: ff(6),
            stddev: ff(7),
            extrema: extrema(8, 9),
        }
    }
}

impl Image {
    // ── Constructors ──

    #[allow(clippy::too_many_arguments)]
    /// Creates a new image with the requested Pillow mode, dimensions, and fill color.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when dimensions are invalid or the mode is not
    /// supported by core image construction.
    pub fn new(
        width: u32,
        height: u32,
        mode: &str,
        color: (u8, u8, u8, u8),
    ) -> Result<Self, PilError> {
        let img = match mode {
            "RGB" => DynamicImage::ImageRgb8(pillow_rs_image::RgbImage::from_pixel(
                width,
                height,
                pillow_rs_image::Rgb([color.0, color.1, color.2]),
            )),
            "RGBA" => DynamicImage::ImageRgba8(pillow_rs_image::RgbaImage::from_pixel(
                width,
                height,
                pillow_rs_image::Rgba([color.0, color.1, color.2, color.3]),
            )),
            "L" => DynamicImage::ImageLuma8(pillow_rs_image::GrayImage::from_pixel(
                width,
                height,
                pillow_rs_image::Luma([color.0]),
            )),
            "LA" => DynamicImage::ImageLumaA8(pillow_rs_image::GrayAlphaImage::from_pixel(
                width,
                height,
                pillow_rs_image::LumaA([color.0, color.3]),
            )),
            "1" => DynamicImage::ImageLuma8(pillow_rs_image::GrayImage::from_pixel(
                width,
                height,
                // PIL: stores the exact pixel value (0 or 1 or 255).
                // PIL's new("1") stores the raw color value as-is.
                pillow_rs_image::Luma([color.0]),
            )),
            // P-mode: PIL creates a palette where index 0 maps to the given color,
            // with all other entries zero (not the web palette).
            "P" => {
                let mut pal = vec![0u8; 768];
                pal[0] = color.0;
                pal[1] = color.1;
                pal[2] = color.2;
                return Ok(Image::Paletted(PalettedData {
                    indices: pillow_rs_image::GrayImage::from_pixel(
                        width,
                        height,
                        pillow_rs_image::Luma([0u8]),
                    ),
                    palette: pal,
                }));
            }
            "CMYK" => DynamicImage::ImageRgba8(pillow_rs_image::RgbaImage::from_pixel(
                width,
                height,
                pillow_rs_image::Rgba([color.0, color.1, color.2, color.3]),
            )),
            "YCbCr" | "HSV" => DynamicImage::ImageRgb8(pillow_rs_image::RgbImage::from_pixel(
                width,
                height,
                pillow_rs_image::Rgb([color.0, color.1, color.2]),
            )),
            // I and F modes store 4 bytes per pixel (int32/float32 LE)
            "I" | "F" => DynamicImage::ImageRgba8(pillow_rs_image::RgbaImage::from_pixel(
                width,
                height,
                // For I mode: store 4-byte int32 LE (color.0 as low byte, others 0)
                pillow_rs_image::Rgba([color.0, 0, 0, 0]),
            )),
            _ => return Err(PilError::ValueError(format!("Unsupported mode: {}", mode))),
        };
        let explicit = if matches!(mode, "CMYK" | "YCbCr" | "HSV" | "I" | "F" | "1") {
            Some(mode.to_string())
        } else {
            None
        };
        Ok(Image::Loaded(img, explicit))
    }

    /// Creates an image from tightly packed raw bytes.
    ///
    /// `mode` uses Pillow mode names. Modes `L`, `LA`, `RGB`, `RGBA`, `CMYK`,
    /// `HSV`, `YCbCr`, `I`, `F`, and `P` expect one full pixel after another.
    /// Mode `"1"` expects Pillow's packed bitmap layout: eight pixels per byte,
    /// most-significant bit first, with each row padded to a byte boundary.
    ///
    /// Extra bytes are ignored, matching Pillow's permissive `frombytes`
    /// behavior. Too few bytes is an error.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when dimensions are zero, allocation checks fail,
    /// the mode is unsupported, or `data` is shorter than the required mode
    /// layout.
    pub fn frombytes(mode: &str, size: (u32, u32), data: &[u8]) -> Result<Self, PilError> {
        let (w, h) = size;
        if w == 0 || h == 0 {
            return Err(PilError::ValueError("frombytes: size must be > 0".into()));
        }
        let expected = match mode {
            "L" => CheckedDims::new(w, h, 1)?.total_bytes(),
            "LA" => CheckedDims::new(w, h, 2)?.total_bytes(),
            "RGB" | "HSV" | "YCbCr" => CheckedDims::new(w, h, 3)?.total_bytes(),
            "RGBA" | "CMYK" | "I" | "F" => CheckedDims::new(w, h, 4)?.total_bytes(),
            "P" => CheckedDims::new(w, h, 1)?.total_bytes(),
            "1" => (w as usize).div_ceil(8) * h as usize,
            _ => {
                return Err(PilError::ValueError(format!(
                    "frombytes: unsupported mode {}",
                    mode
                )));
            }
        };
        if data.len() < expected {
            return Err(PilError::ValueError("not enough image data".into()));
        }
        let img = match mode {
            "L" => DynamicImage::ImageLuma8(
                pillow_rs_image::GrayImage::from_raw(w, h, data[..expected].to_vec())
                    .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
            ),
            "RGB" => DynamicImage::ImageRgb8(
                pillow_rs_image::RgbImage::from_raw(w, h, data[..expected].to_vec())
                    .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
            ),
            "RGBA" => DynamicImage::ImageRgba8(
                pillow_rs_image::RgbaImage::from_raw(w, h, data[..expected].to_vec())
                    .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
            ),
            "LA" => DynamicImage::ImageLumaA8(
                pillow_rs_image::GrayAlphaImage::from_raw(w, h, data[..expected].to_vec())
                    .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
            ),
            "P" => {
                return Ok(Image::Paletted(PalettedData {
                    indices: pillow_rs_image::GrayImage::from_raw(w, h, data[..expected].to_vec())
                        .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
                    palette: default_palette(),
                }));
            }
            "CMYK" | "I" | "F" => DynamicImage::ImageRgba8(
                pillow_rs_image::RgbaImage::from_raw(w, h, data[..expected].to_vec())
                    .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
            ),
            "HSV" | "YCbCr" => DynamicImage::ImageRgb8(
                pillow_rs_image::RgbImage::from_raw(w, h, data[..expected].to_vec())
                    .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
            ),
            "1" => {
                // PIL packs 8 pixels per byte, MSB first, rows padded to byte boundary
                let row_bytes = (w as usize).div_ceil(8);
                let mut pixels = CheckedDims::new(w, h, 1)?.alloc_buffer();
                for y in 0..h as usize {
                    for x in 0..w as usize {
                        let byte_idx = y * row_bytes + x / 8;
                        let bit_idx = 7 - (x % 8); // MSB first
                        let val = if byte_idx < data.len() && (data[byte_idx] >> bit_idx) & 1 != 0 {
                            255
                        } else {
                            0
                        };
                        pixels[y * w as usize + x] = val;
                    }
                }
                DynamicImage::ImageLuma8(
                    pillow_rs_image::GrayImage::from_raw(w, h, pixels)
                        .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
                )
            }
            _ => {
                // CMYK, HSV, YCbCr, I, F stored as RGBA bytes
                let expected = (w * h * 4) as usize;
                let mut pixels = vec![0u8; expected];
                let copy_len = data.len().min(expected);
                pixels[..copy_len].copy_from_slice(&data[..copy_len]);
                DynamicImage::ImageRgba8(
                    pillow_rs_image::RgbaImage::from_raw(w, h, pixels).ok_or_else(|| {
                        PilError::ValueError("frombytes: RGBA buffer error".into())
                    })?,
                )
            }
        };
        let explicit_mode = match mode {
            "1" | "CMYK" | "HSV" | "YCbCr" | "I" | "F" => Some(mode.to_string()),
            _ => None,
        };
        Ok(Image::Loaded(img, explicit_mode))
    }

    /// Creates a lazy image from a filesystem path.
    ///
    /// The image is decoded when pixel data is first needed. PNG palette data
    /// may be inspected eagerly so P-mode behavior can be preserved.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when the provided format string is unknown or
    /// palette probing requires I/O that fails.
    pub fn open(path: &str, format: Option<&str>) -> Result<Self, PilError> {
        let fmt = format
            .and_then(|f| parse_format_str(f).ok())
            .or_else(|| ImageFormat::from_path(PathBuf::from(path)).ok());
        // If this is a PNG file, check if it uses a palette (Indexed color type)
        if fmt == Some(ImageFormat::Png) {
            let file = std::fs::File::open(path).map_err(PilError::Io)?;
            let mut reader = BufReader::new(file);
            if let Ok(paletted) = decode_paletted_png_reader(&mut reader) {
                return Ok(Image::Paletted(paletted));
            }
            // Not paletted (or error) — fall through to lazy Path
        }
        Ok(Image::Path {
            path: PathBuf::from(path),
            format: fmt,
            is_paletted: false,
        })
    }

    /// Creates a lazy image from encoded image bytes.
    ///
    /// Format is detected from magic bytes. PNG palette data may be inspected
    /// eagerly so P-mode behavior can be preserved.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] if eager palette probing finds invalid data.
    pub fn open_bytes(data: Vec<u8>) -> Result<Self, PilError> {
        let format = detect_format_from_magic(&data);
        // If this is a PNG file, check if it uses a palette (Indexed color type)
        if format == Some(ImageFormat::Png) {
            let mut cursor = std::io::Cursor::new(&data);
            if let Ok(paletted) = decode_paletted_png_reader(&mut cursor) {
                return Ok(Image::Paletted(paletted));
            }
            // Not paletted — fall through to lazy Bytes
        }
        Ok(Image::Bytes {
            data: Arc::new(data),
            format,
            is_paletted: false,
        })
    }

    // ── Materialize ──

    /// Execute the pipeline chain and return a decoded DynamicImage.
    /// This is where all the lazy work gets done.
    /// Check whether an op can be applied directly to palette indices (P-mode),
    /// versus needing actual RGB color values. Ops that operate on single-channel
    /// pixel values (indices) are safe. Color-dependent ops (filters, enhance,
    /// convert) need RGB.
    fn is_palette_safe_op(op: &PipelineOp) -> bool {
        match op {
            // Geometry ops — operate on pixels regardless of meaning
            PipelineOp::Crop { .. } => true,
            PipelineOp::Resize { filter, .. } => {
                matches!(filter, ResampleFilter::Nearest | ResampleFilter::Box)
            }
            PipelineOp::Rotate { .. } => true, // Always uses nearest for P-mode
            PipelineOp::Transpose { .. } => true,
            PipelineOp::Transform { .. } => true, // Always uses nearest for P-mode
            PipelineOp::EffectSpread { .. } => true,
            PipelineOp::Reduce { .. } => true,
            PipelineOp::Thumbnail { filter, .. } => {
                matches!(filter, ResampleFilter::Nearest | ResampleFilter::Box)
            }

            // Value ops — apply function/LUT to each pixel value (index)
            PipelineOp::PointOp { .. } => true,
            PipelineOp::Color3DLut { .. } => false, // Needs actual color values, not palette indices
            PipelineOp::Invert => true,
            PipelineOp::InvertChops => true,
            PipelineOp::Eval { .. } => true,
            PipelineOp::PutPixel { .. } => true,
            PipelineOp::PutData { .. } => true,
            PipelineOp::PutAlpha { .. } => true,

            // Duplicate / Constant / Offset — value-safe
            PipelineOp::Duplicate => true,
            PipelineOp::Constant { .. } => true,
            PipelineOp::Offset { .. } => true,

            // Blend / Composite — blend index values
            PipelineOp::Blend { .. } => true,
            PipelineOp::BlendModule { .. } => true,
            PipelineOp::Composite { .. } => true,
            PipelineOp::CompositeModule { .. } => true,

            // Draw ops — palette-safe (write single-byte index values directly)
            PipelineOp::DrawLine { .. }
            | PipelineOp::DrawRectangle { .. }
            | PipelineOp::DrawRoundedRect { .. }
            | PipelineOp::DrawEllipse { .. }
            | PipelineOp::DrawCircle { .. }
            | PipelineOp::DrawPolygon { .. }
            | PipelineOp::DrawArc { .. }
            | PipelineOp::DrawChord { .. }
            | PipelineOp::DrawPieslice { .. }
            | PipelineOp::DrawPoint { .. } => true,

            // Enhance ops — NOT safe for P-mode (need color)
            // Filter ops — NOT safe (need color)
            // Convert / Quantize — NOT safe (change mode)
            _ => false,
        }
    }

    /// Decodes or executes this image into a concrete pixel buffer.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when lazy decoding, pipeline execution, or format
    /// conversion fails.
    pub fn materialize(&self) -> Result<DynamicImage, PilError> {
        match self {
            Image::Loaded(img, _) => Ok(img.clone()),
            Image::Paletted(data) => Ok(DynamicImage::ImageLuma8(data.indices.clone())),
            Image::Path { path, .. } => {
                let file_data = std::fs::read(path).map_err(PilError::Io)?;
                let decoded = pillow_rs_image::decode(&file_data)
                    .ok_or_else(|| PilError::UnidentifiedImageError(path.display().to_string()))?;
                DynamicImage::from_decoded(&decoded)
                    .ok_or_else(|| PilError::ValueError("decode buffer error".into()))
            }
            Image::Bytes { data, .. } => {
                let decoded = pillow_rs_image::decode(data)
                    .ok_or_else(|| PilError::UnidentifiedImageError("unknown format".into()))?;
                DynamicImage::from_decoded(&decoded)
                    .ok_or_else(|| PilError::ValueError("decode buffer error".into()))
            }
            Image::Pipeline {
                source,
                ops,
                explicit_mode,
                backend,
                palette: _palette,
                ..
            } => {
                let mut img = source.materialize()?;
                // At execution time: if source was Paletted, the materialized Luma8
                // holds palette indices. For palette-safe ops, operate on indices
                // directly (preserving P-mode). For other ops, convert to RGB so
                // filters, enhance, etc. work on actual colors.
                let is_p_mode =
                    matches!(**source, Image::Paletted(_)) || source.explicit_mode() == Some("P");
                if is_p_mode {
                    let all_safe = ops.iter().all(Self::is_palette_safe_op);
                    if all_safe {
                        // Operate directly on palette indices (Luma8 = index bytes)
                        let b = backend.unwrap_or_else(|| crate::compute::route(ops, None));
                        img = crate::compute::execute_batch(b, ops, &img, Some("P"))?;
                        return Ok(img);
                    }
                    // Non-safe ops: convert to RGB
                    if let Some(rgb) = source.paletted_to_rgb() {
                        img = rgb;
                    } else {
                        // Pipeline with P-mode Loaded source: indices in img (Luma8),
                        // palette stored on pipeline. Convert indices to RGB.
                        let palette = _palette.clone().or_else(|| source.palette());
                        if let Some(palette) = palette {
                            let (w, h) = (img.width(), img.height());
                            let indices = img.to_luma8();
                            let rgb = pillow_rs_image::RgbImage::from_fn(w, h, |x, y| {
                                let idx = indices.get_pixel(x, y)[0] as usize;
                                let p = idx * 3;
                                let r = palette.get(p).copied().unwrap_or(0);
                                let g = palette.get(p + 1).copied().unwrap_or(0);
                                let b = palette.get(p + 2).copied().unwrap_or(0);
                                pillow_rs_image::Rgb([r, g, b])
                            });
                            img = DynamicImage::ImageRgb8(rgb);
                        }
                    }
                }

                // Determine the backend for this pipeline.
                // Explicit override OR auto-select: first active backend that supports ALL ops.
                let b = backend.unwrap_or_else(|| crate::compute::route(ops, None));

                img = crate::compute::execute_batch(b, ops, &img, explicit_mode.as_deref())?;
                Ok(img)
            }
        }
    }

    /// Materializes a `P` image as palette index bytes.
    ///
    /// Paletted images return a one-byte-per-pixel `Luma8` buffer containing
    /// palette indices rather than RGB colors. Non-paletted images delegate to
    /// [`Image::materialize`].
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when lazy decoding or pipeline execution fails.
    pub fn materialize_indices(&self) -> Result<DynamicImage, PilError> {
        match self {
            Image::Paletted(data) => Ok(DynamicImage::ImageLuma8(data.indices.clone())),
            Image::Pipeline {
                source,
                ops,
                explicit_mode,
                backend,
                ..
            } if matches!(**source, Image::Paletted(_))
                || explicit_mode.as_deref() == Some("P") =>
            {
                let mut img = source.materialize()?; // Paletted → Luma8 (indices)
                // Check if all ops are palette-safe
                if ops.iter().all(Self::is_palette_safe_op) {
                    let b = backend.unwrap_or_else(|| crate::compute::route(ops, None));
                    img = crate::compute::execute_batch(b, ops, &img, Some("P"))?;
                    Ok(img)
                } else {
                    // Fall back to normal materialize (converts to RGB)
                    self.materialize()
                }
            }
            _ => self.materialize(),
        }
    }

    // ── Pipeline ops ──

    /// Returns a lazy image with `op` appended to its pipeline.
    ///
    /// Existing pipelines are extended in order. Non-pipeline images become the
    /// source of a new pipeline. Mode metadata and palettes are carried forward
    /// when the operation can preserve them; operations that fundamentally
    /// change mode clear or replace the explicit mode tag.
    pub fn push_op(source: &Image, op: PipelineOp) -> Image {
        // Ops that change the image mode fundamentally should clear explicit_mode
        let explicit_mode = match &op {
            PipelineOp::Grayscale | PipelineOp::Convert { .. } | PipelineOp::Quantize { .. } => {
                None
            }
            // Draw ops: preserve explicit_mode for P-mode (draw on palette indices),
            // but clear for other modes (draw produces RGBA).
            PipelineOp::DrawLine { .. }
            | PipelineOp::DrawRectangle { .. }
            | PipelineOp::DrawRoundedRect { .. }
            | PipelineOp::DrawEllipse { .. }
            | PipelineOp::DrawCircle { .. }
            | PipelineOp::DrawPolygon { .. }
            | PipelineOp::DrawArc { .. }
            | PipelineOp::DrawChord { .. }
            | PipelineOp::DrawPieslice { .. }
            | PipelineOp::DrawPoint { .. } => {
                if source.explicit_mode() == Some("P") || matches!(source, Image::Paletted(_)) {
                    Some("P".to_string())
                } else {
                    None
                }
            }
            _ => source.explicit_mode().map(|s| s.to_string()),
        };
        let source_palette = source.extract_palette();
        match source {
            Image::Pipeline {
                source,
                ops,
                format,
                ..
            } => {
                let mut new_ops = ops.clone();
                new_ops.push(op);
                Image::Pipeline {
                    source: Arc::clone(source),
                    ops: new_ops,
                    format: *format,
                    explicit_mode,
                    backend: source.backend(),
                    palette: source_palette.or_else(|| source.palette()),
                }
            }
            other => {
                let fmt = match other {
                    Image::Pipeline { format, .. } => *format,
                    _ => None,
                };
                Image::Pipeline {
                    source: Arc::new(other.clone()),
                    ops: vec![op],
                    format: fmt,
                    explicit_mode,
                    backend: other.backend(),
                    palette: source_palette,
                }
            }
        }
    }

    // ── Immediate ops (force materialize) ──

    /// Returns one pixel as an RGBA tuple.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::IndexError`] when coordinates are outside the image.
    pub fn getpixel(&self, x: u32, y: u32) -> Result<(u8, u8, u8, u8), PilError> {
        let (w, h) = self.size()?;
        if x >= w || y >= h {
            return Err(PilError::IndexError("image index out of range".into()));
        }
        let img = self.materialize()?;
        let rgba = img.get_pixel(x, y).0;
        Ok((
            rgba[0],
            rgba.get(1).copied().unwrap_or(0),
            rgba.get(2).copied().unwrap_or(0),
            rgba.get(3).copied().unwrap_or(255),
        ))
    }

    /// Queues an in-place single-pixel write.
    ///
    /// The color is normalized as `(r, g, b, a)` before entering the pipeline.
    /// Materialization performs the mode-specific storage conversion.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::IndexError`] when coordinates are outside the image,
    /// or another [`PilError`] when image size lookup requires decoding and that
    /// fails.
    pub fn putpixel(&mut self, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) -> Result<(), PilError> {
        let (w, h) = self.size()?;
        if x >= w || y >= h {
            return Err(PilError::IndexError("image index out of range".into()));
        }
        let new_self = Image::push_op(
            self,
            PipelineOp::PutPixel {
                x,
                y,
                color: (r, g, b, a),
            },
        );
        *self = new_self;
        Ok(())
    }

    /// Queues a Pillow-style single-integer pixel write.
    ///
    /// Binding crates use this path when host input is a scalar instead of a
    /// color tuple. `mode` decides how the scalar expands into the internal
    /// RGBA tuple before the same [`Image::putpixel`] pipeline path is used.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Image::putpixel`].
    pub fn putpixel_mode(&mut self, x: u32, y: u32, v: u8, mode: &str) -> Result<(), PilError> {
        let (r, g, b, a) = match mode {
            "L" | "1" | "P" => (v, v, v, 255),
            "LA" => (v, 0, 0, 0),
            "RGB" => (v, 0, 0, 255),
            "RGBA" | "CMYK" => (v, 0, 0, 0),
            _ => (v, v, v, 255),
        };
        self.putpixel(x, y, r, g, b, a)
    }

    /// Returns Pillow-compatible image statistics in structured form.
    ///
    /// Single-band results use scalar [`StatValue`] variants. Multi-band
    /// results use list variants in band order.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization fails.
    pub fn stat_formatted(&self) -> Result<StatResult, PilError> {
        let bands = self.stat()?;
        Ok(StatResult::from_bands(&bands))
    }

    /// Computes per-band statistics.
    ///
    /// The return value is indexed by band. Each band contains:
    /// `[count, sum, sum2, mean, median, rms, var, stddev, min, max]`.
    /// Integer (`"I"`) and floating (`"F"`) modes follow Pillow's histogram
    /// fallback behavior instead of reporting raw numeric-domain statistics.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization fails.
    pub fn stat(&self) -> Result<Vec<Vec<f64>>, PilError> {
        let explicit_mode = self.explicit_mode();
        let is_f = explicit_mode == Some("F");
        let is_i = explicit_mode == Some("I");

        if is_f || is_i {
            // F mode: float32, I mode: int32. Both are single-band values
            // stored as 4 RGBA bytes per pixel. PIL's Stat uses a 256-bin
            // histogram with linear scaling from [min, max] to [0, 255]:
            //   bin = (int)((value - min) * 255 / (max - min))
            // Stats are computed from bin indices, not original values.
            let img = self.materialize()?;
            let rgba = img.as_bytes();
            let n_pixels = rgba.len() / 4;
            if n_pixels == 0 {
                return Ok(vec![vec![0.0; 10]]);
            }
            let mut values: Vec<f64> = Vec::with_capacity(n_pixels);
            for i in 0..n_pixels {
                let base = i * 4;
                let bytes: [u8; 4] = [rgba[base], rgba[base + 1], rgba[base + 2], rgba[base + 3]];
                if is_f {
                    values.push(f32::from_le_bytes(bytes) as f64);
                } else {
                    values.push(i32::from_le_bytes(bytes) as f64);
                }
            }
            let mut sorted = values.clone();
            sorted.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let min_val = sorted[0];
            let max_val = sorted[sorted.len() - 1];
            if (max_val - min_val).abs() < f64::EPSILON {
                return Ok(vec![vec![
                    n_pixels as f64,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                ]]);
            }
            let scale = 255.0 / (max_val - min_val);
            let mut hist = [0i64; 256];
            for &v in &values {
                let bin = ((v - min_val) * scale) as usize;
                if bin < 256 {
                    hist[bin] += 1;
                }
            }
            let count = n_pixels as f64;
            let sum: f64 = hist
                .iter()
                .enumerate()
                .map(|(i, &c)| i as f64 * c as f64)
                .sum();
            let sum2: f64 = hist
                .iter()
                .enumerate()
                .map(|(i, &c)| (i as f64) * (i as f64) * c as f64)
                .sum();
            let mean = sum / count;
            let rms = (sum2 / count).sqrt();
            let var = (sum2 - sum * sum / count) / count;
            let var = if var < 0.0 { 0.0 } else { var };
            let stddev = var.sqrt();
            let mut cum = 0i64;
            let half = (count / 2.0) as i64;
            let mut median = 0.0;
            for (i, &c) in hist.iter().enumerate() {
                cum += c;
                if cum > half {
                    median = i as f64;
                    break;
                }
            }
            let mut min_bin = 255usize;
            let mut max_bin = 0usize;
            for (i, &c) in hist.iter().enumerate() {
                if c > 0 {
                    min_bin = min_bin.min(i);
                    max_bin = max_bin.max(i);
                }
            }
            return Ok(vec![vec![
                count,
                sum,
                sum2,
                mean,
                median,
                rms,
                var,
                stddev,
                min_bin as f64,
                max_bin as f64,
            ]]);
        }

        let img = self.materialize()?;
        let n_bands = img.color().channel_count() as usize;
        let (w, h) = (img.width() as usize, img.height() as usize);
        let n_pixels = w * h;

        // Extract bands correctly for each image type
        let mut bands: Vec<Vec<u8>> = vec![Vec::with_capacity(n_pixels); n_bands];

        match n_bands {
            1 => {
                let gray = img.to_luma8();
                for px in gray.pixels() {
                    bands[0].push(px[0]);
                }
            }
            2 => {
                // LA mode: channel 0 = L (from R), channel 1 = A (from A)
                let rgba = img.to_rgba8();
                for px in rgba.pixels() {
                    bands[0].push(px[0]); // L = R
                    bands[1].push(px[3]); // A = A
                }
            }
            3 => {
                let rgb = img.to_rgb8();
                for px in rgb.pixels() {
                    bands[0].push(px[0]);
                    bands[1].push(px[1]);
                    bands[2].push(px[2]);
                }
            }
            _ => {
                let rgba = img.to_rgba8();
                for px in rgba.pixels() {
                    for b in 0..4 {
                        bands[b].push(px[b]);
                    }
                }
            }
        }

        for b in bands.iter_mut() {
            b.sort_unstable();
        }

        let mut results = Vec::with_capacity(n_bands);
        for band in &bands {
            let count = band.len() as f64;
            if count == 0.0 {
                results.push(vec![0.0; 10]);
                continue;
            }
            let sum: f64 = band.iter().map(|&x| x as f64).sum();
            let sum2: f64 = band.iter().map(|&x| (x as f64) * (x as f64)).sum();
            let mean = sum / count;
            let rms = (sum2 / count).sqrt();
            // PIL computes variance as: (sum2 - sum*sum/count) / count — avoids rms rounding
            let var = (sum2 - sum * sum / count) / count;
            let var = if var < 0.0 { 0.0 } else { var };
            let stddev = var.sqrt();
            let min = band[0] as f64;
            let max = band[band.len() - 1] as f64;
            let median = band[band.len() / 2] as f64;
            results.push(vec![
                count, sum, sum2, mean, median, rms, var, stddev, min, max,
            ]);
        }
        Ok(results)
    }

    /// Returns Pillow band names for the current image mode.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when lazy image data must be decoded and decoding
    /// fails.
    pub fn getbands(&self) -> Result<Vec<String>, PilError> {
        if matches!(self, Image::Paletted(_)) {
            return Ok(vec!["P".to_string()]);
        }
        // Check explicit mode for non-standard band names
        if let Image::Loaded(_, Some(m)) = self {
            let bands: Vec<String> = match m.as_str() {
                "CMYK" => vec![
                    "C".to_string(),
                    "M".to_string(),
                    "Y".to_string(),
                    "K".to_string(),
                ],
                "YCbCr" => vec!["Y".to_string(), "Cb".to_string(), "Cr".to_string()],
                "HSV" => vec!["H".to_string(), "S".to_string(), "V".to_string()],
                "I" | "F" | "P" | "1" => vec![m.clone()],
                _ => vec![],
            };
            if !bands.is_empty() {
                return Ok(bands.iter().map(|s| s.to_string()).collect());
            }
        }
        let img = self.materialize()?;
        let bands = match img.color().channel_count() {
            1 => vec!["L".to_string()],
            2 => vec!["L".to_string(), "A".to_string()],
            3 => vec!["R".to_string(), "G".to_string(), "B".to_string()],
            4 => vec![
                "R".to_string(),
                "G".to_string(),
                "B".to_string(),
                "A".to_string(),
            ],
            _ => vec!["?".to_string()],
        };
        Ok(bands)
    }

    /// Encodes and writes the image to a filesystem path.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when format detection, encoding, materialization, or
    /// filesystem writing fails.
    pub fn save(&self, path: &str, format: Option<&str>) -> Result<(), PilError> {
        // Paletted images: convert via the palette to RGB for visual correctness
        let img = self.paletted_to_rgb().unwrap_or(self.materialize()?);
        let save_format = if let Some(fmt) = format {
            parse_format_str(fmt)?
        } else {
            ImageFormat::from_path(path)
                .map_err(|_| PilError::UnknownFormat("Cannot determine format from path".into()))?
        };
        let encoded = pillow_rs_image::encode_default(&img.into_decoded(), save_format)
            .ok_or_else(|| PilError::UnknownFormat("encoding failed".into()))?;
        std::fs::write(path, encoded).map_err(PilError::Io)?;
        Ok(())
    }

    /// Returns raw image bytes in the image's current Pillow mode.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization or mode-specific byte packing
    /// fails.
    pub fn tobytes(&self) -> Result<Vec<u8>, PilError> {
        self.tobytes_formatted(self.explicit_mode().unwrap_or(""))
    }

    /// Returns raw image bytes using an explicit Pillow mode override.
    ///
    /// This is the binding-facing form of [`Image::tobytes`]. Modes `"F"` and
    /// `"I"` return the internal little-endian 4-byte scalar representation.
    /// Mode `"1"` packs pixels eight per byte, most-significant bit first.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization or mode-specific byte packing
    /// fails.
    pub fn tobytes_formatted(&self, mode: &str) -> Result<Vec<u8>, PilError> {
        // Fast path for Paletted: return raw index bytes
        if let Image::Paletted(data) = self {
            return Ok(data.indices.as_raw().to_vec());
        }
        let img = self.materialize()?;

        // F/I modes: stored as RGBA8 internally (f32/i32 LE bytes packed as RGBA).
        // Read the raw bytes directly — img.as_bytes() already contains the correct
        // f32/i32 LE representation (4 bytes per pixel).
        if matches!(mode, "F" | "I") {
            return Ok(img.as_bytes().to_vec());
        }

        // For mode "1" images, pack 8 pixels per byte (MSB first) matching PIL.
        if mode == "1" && img.color() == pillow_rs_image::ColorType::L8 {
            let gray = img.to_luma8();
            let (w, h) = gray.dimensions();
            let row_bytes = w.div_ceil(8) as usize;
            let mut packed = vec![0u8; row_bytes * h as usize];
            for y in 0..h as usize {
                for x in 0..w as usize {
                    let pixel = gray.get_pixel(x as u32, y as u32)[0];
                    if pixel != 0 {
                        let byte_idx = y * row_bytes + x / 8;
                        let bit_idx = 7 - (x % 8);
                        packed[byte_idx] |= 1 << bit_idx;
                    }
                }
            }
            return Ok(packed);
        }
        Ok(img.as_bytes().to_vec())
    }

    /// Locks this image pipeline to one compute backend.
    ///
    /// The backend choice is applied when the image is materialized. Non-pipeline
    /// images are returned unchanged because there is no deferred work to route.
    pub fn use_backend(mut self, b: crate::compute::Backend) -> Image {
        if let Image::Pipeline { backend, .. } = &mut self {
            *backend = Some(b);
        }
        self
    }

    /// Returns the backend explicitly locked on this pipeline, if any.
    pub fn backend(&self) -> Option<crate::compute::Backend> {
        match self {
            Image::Pipeline { backend, .. } => *backend,
            _ => None,
        }
    }

    /// Returns the explicit Pillow mode tag carried by this image.
    ///
    /// Some Pillow modes cannot be represented by `pillow-rs-image` color types
    /// alone. This method exposes the side-channel mode tag for modes such as
    /// `"1"`, `"P"`, `"CMYK"`, `"HSV"`, `"YCbCr"`, `"I"`, and `"F"`.
    pub fn explicit_mode(&self) -> Option<&str> {
        match self {
            Image::Loaded(_, Some(m)) => Some(m.as_str()),
            Image::Paletted(_) => Some("P"),
            Image::Pipeline {
                explicit_mode: Some(m),
                ..
            } => Some(m.as_str()),
            _ => None,
        }
    }

    /// Returns palette data for `P` images as RGB triples.
    ///
    /// The full palette is 768 bytes: 256 entries of `R, G, B`. Pipeline images
    /// can carry a copied palette so palette-safe operations preserve `P` mode.
    pub fn palette(&self) -> Option<Vec<u8>> {
        match self {
            Image::Paletted(data) => Some(data.palette.clone()),
            Image::Pipeline { palette, .. } => palette.clone(),
            _ => None,
        }
    }

    /// Returns palette data trimmed like Pillow's `getpalette`.
    ///
    /// Trailing all-zero RGB triples are removed. A full palette may still return
    /// 768 bytes when later entries are non-zero.
    pub fn getpalette_trimmed(&self) -> Option<Vec<u8>> {
        let raw = self.palette()?;
        let mut last = raw.len();
        while last >= 3 && raw[last - 3] == 0 && raw[last - 2] == 0 && raw[last - 1] == 0 {
            last = last.saturating_sub(3);
        }
        Some(raw[..last].to_vec())
    }

    /// Applies stored transparency information when available.
    ///
    /// Current core storage does not keep a separate palette transparency table.
    /// For [`Image::Paletted`] values this expands indices to RGBA pixels with
    /// opaque alpha; other image variants are unchanged.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; the result type is preserved for Pillow API
    /// compatibility and future metadata-backed transparency handling.
    pub fn apply_transparency(&mut self) -> Result<(), PilError> {
        // Extract Paletted data via clone to avoid borrow issues with self-mutation
        let pal_data = match self {
            Image::Paletted(data) => Some((data.indices.clone(), data.palette.clone())),
            _ => None,
        };

        if let Some((indices, palette)) = pal_data {
            let (w, h) = indices.dimensions();
            let rgba = pillow_rs_image::RgbaImage::from_fn(w, h, |x, y| {
                let idx = indices.get_pixel(x, y)[0] as usize;
                let base = idx * 3;
                let r = palette.get(base).copied().unwrap_or(0);
                let g = palette.get(base + 1).copied().unwrap_or(0);
                let b = palette.get(base + 2).copied().unwrap_or(0);
                pillow_rs_image::Rgba([r, g, b, 255])
            });
            *self = Image::Loaded(DynamicImage::ImageRgba8(rgba), None);
        }
        Ok(())
    }

    /// Returns child images for multi-frame formats.
    ///
    /// Multi-frame decoding is not implemented in core yet, so this currently
    /// returns an empty vector for all inputs.
    pub fn get_child_images(&self) -> Vec<Image> {
        vec![]
    }

    /// Returns EXIF metadata bytes.
    ///
    /// Core does not currently parse EXIF metadata from encoded containers, so
    /// this returns an empty byte vector.
    pub fn getexif(&self) -> Vec<u8> {
        // Return empty vec — no EXIF data extracted. PIL returns empty Exif dict {}.
        // Full EXIF extraction would need TIFF/EXIF parsing from JPEG/HEIF headers.
        Vec::new()
    }

    /// Returns XMP metadata fields.
    ///
    /// Core does not currently parse XMP packets from encoded containers, so
    /// this returns an empty map.
    pub fn getxmp(&self) -> std::collections::HashMap<String, String> {
        std::collections::HashMap::new()
    }

    /// Returns Pillow's internal-image handle equivalent.
    ///
    /// Pure Rust core has no C `Imaging` pointer or capsule to expose, so this
    /// returns `None`. Binding crates may format that as a Pillow-compatible
    /// placeholder.
    pub fn getim(&self) -> Option<u64> {
        None
    }

    /// Extract palette from Paletted variant (for Pipeline propagation).
    fn extract_palette(&self) -> Option<Vec<u8>> {
        match self {
            Image::Paletted(data) => Some(data.palette.clone()),
            Image::Pipeline { palette, .. } => palette.clone(),
            _ => None,
        }
    }

    /// Encodes the image as PNG bytes.
    ///
    /// Paletted images are rendered through their palette before encoding so
    /// saved output contains visible RGB colors rather than raw index bytes.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization or PNG encoding fails.
    pub fn to_png_bytes(&self) -> Result<Vec<u8>, PilError> {
        match self.paletted_to_rgb() {
            Some(img) => {
                let encoded =
                    pillow_rs_image::encode_default(&img.into_decoded(), ImageFormat::Png)
                        .ok_or_else(|| PilError::UnknownFormat("PNG encode failed".into()))?;
                Ok(encoded)
            }
            None => {
                let img = self.materialize()?;
                let encoded =
                    pillow_rs_image::encode_default(&img.into_decoded(), ImageFormat::Png)
                        .ok_or_else(|| PilError::UnknownFormat("PNG encode failed".into()))?;
                Ok(encoded)
            }
        }
    }

    /// Materializes the image for color-space image operations.
    ///
    /// `P` images are converted through their palette to RGB so operations such
    /// as paste, composite, and filters operate on visible colors. Use
    /// [`Image::materialize_indices`] when an operation needs palette indices.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when lazy decoding or pipeline execution fails.
    pub fn materialize_for_ops(&self) -> Result<DynamicImage, PilError> {
        // Fast path: Paletted images have direct palette→RGB conversion
        if let Some(rgb) = self.paletted_to_rgb() {
            return Ok(rgb);
        }
        // Pipeline-based P-mode images need palette→RGB conversion
        if matches!(self, Image::Pipeline { .. }) && self.explicit_mode() == Some("P") {
            let img = self.materialize()?; // Luma8 indices
            if let Some(palette) = self.palette() {
                let (w, h) = img.dimensions();
                let indices = img.to_luma8();
                let rgb = pillow_rs_image::RgbImage::from_fn(w, h, |x, y| {
                    let idx = indices.get_pixel(x, y)[0] as usize;
                    let p = idx * 3;
                    let r = palette.get(p).copied().unwrap_or(0);
                    let g = palette.get(p + 1).copied().unwrap_or(0);
                    let b = palette.get(p + 2).copied().unwrap_or(0);
                    pillow_rs_image::Rgb([r, g, b])
                });
                return Ok(DynamicImage::ImageRgb8(rgb));
            }
        }
        self.materialize()
    }

    /// Convert Paletted image to RGB for rendering/saving. Returns None for non-Paletted.
    pub(crate) fn paletted_to_rgb(&self) -> Option<DynamicImage> {
        if let Image::Paletted(data) = self {
            let rgb = pillow_rs_image::RgbImage::from_fn(
                data.indices.width(),
                data.indices.height(),
                |x, y| {
                    let idx = data.indices.get_pixel(x, y)[0] as usize;
                    let p = idx * 3;
                    let r = data.palette.get(p).copied().unwrap_or(0);
                    let g = data.palette.get(p + 1).copied().unwrap_or(0);
                    let b = data.palette.get(p + 2).copied().unwrap_or(0);
                    pillow_rs_image::Rgb([r, g, b])
                },
            );
            Some(DynamicImage::ImageRgb8(rgb))
        } else {
            None
        }
    }

    /// Returns image dimensions as `(width, height)` in pixels.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when a lazy image must be decoded and decoding
    /// fails.
    pub fn size(&self) -> Result<(u32, u32), PilError> {
        if let Image::Paletted(data) = self {
            return Ok(data.indices.dimensions());
        }
        let img = self.materialize()?;
        Ok((img.width(), img.height()))
    }

    /// Returns the Pillow mode string for this image.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when lazy data must be materialized to determine
    /// the mode and that materialization fails.
    pub fn mode(&self) -> Result<String, PilError> {
        if matches!(self, Image::Paletted(_)) {
            return Ok("P".to_string());
        }
        if let Image::Loaded(_, Some(m)) = self {
            return Ok(m.clone());
        }
        if let Image::Pipeline {
            explicit_mode: Some(m),
            ..
        } = self
        {
            return Ok(m.clone());
        }
        let img = self.materialize()?;
        // Check format-based mode for Path/Bytes
        let (fmt, is_paletted) = match self {
            Image::Path {
                format,
                is_paletted: ip,
                ..
            } => (*format, *ip),
            Image::Bytes {
                format,
                is_paletted: ip,
                ..
            } => (*format, *ip),
            _ => (None, false),
        };
        let mut detected = detect_format_mode(&img, fmt);
        if detected.is_none() && is_paletted {
            detected = Some("P".to_string());
        }
        if let Some(d) = detected {
            return Ok(d);
        }
        Ok(color_type_to_mode(img.color()).to_string())
    }

    /// Returns the known image format name, if the image came from encoded input.
    pub fn format_name(&self) -> Option<String> {
        match self {
            Image::Loaded(_, _) | Image::Paletted(_) => None,
            Image::Path { format, .. } => format.map(|f| format!("{:?}", f).to_uppercase()),
            Image::Bytes { format, .. } => format.map(|f| format!("{:?}", f).to_uppercase()),
            Image::Pipeline { format, .. } => format.map(|f| format!("{:?}", f).to_uppercase()),
        }
    }

    /// Forces lazy decoding and pipeline execution.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization fails.
    pub fn load(&self) -> Result<(), PilError> {
        self.materialize()?;
        Ok(())
    }

    /// Returns a deep clone of the image handle and its current lazy state.
    pub fn copy(&self) -> Self {
        self.clone()
    }

    /// Returns pixel data as a flat sequence of channel bytes.
    ///
    /// When `band` is `Some`, returns only that channel. Negative bands are not
    /// interpreted here; callers should pass a non-negative band index. Without
    /// `band`, bytes are returned in the current decoded image channel order.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization fails.
    pub fn getdata(&self, band: Option<i32>) -> Result<Vec<u8>, PilError> {
        let img = self.materialize()?;
        let band = band.unwrap_or(-1);
        if band >= 0 {
            let rgba = img.to_rgba8();
            let b = band.min(3) as usize;
            return Ok(rgba.pixels().map(|p| p[b]).collect());
        }
        match img.color() {
            pillow_rs_image::ColorType::L8 | pillow_rs_image::ColorType::L16 => {
                let gray = img.to_luma8();
                Ok(gray.into_raw())
            }
            pillow_rs_image::ColorType::La8 | pillow_rs_image::ColorType::La16 => {
                let ga = img.to_luma_alpha8();
                let mut out = Vec::with_capacity((ga.width() * ga.height() * 2) as usize);
                for p in ga.pixels() {
                    out.push(p[0]);
                    out.push(p[1]);
                }
                Ok(out)
            }
            pillow_rs_image::ColorType::Rgb8
            | pillow_rs_image::ColorType::Rgb16
            | pillow_rs_image::ColorType::Rgb32F => {
                let rgb = img.to_rgb8();
                Ok(rgb.into_raw())
            }
            _ => {
                let rgba = img.to_rgba8();
                Ok(rgba.into_raw())
            }
        }
    }

    /// Queues replacement pixel data from a flat byte sequence.
    ///
    /// `data` must match the active image mode and dimensions expected by the
    /// pipeline operation. Validation happens when the pipeline is materialized.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; materialization reports invalid byte length
    /// or mode mismatches.
    pub fn putdata(&mut self, data: &[u8]) -> Result<(), PilError> {
        let new_self = Image::push_op(
            self,
            PipelineOp::PutData {
                data: data.to_vec(),
            },
        );
        *self = new_self;
        Ok(())
    }

    /// Extracts one channel as an `L` image.
    ///
    /// Negative indices count from the end, matching Pillow band indexing.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when `channel` is out of range, or
    /// another [`PilError`] when materialization fails.
    pub fn getchannel(&self, channel: i32) -> Result<Image, PilError> {
        // Validate channel index (requires materialized image for band count)
        let img = self.materialize()?;
        let bands = img.color().channel_count();
        let ch = if channel < 0 {
            (bands as i32 + channel) as usize
        } else {
            channel as usize
        };
        if ch >= bands as usize {
            return Err(PilError::ValueError(format!(
                "Channel {} out of range (0-{})",
                channel,
                bands - 1
            )));
        }
        // Defer extraction via pipeline
        Ok(Image::push_op(
            self,
            PipelineOp::ExtractBand { index: ch as u8 },
        ))
    }

    /// Queues replacement of the alpha channel.
    ///
    /// Materialization preserves Pillow-style mode intent: `L` becomes `LA`,
    /// `RGB` becomes `RGBA`, and existing `LA`/`RGBA` alpha bytes are replaced.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; materialization reports later pipeline
    /// failures.
    pub fn putalpha(&mut self, alpha: u8) -> Result<(), PilError> {
        let new_self = Image::push_op(self, PipelineOp::PutAlpha { alpha });
        *self = new_self;
        Ok(())
    }

    /// Returns unique colors and their counts.
    ///
    /// The result is `None` when the image contains more than `maxcolors`
    /// distinct colors. Each color is returned as bytes matching the image mode.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when mode detection or materialization fails.
    #[allow(clippy::type_complexity)]
    pub fn getcolors(&self, maxcolors: u32) -> Result<Option<Vec<(u32, Vec<u8>)>>, PilError> {
        let mode = self.mode()?;
        // For 1, L, P modes, PIL uses histogram (pixel value ascending)
        if mode == "1" || mode == "L" || mode == "P" {
            return self.getcolors_histogram(maxcolors);
        }
        // For multi-channel modes, use pixel-level counting
        let img = self.materialize()?;
        let n_bands = match img.color() {
            pillow_rs_image::ColorType::L8 | pillow_rs_image::ColorType::L16 => 1,
            pillow_rs_image::ColorType::La8 | pillow_rs_image::ColorType::La16 => 2,
            pillow_rs_image::ColorType::Rgb8 | pillow_rs_image::ColorType::Rgb16 => 3,
            _ => 4,
        };
        let mut counts: std::collections::HashMap<Vec<u8>, u32> = std::collections::HashMap::new();
        match n_bands {
            2 => {
                let la = img.to_luma_alpha8();
                for p in la.pixels() {
                    let key = vec![p[0], p[1]];
                    *counts.entry(key).or_insert(0) += 1;
                }
            }
            _ => {
                let rgba = img.to_rgba8();
                for p in rgba.pixels() {
                    let key: Vec<u8> = p.0[..n_bands].to_vec();
                    *counts.entry(key).or_insert(0) += 1;
                }
            }
        }
        if counts.len() > maxcolors as usize {
            return Ok(None);
        }
        let mut result: Vec<_> = counts.into_iter().map(|(k, v)| (v, k)).collect();
        // PIL sorts by color value descending.
        // For LA mode, PIL's C getcolors32 produces odds-descending then evens-descending
        // (due to its internal hash-table ordering with A+L*256 encoding).
        if n_bands == 2 {
            result.sort_by(|a, b| {
                // Primary: parity of first byte (odd first = 1 before 0)
                let a_odd = a.1[0] & 1;
                let b_odd = b.1[0] & 1;
                if a_odd != b_odd {
                    return b_odd.cmp(&a_odd);
                }
                // Secondary: full value descending
                b.1.cmp(&a.1)
            });
        } else {
            result.sort_by(|a, b| b.1.cmp(&a.1));
        }
        Ok(Some(result))
    }

    /// Histogram-based getcolors for 1, L, P modes.
    /// Matches PIL's Python-level implementation:
    ///   h = self.im.histogram()
    ///   out = [(h[i], i) for i in range(256) if h[i]]
    fn getcolors_histogram(&self, maxcolors: u32) -> Result<Option<Vec<(u32, Vec<u8>)>>, PilError> {
        let img = self.materialize()?;
        // Compute 256-bin histogram
        let mut hist = [0u32; 256];
        match img.color() {
            pillow_rs_image::ColorType::L8 | pillow_rs_image::ColorType::L16 => {
                let luma = img.to_luma8();
                for p in luma.pixels() {
                    hist[p[0] as usize] += 1;
                }
            }
            _ => {
                // For P mode and mode 1, image crate may store differently,
                // convert to luma for indexing
                let luma = img.to_luma8();
                for p in luma.pixels() {
                    hist[p[0] as usize] += 1;
                }
            }
        }
        // Build result: [(count, pixel_value)] in pixel value ascending order
        let result: Vec<(u32, Vec<u8>)> = (0..=255u8)
            .filter(|&i| hist[i as usize] > 0)
            .map(|i| (hist[i as usize], vec![i]))
            .collect();
        if result.len() > maxcolors as usize {
            return Ok(None);
        }
        Ok(Some(result))
    }

    /// Returns Shannon entropy using Pillow-compatible per-band histograms.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization fails.
    pub fn entropy(&self) -> Result<f64, PilError> {
        let img = self.materialize()?;
        let n_bands = match img.color() {
            pillow_rs_image::ColorType::L8 | pillow_rs_image::ColorType::L16 => 1,
            pillow_rs_image::ColorType::La8 | pillow_rs_image::ColorType::La16 => 2,
            pillow_rs_image::ColorType::Rgb8 | pillow_rs_image::ColorType::Rgb16 => 3,
            _ => 4,
        };
        let mut hists = vec![[0u32; 256]; n_bands];
        // Use mode-aware pixel reading (to_rgba8 remaps LA channels incorrectly for histogram)
        match img.color() {
            pillow_rs_image::ColorType::La8 | pillow_rs_image::ColorType::La16 => {
                let la = img.to_luma_alpha8();
                for px in la.pixels() {
                    hists[0][px[0] as usize] += 1;
                    hists[1][px[1] as usize] += 1;
                }
            }
            pillow_rs_image::ColorType::L8 | pillow_rs_image::ColorType::L16 => {
                let luma = img.to_luma8();
                for px in luma.pixels() {
                    hists[0][px[0] as usize] += 1;
                }
            }
            _ => {
                let rgba = img.to_rgba8();
                for px in rgba.pixels() {
                    for b in 0..n_bands {
                        hists[b][px[b] as usize] += 1;
                    }
                }
            }
        }
        let total = (img.width() * img.height() * n_bands as u32) as f64;
        let mut entropy = 0.0f64;
        for band_hist in &hists {
            for &h in band_hist {
                if h > 0 {
                    let p = h as f64 / total;
                    entropy -= p * p.log2();
                }
            }
        }
        Ok(entropy)
    }

    /// Returns horizontal and vertical non-zero pixel projections.
    ///
    /// The first vector has one entry per column and the second has one entry
    /// per row. Entries are `1` when any converted-luma pixel on that axis is
    /// non-zero, otherwise `0`.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization fails.
    pub fn getprojection(&self) -> Result<(Vec<u32>, Vec<u32>), PilError> {
        let img = self.materialize()?;
        let (w, h) = (img.width() as usize, img.height() as usize);
        let mut h_proj = vec![0u32; w];
        let mut v_proj = vec![0u32; h];
        let luma = img.to_luma8();
        for (x, y, px) in luma.enumerate_pixels() {
            if px[0] != 0 {
                h_proj[x as usize] = 1;
                v_proj[y as usize] = 1;
            }
        }
        Ok((h_proj, v_proj))
    }

    /// Converts the image to X11 bitmap (`XBM`) source bytes.
    ///
    /// Mode `"1"` treats any non-zero pixel as white. Other modes are converted
    /// through luma and thresholded at `128`.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when mode detection or materialization fails.
    pub fn tobitmap(&self) -> Result<Vec<u8>, PilError> {
        let mode = self.mode()?;
        let is_mode1 = mode == "1";
        let img = self.materialize()?;
        let gray = img.to_luma8();
        let (w, h) = (gray.width(), gray.height());
        let row_bytes = w.div_ceil(8) as usize;
        let mut bits = vec![0u8; row_bytes * h as usize];
        for y in 0..h {
            for x in 0..w {
                let v = gray.get_pixel(x, y)[0];
                // Mode "1": any non-zero value is white (PIL stores mode "1" pixels
                // as bit values where any non-zero = 1). Mode "L": threshold at 128.
                let is_white = if is_mode1 { v != 0 } else { v >= 128 };
                if is_white {
                    // PIL XBM: 1 = white, 0 = black; LSB = leftmost pixel
                    let byte_idx = (x / 8) as usize;
                    let bit_idx = x % 8;
                    bits[(y as usize) * row_bytes + byte_idx] |= 1u8 << bit_idx;
                }
            }
        }
        // PIL tobitmap format: XBM C source, 15 hex values per line
        let mut xbm = String::new();
        xbm.push_str(&format!("#define image_width {}\n", w));
        xbm.push_str(&format!("#define image_height {}\n", h));
        xbm.push_str("static char image_bits[] = {\n");
        let hexes: Vec<String> = bits.iter().map(|b| format!("0x{:02x}", b)).collect();
        let total = hexes.len();
        for (i, chunk) in hexes.chunks(15).enumerate() {
            let start = i * 15;
            let end = (start + chunk.len()).min(total);
            let is_last = end >= total;
            if is_last {
                // Last line: no trailing comma
                xbm.push_str(&chunk.join(","));
            } else {
                // Full line: trailing comma
                xbm.push_str(&chunk.join(","));
                xbm.push(',');
            }
            xbm.push('\n');
        }
        xbm.push_str("};");
        Ok(xbm.into_bytes())
    }

    /// Seeks to a frame in a multi-frame image.
    ///
    /// Multi-frame decoding is not implemented in core yet, so this accepts the
    /// request and leaves the image unchanged.
    pub fn seek(&self, _frame: u32) -> Result<(), PilError> {
        Ok(())
    }

    /// Returns the current frame number.
    ///
    /// Multi-frame decoding is not implemented in core yet, so this always
    /// returns `0`.
    pub fn tell(&self) -> u32 {
        0
    }

    /// Remaps palette indices through `dest_map`.
    ///
    /// `P` images operate on palette indices directly. Other modes operate on
    /// RGB color values and return a `P`-tagged pipeline result. Values absent
    /// from `dest_map` map to `0`, matching Pillow's inverse-map behavior.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; invalid map behavior is handled during
    /// pipeline execution.
    pub fn remap_palette(&self, dest_map: &[u8]) -> Result<Image, PilError> {
        // Defer remap via pipeline — PipelineOp::RemapPalette handles P/L/RGB modes
        // PIL: remap_palette always returns a P-mode image (palette indices).
        let mut result = Image::push_op(
            self,
            PipelineOp::RemapPalette {
                dest_map: dest_map.to_vec(),
            },
        );
        // Always tag the result as P-mode, regardless of source mode.
        if let Image::Pipeline {
            explicit_mode: em, ..
        } = &mut result
        {
            *em = Some("P".to_string());
        }
        Ok(result)
    }
}
/// Decode a paletted PNG from a reader, returning the index bytes + palette.
/// Returns an error if the PNG is not paletted (Indexed color type).
fn decode_paletted_png_reader<R: Read + Seek>(r: &mut R) -> Result<PalettedData, PilError> {
    use png::ColorType;
    let mut reader = BufReader::new(r);
    let decoder = png::Decoder::new(&mut reader);
    let mut png_reader = decoder.read_info().map_err(|e| {
        PilError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            e.to_string(),
        ))
    })?;

    // Extract info upfront to release the immutable borrow before next_frame
    let (w, h, pal) = {
        let info = png_reader.info();
        if info.color_type != ColorType::Indexed {
            return Err(PilError::ValueError("not a paletted PNG".into()));
        }
        if info.width == 0 || info.height == 0 {
            return Err(PilError::ValueError("empty PNG image".into()));
        }
        let mut pal = info.palette.clone().unwrap_or_default().to_vec();
        pal.resize(768, 0u8);
        (info.width, info.height, pal)
    };

    let out_size = png_reader
        .output_buffer_size()
        .ok_or_else(|| PilError::ValueError("Could not determine output buffer size".into()))?;
    let mut buf = vec![0u8; out_size];
    png_reader.next_frame(&mut buf).map_err(|e| {
        PilError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            e.to_string(),
        ))
    })?;

    // For indexed PNGs, output buffer contains palette indices (w*h bytes).
    let max_size = (w as usize) * (h as usize);
    let indices = if buf.len() >= max_size {
        pillow_rs_image::GrayImage::from_raw(w, h, buf[..max_size].to_vec())
            .ok_or_else(|| PilError::ValueError("paletted PNG decode: buffer error".into()))?
    } else {
        return Err(PilError::ValueError(
            "paletted PNG decode: insufficient data".into(),
        ));
    };

    Ok(PalettedData {
        indices,
        palette: pal,
    })
}

/// Detects image format from magic bytes.
fn detect_format_from_magic(data: &[u8]) -> Option<ImageFormat> {
    if data.len() >= 3 && &data[..3] == b"GIF" {
        Some(ImageFormat::Gif)
    } else if data.len() >= 8 && &data[..8] == b"\x89PNG\r\n\x1a\n" {
        Some(ImageFormat::Png)
    } else if data.len() >= 2 && &data[..2] == b"BM" {
        Some(ImageFormat::Bmp)
    } else if data.len() >= 2 && &data[..2] == b"\xff\xd8" {
        Some(ImageFormat::Jpeg)
    } else {
        None
    }
}

/// Detect the correct PIL mode from format + color type after decoding.
fn detect_format_mode(img: &DynamicImage, format: Option<ImageFormat>) -> Option<String> {
    match format {
        Some(ImageFormat::Gif) => {
            // GIF: bilevel source becomes L in PIL, RGB source becomes P
            let ch = img.color().channel_count();
            if ch == 1 || ch == 2 || is_bilevel(img) {
                Some("L".to_string())
            } else {
                Some("P".to_string())
            }
        }
        Some(ImageFormat::Png) => {
            let ch = img.color().channel_count();
            if ch == 1 || ch == 2 {
                if is_bilevel(img) {
                    Some("1".to_string())
                } else if ch == 2 {
                    Some("LA".to_string())
                } else {
                    Some("L".to_string())
                }
            } else {
                None // Determined by caller via is_paletted flag
            }
        }
        Some(ImageFormat::Bmp) => {
            if is_grayscale_rgb(img) {
                Some("L".to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Check if all pixel values are exactly 0 or 255 (bilevel image).
fn is_bilevel(img: &DynamicImage) -> bool {
    let luma = img.to_luma8();
    luma.pixels().all(|p| p[0] == 0 || p[0] == 255)
}

/// Check if an RGB image is actually grayscale (all channels equal per pixel).
fn is_grayscale_rgb(img: &DynamicImage) -> bool {
    if img.color().channel_count() < 3 {
        return true;
    }
    let rgb = img.to_rgb8();
    rgb.pixels().all(|p| p[0] == p[1] && p[1] == p[2])
}

/// Preserves the input image color type after operations that may widen to RGBA.
///
/// For L/LA modes, extracts the R channel directly (GPU stores luma in R, and
/// G/B may be stale after mode-aware processing). Uses `to_luma8()`/`to_luma_alpha8()`
/// only as a fallback.
pub fn preserve_mode(original: &DynamicImage, result: DynamicImage) -> DynamicImage {
    let orig_color = original.color();
    let res_color = result.color();
    if orig_color == res_color {
        return result;
    }
    match orig_color {
        pillow_rs_image::ColorType::L8 => {
            // Extract R channel directly — GPU mode-aware shaders only update R for L mode.
            // G and B may be stale; to_luma8() weights all three channels and would be wrong.
            let rgba = result.to_rgba8();
            let (w, h) = rgba.dimensions();
            let luma: Vec<u8> = rgba.pixels().map(|px| px[0]).collect();
            DynamicImage::ImageLuma8(
                pillow_rs_image::GrayImage::from_raw(w, h, luma)
                    .unwrap_or_else(|| result.to_luma8()),
            )
        }
        pillow_rs_image::ColorType::La8 => {
            // Extract R (luma) and A (alpha) directly.
            let rgba = result.to_rgba8();
            let (w, h) = rgba.dimensions();
            let la: Vec<u8> = rgba.pixels().flat_map(|px| [px[0], px[3]]).collect();
            DynamicImage::ImageLumaA8(
                pillow_rs_image::GrayAlphaImage::from_raw(w, h, la)
                    .unwrap_or_else(|| result.to_luma_alpha8()),
            )
        }
        pillow_rs_image::ColorType::Rgb8 => DynamicImage::ImageRgb8(result.to_rgb8()),
        pillow_rs_image::ColorType::Rgba8 => DynamicImage::ImageRgba8(result.to_rgba8()),
        _ => result,
    }
}
/// Converts raw flat bytes into a [`DynamicImage`] with `channels` bytes per pixel.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `channels` is not one of `1`, `2`,
/// `3`, or `4`, or when the byte length does not match `w * h * channels`.
pub fn raw_bytes_to_image(
    w: u32,
    h: u32,
    data: Vec<u8>,
    channels: usize,
) -> Result<DynamicImage, PilError> {
    match channels {
        1 => Ok(DynamicImage::ImageLuma8(
            pillow_rs_image::GrayImage::from_raw(w, h, data)
                .ok_or_else(|| PilError::ValueError("raw_bytes_to_image: buffer error".into()))?,
        )),
        2 => Ok(DynamicImage::ImageLumaA8(
            pillow_rs_image::GrayAlphaImage::from_raw(w, h, data)
                .ok_or_else(|| PilError::ValueError("raw_bytes_to_image: buffer error".into()))?,
        )),
        3 => Ok(DynamicImage::ImageRgb8(
            pillow_rs_image::RgbImage::from_raw(w, h, data)
                .ok_or_else(|| PilError::ValueError("raw_bytes_to_image: buffer error".into()))?,
        )),
        4 => Ok(DynamicImage::ImageRgba8(
            pillow_rs_image::RgbaImage::from_raw(w, h, data)
                .ok_or_else(|| PilError::ValueError("raw_bytes_to_image: buffer error".into()))?,
        )),
        _ => Err(PilError::ValueError(format!(
            "raw_bytes_to_image: unsupported channel count {}",
            channels
        ))),
    }
}

/// Computes basic statistics for non-image numeric input.
///
/// This is the Pillow `ImageStat` fallback for plain lists. The returned tuple
/// is `(count, sum, mean, min, max)`.
pub fn stat_from_list(data: &[f64]) -> (f64, f64, f64, f64, f64) {
    let count = data.len() as f64;
    let sum: f64 = data.iter().sum();
    let mean = if count > 0.0 { sum / count } else { 0.0 };
    let min_val = if count > 0.0 {
        data.iter().cloned().fold(f64::MAX, f64::min)
    } else {
        0.0
    };
    let max_val = if count > 0.0 {
        data.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    } else {
        0.0
    };
    (count, sum, mean, min_val, max_val)
}

/// Executes one [`PipelineOp`] against a materialized image.
///
/// `explicit_mode` carries Pillow mode tags, such as `"F"` or `"P"`, that the
/// underlying [`DynamicImage`] cannot express directly. The optional palette is
/// accepted for API symmetry with older call sites; CPU registry execution
/// currently receives palette state through the operation or image pipeline.
///
/// # Errors
///
/// Returns [`PilError`] from the CPU registry implementation.
pub fn execute_op(
    img: &DynamicImage,
    op: &PipelineOp,
    explicit_mode: Option<&str>,
    _palette: Option<&[u8]>,
) -> Result<DynamicImage, PilError> {
    crate::compute::registry::execute_cpu(op, img, explicit_mode)
}
