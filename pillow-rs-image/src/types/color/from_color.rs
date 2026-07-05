//! `FromColor` trait + cross-type conversions.

use super::from_primitive::{FromPrimitive, rgb_to_luma};
use super::types::{Luma, LumaA, Rgb, Rgba};
use crate::types::traits::{Enlargeable, Pixel, Primitive};

pub trait FromColor<Other> {
    /// Changes `self` to represent `Other` in the color space of `Self`.
    fn from_color(&mut self, _: &Other);
}

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
