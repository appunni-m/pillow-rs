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
//! modes such as `CMYK`, `HSV`, `YCbCr`, `I`, `F`, and the 16-bit luma raw
//! modes may be carried through Pillow-owned raster buffers with an explicit
//! mode tag.
//!
//! # Lazy Execution
//!
//! Operations that can be represented as [`crate::pipeline::PipelineOp`] values
//! may be deferred. Calling [`Image::materialize`], [`Image::encode`], or
//! [`Image::tobytes`] forces decoding and pipeline execution.

use image_slash_star::{
    Decoded, DecodedImage, EncodedImage, ImageFormat, ImageInfo, ImageMode, ImagePalette,
};
use std::sync::{Arc, OnceLock};

use crate::checked_dims::CheckedDims;
use crate::color::color_type_to_mode;
use crate::error::PilError;
use crate::format::parse_format_str;
use crate::pipeline::{PipelineOp, ResampleFilter, TransformMethod};
use crate::raster::{DynamicImage, GenericImageView};

/// Host-neutral input for converting Pillow's scalar `ImagingCore` view to bytes.
#[derive(Debug, Clone)]
pub enum ImagingCoreBytesInput {
    /// A one-band sequence of integer samples.
    Scalars(Vec<i64>),
    /// A one-band sequence of floating-point samples.
    Floats(Vec<f64>),
    /// A multiband or otherwise non-scalar sequence.
    Multiband,
}

/// Converts scalar `ImagingCore` samples to bytes with Pillow's errors.
pub fn imaging_core_to_bytes(input: ImagingCoreBytesInput) -> Result<Vec<u8>, PilError> {
    match input {
        ImagingCoreBytesInput::Scalars(values) => values
            .into_iter()
            .map(|value| {
                u8::try_from(value)
                    .map_err(|_| PilError::ValueError("bytes must be in range(0, 256)".into()))
            })
            .collect(),
        // Pillow's bytes(list[float]) path reports the element type rather than
        // exposing the binding's internal sequence classification.
        ImagingCoreBytesInput::Floats(_) => Err(PilError::TypeError(
            "'float' object cannot be interpreted as an integer".into(),
        )),
        // ImagingCore returns tuples for multiband pixels, so Python's bytes()
        // reports the tuple element type exactly as Pillow does.
        ImagingCoreBytesInput::Multiband => Err(PilError::TypeError(
            "'tuple' object cannot be interpreted as an integer".into(),
        )),
    }
}

/// Prepared fields for Pillow's lightweight public EXIF compatibility object.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExifCompatFields {
    /// Original EXIF bytes retained only for fully loaded metadata.
    pub loaded_exif: Option<Vec<u8>>,
    /// First eight bytes of the TIFF payload, when a record was supplied.
    pub head: Option<Vec<u8>>,
    /// TIFF byte order marker, if the payload has a valid TIFF magic.
    pub endian: Option<String>,
    /// Whether the compatibility object should expose Pillow's `bigtiff=False`.
    pub bigtiff: bool,
    /// Whether a non-empty EXIF record was supplied.
    pub has_source: bool,
}

/// Prepares EXIF compatibility metadata without touching Python objects.
pub fn prepare_exif_compat(raw: Option<&[u8]>, loaded_exif: bool) -> ExifCompatFields {
    let Some(raw) = raw.filter(|raw| !raw.is_empty()) else {
        return ExifCompatFields::default();
    };
    let payload = raw.strip_prefix(b"Exif\0\0").unwrap_or(raw);
    let endian = (payload.len() >= 4)
        .then(|| &payload[2..4])
        .filter(|magic| *magic == b"*\0" || *magic == b"\0*")
        .map(|_| {
            if payload.starts_with(b"II") {
                "<".to_owned()
            } else {
                ">".to_owned()
            }
        });
    ExifCompatFields {
        loaded_exif: loaded_exif.then(|| raw.to_vec()),
        head: Some(payload[..payload.len().min(8)].to_vec()),
        endian,
        bigtiff: !loaded_exif,
        has_source: true,
    }
}

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
    pub indices: crate::raster::GrayImage,
    /// Retained palette data as RGB triples.
    pub palette: Vec<u8>,
    /// Optional per-entry alpha values retained from the encoded palette.
    pub palette_alpha: Vec<u8>,
    /// Encoded container format when this image was decoded from a source.
    pub source_format: Option<ImageFormat>,
    /// Header metadata retained from the encoded source.
    pub info: Option<ImageInfo>,
    /// Raw EXIF payload retained from the encoded source.
    pub exif: Option<Vec<u8>>,
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

/// Host-neutral values exposed through Pillow's compatibility `Image.info`
/// mapping.
#[derive(Debug, Clone, PartialEq)]
pub enum ImageInfoValue {
    /// A scalar integer value.
    Integer(i64),
    /// A scalar floating-point value.
    Float(f64),
    /// A text value.
    String(String),
    /// An opaque byte value.
    Bytes(Vec<u8>),
    /// A list of integer values.
    IntegerList(Vec<i64>),
    /// A list of floating-point values.
    FloatList(Vec<f64>),
    /// A tuple of integer values.
    IntegerTuple(Vec<i64>),
    /// A nested mapping.
    Object(Vec<(String, ImageInfoValue)>),
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
    /// Raw EXIF payload retained from the encoded source.
    pub exif: Option<Vec<u8>>,
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

/// A Pillow pixel value after the core has applied the logical mode's band
/// shape. Bindings only need to map these variants to their native
/// scalar/tuple value types.
#[derive(Debug, Clone, PartialEq)]
pub enum FormattedPixelValue {
    /// An unsigned byte scalar from `1`, `L`, or `P` mode.
    Scalar(u8),
    /// A signed 32-bit scalar from `I` mode.
    Integer(i32),
    /// A 32-bit floating-point scalar from `F` mode, widened for bindings.
    Float(f64),
    /// A multiband pixel value in Pillow band order.
    Components(Vec<u8>),
}

/// Pillow's flat versus multiband `getdata` result after core formatting.
#[derive(Debug, Clone, PartialEq)]
pub enum FormattedImageData {
    /// Scalar samples, including an explicitly selected band.
    Scalars(Vec<u8>),
    /// Signed 32-bit scalar samples from `I` mode.
    IntegerScalars(Vec<i32>),
    /// 32-bit floating-point scalar samples from `F` mode, widened for bindings.
    FloatScalars(Vec<f64>),
    /// One component vector per multiband pixel.
    Components(Vec<Vec<u8>>),
}

/// Pillow's public extrema shape: a single-band image returns one pair, while
/// multiband images return one pair per band.
#[derive(Debug, Clone, PartialEq)]
pub enum FormattedExtrema {
    /// An image with a zero width or height has no extrema.
    Empty,
    /// An empty multiband image has one `None` value per band.
    EmptyMultiple(usize),
    /// One `(minimum, maximum)` pair.
    Single((u8, u8)),
    /// One `(minimum, maximum)` pair per band.
    Multiple(Vec<(u8, u8)>),
    /// A signed 32-bit scalar image's `(minimum, maximum)` pair.
    Integer((i32, i32)),
    /// A 32-bit floating-point scalar image's `(minimum, maximum)` pair.
    Float((f64, f64)),
}

fn pillow_band_count(mode: &str) -> usize {
    match mode {
        "L" | "1" | "P" | "I" | "F" | "I;16" | "I;16L" | "I;16B" | "I;16N" => 1,
        "LA" | "PA" => 2,
        "RGB" | "YCbCr" | "HSV" => 3,
        _ => 4,
    }
}

/// Host-neutral selector for `Image.getchannel`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelSelector {
    /// Numeric band index.
    Index(i32),
    /// Pillow band name such as `"R"` or `"A"`.
    Name(String),
    /// A host value that is neither an integer nor a string, retaining its
    /// type name for Pillow's public conversion error.
    Invalid(String),
}

/// Host-neutral value extracted for a public `Image.putpixel` call.
#[derive(Debug, Clone, PartialEq)]
pub enum PutPixelValue {
    /// An integer scalar, retaining its range for core validation.
    Integer(i64),
    /// A non-integral numeric value, accepted by Pillow's `F` mode.
    Float(f64),
    /// A list or tuple of byte components.
    Components(Vec<u8>),
    /// A value that is not a supported scalar or component sequence.
    Invalid,
}

/// Host-neutral input for Pillow's `Image.putalpha` wrapper.
#[derive(Debug, Clone)]
pub enum PutAlphaInput {
    /// An integer value. Pillow clamps this to one byte for values outside
    /// 0..255.
    Integer(i64),
    /// An image mask supplied by the caller.
    Image(Image),
    /// A value that cannot be interpreted as an integer or image mask.
    Invalid(String),
}

/// Host-neutral values extracted for a public `Image.new` color argument.
#[derive(Debug, Clone)]
pub struct PythonNewColorInput {
    string: Option<String>,
    single: Option<u8>,
    rgb: Option<(u8, u8, u8)>,
    rgba: Option<(u8, u8, u8, u8)>,
    luma_alpha: Option<(u8, u8)>,
    integer: Option<i32>,
    float: Option<f64>,
    provided: bool,
}

impl PythonNewColorInput {
    /// Builds a constructor input from binding-level type extraction only.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        string: Option<String>,
        single: Option<u8>,
        rgb: Option<(u8, u8, u8)>,
        rgba: Option<(u8, u8, u8, u8)>,
        luma_alpha: Option<(u8, u8)>,
        integer: Option<i32>,
        float: Option<f64>,
        provided: bool,
    ) -> Self {
        Self {
            string,
            single,
            rgb,
            rgba,
            luma_alpha,
            integer,
            float,
            provided,
        }
    }
}

/// Host-neutral mode input for the Python `Image.open` wrapper.
#[derive(Debug, Clone)]
pub enum PythonOpenModeInput {
    /// No mode override was supplied.
    None,
    /// A mode value extracted from the host.
    Name(String),
    /// A non-string host value.
    Invalid(String),
}

/// Host-neutral format allow-list input for the Python `Image.open` wrapper.
#[derive(Debug, Clone)]
pub enum PythonOpenFormatsInput {
    /// No format allow-list was supplied.
    None,
    /// A list or tuple of format names.
    Names(Vec<String>),
    /// A value that is not a list or tuple.
    Invalid(String),
}

/// Validates the Python `Image.open` mode and format arguments.
pub fn validate_python_open_inputs(
    mode: PythonOpenModeInput,
    formats: PythonOpenFormatsInput,
) -> Result<(), PilError> {
    match mode {
        PythonOpenModeInput::None => {}
        PythonOpenModeInput::Name(name) if name == "r" => {}
        PythonOpenModeInput::Name(name) => {
            return Err(PilError::ValueError(format!("bad mode '{name}'")));
        }
        PythonOpenModeInput::Invalid(value) => {
            // Pillow formats a non-string mode with its value directly; the
            // quotes are reserved for an invalid string mode.
            return Err(PilError::ValueError(format!("bad mode {value}")));
        }
    }
    if matches!(formats, PythonOpenFormatsInput::Invalid(_)) {
        return Err(PilError::TypeError(
            "formats must be a list or tuple".to_owned(),
        ));
    }
    Ok(())
}

/// Validates byte paths before the binding translates decode failures.
pub fn validate_python_open_source_bytes(data: &[u8]) -> Result<(), PilError> {
    if data.contains(&0) {
        return Err(PilError::ValueError("embedded null byte".to_owned()));
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum FromBytesMode {
    L,
    LA,
    L16,
    RGB,
    RGBA,
    CMYK,
    HSV,
    YCbCr,
    I,
    F,
    P,
    Mode1,
}

pub(crate) enum ScalarImageSamples {
    Integer(Vec<i32>),
    Float(Vec<f64>),
}

fn decode_integer_samples(raw: &[u8]) -> Vec<i32> {
    raw.chunks_exact(4)
        .map(|sample| i32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]))
        .collect()
}

fn decode_float_samples(raw: &[u8]) -> Vec<f64> {
    raw.chunks_exact(4)
        .map(|sample| f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]) as f64)
        .collect()
}

fn validate_scalar_storage(image: &DynamicImage, mode: &str) -> Result<(), PilError> {
    if !matches!(mode, "I" | "F") {
        return Ok(());
    }
    let expected = CheckedDims::new(image.width(), image.height(), 4)?.total_bytes();
    if image.as_bytes().len() != expected {
        return Err(PilError::InternalError(format!(
            "{mode} image has invalid scalar storage"
        )));
    }
    Ok(())
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

fn is_l16_mode(mode: &str) -> bool {
    matches!(mode, "I;16" | "I;16L" | "I;16B" | "I;16N")
}

fn putalpha_conversion_target(mode: &str) -> Option<&'static str> {
    match mode {
        "1" | "I" | "I;16" | "I;16L" | "I;16B" | "I;16N" | "F" => Some("LA"),
        "YCbCr" | "HSV" => Some("RGBA"),
        _ => None,
    }
}

fn l16_uses_big_endian(mode: &str) -> bool {
    match mode {
        "I;16B" => true,
        "I;16L" => false,
        "I;16" | "I;16N" => cfg!(target_endian = "big"),
        _ => false,
    }
}

fn putdata_l16_sample(value: &PutDataValue, scale: f64, offset: f64) -> Result<u16, PilError> {
    match value {
        // _imaging.c:_putdata treats I;16 as an unsigned 16-bit sample
        // destination. Rust's float-to-integer cast provides the same
        // bounded conversion for the public numeric path.
        PutDataValue::Number(number) => Ok((number * scale + offset) as u16),
        // Pillow's I;16 decoder reaches the flattened-sequence guard before
        // numeric coercion for nested/non-numeric values. Keep that public
        // error text exact instead of exposing the generic scalar path.
        _ => Err(PilError::TypeError("sequence must be flattened".to_owned())),
    }
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
            exif: None,
        })
    }

    /// Creates an internal L-mode mask, including Pillow's valid empty
    /// `(0, 0)` glyph-mask representation.
    ///
    /// Unlike public [`Image::frombytes`], this constructor is intended for
    /// algorithm-produced mask storage and therefore accepts zero dimensions.
    pub fn from_luma_mask(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, PilError> {
        let expected = (width as usize)
            .checked_mul(height as usize)
            .ok_or_else(|| PilError::ValueError("mask dimensions overflow".into()))?;
        if pixels.len() != expected {
            return Err(PilError::ValueError(format!(
                "mask byte length mismatch: expected {expected}, got {}",
                pixels.len()
            )));
        }
        let image = crate::raster::GrayImage::from_raw(width, height, pixels)
            .ok_or_else(|| PilError::InternalError("failed to construct L mask".into()))?;
        Ok(Self::from_dynamic(DynamicImage::ImageLuma8(image), None))
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
            "RGB" => DynamicImage::ImageRgb8(crate::raster::RgbImage::from_pixel(
                width,
                height,
                crate::raster::Rgb([color.0, color.1, color.2]),
            )),
            // Pillow's RGBa and RGBX keep an explicit mode tag over the same
            // four-byte storage layout as RGBA; preserve that tag at the
            // Image boundary.
            "RGBA" | "RGBa" | "RGBX" => {
                DynamicImage::ImageRgba8(crate::raster::RgbaImage::from_pixel(
                    width,
                    height,
                    crate::raster::Rgba([color.0, color.1, color.2, color.3]),
                ))
            }
            "L" => DynamicImage::ImageLuma8(crate::raster::GrayImage::from_pixel(
                width,
                height,
                crate::raster::Luma([color.0]),
            )),
            "LA" => DynamicImage::ImageLumaA8(crate::raster::GrayAlphaImage::from_pixel(
                width,
                height,
                crate::raster::LumaA([color.0, color.3]),
            )),
            // The binding packs the low 16 bits of an Image.new scalar into
            // color.0/color.1 in little-endian order. Keep the logical sample
            // native in the raster and apply the requested raw byte order only
            // when the public byte representation is requested.
            "I;16" | "I;16L" | "I;16B" | "I;16N" => {
                DynamicImage::ImageLuma16(crate::raster::ImageBuffer::from_pixel(
                    width,
                    height,
                    crate::raster::Luma([u16::from_le_bytes([color.0, color.1])]),
                ))
            }
            "PA" => DynamicImage::ImageLumaA8(crate::raster::GrayAlphaImage::from_pixel(
                width,
                height,
                crate::raster::LumaA([color.0, color.3]),
            )),
            "1" => DynamicImage::ImageLuma8(crate::raster::GrayImage::from_pixel(
                width,
                height,
                // PIL: stores the exact pixel value (0 or 1 or 255).
                // PIL's new("1") stores the raw color value as-is.
                crate::raster::Luma([color.0]),
            )),
            // A tuple color allocates palette entry zero. Scalar P fills use
            // `new_palette_index` because this resolved tuple no longer carries
            // the Python argument's scalar-versus-tuple distinction.
            "P" => {
                return Ok(Image::Paletted(PalettedData {
                    indices: crate::raster::GrayImage::from_pixel(
                        width,
                        height,
                        crate::raster::Luma([0u8]),
                    ),
                    // Pillow retains one structural RGB entry here. Keeping
                    // 768 zero-padded bytes would make getpalette() invent 255
                    // trailing black entries now that palette length is exact.
                    palette: vec![color.0, color.1, color.2],
                    palette_alpha: Vec::new(),
                    source_format: None,
                    info: None,
                    exif: None,
                    materialized: materialization_cache(),
                }));
            }
            "CMYK" => DynamicImage::ImageRgba8(crate::raster::RgbaImage::from_pixel(
                width,
                height,
                crate::raster::Rgba([color.0, color.1, color.2, color.3]),
            )),
            "YCbCr" | "HSV" => DynamicImage::ImageRgb8(crate::raster::RgbImage::from_pixel(
                width,
                height,
                crate::raster::Rgb([color.0, color.1, color.2]),
            )),
            // I and F modes store all four resolved int32/float32 LE bytes.
            "I" | "F" => DynamicImage::ImageRgba8(crate::raster::RgbaImage::from_pixel(
                width,
                height,
                crate::raster::Rgba([color.0, color.1, color.2, color.3]),
            )),
            _ => return Err(PilError::ValueError("unrecognized image mode".into())),
        };
        let explicit = if matches!(
            mode,
            "CMYK"
                | "YCbCr"
                | "HSV"
                | "I"
                | "F"
                | "RGBa"
                | "RGBX"
                | "PA"
                | "1"
                | "I;16"
                | "I;16L"
                | "I;16B"
                | "I;16N"
        ) {
            Some(mode.to_string())
        } else {
            None
        };
        Ok(Image::from_dynamic(img, explicit))
    }

    /// Creates an image using Pillow's host-facing `Image.new` color rules.
    ///
    /// Bindings extract host values into [`PythonNewColorInput`]; this method
    /// owns mode-specific color resolution and the distinction between scalar
    /// and tuple-created palette images.
    pub fn new_with_input(
        width: u32,
        height: u32,
        mode: &str,
        input: PythonNewColorInput,
    ) -> Result<Self, PilError> {
        let color = crate::color::resolve_new_color(
            mode,
            input.string.as_deref(),
            input.single,
            input.rgb,
            input.rgba,
            input.luma_alpha,
            input.integer,
            input.float,
        )?;
        if mode == "P" {
            if let Some(index) = input.single {
                return Ok(Self::new_palette_index(width, height, index));
            }
            if !input.provided {
                return Ok(Self::new_palette_index(width, height, 0));
            }
            let tuple_color = if let Some((r, g, b, a)) = input.rgba {
                if a != 255 {
                    return Err(PilError::ValueError(
                        "cannot add non-opaque RGBA color to RGB palette".to_owned(),
                    ));
                }
                (r, g, b, a)
            } else if let Some((r, g, b)) = input.rgb {
                (r, g, b, 255)
            } else {
                color
            };
            return Self::new(width, height, mode, tuple_color);
        }
        Self::new(width, height, mode, color)
    }

    /// Creates a `P` image filled with one raw palette index and no palette.
    ///
    /// Pillow distinguishes a scalar `Image.new("P", ..., index)` argument
    /// from a tuple color: the scalar is stored directly in every pixel while
    /// the image retains an empty palette.
    pub fn new_palette_index(width: u32, height: u32, index: u8) -> Self {
        Image::Paletted(PalettedData {
            indices: crate::raster::GrayImage::from_pixel(
                width,
                height,
                crate::raster::Luma([index]),
            ),
            palette: Vec::new(),
            palette_alpha: Vec::new(),
            source_format: None,
            info: None,
            exif: None,
            materialized: materialization_cache(),
        })
    }

    /// Creates an image from tightly packed raw bytes.
    ///
    /// `mode` uses Pillow mode names. Modes `L`, `LA`, `RGB`, `RGBA`, `CMYK`,
    /// `HSV`, `YCbCr`, `I`, `F`, and `P` expect one full pixel after another.
    /// The unsigned 16-bit luma raw modes `I;16`, `I;16L`, `I;16B`, and
    /// `I;16N` consume two bytes per sample with the mode's declared byte
    /// order.
    /// Mode `"1"` expects Pillow's packed bitmap layout: eight pixels per byte,
    /// most-significant bit first, with each row padded to a byte boundary.
    ///
    /// Extra bytes are ignored, matching Pillow's permissive `frombytes`
    /// behavior. Too few bytes is an error.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when allocation checks fail, the mode is
    /// unsupported, or `data` is shorter than the required mode layout.
    pub fn frombytes(mode: &str, size: (u32, u32), data: &[u8]) -> Result<Self, PilError> {
        let (w, h) = size;
        let frombytes_mode = match mode {
            "L" => FromBytesMode::L,
            "LA" => FromBytesMode::LA,
            "I;16" | "I;16L" | "I;16B" | "I;16N" => FromBytesMode::L16,
            "RGB" => FromBytesMode::RGB,
            "RGBA" | "RGBa" | "RGBX" => FromBytesMode::RGBA,
            "CMYK" => FromBytesMode::CMYK,
            "HSV" => FromBytesMode::HSV,
            "YCbCr" => FromBytesMode::YCbCr,
            "I" => FromBytesMode::I,
            "F" => FromBytesMode::F,
            "P" => FromBytesMode::P,
            "1" => FromBytesMode::Mode1,
            _ => return Err(PilError::ValueError("unrecognized image mode".into())),
        };
        if w == 0 || h == 0 {
            // Pillow accepts empty frombytes images and ignores the payload
            // because there are no samples to decode. Reuse the established
            // empty-image constructors while keeping CheckedDims strict for
            // all non-empty allocations.
            if matches!(frombytes_mode, FromBytesMode::P) {
                return Ok(Image::Paletted(PalettedData {
                    indices: crate::raster::GrayImage::from_pixel(w, h, crate::raster::Luma([0u8])),
                    palette: Vec::new(),
                    palette_alpha: Vec::new(),
                    source_format: None,
                    info: None,
                    exif: None,
                    materialized: materialization_cache(),
                }));
            }
            if matches!(frombytes_mode, FromBytesMode::L16) {
                return Ok(Self::from_dynamic(
                    DynamicImage::ImageLuma16(crate::raster::ImageBuffer::from_pixel(
                        w,
                        h,
                        crate::raster::Luma([0u16]),
                    )),
                    Some(mode.to_owned()),
                ));
            }
            return Self::new(w, h, mode, (0, 0, 0, 0));
        }
        let expected = match frombytes_mode {
            FromBytesMode::L | FromBytesMode::P => CheckedDims::new(w, h, 1)?.total_bytes(),
            FromBytesMode::LA => CheckedDims::new(w, h, 2)?.total_bytes(),
            FromBytesMode::L16 => CheckedDims::new(w, h, 2)?.total_bytes(),
            FromBytesMode::RGB | FromBytesMode::HSV | FromBytesMode::YCbCr => {
                CheckedDims::new(w, h, 3)?.total_bytes()
            }
            FromBytesMode::RGBA | FromBytesMode::CMYK | FromBytesMode::I | FromBytesMode::F => {
                CheckedDims::new(w, h, 4)?.total_bytes()
            }
            FromBytesMode::Mode1 => (w as usize).div_ceil(8) * h as usize,
        };
        if data.len() < expected {
            return Err(PilError::ValueError("not enough image data".into()));
        }
        let img = match frombytes_mode {
            FromBytesMode::L => DynamicImage::ImageLuma8(
                crate::raster::GrayImage::from_raw(w, h, data[..expected].to_vec())
                    .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
            ),
            FromBytesMode::L16 => {
                let pixels = data[..expected]
                    .chunks_exact(2)
                    .map(|sample| {
                        if l16_uses_big_endian(mode) {
                            u16::from_be_bytes([sample[0], sample[1]])
                        } else {
                            u16::from_le_bytes([sample[0], sample[1]])
                        }
                    })
                    .collect();
                DynamicImage::ImageLuma16(
                    crate::raster::ImageBuffer::from_raw(w, h, pixels)
                        .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
                )
            }
            FromBytesMode::RGB => DynamicImage::ImageRgb8(
                crate::raster::RgbImage::from_raw(w, h, data[..expected].to_vec())
                    .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
            ),
            FromBytesMode::RGBA => DynamicImage::ImageRgba8(
                crate::raster::RgbaImage::from_raw(w, h, data[..expected].to_vec())
                    .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
            ),
            FromBytesMode::LA => DynamicImage::ImageLumaA8(
                crate::raster::GrayAlphaImage::from_raw(w, h, data[..expected].to_vec())
                    .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
            ),
            FromBytesMode::P => {
                return Ok(Image::Paletted(PalettedData {
                    indices: crate::raster::GrayImage::from_raw(w, h, data[..expected].to_vec())
                        .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
                    // Raw P bytes are indices only. Pillow does not synthesize
                    // palette entries until a palette is explicitly attached.
                    palette: Vec::new(),
                    palette_alpha: Vec::new(),
                    source_format: None,
                    info: None,
                    exif: None,
                    materialized: materialization_cache(),
                }));
            }
            FromBytesMode::CMYK | FromBytesMode::I | FromBytesMode::F => DynamicImage::ImageRgba8(
                crate::raster::RgbaImage::from_raw(w, h, data[..expected].to_vec())
                    .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
            ),
            FromBytesMode::HSV | FromBytesMode::YCbCr => DynamicImage::ImageRgb8(
                crate::raster::RgbImage::from_raw(w, h, data[..expected].to_vec())
                    .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
            ),
            FromBytesMode::Mode1 => {
                // PIL packs 8 pixels per byte, MSB first, rows padded to byte boundary
                let row_bytes = (w as usize).div_ceil(8);
                let mut pixels = CheckedDims::new(w, h, 1)?.alloc_buffer();
                for y in 0..h as usize {
                    for x in 0..w as usize {
                        let byte_idx = y * row_bytes + x / 8;
                        let bit_idx = 7 - (x % 8); // MSB first
                        let val = if (data[byte_idx] >> bit_idx) & 1 != 0 {
                            255
                        } else {
                            0
                        };
                        pixels[y * w as usize + x] = val;
                    }
                }
                DynamicImage::ImageLuma8(
                    crate::raster::GrayImage::from_raw(w, h, pixels)
                        .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
                )
            }
        };
        let explicit_mode = match frombytes_mode {
            FromBytesMode::Mode1
            | FromBytesMode::CMYK
            | FromBytesMode::HSV
            | FromBytesMode::YCbCr
            | FromBytesMode::I
            | FromBytesMode::F => Some(mode.to_string()),
            FromBytesMode::RGBA if matches!(mode, "RGBa" | "RGBX") => Some(mode.to_string()),
            FromBytesMode::L16 => Some(mode.to_string()),
            FromBytesMode::L
            | FromBytesMode::LA
            | FromBytesMode::RGB
            | FromBytesMode::RGBA
            | FromBytesMode::P => None,
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
        let formats = format.map(|format| vec![format]);
        Self::open_bytes_with_formats(data, formats.as_deref())
    }

    /// Creates a lazy image from encoded bytes while restricting accepted formats.
    ///
    /// Pillow's ``Image.open(formats=...)`` argument is an allow-list, rather
    /// than a decoder selection hint. The header is still detected first; the
    /// image is rejected when its detected format is not present in the list.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when a format name is unknown, the encoded header
    /// is unknown or malformed, or no requested format matches the header.
    pub fn open_bytes_with_formats(
        data: Vec<u8>,
        formats: Option<&[&str]>,
    ) -> Result<Self, PilError> {
        let requested = formats
            .map(|formats| {
                formats
                    .iter()
                    .map(|format| parse_format_str(format))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        let data: Arc<[u8]> = data.into();
        let source = EncodedImage::new(Arc::clone(&data))
            .map_err(|error| map_codec_error(error, "memory"))?;
        let info = source.info().clone();
        if requested
            .as_ref()
            .is_some_and(|requested| !requested.contains(&info.format))
        {
            return Err(PilError::ValueError(format!(
                "requested formats {requested:?} but detected {:?}",
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
            // Pillow's Chops.c arithmetic also operates on raw indexed
            // samples for P images. The result remains P, but its newly
            // allocated core has no palette attached.
            | PipelineOp::Add { .. }
            | PipelineOp::Subtract { .. }
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
            PipelineOp::PutAlphaData {
                mode: crate::pipeline::PixelMode::P | crate::pipeline::PixelMode::PA,
                ..
            } => true,
            // Pillow keeps P/PA images indexed through ImageOps.pad. The
            // resize step uses nearest-neighbour samples for these modes,
            // and the pad fill is another raw index/alpha pair.
            PipelineOp::Pad { .. } => true,
            // Pillow's PA bands are the raw index and alpha bytes. Extracting
            // either band must happen before palette expansion, just like the
            // corresponding ImagingCore band operation.
            PipelineOp::ExtractBand { .. } => true,
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

    /// Returns whether an operation can stay in the source's indexed sample
    /// layout. Pillow's `libImaging/Filter.c` `PA` path applies convolution to
    /// the raw index/alpha bands and preserves the `PA` mode; unlike `P`, `PA`
    /// is a valid input for the built-in convolution filters. Keep this
    /// source-dependent exception out of [`Self::is_palette_safe_op`] so a
    /// direct filter on `P` cannot bypass `validate_filter` and become
    /// accepted.
    fn is_palette_safe_op_for_source(source: &Image, op: &PipelineOp) -> bool {
        Self::is_palette_safe_op(op)
            || (source.explicit_mode() == Some("PA")
                && matches!(
                    op,
                    PipelineOp::Filter3x3 { .. } | PipelineOp::Filter5x5 { .. }
                ))
            // Pillow keeps nearest-neighbor PA rotation in its native
            // index/alpha sample layout, including a two-element fillcolor.
            // The generic palette-safe rule only accepts a rotation without
            // a fill because P and PA fill values have different semantics;
            // this source-specific case carries the already-normalized PA
            // index/alpha pair through the native-channel rotate kernel.
            || (source.explicit_mode() == Some("PA")
                && matches!(
                    op,
                    PipelineOp::Rotate {
                        nearest: true,
                        ..
                    }
                ))
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
        let image = self.materialized_shared()?;
        let mode = self.mode_from_materialized(&image);
        validate_scalar_storage(&image, &mode)?;
        Ok(image.as_ref().clone())
    }

    /// Materialize a lazy image once before creating independent branches.
    ///
    /// Pillow's eager image objects can be cropped repeatedly without
    /// replaying an earlier resize.  Keeping the shared materialized pixels in
    /// a concrete image preserves that behavior for callers that branch a
    /// lazy pipeline, while each derived branch can still remain lazy.
    pub(crate) fn materialized_branch(&self) -> Result<Image, PilError> {
        match self {
            Image::Bytes { format, info, .. } => {
                // Pillow loads an encoded image before constructing a crop;
                // a deferred decode failure therefore belongs to crop(), not
                // to a later property access on the returned image.
                let image = self.materialized_shared()?;
                image_from_materialized(image, *format, info.clone(), self.exif_metadata())
            }
            Image::Pipeline { .. } => {
                let image = self.materialized_shared()?;
                Ok(Image::Loaded(LoadedData {
                    decoded_mode: image.color().into(),
                    explicit_mode: self.explicit_mode().map(str::to_owned),
                    palette: self.palette(),
                    palette_alpha: self.palette_alpha(),
                    source_format: None,
                    info: None,
                    exif: self.exif_metadata(),
                    image,
                }))
            }
            _ => Ok(self.clone()),
        }
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
                    decoded_to_dynamic(source.decode().map_err(map_deferred_decode_error)?)
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
        let selected = match backend {
            Some(backend) => backend,
            None => crate::compute::route(ops, None)?,
        };
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
        let selected = match backend {
            Some(backend) => backend,
            None => crate::compute::route(ops, None)?,
        };
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
            let all_safe = ops
                .iter()
                .all(|op| Self::is_palette_safe_op_for_source(source, op));
            if all_safe {
                let selected = match backend {
                    Some(backend) => backend,
                    None => crate::compute::route(ops, None)?,
                };
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

        let selected = match backend {
            Some(backend) => backend,
            None => crate::compute::route(ops, None)?,
        };
        img = crate::compute::execute_batch(selected, ops, &img, explicit_mode.as_deref())?;
        Ok(Arc::new(img))
    }

    fn materialize_uncached(&self) -> Result<DynamicImage, PilError> {
        match self {
            Image::Loaded(data) => Ok(data.image.as_ref().clone()),
            Image::Paletted(data) => Ok(DynamicImage::ImageLuma8(data.indices.clone())),
            Image::Bytes { source, .. } => decoded_to_dynamic(
                &image_slash_star::decode(source.bytes()).map_err(map_deferred_decode_error)?,
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
        let palette_safe = source_is_paletted && Self::is_palette_safe_op_for_source(source, &op);
        let putalpha_mode = match &op {
            PipelineOp::PutAlpha { mode, .. } | PipelineOp::PutAlphaData { mode, .. } => {
                Some(*mode)
            }
            _ => None,
        };
        let promotes_p_to_pa = source.has_palette_mode()
            && matches!(
                &op,
                PipelineOp::PutAlpha {
                    mode: crate::pipeline::PixelMode::P,
                    ..
                } | PipelineOp::PutAlphaData {
                    mode: crate::pipeline::PixelMode::P,
                    ..
                }
            );
        let explicit_mode = if matches!(&op, PipelineOp::ExtractBand { .. }) {
            None
        } else if source_is_paletted {
            if source.explicit_mode() == Some("PA") && matches!(&op, PipelineOp::PutPixel { .. }) {
                // PA writes keep the raw index/alpha sample layout even when
                // no RGB palette is attached, so later conversion still sees
                // the source as PA rather than generic LA.
                Some("PA".to_owned())
            } else {
                palette_safe.then(|| {
                    if source.explicit_mode() == Some("PA") {
                        "PA".to_owned()
                    } else {
                        "P".to_owned()
                    }
                })
            }
        } else {
            match &op {
                PipelineOp::Grayscale
                | PipelineOp::Convert { .. }
                | PipelineOp::Quantize { .. }
                | PipelineOp::ExtractBand { .. } => None,
                _ => source.explicit_mode().map(str::to_owned),
            }
        };
        // `push_op` always constructs an Image::Pipeline. Keep putalpha's
        // output-mode promotion here so P becomes PA without expanding its
        // indices, while CMYK becomes ordinary RGBA without a CMYK tag;
        // callers do not need an impossible fallback match after queuing it.
        let explicit_mode = match putalpha_mode {
            Some(crate::pipeline::PixelMode::P | crate::pipeline::PixelMode::PA) => {
                Some("PA".to_owned())
            }
            Some(crate::pipeline::PixelMode::CMYK) => None,
            _ => explicit_mode,
        };
        let preserve_palette = palette_safe && !matches!(&op, PipelineOp::ExtractBand { .. });
        let source_palette = if preserve_palette {
            source.extract_palette()
        } else {
            None
        };
        let source_palette_alpha = if preserve_palette {
            source.palette_alpha()
        } else {
            None
        };
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
                    // Pillow derived images (resize, crop, getchannel,
                    // convert, rotate, ...) report format None; only the
                    // originally opened image carries the container format.
                    format: None,
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
                backend,
                ..
            } => {
                let mut new_ops = ops.clone();
                new_ops.push(op);
                Image::Pipeline {
                    source: Arc::clone(pipeline_source),
                    ops: new_ops,
                    format: None,
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
                format: None,
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
            format: None,
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
        let img = self.materialized_shared()?;
        let (w, h) = (img.width(), img.height());
        if x >= w || y >= h {
            return Err(PilError::IndexError("image index out of range".into()));
        }
        let rgba = img.get_pixel(x, y).0;
        Ok((
            rgba[0],
            rgba.get(1).copied().unwrap_or(0),
            rgba.get(2).copied().unwrap_or(0),
            rgba.get(3).copied().unwrap_or(255),
        ))
    }

    /// Returns one pixel with Pillow's mode-specific scalar or tuple shape.
    pub fn getpixel_formatted(&self, x: u32, y: u32) -> Result<FormattedPixelValue, PilError> {
        let img = self.materialized_shared()?;
        let mode = self.mode_from_materialized(&img);
        let (width, height) = (img.width(), img.height());
        if mode == "I" || mode == "F" {
            if x >= width || y >= height {
                return Err(PilError::IndexError("image index out of range".into()));
            }
            let index = (y as usize)
                .checked_mul(width as usize)
                .and_then(|row| row.checked_add(x as usize))
                .ok_or_else(|| PilError::InternalError("pixel index overflow".into()))?;
            return match Self::scalar_samples_from_materialized(&img, &mode) {
                ScalarImageSamples::Integer(values) => values
                    .get(index)
                    .copied()
                    .map(FormattedPixelValue::Integer)
                    .ok_or_else(|| PilError::InternalError("I pixel index out of bounds".into())),
                ScalarImageSamples::Float(values) => values
                    .get(index)
                    .copied()
                    .map(FormattedPixelValue::Float)
                    .ok_or_else(|| PilError::InternalError("F pixel index out of bounds".into())),
            };
        }

        let (r, g, b, a) = self.getpixel(x, y)?;
        Ok(match mode.as_str() {
            "L" | "1" | "P" => FormattedPixelValue::Scalar(r),
            "LA" | "PA" => FormattedPixelValue::Components(vec![r, a]),
            "RGB" => FormattedPixelValue::Components(vec![r, g, b]),
            "RGBA" | "RGBa" | "RGBX" | "CMYK" => FormattedPixelValue::Components(vec![r, g, b, a]),
            _ => FormattedPixelValue::Components(vec![r, g, b]),
        })
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
        // Pillow's Image.putpixel creates an empty ImagePalette when a public
        // P-mode operation leaves no palette attached (for example, after
        // ImageDraw.bitmap). Treat the missing palette as an empty table so
        // tuple-color writes allocate the same first entry instead of raising
        // a binding-visible PaletteError.
        let mut palette = self.extract_palette().unwrap_or_default();
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
            // A scalar PA write supplies the palette index and leaves the
            // alpha sample transparent, matching Pillow's getink expansion.
            "LA" | "PA" => (v, 0, 0, 0),
            "RGB" => (v, 0, 0, 255),
            // Pillow treats a scalar as the first sample for every
            // three-band mode, including YCbCr and HSV; the remaining
            // samples are zero rather than copies of the scalar.
            "YCbCr" | "HSV" => (v, 0, 0, 255),
            "RGBA" | "RGBX" | "CMYK" => (v, 0, 0, 0),
            _ => (v, v, v, 255),
        };
        self.putpixel(x, y, r, g, b, a)
    }

    /// Writes a scalar pixel for Pillow's numeric `I` and `F` modes.
    ///
    /// Those modes preserve the host numeric value as a four-byte signed or
    /// floating-point sample. The ordinary byte-oriented mode helper remains
    /// intentionally narrow for `L`, `1`, and `P` inputs.
    pub fn putpixel_mode_scalar(
        &mut self,
        x: u32,
        y: u32,
        value: f64,
        mode: &str,
    ) -> Result<(), PilError> {
        if !matches!(mode, "I" | "F") {
            return self.putpixel_mode(x, y, value as u8, mode);
        }
        let (width, height) = self.size()?;
        if x >= width || y >= height {
            return Err(PilError::IndexError("image index out of range".into()));
        }
        let pixel_index = (y as usize)
            .checked_mul(width as usize)
            .and_then(|row| row.checked_add(x as usize))
            .ok_or_else(|| PilError::DimensionError("pixel index overflow".into()))?;
        self.putdata_value_at(pixel_index, &PutDataValue::Number(value), 1.0, 0.0)
    }

    /// Applies Pillow's public `putpixel` scalar/tuple normalization.
    pub fn putpixel_value(&mut self, x: u32, y: u32, value: PutPixelValue) -> Result<(), PilError> {
        let mode = self.mode()?;
        match value {
            PutPixelValue::Integer(value) => {
                if matches!(mode.as_str(), "I" | "F") {
                    return self.putpixel_mode_scalar(x, y, value as f64, &mode);
                }
                // Pillow's _imaging.c byte coercion clips scalar indices for
                // 1/L/P, while its multiband scalar path casts the first
                // sample to a byte. Keep that distinction in the Rust core so
                // every binding shares the same public behavior.
                let value = if matches!(mode.as_str(), "1" | "L" | "P") {
                    value.clamp(0, 255) as u8
                } else {
                    value as u8
                };
                self.putpixel_mode(x, y, value, &mode)
            }
            PutPixelValue::Float(value) => {
                if mode == "F" {
                    self.putpixel_mode_scalar(x, y, value, &mode)
                } else {
                    Err(if mode.len() == 1 {
                        PilError::TypeError("color must be int or single-element tuple".into())
                    } else {
                        PilError::TypeError("color must be int or tuple".into())
                    })
                }
            }
            PutPixelValue::Invalid => Err(if mode.len() == 1 {
                PilError::TypeError("color must be int or single-element tuple".into())
            } else {
                // Pillow routes non-integral component sequences through its
                // tuple-arity validator, rather than the scalar tuple error.
                PilError::TypeError(
                    "color must be int, or tuple of one, three or four elements".into(),
                )
            }),
            PutPixelValue::Components(values) => match values.as_slice() {
                [value] => self.putpixel_mode(x, y, *value, &mode),
                [value, alpha] => self.putpixel(x, y, *value, 0, 0, *alpha),
                [r, g, b] => self.putpixel(x, y, *r, *g, *b, 255),
                [r, g, b, a] => self.putpixel(x, y, *r, *g, *b, *a),
                _ => Err(PilError::TypeError(
                    "color must be int, or tuple of one, three or four elements".into(),
                )),
            },
        }
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

    /// Computes Pillow statistics after validating a transparency mask.
    ///
    /// The mask validation is kept in the core so every binding observes the
    /// same mode and size contract. Masked histogram accumulation remains a
    /// separate implementation concern; this method preserves the established
    /// unmasked statistics result for the current backend.
    pub fn stat_formatted_with_mask(
        &self,
        mask: crate::ops::imageops::ImageOpsMask,
    ) -> Result<StatResult, PilError> {
        crate::ops::imageops::validate_imageops_mask(self, mask)?;
        self.stat_formatted()
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
                // Pillow's I/F histogram path requires a finite min/max and
                // rejects an empty image before constructing ImageStat.Stat.
                return Err(PilError::ValueError("min/max not given".into()));
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
                // Pillow's I/F histogram is empty when every value is equal
                // (max == min), so its Stat min/max fall back to the histogram
                // defaults of 255 and 0 for every band.
                return Ok(vec![vec![
                    n_pixels as f64,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    255.0,
                    0.0,
                ]]);
            }
            let scale = 255.0 / (max_val - min_val);
            let mut hist = [0i64; 256];
            for &v in &values {
                // Pillow assigns the maximum sample to histogram bin 255.
                // Avoid letting a representationally-tiny floating error turn
                // the endpoint into bin 254.
                let bin = if (v - max_val).abs() < f64::EPSILON {
                    255
                } else {
                    ((v - min_val) * scale).clamp(0.0, 255.0) as usize
                };
                // `clamp` bounds every finite value to the histogram domain;
                // Rust's float-to-`usize` conversion also maps NaN to zero.
                // The index is therefore always one of the 256 slots.
                hist[bin] += 1;
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
                // Pillow's byte-oriented histogram starts an empty band at
                // extrema (255, 0), while all aggregate statistics remain 0.
                results.push(vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 255.0, 0.0]);
                continue;
            }
            let sum: f64 = band.iter().map(|&x| x as f64).sum();
            let sum2: f64 = band.iter().map(|&x| (x as f64) * (x as f64)).sum();
            let mean = sum / count;
            let rms = (sum2 / count).sqrt();
            // Pillow 12.2 returns the raw floating-point expression here. It
            // intentionally preserves tiny negative round-off values (for
            // example, a 7,292,605-pixel uniform L=182 image produces
            // -4.184729342258356e-12), so do not clamp the result.
            let var = (sum2 - sum * sum / count) / count;
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
    /// Returns [`PilError`] when neither the retained mode metadata nor lazy
    /// image data is sufficient to determine the bands.
    pub fn getbands(&self) -> Result<Vec<String>, PilError> {
        // Pillow's Image.getbands -> ImagingCore.getbands derives names from
        // canonical mode metadata without touching pixel storage. `mode()`
        // already preserves explicit tags for non-standard modes and can read
        // a decoder header before pixel data is available, so this handles
        // truncated lazy images without a duplicate pixel-channel fallback.
        let mode = self.mode()?;
        let bands: Vec<&str> = match mode.as_str() {
            "CMYK" => vec!["C", "M", "Y", "K"],
            "YCbCr" => vec!["Y", "Cb", "Cr"],
            "HSV" => vec!["H", "S", "V"],
            "PA" => vec!["P", "A"],
            "RGBa" => vec!["R", "G", "B", "a"],
            "RGBX" => vec!["R", "G", "B", "X"],
            "I" | "F" | "P" | "1" => {
                vec![mode.as_str()]
            }
            // Pillow exposes all unsigned 16-bit luma variants as one logical
            // band named "I", regardless of their byte order.
            "I;16" | "I;16L" | "I;16B" | "I;16N" => vec!["I"],
            "L" => vec!["L"],
            "LA" => vec!["L", "A"],
            "RGB" => vec!["R", "G", "B"],
            "RGBA" => vec!["R", "G", "B", "A"],
            _ => Vec::new(),
        };
        Ok(bands.into_iter().map(str::to_string).collect())
    }

    /// Encodes the image using the requested Pillow format name.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when the format is unsupported or image
    /// materialization or encoding fails.
    pub fn encode(&self, format: &str) -> Result<Vec<u8>, PilError> {
        let save_format = parse_format_str(format)?;
        let upper_format = format.to_ascii_uppercase();
        let mode = self.mode()?;
        // Pillow's encoders reject unsupported mode/format combinations with
        // an OSError naming both; replicate the public messages exactly.
        let rejection = match upper_format.as_str() {
            "PNG" if mode == "F" => Some("cannot write mode F as PNG"),
            "PNG" if mode == "CMYK" => Some("cannot write mode CMYK as PNG"),
            "BMP" if mode == "I" => Some("cannot write mode I as BMP"),
            "BMP" if mode == "F" => Some("cannot write mode F as BMP"),
            "BMP" if mode == "CMYK" => Some("cannot write mode CMYK as BMP"),
            _ => None,
        };
        if let Some(message) = rejection {
            return Err(PilError::OsError(message.into()));
        }
        if upper_format == "PNG" {
            return self.to_png_bytes();
        }
        let decoded = self.decoded_for_encoding()?;
        Ok(image_slash_star::encode_default(&decoded, save_format)?)
    }

    /// Resolves Pillow's explicit save format or a host-supplied path
    /// extension before encoding.
    ///
    /// Filesystem access and extension extraction remain in host bindings, but
    /// format validity and Pillow's public error categories are shared here.
    pub fn resolve_save_format(
        explicit: Option<&str>,
        extension: Option<&str>,
    ) -> Result<String, PilError> {
        if let Some(format) = explicit {
            parse_format_str(format)
                .map(|_| format.to_owned())
                .map_err(|_| PilError::KeyError(format.to_owned()))
        } else {
            let extension = extension.ok_or_else(|| {
                // Pillow's Image.save extension lookup reports an empty
                // suffix as a ValueError with this exact public message.
                PilError::ValueError("unknown file extension: ".into())
            })?;
            parse_format_str(extension)
                .map(|_| extension.to_owned())
                .map_err(|_| PilError::ValueError(format!("unknown file extension: .{extension}")))
        }
    }

    /// Returns raw image bytes in the image's current Pillow mode.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization or mode-specific byte packing
    /// fails.
    pub fn tobytes(&self) -> Result<Vec<u8>, PilError> {
        let mode = self.mode()?;
        self.tobytes_formatted(&mode)
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

        if is_l16_mode(mode) {
            let native_is_big = cfg!(target_endian = "big");
            let raw_is_big = l16_uses_big_endian(mode);
            let bytes = img.as_bytes();
            if native_is_big == raw_is_big {
                return Ok(bytes.to_vec());
            }
            let mut swapped = Vec::with_capacity(bytes.len());
            for sample in bytes.chunks_exact(2) {
                swapped.extend_from_slice(&[sample[1], sample[0]]);
            }
            return Ok(swapped);
        }

        // For mode "1" images, pack 8 pixels per byte (MSB first) matching PIL.
        if mode == "1" && img.color() == crate::raster::ColorType::L8 {
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

    /// Returns public image bytes using Pillow's raw encoder arguments.
    ///
    /// `BGR` and `BGRA` preserve channel width while exchanging red and blue;
    /// raw `RGBA` from an RGB image appends Pillow's opaque filler alpha.
    /// Other encoder names or raw modes retain the ordinary mode byte layout.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization or mode-specific byte packing
    /// fails.
    pub fn tobytes_encoded(
        &self,
        mode: &str,
        encoder_name: &str,
        args: &[String],
    ) -> Result<Vec<u8>, PilError> {
        if encoder_name != "raw" {
            return Err(PilError::IOError(format!(
                "encoder {encoder_name} not available"
            )));
        }
        let raw_mode = args.first().map(String::as_str).unwrap_or(mode);
        let mode_supports_raw = raw_mode == mode
            || (mode == "RGB" && matches!(raw_mode, "BGR" | "RGBA"))
            || (mode == "RGBA" && raw_mode == "BGRA");
        if !mode_supports_raw {
            return Err(PilError::ValueError(format!(
                "No packer found from {mode} to {raw_mode}"
            )));
        }
        let mut data = self.tobytes_formatted(mode)?;
        if mode == "RGB" && raw_mode == "RGBA" {
            let mut expanded = Vec::with_capacity(data.len() / 3 * 4);
            for pixel in data.chunks_exact(3) {
                expanded.extend_from_slice(pixel);
                expanded.push(255);
            }
            data = expanded;
        }
        match args.first().map(String::as_str) {
            Some("BGRA") => {
                for pixel in data.chunks_exact_mut(4) {
                    pixel.swap(0, 2);
                }
            }
            Some("BGR") => {
                for pixel in data.chunks_exact_mut(3) {
                    pixel.swap(0, 2);
                }
            }
            _ => {}
        }
        Ok(data)
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
    /// Some Pillow modes cannot be represented by the generic raster variants
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
                .map(|palette| palette.rgb.clone()),
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

    /// Converts the retained RGB palette to a Pillow ``getpalette(rawmode)``
    /// layout.
    ///
    /// Pillow's C `getpalette` accepts ``"RGB"``, ``"RGBA"`` (real per-entry
    /// alpha), ``"RGBX"`` (alpha forced to 255, only for palettes without
    /// alpha data), and the single-channel selectors ``"R"``/``"G"``/``"B"``;
    /// every other rawmode raises ``ValueError: unrecognized raw mode``.
    pub fn getpalette_rawmode(&self, rawmode: &str) -> Result<Option<Vec<u8>>, PilError> {
        let Some(palette) = self.getpalette_trimmed() else {
            return Ok(None);
        };
        // Pillow's Image.getpalette reads alpha from the committed palette
        // object, not from pending info["transparency"] decoded alongside a
        // P image. Keep the pending marker available to Image.info and
        // apply_transparency, but do not expose it as RGBA palette bytes yet.
        let palette_alpha = if self.pending_palette_transparency().is_none() {
            self.palette_alpha().unwrap_or_default()
        } else {
            Vec::new()
        };
        let has_alpha = !palette_alpha.is_empty();
        match rawmode {
            "RGB" => Ok(Some(palette)),
            "RGBA" => Ok(self.getpalette_rgba()),
            "RGBX" => {
                // Pillow has no RGBA->RGBX packer, so palettes carrying alpha
                // data raise "unrecognized raw mode" for RGBX; palettes
                // without alpha pack to RGB plus a 255 pad byte.
                if has_alpha {
                    return Err(PilError::ValueError("unrecognized raw mode".into()));
                }
                let mut out = Vec::with_capacity(palette.len() / 3 * 4);
                for color in palette.chunks_exact(3) {
                    out.extend_from_slice(color);
                    out.push(255);
                }
                Ok(Some(out))
            }
            "R" | "G" | "B" => {
                let channel = match rawmode {
                    "R" => 0,
                    "G" => 1,
                    _ => 2,
                };
                Ok(Some(
                    palette
                        .chunks_exact(3)
                        .map(|color| color[channel])
                        .collect(),
                ))
            }
            _ => Err(PilError::ValueError("unrecognized raw mode".into())),
        }
    }

    /// Returns palette data using Pillow's public raw-mode and missing-palette
    /// behavior.
    pub fn getpalette_with_input(
        &self,
        rawmode: Option<&str>,
    ) -> Result<Option<Vec<u8>>, PilError> {
        let rawmode = rawmode.or_else(|| self.palette_mode()).unwrap_or("RGB");
        let palette = self.getpalette_rawmode(rawmode)?;
        if palette.is_none() && matches!(self.mode()?.as_str(), "P" | "PA") {
            return Ok(Some(Vec::new()));
        }
        Ok(palette)
    }

    /// Returns RGB triples for an indexed Qt-compatible color table.
    ///
    /// Qt owns the final `qRgb` packing and `QImage` object construction; the
    /// Rust core owns the mode-aware table values so palette traversal is
    /// shared with every binding.
    pub fn indexed_color_table(&self, mode: &str) -> Result<Vec<(u8, u8, u8)>, PilError> {
        match mode {
            "L" => Ok((0..=u8::MAX).map(|value| (value, value, value)).collect()),
            "P" => Ok(self
                .getpalette_with_input(Some("RGB"))?
                .unwrap_or_default()
                .chunks_exact(3)
                .map(|color| (color[0], color[1], color[2]))
                .collect()),
            _ => Ok(Vec::new()),
        }
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
        let exif = self.exif_metadata();
        let materialized = self.materialize()?;

        *self = match mode.as_str() {
            "L" | "P" => Image::Paletted(PalettedData {
                indices: materialized.to_luma8(),
                palette,
                palette_alpha,
                source_format,
                info,
                exif,
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
                    exif,
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

    /// Returns the pending transparency value in the shape exposed by
    /// Pillow's `Image.info` mapping.
    pub fn pending_transparency_info(&self) -> Option<ImageInfoValue> {
        match self.pending_palette_transparency()? {
            PaletteTransparency::Index(index) => Some(ImageInfoValue::Integer(i64::from(index))),
            PaletteTransparency::Table(alpha) => Some(ImageInfoValue::Bytes(alpha)),
        }
    }

    /// Returns compatibility metadata that belongs in Pillow's `Image.info`.
    ///
    /// The format defaults mirror the Python compatibility surface used by
    /// this crate. Keeping their selection beside the source format and the
    /// pending palette metadata makes copies and every binding observe the
    /// same rules without duplicating format branches in host code.
    pub fn compatibility_info(&self) -> Vec<(String, ImageInfoValue)> {
        let format = self
            .source_format()
            .or_else(|| self.image_info().map(|info| info.format));
        // image-slash-star currently retains the container format but not
        // exact JPEG density or WebP frame timing. JPEG's 72-dpi fallback is
        // added only when retained EXIF provenance makes that Pillow result
        // observable. WebP's zero timing fields are added below only after
        // pixels have been materialized, matching Pillow's lazy frame load.
        let mut fields = match format {
            Some(ImageFormat::Jpeg) => {
                let mut fields = vec![
                    ("jfif".to_owned(), ImageInfoValue::Integer(257)),
                    (
                        "jfif_version".to_owned(),
                        ImageInfoValue::IntegerTuple(vec![1, 1]),
                    ),
                    ("jfif_unit".to_owned(), ImageInfoValue::Integer(0)),
                    (
                        "jfif_density".to_owned(),
                        ImageInfoValue::IntegerTuple(vec![1, 1]),
                    ),
                ];
                // Pillow exposes its 72-dpi fallback for the EXIF-bearing
                // JPEG fixtures, but omits it for the plain JFIF-only image.
                // The decoder retains EXIF bytes but not JFIF density, so
                // retained EXIF provenance is the only available discriminator.
                if self.exif_metadata().is_some() {
                    fields.push(("dpi".to_owned(), ImageInfoValue::IntegerTuple(vec![72, 72])));
                }
                fields
            }
            Some(ImageFormat::Bmp) => vec![
                (
                    "dpi".to_owned(),
                    ImageInfoValue::FloatList(vec![96.01194815354799, 96.01194815354799]),
                ),
                ("compression".to_owned(), ImageInfoValue::Integer(0)),
            ],
            Some(ImageFormat::Gif) => vec![
                (
                    "version".to_owned(),
                    ImageInfoValue::Object(vec![
                        (
                            "kind".to_owned(),
                            ImageInfoValue::String("bytes".to_owned()),
                        ),
                        (
                            "encoding".to_owned(),
                            ImageInfoValue::String("base64".to_owned()),
                        ),
                        (
                            "data".to_owned(),
                            ImageInfoValue::String("R0lGODdh".to_owned()),
                        ),
                    ]),
                ),
                ("background".to_owned(), ImageInfoValue::Integer(0)),
            ],
            Some(ImageFormat::Tiff) => vec![
                (
                    "compression".to_owned(),
                    ImageInfoValue::String("raw".to_owned()),
                ),
                ("dpi".to_owned(), ImageInfoValue::IntegerList(vec![1, 1])),
                (
                    "resolution".to_owned(),
                    ImageInfoValue::IntegerList(vec![1, 1]),
                ),
            ],
            Some(ImageFormat::WebP) => {
                let mut fields = vec![
                    ("loop".to_owned(), ImageInfoValue::Integer(1)),
                    (
                        "background".to_owned(),
                        ImageInfoValue::IntegerList(vec![255, 255, 255, 255]),
                    ),
                ];
                if self.is_materialized() {
                    fields.extend([
                        ("timestamp".to_owned(), ImageInfoValue::Integer(0)),
                        ("duration".to_owned(), ImageInfoValue::Integer(0)),
                    ]);
                }
                fields
            }
            _ => Vec::new(),
        };
        if matches!(format, Some(ImageFormat::Jpeg))
            && let Some(exif) = self.exif_metadata()
        {
            fields.push(("exif".to_owned(), ImageInfoValue::Bytes(exif)));
        }
        if let Some(transparency) = self.pending_transparency_info() {
            fields.push(("transparency".to_owned(), transparency));
        }
        fields
    }

    /// Returns conversion-time transparency metadata in Pillow's public
    /// scalar, byte-string, or tuple representation.
    pub fn converted_transparency_info(&self, target_mode: &str) -> Option<ImageInfoValue> {
        if matches!(target_mode, "LA" | "RGBA") && self.explicit_mode() == Some("PA") {
            return self.pending_transparency_info();
        }
        let transparency = self.converted_palette_transparency(target_mode)?;
        match transparency.as_slice() {
            [value] => Some(ImageInfoValue::Integer(i64::from(*value))),
            values => Some(ImageInfoValue::IntegerTuple(
                values.iter().map(|&value| i64::from(value)).collect(),
            )),
        }
    }

    /// Returns conversion-time fields that should be merged into the target
    /// image's Pillow compatibility metadata.
    pub fn converted_compatibility_info(&self, target_mode: &str) -> Vec<(String, ImageInfoValue)> {
        self.converted_transparency_info(target_mode)
            .map(|value| vec![("transparency".to_owned(), value)])
            .unwrap_or_default()
    }

    /// Returns the `info["transparency"]` value a Pillow `convert` to
    /// `target_mode` would carry for a palette image.
    ///
    /// Pillow converts a single transparency index through the palette when
    /// the target has no alpha band (RGB keeps the palette color, L its BT.601
    /// gray level) and drops transparency entirely for alpha targets (RGBA,
    /// LA, PA) and for byte-table transparency.
    pub fn converted_palette_transparency(&self, target_mode: &str) -> Option<Vec<u8>> {
        if !matches!(target_mode, "L" | "RGB") {
            return None;
        }
        let index = match self.pending_palette_transparency()? {
            PaletteTransparency::Index(index) => index,
            PaletteTransparency::Table(_) => return None,
        };
        let palette = self.palette()?;
        let base = usize::from(index) * 3;
        let r = *palette.get(base)?;
        let g = *palette.get(base + 1)?;
        let b = *palette.get(base + 2)?;
        if target_mode == "RGB" {
            Some(vec![r, g, b])
        } else {
            // BT.601 fixed-point with the same rounding bias as pil_grayscale.
            let y = (19595u32 * u32::from(r)
                + 38470u32 * u32::from(g)
                + 7471u32 * u32::from(b)
                + 32768)
                >> 16;
            Some(vec![y.min(255) as u8])
        }
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
        let Some(pending) = self.pending_palette_transparency() else {
            return Ok(());
        };
        if !self.has_palette_mode() {
            return Ok(());
        }

        // Pillow's Image.py:apply_transparency starts from getpalette("RGBA")
        // and overlays the pending info value. That matters after putpalette:
        // the new palette may have no alpha table even though the source image
        // still carries an indexed transparency marker.
        let palette = self.palette().unwrap_or_else(default_palette);
        let palette_entries = palette.len() / 3;
        let mut palette_alpha = self.palette_alpha().unwrap_or_default();
        palette_alpha.resize(palette_entries, 255);
        palette_alpha.truncate(palette_entries);
        match pending {
            PaletteTransparency::Index(index) => {
                if let Some(alpha) = palette_alpha.get_mut(usize::from(index)) {
                    *alpha = 0;
                } else {
                    // Pillow's Image.apply_transparency writes the pending
                    // index directly into the palette alpha list. A retained
                    // transparency index beyond a shortened putpalette raises
                    // the list-assignment IndexError instead of being ignored.
                    return Err(PilError::IndexError(
                        "list assignment index out of range".into(),
                    ));
                }
            }
            PaletteTransparency::Table(table) => {
                for (alpha, value) in palette_alpha.iter_mut().zip(table) {
                    *alpha = value;
                }
            }
        }

        if let Image::Paletted(data) = self {
            data.palette_alpha = palette_alpha;
        } else {
            let indices = self.materialize()?.to_luma8();
            let source_format = self.source_format();
            let info = self.image_info();
            let exif = self.exif_metadata();
            *self = Image::Paletted(PalettedData {
                indices,
                palette,
                palette_alpha,
                source_format,
                info,
                exif,
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

    /// Returns EXIF metadata bytes retained from the encoded source.
    ///
    /// JPEG sources carry the EXIF payload in their APP1 segment; TIFF
    /// sources are themselves a TIFF IFD0 container.  Other formats currently
    /// expose no EXIF bytes, matching Pillow's empty `Exif` object for images
    /// without embedded metadata.
    pub fn getexif(&self) -> Vec<u8> {
        match self {
            Image::Loaded(data) => data.exif.clone().unwrap_or_default(),
            Image::Paletted(data) => data.exif.clone().unwrap_or_default(),
            Image::Pipeline { source, .. } => source.getexif(),
            Image::Bytes { source, .. } => match source.info().format {
                ImageFormat::Jpeg => Self::extract_jpeg_exif(source.bytes()).unwrap_or_default(),
                ImageFormat::Tiff => {
                    // Pillow builds its `Exif` object from the TIFF IFD0
                    // itself; the raw container bytes are the same IFD
                    // payload the orientation parser consumes.
                    source.bytes().to_vec()
                }
                _ => Vec::new(),
            },
        }
    }

    /// Returns retained EXIF bytes in the optional form used by materialized
    /// and derived image storage.
    pub(crate) fn exif_metadata(&self) -> Option<Vec<u8>> {
        let exif = self.getexif();
        (!exif.is_empty()).then_some(exif)
    }

    /// Extracts the Exif APP1 payload from a JPEG byte stream.
    ///
    /// Scans the sequential marker list for `0xFFE1` and returns the segment
    /// payload when it starts with the Exif signature.  This mirrors Pillow's
    /// `JpegImagePlugin` APP1 handling, which retains the payload starting at
    /// the `Exif\0\0` signature.
    fn extract_jpeg_exif(data: &[u8]) -> Option<Vec<u8>> {
        if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 {
            return None;
        }
        let mut offset = 2usize;
        while offset + 4 <= data.len() {
            if data[offset] != 0xFF {
                return None;
            }
            let marker = data[offset + 1];
            // Standalone markers (RST0-7, TEM) and the SOI marker carry no
            // length field.
            if marker == 0xD8 || (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
                offset += 2;
                continue;
            }
            if marker == 0xD9 {
                return None;
            }
            // Pillow's JpegImagePlugin stops collecting header metadata at
            // SOS and hands the remainder to the entropy decoder. Do not
            // interpret entropy bytes (including later marker-like bytes) as
            // additional APP segments.
            if marker == 0xDA {
                return None;
            }
            let length = usize::from(u16::from_be_bytes([data[offset + 2], data[offset + 3]]));
            if length < 2 {
                return None;
            }
            let segment_end = offset + 2 + length;
            if segment_end > data.len() {
                return None;
            }
            if marker == 0xE1 {
                let payload = &data[offset + 4..segment_end];
                if payload.len() > 6 && payload.starts_with(b"Exif\x00\x00") {
                    return Some(payload.to_vec());
                }
                return None;
            }
            offset = segment_end;
        }
        None
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
            let mut palette_bytes = operational_palette(data);
            if palette_bytes.len() < 768 {
                // The underlying indexed encoder requires a full 256-entry
                // palette; Pillow synthesizes missing entries at save time.
                palette_bytes.resize(768, 0);
            }
            let palette = ImagePalette::new(palette_bytes, data.palette_alpha.clone())?;
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
            let mut palette = self.extract_palette().ok_or_else(|| {
                PilError::PaletteError("P-mode pipeline has no retained palette".to_owned())
            })?;
            if palette.len() < 768 {
                palette.resize(768, 0);
            }
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
        Ok(self.mode_from_materialized(&img))
    }

    /// Returns the mode after the caller has already materialized this image.
    ///
    /// The fallible [`Image::mode`] method must decode lazy inputs when no
    /// header mode is available. Once a concrete buffer is in hand, that
    /// operation cannot fail; keeping this path infallible also makes it clear
    /// that mode selection does not trigger a second decode.
    pub(crate) fn mode_from_materialized(&self, image: &DynamicImage) -> String {
        match self {
            Image::Paletted(_) => "P".to_owned(),
            Image::Loaded(data) => data
                .explicit_mode
                .clone()
                .unwrap_or_else(|| image_mode_name(data.decoded_mode).to_owned()),
            Image::Bytes {
                info: Some(info), ..
            } => image_mode_name(info.mode).to_owned(),
            Image::Pipeline {
                explicit_mode: Some(mode),
                ..
            } => mode.clone(),
            _ => color_type_to_mode(image.color()).to_owned(),
        }
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
            Image::Bytes { format, info, .. } => image_from_materialized(
                self.materialized_shared()?,
                *format,
                info.clone(),
                self.exif_metadata(),
            )?,
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
                            exif: source.exif_metadata(),
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
                        exif: source.exif_metadata(),
                    })
                }
            }
        };
        *self = loaded;
        Ok(())
    }

    /// Replaces retained EXIF bytes after a metadata-only image operation.
    ///
    /// The operation is intentionally applied after loading so the lazy image
    /// representation remains the single source of truth for pixel and
    /// metadata state.
    pub(crate) fn with_exif_metadata(mut self, exif: Option<Vec<u8>>) -> Result<Self, PilError> {
        self.load()?;
        match &mut self {
            Image::Loaded(data) => data.exif = exif,
            Image::Paletted(data) => data.exif = exif,
            Image::Bytes { .. } | Image::Pipeline { .. } => {
                unreachable!("load must materialize EXIF metadata target")
            }
        }
        Ok(self)
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
        // Pillow's copy returns a fresh image whose `format` is None even when
        // the source came from a file; only the originally opened image
        // carries the container format.
        let mut copy = self.clone();
        match &mut copy {
            Image::Loaded(data) => data.source_format = None,
            Image::Paletted(data) => data.source_format = None,
            Image::Bytes { format, .. } => *format = None,
            Image::Pipeline { format, .. } => *format = None,
        }
        copy
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
        if let Some(band) = band {
            // Pillow validates the requested logical band before extracting
            // it. Keep this check in the core so every binding observes the
            // same contract instead of silently clamping to RGBA channel 3.
            let mode = self.mode_from_materialized(&img);
            let band_count = pillow_band_count(&mode);
            if band < 0 || band as usize >= band_count {
                return Err(PilError::ValueError("band index out of range".into()));
            }
            let rgba = img.to_rgba8();
            let b = band as usize;
            return Ok(rgba.pixels().map(|p| p[b]).collect());
        }
        match img.color() {
            crate::raster::ColorType::L8 | crate::raster::ColorType::L16 => {
                let gray = img.to_luma8();
                Ok(gray.into_raw())
            }
            crate::raster::ColorType::La8 | crate::raster::ColorType::La16 => {
                let ga = img.to_luma_alpha8();
                let mut out = Vec::with_capacity((ga.width() * ga.height() * 2) as usize);
                for p in ga.pixels() {
                    out.push(p[0]);
                    out.push(p[1]);
                }
                Ok(out)
            }
            crate::raster::ColorType::Rgb8
            | crate::raster::ColorType::Rgb16
            | crate::raster::ColorType::Rgb32F => {
                let rgb = img.to_rgb8();
                Ok(rgb.into_raw())
            }
            _ => {
                let rgba = img.to_rgba8();
                Ok(rgba.into_raw())
            }
        }
    }

    /// Returns `getdata` values with Pillow's scalar/multiband shape applied.
    ///
    /// The byte extraction and logical band count are core behavior. Host
    /// bindings should only convert [`FormattedImageData`] into their native
    /// list/tuple representation.
    pub fn getdata_formatted(&self, band: Option<i32>) -> Result<FormattedImageData, PilError> {
        let mode = self.mode()?;
        if mode == "I" || mode == "F" {
            if band.is_some() {
                return Err(PilError::ValueError("image has wrong mode".into()));
            }
            return Ok(match self.scalar_samples(&mode)? {
                ScalarImageSamples::Integer(values) => FormattedImageData::IntegerScalars(values),
                ScalarImageSamples::Float(values) => FormattedImageData::FloatScalars(values),
            });
        }
        let raw = self.getdata(band)?;
        if band.is_some() {
            return Ok(FormattedImageData::Scalars(raw));
        }

        let band_count = pillow_band_count(&mode);
        if band_count == 1 {
            return Ok(FormattedImageData::Scalars(raw));
        }

        Ok(FormattedImageData::Components(
            raw.chunks_exact(band_count)
                .map(|pixel| pixel.to_vec())
                .collect(),
        ))
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
        self.validate_putdata_length(values.len())?;

        let mode_name = self.mode()?;
        if is_l16_mode(&mode_name) {
            // Keep I;16 out of PixelMode's byte-oriented deferred path. Its
            // generic fallback widens to RGBA8 and loses the 16-bit sample.
            for (pixel_index, value) in values.iter().enumerate() {
                self.putdata_value_at(pixel_index, value, scale, offset)?;
            }
            return Ok(());
        }
        let mode = crate::pipeline::PixelMode::from_name(&mode_name).ok_or_else(|| {
            PilError::ValueError(format!("unsupported putdata mode: {mode_name}"))
        })?;
        let data = putdata_bytes(mode, values, scale, offset)?;
        self.putdata(&data)
    }

    /// Replaces numeric samples through the shared `putdata` normalization path.
    ///
    /// Bindings use this callback-free entry point for exact built-in numeric
    /// sequences. Keeping the conversion in core reaches the same bulk path
    /// for every binding while custom Python values can still use
    /// [`Image::putdata_value_at`] to preserve re-entrant coercion order.
    pub fn putdata_numeric_values(
        &mut self,
        values: &[f64],
        scale: f64,
        offset: f64,
    ) -> Result<(), PilError> {
        let values = values
            .iter()
            .copied()
            .map(PutDataValue::Number)
            .collect::<Vec<_>>();
        self.putdata_values(&values, scale, offset)
    }

    /// Validates the sequence length before host-language value coercion.
    ///
    /// Pillow rejects an oversized `putdata` sequence before converting any
    /// element, so bindings must delegate this check before they iterate a
    /// Python sequence. Keeping it here also gives every binding the same
    /// dimension and error behavior.
    pub fn validate_putdata_length(&self, entry_count: usize) -> Result<(), PilError> {
        let (width, height) = self.size()?;
        let pixel_count = (width as usize)
            .checked_mul(height as usize)
            .ok_or_else(|| PilError::DimensionError("pixel count overflow".into()))?;
        if entry_count > pixel_count {
            return Err(PilError::TypeError("too many data entries".into()));
        }
        Ok(())
    }

    /// Replaces raw I;16 samples from Pillow's bytes fast path.
    ///
    /// Pillow treats a `bytes` argument to `putdata` as packed storage for
    /// I;16 modes, rather than as one numeric sample per byte. The payload is
    /// copied into the image's raw two-byte sample buffer; an incomplete final
    /// sample remains zero-filled, and bytes beyond the allocated raw storage
    /// are ignored.
    pub fn putdata_l16_bytes(&mut self, data: &[u8]) -> Result<(), PilError> {
        self.validate_putdata_length(data.len())?;
        let (width, height) = self.size()?;
        let dimensions = CheckedDims::new(width, height, 2)?;
        let mode = self.mode()?;
        let mut raw = dimensions.alloc_buffer();
        let copy_len = data.len().min(raw.len());
        raw[..copy_len].copy_from_slice(&data[..copy_len]);
        self.load()?;
        if let Image::Loaded(image_data) = self {
            let image = Arc::make_mut(&mut image_data.image);
            let destination = image
                .as_mut_luma16()
                .ok_or_else(|| PilError::InternalError("putdata I;16 storage mismatch".into()))?;
            for (pixel, sample) in destination.as_mut().iter_mut().zip(raw.chunks_exact(2)) {
                *pixel = if l16_uses_big_endian(&mode) {
                    u16::from_be_bytes([sample[0], sample[1]])
                } else {
                    u16::from_le_bytes([sample[0], sample[1]])
                };
            }
            return Ok(());
        }
        Err(PilError::InternalError(
            "putdata I;16 did not materialize writable storage".into(),
        ))
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
        if is_l16_mode(&mode_name) {
            let sample = putdata_l16_sample(value, scale, offset)?;
            self.load()?;
            if let Image::Loaded(data) = self {
                let image = Arc::make_mut(&mut data.image);
                let destination = image.as_mut_luma16().ok_or_else(|| {
                    PilError::InternalError("putdata I;16 storage mismatch".into())
                })?;
                let pixel = destination.as_mut().get_mut(pixel_index).ok_or_else(|| {
                    PilError::InternalError("putdata I;16 offset out of bounds".into())
                })?;
                *pixel = sample;
                return Ok(());
            }
            return Err(PilError::InternalError(
                "putdata I;16 did not materialize writable storage".into(),
            ));
        }
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
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when `channel` is out of range, or
    /// another [`PilError`] when materialization fails.
    pub fn getchannel(&self, channel: i32) -> Result<Image, PilError> {
        // Validate channel index (requires materialized image for band count)
        let img = self.materialized_shared()?;
        let bands = img.color().channel_count();
        if channel < 0 || channel >= bands as i32 {
            return Err(PilError::ValueError("band index out of range".into()));
        }
        let ch = channel as usize;
        // Pillow treats a single-band P image's only band as the palette-index
        // image itself, preserving its mode and retained palette. Extracting
        // it through the generic raster path would expand the indices to L and
        // silently lose that observable palette contract. `has_palette_mode`
        // intentionally excludes PA, and a P image exposes exactly one
        // channel, so a valid selector is always 0.
        if self.has_palette_mode() {
            return Ok(self.copy());
        }
        // Defer extraction via pipeline
        Ok(Image::push_op(
            self,
            PipelineOp::ExtractBand { index: ch as u8 },
        ))
    }

    /// Resolves a numeric or named public channel selector before extraction.
    pub fn getchannel_selector(&self, selector: ChannelSelector) -> Result<Image, PilError> {
        let channel = match selector {
            ChannelSelector::Index(channel) => channel,
            ChannelSelector::Name(name) => {
                let bands = self.getbands()?;
                bands.iter().position(|band| band == &name).map_or_else(
                    || {
                        Err(PilError::ValueError(format!(
                            "The image has no channel \"{name}\""
                        )))
                    },
                    |index| Ok(index as i32),
                )?
            }
            ChannelSelector::Invalid(type_name) => {
                return Err(PilError::TypeError(format!(
                    "'{type_name}' object cannot be interpreted as an integer"
                )));
            }
        };
        self.getchannel(channel)
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
        if let Some(target) = putalpha_conversion_target(&mode_name) {
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
        let new_self = Image::push_op(self, PipelineOp::PutAlpha { alpha, mode });
        *self = new_self;
        Ok(())
    }

    /// Applies the Python-facing scalar-or-mask `putalpha` contract.
    pub fn putalpha_with_input(&mut self, input: PutAlphaInput) -> Result<(), PilError> {
        match input {
            PutAlphaInput::Integer(value) => self.putalpha(value.clamp(0, 255) as u8),
            PutAlphaInput::Image(mask) => self.putalpha_data(&mask),
            PutAlphaInput::Invalid(type_name) => Err(PilError::TypeError(format!(
                "'{type_name}' object cannot be interpreted as an integer"
            ))),
        }
    }

    /// Replaces the alpha channel from an `L` mask image, matching Pillow's
    /// image-backed ``Image.putalpha``.
    pub fn putalpha_data(&mut self, mask: &Image) -> Result<(), PilError> {
        let mask_img = mask.materialize()?;
        let mask_mode = mask.mode()?;
        if mask_mode != "1" && mask_mode != "L" {
            return Err(PilError::ValueError("illegal image mode".into()));
        }
        if (mask_img.width(), mask_img.height()) != (self.size()?.0, self.size()?.1) {
            return Err(PilError::ValueError("images do not match".into()));
        }
        let mask_luma = mask_img.to_luma8();
        let mode_name = self.mode()?;
        if let Some(target) = putalpha_conversion_target(&mode_name) {
            // Keep image-backed alpha on the same failed conversion path as
            // scalar alpha. Pillow rejects these source/target pairs before
            // queuing the alpha-data operation.
            return Err(PilError::ValueError(format!(
                "conversion from {mode_name} to {target} not supported"
            )));
        }
        let mode = crate::pipeline::PixelMode::from_name(&mode_name).ok_or_else(|| {
            PilError::ValueError(format!("unsupported putalpha mode: {mode_name}"))
        })?;
        let new_self = Image::push_op(
            self,
            PipelineOp::PutAlphaData {
                mask: Arc::new(crate::raster::DynamicImage::ImageLuma8(mask_luma)),
                mode,
            },
        );
        *self = new_self;
        Ok(())
    }

    /// Returns unique colors and their counts.
    ///
    /// The result is `None` when the image contains more than `maxcolors`
    /// distinct colors. Each color is returned in the scalar or tuple shape
    /// matching the image mode. `I` values retain signed 32-bit precision and
    /// `F` values retain their 32-bit floating-point value.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when mode detection or materialization fails.
    #[allow(clippy::type_complexity)]
    pub fn getcolors(
        &self,
        maxcolors: u32,
    ) -> Result<Option<Vec<(u32, FormattedPixelValue)>>, PilError> {
        let mode = self.mode()?;
        if is_l16_mode(&mode) {
            // Pillow's ImagingCore rejects unsigned 16-bit luma modes here;
            // this is the public mode contract, independent of TIFF decoding.
            return Err(PilError::ValueError("image has wrong mode".into()));
        }
        let img = self.materialized_shared()?;
        if mode == "I" || mode == "F" {
            return Self::getcolors_scalar(maxcolors, &mode, &img);
        }
        // For 1, L, P modes, PIL uses histogram (pixel value ascending)
        if mode == "1" || mode == "L" || mode == "P" {
            return Self::getcolors_histogram(maxcolors, &img);
        }
        // For multi-channel modes, use pixel-level counting
        let n_bands = match img.color() {
            crate::raster::ColorType::La8 | crate::raster::ColorType::La16 => 2,
            crate::raster::ColorType::Rgb8 | crate::raster::ColorType::Rgb16 => 3,
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
        Ok(Some(
            result
                .into_iter()
                .map(|(count, color)| (count, FormattedPixelValue::Components(color)))
                .collect(),
        ))
    }

    /// Returns `getcolors` with Pillow's scalar/tuple color shape applied.
    ///
    /// Color counting and mode-specific truncation remain in the core; the
    /// binding is responsible only for constructing the host list and tuple
    /// objects.
    pub fn getcolors_formatted(
        &self,
        maxcolors: u32,
    ) -> Result<Option<Vec<(u32, FormattedPixelValue)>>, PilError> {
        self.getcolors(maxcolors)
    }

    /// Returns extrema using Pillow's one-pair versus per-band result shape.
    pub fn getextrema_formatted(&self) -> Result<FormattedExtrema, PilError> {
        let (width, height) = self.size()?;
        if width == 0 || height == 0 {
            let bands = pillow_band_count(&self.mode()?);
            return Ok(if bands == 1 {
                FormattedExtrema::Empty
            } else {
                FormattedExtrema::EmptyMultiple(bands)
            });
        }

        let mode = self.mode()?;
        if mode == "I" {
            let values = self.scalar_integer_samples()?;
            let (minimum, maximum) = values
                .into_iter()
                .fold(None, |extrema: Option<(i32, i32)>, value| {
                    Some(match extrema {
                        Some((minimum, maximum)) => (minimum.min(value), maximum.max(value)),
                        None => (value, value),
                    })
                })
                .ok_or_else(|| PilError::InternalError("I image has no scalar samples".into()))?;
            return Ok(FormattedExtrema::Integer((minimum, maximum)));
        }
        if mode == "F" {
            let values = self.scalar_float_samples()?;
            let (minimum, maximum) = values
                .into_iter()
                .fold(None, |extrema: Option<(f64, f64)>, value| {
                    Some(match extrema {
                        Some((minimum, maximum)) => (minimum.min(value), maximum.max(value)),
                        None => (value, value),
                    })
                })
                .ok_or_else(|| PilError::InternalError("F image has no scalar samples".into()))?;
            return Ok(FormattedExtrema::Float((minimum, maximum)));
        }

        let extrema = self.getextrema()?;
        if extrema.len() == 1 {
            Ok(FormattedExtrema::Single(extrema[0]))
        } else {
            Ok(FormattedExtrema::Multiple(extrema))
        }
    }

    /// Histogram-based getcolors for 1, L, P modes.
    /// Matches PIL's Python-level implementation:
    ///   h = self.im.histogram()
    ///   out = [(h[i], i) for i in range(256) if h[i]]
    fn getcolors_histogram(
        maxcolors: u32,
        img: &DynamicImage,
    ) -> Result<Option<Vec<(u32, FormattedPixelValue)>>, PilError> {
        // Compute a 256-bin histogram.  All Rust-backed single-band modes,
        // including P and 1, expose their samples through the luma view after
        // materialization, so a second color-type arm only added an unreachable
        // coverage region without changing the result.
        let mut hist = [0u32; 256];
        let luma = img.to_luma8();
        for p in luma.pixels() {
            hist[p[0] as usize] += 1;
        }
        // Build result: [(count, pixel_value)] in pixel value ascending order
        let result: Vec<(u32, FormattedPixelValue)> = (0..=255u8)
            .filter(|&i| hist[i as usize] > 0)
            .map(|i| (hist[i as usize], FormattedPixelValue::Scalar(i)))
            .collect();
        if result.len() > maxcolors as usize {
            return Ok(None);
        }
        Ok(Some(result))
    }

    /// Counts Pillow's scalar `I` and `F` modes without converting their
    /// four-byte samples through the byte-oriented RGBA view.
    fn getcolors_scalar(
        maxcolors: u32,
        mode: &str,
        img: &DynamicImage,
    ) -> Result<Option<Vec<(u32, FormattedPixelValue)>>, PilError> {
        match Self::scalar_samples_from_materialized(img, mode) {
            ScalarImageSamples::Integer(values) => {
                let mut counts = std::collections::HashMap::<i32, u32>::new();
                for value in values {
                    *counts.entry(value).or_insert(0) += 1;
                }
                if counts.len() > maxcolors as usize {
                    return Ok(None);
                }
                let mut result: Vec<_> = counts.into_iter().collect();
                // Pillow's scalar ImagingCore colors are returned in descending
                // sample order for the deterministic values used by this API.
                result.sort_by(|a, b| b.0.cmp(&a.0));
                Ok(Some(
                    result
                        .into_iter()
                        .map(|(value, count)| (count, FormattedPixelValue::Integer(value)))
                        .collect(),
                ))
            }
            ScalarImageSamples::Float(values) => {
                let mut counts = std::collections::HashMap::<u32, u32>::new();
                for value in values {
                    // C float equality treats positive and negative zero as equal.
                    let key = if value == 0.0 {
                        0
                    } else {
                        (value as f32).to_bits()
                    };
                    *counts.entry(key).or_insert(0) += 1;
                }
                if counts.len() > maxcolors as usize {
                    return Ok(None);
                }
                let mut result: Vec<_> = counts
                    .into_iter()
                    .map(|(bits, count)| (f32::from_bits(bits) as f64, count))
                    .collect();
                result.sort_by(|a, b| b.0.total_cmp(&a.0));
                Ok(Some(
                    result
                        .into_iter()
                        .map(|(value, count)| (count, FormattedPixelValue::Float(value)))
                        .collect(),
                ))
            }
        }
    }

    /// Reads the retained four-byte scalar storage for `I` and `F` modes.
    pub(crate) fn scalar_samples(&self, mode: &str) -> Result<ScalarImageSamples, PilError> {
        if mode == "I" {
            return self
                .read_scalar_storage(mode, decode_integer_samples)
                .map(ScalarImageSamples::Integer);
        }
        self.read_scalar_storage(mode, decode_float_samples)
            .map(ScalarImageSamples::Float)
    }

    /// Decodes scalar samples from a buffer that `Image::materialize` has
    /// already validated for the requested scalar mode.
    pub(crate) fn scalar_samples_from_materialized(
        image: &DynamicImage,
        mode: &str,
    ) -> ScalarImageSamples {
        if mode == "I" {
            ScalarImageSamples::Integer(decode_integer_samples(image.as_bytes()))
        } else {
            ScalarImageSamples::Float(decode_float_samples(image.as_bytes()))
        }
    }

    fn scalar_integer_samples(&self) -> Result<Vec<i32>, PilError> {
        self.read_scalar_storage("I", decode_integer_samples)
    }

    fn scalar_float_samples(&self) -> Result<Vec<f64>, PilError> {
        self.read_scalar_storage("F", decode_float_samples)
    }

    fn read_scalar_storage<T>(
        &self,
        mode: &str,
        decode: impl FnOnce(&[u8]) -> T,
    ) -> Result<T, PilError> {
        let img = self.materialized_shared()?;
        let expected = CheckedDims::new(img.width(), img.height(), 4)?.total_bytes();
        let raw = img.as_bytes();
        if raw.len() != expected {
            return Err(PilError::InternalError(format!(
                "{mode} image has invalid scalar storage"
            )));
        }
        Ok(decode(raw))
    }

    /// Returns Shannon entropy using Pillow-compatible per-band histograms.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization fails.
    pub fn entropy(&self) -> Result<f64, PilError> {
        self.entropy_with_mask(None)
    }

    /// Computes entropy over pixels where an optional mask is non-zero,
    /// matching Pillow's masked-entropy semantics.
    pub fn entropy_with_mask(&self, mask: Option<&Image>) -> Result<f64, PilError> {
        let img = self.materialized_shared()?;
        let mask_luma = if let Some(mask) = mask {
            let mask_img = mask.materialize()?;
            if (mask_img.width(), mask_img.height()) != (img.width(), img.height()) {
                return Err(PilError::ValueError("images do not match".into()));
            }
            let mode = mask.mode()?;
            if mode != "1" && mode != "L" {
                return Err(PilError::ValueError("bad transparency mask".into()));
            }
            Some(mask_img.to_luma8())
        } else {
            None
        };
        let mask_px = mask_luma.as_ref();
        let masked = |x: u32, y: u32| -> bool {
            match mask_px {
                Some(m) => {
                    let px = m.get_pixel(x, y);
                    px[0] != 0
                }
                None => true,
            }
        };
        let n_bands = match img.color() {
            crate::raster::ColorType::L8 | crate::raster::ColorType::L16 => 1,
            crate::raster::ColorType::La8 | crate::raster::ColorType::La16 => 2,
            crate::raster::ColorType::Rgb8 | crate::raster::ColorType::Rgb16 => 3,
            _ => 4,
        };
        let mut hists = vec![[0u32; 256]; n_bands];
        // Use mode-aware pixel reading (to_rgba8 remaps LA channels incorrectly for histogram)
        match img.color() {
            crate::raster::ColorType::La8 | crate::raster::ColorType::La16 => {
                let la = img.to_luma_alpha8();
                for (y, row) in la.rows().enumerate() {
                    for (x, px) in row.enumerate() {
                        if !masked(x as u32, y as u32) {
                            continue;
                        }
                        hists[0][px[0] as usize] += 1;
                        hists[1][px[1] as usize] += 1;
                    }
                }
            }
            crate::raster::ColorType::L8 | crate::raster::ColorType::L16 => {
                let luma = img.to_luma8();
                for (y, row) in luma.rows().enumerate() {
                    for (x, px) in row.enumerate() {
                        if !masked(x as u32, y as u32) {
                            continue;
                        }
                        hists[0][px[0] as usize] += 1;
                    }
                }
            }
            _ => {
                let rgba = img.to_rgba8();
                for (y, row) in rgba.rows().enumerate() {
                    for (x, px) in row.enumerate() {
                        if !masked(x as u32, y as u32) {
                            continue;
                        }
                        for b in 0..n_bands {
                            hists[b][px[b] as usize] += 1;
                        }
                    }
                }
            }
        }
        let counted: u32 = hists.iter().map(|band| band.iter().sum::<u32>()).sum();
        let total = counted as f64;
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
    /// per row. Entries are `1` when any source sample on that axis is
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
        let mode = self.mode_from_materialized(&img);
        let mut mark = |index: usize| {
            let x = index % w;
            let y = index / w;
            h_proj[x] = 1;
            v_proj[y] = 1;
        };

        // Pillow's ImagingGetProjection checks source bands for non-zero
        // samples. Converting RGB/RGBA to luma first loses low red/blue and
        // alpha-only pixels, even though Pillow projects those pixels.
        if matches!(mode.as_str(), "I" | "F") {
            match Self::scalar_samples_from_materialized(&img, &mode) {
                ScalarImageSamples::Integer(values) => {
                    for (index, value) in values.into_iter().enumerate() {
                        if value != 0 {
                            mark(index);
                        }
                    }
                }
                ScalarImageSamples::Float(values) => {
                    for (index, value) in values.into_iter().enumerate() {
                        if value != 0.0 {
                            mark(index);
                        }
                    }
                }
            }
        } else {
            match mode.as_str() {
                "L" | "1" | "P" => {
                    for (index, pixel) in img.to_luma8().pixels().enumerate() {
                        if pixel[0] != 0 {
                            mark(index);
                        }
                    }
                }
                "LA" | "PA" => {
                    for (index, pixel) in img.to_luma_alpha8().pixels().enumerate() {
                        if pixel[0] != 0 || pixel[1] != 0 {
                            mark(index);
                        }
                    }
                }
                "RGB" | "HSV" | "YCbCr" => {
                    for (index, pixel) in img.to_rgb8().pixels().enumerate() {
                        if pixel.0.iter().any(|value| *value != 0) {
                            mark(index);
                        }
                    }
                }
                _ => {
                    for (index, pixel) in img.to_rgba8().pixels().enumerate() {
                        if pixel.0.iter().any(|value| *value != 0) {
                            mark(index);
                        }
                    }
                }
            }
        }
        Ok((h_proj, v_proj))
    }

    /// Converts the image to X11 bitmap (`XBM`) source bytes.
    ///
    /// Mode `"1"` treats any non-zero pixel as white. Other modes are rejected,
    /// matching Pillow's `not a bitmap` contract.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when mode detection or materialization fails.
    pub fn tobitmap(&self) -> Result<Vec<u8>, PilError> {
        let mode = self.mode()?;
        if mode != "1" {
            return Err(PilError::ValueError("not a bitmap".to_owned()));
        }
        let img = self.materialized_shared()?;
        let gray = img.to_luma8();
        let (w, h) = (gray.width(), gray.height());
        let row_bytes = w.div_ceil(8) as usize;
        let mut bits = vec![0u8; row_bytes * h as usize];
        for y in 0..h {
            for x in 0..w {
                let v = gray.get_pixel(x, y)[0];
                // The mode check above makes this an `Image` mode "1" path:
                // Pillow stores any non-zero source value as a set bit.
                let is_white = v != 0;
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
    /// Multi-frame decoding is not implemented in core yet, so only frame `0`
    /// is accepted and other frame numbers raise [`PilError::EOFError`].
    pub fn seek(&self, frame: u32) -> Result<(), PilError> {
        if frame != self.tell() {
            return Err(PilError::EOFError("no more images in file".to_owned()));
        }
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
    exif: Option<Vec<u8>>,
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
            exif: exif.clone(),
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
        exif,
    }))
}

fn map_codec_error(error: image_slash_star::ImageError, source: &str) -> PilError {
    if matches!(error, image_slash_star::ImageError::UnknownFormat) {
        PilError::UnidentifiedImageError(source.to_owned())
    } else {
        PilError::ImageError(error)
    }
}

fn map_deferred_decode_error(error: image_slash_star::ImageError) -> PilError {
    match error {
        // Pillow's lazy Image.load path collapses malformed encoded payloads
        // to this stable OSError message, even though open() accepted the
        // format header. Keep codec-specific diagnostics out of the public
        // error contract for deferred loads.
        image_slash_star::ImageError::Malformed { .. } => {
            PilError::IOError("cannot load this image".to_owned())
        }
        other => map_codec_error(other, "memory"),
    }
}

fn decoded_to_dynamic(decoded: &Decoded<DecodedImage>) -> Result<DynamicImage, PilError> {
    let content = &decoded.content;
    match content.mode {
        ImageMode::P8 => crate::raster::GrayImage::from_raw(
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
            crate::raster::GrayImage::from_raw(content.width, content.height, unpacked)
                .map(DynamicImage::ImageLuma8)
                .ok_or_else(|| PilError::DimensionError("invalid packed bilevel buffer".to_owned()))
        }
        ImageMode::Cmyk8 | ImageMode::F32 | ImageMode::I32 => crate::raster::RgbaImage::from_raw(
            content.width,
            content.height,
            content.pixels.clone(),
        )
        .map(DynamicImage::ImageRgba8)
        .ok_or_else(|| PilError::DimensionError("invalid four-byte buffer".to_owned())),
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
        "LA" => {
            // Pillow ImagePalette compiles an LA palette to RGB triples with
            // the L channel replicated and the A channel carried as palette
            // alpha (ImagePalette.py `_raw`/mode transforms); getpalette
            // therefore returns (L,L,L,A) per entry.
            let complete = data.len() / 2 * 2;
            if complete > 512 {
                return Err(PilError::ValueError("invalid palette size".to_owned()));
            }
            let mut rgb = Vec::with_capacity(complete / 2 * 3);
            let mut alpha = Vec::with_capacity(complete / 2);
            for color in data[..complete].chunks_exact(2) {
                rgb.extend_from_slice(&[color[0], color[0], color[0]]);
                alpha.push(color[1]);
            }
            Ok((rgb, alpha))
        }
        _ => Err(PilError::ValueError("unrecognized raw mode".to_owned())),
    }
}

pub(crate) fn expand_palette(
    indices: &crate::raster::GrayImage,
    palette: &[u8],
    palette_alpha: &[u8],
) -> DynamicImage {
    if palette_alpha.is_empty() {
        return DynamicImage::ImageRgb8(crate::raster::RgbImage::from_fn(
            indices.width(),
            indices.height(),
            |x, y| {
                let index = usize::from(indices.get_pixel(x, y)[0]);
                let base = index * 3;
                crate::raster::Rgb([
                    palette.get(base).copied().unwrap_or(0),
                    palette.get(base + 1).copied().unwrap_or(0),
                    palette.get(base + 2).copied().unwrap_or(0),
                ])
            },
        ));
    }
    DynamicImage::ImageRgba8(crate::raster::RgbaImage::from_fn(
        indices.width(),
        indices.height(),
        |x, y| {
            let index = usize::from(indices.get_pixel(x, y)[0]);
            let base = index * 3;
            crate::raster::Rgba([
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
pub(crate) fn expand_palette_alpha(
    indices_alpha: &crate::raster::GrayAlphaImage,
    palette: &[u8],
) -> DynamicImage {
    DynamicImage::ImageRgba8(crate::raster::RgbaImage::from_fn(
        indices_alpha.width(),
        indices_alpha.height(),
        |x, y| {
            let pixel = indices_alpha.get_pixel(x, y);
            let index = usize::from(pixel[0]);
            let base = index * 3;
            crate::raster::Rgba([
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
        crate::raster::ColorType::L8 => {
            // Extract R channel directly — GPU mode-aware shaders only update R for L mode.
            // G and B may be stale; to_luma8() weights all three channels and would be wrong.
            let rgba = result.to_rgba8();
            let (w, h) = rgba.dimensions();
            let luma: Vec<u8> = rgba.pixels().map(|px| px[0]).collect();
            DynamicImage::ImageLuma8(
                crate::raster::GrayImage::from_raw(w, h, luma).unwrap_or_else(|| result.to_luma8()),
            )
        }
        crate::raster::ColorType::La8 => {
            // Extract R (luma) and A (alpha) directly.
            let rgba = result.to_rgba8();
            let (w, h) = rgba.dimensions();
            let la: Vec<u8> = rgba.pixels().flat_map(|px| [px[0], px[3]]).collect();
            DynamicImage::ImageLumaA8(
                crate::raster::GrayAlphaImage::from_raw(w, h, la)
                    .unwrap_or_else(|| result.to_luma_alpha8()),
            )
        }
        crate::raster::ColorType::Rgb8 => DynamicImage::ImageRgb8(result.to_rgb8()),
        crate::raster::ColorType::Rgba8 => DynamicImage::ImageRgba8(result.to_rgba8()),
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
            crate::raster::GrayImage::from_raw(w, h, data)
                .ok_or_else(|| PilError::ValueError("raw_bytes_to_image: buffer error".into()))?,
        )),
        2 => Ok(DynamicImage::ImageLumaA8(
            crate::raster::GrayAlphaImage::from_raw(w, h, data)
                .ok_or_else(|| PilError::ValueError("raw_bytes_to_image: buffer error".into()))?,
        )),
        3 => Ok(DynamicImage::ImageRgb8(
            crate::raster::RgbImage::from_raw(w, h, data)
                .ok_or_else(|| PilError::ValueError("raw_bytes_to_image: buffer error".into()))?,
        )),
        4 => Ok(DynamicImage::ImageRgba8(
            crate::raster::RgbaImage::from_raw(w, h, data)
                .ok_or_else(|| PilError::ValueError("raw_bytes_to_image: buffer error".into()))?,
        )),
        _ => Err(PilError::ValueError(format!(
            "raw_bytes_to_image: unsupported channel count {}",
            channels
        ))),
    }
}

/// Computes Pillow `ImageStat.Stat` values from a precomputed histogram.
///
/// A histogram is partitioned into 256-bin bands. Each returned band contains
/// `[count, sum, sum2, mean, median, rms, var, stddev, min, max]`, matching the
/// representation used by [`Image::stat`].
pub fn stat_from_histogram(data: &[f64]) -> StatResult {
    let bands: Vec<Vec<f64>> = data
        .chunks(256)
        .map(|histogram| {
            let count: f64 = histogram.iter().sum();
            let sum: f64 = histogram
                .iter()
                .enumerate()
                .map(|(index, &frequency)| index as f64 * frequency)
                .sum();
            let sum2: f64 = histogram
                .iter()
                .enumerate()
                .map(|(index, &frequency)| {
                    let value = index as f64;
                    value * value * frequency
                })
                .sum();
            let mean = if count > 0.0 { sum / count } else { 0.0 };
            let rms = if count > 0.0 {
                (sum2 / count).sqrt()
            } else {
                0.0
            };
            let variance = if count > 0.0 {
                ((sum2 - (sum * sum) / count) / count).max(0.0)
            } else {
                0.0
            };
            let stddev = variance.sqrt();

            let mut cumulative = 0.0;
            let half = (count / 2.0).floor();
            let mut median: f64 = 255.0;
            let mut min_bin: f64 = 255.0;
            let mut max_bin: f64 = 0.0;
            for (index, &frequency) in histogram.iter().enumerate() {
                cumulative += frequency;
                if cumulative > half && median == 255.0 {
                    median = index as f64;
                }
                if frequency > 0.0 {
                    min_bin = min_bin.min(index as f64);
                    max_bin = max_bin.max(index as f64);
                }
            }

            vec![
                count, sum, sum2, mean, median, rms, variance, stddev, min_bin, max_bin,
            ]
        })
        .collect();
    StatResult::from_bands(&bands)
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
#[allow(dead_code)]
pub(crate) fn execute_op(
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
