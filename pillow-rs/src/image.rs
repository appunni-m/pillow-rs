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
use std::sync::{Arc, OnceLock};

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::pipeline::{ColorMode, PipelineOp, PixelMode, TransposeMethod};
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
mod pixel_access;
mod projection;
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
