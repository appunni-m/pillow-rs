//! Pillow-owned pixel structs and extended color metadata.

use std::ops::{Index, IndexMut};

/// Unpacked pixel storage layout used by a raster image.
///
/// This describes storage, not Pillow mode identity. Use [`crate::raster::Mode`]
/// when an operation needs to select behavior by exact image mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ColorType {
    /// Eight-bit luminance.
    L8,
    /// Eight-bit luminance and alpha.
    La8,
    /// Eight-bit red, green, and blue.
    Rgb8,
    /// Eight-bit red, green, blue, and alpha.
    Rgba8,
    /// Eight-bit cyan, magenta, yellow, and black.
    Cmyk8,
    /// Sixteen-bit luminance.
    L16,
    /// Sixteen-bit luminance and alpha.
    La16,
    /// Sixteen-bit red, green, and blue.
    Rgb16,
    /// Sixteen-bit red, green, blue, and alpha.
    Rgba16,
    /// Thirty-two-bit floating-point red, green, and blue.
    Rgb32F,
    /// Thirty-two-bit floating-point red, green, blue, and alpha.
    Rgba32F,
    /// Thirty-two-bit floating-point luminance.
    L32F,
    /// Thirty-two-bit integer luminance.
    L32I,
}

impl ColorType {
    /// Number of bytes in one unpacked pixel.
    #[must_use]
    pub const fn bytes_per_pixel(self) -> u8 {
        match self {
            Self::L8 => 1,
            Self::L16 | Self::La8 => 2,
            Self::Rgb8 => 3,
            Self::Rgba8 | Self::Cmyk8 | Self::La16 | Self::L32F | Self::L32I => 4,
            Self::Rgb16 => 6,
            Self::Rgba16 => 8,
            Self::Rgb32F => 12,
            Self::Rgba32F => 16,
        }
    }

    /// Number of channels in one unpacked pixel.
    #[must_use]
    pub const fn channel_count(self) -> u8 {
        match self {
            Self::L8 | Self::L16 | Self::L32F | Self::L32I => 1,
            Self::La8 | Self::La16 => 2,
            Self::Rgb8 | Self::Rgb16 | Self::Rgb32F => 3,
            Self::Rgba8 | Self::Cmyk8 | Self::Rgba16 | Self::Rgba32F => 4,
        }
    }

    /// Number of bits in one unpacked pixel.
    #[must_use]
    pub const fn bits_per_pixel(self) -> u16 {
        self.bytes_per_pixel() as u16 * 8
    }

    /// Whether this storage layout contains alpha.
    #[must_use]
    pub const fn has_alpha(self) -> bool {
        matches!(
            self,
            Self::La8 | Self::Rgba8 | Self::La16 | Self::Rgba16 | Self::Rgba32F
        )
    }

    /// Whether this storage layout contains color rather than only luminance.
    #[must_use]
    pub const fn has_color(self) -> bool {
        matches!(
            self,
            Self::Rgb8
                | Self::Rgba8
                | Self::Cmyk8
                | Self::Rgb16
                | Self::Rgba16
                | Self::Rgb32F
                | Self::Rgba32F
        )
    }
}

/// A Pillow processing color layout, including packed and sub-byte variants.
#[derive(Copy, PartialEq, Eq, Debug, Clone, Hash)]
#[non_exhaustive]
pub enum ExtendedColorType {
    /// Pixel is 8-bit alpha
    A8,
    /// Pixel is 1-bit luminance
    L1,
    /// Pixel is 1-bit luminance with an alpha channel
    La1,
    /// Pixel contains 1-bit R, G and B channels
    Rgb1,
    /// Pixel is 1-bit RGB with an alpha channel
    Rgba1,
    /// Pixel is 2-bit luminance
    L2,
    /// Pixel is 2-bit luminance with an alpha channel
    La2,
    /// Pixel contains 2-bit R, G and B channels
    Rgb2,
    /// Pixel is 2-bit RGB with an alpha channel
    Rgba2,
    /// Pixel is 4-bit luminance
    L4,
    /// Pixel is 4-bit luminance with an alpha channel
    La4,
    /// Pixel contains 4-bit R, G and B channels
    Rgb4,
    /// Pixel is 4-bit RGB with an alpha channel
    Rgba4,
    /// Pixel contains 5-bit R, G and B channels packed into 2 bytes
    Rgb5x1,
    /// Pixel is 8-bit luminance
    L8,
    /// Pixel is 8-bit luminance with an alpha channel
    La8,
    /// Pixel contains 8-bit R, G and B channels
    Rgb8,
    /// Pixel is 8-bit RGB with an alpha channel
    Rgba8,
    /// Pixel is 16-bit luminance
    L16,
    /// Pixel is 16-bit luminance with an alpha channel
    La16,
    /// Pixel contains 16-bit R, G and B channels
    Rgb16,
    /// Pixel is 16-bit RGB with an alpha channel
    Rgba16,
    /// Pixel contains 8-bit B, G and R channels
    Bgr8,
    /// Pixel is 8-bit BGR with an alpha channel
    Bgra8,
    /// Pixel is 32-bit float RGB
    Rgb32F,
    /// Pixel is 32-bit float RGBA
    Rgba32F,
    /// Pixel is 32-bit float luminance
    L32F,
    /// Pixel is 32-bit integer luminance
    L32I,
    /// Pixel is 8-bit CMYK
    Cmyk8,
    /// Pixel is 16-bit CMYK
    Cmyk16,
    /// Pixel is of unknown color type with the specified bits per pixel.
    Unknown(u8),
}

impl ExtendedColorType {
    /// Get the number of channels for colors of this type.
    #[must_use]
    pub fn channel_count(self) -> u8 {
        match self {
            ExtendedColorType::A8
            | ExtendedColorType::L1
            | ExtendedColorType::L2
            | ExtendedColorType::L4
            | ExtendedColorType::L8
            | ExtendedColorType::L16
            | ExtendedColorType::L32F
            | ExtendedColorType::L32I
            | ExtendedColorType::Unknown(_) => 1,
            ExtendedColorType::La1
            | ExtendedColorType::La2
            | ExtendedColorType::La4
            | ExtendedColorType::La8
            | ExtendedColorType::La16 => 2,
            ExtendedColorType::Rgb1
            | ExtendedColorType::Rgb2
            | ExtendedColorType::Rgb4
            | ExtendedColorType::Rgb5x1
            | ExtendedColorType::Rgb8
            | ExtendedColorType::Rgb16
            | ExtendedColorType::Rgb32F
            | ExtendedColorType::Bgr8 => 3,
            ExtendedColorType::Rgba1
            | ExtendedColorType::Rgba2
            | ExtendedColorType::Rgba4
            | ExtendedColorType::Rgba8
            | ExtendedColorType::Rgba16
            | ExtendedColorType::Rgba32F
            | ExtendedColorType::Bgra8
            | ExtendedColorType::Cmyk8
            | ExtendedColorType::Cmyk16 => 4,
        }
    }

    /// Returns the number of bits per pixel for this color type.
    #[must_use]
    pub fn bits_per_pixel(&self) -> u16 {
        match *self {
            ExtendedColorType::A8 => 8,
            ExtendedColorType::L1 => 1,
            ExtendedColorType::La1 => 2,
            ExtendedColorType::Rgb1 => 3,
            ExtendedColorType::Rgba1 => 4,
            ExtendedColorType::L2 => 2,
            ExtendedColorType::La2 => 4,
            ExtendedColorType::Rgb2 => 6,
            ExtendedColorType::Rgba2 => 8,
            ExtendedColorType::L4 => 4,
            ExtendedColorType::La4 => 8,
            ExtendedColorType::Rgb4 => 12,
            ExtendedColorType::Rgba4 => 16,
            ExtendedColorType::Rgb5x1 => 16,
            ExtendedColorType::L8 => 8,
            ExtendedColorType::La8 => 16,
            ExtendedColorType::Rgb8 => 24,
            ExtendedColorType::Rgba8 => 32,
            ExtendedColorType::L16 => 16,
            ExtendedColorType::La16 => 32,
            ExtendedColorType::Rgb16 => 48,
            ExtendedColorType::Rgba16 => 64,
            ExtendedColorType::Rgb32F => 96,
            ExtendedColorType::Rgba32F => 128,
            ExtendedColorType::L32F => 32,
            ExtendedColorType::L32I => 32,
            ExtendedColorType::Bgr8 => 24,
            ExtendedColorType::Bgra8 => 32,
            ExtendedColorType::Cmyk8 => 32,
            ExtendedColorType::Cmyk16 => 64,
            ExtendedColorType::Unknown(bpp) => bpp as u16,
        }
    }

    /// Returns the ColorType that is equivalent to this ExtendedColorType.
    pub fn color_type(&self) -> Option<ColorType> {
        match *self {
            ExtendedColorType::L8 => Some(ColorType::L8),
            ExtendedColorType::La8 => Some(ColorType::La8),
            ExtendedColorType::Rgb8 => Some(ColorType::Rgb8),
            ExtendedColorType::Rgba8 => Some(ColorType::Rgba8),
            ExtendedColorType::L16 => Some(ColorType::L16),
            ExtendedColorType::La16 => Some(ColorType::La16),
            ExtendedColorType::Rgb16 => Some(ColorType::Rgb16),
            ExtendedColorType::Rgba16 => Some(ColorType::Rgba16),
            ExtendedColorType::Rgb32F => Some(ColorType::Rgb32F),
            ExtendedColorType::Rgba32F => Some(ColorType::Rgba32F),
            ExtendedColorType::L32F => Some(ColorType::L32F),
            ExtendedColorType::L32I => Some(ColorType::L32I),
            _ => None,
        }
    }
}

impl From<ColorType> for ExtendedColorType {
    fn from(c: ColorType) -> Self {
        match c {
            ColorType::L8 => ExtendedColorType::L8,
            ColorType::La8 => ExtendedColorType::La8,
            ColorType::Rgb8 => ExtendedColorType::Rgb8,
            ColorType::Rgba8 => ExtendedColorType::Rgba8,
            ColorType::Cmyk8 => ExtendedColorType::Cmyk8,
            ColorType::L16 => ExtendedColorType::L16,
            ColorType::La16 => ExtendedColorType::La16,
            ColorType::Rgb16 => ExtendedColorType::Rgb16,
            ColorType::Rgba16 => ExtendedColorType::Rgba16,
            ColorType::Rgb32F => ExtendedColorType::Rgb32F,
            ColorType::Rgba32F => ExtendedColorType::Rgba32F,
            ColorType::L32F => ExtendedColorType::L32F,
            ColorType::L32I => ExtendedColorType::L32I,
        }
    }
}
// ---------------------------------------------------------------------------

/// RGB colors.
///
/// For the purpose of color conversion, as well as blending, the implementation of `Pixel`
/// assumes an `sRGB` color space of its data.
#[derive(PartialEq, Eq, Clone, Debug, Copy, Hash, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(transparent)]
pub struct Rgb<T>(pub [T; 3]);

/// Grayscale colors.
#[derive(PartialEq, Eq, Clone, Debug, Copy, Hash, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(transparent)]
pub struct Luma<T>(pub [T; 1]);

/// RGB colors + alpha channel
#[derive(PartialEq, Eq, Clone, Debug, Copy, Hash, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(transparent)]
pub struct Rgba<T>(pub [T; 4]);

/// Grayscale colors + alpha channel
#[derive(PartialEq, Eq, Clone, Debug, Copy, Hash, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(transparent)]
pub struct LumaA<T>(pub [T; 2]);
impl<T> Index<usize> for Luma<T> {
    type Output = T;
    #[inline(always)]
    fn index(&self, index: usize) -> &T {
        &self.0[index]
    }
}

impl<T> IndexMut<usize> for Luma<T> {
    #[inline(always)]
    fn index_mut(&mut self, index: usize) -> &mut T {
        &mut self.0[index]
    }
}

impl<T> Index<usize> for LumaA<T> {
    type Output = T;
    #[inline(always)]
    fn index(&self, index: usize) -> &T {
        &self.0[index]
    }
}

impl<T> IndexMut<usize> for LumaA<T> {
    #[inline(always)]
    fn index_mut(&mut self, index: usize) -> &mut T {
        &mut self.0[index]
    }
}

impl<T> Index<usize> for Rgb<T> {
    type Output = T;
    #[inline(always)]
    fn index(&self, index: usize) -> &T {
        &self.0[index]
    }
}

impl<T> IndexMut<usize> for Rgb<T> {
    #[inline(always)]
    fn index_mut(&mut self, index: usize) -> &mut T {
        &mut self.0[index]
    }
}

impl<T> Index<usize> for Rgba<T> {
    type Output = T;
    #[inline(always)]
    fn index(&self, index: usize) -> &T {
        &self.0[index]
    }
}

impl<T> IndexMut<usize> for Rgba<T> {
    #[inline(always)]
    fn index_mut(&mut self, index: usize) -> &mut T {
        &mut self.0[index]
    }
}

// ---------------------------------------------------------------------------
// From<[T; N]> for pixel types
// ---------------------------------------------------------------------------
impl<T> From<[T; 1]> for Luma<T> {
    fn from(c: [T; 1]) -> Self {
        Luma(c)
    }
}

impl<T> From<[T; 2]> for LumaA<T> {
    fn from(c: [T; 2]) -> Self {
        LumaA(c)
    }
}

impl<T> From<[T; 3]> for Rgb<T> {
    fn from(c: [T; 3]) -> Self {
        Rgb(c)
    }
}

impl<T> From<[T; 4]> for Rgba<T> {
    fn from(c: [T; 4]) -> Self {
        Rgba(c)
    }
}

// ---------------------------------------------------------------------------
// FromColor trait - convert between pixel color types
// ---------------------------------------------------------------------------
