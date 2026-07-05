//! Compact pixel-format metadata for compute backends.
//!
//! [`PixelFormat`] converts Pillow/core mode information into the fixed
//! `L`/`LA`/`RGB`/`RGBA` family supported by CPU, SIMD, and GPU dispatch.
//! Non-standard Pillow modes must be converted before reaching compute code.

// ============================================================================
// AS PER DESIGN — DO NOT REMOVE:
//   PixelFormat replaces all bare integer mode codes (0=L, 1=LA, 2=RGB, 3=RGBA)
//   that were scattered across 80+ locations in SIMD and CPU backend code.
//
//   The `#[repr(u8)]` encoding IS the wire format for GPU/SIMD backends.
//   If you change the discriminants, you MUST update every WGSL shader that
//   reads `mode_code` and every SIMD adapter that branches on format.
//
//   CI enforces: no bare `mode >= 2` / `mode == 1 || mode == 3` — use
//   PixelFormat methods instead (see scripts/check_magic_numbers.sh).
// ============================================================================

use crate::pipeline::ColorMode;

/// Pixel format encoding for backend compute dispatch.
///
/// AS PER DESIGN — DO NOT REMOVE:
/// Discriminants are the ABI between CPU code and GPU/SIMD backends.
/// `L = 0, LA = 1, RGB = 2, RGBA = 3` is fixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PixelFormat {
    /// One 8-bit luma channel per pixel.
    L = 0,
    /// 8-bit luma plus 8-bit alpha per pixel.
    LA = 1,
    /// Three 8-bit red, green, and blue channels per pixel.
    RGB = 2,
    /// Three 8-bit color channels plus 8-bit alpha per pixel.
    RGBA = 3,
}

impl PixelFormat {
    /// Returns the number of color channels, excluding alpha.
    ///
    /// `L` and `LA` have one color channel. `RGB` and `RGBA` have three.
    #[inline]
    pub fn color_channels(self) -> u8 {
        match self {
            Self::L | Self::LA => 1,
            Self::RGB | Self::RGBA => 3,
        }
    }

    /// Returns the total number of stored channels per pixel.
    ///
    /// This is the byte multiplier used by tightly packed `u8` buffers:
    /// `L=1`, `LA=2`, `RGB=3`, and `RGBA=4`.
    #[inline]
    pub fn channels(self) -> u8 {
        match self {
            Self::L => 1,
            Self::LA => 2,
            Self::RGB => 3,
            Self::RGBA => 4,
        }
    }

    /// Returns whether this format stores alpha.
    #[inline]
    pub fn has_alpha(self) -> bool {
        matches!(self, Self::LA | Self::RGBA)
    }

    /// Returns the tightly packed row stride for width `w`.
    ///
    /// The result is `w * PixelFormat::channels()`.
    #[inline]
    pub fn row_stride(self, w: u32) -> usize {
        w as usize * self.channels() as usize
    }

    /// Returns the ABI value passed to GPU shaders.
    ///
    /// This must match `mode_code` constants in WGSL shader parameter blocks.
    #[inline]
    pub fn gpu_encoding(self) -> u32 {
        self as u32
    }

    /// Returns the ABI value passed to SIMD adapters.
    ///
    /// This must match the mode branching used by
    /// `compute/pool_simd/ops/adapters.rs`.
    #[inline]
    pub fn simd_encoding(self) -> u32 {
        self as u32
    }
}

// AS PER DESIGN — DO NOT REMOVE:
// Conversion from ColorMode. Non-standard modes (CMYK, YCbCr, HSV, I, F, P, Mode1)
// must be converted to a standard pixel format BEFORE reaching the compute backend.
// Those conversions happen in `ops/convert.rs`.
impl TryFrom<&ColorMode> for PixelFormat {
    type Error = ();

    fn try_from(mode: &ColorMode) -> Result<Self, Self::Error> {
        match mode {
            ColorMode::L => Ok(Self::L),
            ColorMode::LA => Ok(Self::LA),
            ColorMode::RGB => Ok(Self::RGB),
            ColorMode::RGBA => Ok(Self::RGBA),
            _ => Err(()), // Non-standard modes: convert first, then try again
        }
    }
}

impl TryFrom<ColorMode> for PixelFormat {
    type Error = ();

    fn try_from(mode: ColorMode) -> Result<Self, Self::Error> {
        Self::try_from(&mode)
    }
}

// AS PER DESIGN — DO NOT REMOVE: Tests validate encoding stability.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discriminants_are_stable() {
        // AS PER DESIGN: If these fail, WGSL shaders and SIMD code MUST be updated.
        assert_eq!(PixelFormat::L as u8, 0);
        assert_eq!(PixelFormat::LA as u8, 1);
        assert_eq!(PixelFormat::RGB as u8, 2);
        assert_eq!(PixelFormat::RGBA as u8, 3);
    }

    #[test]
    fn channel_counts() {
        assert_eq!(PixelFormat::L.channels(), 1);
        assert_eq!(PixelFormat::LA.channels(), 2);
        assert_eq!(PixelFormat::RGB.channels(), 3);
        assert_eq!(PixelFormat::RGBA.channels(), 4);
    }

    #[test]
    fn alpha_detection() {
        assert!(!PixelFormat::L.has_alpha());
        assert!(PixelFormat::LA.has_alpha());
        assert!(!PixelFormat::RGB.has_alpha());
        assert!(PixelFormat::RGBA.has_alpha());
    }

    #[test]
    fn from_color_mode() {
        assert_eq!(PixelFormat::try_from(ColorMode::L).unwrap(), PixelFormat::L);
        assert_eq!(
            PixelFormat::try_from(ColorMode::RGBA).unwrap(),
            PixelFormat::RGBA
        );
    }

    #[test]
    fn nonstandard_modes_rejected() {
        assert!(PixelFormat::try_from(ColorMode::CMYK).is_err());
        assert!(PixelFormat::try_from(ColorMode::P).is_err());
    }
}
