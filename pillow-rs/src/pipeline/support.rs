//! Supporting enums and state for pipeline operation descriptors.

/// Resampling filter used by resize-like operations.
#[derive(Debug, Clone, Copy)]
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
#[derive(Debug, Clone)]
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

/// Pillow image mode used by pipeline operations.
///
/// This enum carries mode intent through the pipeline before conversion to a
/// concrete image-buffer representation.
#[derive(Debug, Clone)]
pub enum ColorMode {
    /// 8-bit luma.
    L,
    /// 8-bit luma plus alpha.
    LA,
    /// 8-bit red, green, and blue.
    RGB,
    /// 8-bit red, green, blue, and alpha.
    RGBA,
    /// Cyan, magenta, yellow, and key/black.
    CMYK,
    /// Luma and chroma channels.
    YCbCr,
    /// Hue, saturation, and value.
    HSV,
    /// 32-bit signed integer pixels.
    I,
    /// 32-bit floating-point pixels.
    F,
    /// Palette-indexed pixels.
    P,
    /// Pillow mode `"1"` binary pixels.
    Mode1,
}

/// Logical Pillow sample layout for mutating pixel operations.
///
/// `DynamicImage` cannot distinguish raw `P`, `CMYK`, `I`, or `F` samples
/// from the standard buffers used to store them. Mutating operations carry
/// this mode alongside their bytes so every compute backend interprets the
/// same samples without relying on output-mode metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum PixelMode {
    /// 8-bit luma.
    L = 0,
    /// 8-bit luma plus alpha.
    LA = 1,
    /// 8-bit red, green, and blue.
    RGB = 2,
    /// 8-bit red, green, blue, and alpha.
    RGBA = 3,
    /// Palette indices.
    P = 4,
    /// Palette indices plus per-pixel alpha.
    PA = 5,
    /// Cyan, magenta, yellow, and key/black.
    CMYK = 6,
    /// Pillow mode `"1"` binary pixels.
    Mode1 = 7,
    /// Luma and chroma channels.
    YCbCr = 8,
    /// Hue, saturation, and value.
    HSV = 9,
    /// 32-bit signed integer pixels.
    I = 10,
    /// 32-bit floating-point pixels.
    F = 11,
}

impl PixelMode {
    /// Parses a Pillow mode name used by the core image API.
    pub(crate) fn from_name(mode: &str) -> Option<Self> {
        match mode {
            "1" => Some(Self::Mode1),
            "L" => Some(Self::L),
            "LA" => Some(Self::LA),
            "P" => Some(Self::P),
            "PA" => Some(Self::PA),
            "RGB" => Some(Self::RGB),
            "RGBA" | "RGBa" | "RGBX" => Some(Self::RGBA),
            "CMYK" => Some(Self::CMYK),
            "YCbCr" => Some(Self::YCbCr),
            "HSV" => Some(Self::HSV),
            "I" => Some(Self::I),
            "F" => Some(Self::F),
            _ => None,
        }
    }

    /// Number of raw bytes per pixel used by `putdata`.
    pub(crate) const fn channels(self) -> usize {
        match self {
            Self::L | Self::P | Self::Mode1 => 1,
            Self::LA | Self::PA => 2,
            Self::RGB | Self::YCbCr | Self::HSV => 3,
            Self::RGBA | Self::CMYK | Self::I | Self::F => 4,
        }
    }

    /// Stable code shared with scalar and WGSL kernels.
    #[cfg(feature = "gpu")]
    pub(crate) const fn code(self) -> u32 {
        self as u32
    }
}

/// Dither algorithm used by conversion or quantization operations.
#[derive(Debug, Clone)]
pub enum DitherMethod {
    /// No dithering.
    None,
    /// Floyd-Steinberg error diffusion.
    FloydSteinberg,
}
