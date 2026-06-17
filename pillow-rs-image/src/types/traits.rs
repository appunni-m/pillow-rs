//! Core traits for the pillow-rs-image type system.
//!
//! This module provides the `Pixel`, `Primitive`, `GenericImageView`, and
//! `GenericImage` traits that form the foundation of the image type system,
//! matching the `image` crate's API.

use super::buffer::ImageBuffer;
use super::color::{Luma, LumaA, Rgb, Rgba};

// ---------------------------------------------------------------------------
// Primitive
// ---------------------------------------------------------------------------

/// The type of each channel in a pixel. For example, this can be `u8`, `u16`, `f32`.
pub trait Primitive: Copy + Clone + PartialOrd + Sized + Default {
    /// The maximum value for this type of primitive within the context of color.
    /// For floats, the maximum is `1.0`, whereas the integer types inherit their usual maximum values.
    const DEFAULT_MAX_VALUE: Self;

    /// The minimum value for this type of primitive within the context of color.
    /// For floats, the minimum is `0.0`, whereas the integer types inherit their usual minimum values.
    const DEFAULT_MIN_VALUE: Self;

    /// Convert to f32.
    fn to_f32(self) -> f32;

    /// Convert from f32 (clamped to valid range).
    fn from_f32(val: f32) -> Self;

    /// Convert to u64.
    fn to_u64(self) -> u64;

    /// Convert from u64 (clamped).
    fn from_u64(val: u64) -> Self;
}

macro_rules! impl_primitive_int {
    ($ty:ty) => {
        impl Primitive for $ty {
            const DEFAULT_MAX_VALUE: Self = <$ty>::MAX;
            const DEFAULT_MIN_VALUE: Self = 0;

            #[inline]
            fn to_f32(self) -> f32 {
                self as f32
            }

            #[inline]
            fn from_f32(val: f32) -> Self {
                let clamped = val.clamp(0.0, <$ty>::MAX as f32);
                clamped as $ty
            }

            #[inline]
            fn to_u64(self) -> u64 {
                self as u64
            }

            #[inline]
            fn from_u64(val: u64) -> Self {
                val.min(<$ty>::MAX as u64) as $ty
            }
        }
    };
}

impl_primitive_int!(u8);
impl_primitive_int!(u16);
impl_primitive_int!(u32);
impl_primitive_int!(u64);
impl_primitive_int!(u128);
impl_primitive_int!(usize);

impl Primitive for f32 {
    const DEFAULT_MAX_VALUE: Self = 1.0;
    const DEFAULT_MIN_VALUE: Self = 0.0;

    #[inline]
    fn to_f32(self) -> f32 {
        self
    }

    #[inline]
    fn from_f32(val: f32) -> Self {
        val.clamp(0.0, 1.0)
    }

    #[inline]
    fn to_u64(self) -> u64 {
        self as u64
    }

    #[inline]
    fn from_u64(val: u64) -> Self {
        (val as f32).clamp(0.0, 1.0)
    }
}

impl Primitive for f64 {
    const DEFAULT_MAX_VALUE: Self = 1.0;
    const DEFAULT_MIN_VALUE: Self = 0.0;

    #[inline]
    fn to_f32(self) -> f32 {
        self as f32
    }

    #[inline]
    fn from_f32(val: f32) -> Self {
        val as f64
    }

    #[inline]
    fn to_u64(self) -> u64 {
        self as u64
    }

    #[inline]
    fn from_u64(val: u64) -> Self {
        (val as f64).clamp(0.0, 1.0)
    }
}

/// An `Enlargeable::Larger` value should be enough to calculate
/// the sum (average) of a few hundred or thousand Enlargeable values.
pub trait Enlargeable: Primitive {
    type Larger: Primitive;

    fn clamp_from(n: Self::Larger) -> Self;
    fn to_larger(self) -> Self::Larger;
}

impl Enlargeable for u8 {
    type Larger = u32;

    #[inline]
    fn clamp_from(n: Self::Larger) -> Self {
        n.min(u8::MAX as u32) as u8
    }

    #[inline]
    fn to_larger(self) -> Self::Larger {
        self as u32
    }
}

impl Enlargeable for u16 {
    type Larger = u32;

    #[inline]
    fn clamp_from(n: Self::Larger) -> Self {
        n.min(u16::MAX as u32) as u16
    }

    #[inline]
    fn to_larger(self) -> Self::Larger {
        self as u32
    }
}

impl Enlargeable for u32 {
    type Larger = u64;

    #[inline]
    fn clamp_from(n: Self::Larger) -> Self {
        n.min(u32::MAX as u64) as u32
    }

    #[inline]
    fn to_larger(self) -> Self::Larger {
        self as u64
    }
}

impl Enlargeable for u64 {
    type Larger = u128;

    #[inline]
    fn clamp_from(n: Self::Larger) -> Self {
        n.min(u64::MAX as u128) as u64
    }

    #[inline]
    fn to_larger(self) -> Self::Larger {
        self as u128
    }
}

impl Enlargeable for usize {
    type Larger = u128;

    #[inline]
    fn clamp_from(n: Self::Larger) -> Self {
        n.min(usize::MAX as u128) as usize
    }

    #[inline]
    fn to_larger(self) -> Self::Larger {
        self as u128
    }
}

impl Enlargeable for f32 {
    type Larger = f64;

    #[inline]
    fn clamp_from(n: Self::Larger) -> Self {
        n.clamp(f32::MIN as f64, f32::MAX as f64) as f32
    }

    #[inline]
    fn to_larger(self) -> Self::Larger {
        self as f64
    }
}

/// Types which are safe to treat as an immutable byte slice in a pixel layout
/// for image encoding.
pub trait EncodableLayout: seals::EncodableLayout {
    /// Get the bytes of this value.
    fn as_bytes(&self) -> &[u8];
}

impl EncodableLayout for [u8] {
    fn as_bytes(&self) -> &[u8] {
        self
    }
}

impl EncodableLayout for [u16] {
    fn as_bytes(&self) -> &[u8] {
        let len = self.len() * 2;
        unsafe { std::slice::from_raw_parts(self.as_ptr() as *const u8, len) }
    }
}

impl EncodableLayout for [f32] {
    fn as_bytes(&self) -> &[u8] {
        let len = self.len() * 4;
        unsafe { std::slice::from_raw_parts(self.as_ptr() as *const u8, len) }
    }
}

mod seals {
    pub trait EncodableLayout {}
    impl EncodableLayout for [u8] {}
    impl EncodableLayout for [u16] {}
    impl EncodableLayout for [f32] {}
}

// ---------------------------------------------------------------------------
// Pixel trait
// ---------------------------------------------------------------------------

/// A generalized pixel.
///
/// A pixel object is usually not used standalone but as a view into an image buffer.
pub trait Pixel: Copy + Clone {
    /// The scalar type that is used to store each channel in this pixel.
    type Subpixel: Primitive;

    /// The number of channels of this pixel type.
    const CHANNEL_COUNT: u8;

    /// Returns the components as a slice.
    fn channels(&self) -> &[Self::Subpixel];

    /// Returns the components as a mutable slice
    fn channels_mut(&mut self) -> &mut [Self::Subpixel];

    /// A string that can help to interpret the meaning each channel.
    const COLOR_MODEL: &'static str;

    /// Returns true if the alpha channel is contained.
    const HAS_ALPHA: bool = false;

    /// Retrieve the value of the alpha channel for this pixel.
    #[inline]
    fn alpha(&self) -> Self::Subpixel {
        if Self::HAS_ALPHA {
            *self.to_luma_alpha().channels().last().unwrap()
        } else {
            Self::Subpixel::DEFAULT_MAX_VALUE
        }
    }

    /// Returns the channels of this pixel as a 4 tuple. If the pixel
    /// has less than 4 channels the remainder is filled with the maximum value
    #[deprecated(since = "0.24.0", note = "Use `channels()` or `channels_mut()`")]
    fn channels4(
        &self,
    ) -> (
        Self::Subpixel,
        Self::Subpixel,
        Self::Subpixel,
        Self::Subpixel,
    );

    /// Construct a pixel from the 4 channels a, b, c and d.
    /// If the pixel does not contain 4 channels the extra are ignored.
    #[deprecated(
        since = "0.24.0",
        note = "Use the constructor of the pixel, for example `Rgba([r,g,b,a])` or `Pixel::from_slice`"
    )]
    fn from_channels(
        a: Self::Subpixel,
        b: Self::Subpixel,
        c: Self::Subpixel,
        d: Self::Subpixel,
    ) -> Self;

    /// Returns a view into a slice.
    fn from_slice(slice: &[Self::Subpixel]) -> &Self;

    /// Returns mutable view into a mutable slice.
    fn from_slice_mut(slice: &mut [Self::Subpixel]) -> &mut Self;

    /// Convert this pixel to RGB
    fn to_rgb(&self) -> Rgb<Self::Subpixel>;

    /// Convert this pixel to RGB with an alpha channel
    fn to_rgba(&self) -> Rgba<Self::Subpixel>;

    /// Convert this pixel to luma
    fn to_luma(&self) -> Luma<Self::Subpixel>;

    /// Convert this pixel to luma with an alpha channel
    fn to_luma_alpha(&self) -> LumaA<Self::Subpixel>;

    /// Apply the function `f` to each channel of this pixel.
    fn map<F>(&self, f: F) -> Self
    where
        F: FnMut(Self::Subpixel) -> Self::Subpixel;

    /// Apply the function `f` to each channel of this pixel.
    fn apply<F>(&mut self, f: F)
    where
        F: FnMut(Self::Subpixel) -> Self::Subpixel;

    /// Apply the function `f` to each channel except the alpha channel.
    /// Apply the function `g` to the alpha channel.
    fn map_with_alpha<F, G>(&self, f: F, g: G) -> Self
    where
        F: FnMut(Self::Subpixel) -> Self::Subpixel,
        G: FnMut(Self::Subpixel) -> Self::Subpixel;

    /// Apply the function `f` to each channel except the alpha channel.
    /// Apply the function `g` to the alpha channel. Works in-place.
    fn apply_with_alpha<F, G>(&mut self, f: F, g: G)
    where
        F: FnMut(Self::Subpixel) -> Self::Subpixel,
        G: FnMut(Self::Subpixel) -> Self::Subpixel;

    /// Apply the function `f` to each channel except the alpha channel.
    fn map_without_alpha<F>(&self, f: F) -> Self
    where
        F: FnMut(Self::Subpixel) -> Self::Subpixel,
    {
        let mut this = *self;
        this.apply_with_alpha(f, |x| x);
        this
    }

    /// Apply the function `f` to each channel except the alpha channel.
    fn apply_without_alpha<F>(&mut self, f: F)
    where
        F: FnMut(Self::Subpixel) -> Self::Subpixel,
    {
        self.apply_with_alpha(f, |x| x);
    }

    /// Apply the function `f` to each channel of this pixel and
    /// `other` pairwise.
    fn map2<F>(&self, other: &Self, f: F) -> Self
    where
        F: FnMut(Self::Subpixel, Self::Subpixel) -> Self::Subpixel;

    /// Apply the function `f` to each channel of this pixel and
    /// `other` pairwise. Works in-place.
    fn apply2<F>(&mut self, other: &Self, f: F)
    where
        F: FnMut(Self::Subpixel, Self::Subpixel) -> Self::Subpixel;

    /// Invert this pixel
    fn invert(&mut self);

    /// Blend the color of a given pixel into ourself, taking into account alpha channels
    fn blend(&mut self, other: &Self);
}

// ---------------------------------------------------------------------------
// GenericImageView
// ---------------------------------------------------------------------------

/// Trait to inspect an image.
pub trait GenericImageView {
    /// The type of pixel.
    type Pixel: Pixel;

    /// The width and height of this image.
    fn dimensions(&self) -> (u32, u32);

    /// The width of this image.
    fn width(&self) -> u32 {
        let (w, _) = self.dimensions();
        w
    }

    /// The height of this image.
    fn height(&self) -> u32 {
        let (_, h) = self.dimensions();
        h
    }

    /// Returns true if this x, y coordinate is contained inside the image.
    fn in_bounds(&self, x: u32, y: u32) -> bool {
        let (width, height) = self.dimensions();
        x < width && y < height
    }

    /// Returns the pixel located at (x, y). Indexed from top left.
    ///
    /// # Panics
    ///
    /// Panics if `(x, y)` is out of bounds.
    fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel;

    /// Returns the pixel located at (x, y). Indexed from top left.
    ///
    /// This function can be implemented in a way that ignores bounds checking.
    ///
    /// # Safety
    ///
    /// The coordinates must be [`in_bounds`] of the image.
    unsafe fn unsafe_get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
        self.get_pixel(x, y)
    }

    /// Returns an Iterator over the pixels of this image.
    /// The iterator yields the coordinates of each pixel
    /// along with their value
    fn pixels(&self) -> Pixels<'_, Self>
    where
        Self: Sized,
    {
        let (width, height) = self.dimensions();
        Pixels {
            image: self,
            x: 0,
            y: 0,
            width,
            height,
        }
    }

    /// Create an empty [`ImageBuffer`] with the same pixel type as this image.
    fn buffer_like(&self) -> ImageBuffer<Self::Pixel, Vec<<Self::Pixel as Pixel>::Subpixel>> {
        let (w, h) = self.dimensions();
        ImageBuffer::new(w, h)
    }

    /// Create an empty [`ImageBuffer`] with different dimensions.
    fn buffer_with_dimensions(
        &self,
        width: u32,
        height: u32,
    ) -> ImageBuffer<Self::Pixel, Vec<<Self::Pixel as Pixel>::Subpixel>> {
        ImageBuffer::new(width, height)
    }
}

/// Immutable pixel iterator over a GenericImageView.
#[derive(Debug)]
pub struct Pixels<'a, I: ?Sized + 'a> {
    image: &'a I,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl<I: GenericImageView> Iterator for Pixels<'_, I> {
    type Item = (u32, u32, I::Pixel);

    fn next(&mut self) -> Option<(u32, u32, I::Pixel)> {
        if self.x >= self.width {
            self.x = 0;
            self.y += 1;
        }

        if self.y >= self.height {
            None
        } else {
            let pixel = self.image.get_pixel(self.x, self.y);
            let p = (self.x, self.y, pixel);
            self.x += 1;
            Some(p)
        }
    }
}

impl<I: ?Sized> Clone for Pixels<'_, I> {
    fn clone(&self) -> Self {
        Pixels { ..*self }
    }
}

// ---------------------------------------------------------------------------
// GenericImage
// ---------------------------------------------------------------------------

/// A trait for manipulating images.
pub trait GenericImage: GenericImageView {
    /// Gets a reference to the mutable pixel at location (x, y).
    #[deprecated(since = "0.24.0", note = "Use `get_pixel` and `put_pixel` instead.")]
    fn get_pixel_mut(&mut self, x: u32, y: u32) -> &mut Self::Pixel;

    /// Put a pixel at location (x, y). Indexed from top left.
    ///
    /// # Panics
    ///
    /// Panics if `(x, y)` is out of bounds.
    fn put_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel);

    /// Puts a pixel at location (x, y), ignoring bounds checking.
    ///
    /// # Safety
    ///
    /// The coordinates must be [`in_bounds`] of the image.
    unsafe fn unsafe_put_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
        self.put_pixel(x, y, pixel);
    }

    /// Put a pixel at location (x, y), taking into account alpha channels
    #[deprecated(
        since = "0.24.0",
        note = "Use iterator `pixels_mut` to blend the pixels directly"
    )]
    fn blend_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel);

    /// Copies all of the pixels from another image into this image.
    fn copy_from<O>(&mut self, other: &O, x: u32, y: u32) -> Result<(), super::ImageError>
    where
        O: GenericImageView<Pixel = Self::Pixel>,
    {
        let (width, height) = other.dimensions();

        // Check bounds
        if x + width > self.width() || y + height > self.height() {
            return Err(super::ImageError::Dimensions);
        }

        for k in 0..height {
            for i in 0..width {
                let p = other.get_pixel(i, k);
                self.put_pixel(i + x, k + y, p);
            }
        }

        Ok(())
    }

    /// Copies all of the pixels from one part of this image to another part of this image.
    fn copy_within(&mut self, source: super::Rect, x: u32, y: u32) -> bool {
        let (sx, sy, width, height) = (source.x, source.y, source.width, source.height);
        let dx = x;
        let dy = y;
        if sx >= self.width() || dx >= self.width() {
            return false;
        }
        if sy >= self.height() || dy >= self.height() {
            return false;
        }
        if self.width() - dx.max(sx) < width || self.height() - dy.max(sy) < height {
            return false;
        }

        match (sx < dx, sy < dy) {
            (true, true) => {
                for y in (0..height).rev() {
                    let sy = sy + y;
                    let dy = dy + y;
                    for x in (0..width).rev() {
                        let sx = sx + x;
                        let dx = dx + x;
                        let pixel = self.get_pixel(sx, sy);
                        self.put_pixel(dx, dy, pixel);
                    }
                }
            }
            (true, false) => {
                for y in 0..height {
                    let sy = sy + y;
                    let dy = dy + y;
                    for x in (0..width).rev() {
                        let sx = sx + x;
                        let dx = dx + x;
                        let pixel = self.get_pixel(sx, sy);
                        self.put_pixel(dx, dy, pixel);
                    }
                }
            }
            (false, true) => {
                for y in (0..height).rev() {
                    let sy = sy + y;
                    let dy = dy + y;
                    for x in 0..width {
                        let sx = sx + x;
                        let dx = dx + x;
                        let pixel = self.get_pixel(sx, sy);
                        self.put_pixel(dx, dy, pixel);
                    }
                }
            }
            (false, false) => {
                for y in 0..height {
                    let sy = sy + y;
                    let dy = dy + y;
                    for x in 0..width {
                        let sx = sx + x;
                        let dx = dx + x;
                        let pixel = self.get_pixel(sx, sy);
                        self.put_pixel(dx, dy, pixel);
                    }
                }
            }
        }
        true
    }
}
