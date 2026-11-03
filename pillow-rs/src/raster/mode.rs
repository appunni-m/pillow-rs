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
    /// Every public and internal raster identity.
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

    /// Compatibility alias for Pillow's `L` mode.
    pub const L: Self = Self::ImageLuma8;
    /// Compatibility alias for Pillow's `LA` mode.
    pub const LA: Self = Self::ImageLumaA8;
    /// Compatibility alias for Pillow's premultiplied `La` mode.
    #[allow(
        non_upper_case_globals,
        reason = "Preserve the established Rust alias spelling for Pillow's La mode."
    )]
    pub const LaPremultiplied: Self = Self::ImageLaPremultiplied8;
    /// Compatibility alias for Pillow's `P` mode.
    pub const P: Self = Self::ImagePalette8;
    /// Compatibility alias for Pillow's bilevel `1` mode.
    #[allow(
        non_upper_case_globals,
        reason = "Preserve the established Rust alias spelling for Pillow's 1 mode."
    )]
    pub const Mode1: Self = Self::One;
    /// Compatibility alias for Pillow's `RGB` mode.
    pub const RGB: Self = Self::ImageRgb8;
    /// Compatibility alias for Pillow's `RGBA` mode.
    pub const RGBA: Self = Self::ImageRgba8;
    /// Compatibility alias for Pillow's premultiplied `RGBa` mode.
    #[allow(
        non_upper_case_globals,
        reason = "Preserve the established Rust alias spelling for Pillow's RGBa mode."
    )]
    pub const RgbPremultiplied: Self = Self::ImageRgbPremultiplied8;
    /// Compatibility alias for Pillow's padded `RGBX` mode.
    #[allow(
        non_upper_case_globals,
        reason = "Preserve the established Rust alias spelling for Pillow's RGBX mode."
    )]
    pub const Rgbx: Self = Self::ImageRgbx8;
    /// Compatibility alias for Pillow's `CMYK` mode.
    pub const CMYK: Self = Self::ImageCmyk8;
    /// Compatibility alias for Pillow's `YCbCr` mode.
    #[allow(
        non_upper_case_globals,
        reason = "Preserve the established Rust alias spelling for Pillow's YCbCr mode."
    )]
    pub const YCbCr: Self = Self::ImageYCbCr8;
    /// Compatibility alias for Pillow's `HSV` mode.
    pub const HSV: Self = Self::ImageHsv8;
    /// Compatibility alias for Pillow's `I` mode.
    pub const I: Self = Self::ImageI32;
    /// Compatibility alias for Pillow's `F` mode.
    pub const F: Self = Self::ImageF32;
    /// Compatibility alias for Pillow's native-endian `I;16` mode.
    pub const I16: Self = Self::ImageI16;

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

    /// Returns the Pillow spelling for modes with a Pillow representation.
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
            Self::ImageLumaA16
            | Self::ImageRgb16
            | Self::ImageRgba16
            | Self::ImageRgb32F
            | Self::ImageRgba32F => None,
        }
    }

    /// Returns the number of sample bands represented by this identity.
    #[must_use]
    pub const fn band_count(self) -> usize {
        match self {
            Self::One
            | Self::ImageLuma8
            | Self::ImagePalette8
            | Self::ImageI32
            | Self::ImageF32
            | Self::ImageI16
            | Self::ImageI16L
            | Self::ImageI16B
            | Self::ImageI16N => 1,
            Self::ImageLumaA8
            | Self::ImageLaPremultiplied8
            | Self::ImagePaletteAlpha8
            | Self::ImageLumaA16 => 2,
            Self::ImageRgb8
            | Self::ImageYCbCr8
            | Self::ImageLab8
            | Self::ImageHsv8
            | Self::ImageRgb16
            | Self::ImageRgb32F => 3,
            Self::ImageRgba8
            | Self::ImageRgbPremultiplied8
            | Self::ImageRgbx8
            | Self::ImageCmyk8
            | Self::ImageRgba16
            | Self::ImageRgba32F => 4,
        }
    }

    /// Returns Pillow's physical bytes per pixel for modes with a byte layout.
    #[must_use]
    pub(crate) const fn pillow_pixel_size(self) -> Option<usize> {
        match self {
            Self::One | Self::ImageLuma8 | Self::ImagePalette8 => Some(1),
            Self::ImageI16 | Self::ImageI16L | Self::ImageI16B | Self::ImageI16N => Some(2),
            Self::ImageLumaA8
            | Self::ImageLaPremultiplied8
            | Self::ImagePaletteAlpha8
            | Self::ImageRgb8
            | Self::ImageRgba8
            | Self::ImageRgbPremultiplied8
            | Self::ImageRgbx8
            | Self::ImageCmyk8
            | Self::ImageYCbCr8
            | Self::ImageLab8
            | Self::ImageHsv8
            | Self::ImageI32
            | Self::ImageF32 => Some(4),
            Self::ImageLumaA16
            | Self::ImageRgb16
            | Self::ImageRgba16
            | Self::ImageRgb32F
            | Self::ImageRgba32F => None,
        }
    }

    /// Returns native byte-carrier bytes per pixel where raw byte strategies apply.
    #[must_use]
    pub(crate) const fn byte_carrier_pixel_width(self) -> Option<usize> {
        match self {
            Self::One | Self::ImageLuma8 | Self::ImagePalette8 => Some(1),
            Self::ImageLumaA8 | Self::ImageLaPremultiplied8 | Self::ImagePaletteAlpha8 => {
                Some(2)
            }
            Self::ImageRgb8 | Self::ImageYCbCr8 | Self::ImageLab8 | Self::ImageHsv8 => Some(3),
            Self::ImageRgba8
            | Self::ImageRgbPremultiplied8
            | Self::ImageRgbx8
            | Self::ImageCmyk8
            | Self::ImageI32
            | Self::ImageF32 => Some(4),
            Self::ImageI16
            | Self::ImageI16L
            | Self::ImageI16B
            | Self::ImageI16N
            | Self::ImageLumaA16
            | Self::ImageRgb16
            | Self::ImageRgba16
            | Self::ImageRgb32F
            | Self::ImageRgba32F => None,
        }
    }

    /// Returns the number of Pillow physical byte lanes omitted by the carrier.
    #[must_use]
    pub(crate) const fn hidden_pixel_lane_count(self) -> Option<usize> {
        match self {
            Self::ImageLumaA8 | Self::ImageLaPremultiplied8 | Self::ImagePaletteAlpha8 => Some(2),
            Self::ImageRgb8 | Self::ImageYCbCr8 | Self::ImageLab8 | Self::ImageHsv8 => Some(1),
            Self::One
            | Self::ImageLuma8
            | Self::ImagePalette8
            | Self::ImageRgba8
            | Self::ImageRgbPremultiplied8
            | Self::ImageRgbx8
            | Self::ImageCmyk8
            | Self::ImageI32
            | Self::ImageF32
            | Self::ImageI16
            | Self::ImageI16L
            | Self::ImageI16B
            | Self::ImageI16N
            | Self::ImageLumaA16
            | Self::ImageRgb16
            | Self::ImageRgba16
            | Self::ImageRgb32F
            | Self::ImageRgba32F => None,
        }
    }

    /// Returns the physical byte offset of one omitted lane in the Pillow pixel.
    #[must_use]
    pub(crate) const fn hidden_pixel_lane_offset(self, lane: usize) -> Option<usize> {
        match (self.hidden_pixel_lane_count(), lane) {
            (Some(2), 0) => Some(1),
            (Some(2), 1) => Some(2),
            (Some(1), 0) => Some(3),
            _ => None,
        }
    }

    /// Returns a byte-carrier byte's position in the Pillow physical pixel.
    #[must_use]
    pub(crate) const fn byte_carrier_physical_offset(self, byte: usize) -> Option<usize> {
        match (self.byte_carrier_pixel_width(), self.hidden_pixel_lane_count(), byte) {
            (Some(1), None, 0) => Some(0),
            (Some(2), Some(2), 0) => Some(0),
            (Some(2), Some(2), 1) => Some(3),
            (Some(3), Some(1), 0..=2) => Some(byte),
            (Some(4), None, 0..=3) => Some(byte),
            _ => None,
        }
    }

    /// Returns Pillow's public band count, or zero for internal-only modes.
    #[must_use]
    pub const fn pillow_band_count(self) -> usize {
        match self.pillow_name() {
            Some(_) => self.band_count(),
            None => 0,
        }
    }

    /// Returns the index of an uppercase `A` band, if present.
    ///
    /// This describes the band name, not alpha semantics: the `A` band in Lab
    /// is a color axis, while the `A` band in LA/RGBA is alpha.
    #[must_use]
    pub const fn uppercase_a_band_index(self) -> Option<usize> {
        match self {
            Self::ImageLumaA8 | Self::ImagePaletteAlpha8 | Self::ImageLab8 | Self::ImageLumaA16 => {
                Some(1)
            }
            Self::ImageRgba8 | Self::ImageRgba16 | Self::ImageRgba32F => Some(3),
            Self::One
            | Self::ImageLuma8
            | Self::ImageLaPremultiplied8
            | Self::ImagePalette8
            | Self::ImageRgb8
            | Self::ImageRgbPremultiplied8
            | Self::ImageRgbx8
            | Self::ImageCmyk8
            | Self::ImageYCbCr8
            | Self::ImageHsv8
            | Self::ImageI32
            | Self::ImageF32
            | Self::ImageI16
            | Self::ImageI16L
            | Self::ImageI16B
            | Self::ImageI16N
            | Self::ImageRgb16
            | Self::ImageRgb32F => None,
        }
    }

    /// Whether this identity includes alpha, including premultiplied alpha.
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
