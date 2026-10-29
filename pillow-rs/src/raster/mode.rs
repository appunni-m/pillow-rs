//! Pillow-owned raster identities used by typed operation contracts.

/// Exact identity of a Pillow-owned raster, independently of its pixel layout.
///
/// [`super::DynamicImage`] stores this identity in its own variant tag. This
/// payload-free selector is used for construction, conversion, lazy output
/// metadata, and operation support; it is not stored beside materialized pixels.
/// Host adapters own conversion to and from Pillow's string spellings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Mode {
    /// Pillow's bilevel `1` mode, expanded to one byte per pixel.
    One,
    /// Pillow's 8-bit luma (`L`) mode.
    ImageLuma8,
    /// Pillow's 8-bit luma with alpha (`LA`) mode.
    ImageLumaA8,
    /// Pillow's premultiplied luma with alpha (`La`) mode.
    ImageLaPremultiplied8,
    /// Pillow's 8-bit palette-index (`P`) mode.
    ImagePalette8,
    /// Pillow's palette-index with alpha (`PA`) mode.
    ImagePaletteAlpha8,
    /// Pillow's 8-bit RGB mode.
    ImageRgb8,
    /// Pillow's 8-bit RGBA mode.
    ImageRgba8,
    /// Pillow's premultiplied RGBA (`RGBa`) mode.
    ImageRgbPremultiplied8,
    /// Pillow's 8-bit RGB with an unused padding byte (`RGBX`) mode.
    ImageRgbx8,
    /// Pillow's 8-bit CMYK mode, stored in four raw bytes per pixel.
    ImageCmyk8,
    /// Pillow's 8-bit YCbCr mode.
    ImageYCbCr8,
    /// Pillow's 8-bit LAB mode.
    ImageLab8,
    /// Pillow's 8-bit HSV mode.
    ImageHsv8,
    /// Pillow's signed 32-bit integer (`I`) samples in the existing raw carrier.
    ImageI32,
    /// Pillow's 32-bit float (`F`) samples in the existing raw carrier.
    ImageF32,
    /// Pillow's native-endian unsigned 16-bit (`I;16`) mode.
    ImageI16,
    /// Pillow's little-endian unsigned 16-bit (`I;16L`) mode.
    ImageI16L,
    /// Pillow's big-endian unsigned 16-bit (`I;16B`) mode.
    ImageI16B,
    /// Pillow's native-order unsigned 16-bit (`I;16N`) mode.
    ImageI16N,
    /// Internal 16-bit luma-alpha storage, not a Pillow mode identity.
    ImageLumaA16,
    /// Internal 16-bit RGB storage, not a Pillow mode identity.
    ImageRgb16,
    /// Internal 16-bit RGBA storage, not a Pillow mode identity.
    ImageRgba16,
    /// Internal 32-bit float RGB storage, not a Pillow mode identity.
    ImageRgb32F,
    /// Internal 32-bit float RGBA storage, not a Pillow mode identity.
    ImageRgba32F,
}

impl Mode {
    /// Every raster identity, including internal high-depth representations.
    pub const ALL: [Self; 25] = [
        Self::One,
        Self::ImageLuma8,
        Self::ImageLumaA8,
        Self::ImageLaPremultiplied8,
        Self::ImagePalette8,
        Self::ImagePaletteAlpha8,
        Self::ImageRgb8,
        Self::ImageRgba8,
        Self::ImageRgbPremultiplied8,
        Self::ImageRgbx8,
        Self::ImageCmyk8,
        Self::ImageYCbCr8,
        Self::ImageLab8,
        Self::ImageHsv8,
        Self::ImageI32,
        Self::ImageF32,
        Self::ImageI16,
        Self::ImageI16L,
        Self::ImageI16B,
        Self::ImageI16N,
        Self::ImageLumaA16,
        Self::ImageRgb16,
        Self::ImageRgba16,
        Self::ImageRgb32F,
        Self::ImageRgba32F,
    ];

    /// Parses a Pillow mode spelling at an external API boundary.
    ///
    /// Internal storage identities have no Pillow spelling and therefore
    /// cannot be constructed from this input. Core operation APIs should take
    /// the resulting enum directly instead of accepting mode strings.
    #[must_use]
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

    /// Returns the Pillow spelling for an externally representable mode.
    #[must_use]
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
            Self::ImageLumaA16 | Self::ImageRgb16 | Self::ImageRgba16
            | Self::ImageRgb32F | Self::ImageRgba32F => None,
        }
    }

    /// Whether this exact identity includes alpha, including premultiplied alpha.
    #[must_use]
    pub const fn has_alpha(self) -> bool {
        matches!(
            self,
            Self::ImageLumaA8
                | Self::ImageLaPremultiplied8
                | Self::ImagePaletteAlpha8
                | Self::ImageRgba8
                | Self::ImageRgbPremultiplied8
                | Self::ImageLumaA16
                | Self::ImageRgba16
                | Self::ImageRgba32F
        )
    }
}
