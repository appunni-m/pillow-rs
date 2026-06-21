//! `Pixel` trait implementations for `Rgb<T>` and `Rgba<T>`.

use super::blend::Blend;
use super::from_color::FromColor;
use super::invert::Invert;
use super::types::{Luma, LumaA, Rgb, Rgba};
use crate::types::traits::{Enlargeable, Pixel, Primitive};

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
        bytemuck::cast_ref(<&[T; 3]>::try_from(slice).expect("Rgb expects 3 channels"))
    }
    fn from_slice_mut(slice: &mut [T]) -> &mut Rgb<T> {
        bytemuck::cast_mut(<&mut [T; 3]>::try_from(slice).expect("Rgb expects 3 channels"))
    }

    fn to_rgb(&self) -> Rgb<T> {
        *self
    }
    fn to_rgba(&self) -> Rgba<T> {
        let mut pix = Rgba([T::DEFAULT_MIN_VALUE; 4]);
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
    fn map<F>(&self, f: F) -> Rgb<T>
    where
        F: FnMut(T) -> T,
    {
        let mut this = *self;
        this.apply(f);
        this
    }
    fn apply<F>(&mut self, mut f: F)
    where
        F: FnMut(T) -> T,
    {
        for v in &mut self.0 {
            *v = f(*v);
        }
    }
    fn map_with_alpha<F, G>(&self, f: F, _g: G) -> Rgb<T>
    where
        F: FnMut(T) -> T,
        G: FnMut(T) -> T,
    {
        self.map(f)
    }
    fn apply_with_alpha<F, G>(&mut self, f: F, _g: G)
    where
        F: FnMut(T) -> T,
        G: FnMut(T) -> T,
    {
        self.apply(f);
    }
    fn map2<F>(&self, other: &Rgb<T>, f: F) -> Rgb<T>
    where
        F: FnMut(T, T) -> T,
    {
        let mut this = *self;
        this.apply2(other, f);
        this
    }
    fn apply2<F>(&mut self, other: &Rgb<T>, mut f: F)
    where
        F: FnMut(T, T) -> T,
    {
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
        bytemuck::cast_ref(<&[T; 4]>::try_from(slice).expect("Rgba expects 4 channels"))
    }
    fn from_slice_mut(slice: &mut [T]) -> &mut Rgba<T> {
        bytemuck::cast_mut(<&mut [T; 4]>::try_from(slice).expect("Rgba expects 4 channels"))
    }

    fn to_rgb(&self) -> Rgb<T> {
        let mut pix = Rgb([T::DEFAULT_MIN_VALUE; 3]);
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
    fn map<F>(&self, f: F) -> Rgba<T>
    where
        F: FnMut(T) -> T,
    {
        let mut this = *self;
        this.apply(f);
        this
    }
    fn apply<F>(&mut self, mut f: F)
    where
        F: FnMut(T) -> T,
    {
        for v in &mut self.0 {
            *v = f(*v);
        }
    }
    fn map_with_alpha<F, G>(&self, f: F, g: G) -> Rgba<T>
    where
        F: FnMut(T) -> T,
        G: FnMut(T) -> T,
    {
        let mut this = *self;
        this.apply_with_alpha(f, g);
        this
    }
    fn apply_with_alpha<F, G>(&mut self, mut f: F, mut g: G)
    where
        F: FnMut(T) -> T,
        G: FnMut(T) -> T,
    {
        for v in self.0[..3].iter_mut() {
            *v = f(*v);
        }
        self.0[3] = g(self.0[3]);
    }
    fn map2<F>(&self, other: &Rgba<T>, f: F) -> Rgba<T>
    where
        F: FnMut(T, T) -> T,
    {
        let mut this = *self;
        this.apply2(other, f);
        this
    }
    fn apply2<F>(&mut self, other: &Rgba<T>, mut f: F)
    where
        F: FnMut(T, T) -> T,
    {
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
