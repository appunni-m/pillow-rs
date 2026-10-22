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

use bytes::Bytes;
use image_slash_star::{
    Decoded, DecodedImage, EncodedImage, ImageFormat, ImageInfo, ImageMode, ImagePalette,
};
use std::sync::{Arc, OnceLock};

use crate::checked_dims::CheckedDims;
use crate::color::color_type_to_mode;
use crate::error::PilError;
use crate::pipeline::{ColorMode, PipelineOp, PixelMode, TransposeMethod};
use crate::raster::{DynamicImage, GenericImageView};

mod backend;
mod byte_export;
mod construction;
#[path = "image/encode.rs"]
mod encode;
mod materialization;
mod metadata;
mod palette;
#[path = "image/pipeline_admission.rs"]
mod pipeline_admission;
mod pipeline_ops;
mod pixel_access;
mod statistics;

/// Persistent operation list used by lazy image nodes.
///
/// Appending to an existing `Image::Pipeline` keeps an immutable parent link
/// instead of cloning the complete operation vector. The first execution of a
/// node flattens the chain once and shares that immutable slice with every
/// subsequent route, metadata query, or backend execution. This keeps graph
/// construction linear for long lazy chains while preserving the existing
/// slice-based backend contract at the execution boundary.
#[derive(Debug, Clone)]
pub struct PipelineOps {
    chain: Arc<PipelineOpChain>,
    prefix_cache: Option<PipelinePrefixCache>,
}

#[derive(Debug)]
struct PipelineOpChain {
    parent: Option<Arc<PipelineOpChain>>,
    op: Option<PipelineOp>,
    len: usize,
    mode_preserving: bool,
    flattened: OnceLock<Arc<[PipelineOp]>>,
}

impl PipelineOps {
    pub(crate) fn empty() -> Self {
        Self {
            chain: Arc::new(PipelineOpChain {
                parent: None,
                op: None,
                len: 0,
                mode_preserving: true,
                flattened: OnceLock::new(),
            }),
            prefix_cache: None,
        }
    }

    pub(crate) fn one(op: PipelineOp) -> Self {
        let mode_preserving = op_preserves_mode(&op);
        Self {
            chain: Arc::new(PipelineOpChain {
                parent: None,
                op: Some(op),
                len: 1,
                mode_preserving,
                flattened: OnceLock::new(),
            }),
            prefix_cache: None,
        }
    }

    pub(crate) fn append(&self, op: PipelineOp) -> Self {
        Self {
            chain: Arc::new(PipelineOpChain {
                parent: Some(Arc::clone(&self.chain)),
                mode_preserving: self.chain.mode_preserving && op_preserves_mode(&op),
                op: Some(op),
                len: self.chain.len + 1,
                flattened: OnceLock::new(),
            }),
            prefix_cache: self.prefix_cache.clone(),
        }
    }

    pub(crate) fn from_vec(ops: Vec<PipelineOp>) -> Self {
        ops.into_iter()
            .fold(Self::empty(), |pipeline, op| pipeline.append(op))
    }

    pub(crate) fn as_slice(&self) -> &[PipelineOp] {
        self.chain
            .flattened
            .get_or_init(|| {
                let mut reversed = Vec::with_capacity(self.chain.len);
                let mut cursor = Some(Arc::clone(&self.chain));
                while let Some(node) = cursor {
                    if let Some(op) = node.op.as_ref() {
                        reversed.push(op.clone());
                    }
                    cursor = node.parent.as_ref().map(Arc::clone);
                }
                reversed.reverse();
                Arc::<[PipelineOp]>::from(reversed)
            })
            .as_ref()
    }

    pub(crate) fn to_vec(&self) -> Vec<PipelineOp> {
        self.as_slice().to_vec()
    }

    pub(crate) fn mode_preserving(&self) -> bool {
        self.chain.mode_preserving
    }

    pub(crate) fn len(&self) -> usize {
        self.chain.len
    }

    pub(crate) fn prefix_cache(&self) -> Option<&PipelinePrefixCache> {
        self.prefix_cache.as_ref()
    }

    fn with_prefix_cache(mut self, prefix_cache: Option<PipelinePrefixCache>) -> Self {
        self.prefix_cache = prefix_cache;
        self
    }
}

/// Whether an operation leaves the logical Pillow mode unchanged.
///
/// This metadata is deliberately conservative.  Returning `false` only
/// means that a later mode query may materialize; it never changes execution
/// semantics.  The fast path is important for lazy native-mode chains such as
/// repeated `ImageOps.invert`, whose validation needs the source mode but not
/// the pixels.
fn op_preserves_mode(op: &PipelineOp) -> bool {
    matches!(
        op,
        PipelineOp::Resize { .. }
            | PipelineOp::ResizeBoxed { .. }
            | PipelineOp::Crop { .. }
            | PipelineOp::Rotate { .. }
            | PipelineOp::Transpose { .. }
            | PipelineOp::Thumbnail { .. }
            | PipelineOp::Reduce { .. }
            | PipelineOp::Filter3x3 { .. }
            | PipelineOp::Filter5x5 { .. }
            | PipelineOp::GaussianBlur { .. }
            | PipelineOp::BoxBlur { .. }
            | PipelineOp::BoxBlurXY { .. }
            | PipelineOp::MedianFilter { .. }
            | PipelineOp::MaxFilter { .. }
            | PipelineOp::MinFilter { .. }
            | PipelineOp::RankFilter { .. }
            | PipelineOp::Autocontrast { .. }
            | PipelineOp::Invert
            | PipelineOp::Flip
            | PipelineOp::Mirror
            | PipelineOp::Posterize { .. }
            | PipelineOp::Solarize { .. }
            | PipelineOp::Contain { .. }
            | PipelineOp::Cover { .. }
            | PipelineOp::Fit { .. }
            | PipelineOp::Pad { .. }
            | PipelineOp::Scale { .. }
            | PipelineOp::Expand { .. }
            | PipelineOp::CropBorder { .. }
            | PipelineOp::Add { .. }
            | PipelineOp::Subtract { .. }
            | PipelineOp::Multiply { .. }
            | PipelineOp::Screen { .. }
            | PipelineOp::Darker { .. }
            | PipelineOp::Lighter { .. }
            | PipelineOp::Difference { .. }
            | PipelineOp::Overlay { .. }
            | PipelineOp::HardLight { .. }
            | PipelineOp::SoftLight { .. }
            | PipelineOp::AddModulo { .. }
            | PipelineOp::SubtractModulo { .. }
            | PipelineOp::LogicalAnd { .. }
            | PipelineOp::LogicalOr { .. }
            | PipelineOp::LogicalXor { .. }
            | PipelineOp::Offset { .. }
            | PipelineOp::Duplicate
            | PipelineOp::InvertChops
            | PipelineOp::Brightness { .. }
            | PipelineOp::Contrast { .. }
            | PipelineOp::ColorSaturation { .. }
            | PipelineOp::Sharpness { .. }
            | PipelineOp::EffectSpread { .. }
            | PipelineOp::Paste { .. }
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
            | PipelineOp::PutPixel { .. }
            // PutData records the image's current mode when it is queued, so
            // the generic mode-preserving planner path is authoritative.
            | PipelineOp::PutData { .. }
            | PipelineOp::Eval { .. }
    )
}

fn known_putalpha_mode(mode: PixelMode) -> Option<&'static str> {
    match mode {
        PixelMode::YCbCr | PixelMode::HSV | PixelMode::RGB => Some("RGBA"),
        PixelMode::L | PixelMode::LA => Some("LA"),
        PixelMode::RGBA => Some("RGBA"),
        PixelMode::P | PixelMode::PA => Some("PA"),
        PixelMode::CMYK => Some("RGBA"),
        // Palette putalpha is tagged as PA in push_op before this planner is
        // reached. Public putalpha also rejects 1/I/F, YCbCr, and HSV before
        // queueing; keep this fallback for malformed/internal descriptors.
        _ => None,
    }
}

/// Computes a statically known logical mode for one pipeline operation.
///
/// This is intentionally conservative: `None` retains the existing
/// materialization fallback and therefore cannot change public behavior. The
/// mode cache is separate from the pixel cache because metadata reads must not
/// publish or expose a partially executed image.
pub(crate) fn known_pipeline_op_mode(op: &PipelineOp, current: &str) -> Option<String> {
    if op_preserves_mode(op) {
        return Some(current.to_owned());
    }
    Some(
        match op {
            PipelineOp::Equalize | PipelineOp::EqualizeMasked { .. }
                if matches!(current, "P" | "PA") =>
            {
                "RGB"
            }
            PipelineOp::Equalize | PipelineOp::EqualizeMasked { .. } => current,
            // Public convert and merge constructors attach `explicit_mode`
            // for non-raster-native targets (including CMYK, YCbCr, HSV, I,
            // F, P, and 1). `Image::mode` returns that tag before reaching
            // this planner, so only these four raster-native modes are live
            // here. Keep the fallback conservative for malformed/internal
            // descriptors instead of manufacturing metadata.
            PipelineOp::Convert { mode, .. } | PipelineOp::Merge { mode, .. } => match mode {
                ColorMode::L => "L",
                ColorMode::LA => "LA",
                ColorMode::RGB => "RGB",
                ColorMode::RGBA => "RGBA",
                _ => return None,
            },
            PipelineOp::ConvertLab { .. } => "LAB",
            PipelineOp::Grayscale => "L",
            PipelineOp::Colorize { .. } => "RGB",
            PipelineOp::Constant { .. } => "L",
            PipelineOp::PutAlpha { mode, .. } | PipelineOp::PutAlphaData { mode, .. } => {
                known_putalpha_mode(*mode)?
            }
            PipelineOp::ExtractBand { .. } => "L",
            PipelineOp::EffectNoise { .. } => "L",
            _ => return None,
        }
        .to_owned(),
    )
}

/// Rounds a non-negative dimension using Python's ties-to-even rule.
///
/// Metadata propagation must never manufacture a dimension for an invalid or
/// overflowing operation.  Returning `None` lets the normal materialization
/// path preserve the operation's existing error behavior.
fn round_dimension(value: f64) -> Option<u32> {
    if !value.is_finite() || value < 0.0 || value > f64::from(u32::MAX) {
        return None;
    }
    let lower = value.floor();
    let fraction = value - lower;
    let rounded = if fraction < 0.5 {
        lower
    } else if fraction > 0.5 || (lower as u64) % 2 == 1 {
        lower + 1.0
    } else {
        lower
    };
    (rounded <= f64::from(u32::MAX)).then_some(rounded as u32)
}

/// Computes the output shape for a lazy operation without touching pixels.
///
/// The match is intentionally conservative: operations with secondary images,
/// validation that currently occurs during execution, or mode-specific storage
/// transitions return `None` and retain the existing materialization fallback.
fn known_pipeline_op_dimensions(
    op: &PipelineOp,
    (width, height): (u32, u32),
) -> Option<(u32, u32)> {
    match op {
        PipelineOp::Resize { w, h, .. }
        | PipelineOp::ResizeBoxed { w, h, .. }
        | PipelineOp::Thumbnail { w, h, .. }
        | PipelineOp::Pad { w, h, .. }
        | PipelineOp::Transform { w, h, .. } => Some((*w, *h)),
        PipelineOp::Crop {
            left,
            top,
            right,
            bottom,
        } => Some((right.checked_sub(*left)?, bottom.checked_sub(*top)?)),
        PipelineOp::Expand { border, .. } => {
            let border = border.checked_mul(2)?;
            Some((width.checked_add(border)?, height.checked_add(border)?))
        }
        PipelineOp::CropBorder { border } => {
            let border = border.checked_mul(2)?;
            Some((width.checked_sub(border)?, height.checked_sub(border)?))
        }
        PipelineOp::Transpose { method } => Some(match method {
            TransposeMethod::Rotate90
            | TransposeMethod::Rotate270
            | TransposeMethod::Transpose
            | TransposeMethod::Transverse => (height, width),
            TransposeMethod::FlipLeftRight
            | TransposeMethod::FlipTopBottom
            | TransposeMethod::Rotate180 => (width, height),
        }),
        PipelineOp::Rotate {
            angle,
            expand,
            center,
            translate,
            ..
        } => {
            if !expand {
                return Some((width, height));
            }
            // Pillow takes an exact transpose fast path for right-angle
            // rotations before constructing the affine matrix, regardless
            // of the requested resampling filter. Reflect that public shape
            // here so lazy `size` agrees with the materialized result.
            let normalized = angle.rem_euclid(360.0);
            if center.is_none()
                && translate.is_none()
                && [90.0, 270.0]
                    .into_iter()
                    .any(|right_angle| (normalized - right_angle).abs() <= f64::EPSILON)
            {
                return Some((height, width));
            }
            let sw = f64::from(width);
            let sh = f64::from(height);
            let radians = -angle.to_radians();
            let affine_a = crate::ops::rotate::round_rotate_coefficient(radians.cos());
            let affine_b = crate::ops::rotate::round_rotate_coefficient(radians.sin());
            let affine_d = crate::ops::rotate::round_rotate_coefficient(-radians.sin());
            let affine_e = affine_a;
            let (center_x, center_y) = center.unwrap_or((sw / 2.0, sh / 2.0));
            let (translate_x, translate_y) = translate.unwrap_or((0.0, 0.0));
            let affine_c = affine_a * (-center_x - translate_x)
                + affine_b * (-center_y - translate_y)
                + center_x;
            let affine_f = affine_d * (-center_x - translate_x)
                + affine_e * (-center_y - translate_y)
                + center_y;
            let transform = |x: f64, y: f64| {
                (
                    affine_a * x + affine_b * y + affine_c,
                    affine_d * x + affine_e * y + affine_f,
                )
            };
            let corners = [(0.0, 0.0), (sw, 0.0), (sw, sh), (0.0, sh)];
            let (mut min_x, mut min_y, mut max_x, mut max_y) =
                (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
            for &(x, y) in &corners {
                let (rotated_x, rotated_y) = transform(x, y);
                min_x = min_x.min(rotated_x);
                max_x = max_x.max(rotated_x);
                min_y = min_y.min(rotated_y);
                max_y = max_y.max(rotated_y);
            }
            Some((
                round_dimension(max_x.ceil() - min_x.floor())?,
                round_dimension(max_y.ceil() - min_y.floor())?,
            ))
        }
        PipelineOp::Reduce { x_factor, y_factor } => Some((
            width.div_ceil((*x_factor).max(1)),
            height.div_ceil((*y_factor).max(1)),
        )),
        PipelineOp::Scale { factor, .. } => Some((
            round_dimension(f64::from(width) * *factor)?.max(1),
            round_dimension(f64::from(height) * *factor)?.max(1),
        )),
        PipelineOp::Contain { w, h, .. } | PipelineOp::Cover { w, h, .. } => {
            if width == 0 || height == 0 || *w == 0 || *h == 0 {
                return None;
            }
            let image_ratio = f64::from(width) / f64::from(height);
            let destination_ratio = f64::from(*w) / f64::from(*h);
            if (image_ratio - destination_ratio).abs() < 1e-10 {
                return Some((*w, *h));
            }
            let cover = matches!(op, PipelineOp::Cover { .. });
            if (image_ratio > destination_ratio) != cover {
                let new_height =
                    round_dimension(f64::from(height) / f64::from(width) * f64::from(*w))?;
                Some((*w, if cover { new_height.max(1) } else { new_height }))
            } else {
                let new_width =
                    round_dimension(f64::from(width) / f64::from(height) * f64::from(*h))?;
                Some((if cover { new_width.max(1) } else { new_width }, *h))
            }
        }
        // These operations are unary and have no deferred dimension or
        // secondary-image validation.  Keeping them here is what makes long
        // point/filter/draw chains answer `size()` without replaying pixels.
        PipelineOp::Convert { .. }
        | PipelineOp::ConvertLab { .. }
        | PipelineOp::RemapPalette { .. }
        | PipelineOp::Color3DLut { .. }
        | PipelineOp::ExtractBand { .. }
        | PipelineOp::Filter3x3 { .. }
        | PipelineOp::Filter5x5 { .. }
        | PipelineOp::GaussianBlur { .. }
        | PipelineOp::BoxBlur { .. }
        | PipelineOp::BoxBlurXY { .. }
        | PipelineOp::MedianFilter { .. }
        | PipelineOp::MaxFilter { .. }
        | PipelineOp::MinFilter { .. }
        | PipelineOp::RankFilter { .. }
        | PipelineOp::Autocontrast { .. }
        | PipelineOp::Equalize
        | PipelineOp::EqualizeMasked { .. }
        | PipelineOp::Invert
        | PipelineOp::Flip
        | PipelineOp::Mirror
        | PipelineOp::Posterize { .. }
        | PipelineOp::Solarize { .. }
        | PipelineOp::Grayscale
        | PipelineOp::Colorize { .. }
        | PipelineOp::Constant { .. }
        // Public putpixel validates coordinates before queuing and cannot
        // change the image dimensions. Keeping its shape known prevents a
        // later write from materializing the earlier writes just to check
        // bounds.
        | PipelineOp::PutPixel { .. }
        | PipelineOp::Offset { .. }
        | PipelineOp::Duplicate
        | PipelineOp::InvertChops
        | PipelineOp::Brightness { .. }
        | PipelineOp::Contrast { .. }
        | PipelineOp::ColorSaturation { .. }
        | PipelineOp::Sharpness { .. }
        | PipelineOp::EffectSpread { .. }
        | PipelineOp::Eval { .. }
        | PipelineOp::EffectNoise { .. }
        | PipelineOp::DrawLine { .. }
        | PipelineOp::DrawRectangle { .. }
        | PipelineOp::DrawRoundedRect { .. }
        | PipelineOp::DrawEllipse { .. }
        | PipelineOp::DrawCircle { .. }
        | PipelineOp::DrawPolygon { .. }
        | PipelineOp::DrawArc { .. }
        | PipelineOp::DrawChord { .. }
        | PipelineOp::DrawPieslice { .. }
        | PipelineOp::DrawPoint { .. } => Some((width, height)),
        _ => None,
    }
}

impl std::ops::Deref for PipelineOps {
    type Target = [PipelineOp];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

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

/// Pillow's `ImagingGetColors` hash-table sizes and associated probing
/// polynomials, indexed by the first table size greater than `maxcolors`.
const PILLOW_COLOR_TABLE_SIZES: &[(u32, u32)] = &[
    (4, 3),
    (8, 3),
    (16, 3),
    (32, 5),
    (64, 3),
    (128, 3),
    (256, 29),
    (512, 17),
    (1024, 9),
    (2048, 5),
    (4096, 83),
    (8192, 27),
    (16384, 43),
    (32768, 3),
    (65536, 45),
    (131072, 9),
    (262144, 39),
    (524288, 39),
    (1048576, 9),
    (2097152, 5),
    (4194304, 3),
    (8388608, 33),
    (16777216, 27),
    (33554432, 9),
    (67108864, 71),
    (134217728, 39),
    (268435456, 9),
    (536870912, 5),
    (1073741824, 83),
];

fn pillow_color_table_parameters(maxcolors: u32) -> Option<(u32, u32)> {
    PILLOW_COLOR_TABLE_SIZES
        .iter()
        .find(|(size, _)| *size > maxcolors)
        .map(|(size, polynomial)| (size - 1, *polynomial))
}

const PILLOW_COLOR_DIRECT_TABLE_MAX_BYTES: usize = 8 * 1024 * 1024;

fn pillow_color_direct_table_slots(code_mask: u32, pixel_count: usize) -> Option<usize> {
    let slots = usize::try_from(code_mask).ok()?.checked_add(1)?;
    if code_mask <= 1023 {
        return Some(slots);
    }

    let bytes = slots.checked_mul(std::mem::size_of::<u64>())?;
    (bytes <= PILLOW_COLOR_DIRECT_TABLE_MAX_BYTES && slots <= pixel_count.saturating_mul(2))
        .then_some(slots)
}

fn pillow_image_pixel_count(image: &DynamicImage) -> usize {
    let (width, height) = image.dimensions();
    usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .unwrap_or(0)
}

enum PillowColorCounts {
    Direct(Vec<u64>),
    Sparse(std::collections::HashMap<u32, (u32, u32)>),
}

impl PillowColorCounts {
    fn new(code_mask: u32, sparse_capacity: usize, pixel_count: usize) -> Self {
        // Keep large tables sparse for small images or memory-heavy limits.
        // Dense slots avoid a second randomized hash probe per input pixel
        // when a bounded table is well amortized by the scan.
        if let Some(slot_count) = pillow_color_direct_table_slots(code_mask, pixel_count) {
            if slot_count <= 1024 {
                return Self::Direct(vec![0; slot_count]);
            }

            let mut slots = Vec::new();
            if slots.try_reserve_exact(slot_count).is_ok() {
                slots.resize(slot_count, 0);
                return Self::Direct(slots);
            }
        }
        Self::Sparse(std::collections::HashMap::with_capacity(sparse_capacity))
    }

    fn count(
        &mut self,
        colors: &mut usize,
        pixel: u32,
        maxcolors: usize,
        code_mask: u32,
        code_polynomial: u32,
    ) -> bool {
        match self {
            Self::Direct(slots) => count_pillow_color_direct(
                slots,
                colors,
                pixel,
                maxcolors,
                code_mask,
                code_polynomial,
            ),
            Self::Sparse(table) => count_pillow_color_sparse(
                table,
                colors,
                pixel,
                maxcolors,
                code_mask,
                code_polynomial,
            ),
        }
    }

    fn count_repeated(
        &mut self,
        colors: &mut usize,
        pixel: u32,
        repetitions: usize,
        maxcolors: usize,
        code_mask: u32,
        code_polynomial: u32,
    ) -> Option<bool> {
        if repetitions <= 1 {
            return Some(self.count(colors, pixel, maxcolors, code_mask, code_polynomial));
        }
        let repetitions = u32::try_from(repetitions).ok()?;
        match self {
            Self::Direct(slots) => count_pillow_color_direct_repeated(
                slots,
                colors,
                pixel,
                repetitions,
                maxcolors,
                code_mask,
                code_polynomial,
            ),
            Self::Sparse(table) => count_pillow_color_sparse_repeated(
                table,
                colors,
                pixel,
                repetitions,
                maxcolors,
                code_mask,
                code_polynomial,
            ),
        }
    }

    fn into_slot_order(self, distinct_colors: usize) -> Vec<(u32, (u32, u32))> {
        match self {
            Self::Direct(slots) => {
                let mut result = Vec::with_capacity(distinct_colors);
                result.extend(slots.into_iter().enumerate().filter_map(|(slot, packed)| {
                    (packed != 0).then_some((slot as u32, (packed as u32, (packed >> 32) as u32)))
                }));
                result
            }
            Self::Sparse(table) => {
                let mut result: Vec<_> = table.into_iter().collect();
                result.sort_unstable_by_key(|(slot, _)| *slot);
                result
            }
        }
    }
}

/// Insert one Pillow-packed pixel using the exact probe sequence used by
/// `ImagingGetColors` in a directly indexed small table.
fn count_pillow_color_direct(
    slots: &mut [u64],
    colors: &mut usize,
    pixel: u32,
    maxcolors: usize,
    code_mask: u32,
    code_polynomial: u32,
) -> bool {
    let mut slot = (!pixel) & code_mask;
    let mut increment = 0u32;
    let mut first_conflict = true;

    loop {
        let entry = &mut slots[slot as usize];
        if *entry == 0 {
            if *colors == maxcolors {
                return false;
            }
            *entry = (1u64 << 32) | u64::from(pixel);
            *colors += 1;
            return true;
        }
        if *entry as u32 == pixel {
            let count = (*entry >> 32) as u32 + 1;
            *entry = (u64::from(count) << 32) | u64::from(pixel);
            return true;
        }

        if first_conflict {
            increment = (pixel ^ (pixel >> 3)) & code_mask;
            if increment == 0 {
                increment = code_mask;
            }
            first_conflict = false;
        } else {
            increment <<= 1;
            if increment > code_mask {
                increment ^= code_polynomial;
            }
        }
        slot = (slot + increment) & code_mask;
    }
}

/// Insert one Pillow-packed pixel using the exact probe sequence used by
/// `ImagingGetColors`. Sparse storage avoids allocations proportional to
/// large `maxcolors` values.
fn count_pillow_color_sparse(
    table: &mut std::collections::HashMap<u32, (u32, u32)>,
    colors: &mut usize,
    pixel: u32,
    maxcolors: usize,
    code_mask: u32,
    code_polynomial: u32,
) -> bool {
    let mut slot = (!pixel) & code_mask;
    let mut increment = 0u32;
    let mut first_conflict = true;

    loop {
        match table.entry(slot) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                let (stored_pixel, count) = entry.get_mut();
                if *stored_pixel == pixel {
                    *count += 1;
                    return true;
                }
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                if *colors == maxcolors {
                    return false;
                }
                entry.insert((pixel, 1));
                *colors += 1;
                return true;
            }
        }

        if first_conflict {
            increment = (pixel ^ (pixel >> 3)) & code_mask;
            if increment == 0 {
                increment = code_mask;
            }
            first_conflict = false;
        } else {
            increment <<= 1;
            if increment > code_mask {
                increment ^= code_polynomial;
            }
        }
        slot = (slot + increment) & code_mask;
    }
}

/// Count scalar pixels after the prefix check has ruled out adjacent runs.
/// Match the storage representation once instead of redispatching through the
/// enum for every sample.
fn count_pillow_color_scalar_samples(
    counts: &mut PillowColorCounts,
    colors: &mut usize,
    bytes: &[u8],
    maxcolors: usize,
    code_mask: u32,
    code_polynomial: u32,
) -> bool {
    match counts {
        PillowColorCounts::Direct(slots) => {
            for sample in bytes.chunks_exact(4) {
                let pixel = u32::from_ne_bytes([sample[0], sample[1], sample[2], sample[3]]);
                if !count_pillow_color_direct(
                    slots,
                    colors,
                    pixel,
                    maxcolors,
                    code_mask,
                    code_polynomial,
                ) {
                    return false;
                }
            }
        }
        PillowColorCounts::Sparse(table) => {
            for sample in bytes.chunks_exact(4) {
                let pixel = u32::from_ne_bytes([sample[0], sample[1], sample[2], sample[3]]);
                if !count_pillow_color_sparse(
                    table,
                    colors,
                    pixel,
                    maxcolors,
                    code_mask,
                    code_polynomial,
                ) {
                    return false;
                }
            }
        }
    }
    true
}

/// Count an adjacent run with the same Pillow probe sequence as scalar inserts.
/// `None` asks the caller to replay the run through `count`, preserving the
/// scalar counter-overflow behavior.
fn count_pillow_color_direct_repeated(
    slots: &mut [u64],
    colors: &mut usize,
    pixel: u32,
    repetitions: u32,
    maxcolors: usize,
    code_mask: u32,
    code_polynomial: u32,
) -> Option<bool> {
    let mut slot = (!pixel) & code_mask;
    let mut increment = 0u32;
    let mut first_conflict = true;

    loop {
        let entry = &mut slots[slot as usize];
        if *entry == 0 {
            if *colors == maxcolors {
                return Some(false);
            }
            *entry = (u64::from(repetitions) << 32) | u64::from(pixel);
            *colors += 1;
            return Some(true);
        }
        if *entry as u32 == pixel {
            let count = (*entry >> 32) as u32;
            let count = count.checked_add(repetitions)?;
            *entry = (u64::from(count) << 32) | u64::from(pixel);
            return Some(true);
        }

        if first_conflict {
            increment = (pixel ^ (pixel >> 3)) & code_mask;
            if increment == 0 {
                increment = code_mask;
            }
            first_conflict = false;
        } else {
            increment <<= 1;
            if increment > code_mask {
                increment ^= code_polynomial;
            }
        }
        slot = (slot + increment) & code_mask;
    }
}

/// Count an adjacent run in sparse Pillow slots without changing slot order.
/// `None` asks the caller to replay the run through `count` on counter overflow.
fn count_pillow_color_sparse_repeated(
    table: &mut std::collections::HashMap<u32, (u32, u32)>,
    colors: &mut usize,
    pixel: u32,
    repetitions: u32,
    maxcolors: usize,
    code_mask: u32,
    code_polynomial: u32,
) -> Option<bool> {
    let mut slot = (!pixel) & code_mask;
    let mut increment = 0u32;
    let mut first_conflict = true;

    loop {
        match table.get_mut(&slot) {
            Some((stored_pixel, count)) if *stored_pixel == pixel => {
                *count = count.checked_add(repetitions)?;
                return Some(true);
            }
            Some(_) => {}
            None => {
                if *colors == maxcolors {
                    return Some(false);
                }
                table.insert(slot, (pixel, repetitions));
                *colors += 1;
                return Some(true);
            }
        }

        if first_conflict {
            increment = (pixel ^ (pixel >> 3)) & code_mask;
            if increment == 0 {
                increment = code_mask;
            }
            first_conflict = false;
        } else {
            increment <<= 1;
            if increment > code_mask {
                increment ^= code_polynomial;
            }
        }
        slot = (slot + increment) & code_mask;
    }
}

fn pillow_color_components(pixel: u32, bands: usize) -> FormattedPixelValue {
    let bytes = pixel.to_ne_bytes();
    match bands {
        // Pillow's four-byte LA storage places alpha in the final byte.
        2 => FormattedPixelValue::InlineComponents {
            bytes: [bytes[0], bytes[3], 0, 0],
            bands: 2,
        },
        3 => FormattedPixelValue::InlineComponents {
            bytes: [bytes[0], bytes[1], bytes[2], 0],
            bands: 3,
        },
        4 => FormattedPixelValue::InlineComponents { bytes, bands: 4 },
        _ => unreachable!("getcolors only supports one through four bands"),
    }
}

fn pillow_color_pixel(components: &[u8]) -> u32 {
    let mut bytes = [0u8; 4];
    match components {
        [luma, alpha] => {
            bytes[0] = *luma;
            // Pillow expands L into the three color bytes of its internal
            // 32-bit LA pixel before placing alpha in the final byte.
            bytes[1] = *luma;
            bytes[2] = *luma;
            bytes[3] = *alpha;
        }
        [first, second, third] => bytes[..3].copy_from_slice(&[*first, *second, *third]),
        [first, second, third, fourth] => {
            bytes.copy_from_slice(&[*first, *second, *third, *fourth])
        }
        _ => unreachable!("getcolors only supports one through four bands"),
    }
    u32::from_ne_bytes(bytes)
}

enum RgbGrayscaleColorCounts {
    NotGrayscale,
    TooManyColors,
    Colors(Vec<(u32, FormattedPixelValue)>),
}

enum ScalarColorDomainCounts {
    NotApplicable,
    TooManyColors,
    Colors(Vec<(u32, FormattedPixelValue)>),
}

fn scalar_byte_domain_index(mode: &str, pixel: u32) -> Option<usize> {
    match mode {
        "I" => {
            let value = i32::from_ne_bytes(pixel.to_ne_bytes());
            if (0..=255).contains(&value) {
                usize::try_from(value).ok()
            } else {
                None
            }
        }
        "F" => {
            if pixel == 0 {
                return Some(0);
            }
            if pixel >> 31 != 0 {
                return None;
            }
            let exponent = (pixel >> 23) & 0xff;
            if !(127..=134).contains(&exponent) {
                return None;
            }
            let shift = 23 - (exponent - 127);
            let significand = (1 << 23) | (pixel & 0x7f_ffff);
            let fractional_mask = (1 << shift) - 1;
            if significand & fractional_mask != 0 {
                return None;
            }
            let value = significand >> shift;
            if value <= 255 {
                usize::try_from(value).ok()
            } else {
                None
            }
        }
        _ => None,
    }
}

fn scalar_byte_domain_is_plausible(mode: &str, bytes: &[u8]) -> bool {
    let sample_count = bytes.len() / 4;
    if sample_count == 0 {
        return true;
    }
    let last_index = sample_count - 1;
    let base_step = last_index / 15;
    let remainder = last_index % 15;
    for ordinal in 0..=15usize {
        let Some(sample_index) = base_step
            .checked_mul(ordinal)
            .and_then(|base| base.checked_add((remainder * ordinal) / 15))
        else {
            return false;
        };
        let Some(start) = sample_index.checked_mul(4) else {
            return false;
        };
        let Some(end) = start.checked_add(4) else {
            return false;
        };
        let Some(sample) = bytes.get(start..end) else {
            return false;
        };
        let pixel = u32::from_ne_bytes([sample[0], sample[1], sample[2], sample[3]]);
        if scalar_byte_domain_index(mode, pixel).is_none() {
            return false;
        }
    }
    true
}

fn getcolors_scalar_byte_domain(
    mode: &str,
    bytes: &[u8],
    maxcolors: usize,
    code_mask: u32,
    code_polynomial: u32,
) -> ScalarColorDomainCounts {
    let mut histogram = [0u32; 256];
    let mut first_seen = Vec::<u8>::with_capacity(256);
    for sample in bytes.chunks_exact(4) {
        let pixel = u32::from_ne_bytes([sample[0], sample[1], sample[2], sample[3]]);
        let Some(index) = scalar_byte_domain_index(mode, pixel) else {
            return ScalarColorDomainCounts::NotApplicable;
        };
        let count = histogram[index];
        let Some(next_count) = count.checked_add(1) else {
            return ScalarColorDomainCounts::NotApplicable;
        };
        if count == 0 {
            if first_seen.len() == maxcolors {
                return ScalarColorDomainCounts::TooManyColors;
            }
            let Ok(index) = u8::try_from(index) else {
                return ScalarColorDomainCounts::NotApplicable;
            };
            first_seen.push(index);
        }
        histogram[index] = next_count;
    }

    // Preserve Pillow's first-insertion probe sequence without allocating its
    // maxcolors-sized direct table for this bounded scalar domain.
    let mut counts =
        PillowColorCounts::Sparse(std::collections::HashMap::with_capacity(first_seen.len()));
    let mut distinct_colors = 0usize;
    for value in first_seen {
        let index = usize::from(value);
        let pixel = if mode == "I" {
            u32::from_ne_bytes(i32::from(value).to_ne_bytes())
        } else {
            f32::from(value).to_bits()
        };
        let Ok(repetitions) = usize::try_from(histogram[index]) else {
            return ScalarColorDomainCounts::NotApplicable;
        };
        let Some(counted) = counts.count_repeated(
            &mut distinct_colors,
            pixel,
            repetitions,
            maxcolors,
            code_mask,
            code_polynomial,
        ) else {
            return ScalarColorDomainCounts::NotApplicable;
        };
        if !counted {
            return ScalarColorDomainCounts::TooManyColors;
        }
    }

    ScalarColorDomainCounts::Colors(
        counts
            .into_slot_order(distinct_colors)
            .into_iter()
            .map(|(_, (pixel, count))| {
                let value = if mode == "I" {
                    FormattedPixelValue::Integer(i32::from_ne_bytes(pixel.to_ne_bytes()))
                } else {
                    FormattedPixelValue::Float(f32::from_bits(pixel) as f64)
                };
                (count, value)
            })
            .collect(),
    )
}

const SCALAR_I_SPARSE_RUN_COLOR_LIMIT: usize = 4096;
const SCALAR_I_REPEATED_RUN_MIN_PREFIX_SAMPLES: usize = 256;
const SCALAR_I_REPEATED_RUN_MIN_DIRECT_SLOTS: usize = 65_536;

fn getcolors_scalar_i_repeated_runs(
    bytes: &[u8],
    maxcolors: usize,
    code_mask: u32,
    code_polynomial: u32,
) -> ScalarColorDomainCounts {
    let exact_len = bytes.len() / 4 * 4;
    let mut counts = PillowColorCounts::Sparse(std::collections::HashMap::with_capacity(256));
    let mut distinct_colors = 0usize;
    let mut offset = 0usize;

    while offset < exact_len {
        let sample = &bytes[offset..offset + 4];
        let pixel = u32::from_ne_bytes([sample[0], sample[1], sample[2], sample[3]]);

        let pattern = [
            sample[0], sample[1], sample[2], sample[3], sample[0], sample[1], sample[2], sample[3],
            sample[0], sample[1], sample[2], sample[3], sample[0], sample[1], sample[2], sample[3],
        ];
        let mut run_end = offset;
        while let Some(end) = run_end.checked_add(pattern.len()) {
            if end > exact_len || bytes[run_end..end] != pattern[..] {
                break;
            }
            run_end = end;
        }
        while let Some(end) = run_end.checked_add(4) {
            if end > exact_len || bytes[run_end..end] != sample[..] {
                break;
            }
            run_end = end;
        }

        let repetitions = (run_end - offset) / 4;
        match counts.count_repeated(
            &mut distinct_colors,
            pixel,
            repetitions,
            maxcolors,
            code_mask,
            code_polynomial,
        ) {
            Some(true) => {}
            Some(false) => return ScalarColorDomainCounts::TooManyColors,
            None => {
                for _ in 0..repetitions {
                    if !counts.count(
                        &mut distinct_colors,
                        pixel,
                        maxcolors,
                        code_mask,
                        code_polynomial,
                    ) {
                        return ScalarColorDomainCounts::TooManyColors;
                    }
                }
            }
        }
        if distinct_colors > SCALAR_I_SPARSE_RUN_COLOR_LIMIT {
            return ScalarColorDomainCounts::NotApplicable;
        }
        offset = run_end;
    }

    ScalarColorDomainCounts::Colors(
        counts
            .into_slot_order(distinct_colors)
            .into_iter()
            .map(|(_, (pixel, count))| {
                (
                    count,
                    FormattedPixelValue::Integer(i32::from_ne_bytes(pixel.to_ne_bytes())),
                )
            })
            .collect(),
    )
}

fn scalar_prefix_has_repeated_samples(bytes: &[u8]) -> bool {
    let mut samples = bytes.chunks_exact(4).take(64);
    let Some(mut previous) = samples.next() else {
        return false;
    };
    let mut repeated_pairs = 0usize;
    for sample in samples {
        if sample == previous {
            repeated_pairs += 1;
            if repeated_pairs >= 4 {
                return true;
            }
        }
        previous = sample;
    }
    false
}

fn scalar_i_prefix_has_long_run(bytes: &[u8]) -> bool {
    let Some(first) = bytes.get(..4) else {
        return false;
    };
    bytes
        .chunks_exact(4)
        .take(SCALAR_I_REPEATED_RUN_MIN_PREFIX_SAMPLES)
        .all(|sample| sample == first)
}

fn getcolors_rgb8_grayscale(
    image: &crate::raster::RgbImage,
    maxcolors: usize,
    code_mask: u32,
    code_polynomial: u32,
) -> RgbGrayscaleColorCounts {
    let mut histogram = [0u32; 256];
    let mut distinct_colors = 0usize;
    for pixel in image.as_raw().chunks_exact(3) {
        let [red, green, blue] = [pixel[0], pixel[1], pixel[2]];
        if red != green || green != blue {
            return RgbGrayscaleColorCounts::NotGrayscale;
        }
        let count = &mut histogram[usize::from(red)];
        if *count == 0 {
            distinct_colors += 1;
            if distinct_colors > maxcolors {
                return RgbGrayscaleColorCounts::TooManyColors;
            }
        }
        *count += 1;
    }

    // At most 256 grayscale keys are occupied. Preserve Pillow's slot order
    // with its same probe recurrence, without allocating or zeroing its large
    // `maxcolors`-sized table for every scanned pixel.
    let mut occupied = Vec::<(u32, u32, u32)>::with_capacity(distinct_colors);
    for (gray, count) in histogram.into_iter().enumerate() {
        if count == 0 {
            continue;
        }
        let Ok(gray) = u8::try_from(gray) else {
            continue;
        };
        let pixel = pillow_color_pixel(&[gray, gray, gray]);
        let mut slot = (!pixel) & code_mask;
        let mut increment = 0u32;
        let mut first_conflict = true;

        loop {
            if !occupied
                .iter()
                .any(|(occupied_slot, _, _)| *occupied_slot == slot)
            {
                occupied.push((slot, pixel, count));
                break;
            }

            if first_conflict {
                increment = (pixel ^ (pixel >> 3)) & code_mask;
                if increment == 0 {
                    increment = code_mask;
                }
                first_conflict = false;
            } else {
                increment <<= 1;
                if increment > code_mask {
                    increment ^= code_polynomial;
                }
            }
            slot = (slot + increment) & code_mask;
        }
    }

    occupied.sort_unstable_by_key(|(slot, _, _)| *slot);
    RgbGrayscaleColorCounts::Colors(
        occupied
            .into_iter()
            .map(|(_, pixel, count)| (count, pillow_color_components(pixel, 3)))
            .collect(),
    )
}

#[cfg(test)]
mod pillow_color_counts_tests {
    use super::{
        Image, PILLOW_COLOR_DIRECT_TABLE_MAX_BYTES, PillowColorCounts, RgbGrayscaleColorCounts,
        SCALAR_I_REPEATED_RUN_MIN_PREFIX_SAMPLES, SCALAR_I_SPARSE_RUN_COLOR_LIMIT,
        ScalarColorDomainCounts, getcolors_rgb8_grayscale, getcolors_scalar_i_repeated_runs,
        pillow_color_components, pillow_color_direct_table_slots, pillow_color_pixel,
        pillow_color_table_parameters, scalar_i_prefix_has_long_run,
        scalar_prefix_has_repeated_samples,
    };
    use crate::raster::{DynamicImage, RgbImage};

    fn scalar_i_bytes(runs: &[(i32, usize)]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for &(value, repetitions) in runs {
            for _ in 0..repetitions {
                bytes.extend_from_slice(&value.to_ne_bytes());
            }
        }
        bytes
    }

    #[test]
    fn scalar_i_repeated_runs_match_scalar_inserts_and_preserve_slot_order() {
        let bytes = scalar_i_bytes(&[
            (7, 3_072),
            (2, 1_001),
            (7, 19),
            (0, 5),
            (255, 4_096),
            (-1, 3),
        ]);
        let maxcolors = 1_000_000;
        let (code_mask, code_polynomial) =
            pillow_color_table_parameters(maxcolors).expect("test limit has a Pillow table");
        let actual = getcolors_scalar_i_repeated_runs(
            &bytes,
            maxcolors as usize,
            code_mask,
            code_polynomial,
        );
        let mut expected = PillowColorCounts::Direct(vec![0; code_mask as usize + 1]);
        let mut expected_colors = 0usize;
        for sample in bytes.chunks_exact(4) {
            let pixel = u32::from_ne_bytes([sample[0], sample[1], sample[2], sample[3]]);
            assert!(expected.count(
                &mut expected_colors,
                pixel,
                maxcolors as usize,
                code_mask,
                code_polynomial,
            ));
        }
        let expected = expected
            .into_slot_order(expected_colors)
            .into_iter()
            .map(|(_, (pixel, count))| {
                (
                    count,
                    super::FormattedPixelValue::Integer(i32::from_ne_bytes(pixel.to_ne_bytes())),
                )
            })
            .collect::<Vec<_>>();
        let ScalarColorDomainCounts::Colors(actual) = actual else {
            panic!("repeated I input must produce colors");
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn scalar_i_repeated_runs_preserve_maxcolors_limit() {
        let bytes = scalar_i_bytes(&[(1, 8), (2, 8), (3, 8)]);
        let (code_mask, code_polynomial) =
            pillow_color_table_parameters(2).expect("test limit has a Pillow table");
        assert!(matches!(
            getcolors_scalar_i_repeated_runs(&bytes, 2, code_mask, code_polynomial),
            ScalarColorDomainCounts::TooManyColors
        ));
    }

    #[test]
    fn scalar_i_repeated_runs_yield_to_general_path_after_sparse_limit() {
        let mut runs = vec![(0, SCALAR_I_REPEATED_RUN_MIN_PREFIX_SAMPLES)];
        runs.extend((1..=SCALAR_I_SPARSE_RUN_COLOR_LIMIT).map(|value| (value as i32, 1)));
        let bytes = scalar_i_bytes(&runs);
        let maxcolors = 100_000;
        let (code_mask, code_polynomial) =
            pillow_color_table_parameters(maxcolors as u32).expect("test limit has a Pillow table");

        assert!(matches!(
            getcolors_scalar_i_repeated_runs(&bytes, maxcolors, code_mask, code_polynomial,),
            ScalarColorDomainCounts::NotApplicable
        ));
    }

    #[test]
    fn scalar_i_prefix_gate_rejects_short_run_before_high_cardinality_tail() {
        let mut runs = vec![(0, 64)];
        runs.extend((1..=192).map(|value| (value, 1)));
        let bytes = scalar_i_bytes(&runs);

        assert!(!scalar_i_prefix_has_long_run(&bytes));
        assert!(scalar_prefix_has_repeated_samples(&bytes));
    }

    #[test]
    fn dense_table_admission_is_bounded_by_memory_and_scan_size() {
        assert_eq!(pillow_color_direct_table_slots(1023, 0), Some(1024));
        assert_eq!(pillow_color_direct_table_slots(2047, 1023), None);
        assert_eq!(pillow_color_direct_table_slots(2047, 1024), Some(2048));
        assert_eq!(
            pillow_color_direct_table_slots(1_048_575, 524_288),
            Some(PILLOW_COLOR_DIRECT_TABLE_MAX_BYTES / 8)
        );
        assert_eq!(
            pillow_color_direct_table_slots(2_097_151, usize::MAX),
            None,
            "the dense table must stay below its byte cap even for large images"
        );
    }

    #[test]
    fn dense_and_sparse_tables_keep_pillow_slot_order_and_limits() {
        let code_mask = 2047;
        let code_polynomial = 5;
        let pixels = [0, 2048, 1, 2049, 4096, 0, 2048, 8192];
        let mut dense = PillowColorCounts::new(code_mask, 0, 1024);
        assert!(matches!(&dense, PillowColorCounts::Direct(_)));
        let mut sparse = PillowColorCounts::Sparse(std::collections::HashMap::new());
        let mut dense_colors = 0;
        let mut sparse_colors = 0;

        for pixel in pixels {
            let dense_ok = dense.count(&mut dense_colors, pixel, 5, code_mask, code_polynomial);
            let sparse_ok = sparse.count(&mut sparse_colors, pixel, 5, code_mask, code_polynomial);
            assert_eq!(dense_ok, sparse_ok);
        }

        assert_eq!(dense_colors, 5);
        assert_eq!(dense_colors, sparse_colors);
        assert_eq!(
            dense.into_slot_order(dense_colors),
            sparse.into_slot_order(sparse_colors)
        );
        assert!(matches!(
            PillowColorCounts::new(code_mask, 0, 1023),
            PillowColorCounts::Sparse(_)
        ));
    }

    #[test]
    fn large_rgb_grayscale_getcolors_matches_pillow_slot_order() {
        let mut raw = Vec::with_capacity(256 * 256 * 3);
        for _ in 0..256 {
            for gray in 0..=u8::MAX {
                raw.extend_from_slice(&[gray, gray, gray]);
            }
        }
        let rgb = RgbImage::from_raw(256, 256, raw)
            .expect("RGB test data must contain exactly three bytes per pixel");
        let image = Image::from_dynamic(DynamicImage::ImageRgb8(rgb.clone()), Some("RGB".into()));
        let maxcolors = 1_000_000;
        let (code_mask, code_polynomial) =
            pillow_color_table_parameters(maxcolors).expect("test limit has a Pillow table");

        let mut reference = PillowColorCounts::Sparse(std::collections::HashMap::new());
        let mut reference_colors = 0;
        for pixel in rgb.pixels() {
            assert!(reference.count(
                &mut reference_colors,
                pillow_color_pixel(&pixel.0),
                maxcolors as usize,
                code_mask,
                code_polynomial,
            ));
        }
        let expected = reference
            .into_slot_order(reference_colors)
            .into_iter()
            .map(|(_, (pixel, count))| (count, pillow_color_components(pixel, 3)))
            .collect::<Vec<_>>();

        assert!(matches!(
            getcolors_rgb8_grayscale(&rgb, maxcolors as usize, code_mask, code_polynomial),
            RgbGrayscaleColorCounts::Colors(_)
        ));
        assert_eq!(
            image.getcolors(maxcolors).expect("count should succeed"),
            Some(expected)
        );
    }

    #[test]
    fn large_rgb_grayscale_shortcut_rejects_mismatched_or_excess_colors() {
        let non_gray = RgbImage::from_raw(1, 1, vec![1, 2, 1])
            .expect("RGB test data must contain exactly three bytes per pixel");
        let (code_mask, code_polynomial) =
            pillow_color_table_parameters(1_000_000).expect("test limit has a Pillow table");
        assert!(matches!(
            getcolors_rgb8_grayscale(&non_gray, 1_000_000, code_mask, code_polynomial),
            RgbGrayscaleColorCounts::NotGrayscale
        ));

        let gray = RgbImage::from_raw(3, 1, vec![0, 0, 0, 127, 127, 127, 255, 255, 255])
            .expect("RGB test data must contain exactly three bytes per pixel");
        assert!(matches!(
            getcolors_rgb8_grayscale(&gray, 2, code_mask, code_polynomial),
            RgbGrayscaleColorCounts::TooManyColors
        ));
    }
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

/// Shared once-initialized operation-ready pixel result for lazy image nodes.
pub type MaterializationCache = Arc<OnceLock<Result<Arc<DynamicImage>, PilError>>>;

fn take_cached_materialization(cache: MaterializationCache) -> Result<Arc<DynamicImage>, PilError> {
    match Arc::try_unwrap(cache) {
        Ok(cache) => cache.into_inner(),
        Err(cache) => cache.get().cloned(),
    }
    .ok_or_else(|| {
        PilError::InternalError("materialized image cache was unexpectedly empty".to_owned())
    })?
}

/// A materialized prefix that can be shared by flattened sibling pipelines.
///
/// This cache is intentionally limited to a prefix of a mode-preserving,
/// non-palette pipeline. Palette transitions and mode-changing operations
/// retain the ordinary full-chain evaluator because their representation and
/// metadata are part of the operation boundary.
#[derive(Debug, Clone)]
pub struct PipelinePrefixCache {
    /// Number of operations represented by the cached image.
    ops_len: usize,
    /// Shared result for the prefix node.
    materialized: MaterializationCache,
}

/// Shared once-initialized shape metadata for a lazy pipeline node.
///
/// `None` means that the operation list is valid but contains a dimension or
/// validation transition that cannot be determined without materialization.
/// The error case is cached as well so repeated metadata queries do not repeat
/// a failing source-header lookup.
pub type PipelineShapeCache = Arc<OnceLock<Result<Option<(u32, u32)>, PilError>>>;

/// Shared once-initialized logical output mode for a lazy pipeline node.
///
/// `None` means that the operation list contains a mode transition whose
/// representation cannot be described without materializing pixels.
pub type PipelineModeCache = Arc<OnceLock<Result<Option<String>, PilError>>>;

pub(crate) fn materialization_cache() -> MaterializationCache {
    Arc::new(OnceLock::new())
}

pub(crate) fn pipeline_shape_cache() -> PipelineShapeCache {
    Arc::new(OnceLock::new())
}

pub(crate) fn pipeline_mode_cache() -> PipelineModeCache {
    Arc::new(OnceLock::new())
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
            "LAB" => vec!["L", "A", "B"],
            "PA" => vec!["P", "A"],
            "La" => vec!["L", "a"],
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
            Image::Pipeline {
                source, ops, shape, ..
            } => {
                let planned = shape.get_or_init(|| {
                    let mut dimensions = source.size()?;
                    for op in ops.iter() {
                        let Some(next) = known_pipeline_op_dimensions(op, dimensions) else {
                            return Ok(None);
                        };
                        dimensions = next;
                    }
                    Ok(Some(dimensions))
                });
                if let Some(dimensions) = planned.as_ref().map_err(Clone::clone)? {
                    return Ok(*dimensions);
                }
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
            Image::Pipeline {
                source,
                ops,
                explicit_mode: None,
                mode,
                ..
            } => {
                let planned = mode.get_or_init(|| {
                    let mut current = source.mode()?;
                    if ops.mode_preserving() {
                        return Ok(Some(current));
                    }
                    for op in ops.iter() {
                        let Some(next) = known_pipeline_op_mode(op, &current) else {
                            return Ok(None);
                        };
                        current = next;
                    }
                    Ok(Some(current))
                });
                if let Some(mode) = planned.as_ref().map_err(Clone::clone)? {
                    return Ok(mode.clone());
                }
            }
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
                        known_zero_fill: false,
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
            let band = band as usize;
            if mode == "LAB" {
                if let DynamicImage::ImageRgb8(image) = img.as_ref() {
                    return Ok(select_lab_band(image.as_raw(), band));
                }
                return Ok(select_lab_band(img.to_rgb8().as_raw(), band));
            }
            // Keep ordinary byte layouts narrow for banded reads. The old
            // fallback widened the complete frame to RGBA before selecting a
            // single channel, which made a read-only terminal operation pay
            // for an avoidable full-frame allocation and conversion.
            match img.as_ref() {
                DynamicImage::ImageLuma8(image) if band == 0 => {
                    return Ok(image.as_raw().clone());
                }
                DynamicImage::ImageLumaA8(image) if band < 2 => {
                    return Ok(select_interleaved_band(image.as_raw(), 2, band));
                }
                DynamicImage::ImageRgb8(image) if band < 3 => {
                    return Ok(select_interleaved_band(image.as_raw(), 3, band));
                }
                DynamicImage::ImageRgba8(image) if band < 4 => {
                    return Ok(select_interleaved_band(image.as_raw(), 4, band));
                }
                _ => {}
            }
            let rgba = img.to_rgba8();
            return Ok(rgba.pixels().map(|p| p[band]).collect());
        }
        let mode = self.mode_from_materialized(&img);
        if mode == "LAB" {
            let mut data = img.to_rgb8().into_raw();
            for pixel in data.chunks_exact_mut(3) {
                pixel[1] = pixel[1].wrapping_sub(128);
                pixel[2] = pixel[2].wrapping_sub(128);
            }
            return Ok(data);
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

    /// Return the number of samples produced by Pillow's formatted band read.
    ///
    /// This validates the same mode and band constraints as
    /// [`Image::getdata_formatted`]. For indexed `P` images, it reads the size
    /// from image metadata without materializing an image when its dimensions
    /// are already known. Callers can allocate the final host-owned byte buffer
    /// before asking the core to fill it.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when band selection is unsupported for
    /// the mode or the requested band is out of range. Other errors can occur
    /// while loading or materializing the image.
    pub fn getdata_band_len(&self, band: i32) -> Result<usize, PilError> {
        let mode = self.mode()?;
        self.getdata_band_len_for_mode(&mode, band)
    }

    fn getdata_band_len_for_mode(&self, mode: &str, band: i32) -> Result<usize, PilError> {
        if is_l16_mode(mode) || mode == "I" || mode == "F" {
            return Err(PilError::ValueError("image has wrong mode".into()));
        }
        let band_count = pillow_band_count(mode);
        if band < 0 || band as usize >= band_count {
            return Err(PilError::ValueError("band index out of range".into()));
        }
        let (width, height) = self.size()?;
        Ok(crate::checked_dims::CheckedDims::new_allow_empty(width, height, 1)?.total_pixels())
    }

    /// Borrow a directly stored single-band byte plane for a host binding to
    /// copy into its final immutable result object.
    ///
    /// This avoids materialization and a Rust-side copy for owned P, 1, and L
    /// images. The returned slice is a view of the image; bindings must copy
    /// it before returning so later image mutation cannot change a GetData
    /// snapshot.
    pub fn getdata_band_view(&self, band: i32) -> Result<Option<&[u8]>, PilError> {
        if band != 0 {
            return Ok(None);
        }
        let requested_backend = match self.backend() {
            Some(backend) => Some(backend),
            None => crate::compute::preferred_active_backend()?.map(|(backend, _)| backend),
        };
        if requested_backend.is_some_and(|backend| backend != crate::compute::Backend::Cpu) {
            return Ok(None);
        }
        match self {
            Image::Paletted(data) => {
                crate::compute::record_eager_cpu_operation_receipt(
                    "GetData",
                    requested_backend,
                    data.indices.as_raw().len(),
                );
                Ok(Some(data.indices.as_raw()))
            }
            // Preserve `mode()` precedence while avoiding its owned String in
            // this eligibility-only check.
            Image::Loaded(data)
                if matches!(
                    data.explicit_mode
                        .as_deref()
                        .unwrap_or_else(|| image_mode_name(data.decoded_mode)),
                    "1" | "L"
                ) =>
            {
                let values: Option<&[u8]> = match data.image.as_ref() {
                    DynamicImage::ImageLuma8(image) => Some(image.as_raw().as_slice()),
                    _ => None,
                };
                if let Some(values) = values {
                    crate::compute::record_eager_cpu_operation_receipt(
                        "GetData",
                        requested_backend,
                        values.len(),
                    );
                }
                Ok(values)
            }
            _ => Ok(None),
        }
    }

    /// Fill a caller-owned buffer with Pillow's formatted band-read bytes.
    ///
    /// The buffer length must equal [`Image::getdata_band_len`] for the same
    /// band. This form lets host bindings write directly into their final
    /// bytes object without an intermediate Rust `Vec`.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when band selection is unsupported for
    /// the mode or the requested band is out of range, and
    /// [`PilError::InternalError`] when the output length is incorrect. Other
    /// errors can occur while loading or materializing the image.
    pub fn getdata_band_into(&self, band: i32, output: &mut [u8]) -> Result<(), PilError> {
        let mode_name = self.mode()?;
        let mode = mode_name.as_str();
        let expected = self.getdata_band_len_for_mode(mode, band)?;
        if output.len() != expected {
            return Err(PilError::InternalError(
                "getdata band output length mismatch".into(),
            ));
        }
        let selected_backend = match self.backend() {
            Some(backend) => Some(backend),
            None => crate::compute::preferred_active_backend()?.map(|(backend, _)| backend),
        };
        let backend_native = match selected_backend {
            Some(crate::compute::Backend::Simd) => matches!(
                mode,
                "LA" | "PA" | "RGB" | "RGBA" | "RGBa" | "RGBX" | "CMYK" | "HSV" | "YCbCr"
            ),
            Some(crate::compute::Backend::Gpu) => !matches!(mode, "P" | "LAB" | "I" | "F"),
            Some(crate::compute::Backend::Cpu) | None => false,
        };
        if backend_native
            && let Some(backend @ (crate::compute::Backend::Simd | crate::compute::Backend::Gpu)) =
                selected_backend
        {
            if backend == crate::compute::Backend::Simd && matches!(self, Image::Loaded(_)) {
                // A materialized image needs no deferred pipeline boundary.
                // Write SIMD channel extraction into Python's final bytes
                // allocation instead of allocating an intermediate L image
                // and copying that image back into the binding buffer.
                let image = self.materialized_shared()?;
                return crate::compute::execute_eager_simd_bytes(
                    "GetData",
                    image.as_ref(),
                    output,
                    |output| {
                        crate::compute::simd_extract_band_into(
                            image.as_ref(),
                            band as u8,
                            Some(mode),
                            output,
                        )
                    },
                );
            }

            // Keep terminal projection in the lazy graph so the selected
            // backend can fuse it with pending image work and report its real
            // execution receipt. P and LAB stay on the exact host path below:
            // P must expose raw indices, while LAB's a/b samples use Pillow's
            // signed-byte offset rather than ExtractBand's raw channel value.
            let extracted = Image::push_op(self, PipelineOp::ExtractBand { index: band as u8 })
                .use_backend(backend);
            let image = extracted.materialized_shared()?;
            let values = image.as_bytes();
            if values.len() != output.len() {
                return Err(PilError::InternalError(
                    "getdata band backend output length mismatch".into(),
                ));
            }
            output.copy_from_slice(values);
            return Ok(());
        }

        let telemetry =
            crate::compute::begin_eager_cpu_operation_telemetry("GetData", selected_backend);
        let result = (|| {
            if let Image::Paletted(data) = self {
                output.copy_from_slice(data.indices.as_raw());
                return Ok(());
            }

            let img = self.materialized_shared()?;
            let materialized_mode = self.mode_from_materialized(&img);
            let band = band as usize;
            if materialized_mode == "LAB" {
                if let DynamicImage::ImageRgb8(image) = img.as_ref() {
                    select_lab_band_into(image.as_raw(), band, output);
                } else {
                    select_lab_band_into(img.to_rgb8().as_raw(), band, output);
                }
                return Ok(());
            }

            match img.as_ref() {
                DynamicImage::ImageLuma8(image) if band == 0 => {
                    output.copy_from_slice(image.as_raw());
                    return Ok(());
                }
                DynamicImage::ImageLumaA8(image) if band < 2 => {
                    select_interleaved_band_into(image.as_raw(), 2, band, output);
                    return Ok(());
                }
                DynamicImage::ImageRgb8(image) if band < 3 => {
                    select_interleaved_band_into(image.as_raw(), 3, band, output);
                    return Ok(());
                }
                DynamicImage::ImageRgba8(image) if band < 4 => {
                    select_interleaved_band_into(image.as_raw(), 4, band, output);
                    return Ok(());
                }
                _ => {}
            }

            let rgba = img.to_rgba8();
            for (destination, pixel) in output.iter_mut().zip(rgba.pixels()) {
                *destination = pixel[band];
            }
            Ok(())
        })();
        if result.is_ok() {
            crate::compute::finish_eager_cpu_operation_telemetry(telemetry, output.len());
        } else {
            crate::compute::cancel_eager_cpu_operation_telemetry(telemetry);
        }
        result
    }

    /// Returns `getdata` values with Pillow's scalar/multiband shape applied.
    ///
    /// The byte extraction and logical band count are core behavior. Host
    /// bindings should only convert [`FormattedImageData`] into their native
    /// list/tuple representation.
    pub fn getdata_formatted(&self, band: Option<i32>) -> Result<FormattedImageData, PilError> {
        let mode = self.mode()?;
        if is_l16_mode(&mode) {
            if band.is_some() {
                // Pillow's ImagingCore.getdata rejects every explicit band
                // selection for I;16 modes with the mode error, rather than
                // the generic index error used by ordinary multi-band images.
                return Err(PilError::ValueError("image has wrong mode".into()));
            }
            return Ok(FormattedImageData::IntegerScalars(
                self.materialized_shared()?
                    .to_luma16()
                    .as_raw()
                    .iter()
                    .copied()
                    .map(i32::from)
                    .collect(),
            ));
        }
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
                data: Bytes::copy_from_slice(data),
                mode,
            },
        );
        *self = new_self;
        Ok(())
    }

    /// Queues immutable byte data without copying its backing allocation when
    /// the input already matches a complete native sample layout.
    ///
    /// This binding-oriented fast path accepts only mode 1, L, or P and only
    /// unscaled byte data with one byte per sequence entry. It preserves
    /// partial-image `putdata` semantics when the payload is shorter than the
    /// image, while retaining the immutable owner until deferred execution.
    /// Other layouts must continue through [`Image::putdata_bytes_fast_path`]
    /// so Pillow's per-entry coercion and packed-color rules remain intact.
    pub fn putdata_shared_bytes_fast_path(
        &mut self,
        data: Bytes,
        entry_count: usize,
        scale: f64,
        offset: f64,
    ) -> Result<bool, PilError> {
        if scale != 1.0 || offset != 0.0 || data.len() != entry_count {
            return Ok(false);
        }
        let mode_name = self.mode()?;
        if !matches!(mode_name.as_str(), "1" | "L" | "P") {
            return Ok(false);
        }
        self.validate_putdata_length(entry_count)?;
        let mode = PixelMode::from_name(&mode_name).ok_or_else(|| {
            PilError::ValueError(format!("unsupported putdata mode: {mode_name}"))
        })?;
        let new_self = Image::push_op(self, PipelineOp::PutData { data, mode });
        *self = new_self;
        Ok(true)
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

    /// Returns the host-language value shape required by the current mode.
    ///
    /// This is the core-owned mode policy for `putdata`. A binding may still
    /// need to inspect Python protocol behavior, but it does not need to
    /// duplicate the list of numeric and multiband mode names.
    pub fn putdata_value_kind(&self) -> Result<PutDataValueKind, PilError> {
        let mode_name = self.mode()?;
        if matches!(
            mode_name.as_str(),
            "1" | "L" | "P" | "I" | "I;16" | "I;16L" | "I;16B" | "I;16N" | "F"
        ) {
            return Ok(PutDataValueKind::Numeric);
        }
        let mode = crate::pipeline::PixelMode::from_name(&mode_name).ok_or_else(|| {
            PilError::ValueError(format!("unsupported putdata mode: {mode_name}"))
        })?;
        Ok(PutDataValueKind::Components {
            channels: mode.channels(),
        })
    }

    /// Applies a callback-free bulk input through the canonical `putdata`
    /// normalization path.
    pub fn putdata_input(
        &mut self,
        input: PutDataInput<'_>,
        scale: f64,
        offset: f64,
    ) -> Result<(), PilError> {
        match input {
            PutDataInput::Integers(values) => self.putdata_integer_values(values, scale, offset),
            PutDataInput::Numbers(values) => self.putdata_numeric_values(values, scale, offset),
        }
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

    /// Replaces exact integer samples through Pillow's numeric-or-packed
    /// `putdata` path.
    ///
    /// Pillow interprets an integer as a scaled numeric sample for
    /// single-band modes, but as a little-endian packed pixel for multiband
    /// modes. Bindings only need to extract the host integer sequence; this
    /// mode distinction belongs here so every backend receives the same
    /// canonical operation.
    pub fn putdata_integer_values(
        &mut self,
        values: &[i64],
        scale: f64,
        offset: f64,
    ) -> Result<(), PilError> {
        match self.putdata_value_kind()? {
            PutDataValueKind::Numeric => {
                let values = values
                    .iter()
                    .copied()
                    .map(|value| PutDataValue::Number(value as f64))
                    .collect::<Vec<_>>();
                self.putdata_values(&values, scale, offset)
            }
            PutDataValueKind::Components { .. } => {
                let values = values
                    .iter()
                    .copied()
                    .map(PutDataValue::Packed)
                    .collect::<Vec<_>>();
                self.putdata_values(&values, scale, offset)
            }
        }
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

    /// Applies the byte-oriented `putdata` fast paths.
    ///
    /// Returns `Ok(true)` when the current mode consumed the bytes. Other
    /// modes return `Ok(false)` so the binding can preserve Python's generic
    /// sequence and callback behavior.
    pub fn putdata_bytes_fast_path(
        &mut self,
        data: &[u8],
        entry_count: usize,
        scale: f64,
        offset: f64,
    ) -> Result<bool, PilError> {
        let mode_name = self.mode()?;
        if is_l16_mode(&mode_name) {
            self.putdata_l16_bytes(data, scale, offset)?;
            return Ok(true);
        }
        if matches!(mode_name.as_str(), "1" | "L" | "P") {
            if scale == 1.0 && offset == 0.0 && data.len() == entry_count {
                // Bytes already contain the exact one-byte samples Pillow
                // stores for these modes. Queue them directly instead of
                // widening every byte to PutDataValue and encoding it back.
                self.putdata(data)?;
                return Ok(true);
            }
            let values = data
                .iter()
                .copied()
                // Pillow reads the terminating NUL when a bytes subtype
                // reports one more item than its physical payload. Extend
                // that behavior safely for every missing item instead of
                // following its unchecked C pointer read out of bounds.
                .chain(std::iter::repeat(0))
                .take(entry_count)
                .map(|value| PutDataValue::Number(f64::from(value)))
                .collect::<Vec<_>>();
            self.putdata_values(&values, scale, offset)?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Replaces raw I;16 samples from Pillow's bytes fast path.
    ///
    /// Pillow treats a `bytes` argument to `putdata` as byte-oriented storage
    /// for I;16 modes, rather than as one numeric sample per byte. Its
    /// `_imaging.c:_putdata` image8 fast path copies at most one logical row's
    /// worth of input bytes at a time, leaving the remaining byte half of each
    /// 16-bit row zero-filled. With a scale or offset it applies the same
    /// clipping to each input byte before placing it in that row layout.
    pub fn putdata_l16_bytes(
        &mut self,
        data: &[u8],
        scale: f64,
        offset: f64,
    ) -> Result<(), PilError> {
        self.validate_putdata_length(data.len())?;
        let (width, height) = self.size()?;
        let dimensions = CheckedDims::new(width, height, 2)?;
        let mode = self.mode()?;
        let mut raw = dimensions.alloc_buffer();
        let width = width as usize;
        let row_stride = width
            .checked_mul(2)
            .ok_or_else(|| PilError::DimensionError("putdata row stride overflow".into()))?;
        for (index, byte) in data.iter().copied().enumerate() {
            let row = index / width;
            let column = index % width;
            raw[row * row_stride + column] = if scale == 1.0 && offset == 0.0 {
                byte
            } else {
                putdata_clip_u8(f64::from(byte) * scale + offset)
            };
        }
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
            image_data.known_zero_fill = false;
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
                data.known_zero_fill = false;
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
                data.known_zero_fill = false;
            }
            _ => {
                return Err(PilError::InternalError(
                    "putdata did not materialize writable storage".into(),
                ));
            }
        }
        Ok(())
    }

    /// Try applying ImageOps.grayscale immediately for the bounded native-RGB
    /// CPU/SIMD path. Other inputs keep the ordinary lazy pipeline semantics.
    pub(crate) fn try_grayscale_eager_rgb(&self) -> Result<Option<Image>, PilError> {
        if cfg!(feature = "parallel") {
            return Ok(None);
        }
        let Image::Loaded(data) = self else {
            return Ok(None);
        };
        let logical_mode = data
            .explicit_mode
            .as_deref()
            .unwrap_or_else(|| image_mode_name(data.decoded_mode));
        if logical_mode != "RGB" {
            return Ok(None);
        }

        let image = self.materialized_shared()?;
        if !matches!(image.as_ref(), DynamicImage::ImageRgb8(_)) {
            return Ok(None);
        }
        let dims =
            crate::checked_dims::CheckedDims::new_allow_empty(image.width(), image.height(), 3)?;
        let pixels = dims.total_pixels();
        if pixels == 0 || pixels > 256 || image.as_bytes().len() != dims.total_bytes() {
            return Ok(None);
        }

        let Some((backend, _)) = crate::compute::preferred_active_backend()? else {
            return Ok(None);
        };
        let operation = PipelineOp::Grayscale;
        let grayscale = match backend {
            crate::compute::Backend::Cpu => crate::compute::execute_eager_cpu_image_operation(
                "Grayscale",
                image.as_ref(),
                false,
                || crate::compute::registry::execute_cpu(&operation, image.as_ref(), Some("RGB")),
            )?,
            crate::compute::Backend::Simd => crate::compute::execute_eager_simd_image_operation(
                "Grayscale",
                image.as_ref(),
                || {
                    crate::compute::simd_grayscale_with_evidence(
                        image.as_ref(),
                        &operation,
                        Some("RGB"),
                    )
                },
            )?,
            crate::compute::Backend::Gpu => return Ok(None),
        };

        let mut result = Image::from_dynamic(grayscale, None);
        if let Image::Loaded(data) = &mut result {
            data.info = self.image_info();
            data.exif = self.exif_metadata();
        }
        Ok(Some(result))
    }

    /// Extracts one channel as an `L` image.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when `channel` is out of range, or
    /// another [`PilError`] when materialization fails.
    pub fn getchannel(&self, channel: i32) -> Result<Image, PilError> {
        let mode = self.mode()?;
        // Pillow's ImagingCore getchannel entry point rejects scalar I/F
        // storage before band extraction; it does not expose those four-byte
        // samples through the byte-oriented ExtractBand operation.
        if matches!(mode.as_str(), "I" | "F") {
            return Err(PilError::ValueError("image has wrong mode".into()));
        }
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
        // Extracting the sole byte band from an exact-sized, already-loaded
        // L image is an identity. Share its immutable pixel buffer through
        // Image::copy's copy-on-write semantics rather than scanning and
        // allocating a second full-frame buffer. Keep other backends and
        // non-exact storage on the normal extraction path so their execution
        // and trailing-sample behavior remain unchanged.
        if mode == "L" && ch == 0 && matches!(self, Image::Loaded(_)) && !cfg!(feature = "parallel")
        {
            if let Some((crate::compute::Backend::Cpu, _)) =
                crate::compute::preferred_active_backend()?
            {
                let pixel_count = crate::checked_dims::CheckedDims::new_allow_empty(
                    img.width(),
                    img.height(),
                    1,
                )?
                .total_pixels();
                if img.as_bytes().len() == pixel_count {
                    return crate::compute::execute_eager_cpu_shared_result(
                        "ExtractBand",
                        img.as_ref(),
                        || Ok(self.copy()),
                    );
                }
            }
        }
        // Tiny RGB images can extract their band directly instead of paying
        // for a one-operation pipeline and its later materialization. Keep
        // full-frame work on the pipeline path, and retain lazy execution for
        // pending graphs, SIMD, GPU, and opt-in Parallel CPU profiles.
        if matches!(self, Image::Loaded(_))
            && !cfg!(feature = "parallel")
            && mode == "RGB"
            && u64::from(img.width()) * u64::from(img.height()) <= 256
        {
            if let Some((backend, _)) = crate::compute::preferred_active_backend()? {
                let operation = PipelineOp::ExtractBand { index: ch as u8 };
                let extracted = match backend {
                    crate::compute::Backend::Cpu => {
                        Some(crate::compute::execute_eager_cpu_image_operation(
                            "ExtractBand",
                            img.as_ref(),
                            // The ExtractBand kernel records its output
                            // allocation; manual accounting would count it twice.
                            false,
                            || {
                                crate::compute::registry::execute_cpu(
                                    &operation,
                                    img.as_ref(),
                                    Some(&mode),
                                )
                            },
                        )?)
                    }
                    crate::compute::Backend::Simd => {
                        Some(crate::compute::execute_eager_simd_image_operation(
                            "ExtractBand",
                            img.as_ref(),
                            || {
                                let (width, height) = img.dimensions();
                                let mut output = crate::checked_dims::CheckedDims::new_allow_empty(
                                    width, height, 1,
                                )?
                                .alloc_buffer();
                                let (vector_blocks, scalar_tail, path) =
                                    crate::compute::simd_extract_band_into(
                                        img.as_ref(),
                                        ch as u8,
                                        Some(&mode),
                                        &mut output,
                                    )?;
                                let output =
                                    crate::raster::GrayImage::from_raw(width, height, output)
                                        .ok_or_else(|| {
                                            PilError::InternalError(
                                                "SIMD ExtractBand buffer shape mismatch".into(),
                                            )
                                        })?;
                                Ok((
                                    DynamicImage::ImageLuma8(output),
                                    vector_blocks,
                                    scalar_tail,
                                    path,
                                ))
                            },
                        )?)
                    }
                    crate::compute::Backend::Gpu => None,
                };
                if let Some(extracted) = extracted {
                    let mut result = Image::from_dynamic(extracted, None);
                    if let Image::Loaded(data) = &mut result {
                        data.info = self.image_info();
                        data.exif = self.exif_metadata();
                    }
                    return Ok(result);
                }
            }
        }
        // The validation above has already materialized a pending pipeline.
        // Branch from those resolved pixels so extracting multiple channels
        // does not replay a mode-changing parent pipeline for every band.
        let extraction_source =
            if matches!(self, Image::Pipeline { .. }) && !self.has_palette_samples() {
                self.materialized_branch()?
            } else {
                self.clone()
            };
        let extracted = Image::push_op(
            &extraction_source,
            PipelineOp::ExtractBand { index: ch as u8 },
        );
        Ok(match self.backend() {
            Some(backend) => extracted.use_backend(backend),
            None => extracted,
        })
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
    /// matching the image mode. `I` values retain signed 32-bit precision,
    /// `I;16*` values follow Pillow's wrong-mode error contract, and `F` values
    /// retain their 32-bit floating-point value.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when mode detection or materialization fails.
    #[allow(clippy::type_complexity)]
    pub fn getcolors(
        &self,
        maxcolors: u32,
    ) -> Result<Option<Vec<(u32, FormattedPixelValue)>>, PilError> {
        // GetColors over scalar samples is an eager host-CPU scan. Emit a
        // selected-step receipt only when the pixels were already materialized:
        // a first-time pipeline materialization has its own backend receipt and
        // must not be overwritten by this terminal CPU scan.
        let eager_receipt_eligible =
            crate::compute::Backend::pipeline_telemetry_enabled() && self.is_materialized();
        let mode = self.mode()?;
        if is_l16_mode(&mode) {
            // Pillow's ImagingCore rejects unsigned 16-bit luma modes here;
            // this is the public mode contract, independent of TIFF decoding.
            return Err(PilError::ValueError("image has wrong mode".into()));
        }
        let img = self.materialized_shared()?;
        if mode == "I" || mode == "F" {
            let telemetry = if eager_receipt_eligible {
                match crate::compute::preferred_active_backend() {
                    Ok(selected) => Some(crate::compute::begin_eager_cpu_operation_telemetry(
                        "GetColors",
                        selected.map(|(backend, _)| backend),
                    )),
                    Err(_) => None,
                }
            } else {
                None
            };
            let result = Self::getcolors_scalar(maxcolors, &mode, &img);
            if let Some(telemetry) = telemetry {
                if result.is_ok() {
                    crate::compute::finish_eager_cpu_operation_telemetry(telemetry, 0);
                } else {
                    crate::compute::cancel_eager_cpu_operation_telemetry(telemetry);
                }
            }
            return result;
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
        let (code_mask, code_polynomial) = pillow_color_table_parameters(maxcolors)
            .ok_or_else(|| PilError::MemoryError(String::new()))?;
        // Pillow emits multiband colors in its ImagingGetColors hash-table
        // slot order, which is neither insertion nor lexicographic order.
        // Keep Pillow's probe sequence while storing only occupied slots.
        let max_distinct_colors = maxcolors as usize;
        // Large images with the usual small color limit reject after at most
        // 257 distinct colors. Reserve only that bounded working set once.
        let initial_capacity = if maxcolors <= 256 && img.as_bytes().len() >= 64 * 1024 {
            max_distinct_colors + 1
        } else {
            0
        };
        let pixel_count = pillow_image_pixel_count(img.as_ref());
        if maxcolors > 1023 && pixel_count >= 64 * 1024 {
            if let DynamicImage::ImageRgb8(rgb) = img.as_ref() {
                match getcolors_rgb8_grayscale(rgb, max_distinct_colors, code_mask, code_polynomial)
                {
                    RgbGrayscaleColorCounts::NotGrayscale => {}
                    RgbGrayscaleColorCounts::TooManyColors => return Ok(None),
                    RgbGrayscaleColorCounts::Colors(result) => return Ok(Some(result)),
                }
            }
        }
        let mut counts = PillowColorCounts::new(code_mask, initial_capacity, pixel_count);
        let mut distinct_colors = 0usize;
        let mut count_pixel = |pixel: u32| {
            counts.count(
                &mut distinct_colors,
                pixel,
                max_distinct_colors,
                code_mask,
                code_polynomial,
            )
        };
        match (n_bands, img.as_ref()) {
            (2, crate::raster::DynamicImage::ImageLumaA8(image)) => {
                for pixel in image.pixels() {
                    if !count_pixel(pillow_color_pixel(&pixel.0)) {
                        return Ok(None);
                    }
                }
            }
            (3, crate::raster::DynamicImage::ImageRgb8(image)) => {
                for pixel in image.pixels() {
                    if !count_pixel(pillow_color_pixel(&pixel.0)) {
                        return Ok(None);
                    }
                }
            }
            (4, crate::raster::DynamicImage::ImageRgba8(image)) => {
                for pixel in image.pixels() {
                    if !count_pixel(pillow_color_pixel(&pixel.0)) {
                        return Ok(None);
                    }
                }
            }
            (2, _) => {
                let la = img.to_luma_alpha8();
                for pixel in la.pixels() {
                    if !count_pixel(pillow_color_pixel(&pixel.0)) {
                        return Ok(None);
                    }
                }
            }
            (3, _) => {
                let rgb = img.to_rgb8();
                for pixel in rgb.pixels() {
                    if !count_pixel(pillow_color_pixel(&pixel.0)) {
                        return Ok(None);
                    }
                }
            }
            (4, _) => {
                let rgba = img.to_rgba8();
                for pixel in rgba.pixels() {
                    if !count_pixel(pillow_color_pixel(&pixel.0)) {
                        return Ok(None);
                    }
                }
            }
            _ => unreachable!("getcolors only supports one through four bands"),
        }
        let result = counts.into_slot_order(distinct_colors);
        Ok(Some(
            result
                .into_iter()
                .map(|(_, (pixel, count))| (count, pillow_color_components(pixel, n_bands)))
                .collect(),
        ))
    }

    /// Applies Pillow's signed `maxcolors` argument semantics for bindings.
    /// Negative limits still load and validate the image, then return `None`.
    pub fn getcolors_with_signed_limit(
        &self,
        maxcolors: i32,
    ) -> Result<Option<Vec<(u32, FormattedPixelValue)>>, PilError> {
        if maxcolors >= 0 {
            return self.getcolors(maxcolors as u32);
        }
        let mode = self.mode()?;
        if is_l16_mode(&mode) {
            return Err(PilError::ValueError("image has wrong mode".into()));
        }
        self.materialized_shared()?;
        Ok(None)
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

    /// Returns formatted colors using Pillow's signed `maxcolors` limit.
    pub fn getcolors_formatted_with_signed_limit(
        &self,
        maxcolors: i32,
    ) -> Result<Option<Vec<(u32, FormattedPixelValue)>>, PilError> {
        self.getcolors_with_signed_limit(maxcolors)
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
            let image = self.materialized_shared()?;
            let (minimum, maximum) = image
                .as_bytes()
                .chunks_exact(4)
                .map(|sample| i32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]))
                .fold(None, |extrema: Option<(i32, i32)>, value| {
                    Some(match extrema {
                        Some((minimum, maximum)) => (minimum.min(value), maximum.max(value)),
                        None => (value, value),
                    })
                })
                .ok_or_else(|| PilError::InternalError("I image has no scalar samples".into()))?;
            return Ok(FormattedExtrema::Integer((minimum, maximum)));
        }
        if matches!(mode.as_str(), "I;16L" | "I;16B" | "I;16N") {
            // Pillow exposes the native I;16 mode to getextrema, but the
            // explicit byte-order variants are rejected by ImagingCore with
            // the public wrong-mode error.
            return Err(PilError::ValueError("image has wrong mode".into()));
        }
        if is_l16_mode(&mode) {
            let image = self.materialized_shared()?;
            let luma = image.to_luma16();
            let values = luma.as_raw().iter().copied().map(i32::from);
            let (minimum, maximum) = values
                .fold(None, |extrema: Option<(i32, i32)>, value| {
                    Some(match extrema {
                        Some((minimum, maximum)) => (minimum.min(value), maximum.max(value)),
                        None => (value, value),
                    })
                })
                .ok_or_else(|| PilError::InternalError("I;16 image has no samples".into()))?;
            return Ok(FormattedExtrema::Integer((minimum, maximum)));
        }
        if mode == "F" {
            let image = self.materialized_shared()?;
            let (minimum, maximum) = image
                .as_bytes()
                .chunks_exact(4)
                .map(|sample| {
                    f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]) as f64
                })
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
        let max_distinct_colors = maxcolors as usize;
        let mut distinct_colors = 0usize;
        for p in luma.pixels() {
            let count = &mut hist[p[0] as usize];
            if *count == 0 {
                distinct_colors += 1;
                if distinct_colors > max_distinct_colors {
                    return Ok(None);
                }
            }
            *count += 1;
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
        let (code_mask, code_polynomial) = pillow_color_table_parameters(maxcolors)
            .ok_or_else(|| PilError::MemoryError(String::new()))?;
        let max_distinct_colors = maxcolors as usize;
        let bytes = img.as_bytes();
        let pixel_count = pillow_image_pixel_count(img);
        let has_repeated_prefix = if mode == "I" {
            scalar_i_prefix_has_long_run(bytes)
        } else {
            scalar_prefix_has_repeated_samples(bytes)
        };
        let direct_table_slots = pillow_color_direct_table_slots(code_mask, pixel_count);
        if mode == "I"
            && pixel_count >= 4096
            && has_repeated_prefix
            && direct_table_slots
                .is_some_and(|slots| slots > SCALAR_I_REPEATED_RUN_MIN_DIRECT_SLOTS)
            && scalar_byte_domain_is_plausible("I", bytes)
        {
            match getcolors_scalar_i_repeated_runs(
                bytes,
                max_distinct_colors,
                code_mask,
                code_polynomial,
            ) {
                ScalarColorDomainCounts::NotApplicable => {}
                ScalarColorDomainCounts::TooManyColors => return Ok(None),
                ScalarColorDomainCounts::Colors(colors) => return Ok(Some(colors)),
            }
        }
        if pixel_count >= 4096
            && !has_repeated_prefix
            && scalar_byte_domain_is_plausible(mode, bytes)
        {
            match getcolors_scalar_byte_domain(
                mode,
                bytes,
                max_distinct_colors,
                code_mask,
                code_polynomial,
            ) {
                ScalarColorDomainCounts::NotApplicable => {}
                ScalarColorDomainCounts::TooManyColors => return Ok(None),
                ScalarColorDomainCounts::Colors(colors) => return Ok(Some(colors)),
            }
        }
        let initial_capacity = if maxcolors <= 256 && bytes.len() >= 64 * 1024 {
            max_distinct_colors + 1
        } else {
            0
        };
        let mut counts = PillowColorCounts::new(code_mask, initial_capacity, pixel_count);
        let mut distinct_colors = 0usize;
        if has_repeated_prefix {
            let mut samples = bytes.chunks_exact(4).peekable();
            while let Some(sample) = samples.next() {
                let pixel = u32::from_ne_bytes([sample[0], sample[1], sample[2], sample[3]]);
                let mut repetitions = 1usize;
                while let Some(next) = samples.peek() {
                    if *next != sample {
                        break;
                    }
                    samples.next();
                    repetitions += 1;
                }

                if repetitions == 1 {
                    if !counts.count(
                        &mut distinct_colors,
                        pixel,
                        max_distinct_colors,
                        code_mask,
                        code_polynomial,
                    ) {
                        return Ok(None);
                    }
                    continue;
                }

                match counts.count_repeated(
                    &mut distinct_colors,
                    pixel,
                    repetitions,
                    max_distinct_colors,
                    code_mask,
                    code_polynomial,
                ) {
                    Some(true) => {}
                    Some(false) => return Ok(None),
                    None => {
                        for _ in 0..repetitions {
                            if !counts.count(
                                &mut distinct_colors,
                                pixel,
                                max_distinct_colors,
                                code_mask,
                                code_polynomial,
                            ) {
                                return Ok(None);
                            }
                        }
                    }
                }
            }
        } else {
            if !count_pillow_color_scalar_samples(
                &mut counts,
                &mut distinct_colors,
                bytes,
                max_distinct_colors,
                code_mask,
                code_polynomial,
            ) {
                return Ok(None);
            }
        }
        let result = counts.into_slot_order(distinct_colors);
        Ok(Some(
            result
                .into_iter()
                .map(|(_, (pixel, count))| {
                    let value = if mode == "I" {
                        FormattedPixelValue::Integer(i32::from_ne_bytes(pixel.to_ne_bytes()))
                    } else {
                        FormattedPixelValue::Float(f32::from_bits(pixel) as f64)
                    };
                    (count, value)
                })
                .collect(),
        ))
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
        let mask_image = if let Some(mask) = mask {
            let mask_image = mask.materialized_shared()?;
            if (mask_image.width(), mask_image.height()) != (img.width(), img.height()) {
                return Err(PilError::ValueError("images do not match".into()));
            }
            let mode = mask.mode()?;
            if mode != "1" && mode != "L" {
                return Err(PilError::ValueError("bad transparency mask".into()));
            }
            Some(mask_image)
        } else {
            None
        };
        // Borrow native L8 mask bytes instead of cloning the full mask. Other
        // valid mask layouts still use the same luma conversion as before.
        let owned_mask_luma =
            mask_image
                .as_ref()
                .and_then(|mask_image| match mask_image.as_ref() {
                    crate::raster::DynamicImage::ImageLuma8(_) => None,
                    _ => Some(mask_image.to_luma8().into_raw()),
                });
        let mask_raw: Option<&[u8]> = match (&mask_image, &owned_mask_luma) {
            (Some(mask_image), None) => match mask_image.as_ref() {
                crate::raster::DynamicImage::ImageLuma8(luma) => Some(luma.as_raw().as_slice()),
                _ => unreachable!("native mask must use L8 storage"),
            },
            (_, Some(raw)) => Some(raw.as_slice()),
            _ => None,
        };
        let width = img.width() as usize;
        let masked = |index: usize| -> bool {
            match mask_raw {
                Some(mask) => mask[index] != 0,
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
            crate::raster::ColorType::La8 => {
                let crate::raster::DynamicImage::ImageLumaA8(la) = img.as_ref() else {
                    unreachable!("La8 color must use native LA storage");
                };
                let raw = la.as_raw();
                match mask_raw {
                    Some(mask) => {
                        for (index, px) in raw.chunks_exact(2).enumerate() {
                            if mask[index] == 0 {
                                continue;
                            }
                            hists[0][px[0] as usize] += 1;
                            hists[1][px[1] as usize] += 1;
                        }
                    }
                    None => {
                        for px in raw.chunks_exact(2) {
                            hists[0][px[0] as usize] += 1;
                            hists[1][px[1] as usize] += 1;
                        }
                    }
                }
            }
            crate::raster::ColorType::La16 => {
                let la = img.to_luma_alpha8();
                for (y, row) in la.rows().enumerate() {
                    for (x, px) in row.enumerate() {
                        if !masked(y * width + x) {
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
                        if !masked(y * width + x) {
                            continue;
                        }
                        hists[0][px[0] as usize] += 1;
                    }
                }
            }
            crate::raster::ColorType::Rgb8 => {
                // Masked RGB entropy only reads the three logical color
                // bands. Avoid materializing an equivalent RGBA buffer just
                // to append an unused opaque alpha channel on this hot
                // terminal path.
                let crate::raster::DynamicImage::ImageRgb8(rgb) = img.as_ref() else {
                    unreachable!("Rgb8 color must use native RGB storage");
                };
                for (y, row) in rgb.rows().enumerate() {
                    for (x, px) in row.enumerate() {
                        if !masked(y * width + x) {
                            continue;
                        }
                        hists[0][px[0] as usize] += 1;
                        hists[1][px[1] as usize] += 1;
                        hists[2][px[2] as usize] += 1;
                    }
                }
            }
            crate::raster::ColorType::Rgba8 => {
                // Rgba8 storage is already the byte sequence `to_rgba8()`
                // would return, including for four-byte carrier modes. Borrow
                // it directly instead of cloning the entire image before the
                // host histogram pass.
                let crate::raster::DynamicImage::ImageRgba8(rgba) = img.as_ref() else {
                    unreachable!("Rgba8 color must use native RGBA storage");
                };
                let raw = rgba.as_raw();
                match mask_raw {
                    Some(mask) => {
                        for (index, px) in raw.chunks_exact(4).enumerate() {
                            if mask[index] == 0 {
                                continue;
                            }
                            for band in 0..n_bands {
                                hists[band][px[band] as usize] += 1;
                            }
                        }
                    }
                    None => {
                        for px in raw.chunks_exact(4) {
                            for band in 0..n_bands {
                                hists[band][px[band] as usize] += 1;
                            }
                        }
                    }
                }
            }
            _ => {
                let rgba = img.to_rgba8();
                for (y, row) in rgba.rows().enumerate() {
                    for (x, px) in row.enumerate() {
                        if !masked(y * width + x) {
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

    fn cmyk_projection(image: &crate::raster::RgbaImage) -> (Vec<u32>, Vec<u32>) {
        Self::cmyk_projection_with_blocks(image, 16, |samples| {
            let mut packed = [0; 16];
            packed.copy_from_slice(samples);
            u128::from_ne_bytes(packed) != 0
        })
        .0
    }

    fn rgb_projection(image: &crate::raster::RgbImage) -> (Vec<u32>, Vec<u32>) {
        let (width, height) = (image.width() as usize, image.height() as usize);
        let mut horizontal = vec![0u32; width];
        let mut vertical = vec![0u32; height];
        for (y, row) in image.rows().enumerate() {
            let mut row_nonzero = false;
            for (x, pixel) in row.enumerate() {
                if pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0 {
                    horizontal[x] = 1;
                    row_nonzero = true;
                }
            }
            vertical[y] = u32::from(row_nonzero);
        }
        (horizontal, vertical)
    }

    /// Scan CMYK storage in fixed-size groups while leaving block detection to
    /// the selected execution profile. Every group is a whole number of
    /// four-byte pixels; the final partial group is handled pixel by pixel.
    pub(crate) fn cmyk_projection_with_blocks(
        image: &crate::raster::RgbaImage,
        block_bytes: usize,
        mut block_has_nonzero: impl FnMut(&[u8]) -> bool,
    ) -> ((Vec<u32>, Vec<u32>), u64) {
        let (w, h) = (image.width() as usize, image.height() as usize);
        let mut h_proj = vec![0u32; w];
        let mut v_proj = vec![0u32; h];
        let mut covered_columns = 0usize;
        let mut all_columns_covered = w == 0;
        let row_bytes = w * 4;
        let bytes = image.as_raw();
        debug_assert!(block_bytes >= 4 && block_bytes.is_multiple_of(4));
        let nonzero_4 = |samples: &[u8]| {
            let mut packed = [0; 4];
            packed.copy_from_slice(samples);
            u32::from_ne_bytes(packed) != 0
        };
        let pixels_per_block = block_bytes / 4;
        let mut scalar_tail_pixels = 0u64;
        for y in 0..h {
            let row = &bytes[y * row_bytes..(y + 1) * row_bytes];
            let blocks = row.chunks_exact(block_bytes);
            scalar_tail_pixels =
                scalar_tail_pixels.saturating_add((blocks.remainder().len() / 4) as u64);
            let row_nonzero = if all_columns_covered {
                blocks.clone().any(|block| block_has_nonzero(block))
                    || blocks.remainder().chunks_exact(4).any(nonzero_4)
            } else {
                let mut any = false;
                for (block_index, block) in blocks.clone().enumerate() {
                    if !block_has_nonzero(block) {
                        continue;
                    }
                    for (lane, pixel) in block.chunks_exact(4).enumerate() {
                        if !nonzero_4(pixel) {
                            continue;
                        }
                        let x = block_index * pixels_per_block + lane;
                        if h_proj[x] == 0 {
                            h_proj[x] = 1;
                            covered_columns += 1;
                            all_columns_covered = covered_columns == w;
                        }
                        any = true;
                    }
                }
                let tail_start = blocks.len() * block_bytes;
                for (tail_index, pixel) in row[tail_start..].chunks_exact(4).enumerate() {
                    if !nonzero_4(pixel) {
                        continue;
                    }
                    let x = tail_start / 4 + tail_index;
                    if h_proj[x] == 0 {
                        h_proj[x] = 1;
                        covered_columns += 1;
                        all_columns_covered = covered_columns == w;
                    }
                    any = true;
                }
                any
            };
            if row_nonzero {
                v_proj[y] = 1;
            }
        }
        ((h_proj, v_proj), scalar_tail_pixels)
    }

    fn cmyk_projection_with_putpixel_ops(
        image: &crate::raster::RgbaImage,
        ops: &[PipelineOp],
        source_is_known_zero: bool,
    ) -> Result<(Vec<u32>, Vec<u32>), PilError> {
        Self::cmyk_projection_with_putpixel_ops_using(image, ops, source_is_known_zero, |image| {
            let (horizontal, vertical) = Self::cmyk_projection(image);
            (horizontal, vertical, 0, 0)
        })
        .map(|(horizontal, vertical, _, _)| (horizontal, vertical))
    }

    fn rgb_projection_with_putpixel_ops(
        image: &crate::raster::RgbImage,
        ops: &[PipelineOp],
        source_is_known_zero: bool,
    ) -> Result<(Vec<u32>, Vec<u32>), PilError> {
        Self::rgb_projection_with_putpixel_ops_using(image, ops, source_is_known_zero, |image| {
            let (horizontal, vertical) = Self::rgb_projection(image);
            (horizontal, vertical, 0, 0)
        })
        .map(|(horizontal, vertical, _, _)| (horizontal, vertical))
    }

    fn cmyk_projection_with_putpixel_ops_using(
        image: &crate::raster::RgbaImage,
        ops: &[PipelineOp],
        source_is_known_zero: bool,
        project: impl FnOnce(&crate::raster::RgbaImage) -> (Vec<u32>, Vec<u32>, u64, u64),
    ) -> Result<(Vec<u32>, Vec<u32>, u64, u64), PilError> {
        Self::projection_with_putpixel_ops_using(
            image.width(),
            image.height(),
            image.as_raw(),
            4,
            "CMYK",
            ops,
            source_is_known_zero,
            || project(image),
        )
    }

    fn rgb_projection_with_putpixel_ops_using(
        image: &crate::raster::RgbImage,
        ops: &[PipelineOp],
        source_is_known_zero: bool,
        project: impl FnOnce(&crate::raster::RgbImage) -> (Vec<u32>, Vec<u32>, u64, u64),
    ) -> Result<(Vec<u32>, Vec<u32>, u64, u64), PilError> {
        Self::projection_with_putpixel_ops_using(
            image.width(),
            image.height(),
            image.as_raw(),
            3,
            "RGB",
            ops,
            source_is_known_zero,
            || project(image),
        )
    }

    fn projection_with_putpixel_ops_using(
        width: u32,
        height: u32,
        bytes: &[u8],
        channels: usize,
        mode: &str,
        ops: &[PipelineOp],
        source_is_known_zero: bool,
        project: impl FnOnce() -> (Vec<u32>, Vec<u32>, u64, u64),
    ) -> Result<(Vec<u32>, Vec<u32>, u64, u64), PilError> {
        let (w, h) = (width as usize, height as usize);
        let mut pending_writes = Vec::with_capacity(ops.len());
        for (order, op) in ops.iter().enumerate() {
            let PipelineOp::PutPixel { x, y, color, .. } = op else {
                return Err(PilError::InternalError(format!(
                    "{mode} projection fusion received a non-pixel operation"
                )));
            };
            if *x >= width || *y >= height {
                return Err(PilError::IndexError("image index out of range".into()));
            }
            let index = *y as usize * w + *x as usize;
            pending_writes.push((index, order, [color.0, color.1, color.2, color.3]));
        }
        pending_writes.sort_unstable_by_key(|(index, order, _)| (*index, *order));
        let mut writes: Vec<(usize, [u8; 4])> = Vec::with_capacity(pending_writes.len());
        for (index, _, color) in pending_writes {
            if let Some((previous_index, previous_color)) = writes.last_mut()
                && *previous_index == index
            {
                *previous_color = color;
            } else {
                writes.push((index, color));
            }
        }

        // The constructor's immutable fill invariant lets the serial CPU
        // terminal path derive the result from deferred writes without
        // rereading the full backing frame. SIMD callers deliberately leave
        // this false so their receipt still proves vector execution.
        let (mut h_proj, mut v_proj, vector_blocks, scalar_tail) = if source_is_known_zero {
            (vec![0; w], vec![0; h], 0, 0)
        } else {
            project()
        };
        let mut cleared_columns = Vec::new();
        let mut cleared_rows = Vec::new();
        if source_is_known_zero {
            for &(index, color) in &writes {
                if color[..channels].iter().any(|value| *value != 0) {
                    h_proj[index % w] = 1;
                    v_proj[index / w] = 1;
                }
            }
        } else {
            for &(index, color) in &writes {
                let x = index % w;
                let y = index / w;
                if color[..channels].iter().any(|value| *value != 0) {
                    h_proj[x] = 1;
                    v_proj[y] = 1;
                } else {
                    cleared_columns.push(x);
                    cleared_rows.push(y);
                }
            }
        }
        cleared_columns.sort_unstable();
        cleared_columns.dedup();
        cleared_rows.sort_unstable();
        cleared_rows.dedup();

        let pixel_is_nonzero = |index: usize| {
            if let Ok(write) = writes.binary_search_by_key(&index, |(write_index, _)| *write_index)
            {
                writes[write].1[..channels].iter().any(|value| *value != 0)
            } else {
                let offset = index * channels;
                bytes[offset..offset + channels]
                    .iter()
                    .any(|value| *value != 0)
            }
        };
        for x in cleared_columns {
            h_proj[x] = u32::from((0..h).any(|y| pixel_is_nonzero(y * w + x)));
        }
        for y in cleared_rows {
            v_proj[y] = u32::from((0..w).any(|x| pixel_is_nonzero(y * w + x)));
        }
        Ok((h_proj, v_proj, vector_blocks, scalar_tail))
    }

    fn try_getprojection_putpixel_pipeline(
        &self,
    ) -> Result<Option<(Vec<u32>, Vec<u32>)>, PilError> {
        // A projection is the terminal result here. Applying queued RGB or
        // CMYK pixels to the two axes is exact and avoids creating a full-frame
        // image that the caller never receives. The lazy pipeline remains
        // intact for any later image operation.
        let Image::Pipeline {
            source,
            ops,
            explicit_mode,
            backend,
            ..
        } = self
        else {
            return Ok(None);
        };
        if !matches!(source.as_ref(), Image::Loaded(_) | Image::Bytes { .. }) {
            return Ok(None);
        }
        let mode = match explicit_mode.as_deref() {
            Some("RGB") => "RGB",
            Some("CMYK") => "CMYK",
            Some(_) => return Ok(None),
            // RGB is represented by the native three-byte raster and usually
            // has no explicit mode tag. Resolve that ordinary mode from the
            // source before admitting the fused terminal path.
            None if source.mode()? == "RGB" => "RGB",
            None => return Ok(None),
        };
        if ops.is_empty() {
            return Ok(None);
        }
        let terminal_backend = match backend {
            Some(crate::compute::Backend::Cpu) => Some(crate::compute::Backend::Cpu),
            Some(crate::compute::Backend::Simd) => Some(crate::compute::Backend::Simd),
            #[cfg(feature = "gpu")]
            Some(crate::compute::Backend::Gpu) => Some(crate::compute::Backend::Gpu),
            #[cfg(not(feature = "gpu"))]
            Some(crate::compute::Backend::Gpu) => None,
            // A single-backend profile already gives unlocked lazy graphs an
            // unambiguous route. Preserve terminal fusion in those profiles
            // without requiring benchmark adapters to mutate each image
            // handle just to prove which backend executed.
            None => match crate::compute::preferred_active_backend()? {
                Some((backend, true)) => Some(backend),
                _ => None,
            },
        };
        let Some(terminal_backend) = terminal_backend else {
            return Ok(None);
        };
        if !ops
            .as_slice()
            .iter()
            .all(|op| matches!(op, PipelineOp::PutPixel { .. }))
        {
            return Ok(None);
        }
        let prepared = crate::compute::prepare_execution(ops.as_slice(), Some(terminal_backend))?;
        let backend_matches = match terminal_backend {
            crate::compute::Backend::Cpu => prepared.is_serial_cpu_without_fallback(),
            crate::compute::Backend::Simd => prepared.is_simd_without_fallback(),
            crate::compute::Backend::Gpu => prepared.is_gpu_without_fallback(),
        };
        if !backend_matches {
            return Ok(None);
        }
        let source_is_known_zero = matches!(
            source.as_ref(),
            Image::Loaded(data) if data.known_zero_fill
        );
        let input = source.materialized_shared()?;
        if source.mode_from_materialized(&input) != mode {
            return Ok(None);
        }
        #[cfg(feature = "gpu")]
        if terminal_backend == crate::compute::Backend::Gpu {
            let supported = match mode {
                "RGB" => crate::compute::gpu_rgb_projection_terminal_supported(
                    input.as_ref(),
                    ops.as_slice(),
                ),
                "CMYK" => crate::compute::gpu_cmyk_projection_terminal_supported(
                    input.as_ref(),
                    ops.as_slice(),
                ),
                _ => false,
            };
            if !supported {
                return Ok(None);
            }
        }
        let (width, height) = (input.width() as usize, input.height() as usize);
        let output_sizes = [
            width * std::mem::size_of::<u32>(),
            height * std::mem::size_of::<u32>(),
        ];
        let result = match (mode, terminal_backend) {
            ("CMYK", crate::compute::Backend::Cpu) => {
                let DynamicImage::ImageRgba8(image) = input.as_ref() else {
                    return Ok(None);
                };
                crate::compute::execute_prepared_cpu_terminal(
                    &prepared,
                    ops.as_slice(),
                    input.as_ref(),
                    output_sizes,
                    || {
                        Self::cmyk_projection_with_putpixel_ops(
                            image,
                            ops.as_slice(),
                            source_is_known_zero,
                        )
                    },
                )?
            }
            ("RGB", crate::compute::Backend::Cpu) => {
                let DynamicImage::ImageRgb8(image) = input.as_ref() else {
                    return Ok(None);
                };
                crate::compute::execute_prepared_cpu_terminal(
                    &prepared,
                    ops.as_slice(),
                    input.as_ref(),
                    output_sizes,
                    || {
                        Self::rgb_projection_with_putpixel_ops(
                            image,
                            ops.as_slice(),
                            source_is_known_zero,
                        )
                    },
                )?
            }
            ("CMYK", crate::compute::Backend::Simd) => {
                let DynamicImage::ImageRgba8(image) = input.as_ref() else {
                    return Ok(None);
                };
                crate::compute::execute_prepared_simd_terminal(
                    &prepared,
                    ops.as_slice(),
                    input.as_ref(),
                    output_sizes,
                    || {
                        if let Some((horizontal, vertical, vector_blocks, scalar_tail)) =
                            source_is_known_zero
                                .then(|| {
                                    crate::compute::simd_cmyk_projection_from_positive_writes(
                                        image.width(),
                                        image.height(),
                                        ops.as_slice(),
                                    )
                                })
                                .flatten()
                        {
                            Ok(((horizontal, vertical), vector_blocks, scalar_tail))
                        } else {
                            Self::cmyk_projection_with_putpixel_ops_using(
                                image,
                                ops.as_slice(),
                                false,
                                |image| crate::compute::simd_cmyk_projection(image),
                            )
                            .map(
                                |(horizontal, vertical, vector_blocks, scalar_tail)| {
                                    ((horizontal, vertical), vector_blocks, scalar_tail)
                                },
                            )
                        }
                    },
                )?
            }
            ("RGB", crate::compute::Backend::Simd) => {
                let DynamicImage::ImageRgb8(image) = input.as_ref() else {
                    return Ok(None);
                };
                crate::compute::execute_prepared_simd_terminal(
                    &prepared,
                    ops.as_slice(),
                    input.as_ref(),
                    output_sizes,
                    || {
                        Self::rgb_projection_with_putpixel_ops_using(
                            image,
                            ops.as_slice(),
                            false,
                            |image| crate::compute::simd_rgb_projection(image),
                        )
                        .map(
                            |(horizontal, vertical, vector_blocks, scalar_tail)| {
                                ((horizontal, vertical), vector_blocks, scalar_tail)
                            },
                        )
                    },
                )?
            }
            #[cfg(feature = "gpu")]
            ("CMYK", crate::compute::Backend::Gpu) => {
                return crate::compute::execute_prepared_gpu_cmyk_projection_terminal(
                    &prepared,
                    ops.as_slice(),
                    input.as_ref(),
                    source_is_known_zero,
                )
                .map(Some);
            }
            #[cfg(feature = "gpu")]
            ("RGB", crate::compute::Backend::Gpu) => {
                return crate::compute::execute_prepared_gpu_rgb_projection_terminal(
                    &prepared,
                    ops.as_slice(),
                    input.as_ref(),
                    source_is_known_zero,
                )
                .map(Some);
            }
            #[cfg(not(feature = "gpu"))]
            (_, crate::compute::Backend::Gpu) => return Ok(None),
            _ => return Ok(None),
        };
        Ok(Some(result))
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
        if let Some(projection) = self.try_getprojection_putpixel_pipeline()? {
            return Ok(projection);
        }
        let img = self.materialized_shared()?;
        let mode = self.mode_from_materialized(&img);
        if let Some(projection) =
            self.try_getprojection_eager_backend(img.as_ref(), mode.as_str())?
        {
            return Ok(projection);
        }
        if !crate::compute::Backend::pipeline_telemetry_enabled() {
            return Self::getprojection_from_materialized(img.as_ref(), mode.as_str());
        }

        let selected = crate::compute::preferred_active_backend().ok().flatten();
        let requested_backend = selected
            .filter(|(_, only_active_backend)| *only_active_backend)
            .map(|(backend, _)| backend);
        let fallback_reason = requested_backend
            .filter(|backend| *backend != crate::compute::Backend::Cpu)
            .map(|backend| {
                format!(
                    "GetProjection materialized pipeline used the host CPU while {backend:?} was requested"
                )
            });
        crate::compute::execute_eager_cpu_getprojection(
            img.as_ref(),
            requested_backend,
            fallback_reason,
            || Self::getprojection_from_materialized(img.as_ref(), mode.as_str()),
        )
    }

    fn try_getprojection_eager_backend(
        &self,
        img: &DynamicImage,
        mode: &str,
    ) -> Result<Option<(Vec<u32>, Vec<u32>)>, PilError> {
        if !matches!(self, Image::Loaded(_) | Image::Paletted(_)) {
            return Ok(None);
        }
        let Some((selected, only_active_backend)) = crate::compute::preferred_active_backend()?
        else {
            return Ok(None);
        };
        let requested = only_active_backend.then_some(selected);
        match selected {
            crate::compute::Backend::Cpu => {
                crate::compute::execute_eager_cpu_getprojection(img, requested, None, || {
                    Self::getprojection_from_materialized(img, mode)
                })
                .map(Some)
            }
            crate::compute::Backend::Simd if mode == "RGB" => {
                let DynamicImage::ImageRgb8(image) = img else {
                    return Ok(None);
                };
                crate::compute::execute_eager_simd_getprojection(img, || {
                    let (horizontal, vertical, vector_blocks, scalar_tail) =
                        crate::compute::simd_rgb_projection(image);
                    Ok(((horizontal, vertical), vector_blocks, scalar_tail))
                })
                .map(Some)
            }
            crate::compute::Backend::Simd if mode == "CMYK" => {
                let DynamicImage::ImageRgba8(image) = img else {
                    return Ok(None);
                };
                crate::compute::execute_eager_simd_getprojection(img, || {
                    let (horizontal, vertical, vector_blocks, scalar_tail) =
                        crate::compute::simd_cmyk_projection(image);
                    Ok(((horizontal, vertical), vector_blocks, scalar_tail))
                })
                .map(Some)
            }
            crate::compute::Backend::Simd => crate::compute::execute_eager_cpu_getprojection(
                img,
                requested,
                Some(format!("SIMD getprojection is unsupported for mode {mode}")),
                || Self::getprojection_from_materialized(img, mode),
            )
            .map(Some),
            #[cfg(feature = "gpu")]
            crate::compute::Backend::Gpu if mode == "RGB" => {
                if crate::compute::gpu_rgb_projection_supported(img) {
                    crate::compute::execute_eager_gpu_rgb_getprojection(img).map(Some)
                } else {
                    crate::compute::execute_eager_cpu_getprojection(
                        img,
                        requested,
                        Some("GPU getprojection is unsupported for this RGB image".into()),
                        || Self::getprojection_from_materialized(img, mode),
                    )
                    .map(Some)
                }
            }
            #[cfg(feature = "gpu")]
            crate::compute::Backend::Gpu => crate::compute::execute_eager_cpu_getprojection(
                img,
                requested,
                Some(format!("GPU getprojection is unsupported for mode {mode}")),
                || Self::getprojection_from_materialized(img, mode),
            )
            .map(Some),
            #[cfg(not(feature = "gpu"))]
            crate::compute::Backend::Gpu => crate::compute::execute_eager_cpu_getprojection(
                img,
                requested,
                Some(format!("GPU getprojection is unsupported for mode {mode}")),
                || Self::getprojection_from_materialized(img, mode),
            )
            .map(Some),
        }
    }

    fn getprojection_from_materialized(
        img: &DynamicImage,
        mode: &str,
    ) -> Result<(Vec<u32>, Vec<u32>), PilError> {
        let (w, h) = (img.width() as usize, img.height() as usize);
        let mut h_proj = vec![0u32; w];
        let mut v_proj = vec![0u32; h];

        // Pillow's ImagingGetProjection checks source bands for non-zero
        // samples. Converting RGB/RGBA to luma first loses low red/blue and
        // alpha-only pixels, even though Pillow projects those pixels.
        if matches!(mode, "I" | "F") {
            // Materialization validates scalar storage, so decode each sample
            // from the retained frame a row at a time. Projection only records
            // whether a column and row contain any non-zero word, so direct
            // coordinates avoid a division/modulo and repeated row writes per
            // sample. Once every column is marked, each remaining row needs
            // only its first non-zero sample to determine its vertical bit.
            let row_bytes = w.checked_mul(std::mem::size_of::<u32>()).ok_or_else(|| {
                PilError::DimensionError("getprojection row byte count overflow".into())
            })?;
            if row_bytes != 0 {
                let raw = img.as_bytes();
                let mut remaining_columns = w;
                for (y, row) in raw.chunks_exact(row_bytes).take(h).enumerate() {
                    let row_nonzero = if remaining_columns == 0 {
                        row.chunks_exact(4).any(|sample| {
                            u32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]) != 0
                        })
                    } else {
                        let mut row_nonzero = false;
                        if mode == "I" {
                            for (x, sample) in row.chunks_exact(4).enumerate() {
                                let value = i32::from_le_bytes([
                                    sample[0], sample[1], sample[2], sample[3],
                                ]);
                                if value != 0 {
                                    row_nonzero = true;
                                    if h_proj[x] == 0 {
                                        h_proj[x] = 1;
                                        remaining_columns -= 1;
                                    }
                                    if remaining_columns == 0 {
                                        break;
                                    }
                                }
                            }
                        } else {
                            for (x, sample) in row.chunks_exact(4).enumerate() {
                                // Pillow projects F samples by their stored
                                // word: -0.0's sign bit makes it nonzero.
                                let value = u32::from_le_bytes([
                                    sample[0], sample[1], sample[2], sample[3],
                                ]);
                                if value != 0 {
                                    row_nonzero = true;
                                    if h_proj[x] == 0 {
                                        h_proj[x] = 1;
                                        remaining_columns -= 1;
                                    }
                                    if remaining_columns == 0 {
                                        break;
                                    }
                                }
                            }
                        }
                        row_nonzero
                    };
                    if row_nonzero {
                        v_proj[y] = 1;
                    }
                }
            }
        } else {
            let mut mark = |index: usize| {
                let x = index % w;
                let y = index / w;
                h_proj[x] = 1;
                v_proj[y] = 1;
            };
            match img {
                crate::raster::DynamicImage::ImageLuma8(image)
                    if matches!(mode, "L" | "1" | "P") =>
                {
                    for (index, pixel) in image.pixels().enumerate() {
                        if pixel[0] != 0 {
                            mark(index);
                        }
                    }
                }
                crate::raster::DynamicImage::ImageLumaA8(image) if matches!(mode, "LA" | "PA") => {
                    for (index, pixel) in image.pixels().enumerate() {
                        if pixel[0] != 0 || pixel[1] != 0 {
                            mark(index);
                        }
                    }
                }
                crate::raster::DynamicImage::ImageRgb8(image)
                    if matches!(mode, "RGB" | "HSV" | "YCbCr") =>
                {
                    if w >= 64 {
                        // For wider images, walk packed rows directly to avoid
                        // a per-pixel index division in `mark`. Once every
                        // horizontal column is set, later rows only need a
                        // short-circuit nonzero scan for their vertical bit.
                        let row_bytes = w * 3;
                        let mut covered_columns = 0usize;
                        let mut all_columns_covered = false;
                        for (y, row) in image.as_raw().chunks_exact(row_bytes).enumerate() {
                            if all_columns_covered {
                                v_proj[y] = u32::from(row.iter().any(|value| *value != 0));
                                continue;
                            }
                            let mut row_nonzero = false;
                            for (x, pixel) in row.chunks_exact(3).enumerate() {
                                if pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 0 {
                                    continue;
                                }
                                if h_proj[x] == 0 {
                                    h_proj[x] = 1;
                                    covered_columns += 1;
                                }
                                row_nonzero = true;
                                if covered_columns == w {
                                    all_columns_covered = true;
                                    break;
                                }
                            }
                            v_proj[y] = u32::from(row_nonzero);
                        }
                    } else {
                        // Keep the existing traversal for narrow images, where
                        // row-state setup did not produce a reliable gain.
                        for (index, pixel) in image.pixels().enumerate() {
                            if pixel.0.iter().any(|value| *value != 0) {
                                mark(index);
                            }
                        }
                    }
                }
                crate::raster::DynamicImage::ImageRgb8(image) if mode == "LAB" => {
                    // LAB stores its logical A/B zero points at byte value
                    // 128. Scanning through RGBA would synthesize alpha 255
                    // and mark every pixel, while testing raw bytes against
                    // zero would mark neutral LAB pixels.
                    for (index, pixel) in image.pixels().enumerate() {
                        if pixel[0] != 0 || pixel[1] != 128 || pixel[2] != 128 {
                            mark(index);
                        }
                    }
                }
                crate::raster::DynamicImage::ImageRgba8(image)
                    if matches!(mode, "RGBA" | "RGBa" | "RGBX") =>
                {
                    for (index, pixel) in image.pixels().enumerate() {
                        if pixel.0.iter().any(|value| *value != 0) {
                            mark(index);
                        }
                    }
                }
                crate::raster::DynamicImage::ImageRgba8(image) if mode == "CMYK" => {
                    (h_proj, v_proj) = Self::cmyk_projection(image);
                }
                _ => match mode {
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
                },
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
                dest_map: dest_map.to_vec().into(),
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

#[cfg(test)]
mod byte_export_tests {
    use super::{Image, PilError, PutDataValue, is_l16_mode, l16_uses_big_endian};

    const MODES: &[(&str, usize)] = &[
        ("1", 1),
        ("L", 1),
        ("P", 1),
        ("LA", 2),
        ("PA", 2),
        ("RGB", 3),
        ("LAB", 3),
        ("RGBA", 4),
        ("RGBa", 4),
        ("RGBX", 4),
        ("CMYK", 4),
        ("HSV", 3),
        ("YCbCr", 3),
        ("I", 4),
        ("F", 4),
        ("I;16", 2),
        ("I;16L", 2),
        ("I;16B", 2),
        ("I;16N", 2),
    ];

    #[test]
    fn native_default_exports_share_storage() -> Result<(), PilError> {
        for &(mode, channels) in MODES {
            if matches!(mode, "1" | "P")
                || (is_l16_mode(mode) && cfg!(target_endian = "big") != l16_uses_big_endian(mode))
            {
                continue;
            }
            for (width, height) in [(3, 2), (0, 3), (3, 0)] {
                let bytes: Vec<u8> = (0..channels * width as usize * height as usize)
                    .map(|index| index.wrapping_mul(37).wrapping_add(11).to_le_bytes()[0])
                    .collect();
                let image = Image::frombytes(mode, (width, height), &bytes)?;
                let storage = image.materialized_shared()?;
                let exported = image.tobytes_encoded_shared(mode, "raw", &[])?;
                assert_eq!(exported.as_ref(), bytes, "{mode} {width}x{height}");
                assert_eq!(
                    exported.as_ref().as_ptr(),
                    storage.as_bytes().as_ptr(),
                    "native {mode} export should retain the materialized allocation"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn owned_frombytes_adopts_raw_raster_allocation() -> Result<(), PilError> {
        let bytes = vec![0x10, 0x20, 0x30, 0x40, 0x50, 0x60];
        let input_ptr = bytes.as_ptr();
        let image = Image::frombytes_owned("RGB", (2, 1), bytes)?;
        let storage = image.materialized_shared()?;

        assert_eq!(storage.as_bytes().as_ptr(), input_ptr);
        assert_eq!(storage.as_bytes(), &[0x10, 0x20, 0x30, 0x40, 0x50, 0x60]);
        Ok(())
    }

    #[test]
    fn shared_exports_preserve_encoder_layouts_and_errors() -> Result<(), PilError> {
        for &(source_mode, channels) in MODES {
            let bytes: Vec<u8> = (0..9 * 2 * channels)
                .map(|index| index.wrapping_mul(43).wrapping_add(17).to_le_bytes()[0])
                .collect();
            let image = Image::frombytes(source_mode, (9, 2), &bytes)?;
            for mode in [source_mode, "1", "RGB", "BGRA", "I;16L", "I;16B"] {
                for raw_mode in [
                    None,
                    Some(mode),
                    Some("BGR"),
                    Some("BGRA"),
                    Some("RGBA"),
                    Some("invalid"),
                ] {
                    let args: Vec<String> = raw_mode.into_iter().map(str::to_owned).collect();
                    for encoder in ["raw", "invalid"] {
                        let existing = image
                            .tobytes_encoded(mode, encoder, &args)
                            .map_err(|error| format!("{error:?}"));
                        let shared = image
                            .tobytes_encoded_shared(mode, encoder, &args)
                            .map(|bytes| bytes.as_ref().to_vec())
                            .map_err(|error| format!("{error:?}"));
                        assert_eq!(shared, existing, "{source_mode}: {mode}/{encoder}/{args:?}");
                    }
                }
            }
        }
        Ok(())
    }

    #[test]
    fn shared_scalar_exports_preserve_all_sample_bits() -> Result<(), PilError> {
        let words: [u32; 6] = [
            0x8000_0000,
            0x7fc0_1234,
            0x7f80_0001,
            0xff80_0000,
            0x8000_0001,
            0xffff_ffff,
        ];
        let bytes: Vec<u8> = words.into_iter().flat_map(u32::to_le_bytes).collect();
        for mode in ["I", "F"] {
            let image = Image::frombytes(mode, (3, 2), &bytes)?;
            let exported = image.tobytes_encoded_shared(mode, "raw", &[])?;
            assert_eq!(exported.as_ref(), bytes, "{mode}");
        }
        Ok(())
    }

    #[test]
    fn exported_bytes_survive_image_mutation_and_drop() -> Result<(), PilError> {
        for mode in ["L", "P", "I;16N"] {
            let bytes = if mode == "I;16N" {
                vec![1, 2, 3, 4]
            } else {
                vec![1, 2]
            };
            let mut image = Image::frombytes(mode, (2, 1), &bytes)?;
            let exported = image.tobytes_encoded_shared(mode, "raw", &[])?;
            image.putdata_value_at(0, &PutDataValue::Number(77.0), 1.0, 0.0)?;
            assert_ne!(
                image.tobytes()?,
                bytes,
                "{mode} mutation must be observable"
            );
            drop(image);
            assert_eq!(
                exported.as_ref(),
                bytes,
                "{mode} export must stay immutable"
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod putdata_shared_bytes_tests {
    use super::{Image, PilError};
    use crate::compute::Backend;
    use crate::pipeline::{PipelineOp, PixelMode};
    use bytes::Bytes;

    #[test]
    fn native_l_putdata_keeps_the_shared_allocation_and_materializes_exactly()
    -> Result<(), PilError> {
        let payload = Bytes::from_static(&[13, 71, 209, 255]);
        let payload_ptr = payload.as_ptr();
        let mut image = Image::new(4, 1, "L", (0, 0, 0, 255))?;

        assert!(image.putdata_shared_bytes_fast_path(payload, 4, 1.0, 0.0)?);
        let queued_ptr = match &image {
            Image::Pipeline { ops, .. } => match &ops.as_slice()[0] {
                PipelineOp::PutData {
                    data,
                    mode: PixelMode::L,
                } => data.as_ptr(),
                other => panic!("unexpected queued operation: {other:?}"),
            },
            other => panic!("putdata did not queue a pipeline: {other:?}"),
        };
        assert_eq!(queued_ptr, payload_ptr);

        assert_eq!(
            image.use_backend(Backend::Cpu).tobytes()?,
            [13, 71, 209, 255]
        );
        Ok(())
    }
}

#[cfg(test)]
mod getprojection_cmyk_tests {
    use super::{Image, PilError, PutDataValue};

    #[test]
    fn cpu_known_zero_cmyk_projection_uses_sparse_writes() -> Result<(), PilError> {
        use crate::compute::Backend;

        let mut image = Image::new(7, 3, "CMYK", (0, 0, 0, 0))?;
        image.putpixel(1, 0, 1, 0, 0, 0)?;
        image.putpixel(4, 1, 0, 1, 0, 0)?;
        image.putpixel(6, 2, 0, 0, 0, 1)?;
        assert!(matches!(
            &image,
            Image::Pipeline { source, .. }
                if matches!(source.as_ref(), Image::Loaded(data) if data.known_zero_fill)
        ));
        image = image.use_backend(Backend::Cpu);

        assert_eq!(
            image.getprojection()?,
            (vec![0, 1, 0, 0, 1, 0, 1], vec![1, 1, 1])
        );
        Ok(())
    }

    #[test]
    fn eager_pixel_data_mutation_invalidates_known_zero_source() -> Result<(), PilError> {
        use crate::compute::Backend;

        let mut image = Image::new(4, 1, "CMYK", (0, 0, 0, 0))?;
        assert!(matches!(
            &image,
            Image::Loaded(data) if data.known_zero_fill
        ));
        image.putdata_value_at(1, &PutDataValue::Components(vec![1, 0, 0, 0]), 1.0, 0.0)?;
        assert!(matches!(
            &image,
            Image::Loaded(data) if !data.known_zero_fill
        ));
        image.putpixel(0, 0, 0, 0, 0, 0)?;
        image = image.use_backend(Backend::Cpu);

        assert_eq!(image.getprojection()?, (vec![0, 1, 0, 0], vec![1]));
        Ok(())
    }

    #[test]
    fn packed_cmyk_projection_covers_grouped_pixels_and_row_tails() -> Result<(), PilError> {
        let mut image = Image::new(7, 3, "CMYK", (0, 0, 0, 0))?;
        image.putpixel(1, 0, 1, 0, 0, 0)?;
        image.putpixel(4, 1, 0, 1, 0, 0)?;
        image.putpixel(6, 2, 0, 0, 1, 0)?;
        image.putpixel(2, 2, 0, 0, 0, 1)?;

        assert_eq!(
            image.getprojection()?,
            (vec![0, 1, 1, 0, 1, 0, 1], vec![1, 1, 1])
        );
        Ok(())
    }

    #[test]
    fn packed_cmyk_projection_preserves_empty_width_rows() -> Result<(), PilError> {
        let image = Image::new(0, 3, "CMYK", (0, 0, 0, 0))?;
        assert_eq!(image.getprojection()?, (vec![], vec![0, 0, 0]));
        Ok(())
    }

    #[test]
    fn pending_cmyk_projection_applies_overwrites_without_changing_image() -> Result<(), PilError> {
        let mut image = Image::new(7, 3, "CMYK", (1, 0, 0, 0))?;
        for x in 0..7 {
            image.putpixel(x, 2, 0, 0, 0, 0)?;
        }
        for y in 0..2 {
            image.putpixel(4, y, 0, 0, 0, 0)?;
        }
        image.putpixel(3, 0, 0, 0, 0, 0)?;
        image.putpixel(3, 0, 1, 0, 0, 0)?;

        assert_eq!(
            image.getprojection()?,
            (vec![1, 1, 1, 1, 0, 1, 1], vec![1, 1, 0])
        );

        let mut expected = Vec::with_capacity(7 * 3 * 4);
        for y in 0..3 {
            for x in 0..7 {
                expected.extend_from_slice(if y == 2 || x == 4 {
                    &[0, 0, 0, 0]
                } else {
                    &[1, 0, 0, 0]
                });
            }
        }
        assert_eq!(image.tobytes()?, expected);
        Ok(())
    }

    #[test]
    fn simd_pending_cmyk_projection_fuses_writes_and_reports_vector_execution()
    -> Result<(), PilError> {
        use crate::compute::Backend;

        let mut image = Image::new(65, 3, "CMYK", (0, 0, 0, 0))?;
        image.putpixel(1, 0, 1, 0, 0, 0)?;
        image.putpixel(1, 0, 0, 0, 0, 0)?;
        image.putpixel(64, 1, 0, 1, 0, 0)?;
        image.putpixel(3, 2, 0, 0, 0, 1)?;
        let image = image.use_backend(Backend::Simd);

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let projection = image.getprojection()?;
        let receipt = Backend::take_pipeline_telemetry();
        let operation_receipts = Backend::take_pipeline_operation_telemetry();
        Backend::set_pipeline_telemetry_enabled(previous);

        let mut expected_horizontal = vec![0; 65];
        expected_horizontal[3] = 1;
        expected_horizontal[64] = 1;
        assert_eq!(projection, (expected_horizontal, vec![0, 1, 1]));
        let receipt = receipt.expect("SIMD terminal receipt");
        assert_eq!(receipt.1, Backend::Simd);
        assert_eq!(receipt.2, 4);
        assert_eq!(receipt.7, None);
        assert_eq!(operation_receipts.len(), 4);
        assert!(operation_receipts[0].vector_block_count > 0);
        Ok(())
    }

    #[test]
    fn simd_known_zero_cmyk_projection_vectorizes_positive_writes_and_axis_tails()
    -> Result<(), PilError> {
        use crate::compute::Backend;

        let mut image = Image::new(65, 3, "CMYK", (0, 0, 0, 0))?;
        image.putpixel(1, 0, 1, 0, 0, 0)?;
        image.putpixel(1, 0, 0, 1, 0, 0)?;
        image.putpixel(64, 2, 0, 0, 0, 1)?;
        let image = image.use_backend(Backend::Simd);

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let projection = image.getprojection()?;
        let receipt = Backend::take_pipeline_telemetry();
        let operation_receipts = Backend::take_pipeline_operation_telemetry();
        Backend::set_pipeline_telemetry_enabled(previous);

        let mut expected_horizontal = vec![0; 65];
        expected_horizontal[1] = 1;
        expected_horizontal[64] = 1;
        let mut expected_vertical = vec![0; 3];
        expected_vertical[0] = 1;
        expected_vertical[2] = 1;
        assert_eq!(projection, (expected_horizontal, expected_vertical));
        let receipt = receipt.expect("SIMD terminal receipt");
        assert_eq!(receipt.1, Backend::Simd);
        assert_eq!(receipt.2, 3);
        assert_eq!(receipt.7, None);
        assert_eq!(operation_receipts.len(), 3);
        assert_eq!(operation_receipts[0].vector_block_count, 16);
        Ok(())
    }

    #[test]
    fn simd_known_zero_cmyk_projection_scans_when_write_limit_is_exceeded() -> Result<(), PilError>
    {
        use crate::compute::Backend;

        let mut image = Image::new(9, 1, "CMYK", (0, 0, 0, 0))?;
        for x in 0..9 {
            image.putpixel(x, 0, 0, 0, 1, 0)?;
        }
        let image = image.use_backend(Backend::Simd);

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let projection = image.getprojection()?;
        let receipt = Backend::take_pipeline_telemetry();
        let operation_receipts = Backend::take_pipeline_operation_telemetry();
        Backend::set_pipeline_telemetry_enabled(previous);

        assert_eq!(projection, (vec![1; 9], vec![1]));
        let receipt = receipt.expect("SIMD terminal receipt");
        assert_eq!(receipt.1, Backend::Simd);
        assert_eq!(receipt.2, 9);
        assert_eq!(receipt.7, None);
        assert_eq!(operation_receipts[0].vector_block_count, 2);
        Ok(())
    }

    #[cfg(feature = "gpu")]
    #[test]
    fn gpu_pending_cmyk_projection_applies_zero_overwrites_across_wide_row_boundaries()
    -> Result<(), PilError> {
        use crate::compute::Backend;

        let source = crate::raster::DynamicImage::ImageRgba8(crate::raster::RgbaImage::from_pixel(
            515,
            19,
            crate::raster::Rgba([0; 4]),
        ));
        let ops = [
            crate::pipeline::PipelineOp::PutPixel {
                x: 255,
                y: 16,
                color: (1, 0, 0, 0),
                palette_index: false,
            },
            crate::pipeline::PipelineOp::PutPixel {
                x: 255,
                y: 16,
                color: (0, 0, 0, 0),
                palette_index: false,
            },
            crate::pipeline::PipelineOp::PutPixel {
                x: 0,
                y: 0,
                color: (0, 0, 1, 0),
                palette_index: false,
            },
            crate::pipeline::PipelineOp::PutPixel {
                x: 0,
                y: 0,
                color: (0, 0, 0, 0),
                palette_index: false,
            },
            crate::pipeline::PipelineOp::PutPixel {
                x: 257,
                y: 7,
                color: (0, 1, 0, 0),
                palette_index: false,
            },
            crate::pipeline::PipelineOp::PutPixel {
                x: 514,
                y: 18,
                color: (0, 0, 0, 1),
                palette_index: false,
            },
        ];
        if !crate::compute::gpu_cmyk_projection_terminal_supported(&source, &ops) {
            return Ok(());
        }

        let mut image = Image::new(515, 19, "CMYK", (0, 0, 0, 0))?;
        image.putpixel(255, 16, 1, 0, 0, 0)?;
        image.putpixel(255, 16, 0, 0, 0, 0)?;
        image.putpixel(0, 0, 0, 0, 1, 0)?;
        image.putpixel(0, 0, 0, 0, 0, 0)?;
        image.putpixel(257, 7, 0, 1, 0, 0)?;
        image.putpixel(514, 18, 0, 0, 0, 1)?;
        image = image.use_backend(Backend::Gpu);

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let result = image.getprojection();
        let receipt = Backend::take_pipeline_telemetry();
        Backend::set_pipeline_telemetry_enabled(previous);

        let (horizontal, vertical) = result?;
        let mut expected_horizontal = vec![0; 515];
        expected_horizontal[257] = 1;
        expected_horizontal[514] = 1;
        let mut expected_vertical = vec![0; 19];
        expected_vertical[7] = 1;
        expected_vertical[18] = 1;
        assert_eq!(
            (horizontal, vertical),
            (expected_horizontal, expected_vertical)
        );

        let receipt = receipt.expect("GPU projection terminal receipt");
        assert_eq!(receipt.0, Some(Backend::Gpu));
        assert_eq!(receipt.1, Backend::Gpu);
        assert_eq!(receipt.2, ops.len());
        assert_eq!(receipt.6, Some(1));
        assert_eq!(receipt.7, None);
        let resources = receipt.8.expect("GPU projection resources");
        assert_eq!(resources.upload_bytes, 0);
        assert_eq!(resources.readback_bytes, 72);
        assert_eq!(resources.full_frame_copy_count, 0);
        Ok(())
    }

    #[cfg(feature = "gpu")]
    #[test]
    fn gpu_pending_cmyk_projection_applies_overwrites_to_nonzero_source() -> Result<(), PilError> {
        use crate::compute::Backend;

        let (width, height) = (515, 19);
        let mut bytes = vec![0; width as usize * height as usize * 4];
        let source_offset = (3 * width as usize + 100) * 4;
        bytes[source_offset] = 1;
        let source = crate::raster::DynamicImage::ImageRgba8(
            crate::raster::RgbaImage::from_raw(width, height, bytes.clone())
                .expect("source carrier dimensions"),
        );
        let ops = [
            crate::pipeline::PipelineOp::PutPixel {
                x: 100,
                y: 3,
                color: (0, 0, 0, 0),
                palette_index: false,
            },
            crate::pipeline::PipelineOp::PutPixel {
                x: 257,
                y: 7,
                color: (0, 1, 0, 0),
                palette_index: false,
            },
            crate::pipeline::PipelineOp::PutPixel {
                x: 514,
                y: 18,
                color: (0, 0, 0, 1),
                palette_index: false,
            },
        ];
        if !crate::compute::gpu_cmyk_projection_terminal_supported(&source, &ops) {
            return Ok(());
        }

        let mut image = Image::frombytes("CMYK", (width, height), &bytes)?;
        image.putpixel(100, 3, 0, 0, 0, 0)?;
        image.putpixel(257, 7, 0, 1, 0, 0)?;
        image.putpixel(514, 18, 0, 0, 0, 1)?;
        image = image.use_backend(Backend::Gpu);

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let result = image.getprojection();
        let receipt = Backend::take_pipeline_telemetry();
        Backend::set_pipeline_telemetry_enabled(previous);

        let (horizontal, vertical) = result?;
        let mut expected_horizontal = vec![0; width as usize];
        expected_horizontal[257] = 1;
        expected_horizontal[514] = 1;
        let mut expected_vertical = vec![0; height as usize];
        expected_vertical[7] = 1;
        expected_vertical[18] = 1;
        assert_eq!(
            (horizontal, vertical),
            (expected_horizontal, expected_vertical)
        );

        let receipt = receipt.expect("GPU projection terminal receipt");
        assert_eq!(receipt.0, Some(Backend::Gpu));
        assert_eq!(receipt.1, Backend::Gpu);
        assert_eq!(receipt.2, ops.len());
        assert_eq!(receipt.6, Some(1));
        assert_eq!(receipt.7, None);
        Ok(())
    }
}

#[cfg(test)]
mod getprojection_rgb_pipeline_tests {
    use super::{Image, PilError};

    #[test]
    fn cpu_zero_rgb_source_preserves_sparse_overwrite_order() -> Result<(), PilError> {
        use crate::compute::Backend;

        let mut image = Image::new(17, 4, "RGB", (0, 0, 0, 255))?;
        image.putpixel(0, 0, 1, 0, 0, 0)?;
        image.putpixel(16, 3, 0, 1, 0, 0)?;
        image.putpixel(0, 0, 0, 0, 0, 255)?;
        image = image.use_backend(Backend::Cpu);

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let projection = image.getprojection()?;
        let receipt = Backend::take_pipeline_telemetry();
        Backend::set_pipeline_telemetry_enabled(previous);

        let mut expected_horizontal = vec![0; 17];
        expected_horizontal[16] = 1;
        assert_eq!((expected_horizontal, vec![0, 0, 0, 1]), projection);
        let receipt = receipt.expect("CPU sparse RGB projection terminal receipt");
        assert_eq!(receipt.0, Some(Backend::Cpu));
        assert_eq!(receipt.1, Backend::Cpu);
        assert_eq!(receipt.2, 3);
        assert_eq!(receipt.7, None);
        assert_eq!(
            receipt
                .8
                .expect("CPU terminal resource receipt")
                .fused_operation_count,
            3
        );
        Ok(())
    }

    #[test]
    fn cpu_pending_rgb_projection_applies_overwrites_and_ignores_alpha() -> Result<(), PilError> {
        use crate::compute::Backend;

        let mut image = Image::new(7, 4, "RGB", (1, 0, 0, 255))?;
        for x in 0..7 {
            image.putpixel(x, 3, 0, 0, 0, 255)?;
        }
        for y in 0..3 {
            image.putpixel(4, y, 0, 0, 0, 255)?;
        }
        image.putpixel(3, 0, 0, 0, 0, 255)?;
        image.putpixel(3, 0, 0, 1, 0, 0)?;
        image.putpixel(6, 3, 0, 0, 0, 255)?;
        image = image.use_backend(Backend::Cpu);

        assert_eq!(
            image.getprojection()?,
            (vec![1, 1, 1, 1, 0, 1, 1], vec![1, 1, 1, 0])
        );
        Ok(())
    }

    #[test]
    fn simd_pending_rgb_projection_executes_vector_blocks() -> Result<(), PilError> {
        use crate::compute::Backend;

        let mut image = Image::new(17, 4, "RGB", (0, 0, 0, 255))?;
        image.putpixel(0, 0, 1, 0, 0, 0)?;
        image.putpixel(16, 3, 0, 1, 0, 0)?;
        image.putpixel(4, 2, 0, 0, 0, 255)?;
        image = image.use_backend(Backend::Simd);

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let projection = image.getprojection()?;
        let receipt = Backend::take_pipeline_telemetry();
        let operation_receipts = Backend::take_pipeline_operation_telemetry();
        Backend::set_pipeline_telemetry_enabled(previous);

        let mut expected_horizontal = vec![0; 17];
        expected_horizontal[0] = 1;
        expected_horizontal[16] = 1;
        assert_eq!(projection, (expected_horizontal, vec![1, 0, 0, 1]));
        let receipt = receipt.expect("SIMD RGB projection terminal receipt");
        assert_eq!(receipt.1, Backend::Simd);
        assert_eq!(receipt.2, 3);
        assert_eq!(receipt.7, None);
        assert_eq!(operation_receipts.len(), 3);
        assert!(operation_receipts[0].vector_block_count > 0);
        Ok(())
    }

    #[cfg(feature = "gpu")]
    #[test]
    fn gpu_pending_rgb_projection_executes_terminal_and_ignores_alpha() -> Result<(), PilError> {
        use crate::compute::Backend;

        let source = crate::raster::DynamicImage::ImageRgb8(crate::raster::RgbImage::from_pixel(
            17,
            4,
            crate::raster::Rgb([0; 3]),
        ));
        let ops = [
            crate::pipeline::PipelineOp::PutPixel {
                x: 0,
                y: 0,
                color: (1, 0, 0, 0),
                palette_index: false,
            },
            crate::pipeline::PipelineOp::PutPixel {
                x: 16,
                y: 3,
                color: (0, 1, 0, 0),
                palette_index: false,
            },
            crate::pipeline::PipelineOp::PutPixel {
                x: 4,
                y: 2,
                color: (0, 0, 0, 255),
                palette_index: false,
            },
        ];
        if !crate::compute::gpu_rgb_projection_terminal_supported(&source, &ops) {
            return Ok(());
        }

        let mut image = Image::new(17, 4, "RGB", (0, 0, 0, 255))?;
        image.putpixel(0, 0, 1, 0, 0, 0)?;
        image.putpixel(16, 3, 0, 1, 0, 0)?;
        image.putpixel(4, 2, 0, 0, 0, 255)?;
        image = image.use_backend(Backend::Gpu);

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let result = image.getprojection();
        let receipt = Backend::take_pipeline_telemetry();
        Backend::set_pipeline_telemetry_enabled(previous);

        let (horizontal, vertical) = result?;
        let mut expected_horizontal = vec![0; 17];
        expected_horizontal[0] = 1;
        expected_horizontal[16] = 1;
        assert_eq!(
            (horizontal, vertical),
            (expected_horizontal, vec![1, 0, 0, 1])
        );
        let receipt = receipt.expect("GPU RGB projection terminal receipt");
        assert_eq!(receipt.0, Some(Backend::Gpu));
        assert_eq!(receipt.1, Backend::Gpu);
        assert_eq!(receipt.2, ops.len());
        assert_eq!(receipt.6, Some(1));
        assert_eq!(receipt.7, None);
        Ok(())
    }
}

#[cfg(test)]
mod compatibility_info_tests {
    use super::{Image, ImageInfoValue, PilError};

    #[test]
    fn crop_after_pipeline_preserves_jpeg_info() -> Result<(), PilError> {
        let image = Image::open_bytes(
            include_bytes!("../tests/fixtures/assets/image/rgb-small.jpg").to_vec(),
        )?;
        let expected = image.compatibility_info();
        assert!(expected.iter().any(|(key, _)| key == "jfif"));
        let inverted = crate::ops::imageops::invert(&image)?;
        let cropped = inverted.crop(Some((0, 0, 2, 2)))?;
        assert_eq!(cropped.compatibility_info(), expected);
        assert!(cropped.source_format().is_none());
        Ok(())
    }

    #[test]
    fn webp_timing_defaults_appear_only_after_load() -> Result<(), PilError> {
        let mut image = Image::open_bytes(
            include_bytes!("../tests/fixtures/assets/image/rgb-small.webp").to_vec(),
        )?;
        let expected = [
            ("timestamp".to_owned(), ImageInfoValue::Integer(0)),
            ("duration".to_owned(), ImageInfoValue::Integer(0)),
        ];

        assert!(!image.is_materialized());
        let lazy_info = image.compatibility_info();
        assert!(
            lazy_info
                .iter()
                .all(|(key, _)| key != "timestamp" && key != "duration")
        );

        image.load()?;
        let loaded_info = image.compatibility_info();
        assert!(expected.iter().all(|field| loaded_info.contains(field)));
        Ok(())
    }
}

#[cfg(test)]
#[path = "image_getchannel_receipt_tests.rs"]
mod getchannel_receipt_tests;
