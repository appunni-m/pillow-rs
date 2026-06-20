//! Color type definitions — `ColorType`, `ExtendedColorType`, pixel structs.

use std::ops::{Index, IndexMut};

/// An enumeration over supported color types and bit depths.
#[derive(Copy, PartialEq, Eq, Debug, Clone, Hash)]
#[non_exhaustive]
pub enum ColorType {
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
    /// Pixel is 16-bit RGB
    Rgb16,
    /// Pixel is 16-bit RGBA
    Rgba16,

    /// Pixel is 32-bit float RGB
    Rgb32F,
    /// Pixel is 32-bit float RGBA
    Rgba32F,
}

impl ColorType {
    /// Returns the number of bytes contained in a pixel of `ColorType`.
    #[must_use]
    pub fn bytes_per_pixel(self) -> u8 {
        match self {
            ColorType::L8 => 1,
            ColorType::L16 | ColorType::La8 => 2,
            ColorType::Rgb8 => 3,
            ColorType::Rgba8 | ColorType::La16 => 4,
            ColorType::Rgb16 => 6,
            ColorType::Rgba16 => 8,
            ColorType::Rgb32F => 3 * 4,
            ColorType::Rgba32F => 4 * 4,
        }
    }

    /// Returns if there is an alpha channel.
    #[must_use]
    pub fn has_alpha(self) -> bool {
        use ColorType::*;
        match self {
            L8 | L16 | Rgb8 | Rgb16 | Rgb32F => false,
            La8 | Rgba8 | La16 | Rgba16 | Rgba32F => true,
        }
    }

    /// Returns false if the color scheme is grayscale, true otherwise.
    #[must_use]
    pub fn has_color(self) -> bool {
        use ColorType::*;
        match self {
            L8 | L16 | La8 | La16 => false,
            Rgb8 | Rgb16 | Rgba8 | Rgba16 | Rgb32F | Rgba32F => true,
        }
    }

    /// Returns the number of bits contained in a pixel of `ColorType`.
    #[must_use]
    pub fn bits_per_pixel(self) -> u16 {
        <u16 as From<u8>>::from(self.bytes_per_pixel()) * 8
    }

    /// Returns the number of color channels that make up this pixel.
    #[must_use]
    pub fn channel_count(self) -> u8 {
        let e: ExtendedColorType = self.into();
        e.channel_count()
    }

    // Backward-compatible aliases for the old naming convention used by decoders.
    #[doc(hidden)]
    #[allow(non_upper_case_globals)]
    pub const Luma8: ColorType = ColorType::L8;
    #[doc(hidden)]
    #[allow(non_upper_case_globals)]
    pub const LumaA8: ColorType = ColorType::La8;
}
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
            ColorType::L16 => ExtendedColorType::L16,
            ColorType::La16 => ExtendedColorType::La16,
            ColorType::Rgb16 => ExtendedColorType::Rgb16,
            ColorType::Rgba16 => ExtendedColorType::Rgba16,
            ColorType::Rgb32F => ExtendedColorType::Rgb32F,
            ColorType::Rgba32F => ExtendedColorType::Rgba32F,
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
