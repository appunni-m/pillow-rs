//! Color types and pixel representations for pillow-rs-image.
//!
//! This module provides the pixel types `Luma`, `LumaA`, `Rgb`, `Rgba` along
//! with the `ColorType` and `ExtendedColorType` enums, matching the `image` crate.

use std::ops::{Index, IndexMut};

use super::traits::{Enlargeable, Pixel, Primitive};

// ---------------------------------------------------------------------------
// ColorType
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// ExtendedColorType
// ---------------------------------------------------------------------------

/// An enumeration of color types encountered in image formats.
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
// Pixel types
// ---------------------------------------------------------------------------

/// RGB colors.
///
/// For the purpose of color conversion, as well as blending, the implementation of `Pixel`
/// assumes an `sRGB` color space of its data.
#[derive(PartialEq, Eq, Clone, Debug, Copy, Hash)]
#[repr(transparent)]
pub struct Rgb<T>(pub [T; 3]);

/// Grayscale colors.
#[derive(PartialEq, Eq, Clone, Debug, Copy, Hash)]
#[repr(transparent)]
pub struct Luma<T>(pub [T; 1]);

/// RGB colors + alpha channel
#[derive(PartialEq, Eq, Clone, Debug, Copy, Hash)]
#[repr(transparent)]
pub struct Rgba<T>(pub [T; 4]);

/// Grayscale colors + alpha channel
#[derive(PartialEq, Eq, Clone, Debug, Copy, Hash)]
#[repr(transparent)]
pub struct LumaA<T>(pub [T; 2]);

// ---------------------------------------------------------------------------
// Pixel trait implementations
// ---------------------------------------------------------------------------

impl<T: Primitive> Pixel for Luma<T> {
    type Subpixel = T;

    const CHANNEL_COUNT: u8 = 1;

    #[inline(always)]
    fn channels(&self) -> &[T] {
        &self.0
    }

    #[inline(always)]
    fn channels_mut(&mut self) -> &mut [T] {
        &mut self.0
    }

    const COLOR_MODEL: &'static str = "Y";

    const HAS_ALPHA: bool = false;

    #[inline]
    fn alpha(&self) -> Self::Subpixel {
        Self::Subpixel::DEFAULT_MAX_VALUE
    }

    fn channels4(&self) -> (T, T, T, T) {
        let c = self.0;
        (c[0], T::DEFAULT_MAX_VALUE, T::DEFAULT_MAX_VALUE, T::DEFAULT_MAX_VALUE)
    }

    fn from_channels(a: T, _b: T, _c: T, _d: T) -> Luma<T> {
        Luma([a])
    }

    fn from_slice(slice: &[T]) -> &Luma<T> {
        assert_eq!(slice.len(), 1);
        unsafe { &*(slice.as_ptr() as *const Luma<T>) }
    }

    fn from_slice_mut(slice: &mut [T]) -> &mut Luma<T> {
        assert_eq!(slice.len(), 1);
        unsafe { &mut *(slice.as_mut_ptr() as *mut Luma<T>) }
    }

    fn to_rgb(&self) -> Rgb<T> {
        let mut pix = Rgb([T::DEFAULT_MIN_VALUE, T::DEFAULT_MIN_VALUE, T::DEFAULT_MIN_VALUE]);
        pix.from_color(self);
        pix
    }

    fn to_rgba(&self) -> Rgba<T> {
        let mut pix = Rgba([T::DEFAULT_MIN_VALUE, T::DEFAULT_MIN_VALUE, T::DEFAULT_MIN_VALUE, T::DEFAULT_MIN_VALUE]);
        pix.from_color(self);
        pix
    }

    fn to_luma(&self) -> Luma<T> {
        *self
    }

    fn to_luma_alpha(&self) -> LumaA<T> {
        let mut pix = LumaA([T::DEFAULT_MIN_VALUE, T::DEFAULT_MIN_VALUE]);
        pix.from_color(self);
        pix
    }

    fn map<F>(&self, f: F) -> Luma<T> where F: FnMut(T) -> T {
        let mut this = *self;
        this.apply(f);
        this
    }

    fn apply<F>(&mut self, mut f: F) where F: FnMut(T) -> T {
        for v in &mut self.0 {
            *v = f(*v);
        }
    }

    fn map_with_alpha<F, G>(&self, f: F, _g: G) -> Luma<T> where F: FnMut(T) -> T, G: FnMut(T) -> T {
        self.map(f)
    }

    fn apply_with_alpha<F, G>(&mut self, f: F, _g: G) where F: FnMut(T) -> T, G: FnMut(T) -> T {
        self.apply(f);
    }

    fn map2<F>(&self, other: &Luma<T>, f: F) -> Luma<T> where F: FnMut(T, T) -> T {
        let mut this = *self;
        this.apply2(other, f);
        this
    }

    fn apply2<F>(&mut self, other: &Luma<T>, mut f: F) where F: FnMut(T, T) -> T {
        for (a, &b) in self.0.iter_mut().zip(other.0.iter()) {
            *a = f(*a, b);
        }
    }

    fn invert(&mut self) {
        Invert::invert(self);
    }

    fn blend(&mut self, other: &Luma<T>) {
        Blend::blend(self, other);
    }
}

impl<T: Primitive> Pixel for LumaA<T> {
    type Subpixel = T;

    const CHANNEL_COUNT: u8 = 2;

    #[inline(always)]
    fn channels(&self) -> &[T] {
        &self.0
    }

    #[inline(always)]
    fn channels_mut(&mut self) -> &mut [T] {
        &mut self.0
    }

    const COLOR_MODEL: &'static str = "YA";

    const HAS_ALPHA: bool = true;

    #[inline]
    fn alpha(&self) -> Self::Subpixel {
        self.0[1]
    }

    fn channels4(&self) -> (T, T, T, T) {
        let c = self.0;
        (c[0], T::DEFAULT_MAX_VALUE, T::DEFAULT_MAX_VALUE, c[1])
    }

    fn from_channels(a: T, _b: T, _c: T, d: T) -> LumaA<T> {
        LumaA([a, d])
    }

    fn from_slice(slice: &[T]) -> &LumaA<T> {
        assert_eq!(slice.len(), 2);
        unsafe { &*(slice.as_ptr() as *const LumaA<T>) }
    }

    fn from_slice_mut(slice: &mut [T]) -> &mut LumaA<T> {
        assert_eq!(slice.len(), 2);
        unsafe { &mut *(slice.as_mut_ptr() as *mut LumaA<T>) }
    }

    fn to_rgb(&self) -> Rgb<T> {
        let mut pix = Rgb([T::DEFAULT_MIN_VALUE, T::DEFAULT_MIN_VALUE, T::DEFAULT_MIN_VALUE]);
        pix.from_color(self);
        pix
    }

    fn to_rgba(&self) -> Rgba<T> {
        let mut pix = Rgba([T::DEFAULT_MIN_VALUE, T::DEFAULT_MIN_VALUE, T::DEFAULT_MIN_VALUE, T::DEFAULT_MIN_VALUE]);
        pix.from_color(self);
        pix
    }

    fn to_luma(&self) -> Luma<T> {
        let mut pix = Luma([T::DEFAULT_MIN_VALUE]);
        pix.from_color(self);
        pix
    }

    fn to_luma_alpha(&self) -> LumaA<T> {
        *self
    }

    fn map<F>(&self, f: F) -> LumaA<T> where F: FnMut(T) -> T {
        let mut this = *self;
        this.apply(f);
        this
    }

    fn apply<F>(&mut self, mut f: F) where F: FnMut(T) -> T {
        for v in &mut self.0 {
            *v = f(*v);
        }
    }

    fn map_with_alpha<F, G>(&self, f: F, g: G) -> LumaA<T> where F: FnMut(T) -> T, G: FnMut(T) -> T {
        let mut this = *self;
        this.apply_with_alpha(f, g);
        this
    }

    fn apply_with_alpha<F, G>(&mut self, mut f: F, mut g: G) where F: FnMut(T) -> T, G: FnMut(T) -> T {
        self.0[0] = f(self.0[0]);
        self.0[1] = g(self.0[1]);
    }

    fn map2<F>(&self, other: &LumaA<T>, f: F) -> LumaA<T> where F: FnMut(T, T) -> T {
        let mut this = *self;
        this.apply2(other, f);
        this
    }

    fn apply2<F>(&mut self, other: &LumaA<T>, mut f: F) where F: FnMut(T, T) -> T {
        for (a, &b) in self.0.iter_mut().zip(other.0.iter()) {
            *a = f(*a, b);
        }
    }

    fn invert(&mut self) {
        Invert::invert(self);
    }

    fn blend(&mut self, other: &LumaA<T>) {
        Blend::blend(self, other);
    }
}

impl<T: Primitive + Enlargeable> Pixel for Rgb<T> {
    type Subpixel = T;

    const CHANNEL_COUNT: u8 = 3;

    #[inline(always)]
    fn channels(&self) -> &[T] {
        &self.0
    }

    #[inline(always)]
    fn channels_mut(&mut self) -> &mut [T] {
        &mut self.0
    }

    const COLOR_MODEL: &'static str = "RGB";

    const HAS_ALPHA: bool = false;

    fn channels4(&self) -> (T, T, T, T) {
        let c = self.0;
        (c[0], c[1], c[2], T::DEFAULT_MAX_VALUE)
    }

    fn from_channels(a: T, b: T, c: T, _d: T) -> Rgb<T> {
        Rgb([a, b, c])
    }

    fn from_slice(slice: &[T]) -> &Rgb<T> {
        assert_eq!(slice.len(), 3);
        unsafe { &*(slice.as_ptr() as *const Rgb<T>) }
    }

    fn from_slice_mut(slice: &mut [T]) -> &mut Rgb<T> {
        assert_eq!(slice.len(), 3);
        unsafe { &mut *(slice.as_mut_ptr() as *mut Rgb<T>) }
    }

    fn to_rgb(&self) -> Rgb<T> {
        *self
    }

    fn to_rgba(&self) -> Rgba<T> {
        let mut pix = Rgba([T::DEFAULT_MIN_VALUE, T::DEFAULT_MIN_VALUE, T::DEFAULT_MIN_VALUE, T::DEFAULT_MIN_VALUE]);
        pix.from_color(self);
        pix
    }

    fn to_luma(&self) -> Luma<T> {
        let mut pix = Luma([T::DEFAULT_MIN_VALUE]);
        pix.from_color(self);
        pix
    }

    fn to_luma_alpha(&self) -> LumaA<T> {
        let mut pix = LumaA([T::DEFAULT_MIN_VALUE, T::DEFAULT_MIN_VALUE]);
        pix.from_color(self);
        pix
    }

    fn map<F>(&self, f: F) -> Rgb<T> where F: FnMut(T) -> T {
        let mut this = *self;
        this.apply(f);
        this
    }

    fn apply<F>(&mut self, mut f: F) where F: FnMut(T) -> T {
        for v in &mut self.0 {
            *v = f(*v);
        }
    }

    fn map_with_alpha<F, G>(&self, f: F, _g: G) -> Rgb<T> where F: FnMut(T) -> T, G: FnMut(T) -> T {
        self.map(f)
    }

    fn apply_with_alpha<F, G>(&mut self, f: F, _g: G) where F: FnMut(T) -> T, G: FnMut(T) -> T {
        self.apply(f);
    }

    fn map2<F>(&self, other: &Rgb<T>, f: F) -> Rgb<T> where F: FnMut(T, T) -> T {
        let mut this = *self;
        this.apply2(other, f);
        this
    }

    fn apply2<F>(&mut self, other: &Rgb<T>, mut f: F) where F: FnMut(T, T) -> T {
        for (a, &b) in self.0.iter_mut().zip(other.0.iter()) {
            *a = f(*a, b);
        }
    }

    fn invert(&mut self) {
        Invert::invert(self);
    }

    fn blend(&mut self, other: &Rgb<T>) {
        Blend::blend(self, other);
    }
}

impl<T: Primitive + Enlargeable> Pixel for Rgba<T> {
    type Subpixel = T;

    const CHANNEL_COUNT: u8 = 4;

    #[inline(always)]
    fn channels(&self) -> &[T] {
        &self.0
    }

    #[inline(always)]
    fn channels_mut(&mut self) -> &mut [T] {
        &mut self.0
    }

    const COLOR_MODEL: &'static str = "RGBA";

    const HAS_ALPHA: bool = true;

    #[inline]
    fn alpha(&self) -> Self::Subpixel {
        self.0[3]
    }

    fn channels4(&self) -> (T, T, T, T) {
        let c = self.0;
        (c[0], c[1], c[2], c[3])
    }

    fn from_channels(a: T, b: T, c: T, d: T) -> Rgba<T> {
        Rgba([a, b, c, d])
    }

    fn from_slice(slice: &[T]) -> &Rgba<T> {
        assert_eq!(slice.len(), 4);
        unsafe { &*(slice.as_ptr() as *const Rgba<T>) }
    }

    fn from_slice_mut(slice: &mut [T]) -> &mut Rgba<T> {
        assert_eq!(slice.len(), 4);
        unsafe { &mut *(slice.as_mut_ptr() as *mut Rgba<T>) }
    }

    fn to_rgb(&self) -> Rgb<T> {
        let mut pix = Rgb([T::DEFAULT_MIN_VALUE, T::DEFAULT_MIN_VALUE, T::DEFAULT_MIN_VALUE]);
        pix.from_color(self);
        pix
    }

    fn to_rgba(&self) -> Rgba<T> {
        *self
    }

    fn to_luma(&self) -> Luma<T> {
        let mut pix = Luma([T::DEFAULT_MIN_VALUE]);
        pix.from_color(self);
        pix
    }

    fn to_luma_alpha(&self) -> LumaA<T> {
        let mut pix = LumaA([T::DEFAULT_MIN_VALUE, T::DEFAULT_MIN_VALUE]);
        pix.from_color(self);
        pix
    }

    fn map<F>(&self, f: F) -> Rgba<T> where F: FnMut(T) -> T {
        let mut this = *self;
        this.apply(f);
        this
    }

    fn apply<F>(&mut self, mut f: F) where F: FnMut(T) -> T {
        for v in &mut self.0 {
            *v = f(*v);
        }
    }

    fn map_with_alpha<F, G>(&self, f: F, g: G) -> Rgba<T> where F: FnMut(T) -> T, G: FnMut(T) -> T {
        let mut this = *self;
        this.apply_with_alpha(f, g);
        this
    }

    fn apply_with_alpha<F, G>(&mut self, mut f: F, mut g: G) where F: FnMut(T) -> T, G: FnMut(T) -> T {
        for v in self.0[..3].iter_mut() {
            *v = f(*v);
        }
        self.0[3] = g(self.0[3]);
    }

    fn map2<F>(&self, other: &Rgba<T>, f: F) -> Rgba<T> where F: FnMut(T, T) -> T {
        let mut this = *self;
        this.apply2(other, f);
        this
    }

    fn apply2<F>(&mut self, other: &Rgba<T>, mut f: F) where F: FnMut(T, T) -> T {
        for (a, &b) in self.0.iter_mut().zip(other.0.iter()) {
            *a = f(*a, b);
        }
    }

    fn invert(&mut self) {
        Invert::invert(self);
    }

    fn blend(&mut self, other: &Rgba<T>) {
        Blend::blend(self, other);
    }
}

// ---------------------------------------------------------------------------
// Index/IndexMut for pixel types
// ---------------------------------------------------------------------------

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

/// Provides color conversions for the different pixel types.
pub trait FromColor<Other> {
    /// Changes `self` to represent `Other` in the color space of `Self`.
    fn from_color(&mut self, _: &Other);
}

/// Convert from one pixel component type to another.
pub trait FromPrimitive<Component> {
    /// Converts from any pixel component type to this type.
    fn from_primitive(component: Component) -> Self;
}

impl<T: Primitive> FromPrimitive<T> for T {
    fn from_primitive(sample: T) -> Self {
        sample
    }
}

// From f32:
impl FromPrimitive<f32> for u8 {
    fn from_primitive(float: f32) -> Self {
        normalize_float(float, u8::MAX as f32) as u8
    }
}

impl FromPrimitive<f32> for u16 {
    fn from_primitive(float: f32) -> Self {
        normalize_float(float, u16::MAX as f32) as u16
    }
}

// From u16:
impl FromPrimitive<u16> for u8 {
    fn from_primitive(c16: u16) -> Self {
        ((c16 as u32 + 128) / 257) as u8
    }
}

impl FromPrimitive<u16> for f32 {
    fn from_primitive(int: u16) -> Self {
        (int as f32 / u16::MAX as f32).clamp(0.0, 1.0)
    }
}

// From u8:
impl FromPrimitive<u8> for f32 {
    fn from_primitive(int: u8) -> Self {
        (int as f32 / u8::MAX as f32).clamp(0.0, 1.0)
    }
}

impl FromPrimitive<u8> for u16 {
    fn from_primitive(c8: u8) -> Self {
        let x = c8 as u64;
        ((x << 8) | x) as u16
    }
}

#[inline]
fn normalize_float(float: f32, max: f32) -> f32 {
    let clamped = if !(float < 1.0) { 1.0 } else { float.max(0.0) };
    (clamped * max).round()
}

// ---------------------------------------------------------------------------
// Color conversion coefficients
// ---------------------------------------------------------------------------

/// Coefficients to transform from sRGB to a CIE Y (luminance) value.
const SRGB_LUMA: [u32; 3] = [2126, 7152, 722];
const SRGB_LUMA_DIV: u32 = 10000;

#[inline]
fn rgb_to_luma<T: Primitive + Enlargeable>(rgb: &[T]) -> T {
    let l = <T::Larger as Primitive>::from_f32(
        rgb[0].to_f32() * (SRGB_LUMA[0] as f32 / SRGB_LUMA_DIV as f32)
            + rgb[1].to_f32() * (SRGB_LUMA[1] as f32 / SRGB_LUMA_DIV as f32)
            + rgb[2].to_f32() * (SRGB_LUMA[2] as f32 / SRGB_LUMA_DIV as f32)
    );
    T::clamp_from(l)
}

// ---------------------------------------------------------------------------
// FromColor implementations
// ---------------------------------------------------------------------------

// `FromColor` for Luma

impl<S: Primitive, T: Primitive> FromColor<Luma<S>> for Luma<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, other: &Luma<S>) {
        let own = self.channels_mut();
        let other = other.channels();
        own[0] = T::from_primitive(other[0]);
    }
}

impl<S: Primitive, T: Primitive> FromColor<LumaA<S>> for Luma<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, other: &LumaA<S>) {
        self.channels_mut()[0] = T::from_primitive(other.channels()[0]);
    }
}

impl<S: Primitive + Enlargeable, T: Primitive> FromColor<Rgb<S>> for Luma<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, other: &Rgb<S>) {
        let gray = self.channels_mut();
        let rgb = other.channels();
        gray[0] = T::from_primitive(rgb_to_luma(rgb));
    }
}

impl<S: Primitive + Enlargeable, T: Primitive> FromColor<Rgba<S>> for Luma<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, other: &Rgba<S>) {
        let gray = self.channels_mut();
        let rgb = other.channels();
        let l = rgb_to_luma(rgb);
        gray[0] = T::from_primitive(l);
    }
}

// `FromColor` for LumaA

impl<S: Primitive, T: Primitive> FromColor<LumaA<S>> for LumaA<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, other: &LumaA<S>) {
        let own = self.channels_mut();
        let other = other.channels();
        own[0] = T::from_primitive(other[0]);
        own[1] = T::from_primitive(other[1]);
    }
}

impl<S: Primitive + Enlargeable, T: Primitive> FromColor<Rgb<S>> for LumaA<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, other: &Rgb<S>) {
        let gray_a = self.channels_mut();
        let rgb = other.channels();
        gray_a[0] = T::from_primitive(rgb_to_luma(rgb));
        gray_a[1] = T::DEFAULT_MAX_VALUE;
    }
}

impl<S: Primitive + Enlargeable, T: Primitive> FromColor<Rgba<S>> for LumaA<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, other: &Rgba<S>) {
        let gray_a = self.channels_mut();
        let rgba = other.channels();
        gray_a[0] = T::from_primitive(rgb_to_luma(rgba));
        gray_a[1] = T::from_primitive(rgba[3]);
    }
}

impl<S: Primitive, T: Primitive> FromColor<Luma<S>> for LumaA<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, other: &Luma<S>) {
        let gray_a = self.channels_mut();
        gray_a[0] = T::from_primitive(other.channels()[0]);
        gray_a[1] = T::DEFAULT_MAX_VALUE;
    }
}

// `FromColor` for RGBA

impl<S: Primitive, T: Primitive> FromColor<Rgba<S>> for Rgba<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, other: &Rgba<S>) {
        let own = &mut self.0;
        let other = &other.0;
        own[0] = T::from_primitive(other[0]);
        own[1] = T::from_primitive(other[1]);
        own[2] = T::from_primitive(other[2]);
        own[3] = T::from_primitive(other[3]);
    }
}

impl<S: Primitive, T: Primitive> FromColor<Rgb<S>> for Rgba<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, other: &Rgb<S>) {
        let rgba = &mut self.0;
        let rgb = &other.0;
        rgba[0] = T::from_primitive(rgb[0]);
        rgba[1] = T::from_primitive(rgb[1]);
        rgba[2] = T::from_primitive(rgb[2]);
        rgba[3] = T::DEFAULT_MAX_VALUE;
    }
}

impl<S: Primitive, T: Primitive> FromColor<LumaA<S>> for Rgba<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, gray: &LumaA<S>) {
        let rgba = &mut self.0;
        let gray = &gray.0;
        rgba[0] = T::from_primitive(gray[0]);
        rgba[1] = T::from_primitive(gray[0]);
        rgba[2] = T::from_primitive(gray[0]);
        rgba[3] = T::from_primitive(gray[1]);
    }
}

impl<S: Primitive, T: Primitive> FromColor<Luma<S>> for Rgba<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, gray: &Luma<S>) {
        let rgba = &mut self.0;
        let gray = gray.0[0];
        rgba[0] = T::from_primitive(gray);
        rgba[1] = T::from_primitive(gray);
        rgba[2] = T::from_primitive(gray);
        rgba[3] = T::DEFAULT_MAX_VALUE;
    }
}

// `FromColor` for RGB

impl<S: Primitive, T: Primitive> FromColor<Rgb<S>> for Rgb<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, other: &Rgb<S>) {
        let own = &mut self.0;
        let other = &other.0;
        own[0] = T::from_primitive(other[0]);
        own[1] = T::from_primitive(other[1]);
        own[2] = T::from_primitive(other[2]);
    }
}

impl<S: Primitive, T: Primitive> FromColor<Rgba<S>> for Rgb<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, other: &Rgba<S>) {
        let rgb = &mut self.0;
        let rgba = &other.0;
        rgb[0] = T::from_primitive(rgba[0]);
        rgb[1] = T::from_primitive(rgba[1]);
        rgb[2] = T::from_primitive(rgba[2]);
    }
}

impl<S: Primitive, T: Primitive> FromColor<LumaA<S>> for Rgb<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, other: &LumaA<S>) {
        let rgb = &mut self.0;
        let gray = other.0[0];
        rgb[0] = T::from_primitive(gray);
        rgb[1] = T::from_primitive(gray);
        rgb[2] = T::from_primitive(gray);
    }
}

impl<S: Primitive, T: Primitive> FromColor<Luma<S>> for Rgb<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, other: &Luma<S>) {
        let rgb = &mut self.0;
        let gray = other.0[0];
        rgb[0] = T::from_primitive(gray);
        rgb[1] = T::from_primitive(gray);
        rgb[2] = T::from_primitive(gray);
    }
}

// ---------------------------------------------------------------------------
// Blend trait
// ---------------------------------------------------------------------------

trait Blend {
    fn blend(&mut self, other: &Self);
}

impl<T: Primitive> Blend for LumaA<T> {
    fn blend(&mut self, other: &LumaA<T>) {
        if other.0[1].to_f32() >= T::DEFAULT_MAX_VALUE.to_f32() - 0.001 {
            *self = *other;
            return;
        }
        if other.0[1].to_f32() <= 0.001 {
            return;
        }

        let max_t = T::DEFAULT_MAX_VALUE;
        let max_f = max_t.to_f32();

        let bg_luma_f = self.0[0].to_f32() / max_f;
        let bg_a_f = self.0[1].to_f32() / max_f;
        let fg_luma_f = other.0[0].to_f32() / max_f;
        let fg_a_f = other.0[1].to_f32() / max_f;

        let alpha_final = bg_a_f + fg_a_f - bg_a_f * fg_a_f;
        if alpha_final == 0.0 {
            return;
        }

        let bg_luma_a = bg_luma_f * bg_a_f;
        let fg_luma_a = fg_luma_f * fg_a_f;

        let out_luma_a = fg_luma_a + bg_luma_a * (1.0 - fg_a_f);
        let out_luma = out_luma_a / alpha_final;

        *self = LumaA([
            T::from_f32(max_f * out_luma),
            T::from_f32(max_f * alpha_final),
        ]);
    }
}

impl<T: Primitive> Blend for Luma<T> {
    fn blend(&mut self, other: &Luma<T>) {
        *self = *other;
    }
}

impl<T: Primitive> Blend for Rgba<T> {
    fn blend(&mut self, other: &Rgba<T>) {
        if other.0[3].to_f32() >= T::DEFAULT_MAX_VALUE.to_f32() - 0.001 {
            *self = *other;
            return;
        }
        if other.0[3].to_f32() <= 0.001 {
            return;
        }

        let max_t = T::DEFAULT_MAX_VALUE;
        let max_f = max_t.to_f32();

        let (bg_r, bg_g, bg_b, bg_a) = (
            self.0[0].to_f32() / max_f,
            self.0[1].to_f32() / max_f,
            self.0[2].to_f32() / max_f,
            self.0[3].to_f32() / max_f,
        );
        let (fg_r, fg_g, fg_b, fg_a) = (
            other.0[0].to_f32() / max_f,
            other.0[1].to_f32() / max_f,
            other.0[2].to_f32() / max_f,
            other.0[3].to_f32() / max_f,
        );

        let alpha_final = bg_a + fg_a - bg_a * fg_a;
        if alpha_final == 0.0 {
            return;
        }

        let (bg_r_a, bg_g_a, bg_b_a) = (bg_r * bg_a, bg_g * bg_a, bg_b * bg_a);
        let (fg_r_a, fg_g_a, fg_b_a) = (fg_r * fg_a, fg_g * fg_a, fg_b * fg_a);

        let (out_r_a, out_g_a, out_b_a) = (
            fg_r_a + bg_r_a * (1.0 - fg_a),
            fg_g_a + bg_g_a * (1.0 - fg_a),
            fg_b_a + bg_b_a * (1.0 - fg_a),
        );

        let (out_r, out_g, out_b) = (
            out_r_a / alpha_final,
            out_g_a / alpha_final,
            out_b_a / alpha_final,
        );

        *self = Rgba([
            T::from_f32(max_f * out_r),
            T::from_f32(max_f * out_g),
            T::from_f32(max_f * out_b),
            T::from_f32(max_f * alpha_final),
        ]);
    }
}

impl<T: Primitive> Blend for Rgb<T> {
    fn blend(&mut self, other: &Rgb<T>) {
        *self = *other;
    }
}

// ---------------------------------------------------------------------------
// Invert trait
// ---------------------------------------------------------------------------

trait Invert {
    fn invert(&mut self);
}

impl<T: Primitive> Invert for Luma<T> {
    fn invert(&mut self) {
        let max = T::DEFAULT_MAX_VALUE;
        self.0[0] = T::from_f32(max.to_f32() - self.0[0].to_f32());
    }
}

impl<T: Primitive> Invert for LumaA<T> {
    fn invert(&mut self) {
        let max = T::DEFAULT_MAX_VALUE;
        self.0[0] = T::from_f32(max.to_f32() - self.0[0].to_f32());
        // alpha unchanged
    }
}

impl<T: Primitive> Invert for Rgb<T> {
    fn invert(&mut self) {
        let max = T::DEFAULT_MAX_VALUE;
        self.0[0] = T::from_f32(max.to_f32() - self.0[0].to_f32());
        self.0[1] = T::from_f32(max.to_f32() - self.0[1].to_f32());
        self.0[2] = T::from_f32(max.to_f32() - self.0[2].to_f32());
    }
}

impl<T: Primitive> Invert for Rgba<T> {
    fn invert(&mut self) {
        let max = T::DEFAULT_MAX_VALUE;
        self.0[0] = T::from_f32(max.to_f32() - self.0[0].to_f32());
        self.0[1] = T::from_f32(max.to_f32() - self.0[1].to_f32());
        self.0[2] = T::from_f32(max.to_f32() - self.0[2].to_f32());
        // alpha unchanged
    }
}
