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
//! Rust construction APIs use [`Mode`] identities; host bindings translate
//! Pillow strings at the boundary. Raw byte APIs use the current image mode:
//! `L` is one byte per pixel, `RGB` and `LAB` are tightly packed triplets,
//! `RGBA` is tightly packed quadruplets, and `P` returns palette indices.
//! Non-standard modes such as `CMYK`, `HSV`, `YCbCr`, `I`, `F`, and the 16-bit
//! luma raw modes use their exact Pillow-owned `DynamicImage` variants even
//! when those variants share a raw pixel carrier.
//!
//! # Lazy Execution
//!
//! Operations that can be represented as [`crate::pipeline::PipelineOp`] values
//! may be deferred. Calling [`Image::materialize`], [`Image::encode`], or
//! [`Image::tobytes`] forces decoding and pipeline execution.

use image_slash_star::{EncodedImage, ImageFormat, ImageInfo};
use std::sync::Arc;

use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

mod backend;
pub use backend::preserve_mode;
#[cfg(feature = "gpu")]
pub(crate) use backend::{GpuPendingGraph, GpuResultMetadata};
mod byte_export;
pub use byte_export::{ImagingCoreBytesInput, imaging_core_to_bytes};
mod channel_access;
pub use channel_access::{ChannelSelector, PutAlphaInput};
pub(crate) mod color_count;
pub(crate) mod codec_adapter;
mod construction;
pub use construction::{
    NewColorInput, OpenFormatsInput, OpenModeInput, validate_open_inputs,
    validate_open_source_bytes,
};
mod data_access;
pub(crate) use data_access::ScalarImageSamples;
#[path = "image/encode.rs"]
mod encode;
mod entropy;
mod materialization;
mod metadata;
pub(crate) mod mode_boundary;
use metadata::image_mode_name;
pub use metadata::{ExifCompatFields, prepare_exif_compat};
use data_access::validate_scalar_storage;
mod palette;
pub(crate) use palette::{expand_palette, expand_palette_alpha};
use palette::{operational_palette, padded_palette};
#[path = "image/pipeline_admission.rs"]
mod pipeline_admission;
mod pipeline_ops;
pub use pipeline_ops::{
    MaterializationCache, PipelineModeCache, PipelineOps, PipelinePrefixCache,
    PipelineShapeCache,
};
use pipeline_ops::known_pipeline_op_dimensions;
pub(crate) use pipeline_ops::{
    cached_pipeline_mode, declared_pipeline_mode, known_pipeline_op_mode,
    known_pipeline_op_mode_for_source, materialization_cache, pipeline_mode_cache_for,
    pipeline_shape_cache, PipelineModeState, PipelinePaletteOutput,
};
mod pixel_access;
pub use pixel_access::{PaletteColorComponent, PutPixelValue};
mod projection;
mod statistics;
pub use statistics::{StatResult, StatValue, stat_from_histogram, stat_from_list};

/// A decoded P-mode (palette) image.
/// `indices` holds one byte per pixel (the palette index, 0-255).
/// `palette` holds zero to 768 bytes of retained RGB triples. Missing entries
/// are black when the indexed image must be expanded for color operations.
#[derive(Debug, Clone)]
pub struct PalettedData {
    /// Mode-tagged palette index bytes, one byte per pixel.
    pub indices: DynamicImage,
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

/// Reusable execution state for callback-driven, synchronous PutData writes.
///
/// Bindings retain this token while coercing sequence items without holding
/// the mutable image borrow, then pass it into each completed pixel write.
/// The selected route remains fixed for the sequence, even if callbacks
/// change global backend preferences between values. Its drop restores source,
/// callback, and per-item operation receipts in their execution order.
#[doc(hidden)]
pub struct PutDataExecutionContext {
    pub(crate) context: crate::compute::ExecutionContext,
    pub(crate) requested_backend: Option<crate::compute::Backend>,
    pub(crate) selection: Option<crate::compute::registry::support::StrategySelection>,
    pub(crate) operation_receipts: crate::compute::PipelineOperationTelemetryCapture,
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
    /// A CoreOnly multiband pixel containing native unsigned 16-bit samples.
    U16Components(Vec<u16>),
    /// A CoreOnly multiband pixel containing native 32-bit float samples.
    FloatComponents(Vec<f32>),
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
    /// Native unsigned 16-bit extrema for each CoreOnly band.
    MultipleInteger(Vec<(i32, i32)>),
    /// Native 32-bit float extrema for each CoreOnly band, widened for bindings.
    MultipleFloat(Vec<(f64, f64)>),
    /// An integer scalar image's `(minimum, maximum)` pair, including `I;16*`.
    Integer((i32, i32)),
    /// A 32-bit floating-point scalar image's `(minimum, maximum)` pair.
    Float((f64, f64)),
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
        /// Pillow-owned raster identity translated from the codec's inspected header.
        header_mode: Mode,
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
    /// mode: cached or operation-declared output identity.
    Pipeline {
        /// Input image feeding the operation pipeline.
        source: Arc<Image>,
        /// Operations to execute in order.
        ops: PipelineOps,
        /// Output format inherited from the source when known.
        format: Option<ImageFormat>,
        /// Locked backend for this pipeline. None = use global active set.
        backend: Option<crate::compute::Backend>,
        /// Quantize palette (RGB triples) — populated after Quantize op materializes.
        palette: Option<Vec<u8>>,
        /// Per-entry palette alpha retained for index-preserving operations.
        palette_alpha: Option<Vec<u8>>,
        /// Generated ICC profile attached by an in-pipeline color transform.
        compatibility_icc_profile: Option<Arc<[u8]>>,
        /// Pipeline output initialized by the first implicit or explicit load.
        materialized: MaterializationCache,
        /// Shape metadata initialized by the first property query.
        shape: PipelineShapeCache,
        /// Operation-declared or lazily planned output identity.
        mode: PipelineModeCache,
    },
}

#[cfg(test)]
#[path = "image_getchannel_receipt_tests.rs"]
mod getchannel_receipt_tests;
