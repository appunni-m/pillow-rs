use super::primitive::Primitive;
use crate::types::color::{Luma, LumaA, Rgb, Rgba};

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
            let lum_alpha = self.to_luma_alpha();
            let channels = lum_alpha.channels();
            // channels() always returns a non-empty slice for any valid ColorType
            *channels.last().unwrap_or(&channels[0])
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
