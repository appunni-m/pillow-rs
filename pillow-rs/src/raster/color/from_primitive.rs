//! `FromPrimitive` trait + implementations + luminance helpers.

use crate::raster::traits::primitive::saturating_trunc_f32_to_u128;
use crate::raster::traits::{Enlargeable, Primitive};

/// Converts a numeric component into a pixel subcomponent without changing layout.
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
        u8::try_from(saturating_trunc_f32_to_u128(normalize_float(
            float,
            f32::from(u8::MAX),
        )))
        .unwrap_or(u8::MAX)
    }
}

impl FromPrimitive<f32> for u16 {
    fn from_primitive(float: f32) -> Self {
        u16::try_from(saturating_trunc_f32_to_u128(normalize_float(
            float,
            f32::from(u16::MAX),
        )))
        .unwrap_or(u16::MAX)
    }
}

// From u16:
impl FromPrimitive<u16> for u8 {
    fn from_primitive(c16: u16) -> Self {
        let rounded = u32::from(c16).saturating_add(128);
        let scaled = rounded.checked_div(257).unwrap_or_default();
        u8::try_from(scaled).unwrap_or(u8::MAX)
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
        u16::from(c8).saturating_mul(257)
    }
}

#[inline]
pub(super) fn normalize_float(float: f32, max: f32) -> f32 {
    let clamped = if matches!(float.partial_cmp(&1.0), Some(std::cmp::Ordering::Less)) {
        float.max(0.0)
    } else {
        1.0
    };
    clamped.mul_add(max, 0.0).round()
}

// ---------------------------------------------------------------------------
// Color conversion coefficients
// ---------------------------------------------------------------------------

/// Pillow/libImaging fixed-point RGB-to-luminance coefficients.
const SRGB_LUMA: [u32; 3] = [19_595, 38_470, 7_471];
const SRGB_LUMA_DIV: u32 = 65_536;

#[inline]
pub(super) fn rgb_to_luma<T: Primitive + Enlargeable>(rgb: &[T]) -> T {
    let luma = rgb[0].to_f32() * (SRGB_LUMA[0] as f32 / SRGB_LUMA_DIV as f32)
        + rgb[1].to_f32() * (SRGB_LUMA[1] as f32 / SRGB_LUMA_DIV as f32)
        + rgb[2].to_f32() * (SRGB_LUMA[2] as f32 / SRGB_LUMA_DIV as f32);
    let rounded = luma + 0.5 * f32::from(u8::from(T::DEFAULT_MAX_VALUE.to_f32() > 1.0));
    let l = <T::Larger as Primitive>::from_f32(rounded);
    T::clamp_from(l)
}
