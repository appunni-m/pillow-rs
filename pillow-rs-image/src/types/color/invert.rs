//! Private `Invert` trait for color inversion.

use super::types::{Luma, LumaA, Rgb, Rgba};
use crate::types::traits::Primitive;

pub(super) trait Invert {
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
