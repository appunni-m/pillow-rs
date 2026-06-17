//! Private `Blend` trait for alpha compositing.

use super::types::{Luma, LumaA, Rgb, Rgba};
use crate::types::traits::Primitive;

pub(super) trait Blend {
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
