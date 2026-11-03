//! Typed identities for operations with their support declarations.

use super::OperationSupport;
use crate::compute::pool_cpu::ops as cpu_ops;

/// Identity of an operation with its own strategy and parameter contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperationId {
    /// Draw one Pillow primitive directly into the mode-bearing raster.
    Draw,
    /// A crop with coordinates normalized by the image API.
    Crop,
    /// Remove an equal border from all edges, allowing valid empty output.
    CropBorder,
    /// Flip image top-to-bottom while preserving its exact mode.
    Flip,
    /// Mirror image left-to-right while preserving its exact mode.
    Mirror,
    /// Apply one Pillow transpose method while preserving its exact mode.
    Transpose,
    /// Resize an image to an exact output size.
    Resize,
    /// Resize an image from a floating source region to an exact output size.
    ResizeBoxed,
    /// Resize an image to fit inside target bounds while preserving aspect ratio.
    Contain,
    /// Resize an image to cover target bounds while preserving aspect ratio.
    Cover,
    /// Resize and crop an image to target bounds with typed bleed and centering.
    Fit,
    /// Resize an image into a target canvas while preserving aspect ratio.
    Pad,
    /// Scale an image by a floating-point factor.
    Scale,
    /// Apply Pillow's reducing-gap thumbnail algorithm while preserving mode.
    Thumbnail,
    /// Rotate by an arbitrary angle with a typed mode and interpolation strategy.
    Rotate,
    /// Apply an affine, perspective, quadrilateral, or mesh transform.
    Transform,
    /// Reduce native sample blocks while preserving the declared output mode.
    Reduce,
    /// Pillow's source-mode conversion to an eight-bit luma image.
    Grayscale,
    /// Apply one byte lookup table per native byte band, preserving exact mode.
    Eval,
    /// Apply a callable affine point transform to native I/F/I;16 samples.
    PointScalar,
    /// Apply a floating lookup table to a byte sample and produce native F samples.
    PointFloat,
    /// Invert every byte of an image while preserving its exact mode.
    Invert,
    /// Validate ImageOps inversion before queueing its prepared Invert child.
    ImageOpsInvert,
    /// Reduce the stored high bits of native L or RGB samples.
    Posterize,
    /// Invert native L or RGB samples at or above a threshold.
    Solarize,
    /// Map grayscale samples through three color lookup tables.
    Colorize,
    /// Stretch L/RGB contrast from per-channel histogram percentiles.
    Autocontrast,
    /// Apply a native 3x3 Pillow convolution kernel.
    Filter3x3,
    /// Apply a native 5x5 Pillow convolution kernel.
    Filter5x5,
    /// Apply Pillow's Gaussian blur.
    GaussianBlur,
    /// Apply Pillow's native-byte box blur.
    BoxBlur,
    /// Sharpen byte samples by blending the source with its Gaussian blur.
    UnsharpMask,
    /// Apply Pillow's median rank filter.
    MedianFilter,
    /// Apply Pillow's maximum rank filter.
    MaxFilter,
    /// Apply Pillow's minimum rank filter.
    MinFilter,
    /// Select one sample from each Pillow rank-filter neighborhood.
    RankFilter,
    /// Replace each native byte channel with its local most-frequent sample.
    ModeFilter,
    /// Equalize L/RGB samples directly and P samples after palette expansion.
    Equalize,
    /// Add a native-mode border while preserving the exact source identity.
    Expand,
    /// Internal band extraction; the public wrapper validates its band selector first.
    ExtractBand,
    /// Apply a 3D color lookup table with a typed target identity.
    Color3DLut,
    /// Convert exact RGB8 samples to Pillow's packed LAB representation.
    ConvertLab,
    /// Convert one exact source mode to a typed target mode.
    Convert,
    /// Apply Pillow's RGB conversion matrix to one exact source mode.
    ConvertMatrix,
    /// Remap native L/P samples into a P index plane with reordered palette metadata.
    RemapPalette,
    /// Relocate native pixels using Pillow's process-global random stream.
    EffectSpread,
    /// Generate Pillow's L-mode Gaussian noise using the process-global stream.
    EffectNoise,
    /// Construct a fixed 256 by 256 typed linear-gradient raster.
    LinearGradient,
    /// Construct a fixed 256 by 256 typed radial-gradient raster.
    RadialGradient,
    /// Construct a typed native-L Mandelbrot raster.
    EffectMandelbrot,
    /// Alpha-composite one same-sized LA or RGBA image over another.
    AlphaComposite,
    /// Merge native single-band images into one exact target mode.
    Merge,
    /// Blend two compatible Pillow byte-mode images.
    BlendModule,
    /// Composite a source image over a destination canvas through a mask.
    CompositeModule,
    /// Paste a normalized source into a native destination mode.
    Paste,
    /// One normalized native-byte pixel write with caller-validated coordinates.
    PutPixel,
    /// An ordered run of adjacent native-byte writes that clones its source once.
    PutPixelBatch,
    /// Write one normalized numeric I/F/I;16 pixel synchronously into its native carrier.
    PutPixelValueAt,
    /// Replace a normalized byte prefix, retaining the direct payload-layout contract.
    PutData,
    /// Replace I;16 samples from the Pillow byte-row fast path.
    PutDataL16Bytes,
    /// Write one normalized putdata value synchronously before the next value is coerced.
    PutDataValueAt,
    /// Insert or replace a constant alpha sample using the exact source identity.
    PutAlpha,
    /// Replace per-pixel alpha from a mode-bearing secondary image.
    PutAlphaData,
    /// Return the exclusive bounds of non-zero samples in a materialized image.
    GetBBox,
    /// Compute Pillow's 256-bin histogram for each logical image band.
    Histogram,
    /// Compute Shannon entropy from the image's Pillow-compatible histogram.
    Entropy,
    /// Count unique native pixel values while preserving each mode's color shape.
    GetColors,
    /// Reduce a typed source image to a palette-index image.
    Quantize,
    /// Compute per-band statistics from the exact mode's histogram behavior.
    Stat,
    /// Return native per-band minimum and maximum samples.
    GetExtrema,
    /// Return per-band extrema projected to the Rust/JS byte compatibility API.
    GetExtremaBytes,
    /// Split an image into mode-preserving single-band images.
    Split,
    /// Return horizontal and vertical non-zero sample projections.
    GetProjection,
    /// Read the complete stored image into a host-owned byte sequence.
    GetData,
    /// Read one logical band into an allocated host-owned byte sequence.
    GetDataBand,
    /// Fill a caller-owned byte buffer with one logical band.
    GetDataBandInto,
    /// Return one pixel with Pillow's exact-mode scalar or component shape.
    GetPixel,
    /// Return one pixel projected to the Rust/JavaScript RGBA8 convenience shape.
    GetPixelRgba,
    /// Extract one logical channel through the image wrapper's typed route.
    GetChannel,
    /// Blend native Pillow samples with the same-mode black brightness base.
    Brightness,
    /// Blend native samples with Pillow's contrast-degenerate base.
    Contrast,
    /// Blend native byte pixels with their exact-mode color saturation base.
    ColorSaturation,
    /// Blend native samples with the exact-mode 3x3 smoothing result.
    Sharpness,
    /// Add native bytes from a compatible secondary raster with scale and offset.
    ChopsAdd,
    /// Subtract native bytes from a compatible secondary raster with scale and offset.
    ChopsSubtract,
    /// Multiply compatible native-byte samples.
    ChopsMultiply,
    /// Apply the native-byte screen blend operation.
    ChopsScreen,
    /// Select the darker compatible native-byte sample.
    ChopsDarker,
    /// Select the lighter compatible native-byte sample.
    ChopsLighter,
    /// Compute absolute differences between compatible native-byte samples.
    ChopsDifference,
    /// Apply the native-byte overlay lookup table.
    ChopsOverlay,
    /// Apply the native-byte hard-light lookup table.
    ChopsHardLight,
    /// Apply the native-byte soft-light lookup table.
    ChopsSoftLight,
    /// Add compatible native-byte samples modulo 256.
    ChopsAddModulo,
    /// Subtract compatible native-byte samples modulo 256.
    ChopsSubtractModulo,
    /// Apply Pillow mode-1 logical AND.
    ChopsLogicalAnd,
    /// Apply Pillow mode-1 logical OR.
    ChopsLogicalOr,
    /// Apply Pillow mode-1 logical XOR.
    ChopsLogicalXor,
    /// Construct a single-band luma image filled with a constant byte.
    ChopsConstant,
    /// Offset native pixels with wraparound while preserving exact mode.
    ChopsOffset,
    /// Clone the source while preserving exact mode.
    ChopsDuplicate,
    /// Invert every stored byte while preserving exact mode.
    ChopsInvert,
}

impl OperationId {
    /// Returns the contract owned by this operation's execution entry point.
    #[must_use]
    pub fn support(self) -> &'static OperationSupport {
        match self {
            Self::Draw
            | Self::Crop
            | Self::CropBorder
            | Self::Flip
            | Self::Mirror
            | Self::Transpose
            | Self::Resize
            | Self::ResizeBoxed
            | Self::Contain
            | Self::Cover
            | Self::Fit
            | Self::Pad
            | Self::Scale
            | Self::Thumbnail
            | Self::Rotate
            | Self::Transform
            | Self::Reduce => geometry_support(self),
            Self::Filter3x3
            | Self::Filter5x5
            | Self::GaussianBlur
            | Self::BoxBlur
            | Self::UnsharpMask
            | Self::MedianFilter
            | Self::MaxFilter
            | Self::MinFilter
            | Self::RankFilter
            | Self::ModeFilter => filter_support(self),
            Self::Grayscale
            | Self::Eval
            | Self::PointScalar
            | Self::PointFloat
            | Self::Invert
            | Self::ImageOpsInvert
            | Self::Posterize
            | Self::Solarize
            | Self::Colorize
            | Self::Autocontrast
            | Self::Equalize
            | Self::Quantize
            | Self::Expand
            | Self::ExtractBand
            | Self::Color3DLut => imageops_support(self),
            Self::ConvertLab | Self::Convert | Self::ConvertMatrix | Self::RemapPalette => {
                color_support(self)
            }
            Self::EffectSpread
            | Self::EffectNoise
            | Self::LinearGradient
            | Self::RadialGradient
            | Self::EffectMandelbrot => effects_support(self),
            Self::AlphaComposite | Self::Merge | Self::BlendModule | Self::CompositeModule => {
                composite_support(self)
            }
            Self::Histogram => &cpu_ops::analysis::ops::HISTOGRAM_SUPPORT,
            Self::Entropy => &cpu_ops::analysis::ops::ENTROPY_SUPPORT,
            Self::GetColors => &cpu_ops::analysis::ops::GET_COLORS_SUPPORT,
            Self::GetPixel => &cpu_ops::analysis::ops::GET_PIXEL_SUPPORT,
            Self::GetPixelRgba => &cpu_ops::analysis::ops::GET_PIXEL_RGBA_SUPPORT,
            Self::Stat => &cpu_ops::analysis::ops::STAT_SUPPORT,
            Self::Split => &cpu_ops::analysis::ops::SPLIT_SUPPORT,
            Self::Paste
            | Self::PutPixel
            | Self::PutPixelBatch
            | Self::PutPixelValueAt
            | Self::PutData
            | Self::PutDataL16Bytes
            | Self::PutDataValueAt
            | Self::PutAlpha
            | Self::PutAlphaData
            | Self::GetBBox
            | Self::GetExtrema
            | Self::GetExtremaBytes
            | Self::GetProjection
            | Self::GetData
            | Self::GetDataBand
            | Self::GetDataBandInto
            | Self::GetChannel => mutation_support(self),
            Self::Brightness | Self::Contrast | Self::ColorSaturation | Self::Sharpness => {
                enhance_support(self)
            }
            Self::ChopsAdd
            | Self::ChopsSubtract
            | Self::ChopsMultiply
            | Self::ChopsScreen
            | Self::ChopsDarker
            | Self::ChopsLighter
            | Self::ChopsDifference
            | Self::ChopsOverlay
            | Self::ChopsHardLight
            | Self::ChopsSoftLight
            | Self::ChopsAddModulo
            | Self::ChopsSubtractModulo
            | Self::ChopsLogicalAnd
            | Self::ChopsLogicalOr
            | Self::ChopsLogicalXor
            | Self::ChopsConstant
            | Self::ChopsOffset
            | Self::ChopsDuplicate
            | Self::ChopsInvert => chops_support(self),
        }
    }
}

fn geometry_support(id: OperationId) -> &'static OperationSupport {
    match id {
        OperationId::Draw => &cpu_ops::draw::ops::SUPPORT,
        OperationId::Crop => &cpu_ops::crop::ops::SUPPORT,
        OperationId::CropBorder => &cpu_ops::imageops::ops::CROP_BORDER_SUPPORT,
        OperationId::Flip => &cpu_ops::transpose::ops::FLIP_SUPPORT,
        OperationId::Mirror => &cpu_ops::transpose::ops::MIRROR_SUPPORT,
        OperationId::Transpose => &cpu_ops::transpose::ops::TRANSPOSE_SUPPORT,
        OperationId::Resize => &cpu_ops::resize::ops::SUPPORT,
        OperationId::ResizeBoxed => &cpu_ops::resize::ops::BOXED_SUPPORT,
        OperationId::Contain => &cpu_ops::geometry::ops::CONTAIN_SUPPORT,
        OperationId::Cover => &cpu_ops::geometry::ops::COVER_SUPPORT,
        OperationId::Fit => &cpu_ops::geometry::ops::FIT_SUPPORT,
        OperationId::Pad => &cpu_ops::geometry::pad::ops::SUPPORT,
        OperationId::Scale => &cpu_ops::geometry::ops::SCALE_SUPPORT,
        OperationId::Thumbnail => &cpu_ops::thumbnail::ops::SUPPORT,
        OperationId::Rotate => &cpu_ops::rotate::ops::SUPPORT,
        OperationId::Transform => &cpu_ops::transform::ops::SUPPORT,
        OperationId::Reduce => &cpu_ops::reduce::ops::SUPPORT,
        _ => unreachable!("non-geometry operation sent to geometry support"),
    }
}

fn filter_support(id: OperationId) -> &'static OperationSupport {
    match id {
        OperationId::Filter3x3 => &cpu_ops::filter::ops::FILTER3X3_SUPPORT,
        OperationId::Filter5x5 => &cpu_ops::filter::ops::FILTER5X5_SUPPORT,
        OperationId::GaussianBlur => &cpu_ops::filter::ops::GAUSSIAN_BLUR_SUPPORT,
        OperationId::BoxBlur => &cpu_ops::filter::ops::BOX_BLUR_SUPPORT,
        OperationId::UnsharpMask => &cpu_ops::filter::ops::UNSHARP_MASK_SUPPORT,
        OperationId::MedianFilter => &cpu_ops::filter::ops::MEDIAN_FILTER_SUPPORT,
        OperationId::MaxFilter => &cpu_ops::filter::ops::MAX_FILTER_SUPPORT,
        OperationId::MinFilter => &cpu_ops::filter::ops::MIN_FILTER_SUPPORT,
        OperationId::RankFilter => &cpu_ops::filter::ops::RANK_FILTER_SUPPORT,
        OperationId::ModeFilter => &cpu_ops::filter::ops::MODE_FILTER_SUPPORT,
        _ => unreachable!("non-filter operation sent to filter support"),
    }
}

fn imageops_support(id: OperationId) -> &'static OperationSupport {
    match id {
        OperationId::Grayscale => &cpu_ops::grayscale::ops::SUPPORT,
        OperationId::Eval => &cpu_ops::eval::ops::SUPPORT,
        OperationId::PointScalar => &cpu_ops::eval::ops::SCALAR_POINT_SUPPORT,
        OperationId::PointFloat => &cpu_ops::eval::ops::POINT_FLOAT_SUPPORT,
        OperationId::Invert => &cpu_ops::imageops::ops::INVERT_SUPPORT,
        OperationId::ImageOpsInvert => &cpu_ops::imageops::ops::IMAGEOPS_INVERT_SUPPORT,
        OperationId::Posterize => &cpu_ops::imageops::ops::POSTERIZE_SUPPORT,
        OperationId::Solarize => &cpu_ops::imageops::ops::SOLARIZE_SUPPORT,
        OperationId::Colorize => &cpu_ops::imageops::ops::COLORIZE_SUPPORT,
        OperationId::Autocontrast => &cpu_ops::imageops::ops::AUTOCONTRAST_SUPPORT,
        OperationId::Equalize => &cpu_ops::imageops::ops::EQUALIZE_SUPPORT,
        OperationId::Quantize => &cpu_ops::imageops::ops::QUANTIZE_SUPPORT,
        OperationId::Expand => &cpu_ops::imageops::ops::EXPAND_SUPPORT,
        OperationId::ExtractBand => &cpu_ops::put::ops::EXTRACT_BAND_SUPPORT,
        OperationId::Color3DLut => &cpu_ops::color3dlut::ops::SUPPORT,
        _ => unreachable!("non-image operation sent to imageops support"),
    }
}

fn color_support(id: OperationId) -> &'static OperationSupport {
    match id {
        OperationId::ConvertLab => &cpu_ops::color::ops::SUPPORT,
        OperationId::Convert => &cpu_ops::color::convert::ops::SUPPORT,
        OperationId::ConvertMatrix => &cpu_ops::color::convert::ops::MATRIX_SUPPORT,
        OperationId::RemapPalette => &cpu_ops::color::ops::REMAP_PALETTE_SUPPORT,
        _ => unreachable!("non-color operation sent to color support"),
    }
}

fn effects_support(id: OperationId) -> &'static OperationSupport {
    match id {
        OperationId::EffectSpread => &cpu_ops::effects::ops::EFFECT_SPREAD_SUPPORT,
        OperationId::EffectNoise => &cpu_ops::effects::ops::EFFECT_NOISE_SUPPORT,
        OperationId::LinearGradient => &cpu_ops::effects::ops::LINEAR_GRADIENT_SUPPORT,
        OperationId::RadialGradient => &cpu_ops::effects::ops::RADIAL_GRADIENT_SUPPORT,
        OperationId::EffectMandelbrot => &cpu_ops::effects::ops::MANDELBROT_SUPPORT,
        _ => unreachable!("non-effect operation sent to effects support"),
    }
}

fn composite_support(id: OperationId) -> &'static OperationSupport {
    match id {
        OperationId::AlphaComposite => &cpu_ops::composite::ops::ALPHA_COMPOSITE_SUPPORT,
        OperationId::Merge => &cpu_ops::composite::ops::MERGE_SUPPORT,
        OperationId::BlendModule => &cpu_ops::composite::ops::BLEND_MODULE_SUPPORT,
        OperationId::CompositeModule => &cpu_ops::composite::ops::COMPOSITE_MODULE_SUPPORT,
        _ => unreachable!("non-composite operation sent to composite support"),
    }
}

fn mutation_support(id: OperationId) -> &'static OperationSupport {
    match id {
        OperationId::Paste => &cpu_ops::paste::ops::SUPPORT,
        OperationId::PutPixel => &cpu_ops::put::ops::PUT_PIXEL_SUPPORT,
        OperationId::PutPixelBatch => &cpu_ops::put::ops::PUT_PIXEL_BATCH_SUPPORT,
        OperationId::PutPixelValueAt => &cpu_ops::put::ops::PUT_PIXEL_VALUE_AT_SUPPORT,
        OperationId::PutData => &cpu_ops::put::ops::PUT_DATA_SUPPORT,
        OperationId::PutDataL16Bytes => &cpu_ops::put::ops::PUT_DATA_L16_BYTES_SUPPORT,
        OperationId::PutDataValueAt => &cpu_ops::put::ops::PUT_DATA_VALUE_AT_SUPPORT,
        OperationId::PutAlpha => &cpu_ops::put::ops::PUT_ALPHA_SUPPORT,
        OperationId::PutAlphaData => &cpu_ops::put::ops::PUT_ALPHA_DATA_SUPPORT,
        OperationId::GetBBox => &cpu_ops::analysis::ops::GET_BBOX_SUPPORT,
        OperationId::GetExtrema => &cpu_ops::analysis::ops::GET_EXTREMA_SUPPORT,
        OperationId::GetExtremaBytes => &cpu_ops::analysis::ops::GET_EXTREMA_BYTES_SUPPORT,
        OperationId::GetProjection => &cpu_ops::analysis::ops::SUPPORT,
        OperationId::GetData => &cpu_ops::analysis::ops::GET_DATA_SUPPORT,
        OperationId::GetDataBand => &cpu_ops::analysis::ops::GET_DATA_BAND_SUPPORT,
        OperationId::GetDataBandInto => &cpu_ops::analysis::ops::GET_DATA_BAND_INTO_SUPPORT,
        OperationId::GetChannel => &cpu_ops::analysis::ops::GET_CHANNEL_SUPPORT,
        _ => unreachable!("non-mutation operation sent to mutation support"),
    }
}

fn enhance_support(id: OperationId) -> &'static OperationSupport {
    match id {
        OperationId::Brightness => &cpu_ops::enhance::ops::SUPPORT,
        OperationId::Contrast => &cpu_ops::enhance::ops::CONTRAST_SUPPORT,
        OperationId::ColorSaturation => &cpu_ops::enhance::ops::COLOR_SATURATION_SUPPORT,
        OperationId::Sharpness => &cpu_ops::enhance::ops::SHARPNESS_SUPPORT,
        _ => unreachable!("non-enhancement operation sent to enhance support"),
    }
}

fn chops_support(id: OperationId) -> &'static OperationSupport {
    match id {
        OperationId::ChopsAdd => &cpu_ops::chops::ops::ADD_SUPPORT,
        OperationId::ChopsSubtract => &cpu_ops::chops::ops::SUBTRACT_SUPPORT,
        OperationId::ChopsMultiply => &cpu_ops::chops::ops::MULTIPLY_SUPPORT,
        OperationId::ChopsScreen => &cpu_ops::chops::ops::SCREEN_SUPPORT,
        OperationId::ChopsDarker => &cpu_ops::chops::ops::DARKER_SUPPORT,
        OperationId::ChopsLighter => &cpu_ops::chops::ops::LIGHTER_SUPPORT,
        OperationId::ChopsDifference => &cpu_ops::chops::ops::DIFFERENCE_SUPPORT,
        OperationId::ChopsOverlay => &cpu_ops::chops::ops::OVERLAY_SUPPORT,
        OperationId::ChopsHardLight => &cpu_ops::chops::ops::HARD_LIGHT_SUPPORT,
        OperationId::ChopsSoftLight => &cpu_ops::chops::ops::SOFT_LIGHT_SUPPORT,
        OperationId::ChopsAddModulo => &cpu_ops::chops::ops::ADD_MODULO_SUPPORT,
        OperationId::ChopsSubtractModulo => &cpu_ops::chops::ops::SUBTRACT_MODULO_SUPPORT,
        OperationId::ChopsLogicalAnd => &cpu_ops::chops::ops::LOGICAL_AND_SUPPORT,
        OperationId::ChopsLogicalOr => &cpu_ops::chops::ops::LOGICAL_OR_SUPPORT,
        OperationId::ChopsLogicalXor => &cpu_ops::chops::ops::LOGICAL_XOR_SUPPORT,
        OperationId::ChopsConstant => &cpu_ops::chops::ops::CONSTANT_SUPPORT,
        OperationId::ChopsOffset => &cpu_ops::chops::ops::OFFSET_SUPPORT,
        OperationId::ChopsDuplicate => &cpu_ops::chops::ops::DUPLICATE_SUPPORT,
        OperationId::ChopsInvert => &cpu_ops::chops::ops::INVERT_SUPPORT,
        _ => unreachable!("non-Chops operation sent to Chops support"),
    }
}
