//! Supporting enums and state for pipeline operation descriptors.

use crate::compute::registry::support::{OperationId, OperationSupport};
use crate::pipeline::PipelineOp;

/// Resampling filter used by resize-like operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResampleFilter {
    /// Nearest-neighbor sampling.
    Nearest,
    /// Bilinear interpolation.
    Bilinear,
    /// Bicubic interpolation.
    Bicubic,
    /// Lanczos-windowed sinc interpolation.
    Lanczos,
    /// Box filter sampling.
    Box,
    /// Hamming-windowed sampling.
    Hamming,
}

/// Pillow transpose operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransposeMethod {
    /// Mirror left-to-right.
    FlipLeftRight,
    /// Mirror top-to-bottom.
    FlipTopBottom,
    /// Rotate 90 degrees counter-clockwise.
    Rotate90,
    /// Rotate 180 degrees.
    Rotate180,
    /// Rotate 270 degrees counter-clockwise.
    Rotate270,
    /// Transpose across the top-left to bottom-right diagonal.
    Transpose,
    /// Transpose across the top-right to bottom-left diagonal.
    Transverse,
}

/// A composed transpose run, including its exact identity element.
///
/// Keeping identity explicit lets a cancelled prefix compose with later
/// methods. Signed axis permutations are independent of image dimensions,
/// including empty images and images with a one-pixel axis.
#[derive(Debug, Clone)]
pub(crate) enum TransposeTransform {
    Identity,
    Method(TransposeMethod),
}

impl TransposeTransform {
    fn method_bits(method: &TransposeMethod) -> u8 {
        // Low bits reverse the output x/y axes; bit 2 swaps the source axes.
        match method {
            TransposeMethod::FlipLeftRight => 1,
            TransposeMethod::FlipTopBottom => 2,
            TransposeMethod::Rotate180 => 3,
            TransposeMethod::Transpose => 4,
            TransposeMethod::Rotate270 => 5,
            TransposeMethod::Rotate90 => 6,
            TransposeMethod::Transverse => 7,
        }
    }

    /// Append a method in public execution order.
    pub(crate) fn then(&self, next: &TransposeMethod) -> Self {
        let first = match self {
            Self::Identity => 0,
            Self::Method(method) => Self::method_bits(method),
        };
        let next = Self::method_bits(next);
        // If the second method swaps axes, it also swaps the first method's
        // reversal flags. Reversing an axis twice cancels by XOR.
        let first = if next & 4 != 0 {
            (first & 4) | ((first & 1) << 1) | ((first & 2) >> 1)
        } else {
            first
        };
        match first ^ next {
            0 => Self::Identity,
            1 => Self::Method(TransposeMethod::FlipLeftRight),
            2 => Self::Method(TransposeMethod::FlipTopBottom),
            3 => Self::Method(TransposeMethod::Rotate180),
            4 => Self::Method(TransposeMethod::Transpose),
            5 => Self::Method(TransposeMethod::Rotate270),
            6 => Self::Method(TransposeMethod::Rotate90),
            7 => Self::Method(TransposeMethod::Transverse),
            _ => unreachable!("transpose composition uses only three bits"),
        }
    }
}

/// Coordinate transform algorithm.
#[derive(Debug, Clone)]
pub enum TransformMethod {
    /// Affine transform with six coefficients.
    Affine,
    /// Perspective transform with eight coefficients.
    Perspective,
    /// Quadrilateral mapping.
    Quad,
    /// Mesh transform made of local quadrilateral mappings.
    Mesh,
}

/// Compatibility name for the canonical Pillow-owned operation mode.
///
/// Pipeline targets now carry the same exact identity as materialized
/// [`crate::raster::DynamicImage`] values. Historical associated names remain
/// available on `raster::Mode` while callers migrate to its explicit variants.
pub use crate::raster::Mode as ColorMode;

/// Dither algorithm used by conversion or quantization operations.
#[derive(Debug, Clone)]
pub enum DitherMethod {
    /// No dithering.
    None,
    /// Floyd-Steinberg error diffusion.
    FloydSteinberg,
}

impl PipelineOp {
    /// Returns the typed operation contract for this pipeline descriptor.
    ///
    /// Parameterized forms that own different support contracts resolve to
    /// their exact operation identity before callers inspect mode support.
    #[must_use]
    pub fn operation_id(&self) -> OperationId {
        use OperationId as Id;

        match self {
            Self::Resize { .. } => Id::Resize,
            Self::ResizeBoxed { .. } => Id::ResizeBoxed,
            Self::Crop { .. } => Id::Crop,
            Self::Rotate { .. } => Id::Rotate,
            Self::Transpose { .. } => Id::Transpose,
            Self::Thumbnail { .. } => Id::Thumbnail,
            Self::Reduce { .. } => Id::Reduce,
            Self::Convert {
                matrix: Some(_), ..
            } => Id::ConvertMatrix,
            Self::Convert { matrix: None, .. } => Id::Convert,
            Self::ConvertLab { .. } => Id::ConvertLab,
            Self::RemapPalette { .. } => Id::RemapPalette,
            Self::Filter3x3 { .. } => Id::Filter3x3,
            Self::Filter5x5 { .. } => Id::Filter5x5,
            Self::GaussianBlur { .. } => Id::GaussianBlur,
            Self::BoxBlur { .. } => Id::BoxBlur,
            Self::BoxBlurXY { .. } => Id::BoxBlurXY,
            Self::MedianFilter { .. } => Id::MedianFilter,
            Self::MaxFilter { .. } => Id::MaxFilter,
            Self::MinFilter { .. } => Id::MinFilter,
            Self::RankFilter { .. } => Id::RankFilter,
            Self::Autocontrast { .. } => Id::Autocontrast,
            Self::Equalize | Self::EqualizeMasked { .. } => Id::Equalize,
            Self::Invert => Id::Invert,
            Self::Flip => Id::Flip,
            Self::Mirror => Id::Mirror,
            Self::Posterize { .. } => Id::Posterize,
            Self::Solarize { .. } => Id::Solarize,
            Self::Grayscale => Id::Grayscale,
            Self::Colorize { .. } => Id::Colorize,
            Self::Contain { .. } => Id::Contain,
            Self::Cover { .. } => Id::Cover,
            Self::Fit { .. } => Id::Fit,
            Self::Pad { .. } | Self::PadTyped { .. } | Self::PadPositioned { .. } => Id::Pad,
            Self::Scale { .. } => Id::Scale,
            Self::Expand { .. } | Self::ExpandTyped { .. } => Id::Expand,
            Self::CropBorder { .. } => Id::CropBorder,
            Self::Add { .. } => Id::ChopsAdd,
            Self::Subtract { .. } => Id::ChopsSubtract,
            Self::Multiply { .. } => Id::ChopsMultiply,
            Self::Screen { .. } => Id::ChopsScreen,
            Self::Darker { .. } => Id::ChopsDarker,
            Self::Lighter { .. } => Id::ChopsLighter,
            Self::Difference { .. } => Id::ChopsDifference,
            Self::Overlay { .. } => Id::ChopsOverlay,
            Self::HardLight { .. } => Id::ChopsHardLight,
            Self::SoftLight { .. } => Id::ChopsSoftLight,
            Self::AddModulo { .. } => Id::ChopsAddModulo,
            Self::SubtractModulo { .. } => Id::ChopsSubtractModulo,
            Self::LogicalAnd { .. } => Id::ChopsLogicalAnd,
            Self::LogicalOr { .. } => Id::ChopsLogicalOr,
            Self::LogicalXor { .. } => Id::ChopsLogicalXor,
            Self::Constant { .. } => Id::ChopsConstant,
            Self::Offset { .. } => Id::ChopsOffset,
            Self::Duplicate => Id::ChopsDuplicate,
            Self::InvertChops => Id::ChopsInvert,
            Self::Brightness { .. } => Id::Brightness,
            Self::Contrast { .. } => Id::Contrast,
            Self::ColorSaturation { .. } => Id::ColorSaturation,
            Self::Sharpness { .. } => Id::Sharpness,
            Self::EffectSpread { .. } => Id::EffectSpread,
            Self::Paste { .. } => Id::Paste,
            Self::AlphaComposite { .. } => Id::AlphaComposite,
            Self::Merge { .. } => Id::Merge,
            Self::BlendModule { .. } => Id::BlendModule,
            Self::CompositeModule { .. } => Id::CompositeModule,
            Self::Eval { .. } => Id::Eval,
            Self::EffectNoise { .. } => Id::EffectNoise,
            Self::Transform { .. } => Id::Transform,
            Self::Color3DLut { .. } => Id::Color3DLut,
            Self::PutPixel { .. } => Id::PutPixel,
            Self::PutData { .. } => Id::PutData,
            Self::PutAlpha { .. } => Id::PutAlpha,
            Self::PutAlphaData { .. } => Id::PutAlphaData,
            Self::ExtractBand { .. } => Id::ExtractBand,
            Self::DrawLine { .. }
            | Self::DrawRectangle { .. }
            | Self::DrawRoundedRect { .. }
            | Self::DrawEllipse { .. }
            | Self::DrawCircle { .. }
            | Self::DrawPolygon { .. }
            | Self::DrawArc { .. }
            | Self::DrawChord { .. }
            | Self::DrawPieslice { .. }
            | Self::DrawPoint { .. } => Id::Draw,
        }
    }

    /// Returns the exhaustive support descriptor used by preparation.
    #[must_use]
    pub fn support(&self) -> &'static OperationSupport {
        self.operation_id().support()
    }
}
