//! Core Pillow-style image object.
//!
//! [`Image`] stores loaded buffers, lazy encoded-byte inputs, paletted data, and
//! deferred pipelines. Public methods expose Rust equivalents of common
//! `PIL.Image.Image` behavior while keeping I/O and host-language object
//! conversion outside this crate.
//!
//! # Representation
//!
//! `Image` is both a public handle and the crate's lazy representation. Methods
//! such as [`Image::new`] and [`Image::open_bytes`] are the
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
//! internal `image-slash-star` buffers with an explicit mode tag.
//!
//! # Lazy Execution
//!
//! Operations that can be represented as [`crate::pipeline::PipelineOp`] values
//! may be deferred. Calling [`Image::materialize`], [`Image::encode`], or
//! [`Image::tobytes`] forces decoding and pipeline execution.

use image_slash_star::{
    Decoded, DecodedImage, DynamicImage, EncodedImage, GenericImageView, ImageFormat, ImageInfo,
    ImageMode, ImagePalette,
};
use std::sync::{Arc, OnceLock};

use crate::checked_dims::CheckedDims;
use crate::color::color_type_to_mode;
use crate::error::PilError;
use crate::format::parse_format_str;
use crate::pipeline::{PipelineOp, ResampleFilter, TransformMethod};

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
/// `palette` holds zero to 768 bytes of retained RGB triples. Missing entries
/// are black when the indexed image must be expanded for color operations.
#[derive(Debug, Clone)]
pub struct PalettedData {
    /// One palette index byte per pixel.
    pub indices: image_slash_star::GrayImage,
    /// Retained palette data as RGB triples.
    pub palette: Vec<u8>,
    /// Optional per-entry alpha values retained from the encoded palette.
    pub palette_alpha: Vec<u8>,
    /// Encoded container format when this image was decoded from a source.
    pub source_format: Option<ImageFormat>,
    /// Header metadata retained from the encoded source.
    pub info: Option<ImageInfo>,
    /// Shared operation-ready index view initialized on first read.
    pub materialized: MaterializationCache,
}

/// Pillow-compatible pending transparency stored in `Image.info`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteTransparency {
    /// One fully transparent palette index.
    Index(u8),
    /// One alpha byte per palette entry from a PNG `tRNS` table.
    Table(Vec<u8>),
}

/// Materialized image storage with retained codec and palette metadata.
#[derive(Debug, Clone)]
pub struct LoadedData {
    /// Operation-ready pixel storage.
    pub image: Arc<DynamicImage>,
    /// Pillow mode override for layouts not represented by `DynamicImage`.
    pub explicit_mode: Option<String>,
    /// Exact decoded sample mode before operation-buffer adaptation.
    pub decoded_mode: ImageMode,
    /// Palette RGB entries attached to a `PA` image.
    pub palette: Option<Vec<u8>>,
    /// Optional per-entry alpha values for an attached `RGBA` palette.
    pub palette_alpha: Option<Vec<u8>>,
    /// Encoded container format when this image came from a source.
    pub source_format: Option<ImageFormat>,
    /// Header metadata retained from the encoded source.
    pub info: Option<ImageInfo>,
}

/// One public `Image.putdata` pixel after host-language type extraction.
///
/// The variants retain the distinction Pillow makes between numeric samples,
/// packed multiband integers, and component tuples. [`Image::putdata_values`]
/// converts these values into the canonical logical-mode bytes consumed by all
/// compute backends.
#[derive(Debug, Clone, PartialEq)]
pub enum PutDataValue {
    /// A numeric sample for `1`, `L`, `P`, `I`, or `F`.
    Number(f64),
    /// A signed integer whose low bytes encode one multiband pixel.
    Packed(i64),
    /// Tuple components. Pillow parses the first component as a signed 64-bit
    /// integer and later components as signed 32-bit integers.
    Components(Vec<i128>),
}

fn putdata_clip_u8(value: f64) -> u8 {
    if value.is_nan() || value >= 255.0 {
        255
    } else if value <= 0.0 {
        0
    } else {
        value as u8
    }
}

fn putdata_clip_component(value: i128) -> u8 {
    value.clamp(0, 255) as u8
}

fn putdata_bytes(
    mode: crate::pipeline::PixelMode,
    values: &[PutDataValue],
    scale: f64,
    offset: f64,
) -> Result<Vec<u8>, PilError> {
    let capacity = values
        .len()
        .checked_mul(mode.channels())
        .ok_or_else(|| PilError::DimensionError("putdata byte count overflow".into()))?;
    let mut data = Vec::with_capacity(capacity);

    for value in values {
        match (mode, value) {
            (
                crate::pipeline::PixelMode::Mode1
                | crate::pipeline::PixelMode::L
                | crate::pipeline::PixelMode::P,
                PutDataValue::Number(number),
            ) => data.push(putdata_clip_u8(number * scale + offset)),
            (crate::pipeline::PixelMode::I, PutDataValue::Number(number)) => {
                data.extend_from_slice(&((number * scale + offset) as i32).to_le_bytes());
            }
            (crate::pipeline::PixelMode::F, PutDataValue::Number(number)) => {
                data.extend_from_slice(&((number * scale + offset) as f32).to_le_bytes());
            }
            (
                crate::pipeline::PixelMode::LA | crate::pipeline::PixelMode::PA,
                PutDataValue::Packed(packed),
            ) => {
                let bytes = packed.to_le_bytes();
                data.extend_from_slice(&[bytes[0], bytes[3]]);
            }
            (
                crate::pipeline::PixelMode::RGB
                | crate::pipeline::PixelMode::YCbCr
                | crate::pipeline::PixelMode::HSV,
                PutDataValue::Packed(packed),
            ) => data.extend_from_slice(&packed.to_le_bytes()[..3]),
            (
                crate::pipeline::PixelMode::RGBA | crate::pipeline::PixelMode::CMYK,
                PutDataValue::Packed(packed),
            ) => data.extend_from_slice(&packed.to_le_bytes()[..4]),
            (
                crate::pipeline::PixelMode::LA | crate::pipeline::PixelMode::PA,
                PutDataValue::Components(components),
            ) if components.len() == 2 => {
                data.extend(components.iter().copied().map(putdata_clip_component));
            }
            (
                crate::pipeline::PixelMode::RGB
                | crate::pipeline::PixelMode::YCbCr
                | crate::pipeline::PixelMode::HSV,
                PutDataValue::Components(components),
            ) if matches!(components.len(), 3 | 4) => {
                data.extend(components[..3].iter().copied().map(putdata_clip_component));
            }
            (
                crate::pipeline::PixelMode::RGBA | crate::pipeline::PixelMode::CMYK,
                PutDataValue::Components(components),
            ) if matches!(components.len(), 3 | 4) => {
                data.extend(components[..3].iter().copied().map(putdata_clip_component));
                data.push(
                    components
                        .get(3)
                        .copied()
                        .map_or(255, putdata_clip_component),
                );
            }
            (
                crate::pipeline::PixelMode::LA | crate::pipeline::PixelMode::PA,
                PutDataValue::Components(_),
            ) => {
                return Err(PilError::TypeError(
                    "color must be int, or tuple of one or two elements".into(),
                ));
            }
            (
                crate::pipeline::PixelMode::RGB
                | crate::pipeline::PixelMode::RGBA
                | crate::pipeline::PixelMode::CMYK
                | crate::pipeline::PixelMode::YCbCr
                | crate::pipeline::PixelMode::HSV,
                PutDataValue::Components(_),
            ) => {
                return Err(PilError::TypeError(
                    "color must be int, or tuple of one, three or four elements".into(),
                ));
            }
            (
                crate::pipeline::PixelMode::Mode1
                | crate::pipeline::PixelMode::L
                | crate::pipeline::PixelMode::P
                | crate::pipeline::PixelMode::I
                | crate::pipeline::PixelMode::F,
                _,
            ) => {
                return Err(PilError::TypeError("sequence must be flattened".into()));
            }
            _ => {
                return Err(PilError::TypeError("color must be int or tuple".into()));
            }
        }
    }

    Ok(data)
}

/// Core image value used by Pillow-style operations.
///
/// Values may hold decoded pixels, lazy input references, or a deferred
/// operation pipeline. Call [`Image::materialize`] when a concrete
/// [`DynamicImage`] is required.
#[derive(Debug, Clone)]
pub enum Image {
    /// Fully decoded, ready to process or save, with retained source metadata.
    Loaded(LoadedData),
    /// Fully decoded P-mode (palette) image: index bytes + 768-byte palette.
    Paletted(PalettedData),
    /// Byte buffer not yet decoded — lazy.
    Bytes {
        /// Canonical backend source sharing inspection and lazy decode state.
        source: EncodedImage,
        /// Optional detected image format.
        format: Option<ImageFormat>,
        /// Cached header metadata.
        info: Option<ImageInfo>,
        /// Operation-ready pixels initialized by the first implicit or explicit load.
        materialized: MaterializationCache,
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
        /// Per-entry palette alpha retained for index-preserving operations.
        palette_alpha: Option<Vec<u8>>,
        /// Pipeline output initialized by the first implicit or explicit load.
        materialized: MaterializationCache,
    },
}

/// Shared once-initialized operation-ready pixel result for lazy image nodes.
pub type MaterializationCache = Arc<OnceLock<Result<Arc<DynamicImage>, PilError>>>;

pub(crate) fn materialization_cache() -> MaterializationCache {
    Arc::new(OnceLock::new())
}

/// PIL-compatible statistics result. Every statistic is a per-band list.
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
        let fi = |idx: usize| -> StatValue {
            StatValue::IntList(bands.iter().map(|b| b[idx] as i64).collect())
        };
        let ff = |idx: usize| -> StatValue {
            StatValue::FloatList(bands.iter().map(|b| b[idx]).collect())
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
    /// Wraps operation-created pixels without claiming an encoded source.
    pub(crate) fn from_dynamic(image: DynamicImage, explicit_mode: Option<String>) -> Self {
        let decoded_mode = image.color().into();
        Self::Loaded(LoadedData {
            image: Arc::new(image),
            explicit_mode,
            decoded_mode,
            palette: None,
            palette_alpha: None,
            source_format: None,
            info: None,
        })
    }

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
            "RGB" => DynamicImage::ImageRgb8(image_slash_star::RgbImage::from_pixel(
                width,
                height,
                image_slash_star::Rgb([color.0, color.1, color.2]),
            )),
            "RGBA" => DynamicImage::ImageRgba8(image_slash_star::RgbaImage::from_pixel(
                width,
                height,
                image_slash_star::Rgba([color.0, color.1, color.2, color.3]),
            )),
            "L" => DynamicImage::ImageLuma8(image_slash_star::GrayImage::from_pixel(
                width,
                height,
                image_slash_star::Luma([color.0]),
            )),
            "LA" => DynamicImage::ImageLumaA8(image_slash_star::GrayAlphaImage::from_pixel(
                width,
                height,
                image_slash_star::LumaA([color.0, color.3]),
            )),
            "PA" => DynamicImage::ImageLumaA8(image_slash_star::GrayAlphaImage::from_pixel(
                width,
                height,
                image_slash_star::LumaA([color.0, color.3]),
            )),
            "1" => DynamicImage::ImageLuma8(image_slash_star::GrayImage::from_pixel(
                width,
                height,
                // PIL: stores the exact pixel value (0 or 1 or 255).
                // PIL's new("1") stores the raw color value as-is.
                image_slash_star::Luma([color.0]),
            )),
            // A tuple color allocates palette entry zero. Scalar P fills use
            // `new_palette_index` because this resolved tuple no longer carries
            // the Python argument's scalar-versus-tuple distinction.
            "P" => {
                return Ok(Image::Paletted(PalettedData {
                    indices: image_slash_star::GrayImage::from_pixel(
                        width,
                        height,
                        image_slash_star::Luma([0u8]),
                    ),
                    // Pillow retains one structural RGB entry here. Keeping
                    // 768 zero-padded bytes would make getpalette() invent 255
                    // trailing black entries now that palette length is exact.
                    palette: vec![color.0, color.1, color.2],
                    palette_alpha: Vec::new(),
                    source_format: None,
                    info: None,
                    materialized: materialization_cache(),
                }));
            }
            "CMYK" => DynamicImage::ImageRgba8(image_slash_star::RgbaImage::from_pixel(
                width,
                height,
                image_slash_star::Rgba([color.0, color.1, color.2, color.3]),
            )),
            "YCbCr" | "HSV" => DynamicImage::ImageRgb8(image_slash_star::RgbImage::from_pixel(
                width,
                height,
                image_slash_star::Rgb([color.0, color.1, color.2]),
            )),
            // I and F modes store all four resolved int32/float32 LE bytes.
            "I" | "F" => DynamicImage::ImageRgba8(image_slash_star::RgbaImage::from_pixel(
                width,
                height,
                image_slash_star::Rgba([color.0, color.1, color.2, color.3]),
            )),
            _ => return Err(PilError::ValueError(format!("Unsupported mode: {}", mode))),
        };
        let explicit = if matches!(mode, "CMYK" | "YCbCr" | "HSV" | "I" | "F" | "PA" | "1") {
            Some(mode.to_string())
        } else {
            None
        };
        Ok(Image::from_dynamic(img, explicit))
    }

    /// Creates a `P` image filled with one raw palette index and no palette.
    ///
    /// Pillow distinguishes a scalar `Image.new("P", ..., index)` argument
    /// from a tuple color: the scalar is stored directly in every pixel while
    /// the image retains an empty palette.
    pub fn new_palette_index(width: u32, height: u32, index: u8) -> Self {
        Image::Paletted(PalettedData {
            indices: image_slash_star::GrayImage::from_pixel(
                width,
                height,
                image_slash_star::Luma([index]),
            ),
            palette: Vec::new(),
            palette_alpha: Vec::new(),
            source_format: None,
            info: None,
            materialized: materialization_cache(),
        })
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
                image_slash_star::GrayImage::from_raw(w, h, data[..expected].to_vec())
                    .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
            ),
            "RGB" => DynamicImage::ImageRgb8(
                image_slash_star::RgbImage::from_raw(w, h, data[..expected].to_vec())
                    .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
            ),
            "RGBA" => DynamicImage::ImageRgba8(
                image_slash_star::RgbaImage::from_raw(w, h, data[..expected].to_vec())
                    .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
            ),
            "LA" => DynamicImage::ImageLumaA8(
                image_slash_star::GrayAlphaImage::from_raw(w, h, data[..expected].to_vec())
                    .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
            ),
            "P" => {
                return Ok(Image::Paletted(PalettedData {
                    indices: image_slash_star::GrayImage::from_raw(w, h, data[..expected].to_vec())
                        .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
                    // Raw P bytes are indices only. Pillow does not synthesize
                    // palette entries until a palette is explicitly attached.
                    palette: Vec::new(),
                    palette_alpha: Vec::new(),
                    source_format: None,
                    info: None,
                    materialized: materialization_cache(),
                }));
            }
            "CMYK" | "I" | "F" => DynamicImage::ImageRgba8(
                image_slash_star::RgbaImage::from_raw(w, h, data[..expected].to_vec())
                    .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
            ),
            "HSV" | "YCbCr" => DynamicImage::ImageRgb8(
                image_slash_star::RgbImage::from_raw(w, h, data[..expected].to_vec())
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
                    image_slash_star::GrayImage::from_raw(w, h, pixels)
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
                    image_slash_star::RgbaImage::from_raw(w, h, pixels).ok_or_else(|| {
                        PilError::ValueError("frombytes: RGBA buffer error".into())
                    })?,
                )
            }
        };
        let explicit_mode = match mode {
            "1" | "CMYK" | "HSV" | "YCbCr" | "I" | "F" => Some(mode.to_string()),
            _ => None,
        };
        Ok(Image::from_dynamic(img, explicit_mode))
    }

    /// Creates a lazy image from encoded image bytes with an optional format hint.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when the format string is unknown, the encoded
    /// header is unknown or malformed, or the hint does not match the detected
    /// format.
    pub fn open_bytes_with_format(data: Vec<u8>, format: Option<&str>) -> Result<Self, PilError> {
        let requested = format.map(parse_format_str).transpose()?;
        let data: Arc<[u8]> = data.into();
        let source = EncodedImage::new(Arc::clone(&data))
            .map_err(|error| map_codec_error(error, "memory"))?;
        let info = source.info().clone();
        if requested.is_some_and(|requested| requested != info.format) {
            return Err(PilError::ValueError(format!(
                "requested {requested:?} input but detected {:?}",
                info.format
            )));
        }
        Ok(Image::Bytes {
            source,
            format: Some(info.format),
            info: Some(info),
            materialized: materialization_cache(),
        })
    }

    /// Creates a lazy image from encoded image bytes.
    ///
    /// Format and metadata are detected from encoded headers without decoding
    /// pixel payloads.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when the encoded header is unknown or malformed.
    pub fn open_bytes(data: Vec<u8>) -> Result<Self, PilError> {
        Self::open_bytes_with_format(data, None)
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
            PipelineOp::Crop { .. }
            | PipelineOp::Transpose { .. }
            | PipelineOp::Flip
            | PipelineOp::Mirror
            | PipelineOp::CropBorder { .. }
            | PipelineOp::Offset { .. }
            | PipelineOp::Paste { .. }
            // Pillow 12.2.0 libImaging/Effects.c:117-159 scatters the
            // one-byte P indices, then duplicates the complete palette.
            | PipelineOp::EffectSpread { .. }
            | PipelineOp::DrawLine { .. }
            | PipelineOp::DrawRectangle { .. }
            | PipelineOp::DrawRoundedRect { .. }
            | PipelineOp::DrawEllipse { .. }
            | PipelineOp::DrawCircle { .. }
            | PipelineOp::DrawPolygon { .. }
            | PipelineOp::DrawArc { .. }
            | PipelineOp::DrawChord { .. }
            | PipelineOp::DrawPieslice { .. }
            | PipelineOp::DrawPoint { .. }
            // ImageChops.invert applies 255-index directly to P samples. Its
            // newly allocated core image intentionally has no palette.
            | PipelineOp::InvertChops
            // Image.remap_palette builds an inverse index LUT and separately
            // installs the reordered palette on the result.
            | PipelineOp::RemapPalette { .. }
            // Pillow's Image.point maps P indices directly, and Image._new
            // copies the source palette. Image.eval delegates to point.
            | PipelineOp::Eval { .. }
            | PipelineOp::PointOp { .. }
            // Pillow mutates palette indices directly for putdata, while
            // putalpha promotes each index to a PA (index, alpha) pair.
            | PipelineOp::PutData {
                mode: crate::pipeline::PixelMode::P | crate::pipeline::PixelMode::PA,
                ..
            }
            | PipelineOp::PutAlpha {
                mode: crate::pipeline::PixelMode::P | crate::pipeline::PixelMode::PA,
                ..
            } => true,
            PipelineOp::CompositeModule { other, .. } => other.has_palette_mode(),
            PipelineOp::PutPixel {
                palette_index: true,
                ..
            } => true,
            PipelineOp::Rotate { fill: None, .. } => true,
            PipelineOp::Resize { filter, .. } => {
                matches!(filter, ResampleFilter::Nearest)
            }
            PipelineOp::Thumbnail { filter, .. } => {
                matches!(filter, ResampleFilter::Nearest)
            }
            PipelineOp::Transform {
                method: TransformMethod::Affine,
                filter: ResampleFilter::Nearest,
                palette_fill,
                ..
            } => palette_fill.is_some(),
            _ => false,
        }
    }

    fn is_dimension_preserving_draw(op: &PipelineOp) -> bool {
        matches!(
            op,
            PipelineOp::DrawLine { .. }
                | PipelineOp::DrawRectangle { .. }
                | PipelineOp::DrawRoundedRect { .. }
                | PipelineOp::DrawEllipse { .. }
                | PipelineOp::DrawCircle { .. }
                | PipelineOp::DrawPolygon { .. }
                | PipelineOp::DrawArc { .. }
                | PipelineOp::DrawChord { .. }
                | PipelineOp::DrawPieslice { .. }
                | PipelineOp::DrawPoint { .. }
        )
    }

    /// Whether this value currently represents palette indices rather than
    /// visible luma samples. Lazy inputs use cached header metadata so the
    /// first queued operation does not need to decode merely to preserve mode.
    pub(crate) fn has_palette_mode(&self) -> bool {
        match self {
            Image::Paletted(_) => true,
            Image::Bytes {
                info: Some(info), ..
            } => info.mode == ImageMode::P8,
            Image::Pipeline {
                explicit_mode: Some(mode),
                ..
            }
            | Image::Loaded(LoadedData {
                explicit_mode: Some(mode),
                ..
            }) => mode == "P",
            _ => false,
        }
    }

    /// Whether samples are palette indices, optionally paired with per-pixel
    /// alpha in PA mode.
    ///
    /// Keep this separate from `has_palette_mode`: callers such as
    /// `apply_transparency`, P encoding, and palette-index `putpixel` implement
    /// Pillow behavior that applies only to single-band P images.
    fn has_palette_samples(&self) -> bool {
        self.has_palette_mode() || self.explicit_mode() == Some("PA")
    }

    /// Decodes or executes this image into a concrete pixel buffer.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when lazy decoding, pipeline execution, or format
    /// conversion fails.
    pub fn materialize(&self) -> Result<DynamicImage, PilError> {
        self.materialized_shared()
            .map(|image| image.as_ref().clone())
    }

    /// Returns shared operation-ready pixels, persistently initializing lazy
    /// source or pipeline state when necessary.
    fn materialized_shared(&self) -> Result<Arc<DynamicImage>, PilError> {
        match self {
            Image::Loaded(data) => Ok(Arc::clone(&data.image)),
            Image::Paletted(data) => data
                .materialized
                .get_or_init(|| Ok(Arc::new(DynamicImage::ImageLuma8(data.indices.clone()))))
                .clone(),
            Image::Bytes {
                source,
                materialized,
                ..
            } => materialized
                .get_or_init(|| {
                    decoded_to_dynamic(
                        source
                            .decode()
                            .map_err(|error| map_codec_error(error, "memory"))?,
                    )
                    .map(Arc::new)
                })
                .clone(),
            Image::Pipeline {
                source,
                ops,
                explicit_mode,
                backend,
                palette,
                palette_alpha,
                materialized,
                ..
            } => materialized
                .get_or_init(|| {
                    Self::evaluate_pipeline(
                        source,
                        ops,
                        explicit_mode,
                        *backend,
                        palette,
                        palette_alpha,
                    )
                })
                .clone(),
        }
    }

    /// Evaluates a pipeline without publishing its result into the pipeline's
    /// ordinary materialization cache.
    fn evaluate_pipeline(
        source: &Image,
        ops: &[PipelineOp],
        explicit_mode: &Option<String>,
        backend: Option<crate::compute::Backend>,
        palette: &Option<Vec<u8>>,
        palette_alpha: &Option<Vec<u8>>,
    ) -> Result<Arc<DynamicImage>, PilError> {
        let selected = backend.unwrap_or_else(|| crate::compute::route(ops, None));
        crate::compute::validate_backend_support(selected, ops)?;
        Self::evaluate_pipeline_with_image(
            source,
            source.materialize()?,
            ops,
            explicit_mode,
            Some(selected),
            palette,
            palette_alpha,
        )
    }

    fn evaluate_pipeline_uncached(
        source: &Image,
        ops: &[PipelineOp],
        explicit_mode: &Option<String>,
        backend: Option<crate::compute::Backend>,
        palette: &Option<Vec<u8>>,
        palette_alpha: &Option<Vec<u8>>,
    ) -> Result<Arc<DynamicImage>, PilError> {
        let selected = backend.unwrap_or_else(|| crate::compute::route(ops, None));
        crate::compute::validate_backend_support(selected, ops)?;
        Self::evaluate_pipeline_with_image(
            source,
            source.materialize_uncached()?,
            ops,
            explicit_mode,
            Some(selected),
            palette,
            palette_alpha,
        )
    }

    fn evaluate_pipeline_with_image(
        source: &Image,
        mut img: DynamicImage,
        ops: &[PipelineOp],
        explicit_mode: &Option<String>,
        backend: Option<crate::compute::Backend>,
        palette: &Option<Vec<u8>>,
        palette_alpha: &Option<Vec<u8>>,
    ) -> Result<Arc<DynamicImage>, PilError> {
        if source.has_palette_samples() {
            let all_safe = ops.iter().all(Self::is_palette_safe_op);
            if all_safe {
                let selected = backend.unwrap_or_else(|| crate::compute::route(ops, None));
                let palette_mode = if explicit_mode.as_deref() == Some("PA")
                    || source.explicit_mode() == Some("PA")
                {
                    "PA"
                } else {
                    "P"
                };
                img = crate::compute::execute_batch(selected, ops, &img, Some(palette_mode))?;
                return Ok(Arc::new(img));
            }

            let palette_mode = if source.explicit_mode() == Some("PA") {
                "PA"
            } else {
                "P"
            };

            if let Some(palette) = palette.clone().or_else(|| source.extract_palette()) {
                let palette_alpha = palette_alpha
                    .clone()
                    .or_else(|| source.palette_alpha())
                    .unwrap_or_default();
                img = if palette_mode == "PA" {
                    // PIL's Convert.c PA path takes RGB from the palette and
                    // alpha exclusively from each PA sample. Any RGBA palette
                    // alpha is intentionally ignored.
                    expand_palette_alpha(&img.to_luma_alpha8(), &palette)
                } else {
                    expand_palette(&img.to_luma8(), &palette, &palette_alpha)
                };
            }
        }

        let selected = backend.unwrap_or_else(|| crate::compute::route(ops, None));
        img = crate::compute::execute_batch(selected, ops, &img, explicit_mode.as_deref())?;
        Ok(Arc::new(img))
    }

    fn materialize_uncached(&self) -> Result<DynamicImage, PilError> {
        match self {
            Image::Loaded(data) => Ok(data.image.as_ref().clone()),
            Image::Paletted(data) => Ok(DynamicImage::ImageLuma8(data.indices.clone())),
            Image::Bytes { source, .. } => decoded_to_dynamic(
                &image_slash_star::decode(source.bytes())
                    .map_err(|error| map_codec_error(error, "memory"))?,
            ),
            Image::Pipeline {
                source,
                ops,
                explicit_mode,
                backend,
                palette,
                palette_alpha,
                ..
            } => Self::evaluate_pipeline_uncached(
                source,
                ops,
                explicit_mode,
                *backend,
                palette,
                palette_alpha,
            )
            .map(|image| image.as_ref().clone()),
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
        self.materialize()
    }

    // ── Pipeline ops ──

    /// Returns a lazy image with `op` appended to its pipeline.
    ///
    /// Existing pipelines are extended in order. Non-pipeline images become the
    /// source of a new pipeline. Mode metadata and palettes are carried forward
    /// when the operation can preserve them; operations that fundamentally
    /// change mode clear or replace the explicit mode tag.
    pub fn push_op(source: &Image, op: PipelineOp) -> Image {
        let source_is_paletted = source.has_palette_samples();
        let palette_safe = source_is_paletted && Self::is_palette_safe_op(&op);
        let promotes_p_to_pa = source.has_palette_mode()
            && matches!(
                &op,
                PipelineOp::PutAlpha {
                    mode: crate::pipeline::PixelMode::P,
                    ..
                }
            );
        let explicit_mode = if source_is_paletted {
            palette_safe.then(|| {
                if source.explicit_mode() == Some("PA") {
                    "PA".to_owned()
                } else {
                    "P".to_owned()
                }
            })
        } else {
            match &op {
                PipelineOp::Grayscale
                | PipelineOp::Convert { .. }
                | PipelineOp::Quantize { .. }
                | PipelineOp::ExtractBand { .. } => None,
                _ => source.explicit_mode().map(str::to_owned),
            }
        };
        let source_palette = if palette_safe {
            source.extract_palette()
        } else {
            None
        };
        let source_palette_alpha = if palette_safe {
            source.palette_alpha()
        } else {
            None
        };
        let source_format = source.source_format();
        match source {
            // Keep a concrete sample-layout boundary around palette expansion
            // and P↔PA transitions. A flattened batch has only one external
            // mode tag, so it cannot correctly run P operations before
            // putalpha or PA operations after it (especially on the GPU).
            Image::Pipeline { .. }
                if source_is_paletted
                    && (!palette_safe
                        || promotes_p_to_pa
                        || source.explicit_mode() == Some("PA")) =>
            {
                Image::Pipeline {
                    source: Arc::new(source.clone()),
                    ops: vec![op],
                    format: source_format,
                    explicit_mode,
                    backend: source.backend(),
                    palette: source_palette,
                    palette_alpha: source_palette_alpha,
                    materialized: materialization_cache(),
                }
            }
            Image::Pipeline {
                source: pipeline_source,
                ops,
                format,
                backend,
                ..
            } => {
                let mut new_ops = ops.clone();
                new_ops.push(op);
                Image::Pipeline {
                    source: Arc::clone(pipeline_source),
                    ops: new_ops,
                    format: *format,
                    explicit_mode,
                    backend: *backend,
                    palette: source_palette,
                    palette_alpha: source_palette_alpha,
                    materialized: materialization_cache(),
                }
            }
            other => Image::Pipeline {
                source: Arc::new(other.clone()),
                ops: vec![op],
                format: source_format,
                explicit_mode,
                backend: other.backend(),
                palette: source_palette,
                palette_alpha: source_palette_alpha,
                materialized: materialization_cache(),
            },
        }
    }

    /// Returns a lazy one-operation pipeline with an explicit output mode.
    ///
    /// A separate node is required for mode-changing operations: flattening
    /// into an existing pipeline would pass the final mode tag to operations
    /// that execute before the mode transition.
    pub(crate) fn push_mode_changing_op(
        source: &Image,
        op: PipelineOp,
        output_mode: &str,
    ) -> Image {
        Image::Pipeline {
            source: Arc::new(source.clone()),
            ops: vec![op],
            format: source.source_format(),
            explicit_mode: Some(output_mode.to_owned()),
            backend: source.backend(),
            palette: None,
            palette_alpha: None,
            materialized: materialization_cache(),
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
        let img = self.materialized_shared()?;
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
        let (color, palette_index, updated_palette) = if self.has_palette_mode() {
            if a != 255 {
                return Err(PilError::ValueError(
                    "cannot add non-opaque RGBA color to RGB palette".into(),
                ));
            }
            let (index, palette) = self.resolve_palette_color([r, g, b])?;
            ((index, index, index, 255), true, palette)
        } else {
            ((r, g, b, a), false, None)
        };
        let mut new_self = Image::push_op(
            self,
            PipelineOp::PutPixel {
                x,
                y,
                color,
                palette_index,
            },
        );
        if let Some(updated_palette) = updated_palette {
            let Image::Pipeline { palette, .. } = &mut new_self else {
                return Err(PilError::InternalError(
                    "putpixel did not create an operation pipeline".into(),
                ));
            };
            *palette = Some(updated_palette);
        }
        *self = new_self;
        Ok(())
    }

    /// Resolve a Pillow-style RGB tuple to a palette index.
    ///
    /// Existing colors retain their first palette index. A missing color uses
    /// the next table entry, or an unused entry when the table already contains
    /// 256 colors. The returned palette is present only when allocation changed
    /// it.
    fn resolve_palette_color(&self, color: [u8; 3]) -> Result<(u8, Option<Vec<u8>>), PilError> {
        let mut palette = self.extract_palette().ok_or_else(|| {
            PilError::PaletteError("P-mode image has no retained palette".to_owned())
        })?;
        if let Some(index) = palette
            .chunks_exact(3)
            .position(|entry| entry == color.as_slice())
        {
            return Ok((index as u8, None));
        }

        let entries = palette.len() / 3;
        let index = if entries < 256 {
            entries
        } else {
            let mut used = [false; 256];
            for index in self.tobytes()? {
                used[usize::from(index)] = true;
            }
            let transparent_index = self
                .palette_alpha()
                .filter(|alpha| alpha.len() == 1 && alpha[0] == 0)
                .map(|_| 0);
            (0..256)
                .rev()
                .find(|&index| !used[index] && Some(index) != transparent_index)
                .ok_or_else(|| {
                    PilError::ValueError("cannot allocate more than 256 colors".into())
                })?
        };

        if index == entries {
            palette.extend_from_slice(&color);
        } else {
            palette[index * 3..index * 3 + 3].copy_from_slice(&color);
        }
        Ok((index as u8, Some(palette)))
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
        if mode == "P" {
            let (w, h) = self.size()?;
            if x >= w || y >= h {
                return Err(PilError::IndexError("image index out of range".into()));
            }
            *self = Image::push_op(
                self,
                PipelineOp::PutPixel {
                    x,
                    y,
                    color: (v, v, v, 255),
                    palette_index: true,
                },
            );
            return Ok(());
        }
        let (r, g, b, a) = match mode {
            "L" | "1" => (v, v, v, 255),
            "LA" => (v, 0, 0, 0),
            "RGB" => (v, 0, 0, 255),
            "RGBA" | "CMYK" => (v, 0, 0, 0),
            _ => (v, v, v, 255),
        };
        self.putpixel(x, y, r, g, b, a)
    }

    /// Returns Pillow-compatible image statistics in structured form.
    ///
    /// Results use list variants in band order, including single-band images,
    /// matching Pillow's `ImageStat.Stat` attribute contract.
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
            let img = self.materialized_shared()?;
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

        let img = self.materialized_shared()?;
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
        // Check explicit mode for non-standard band names. Deferred pipelines
        // carry the same side-channel modes as loaded images (notably PA).
        if let Some(m) = self.explicit_mode() {
            let bands: Vec<String> = match m {
                "CMYK" => vec![
                    "C".to_string(),
                    "M".to_string(),
                    "Y".to_string(),
                    "K".to_string(),
                ],
                "YCbCr" => vec!["Y".to_string(), "Cb".to_string(), "Cr".to_string()],
                "HSV" => vec!["H".to_string(), "S".to_string(), "V".to_string()],
                "PA" => vec!["P".to_string(), "A".to_string()],
                "I" | "F" | "P" | "1" => vec![m.to_owned()],
                _ => vec![],
            };
            if !bands.is_empty() {
                return Ok(bands.iter().map(|s| s.to_string()).collect());
            }
        }
        let img = self.materialized_shared()?;
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

    /// Encodes the image using the requested Pillow format name.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when the format is unsupported or image
    /// materialization or encoding fails.
    pub fn encode(&self, format: &str) -> Result<Vec<u8>, PilError> {
        let save_format = parse_format_str(format)?;
        let decoded = self.decoded_for_encoding()?;
        Ok(image_slash_star::encode_default(&decoded, save_format)?)
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

    /// Returns one byte per logical image sample.
    ///
    /// Unlike [`Image::tobytes`], mode `"1"` is expanded to one `0` or `255`
    /// byte per pixel. This matches the sequence representation of Pillow's
    /// internal `ImagingCore`, which bitmap-font masks expose directly.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when the image cannot be materialized.
    pub fn tobytes_unpacked(&self) -> Result<Vec<u8>, PilError> {
        if self.mode()? == "1" {
            return Ok(self.materialize()?.to_luma8().into_raw());
        }
        self.tobytes()
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
        let img = self.materialized_shared()?;

        // F/I modes: stored as RGBA8 internally (f32/i32 LE bytes packed as RGBA).
        // Read the raw bytes directly — img.as_bytes() already contains the correct
        // f32/i32 LE representation (4 bytes per pixel).
        if matches!(mode, "F" | "I") {
            return Ok(img.as_bytes().to_vec());
        }

        // For mode "1" images, pack 8 pixels per byte (MSB first) matching PIL.
        if mode == "1" && img.color() == image_slash_star::ColorType::L8 {
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
        self.lock_backend_recursive(b);
        self
    }

    fn lock_backend_recursive(&mut self, b: crate::compute::Backend) {
        let Image::Pipeline {
            source,
            ops,
            backend,
            materialized,
            ..
        } = self
        else {
            return;
        };

        Arc::make_mut(source).lock_backend_recursive(b);
        for op in ops {
            Self::lock_op_backend_recursive(op, b);
        }
        *backend = Some(b);
        // A previously inspected/materialized pipeline must execute again
        // under the newly forced backend, including nested pipeline nodes.
        *materialized = materialization_cache();
    }

    fn lock_op_backend_recursive(op: &mut PipelineOp, b: crate::compute::Backend) {
        match op {
            PipelineOp::Add { other, .. }
            | PipelineOp::Subtract { other, .. }
            | PipelineOp::Multiply { other }
            | PipelineOp::Screen { other }
            | PipelineOp::Darker { other }
            | PipelineOp::Lighter { other }
            | PipelineOp::Difference { other }
            | PipelineOp::Overlay { other }
            | PipelineOp::HardLight { other }
            | PipelineOp::SoftLight { other }
            | PipelineOp::AddModulo { other }
            | PipelineOp::SubtractModulo { other }
            | PipelineOp::LogicalAnd { other }
            | PipelineOp::LogicalOr { other }
            | PipelineOp::LogicalXor { other }
            | PipelineOp::Blend { other, .. }
            | PipelineOp::BlendModule { other, .. } => {
                Arc::make_mut(other).lock_backend_recursive(b);
            }
            PipelineOp::Composite { other, mask }
            | PipelineOp::CompositeModule { other, mask, .. } => {
                Arc::make_mut(other).lock_backend_recursive(b);
                Arc::make_mut(mask).lock_backend_recursive(b);
            }
            PipelineOp::Paste { source, mask, .. } => {
                Arc::make_mut(source).lock_backend_recursive(b);
                if let Some(mask) = mask {
                    Arc::make_mut(mask).lock_backend_recursive(b);
                }
            }
            PipelineOp::AlphaComposite { source, .. } => {
                Arc::make_mut(source).lock_backend_recursive(b);
            }
            PipelineOp::Merge { bands, .. } => {
                for band in bands {
                    band.lock_backend_recursive(b);
                }
            }
            _ => {}
        }
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
    /// Some Pillow modes cannot be represented by `image-slash-star` color types
    /// alone. This method exposes the side-channel mode tag for modes such as
    /// `"1"`, `"P"`, `"CMYK"`, `"HSV"`, `"YCbCr"`, `"I"`, and `"F"`.
    pub fn explicit_mode(&self) -> Option<&str> {
        match self {
            Image::Loaded(LoadedData {
                explicit_mode: Some(m),
                ..
            }) => Some(m.as_str()),
            Image::Paletted(_) => Some("P"),
            Image::Pipeline {
                explicit_mode: Some(m),
                ..
            } => Some(m.as_str()),
            _ => None,
        }
    }

    /// Returns attached palette data as RGB triples.
    ///
    /// A palette contains up to 768 bytes: 256 entries of `R, G, B`. Pipeline
    /// images can carry a copied palette so palette-safe operations preserve
    /// `P` mode.
    pub fn palette(&self) -> Option<Vec<u8>> {
        match self {
            Image::Paletted(data) => Some(data.palette.clone()),
            Image::Loaded(data) => data.palette.clone(),
            Image::Pipeline { palette, .. } => palette.clone(),
            Image::Bytes { info, .. } => info
                .as_ref()
                .and_then(|info| info.palette.as_ref())
                .map(|palette| padded_palette(&palette.rgb)),
        }
    }

    /// Returns retained per-entry alpha values for an indexed image.
    pub fn palette_alpha(&self) -> Option<Vec<u8>> {
        match self {
            Image::Paletted(data) => Some(data.palette_alpha.clone()),
            Image::Loaded(data) => data.palette_alpha.clone(),
            Image::Pipeline { palette_alpha, .. } => palette_alpha.clone(),
            Image::Bytes { info, .. } => info
                .as_ref()
                .and_then(|info| info.palette.as_ref())
                .map(|palette| palette.alpha.clone()),
        }
    }

    /// Returns immutable metadata from the original encoded source.
    ///
    /// Pipeline results retain this provenance; current dimensions and mode
    /// are reported by [`Image::size`] and [`Image::mode`] instead.
    pub fn image_info(&self) -> Option<ImageInfo> {
        match self {
            Image::Loaded(data) => data.info.clone(),
            Image::Paletted(data) => data.info.clone(),
            Image::Bytes { info, .. } => info.clone(),
            Image::Pipeline { source, .. } => source.image_info(),
        }
    }

    pub(crate) fn source_format(&self) -> Option<ImageFormat> {
        match self {
            Image::Loaded(data) => data.source_format,
            Image::Paletted(data) => data.source_format,
            Image::Bytes { format, .. } | Image::Pipeline { format, .. } => *format,
        }
    }

    /// Returns the exact retained RGB palette like Pillow's `getpalette`.
    ///
    /// Palette length is structural, not inferred from color values. In
    /// particular, encoded GIF color tables and explicitly attached palettes
    /// retain trailing black entries.
    pub fn getpalette_trimmed(&self) -> Option<Vec<u8>> {
        self.extract_palette()
    }

    /// Returns palette bytes converted to RGBA, trimmed to the retained RGB
    /// palette length.
    pub fn getpalette_rgba(&self) -> Option<Vec<u8>> {
        let palette = self.getpalette_trimmed()?;
        let alpha = if self.pending_palette_transparency().is_none() {
            self.palette_alpha().unwrap_or_default()
        } else {
            Vec::new()
        };
        let mut rgba = Vec::with_capacity(palette.len() / 3 * 4);
        for (index, color) in palette.chunks_exact(3).enumerate() {
            rgba.extend_from_slice(color);
            rgba.push(alpha.get(index).copied().unwrap_or(255));
        }
        Some(rgba)
    }

    /// Attaches a palette without changing the image's sample bytes.
    ///
    /// Pillow reinterprets `L` samples as palette indices (`P`) and `LA`
    /// samples as index/alpha pairs (`PA`). Existing `P`/`PA` samples are also
    /// retained byte-for-byte while their palette is replaced. The mutation is
    /// stored in this core value rather than binding-only shadow state. `P`
    /// images therefore enter later CPU, SIMD, or GPU work through the existing
    /// shared indexed representation.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] for an illegal image mode, unsupported
    /// raw palette mode, or an oversized palette.
    pub fn putpalette(&mut self, data: &[u8], rawmode: &str) -> Result<(), PilError> {
        let mode = self.mode()?;
        if !matches!(mode.as_str(), "L" | "LA" | "P" | "PA") {
            return Err(PilError::ValueError("illegal image mode".to_owned()));
        }
        let (palette, palette_alpha) = split_palette_data(data, rawmode)?;
        let source_format = self.source_format();
        let info = self.image_info();
        let materialized = self.materialize()?;

        *self = match mode.as_str() {
            "L" | "P" => Image::Paletted(PalettedData {
                indices: materialized.to_luma8(),
                palette,
                palette_alpha,
                source_format,
                info,
                materialized: materialization_cache(),
            }),
            "LA" | "PA" => {
                let image = DynamicImage::ImageLumaA8(materialized.to_luma_alpha8());
                let decoded_mode = image.color().into();
                Image::Loaded(LoadedData {
                    image: Arc::new(image),
                    explicit_mode: Some("PA".to_owned()),
                    decoded_mode,
                    palette: Some(palette),
                    palette_alpha: Some(palette_alpha),
                    source_format,
                    info,
                })
            }
            _ => unreachable!("putpalette mode was validated"),
        };
        Ok(())
    }

    /// Returns pending Pillow `info["transparency"]` metadata for a P image.
    ///
    /// PNG's compact single-transparent-index form is reported as an integer;
    /// other alpha tables retain their byte representation.
    pub fn pending_palette_transparency(&self) -> Option<PaletteTransparency> {
        if !self.has_palette_samples() {
            return None;
        }
        let alpha = self
            .image_info()?
            .palette
            .as_ref()
            .map(|palette| palette.alpha.as_slice())?
            .to_vec();
        if alpha.is_empty() {
            return None;
        }
        let mut transparent_index = None;
        for (index, value) in alpha.iter().copied().enumerate() {
            if value == 255 {
                continue;
            }
            if value != 0 || transparent_index.is_some() {
                return Some(PaletteTransparency::Table(alpha));
            }
            transparent_index = u8::try_from(index).ok();
        }
        transparent_index
            .map(PaletteTransparency::Index)
            .or_else(|| Some(PaletteTransparency::Table(alpha)))
    }

    /// Returns the observable Pillow palette mode.
    pub fn palette_mode(&self) -> Option<&'static str> {
        (self.has_palette_mode() || self.explicit_mode() == Some("PA")).then(|| {
            if self.pending_palette_transparency().is_none()
                && self.palette_alpha().is_some_and(|alpha| !alpha.is_empty())
            {
                "RGBA"
            } else {
                "RGB"
            }
        })
    }

    /// Returns whether mode, pending metadata, or a committed palette carries
    /// transparency data.
    pub fn has_transparency_data(&self) -> bool {
        if self
            .mode()
            .is_ok_and(|mode| matches!(mode.as_str(), "LA" | "La" | "PA" | "RGBA" | "RGBa"))
        {
            return true;
        }
        self.pending_palette_transparency().is_some()
            || (self.has_palette_mode()
                && self.palette_alpha().is_some_and(|alpha| !alpha.is_empty()))
    }

    /// Commits stored palette transparency without changing pixels or mode.
    ///
    /// Pillow's `Image.apply_transparency` keeps a `P` image indexed: it moves
    /// the `info["transparency"]` value into the palette's alpha table. The
    /// codec layer used here canonicalizes indexed transparency into
    /// [`PalettedData::palette_alpha`] while decoding. This method retains that
    /// table, removes the pending `ImageInfo` alpha marker, and materializes a
    /// lazy or deferred P image as indexed storage. It never expands pixels to
    /// `RGBA`.
    ///
    /// # Errors
    ///
    /// Returns an error if a lazy/deferred indexed image cannot be materialized.
    pub fn apply_transparency(&mut self) -> Result<(), PilError> {
        if !self.has_palette_mode() || self.pending_palette_transparency().is_none() {
            return Ok(());
        }

        if !matches!(self, Image::Paletted(_)) {
            let indices = self.materialize()?.to_luma8();
            let palette = self.palette().unwrap_or_else(default_palette);
            let palette_alpha = self.palette_alpha().unwrap_or_default();
            let source_format = self.source_format();
            let info = self.image_info();
            *self = Image::Paletted(PalettedData {
                indices,
                palette,
                palette_alpha,
                source_format,
                info,
                materialized: materialization_cache(),
            });
        }
        if let Image::Paletted(data) = self
            && let Some(palette) = data.info.as_mut().and_then(|info| info.palette.as_mut())
        {
            palette.alpha.clear();
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
    pub(crate) fn extract_palette(&self) -> Option<Vec<u8>> {
        match self {
            Image::Paletted(data) => Some(operational_palette(data)),
            Image::Loaded(data) => data.palette.clone(),
            Image::Pipeline { palette, .. } => palette.clone(),
            Image::Bytes { info, .. } => info
                .as_ref()
                .and_then(|info| info.palette.as_ref())
                .map(|palette| palette.rgb.clone()),
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
        let decoded = self.decoded_for_encoding()?;
        Ok(image_slash_star::encode_default(
            &decoded,
            ImageFormat::Png,
        )?)
    }

    fn decoded_for_encoding(&self) -> Result<DecodedImage, PilError> {
        if let Image::Paletted(data) = self {
            let palette = ImagePalette::new(operational_palette(data), data.palette_alpha.clone())?;
            return Ok(DecodedImage::with_mode(
                data.indices.width(),
                data.indices.height(),
                data.indices.as_raw().to_vec(),
                ImageMode::P8,
            )
            .with_palette(palette));
        }
        if self.has_palette_mode() {
            let indices = self.materialize_indices()?.to_luma8();
            let palette = self.extract_palette().ok_or_else(|| {
                PilError::PaletteError("P-mode pipeline has no retained palette".to_owned())
            })?;
            let alpha = self.palette_alpha().unwrap_or_default();
            return Ok(DecodedImage::with_mode(
                indices.width(),
                indices.height(),
                indices.into_raw(),
                ImageMode::P8,
            )
            .with_palette(ImagePalette::new(palette, alpha)?));
        }
        Ok(self.materialize()?.into_decoded())
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
                let indices = img.to_luma8();
                let palette_alpha = self.palette_alpha().unwrap_or_default();
                return Ok(expand_palette(&indices, &palette, &palette_alpha));
            }
        }
        self.materialize()
    }

    /// Convert Paletted image to RGB for rendering/saving. Returns None for non-Paletted.
    pub(crate) fn paletted_to_rgb(&self) -> Option<DynamicImage> {
        if let Image::Paletted(data) = self {
            Some(expand_palette(
                &data.indices,
                &data.palette,
                &data.palette_alpha,
            ))
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
        match self {
            Image::Loaded(data) => return Ok(data.image.dimensions()),
            Image::Paletted(data) => return Ok(data.indices.dimensions()),
            Image::Bytes {
                info: Some(info), ..
            } => return Ok((info.width, info.height)),
            Image::Pipeline { source, ops, .. }
                if ops.iter().all(Self::is_dimension_preserving_draw) =>
            {
                return source.size();
            }
            _ => {}
        }
        let img = self.materialized_shared()?;
        Ok((img.width(), img.height()))
    }

    /// Returns the Pillow mode string for this image.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when lazy data must be materialized to determine
    /// the mode and that materialization fails.
    pub fn mode(&self) -> Result<String, PilError> {
        match self {
            Image::Paletted(_) => return Ok("P".to_owned()),
            Image::Loaded(data) => {
                return Ok(data
                    .explicit_mode
                    .clone()
                    .unwrap_or_else(|| image_mode_name(data.decoded_mode).to_owned()));
            }
            Image::Bytes {
                info: Some(info), ..
            } => return Ok(image_mode_name(info.mode).to_owned()),
            Image::Pipeline {
                explicit_mode: Some(mode),
                ..
            } => return Ok(mode.clone()),
            _ => {}
        }
        let img = self.materialized_shared()?;
        Ok(color_type_to_mode(img.color()).to_string())
    }

    /// Returns the known image format name, if the image came from encoded input.
    pub fn format_name(&self) -> Option<String> {
        match self {
            Image::Loaded(data) => data.source_format.map(|format| format.as_str().to_owned()),
            Image::Paletted(data) => data.source_format.map(|format| format.as_str().to_owned()),
            Image::Bytes { format, .. } | Image::Pipeline { format, .. } => {
                format.map(|format| format.as_str().to_owned())
            }
        }
    }

    /// Forces lazy decoding and pipeline execution.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization fails.
    pub fn load(&mut self) -> Result<(), PilError> {
        let loaded = match &*self {
            Image::Loaded(_) | Image::Paletted(_) => return Ok(()),
            Image::Bytes { format, info, .. } => {
                image_from_materialized(self.materialized_shared()?, *format, info.clone())?
            }
            Image::Pipeline {
                source,
                explicit_mode,
                format,
                palette,
                palette_alpha,
                ..
            } => {
                if explicit_mode.as_deref() == Some("P") {
                    if let Some(palette) = palette {
                        let indices = self.materialized_shared()?.as_ref().clone().to_luma8();
                        Image::Paletted(PalettedData {
                            indices,
                            palette: palette.clone(),
                            palette_alpha: palette_alpha.clone().unwrap_or_default(),
                            source_format: *format,
                            info: source.image_info(),
                            materialized: materialization_cache(),
                        })
                    } else {
                        return Err(PilError::PaletteError(
                            "P-mode pipeline has no retained palette".to_owned(),
                        ));
                    }
                } else {
                    let image = self.materialized_shared()?;
                    let decoded_mode = image.color().into();
                    Image::Loaded(LoadedData {
                        image,
                        explicit_mode: explicit_mode.clone(),
                        decoded_mode,
                        palette: palette.clone(),
                        palette_alpha: palette_alpha.clone(),
                        source_format: *format,
                        info: source.image_info(),
                    })
                }
            }
        };
        *self = loaded;
        Ok(())
    }

    /// Returns whether this handle can reuse successfully materialized pixels.
    pub fn is_materialized(&self) -> bool {
        match self {
            Image::Loaded(_) | Image::Paletted(_) => true,
            Image::Bytes { materialized, .. } | Image::Pipeline { materialized, .. } => {
                matches!(materialized.get(), Some(Ok(_)))
            }
        }
    }

    /// Fully validates an encoded or deferred image without changing its state.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when decoding or pipeline execution fails.
    pub fn verify(&self) -> Result<(), PilError> {
        match self {
            Image::Bytes { source, .. } => source
                .verify()
                .map_err(|error| map_codec_error(error, "memory"))?,
            Image::Pipeline {
                source,
                ops,
                explicit_mode,
                backend,
                palette,
                palette_alpha,
                ..
            } => {
                Self::evaluate_pipeline_uncached(
                    source,
                    ops,
                    explicit_mode,
                    *backend,
                    palette,
                    palette_alpha,
                )?;
            }
            Image::Loaded(_) | Image::Paletted(_) => {}
        }
        Ok(())
    }

    /// Returns a logically independent image sharing immutable caches until mutation.
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
        let img = self.materialized_shared()?;
        let band = band.unwrap_or(-1);
        if band >= 0 {
            let rgba = img.to_rgba8();
            let b = band.min(3) as usize;
            return Ok(rgba.pixels().map(|p| p[b]).collect());
        }
        match img.color() {
            image_slash_star::ColorType::L8 | image_slash_star::ColorType::L16 => {
                let gray = img.to_luma8();
                Ok(gray.into_raw())
            }
            image_slash_star::ColorType::La8 | image_slash_star::ColorType::La16 => {
                let ga = img.to_luma_alpha8();
                let mut out = Vec::with_capacity((ga.width() * ga.height() * 2) as usize);
                for p in ga.pixels() {
                    out.push(p[0]);
                    out.push(p[1]);
                }
                Ok(out)
            }
            image_slash_star::ColorType::Rgb8
            | image_slash_star::ColorType::Rgb16
            | image_slash_star::ColorType::Rgb32F => {
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
        let mode_name = self.mode()?;
        let mode = crate::pipeline::PixelMode::from_name(&mode_name).ok_or_else(|| {
            PilError::ValueError(format!("unsupported putdata mode: {mode_name}"))
        })?;
        let new_self = Image::push_op(
            self,
            PipelineOp::PutData {
                data: data.to_vec(),
                mode,
            },
        );
        *self = new_self;
        Ok(())
    }

    /// Normalizes one value per pixel and queues exact Pillow `putdata` bytes.
    ///
    /// Pillow 12.2's `_imaging.c:_putdata` applies `scale` and `offset` only to
    /// single-band, signed-integer, and float images. Its multiband `getink`
    /// path instead treats integers as little-endian packed pixels and ignores
    /// scaling. Keeping that distinction here gives CPU, SIMD, and GPU the
    /// same canonical byte payload.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::TypeError`] for incompatible value shapes or too
    /// many pixels, and propagates image mode or dimension errors.
    pub fn putdata_values(
        &mut self,
        values: &[PutDataValue],
        scale: f64,
        offset: f64,
    ) -> Result<(), PilError> {
        let (width, height) = self.size()?;
        let pixel_count = (width as usize)
            .checked_mul(height as usize)
            .ok_or_else(|| PilError::DimensionError("pixel count overflow".into()))?;
        if values.len() > pixel_count {
            return Err(PilError::TypeError("too many data entries".into()));
        }

        let mode_name = self.mode()?;
        let mode = crate::pipeline::PixelMode::from_name(&mode_name).ok_or_else(|| {
            PilError::ValueError(format!("unsupported putdata mode: {mode_name}"))
        })?;
        let data = putdata_bytes(mode, values, scale, offset)?;
        self.putdata(&data)
    }

    /// Writes one normalized `putdata` value immediately at a pixel offset.
    ///
    /// Host-language bindings use this form while coercing values because
    /// Pillow exposes each completed pixel to callbacks triggered by coercing
    /// the following value. The same canonical byte normalization as
    /// [`Image::putdata_values`] is retained, while the already materialized
    /// storage is updated without building one deferred pipeline per pixel.
    /// Bulk, callback-free replacement remains backend-dispatched through
    /// [`Image::putdata_values`]. This immediate host-memory mutation is the
    /// CPU path used by public bindings because observable callback order takes
    /// precedence over deferred SIMD/GPU execution.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::TypeError`] when `value` is incompatible with the
    /// image mode or `pixel_index` is outside the image.
    pub fn putdata_value_at(
        &mut self,
        pixel_index: usize,
        value: &PutDataValue,
        scale: f64,
        offset: f64,
    ) -> Result<(), PilError> {
        let (width, height) = self.size()?;
        let pixel_count = (width as usize)
            .checked_mul(height as usize)
            .ok_or_else(|| PilError::DimensionError("pixel count overflow".into()))?;
        if pixel_index >= pixel_count {
            return Err(PilError::TypeError("too many data entries".into()));
        }

        let mode_name = self.mode()?;
        let mode = crate::pipeline::PixelMode::from_name(&mode_name).ok_or_else(|| {
            PilError::ValueError(format!("unsupported putdata mode: {mode_name}"))
        })?;
        let bytes = putdata_bytes(mode, std::slice::from_ref(value), scale, offset)?;
        let start = pixel_index
            .checked_mul(mode.channels())
            .ok_or_else(|| PilError::DimensionError("putdata byte offset overflow".into()))?;
        let end = start
            .checked_add(bytes.len())
            .ok_or_else(|| PilError::DimensionError("putdata byte offset overflow".into()))?;

        self.load()?;
        match self {
            Image::Paletted(data) if mode == crate::pipeline::PixelMode::P => {
                data.indices.as_mut()[pixel_index] = bytes[0];
                data.materialized = materialization_cache();
            }
            Image::Loaded(data) => {
                let image = Arc::make_mut(&mut data.image);
                let samples: &mut [u8] = match mode {
                    crate::pipeline::PixelMode::Mode1
                    | crate::pipeline::PixelMode::L
                    | crate::pipeline::PixelMode::P => image
                        .as_mut_luma8()
                        .ok_or_else(|| {
                            PilError::InternalError("putdata L storage mismatch".into())
                        })?
                        .as_mut(),
                    crate::pipeline::PixelMode::LA | crate::pipeline::PixelMode::PA => image
                        .as_mut_luma_alpha8()
                        .ok_or_else(|| {
                            PilError::InternalError("putdata LA storage mismatch".into())
                        })?
                        .as_mut(),
                    crate::pipeline::PixelMode::RGB
                    | crate::pipeline::PixelMode::YCbCr
                    | crate::pipeline::PixelMode::HSV => image
                        .as_mut_rgb8()
                        .ok_or_else(|| {
                            PilError::InternalError("putdata RGB storage mismatch".into())
                        })?
                        .as_mut(),
                    crate::pipeline::PixelMode::RGBA
                    | crate::pipeline::PixelMode::CMYK
                    | crate::pipeline::PixelMode::I
                    | crate::pipeline::PixelMode::F => image
                        .as_mut_rgba8()
                        .ok_or_else(|| {
                            PilError::InternalError("putdata RGBA storage mismatch".into())
                        })?
                        .as_mut(),
                };
                let destination = samples.get_mut(start..end).ok_or_else(|| {
                    PilError::InternalError("putdata storage offset out of bounds".into())
                })?;
                destination.copy_from_slice(&bytes);
            }
            _ => {
                return Err(PilError::InternalError(
                    "putdata did not materialize writable storage".into(),
                ));
            }
        }
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
        let img = self.materialized_shared()?;
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
        let mode_name = self.mode()?;
        if let Some(target) = match mode_name.as_str() {
            "1" | "I" | "I;16" | "I;16L" | "I;16B" | "F" => Some("LA"),
            "YCbCr" | "HSV" => Some("RGBA"),
            _ => None,
        } {
            // Pillow Image.py derives the alpha mode from getmodebase(), then
            // surfaces the exact failed core conversion when neither in-place
            // setmode nor ImagingConvert supports this source/target pair.
            return Err(PilError::ValueError(format!(
                "conversion from {mode_name} to {target} not supported"
            )));
        }
        let mode = crate::pipeline::PixelMode::from_name(&mode_name).ok_or_else(|| {
            PilError::ValueError(format!("unsupported putalpha mode: {mode_name}"))
        })?;
        let mut new_self = Image::push_op(self, PipelineOp::PutAlpha { alpha, mode });
        if let Image::Pipeline { explicit_mode, .. } = &mut new_self {
            // Pillow Image.putalpha promotes P to PA without expanding palette
            // indices. CMYK is converted through RGB and therefore becomes
            // ordinary RGBA rather than retaining a CMYK side-channel tag.
            *explicit_mode = match mode {
                crate::pipeline::PixelMode::P | crate::pipeline::PixelMode::PA => {
                    Some("PA".to_owned())
                }
                crate::pipeline::PixelMode::CMYK => None,
                _ => explicit_mode.clone(),
            };
        }
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
        let img = self.materialized_shared()?;
        let n_bands = match img.color() {
            image_slash_star::ColorType::L8 | image_slash_star::ColorType::L16 => 1,
            image_slash_star::ColorType::La8 | image_slash_star::ColorType::La16 => 2,
            image_slash_star::ColorType::Rgb8 | image_slash_star::ColorType::Rgb16 => 3,
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
        let img = self.materialized_shared()?;
        // Compute 256-bin histogram
        let mut hist = [0u32; 256];
        match img.color() {
            image_slash_star::ColorType::L8 | image_slash_star::ColorType::L16 => {
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
        let img = self.materialized_shared()?;
        let n_bands = match img.color() {
            image_slash_star::ColorType::L8 | image_slash_star::ColorType::L16 => 1,
            image_slash_star::ColorType::La8 | image_slash_star::ColorType::La16 => 2,
            image_slash_star::ColorType::Rgb8 | image_slash_star::ColorType::Rgb16 => 3,
            _ => 4,
        };
        let mut hists = vec![[0u32; 256]; n_bands];
        // Use mode-aware pixel reading (to_rgba8 remaps LA channels incorrectly for histogram)
        match img.color() {
            image_slash_star::ColorType::La8 | image_slash_star::ColorType::La16 => {
                let la = img.to_luma_alpha8();
                for px in la.pixels() {
                    hists[0][px[0] as usize] += 1;
                    hists[1][px[1] as usize] += 1;
                }
            }
            image_slash_star::ColorType::L8 | image_slash_star::ColorType::L16 => {
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
                    // Pillow 12.2.0 `_entropy` accumulates
                    // `p * log(p) * M_LOG2E` and negates once at return
                    // (`src/_imaging.c:1401-1423`). Preserve that evaluation
                    // order, including the fused multiply-add emitted by its
                    // optimized C build, for bit-exact float and signed-zero
                    // parity.
                    entropy = (p * p.ln()).mul_add(std::f64::consts::LOG2_E, entropy);
                }
            }
        }
        Ok(-entropy)
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
        let img = self.materialized_shared()?;
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
        let img = self.materialized_shared()?;
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
        self.remap_palette_with_source(dest_map, None)
    }

    /// Remaps indices and installs colors selected from an optional source palette.
    ///
    /// Pillow interprets `dest_map[new_index]` as the old index. Pixel samples
    /// therefore use the inverse mapping, while palette triples are copied in
    /// `dest_map` order.
    pub fn remap_palette_with_source(
        &self,
        dest_map: &[u8],
        source_palette: Option<&[u8]>,
    ) -> Result<Image, PilError> {
        let mode = self.mode()?;
        if !matches!(mode.as_str(), "L" | "P") {
            return Err(PilError::ValueError("illegal image mode".to_owned()));
        }
        if dest_map.len() > 256 {
            return Err(PilError::ValueError(
                "byte must be in range(0, 256)".to_owned(),
            ));
        }

        let uses_attached_palette = source_palette.is_none() && mode == "P";
        // Pillow's Image.remap_palette treats every explicit palette longer
        // than 768 bytes as interleaved RGBA, regardless of the source mode.
        let source_bands = if source_palette.is_some_and(|palette| palette.len() > 768) {
            4
        } else {
            3
        };
        let source_palette = source_palette.map_or_else(
            || {
                if mode == "P" {
                    self.extract_palette().unwrap_or_default()
                } else {
                    (0u8..=u8::MAX)
                        .flat_map(|value| [value, value, value])
                        .collect()
                }
            },
            <[u8]>::to_vec,
        );
        let mut remapped_palette = Vec::with_capacity(dest_map.len() * 3);
        let mut explicit_alpha = (source_bands == 4).then(|| Vec::with_capacity(dest_map.len()));
        for &old_index in dest_map {
            let start = usize::from(old_index) * source_bands;
            if let Some(color) = source_palette.get(start..start + source_bands) {
                // Pillow 12.2 PIL.Image.remap_palette passes raw palette bands
                // through unchanged. Core stores the same RGBA layout as RGB
                // triples plus a per-entry alpha sidecar.
                remapped_palette.extend_from_slice(&color[..3]);
                if let Some(alpha) = &mut explicit_alpha {
                    alpha.push(color[3]);
                }
            }
        }

        let remapped_alpha = if source_bands == 4 {
            explicit_alpha
        } else if uses_attached_palette {
            self.palette_alpha().and_then(|alpha| {
                if alpha.is_empty() {
                    return None;
                }
                Some(
                    dest_map
                        .iter()
                        .map(|&index| alpha.get(usize::from(index)).copied().unwrap_or(255))
                        .collect(),
                )
            })
        } else {
            None
        };

        let mut result = Image::push_op(
            self,
            PipelineOp::RemapPalette {
                dest_map: dest_map.to_vec(),
            },
        );
        if let Image::Pipeline {
            explicit_mode,
            palette,
            palette_alpha,
            ..
        } = &mut result
        {
            *explicit_mode = Some("P".to_owned());
            *palette = Some(remapped_palette);
            *palette_alpha = remapped_alpha;
        }
        Ok(result)
    }
}

fn image_from_materialized(
    image: Arc<DynamicImage>,
    source_format: Option<ImageFormat>,
    info: Option<ImageInfo>,
) -> Result<Image, PilError> {
    let mode = info
        .as_ref()
        .map_or_else(|| image.color().into(), |info| info.mode);
    if mode == ImageMode::P8 {
        let palette = info
            .as_ref()
            .and_then(|info| info.palette.clone())
            .ok_or_else(|| {
                PilError::PaletteError("decoded P-mode image has no retained palette".to_owned())
            })?;
        return Ok(Image::Paletted(PalettedData {
            indices: image.as_ref().clone().to_luma8(),
            palette: padded_palette(&palette.rgb),
            palette_alpha: palette.alpha,
            source_format,
            info,
            materialized: materialization_cache(),
        }));
    }
    let explicit_mode = match mode {
        ImageMode::L1 | ImageMode::Cmyk8 | ImageMode::F32 | ImageMode::I32 => {
            Some(image_mode_name(mode).to_owned())
        }
        _ => None,
    };
    Ok(Image::Loaded(LoadedData {
        image,
        explicit_mode,
        decoded_mode: mode,
        palette: None,
        palette_alpha: None,
        source_format,
        info,
    }))
}

fn map_codec_error(error: image_slash_star::ImageError, source: &str) -> PilError {
    if matches!(error, image_slash_star::ImageError::UnknownFormat) {
        PilError::UnidentifiedImageError(source.to_owned())
    } else {
        PilError::ImageError(error)
    }
}

fn decoded_to_dynamic(decoded: &Decoded<DecodedImage>) -> Result<DynamicImage, PilError> {
    let content = &decoded.content;
    match content.mode {
        ImageMode::P8 => image_slash_star::GrayImage::from_raw(
            content.width,
            content.height,
            content.pixels.clone(),
        )
        .map(DynamicImage::ImageLuma8)
        .ok_or_else(|| PilError::DimensionError("invalid indexed buffer".to_owned())),
        ImageMode::L1 => {
            let row_bytes = content.width.div_ceil(8) as usize;
            let mut unpacked = Vec::with_capacity(content.width as usize * content.height as usize);
            for row in content.pixels.chunks_exact(row_bytes) {
                for x in 0..content.width as usize {
                    let bit = (row[x / 8] >> (7 - (x % 8))) & 1;
                    unpacked.push(if bit == 0 { 0 } else { 255 });
                }
            }
            image_slash_star::GrayImage::from_raw(content.width, content.height, unpacked)
                .map(DynamicImage::ImageLuma8)
                .ok_or_else(|| PilError::DimensionError("invalid packed bilevel buffer".to_owned()))
        }
        ImageMode::Cmyk8 | ImageMode::F32 | ImageMode::I32 => {
            image_slash_star::RgbaImage::from_raw(
                content.width,
                content.height,
                content.pixels.clone(),
            )
            .map(DynamicImage::ImageRgba8)
            .ok_or_else(|| PilError::DimensionError("invalid four-byte buffer".to_owned()))
        }
        mode => DynamicImage::from_decoded(content)
            .ok_or_else(|| PilError::ValueError(format!("unsupported decoded mode {mode:?}"))),
    }
}

fn padded_palette(rgb: &[u8]) -> Vec<u8> {
    let mut palette = rgb.to_vec();
    palette.resize(768, 0);
    palette
}

fn split_palette_data(data: &[u8], rawmode: &str) -> Result<(Vec<u8>, Vec<u8>), PilError> {
    match rawmode {
        "RGB" => {
            let complete = data.len() / 3 * 3;
            if complete > 768 {
                return Err(PilError::ValueError("invalid palette size".to_owned()));
            }
            Ok((data[..complete].to_vec(), Vec::new()))
        }
        "RGBA" => {
            let complete = data.len() / 4 * 4;
            if complete > 1024 {
                return Err(PilError::ValueError("invalid palette size".to_owned()));
            }
            let mut rgb = Vec::with_capacity(complete / 4 * 3);
            let mut alpha = Vec::with_capacity(complete / 4);
            for color in data[..complete].chunks_exact(4) {
                rgb.extend_from_slice(&color[..3]);
                alpha.push(color[3]);
            }
            Ok((rgb, alpha))
        }
        _ => Err(PilError::ValueError("unrecognized raw mode".to_owned())),
    }
}

fn expand_palette(
    indices: &image_slash_star::GrayImage,
    palette: &[u8],
    palette_alpha: &[u8],
) -> DynamicImage {
    if palette_alpha.is_empty() {
        return DynamicImage::ImageRgb8(image_slash_star::RgbImage::from_fn(
            indices.width(),
            indices.height(),
            |x, y| {
                let index = usize::from(indices.get_pixel(x, y)[0]);
                let base = index * 3;
                image_slash_star::Rgb([
                    palette.get(base).copied().unwrap_or(0),
                    palette.get(base + 1).copied().unwrap_or(0),
                    palette.get(base + 2).copied().unwrap_or(0),
                ])
            },
        ));
    }
    DynamicImage::ImageRgba8(image_slash_star::RgbaImage::from_fn(
        indices.width(),
        indices.height(),
        |x, y| {
            let index = usize::from(indices.get_pixel(x, y)[0]);
            let base = index * 3;
            image_slash_star::Rgba([
                palette.get(base).copied().unwrap_or(0),
                palette.get(base + 1).copied().unwrap_or(0),
                palette.get(base + 2).copied().unwrap_or(0),
                palette_alpha.get(index).copied().unwrap_or(255),
            ])
        },
    ))
}

/// Expands Pillow `PA` samples through their RGB palette.
///
/// Unlike `P`, `PA` owns alpha per pixel. Pillow therefore ignores any alpha
/// attached to the palette while converting `PA` to `RGBA`.
fn expand_palette_alpha(
    indices_alpha: &image_slash_star::GrayAlphaImage,
    palette: &[u8],
) -> DynamicImage {
    DynamicImage::ImageRgba8(image_slash_star::RgbaImage::from_fn(
        indices_alpha.width(),
        indices_alpha.height(),
        |x, y| {
            let pixel = indices_alpha.get_pixel(x, y);
            let index = usize::from(pixel[0]);
            let base = index * 3;
            image_slash_star::Rgba([
                palette.get(base).copied().unwrap_or(0),
                palette.get(base + 1).copied().unwrap_or(0),
                palette.get(base + 2).copied().unwrap_or(0),
                pixel[1],
            ])
        },
    ))
}

/// Return mutable palette state without treating internal zero padding as
/// encoded palette entries.
fn operational_palette(data: &PalettedData) -> Vec<u8> {
    let Some(source_palette) = data
        .info
        .as_ref()
        .and_then(|info| info.palette.as_ref())
        .map(|palette| &palette.rgb)
    else {
        return data.palette.clone();
    };
    if data.palette.starts_with(source_palette)
        && data.palette[source_palette.len()..]
            .iter()
            .all(|&component| component == 0)
    {
        source_palette.clone()
    } else {
        data.palette.clone()
    }
}

const fn image_mode_name(mode: ImageMode) -> &'static str {
    match mode {
        ImageMode::L1 => "1",
        ImageMode::P8 => "P",
        ImageMode::L8 => "L",
        ImageMode::La8 | ImageMode::La16 => "LA",
        ImageMode::Rgb8 | ImageMode::Rgb16 | ImageMode::Rgb32F => "RGB",
        ImageMode::Rgba8 | ImageMode::Rgba16 | ImageMode::Rgba32F => "RGBA",
        ImageMode::Cmyk8 => "CMYK",
        ImageMode::L16 => "I;16",
        ImageMode::F32 => "F",
        ImageMode::I32 => "I",
    }
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
        image_slash_star::ColorType::L8 => {
            // Extract R channel directly — GPU mode-aware shaders only update R for L mode.
            // G and B may be stale; to_luma8() weights all three channels and would be wrong.
            let rgba = result.to_rgba8();
            let (w, h) = rgba.dimensions();
            let luma: Vec<u8> = rgba.pixels().map(|px| px[0]).collect();
            DynamicImage::ImageLuma8(
                image_slash_star::GrayImage::from_raw(w, h, luma)
                    .unwrap_or_else(|| result.to_luma8()),
            )
        }
        image_slash_star::ColorType::La8 => {
            // Extract R (luma) and A (alpha) directly.
            let rgba = result.to_rgba8();
            let (w, h) = rgba.dimensions();
            let la: Vec<u8> = rgba.pixels().flat_map(|px| [px[0], px[3]]).collect();
            DynamicImage::ImageLumaA8(
                image_slash_star::GrayAlphaImage::from_raw(w, h, la)
                    .unwrap_or_else(|| result.to_luma_alpha8()),
            )
        }
        image_slash_star::ColorType::Rgb8 => DynamicImage::ImageRgb8(result.to_rgb8()),
        image_slash_star::ColorType::Rgba8 => DynamicImage::ImageRgba8(result.to_rgba8()),
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
            image_slash_star::GrayImage::from_raw(w, h, data)
                .ok_or_else(|| PilError::ValueError("raw_bytes_to_image: buffer error".into()))?,
        )),
        2 => Ok(DynamicImage::ImageLumaA8(
            image_slash_star::GrayAlphaImage::from_raw(w, h, data)
                .ok_or_else(|| PilError::ValueError("raw_bytes_to_image: buffer error".into()))?,
        )),
        3 => Ok(DynamicImage::ImageRgb8(
            image_slash_star::RgbImage::from_raw(w, h, data)
                .ok_or_else(|| PilError::ValueError("raw_bytes_to_image: buffer error".into()))?,
        )),
        4 => Ok(DynamicImage::ImageRgba8(
            image_slash_star::RgbaImage::from_raw(w, h, data)
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

#[cfg(test)]
mod tests {
    use super::Image;
    use crate::compute::Backend;
    use crate::ops::module_fns;

    #[test]
    fn new_p_scalar_fills_indices_without_synthesizing_palette_entries() {
        let image = Image::new_palette_index(3, 2, 7);

        assert_eq!(
            (
                image.mode().expect("mode must be available"),
                image.tobytes().expect("indices must be available"),
                image.getpalette_trimmed()
            ),
            ("P".to_owned(), vec![7; 6], Some(Vec::new()))
        );
    }

    #[test]
    fn new_p_tuple_color_allocates_palette_entry_zero() {
        let image = Image::new(2, 1, "P", (7, 8, 9, 255)).expect("P image must be valid");

        assert_eq!(
            (
                image.tobytes().expect("indices must be available"),
                image.getpalette_trimmed()
            ),
            (vec![0, 0], Some(vec![7, 8, 9]))
        );
    }

    #[test]
    fn new_preserves_i_f_and_pa_sample_layouts() {
        let int_bytes = (-7_i32).to_le_bytes();
        let integer = Image::new(
            2,
            1,
            "I",
            (int_bytes[0], int_bytes[1], int_bytes[2], int_bytes[3]),
        )
        .expect("I image must be valid");
        let float_bytes = 2.5_f32.to_le_bytes();
        let float = Image::new(
            2,
            1,
            "F",
            (
                float_bytes[0],
                float_bytes[1],
                float_bytes[2],
                float_bytes[3],
            ),
        )
        .expect("F image must be valid");
        let palette_alpha = Image::new(2, 1, "PA", (7, 0, 0, 192)).expect("PA image must be valid");

        assert_eq!(integer.mode().expect("I mode"), "I");
        assert_eq!(integer.tobytes().expect("I bytes"), int_bytes.repeat(2));
        assert_eq!(float.mode().expect("F mode"), "F");
        assert_eq!(float.tobytes().expect("F bytes"), float_bytes.repeat(2));
        assert_eq!(palette_alpha.mode().expect("PA mode"), "PA");
        assert_eq!(
            palette_alpha.getbands().expect("PA bands"),
            ["P".to_owned(), "A".to_owned()]
        );
        assert_eq!(palette_alpha.tobytes().expect("PA bytes"), [7, 192, 7, 192]);
    }

    #[test]
    fn frombytes_p_preserves_indices_without_synthesizing_palette_entries() {
        let image = Image::frombytes("P", (3, 1), &[2, 7, 11]).expect("P image must be valid");

        assert_eq!(
            (
                image.tobytes().expect("indices must be available"),
                image.getpalette_trimmed()
            ),
            (vec![2, 7, 11], Some(Vec::new()))
        );
    }

    #[test]
    fn effect_spread_zero_preserves_exact_p_samples_and_rgba_palette() {
        let mut image =
            Image::frombytes("P", (4, 1), &[2, 0, 1, 2]).expect("P image must be valid");
        image
            .putpalette(&[10, 20, 30, 5, 40, 50, 60, 128, 70, 80, 90, 255], "RGBA")
            .expect("RGBA palette must be valid");

        let spread = module_fns::effect_spread(&image, 0)
            .expect("zero-distance spread must queue")
            .use_backend(Backend::Cpu);

        assert_eq!(
            (
                spread.mode().expect("spread mode"),
                spread.size().expect("spread size"),
                spread.tobytes().expect("spread indices"),
                spread.palette(),
                spread.palette_alpha(),
                spread.palette_mode(),
            ),
            (
                "P".to_owned(),
                (4, 1),
                vec![2, 0, 1, 2],
                Some(vec![10, 20, 30, 40, 50, 60, 70, 80, 90]),
                Some(vec![5, 128, 255]),
                Some("RGBA"),
            )
        );
    }

    #[test]
    fn effect_spread_positive_distance_preserves_single_p_sample_and_palette() {
        let mut image = Image::frombytes("P", (1, 1), &[1]).expect("P image must be valid");
        image
            .putpalette(&[1, 2, 3, 11, 4, 5, 6, 222], "RGBA")
            .expect("RGBA palette must be valid");

        let spread = module_fns::effect_spread(&image, 17)
            .expect("positive-distance spread must queue")
            .use_backend(Backend::Cpu);

        assert_eq!(
            (
                spread.mode().expect("spread mode"),
                spread.size().expect("spread size"),
                spread.tobytes().expect("spread indices"),
                spread.palette(),
                spread.palette_alpha(),
            ),
            (
                "P".to_owned(),
                (1, 1),
                vec![1],
                Some(vec![1, 2, 3, 4, 5, 6]),
                Some(vec![11, 222]),
            )
        );
    }

    #[test]
    fn putpalette_reinterprets_l_samples_as_p_indices() {
        let mut image = Image::new(2, 1, "L", (128, 0, 0, 0)).expect("L image must be valid");
        let palette: Vec<u8> = (0..48).collect();

        image
            .putpalette(&palette, "RGB")
            .expect("RGB palette must be valid");

        assert_eq!(
            (
                image.mode().expect("mode must be available"),
                image.tobytes().expect("indices must be available"),
                image.getpalette_trimmed()
            ),
            ("P".to_owned(), vec![128, 128], Some(palette))
        );
    }

    #[test]
    fn putpalette_reinterprets_la_samples_as_pa_pairs() {
        let mut image = Image::new(2, 1, "LA", (7, 0, 0, 90)).expect("LA image must be valid");
        let palette: Vec<u8> = (0..48).collect();

        image
            .putpalette(&palette, "RGB")
            .expect("RGB palette must be valid");

        assert_eq!(
            (
                image.mode().expect("mode must be available"),
                image.getbands().expect("bands must be available"),
                image.tobytes().expect("samples must be available"),
                image.getpalette_trimmed()
            ),
            (
                "PA".to_owned(),
                vec!["P".to_owned(), "A".to_owned()],
                vec![7, 90, 7, 90],
                Some(palette)
            )
        );
    }

    #[test]
    fn entropy_matches_pillow_c_evaluation_order() {
        let image = Image::frombytes("L", (7, 1), &[0, 0, 0, 0, 1, 1, 1])
            .expect("test image must be valid");

        assert_eq!(
            image.entropy().expect("entropy must succeed").to_bits(),
            0x3fef_86fd_27eb_b77f
        );
    }

    #[test]
    fn zero_entropy_preserves_pillow_negative_zero() {
        let image = Image::new(1, 1, "L", (128, 0, 0, 0)).expect("test image must be valid");

        assert_eq!(
            image.entropy().expect("entropy must succeed").to_bits(),
            (-0.0f64).to_bits()
        );
    }
}
