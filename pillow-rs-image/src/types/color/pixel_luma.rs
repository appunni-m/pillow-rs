//! `Pixel` trait implementations for `Luma<T>` and `LumaA<T>`.

use super::blend::Blend;
use super::from_color::FromColor;
use super::invert::Invert;
use super::types::{Luma, LumaA, Rgb, Rgba};
use crate::types::traits::{Pixel, Primitive};

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
        (
            c[0],
            T::DEFAULT_MAX_VALUE,
            T::DEFAULT_MAX_VALUE,
            T::DEFAULT_MAX_VALUE,
        )
    }
    fn from_channels(a: T, _b: T, _c: T, _d: T) -> Luma<T> {
        Luma([a])
    }

    fn from_slice(slice: &[T]) -> &Luma<T> {
        bytemuck::cast_ref(<&[T; 1]>::try_from(slice).expect("Luma expects 1 channel"))
    }
    fn from_slice_mut(slice: &mut [T]) -> &mut Luma<T> {
        bytemuck::cast_mut(<&mut [T; 1]>::try_from(slice).expect("Luma expects 1 channel"))
    }

    fn to_rgb(&self) -> Rgb<T> {
        let mut pix = Rgb([T::DEFAULT_MIN_VALUE; 3]);
        pix.from_color(self);
        pix
    }
    fn to_rgba(&self) -> Rgba<T> {
        let mut pix = Rgba([T::DEFAULT_MIN_VALUE; 4]);
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
    fn map<F>(&self, f: F) -> Luma<T>
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
    fn map_with_alpha<F, G>(&self, f: F, _g: G) -> Luma<T>
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
    fn map2<F>(&self, other: &Luma<T>, f: F) -> Luma<T>
    where
        F: FnMut(T, T) -> T,
    {
        let mut this = *self;
        this.apply2(other, f);
        this
    }
    fn apply2<F>(&mut self, other: &Luma<T>, mut f: F)
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
        bytemuck::cast_ref(<&[T; 2]>::try_from(slice).expect("LumaA expects 2 channels"))
    }
    fn from_slice_mut(slice: &mut [T]) -> &mut LumaA<T> {
        bytemuck::cast_mut(<&mut [T; 2]>::try_from(slice).expect("LumaA expects 2 channels"))
    }

    fn to_rgb(&self) -> Rgb<T> {
        let mut pix = Rgb([T::DEFAULT_MIN_VALUE; 3]);
        pix.from_color(self);
        pix
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
        *self
    }
    fn map<F>(&self, f: F) -> LumaA<T>
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
    fn map_with_alpha<F, G>(&self, f: F, g: G) -> LumaA<T>
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
        self.0[0] = f(self.0[0]);
        self.0[1] = g(self.0[1]);
    }
    fn map2<F>(&self, other: &LumaA<T>, f: F) -> LumaA<T>
    where
        F: FnMut(T, T) -> T,
    {
        let mut this = *self;
        this.apply2(other, f);
        this
    }
    fn apply2<F>(&mut self, other: &LumaA<T>, mut f: F)
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
    fn blend(&mut self, other: &LumaA<T>) {
        Blend::blend(self, other);
    }
}
