//! Pillow logical mode identities, independent of raster storage and codec types.

/// A Pillow-visible mode or an internal sample-mode identity.
///
/// This payload-free enum belongs to the Pillow image wrapper. Raster buffers
/// describe physical sample storage, while `image-slash-star::ImageMode`
/// describes decoded codec samples; neither identifies every logical Pillow
/// mode carried by an image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Pillow bilevel `1` mode, expanded to one byte per pixel.
    One,
    /// Pillow 8-bit luma (`L`) mode.
    ImageLuma8,
    /// Pillow 8-bit luma with alpha (`LA`) mode.
    ImageLumaA8,
    /// Pillow premultiplied luma with alpha (`La`) mode.
    ImageLaPremultiplied8,
    /// Pillow 8-bit palette-index (`P`) mode.
    ImagePalette8,
    /// Pillow palette-index with alpha (`PA`) mode.
    ImagePaletteAlpha8,
    /// Pillow 8-bit RGB mode.
    ImageRgb8,
    /// Pillow 8-bit RGBA mode.
    ImageRgba8,
    /// Pillow premultiplied RGBA (`RGBa`) mode.
    ImageRgbPremultiplied8,
    /// Pillow 8-bit RGB with an unused padding byte (`RGBX`) mode.
    ImageRgbx8,
    /// Pillow 8-bit CMYK mode, stored in four raw bytes per pixel.
    ImageCmyk8,
    /// Pillow 8-bit YCbCr mode.
    ImageYCbCr8,
    /// Pillow 8-bit LAB mode.
    ImageLab8,
    /// Pillow 8-bit HSV mode.
    ImageHsv8,
    /// Signed 32-bit integer (`I`) samples in the existing raw carrier.
    ImageI32,
    /// 32-bit float (`F`) samples in the existing raw carrier.
    ImageF32,
    /// Native-endian unsigned 16-bit (`I;16`) mode.
    ImageI16,
    /// Little-endian unsigned 16-bit (`I;16L`) mode.
    ImageI16L,
    /// Big-endian unsigned 16-bit (`I;16B`) mode.
    ImageI16B,
    /// Native-order unsigned 16-bit (`I;16N`) mode.
    ImageI16N,
    /// Internal 16-bit luma-alpha storage identity.
    ImageLumaA16,
    /// Internal 16-bit RGB storage identity.
    ImageRgb16,
    /// Internal 16-bit RGBA storage identity.
    ImageRgba16,
    /// Internal 32-bit float RGB storage identity.
    ImageRgb32F,
    /// Internal 32-bit float RGBA storage identity.
    ImageRgba32F,
}

impl Mode {
    /// Selects the wrapper-owned mode once from explicit Pillow metadata or
    /// the native raster storage at the image construction boundary.
    pub(crate) fn for_image(image: &crate::raster::DynamicImage, explicit: Option<&str>) -> Self {
        if let Some(mode) = explicit.and_then(Self::from_pillow_name) {
            return mode;
        }
        match image {
            crate::raster::DynamicImage::ImageLuma8(_) => Self::ImageLuma8,
            crate::raster::DynamicImage::ImageLumaA8(_) => Self::ImageLumaA8,
            crate::raster::DynamicImage::ImageRgb8(_) => Self::ImageRgb8,
            crate::raster::DynamicImage::ImageRgba8(_) => Self::ImageRgba8,
            crate::raster::DynamicImage::ImageLuma16(_) => Self::ImageI16,
            crate::raster::DynamicImage::ImageLumaA16(_) => Self::ImageLumaA16,
            crate::raster::DynamicImage::ImageRgb16(_) => Self::ImageRgb16,
            crate::raster::DynamicImage::ImageRgba16(_) => Self::ImageRgba16,
            crate::raster::DynamicImage::ImageRgb32F(_) => Self::ImageRgb32F,
            crate::raster::DynamicImage::ImageRgba32F(_) => Self::ImageRgba32F,
        }
    }

    /// Parses a Pillow mode spelling at an API boundary.
    ///
    /// Internal storage identities have no Pillow spelling and therefore
    /// cannot be produced by this parser.
    pub fn from_pillow_name(name: &str) -> Option<Self> {
        match name {
            "1" => Some(Self::One),
            "L" => Some(Self::ImageLuma8),
            "LA" => Some(Self::ImageLumaA8),
            "La" => Some(Self::ImageLaPremultiplied8),
            "P" => Some(Self::ImagePalette8),
            "PA" => Some(Self::ImagePaletteAlpha8),
            "RGB" => Some(Self::ImageRgb8),
            "RGBA" => Some(Self::ImageRgba8),
            "RGBa" => Some(Self::ImageRgbPremultiplied8),
            "RGBX" => Some(Self::ImageRgbx8),
            "CMYK" => Some(Self::ImageCmyk8),
            "YCbCr" => Some(Self::ImageYCbCr8),
            "LAB" => Some(Self::ImageLab8),
            "HSV" => Some(Self::ImageHsv8),
            "I" => Some(Self::ImageI32),
            "F" => Some(Self::ImageF32),
            "I;16" => Some(Self::ImageI16),
            "I;16L" => Some(Self::ImageI16L),
            "I;16B" => Some(Self::ImageI16B),
            "I;16N" => Some(Self::ImageI16N),
            _ => None,
        }
    }

    /// Returns the Pillow spelling for public modes, or `None` for internal
    /// storage identities that cannot be represented as Pillow mode strings.
    pub const fn pillow_name(self) -> Option<&'static str> {
        match self {
            Self::One => Some("1"),
            Self::ImageLuma8 => Some("L"),
            Self::ImageLumaA8 => Some("LA"),
            Self::ImageLaPremultiplied8 => Some("La"),
            Self::ImagePalette8 => Some("P"),
            Self::ImagePaletteAlpha8 => Some("PA"),
            Self::ImageRgb8 => Some("RGB"),
            Self::ImageRgba8 => Some("RGBA"),
            Self::ImageRgbPremultiplied8 => Some("RGBa"),
            Self::ImageRgbx8 => Some("RGBX"),
            Self::ImageCmyk8 => Some("CMYK"),
            Self::ImageYCbCr8 => Some("YCbCr"),
            Self::ImageLab8 => Some("LAB"),
            Self::ImageHsv8 => Some("HSV"),
            Self::ImageI32 => Some("I"),
            Self::ImageF32 => Some("F"),
            Self::ImageI16 => Some("I;16"),
            Self::ImageI16L => Some("I;16L"),
            Self::ImageI16B => Some("I;16B"),
            Self::ImageI16N => Some("I;16N"),
            Self::ImageLumaA16
            | Self::ImageRgb16
            | Self::ImageRgba16
            | Self::ImageRgb32F
            | Self::ImageRgba32F => None,
        }
    }

    /// Returns the old storage-facing spelling used by unmigrated backend
    /// callbacks. Internal identities deliberately map to their carrier's
    /// spelling only at this compatibility boundary; mode-aware CPU families
    /// must use `self` directly.
    pub(crate) fn compatibility_name(self) -> &'static str {
        match self {
            Self::One => "1",
            Self::ImageLuma8 => "L",
            Self::ImageLumaA8 | Self::ImageLumaA16 => "LA",
            Self::ImageLaPremultiplied8 => "La",
            Self::ImagePalette8 => "P",
            Self::ImagePaletteAlpha8 => "PA",
            Self::ImageRgb8 => "RGB",
            Self::ImageRgba8 => "RGBA",
            Self::ImageRgbPremultiplied8 => "RGBa",
            Self::ImageRgbx8 => "RGBX",
            Self::ImageCmyk8 => "CMYK",
            Self::ImageYCbCr8 => "YCbCr",
            Self::ImageLab8 => "LAB",
            Self::ImageHsv8 => "HSV",
            Self::ImageI32 => "I",
            Self::ImageF32 => "F",
            Self::ImageI16 => "I;16",
            Self::ImageI16L => "I;16L",
            Self::ImageI16B => "I;16B",
            Self::ImageI16N => "I;16N",
            Self::ImageRgb16 | Self::ImageRgb32F => "RGB",
            Self::ImageRgba16 | Self::ImageRgba32F => "RGBA",
        }
    }
}
