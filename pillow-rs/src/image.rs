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
//! `L` is one byte per pixel, `RGB` and `LAB` are tightly packed triplets,
//! `RGBA` is tightly packed quadruplets, and `P` returns palette indices.
//! Non-standard modes such as `CMYK`, `HSV`, `YCbCr`, `I`, `F`, and the 16-bit
//! luma raw modes may be carried through Pillow-owned raster buffers with an
//! explicit mode tag.
//!
//! # Lazy Execution
//!
//! Operations that can be represented as [`crate::pipeline::PipelineOp`] values
//! may be deferred. Calling [`Image::materialize`], [`Image::encode`], or
//! [`Image::tobytes`] forces decoding and pipeline execution.

use image_slash_star::{Decoded, DecodedImage, EncodedImage, ImageFormat, ImageInfo, ImageMode};
use std::sync::Arc;

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, GenericImageView};

mod backend;
mod byte_export;
mod channel_access;
mod color_count;
mod construction;
mod data_access;
#[path = "image/encode.rs"]
mod encode;
mod entropy;
mod materialization;
mod metadata;
mod palette;
#[path = "image/pipeline_admission.rs"]
mod pipeline_admission;
mod pipeline_ops;
pub use pipeline_ops::{
    MaterializationCache, PipelineModeCache, PipelineOps, PipelinePrefixCache, PipelineShapeCache,
};
use pipeline_ops::{known_pipeline_op_dimensions, take_cached_materialization};
pub(crate) use pipeline_ops::{
    known_pipeline_op_mode, materialization_cache, pipeline_mode_cache, pipeline_shape_cache,
};
mod pixel_access;
mod projection;
mod statistics;
pub use statistics::{StatResult, StatValue, stat_from_histogram, stat_from_list};

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

fn clear_palette_info_alpha(info: &mut Option<ImageInfo>) {
    let Some(info) = info.as_mut() else {
        return;
    };
    let Some(palette) = info.palette.as_mut() else {
        return;
    };
    palette.alpha.clear();
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
    /// True only while constructor-created RGB/CMYK samples are known zero.
    /// Direct in-place data mutators clear this provenance.
    pub(crate) known_zero_fill: bool,
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

/// GPU result provenance without retaining the input graph or its pixel owners.
#[cfg(feature = "gpu")]
#[derive(Clone)]
pub(crate) struct GpuResultMetadata {
    info: Option<ImageInfo>,
    exif: Option<Vec<u8>>,
    format: Option<ImageFormat>,
}

#[cfg(feature = "gpu")]
impl GpuResultMetadata {
    pub(crate) fn materialize(&self, image: DynamicImage, mode: &str) -> Image {
        let mut result = Image::from_dynamic(image, Some(mode.to_owned()));
        if let Image::Loaded(data) = &mut result {
            data.info = self.info.clone();
            data.exif = self.exif.clone();
            data.source_format = self.format;
        }
        result
    }
}

/// Owns either an immutable materialized image or bytes produced by a packer.
enum ImageByteExport {
    Shared(Arc<DynamicImage>),
    Packed(Vec<u8>),
}

impl AsRef<[u8]> for ImageByteExport {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Shared(image) => image.as_bytes(),
            Self::Packed(bytes) => bytes,
        }
    }
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

/// Shape of one host-language value accepted by `Image.putdata`.
///
/// Bindings use this result to choose whether to extract a numeric scalar or
/// a multiband tuple. The mode-specific distinction belongs to the core so
/// each binding exposes the same public coercion boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PutDataValueKind {
    /// A scalar sample for `1`, `L`, `P`, `I`, `I;16*`, or `F`.
    Numeric,
    /// A packed integer or component tuple for a multiband mode.
    Components {
        /// Number of logical channels in the image mode.
        channels: usize,
    },
}

/// Callback-free bulk values extracted by a host-language binding.
///
/// Python and other bindings remain responsible for protocol iteration and
/// coercion. Once a built-in sequence has been extracted, this type lets the
/// core own the mode policy and canonical pixel normalization.
#[derive(Debug, Clone, Copy)]
pub enum PutDataInput<'a> {
    /// Integer samples, interpreted as numeric or packed values by the mode.
    Integers(&'a [i64]),
    /// Floating-point numeric samples.
    Numbers(&'a [f64]),
}

/// A Pillow pixel value after the core has applied the logical mode's band
/// shape. Bindings only need to map these variants to their native
/// scalar/tuple value types.
#[derive(Debug, Clone, PartialEq)]
pub enum FormattedPixelValue {
    /// An unsigned byte scalar from `1`, `L`, or `P` mode.
    Scalar(u8),
    /// A signed scalar from `I` or unsigned 16-bit `I;16*` mode.
    Integer(i32),
    /// A 32-bit floating-point scalar from `F` mode, widened for bindings.
    Float(f64),
    /// A multiband pixel value in Pillow band order.
    Components(Vec<u8>),
    /// A small multiband pixel value stored inline to avoid a per-pixel heap allocation.
    InlineComponents {
        /// Pixel bytes in Pillow band order; unused trailing bytes are zero.
        bytes: [u8; 4],
        /// Number of active entries in `bytes`.
        bands: u8,
    },
}

/// Pillow's flat versus multiband `getdata` result after core formatting.
#[derive(Debug, Clone, PartialEq)]
pub enum FormattedImageData {
    /// Scalar samples, including an explicitly selected band.
    Scalars(Vec<u8>),
    /// Signed scalar samples from `I` or unsigned 16-bit `I;16*` mode.
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
    /// An integer scalar image's `(minimum, maximum)` pair, including `I;16*`.
    Integer((i32, i32)),
    /// A 32-bit floating-point scalar image's `(minimum, maximum)` pair.
    Float((f64, f64)),
}

fn pillow_band_count(mode: &str) -> usize {
    match mode {
        "L" | "1" | "P" | "I" | "F" | "I;16" | "I;16L" | "I;16B" | "I;16N" => 1,
        "LA" | "La" | "PA" => 2,
        "RGB" | "YCbCr" | "HSV" | "LAB" => 3,
        _ => 4,
    }
}

/// Copy one channel from a tightly packed interleaved byte raster.
#[inline]
fn select_interleaved_band(data: &[u8], bands: usize, band: usize) -> Vec<u8> {
    data.chunks_exact(bands).map(|pixel| pixel[band]).collect()
}

#[inline]
fn select_interleaved_band_into(data: &[u8], bands: usize, band: usize, output: &mut [u8]) {
    let selected = data.iter().skip(band).step_by(bands);
    for (destination, source) in output.iter_mut().zip(selected) {
        *destination = *source;
    }
}

#[inline]
fn select_lab_band(data: &[u8], band: usize) -> Vec<u8> {
    let mut output = vec![0; data.len() / 3];
    select_lab_band_into(data, band, &mut output);
    output
}

#[inline]
fn select_lab_band_into(data: &[u8], band: usize, output: &mut [u8]) {
    for (destination, pixel) in output.iter_mut().zip(data.chunks_exact(3)) {
        *destination = match band {
            0 => pixel[0],
            1 => pixel[1].wrapping_sub(128),
            2 => pixel[2].wrapping_sub(128),
            _ => unreachable!("LAB has exactly three bands"),
        };
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
    /// A list or tuple of integer components, retaining the source range.
    Components(Vec<i64>),
    /// A list or tuple of floating-point components, retaining fractional values.
    FloatComponents(Vec<f64>),
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
    Image(Box<Image>),
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
) -> Result<Option<Vec<String>>, PilError> {
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
    match formats {
        PythonOpenFormatsInput::None => Ok(None),
        PythonOpenFormatsInput::Names(names) => Ok(Some(names)),
        PythonOpenFormatsInput::Invalid(_) => Err(PilError::TypeError(
            "formats must be a list or tuple".to_owned(),
        )),
    }
}

/// Validates byte paths before the binding translates decode failures.
pub fn validate_python_open_source_bytes(data: &[u8]) -> Result<(), PilError> {
    if data.contains(&0) {
        return Err(PilError::ValueError("embedded null byte".to_owned()));
    }
    Ok(())
}

const MODE1_BYTE_PIXELS: [[u8; 8]; 256] = {
    let mut table = [[0; 8]; 256];
    let mut value = 0;
    while value < 256 {
        let mut bit = 0;
        while bit < 8 {
            table[value][bit] = if value & (128 >> bit) == 0 { 0 } else { 255 };
            bit += 1;
        }
        value += 1;
    }
    table
};

/// Expand complete, validated mode-1 rows; input padding never becomes pixels.
pub(crate) fn unpack_mode1_rows(packed: &[u8], width: usize, pixels: &mut [u8]) {
    if width == 0 {
        return;
    }
    let row_bytes = width.div_ceil(8);
    debug_assert!(pixels.len().is_multiple_of(width));
    debug_assert!(packed.len() >= row_bytes * (pixels.len() / width));
    for (source, destination) in packed
        .chunks_exact(row_bytes)
        .zip(pixels.chunks_exact_mut(width))
    {
        let (blocks, tail) = destination.as_chunks_mut::<8>();
        for (block, &byte) in blocks.iter_mut().zip(source) {
            *block = MODE1_BYTE_PIXELS[usize::from(byte)];
        }
        if !tail.is_empty() {
            tail.copy_from_slice(
                &MODE1_BYTE_PIXELS[usize::from(source[blocks.len()])][..tail.len()],
            );
        }
    }
}

/// Pack validated luma rows using Pillow's nonzero-pixel rule and MSB order.
pub(crate) fn pack_mode1_rows(pixels: &[u8], width: usize, height: usize) -> Vec<u8> {
    if width == 0 || height == 0 {
        return Vec::new();
    }
    debug_assert!(pixels.len() >= width * height);
    let row_bytes = width.div_ceil(8);
    let mut packed = vec![0; row_bytes * height];
    for (source, destination) in pixels
        .chunks_exact(width)
        .zip(packed.chunks_exact_mut(row_bytes))
    {
        let (blocks, tail) = source.as_chunks::<8>();
        for (block, byte) in blocks.iter().zip(destination.iter_mut()) {
            let word = u64::from_le_bytes(*block);
            // Low-seven-bit sums cannot carry between bytes. Combining the
            // sum's high bit with the original high bit tests every byte for
            // nonzero, including values other than the usual 0/255 pair.
            let flags = (((word & 0x7f7f_7f7f_7f7f_7f7f) + 0x7f7f_7f7f_7f7f_7f7f) | word)
                & 0x8080_8080_8080_8080;
            // Gather byte flags into MSB-first pixel order. Discarding the
            // product's upper bits is intentional; input byte order is explicit.
            *byte = ((flags >> 7).wrapping_mul(0x8040_2010_0804_0201) >> 56) as u8;
        }
        if !tail.is_empty() {
            destination[blocks.len()] = tail.iter().enumerate().fold(0, |byte, (bit, &pixel)| {
                byte | (u8::from(pixel != 0) << (7 - bit))
            });
        }
    }
    packed
}

#[cfg(test)]
mod mode1_byte_block_tests {
    use super::{pack_mode1_rows, unpack_mode1_rows};

    #[test]
    fn every_packed_byte_expands_in_msb_order() {
        for value in 0..=255u8 {
            let mut pixels = [0; 8];
            unpack_mode1_rows(&[value], 8, &mut pixels);
            for (bit, pixel) in pixels.into_iter().enumerate() {
                assert_eq!(pixel, if value & (128 >> bit) != 0 { 255 } else { 0 });
            }
        }
    }

    #[test]
    fn every_nonzero_byte_packs_as_a_set_bit() {
        for position in 0..8 {
            for background in [0, 1, 127, 128, 255] {
                for value in 0..=255u8 {
                    let mut pixels = [background; 8];
                    pixels[position] = value;
                    let expected = pixels.iter().enumerate().fold(0, |byte, (bit, &pixel)| {
                        byte | (u8::from(pixel != 0) << (7 - bit))
                    });
                    assert_eq!(pack_mode1_rows(&pixels, 8, 1), [expected]);
                }
            }
        }
    }

    #[test]
    fn rows_keep_padding_separate_at_each_byte_boundary() {
        for width in 0usize..=33 {
            for height in 0..=3 {
                let row_bytes = width.div_ceil(8);
                let mut expected: Vec<u8> = (0..row_bytes * height)
                    .map(|index| (index * 73 + 29) as u8)
                    .collect();
                let mut pixels = vec![0; width * height];
                unpack_mode1_rows(&expected, width, &mut pixels);
                if !width.is_multiple_of(8) {
                    for row in expected.chunks_exact_mut(row_bytes) {
                        row[row_bytes - 1] &= 255 << (8 - width % 8);
                    }
                }
                assert_eq!(pack_mode1_rows(&pixels, width, height), expected);
            }
        }
    }
}

#[cfg(test)]
mod getpixel_rgb_fast_path_tests {
    use super::{Image, PilError};

    #[test]
    fn fast_path_reads_only_loaded_native_rgb_pixels() -> Result<(), PilError> {
        let rgb = Image::new(2, 1, "RGB", (17, 83, 211, 255))?;
        assert_eq!(rgb.getpixel_rgb_if_loaded(1, 0)?, Some([17, 83, 211]));
        assert!(matches!(
            rgb.getpixel_rgb_if_loaded(2, 0),
            Err(PilError::IndexError(_))
        ));

        let gray = Image::new(1, 1, "L", (42, 0, 0, 255))?;
        assert_eq!(gray.getpixel_rgb_if_loaded(0, 0)?, None);

        let deferred = crate::ops::imageops::invert(&rgb)?;
        assert_eq!(deferred.getpixel_rgb_if_loaded(0, 0)?, None);
        Ok(())
    }
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
    // Pillow permits empty scalar images from `Image.frombytes`. CheckedDims
    // intentionally rejects zero dimensions for allocations, so validate the
    // already-existing empty buffer directly before using it for non-empty
    // images.
    let expected = if image.width() == 0 || image.height() == 0 {
        0
    } else {
        CheckedDims::new(image.width(), image.height(), 4)?.total_bytes()
    };
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
        ops: PipelineOps,
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
        /// Shape metadata initialized by the first property query.
        shape: PipelineShapeCache,
        /// Logical output mode initialized by the first mode query.
        mode: PipelineModeCache,
    },
}

#[cfg(feature = "gpu")]
pub(crate) struct GpuPendingGraph<'a> {
    pub(crate) source: &'a Image,
    pub(crate) cached: Option<Arc<DynamicImage>>,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) mode: String,
    pub(crate) encoded_bytes: u64,
    pub(crate) ops: Vec<&'a PipelineOp>,
}

#[cfg(feature = "gpu")]
fn gpu_info_bytes(info: &ImageInfo) -> u64 {
    let profile_bytes = |profile: &image_slash_star::RawIccProfile| {
        (profile.keyword.len() + profile.data.len()) as u64
    };
    4096 + info
        .palette
        .as_ref()
        .map_or(0, |p| (p.rgb.len() + p.alpha.len()) as u64)
        + info.source_color.icc_profile().map_or(0, profile_bytes)
        + info
            .source
            .avif_item_icc_profiles()
            .iter()
            .map(|p| profile_bytes(p.profile()))
            .sum::<u64>()
        + std::mem::size_of_val(info.source.avif_item_locations()) as u64
        + std::mem::size_of_val(info.source.avif_item_properties()) as u64
        + std::mem::size_of_val(info.source.avif_item_plane_properties()) as u64
        + std::mem::size_of_val(info.source.avif_item_codec_properties()) as u64
        + std::mem::size_of_val(info.source.avif_item_relationships()) as u64
        + std::mem::size_of_val(info.source.avif_auxiliary_relationships()) as u64
        + std::mem::size_of_val(info.source.avif_premultiplied_relationships()) as u64
        + std::mem::size_of_val(info.source.avif_item_color_properties()) as u64
        + std::mem::size_of_val(info.source.avif_grid_item_ids()) as u64
}

impl Image {
    /// Inspect the primary graph without decoding or executing an image operation.
    #[cfg(feature = "gpu")]
    pub(crate) fn gpu_pending_graph(&self) -> Result<GpuPendingGraph<'_>, PilError> {
        self.gpu_pending_graph_depth(0)
    }

    #[cfg(feature = "gpu")]
    fn gpu_pending_graph_depth(&self, depth: usize) -> Result<GpuPendingGraph<'_>, PilError> {
        if depth > 64 {
            return Err(PilError::ValueError(
                "GPU graph exceeds 64 source levels".into(),
            ));
        }
        match self {
            Self::Pipeline {
                source,
                ops,
                materialized,
                backend,
                explicit_mode,
                palette,
                mode,
                ..
            } => {
                if backend.is_some_and(|backend| backend != crate::Backend::Gpu) {
                    return Err(PilError::ValueError(
                        "GPU batch graph is locked to another backend".into(),
                    ));
                }
                if palette.is_some() || matches!(explicit_mode.as_deref(), Some("P" | "PA")) {
                    return Err(PilError::ValueError(
                        "GPU graph palette boundary is not yet represented by a native plan".into(),
                    ));
                }
                if let Some(pixels) = materialized.get() {
                    let pixels = pixels.clone()?;
                    let pixel_mode = explicit_mode
                        .clone()
                        .or_else(|| {
                            mode.get()
                                .and_then(|m| m.as_ref().ok().and_then(|m| m.clone()))
                        })
                        .unwrap_or_else(|| image_mode_name(pixels.color().into()).to_owned());
                    return Ok(GpuPendingGraph {
                        source: self,
                        width: pixels.width(),
                        height: pixels.height(),
                        mode: pixel_mode,
                        cached: Some(pixels),
                        encoded_bytes: 0,
                        ops: Vec::new(),
                    });
                }
                let (mut graph, skip) = if let Some(prefix) = ops.prefix_cache()
                    && let Some(pixels) = prefix.materialized.get()
                {
                    let pixels = pixels.clone()?;
                    let pixel_mode = explicit_mode
                        .clone()
                        .unwrap_or_else(|| image_mode_name(pixels.color().into()).to_owned());
                    (
                        GpuPendingGraph {
                            source: self,
                            width: pixels.width(),
                            height: pixels.height(),
                            mode: pixel_mode,
                            cached: Some(pixels),
                            encoded_bytes: 0,
                            ops: Vec::new(),
                        },
                        prefix.ops_len,
                    )
                } else {
                    (source.gpu_pending_graph_depth(depth + 1)?, 0)
                };
                if graph
                    .ops
                    .len()
                    .saturating_add(ops.len().saturating_sub(skip))
                    > 256
                {
                    return Err(PilError::ValueError(
                        "GPU batch graph exceeds 256 pending operations".into(),
                    ));
                }
                graph.ops.extend(ops.as_slice()[skip..].iter());
                Ok(graph)
            }
            Self::Loaded(data) => Ok(GpuPendingGraph {
                source: self,
                cached: Some(data.image.clone()),
                width: data.image.width(),
                height: data.image.height(),
                mode: data
                    .explicit_mode
                    .clone()
                    .unwrap_or_else(|| image_mode_name(data.decoded_mode).to_owned()),
                encoded_bytes: 0,
                ops: Vec::new(),
            }),
            Self::Bytes {
                source,
                info: Some(info),
                materialized,
                ..
            } => Ok(GpuPendingGraph {
                source: self,
                cached: materialized.get().map(|image| image.clone()).transpose()?,
                width: info.width,
                height: info.height,
                mode: image_mode_name(info.mode).to_owned(),
                encoded_bytes: source.bytes().len() as u64,
                ops: Vec::new(),
            }),
            _ => Err(PilError::ValueError(
                "GPU source has no proven native byte header/layout".into(),
            )),
        }
    }

    /// Conservative metadata ownership reservation, without cloning payloads.
    #[cfg(feature = "gpu")]
    pub(crate) fn gpu_metadata_bytes(&self) -> u64 {
        let mut leaf = self;
        while let Self::Pipeline { source, .. } = leaf {
            leaf = source;
        }
        let (info, exif) = match leaf {
            Self::Pipeline { .. } => unreachable!(),
            Self::Loaded(data) => (data.info.as_ref(), data.exif.as_ref()),
            Self::Paletted(data) => (data.info.as_ref(), data.exif.as_ref()),
            Self::Bytes { source, info, .. } => {
                return (source.bytes().len() as u64)
                    .saturating_mul(16)
                    .saturating_add(info.as_ref().map_or(0, gpu_info_bytes));
            }
        };
        4096 + exif.map_or(0, |bytes| bytes.len() as u64) + info.map_or(0, gpu_info_bytes)
    }

    /// Publish pixels produced externally, retaining the normal pipeline metadata.
    #[cfg(feature = "gpu")]
    pub(crate) fn gpu_result_metadata(&self) -> GpuResultMetadata {
        GpuResultMetadata {
            info: self.image_info(),
            exif: self.exif_metadata(),
            format: match self {
                Self::Pipeline { format, .. } | Self::Bytes { format, .. } => *format,
                Self::Loaded(source) => source.source_format,
                Self::Paletted(source) => source.source_format,
            },
        }
    }

    /// Return whether two cloned handles refer to the same immutable source
    /// node for execution-resource reuse.
    ///
    /// Public wrappers intentionally create a fresh `Arc<Image>` for each
    /// operation argument, so comparing those outer handles would miss the
    /// fact that the underlying loaded pixels or lazy materialization cache is
    /// shared. This identity is narrower than pixel equality: independently
    /// created images with equal bytes must not be treated as one secondary.
    pub(crate) fn shares_execution_source(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Loaded(left), Self::Loaded(right)) => Arc::ptr_eq(&left.image, &right.image),
            (Self::Paletted(left), Self::Paletted(right)) => {
                Arc::ptr_eq(&left.materialized, &right.materialized)
            }
            (
                Self::Bytes {
                    materialized: left, ..
                },
                Self::Bytes {
                    materialized: right,
                    ..
                },
            )
            | (
                Self::Pipeline {
                    materialized: left, ..
                },
                Self::Pipeline {
                    materialized: right,
                    ..
                },
            ) => Arc::ptr_eq(left, right),
            _ => false,
        }
    }
}

impl Image {
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
        known_zero_fill: false,
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
    let mut rgb_by_index = [[0u8; 3]; 256];
    for (index, rgb) in rgb_by_index.iter_mut().enumerate() {
        let base = index * 3;
        *rgb = [
            palette.get(base).copied().unwrap_or(0),
            palette.get(base + 1).copied().unwrap_or(0),
            palette.get(base + 2).copied().unwrap_or(0),
        ];
    }

    let samples = indices_alpha.as_raw();
    if let Some(output_len) = samples.len().checked_mul(2) {
        let mut output = Vec::with_capacity(output_len);
        for sample in samples.chunks_exact(2) {
            let rgb = rgb_by_index[usize::from(sample[0])];
            output.extend_from_slice(&[rgb[0], rgb[1], rgb[2], sample[1]]);
        }
        if let Some(image) = crate::raster::RgbaImage::from_raw(
            indices_alpha.width(),
            indices_alpha.height(),
            output,
        ) {
            return DynamicImage::ImageRgba8(image);
        }
    }

    // Preserve the image crate's checked construction fallback if dimensions
    // cannot be represented by a flat RGBA allocation.
    DynamicImage::ImageRgba8(crate::raster::RgbaImage::from_fn(
        indices_alpha.width(),
        indices_alpha.height(),
        |x, y| {
            let pixel = indices_alpha.get_pixel(x, y);
            let rgb = rgb_by_index[usize::from(pixel[0])];
            crate::raster::Rgba([rgb[0], rgb[1], rgb[2], pixel[1]])
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
        _ => "unknown",
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
#[cfg(test)]
#[path = "image_getchannel_receipt_tests.rs"]
mod getchannel_receipt_tests;
