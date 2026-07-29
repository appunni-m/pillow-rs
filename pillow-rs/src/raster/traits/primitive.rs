/// The type of each channel in a pixel. For example, this can be `u8`, `u16`, `f32`.
// AS PER DESIGN: Pod + Zeroable enables safe bytemuck casts, eliminating unsafe blocks.
pub trait Primitive:
    Copy + Clone + PartialOrd + Sized + Default + bytemuck::Pod + bytemuck::Zeroable
{
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
                <$ty>::try_from(saturating_trunc_f32_to_u128(clamped)).unwrap_or(<$ty>::MAX)
            }

            #[inline]
            fn to_u64(self) -> u64 {
                u64::try_from(self).unwrap_or(u64::MAX)
            }

            #[inline]
            fn from_u64(val: u64) -> Self {
                <$ty>::try_from(val).unwrap_or(<$ty>::MAX)
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
        u64::try_from(saturating_trunc_f32_to_u128(self)).unwrap_or(u64::MAX)
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
        f64_to_f32(self)
    }

    #[inline]
    fn from_f32(val: f32) -> Self {
        val as f64
    }

    #[inline]
    fn to_u64(self) -> u64 {
        u64::try_from(saturating_trunc_f64_to_u128(self)).unwrap_or(u64::MAX)
    }

    #[inline]
    fn from_u64(val: u64) -> Self {
        (val as f64).clamp(0.0, 1.0)
    }
}

/// An `Enlargeable::Larger` value should be enough to calculate
/// the sum (average) of a few hundred or thousand Enlargeable values.
pub trait Enlargeable: Primitive {
    /// Wider component type used for intermediate arithmetic.
    type Larger: Primitive;

    /// Narrows an intermediate value while clamping it to this component's range.
    fn clamp_from(n: Self::Larger) -> Self;
    /// Widens this component for intermediate arithmetic.
    fn to_larger(self) -> Self::Larger;
}

impl Enlargeable for u8 {
    type Larger = u32;

    #[inline]
    fn clamp_from(n: Self::Larger) -> Self {
        u8::try_from(n).unwrap_or(u8::MAX)
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
        u16::try_from(n).unwrap_or(u16::MAX)
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
        u32::try_from(n).unwrap_or(u32::MAX)
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
        u64::try_from(n).unwrap_or(u64::MAX)
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
        usize::try_from(n).unwrap_or(usize::MAX)
    }

    #[inline]
    fn to_larger(self) -> Self::Larger {
        self as u128
    }
}

/// Convert a non-negative `f32` to an integer with Rust's saturating cast
/// semantics, without hiding a narrowing conversion from Clippy.
pub(crate) fn saturating_trunc_f32_to_u128(value: f32) -> u128 {
    if value.is_nan() || value <= 0.0 {
        return 0;
    }
    if value.is_infinite() {
        return u128::MAX;
    }

    let bits = value.to_bits();
    let biased_exponent = (bits >> 23) & 0xff;
    if biased_exponent == 0 {
        return 0;
    }
    let exponent = i32::try_from(biased_exponent)
        .unwrap_or_default()
        .saturating_sub(127);
    if exponent < 0 {
        return 0;
    }
    let significand = u128::from((bits & 0x7f_ffff) | 0x80_0000);
    if exponent >= 23 {
        significand << u32::try_from(exponent.saturating_sub(23)).unwrap_or_default()
    } else {
        significand >> u32::try_from(23i32.saturating_sub(exponent)).unwrap_or_default()
    }
}

fn saturating_trunc_f64_to_u128(value: f64) -> u128 {
    if value.is_nan() || value <= 0.0 {
        return 0;
    }
    if value.is_infinite() {
        return u128::MAX;
    }

    let bits = value.to_bits();
    let biased_exponent = (bits >> 52) & 0x7ff;
    if biased_exponent == 0 {
        return 0;
    }
    let exponent = i32::try_from(biased_exponent)
        .unwrap_or_default()
        .saturating_sub(1023);
    if exponent < 0 {
        return 0;
    }
    if exponent >= 128 {
        return u128::MAX;
    }

    let significand = u128::from((bits & 0x000f_ffff_ffff_ffff) | 0x0010_0000_0000_0000);
    if exponent >= 52 {
        significand << u32::try_from(exponent.saturating_sub(52)).unwrap_or_default()
    } else {
        significand >> u32::try_from(52i32.saturating_sub(exponent)).unwrap_or_default()
    }
}

fn f64_to_f32(value: f64) -> f32 {
    let bits = value.to_bits();
    let sign = u32::try_from(bits >> 32).unwrap_or_default() & 0x8000_0000;
    let biased_exponent = (bits >> 52) & 0x7ff;
    let fraction = bits & 0x000f_ffff_ffff_ffff;

    if biased_exponent == 0x7ff {
        let payload = if fraction == 0 {
            0
        } else {
            let narrowed = u32::try_from(fraction >> 29).unwrap_or_default();
            narrowed | 0x0040_0000
        };
        return f32::from_bits(sign | 0x7f80_0000 | payload);
    }
    if biased_exponent == 0 {
        return f32::from_bits(sign);
    }

    let mut exponent = i32::try_from(biased_exponent)
        .unwrap_or_default()
        .saturating_sub(1023);
    if exponent > 127 {
        return f32::from_bits(sign | 0x7f80_0000);
    }
    if exponent < -149 {
        return f32::from_bits(sign);
    }

    let significand = (1u64 << 52) | fraction;
    if exponent >= -126 {
        let mut rounded = round_shift_right(significand, 29);
        if rounded == (1u64 << 24) {
            rounded >>= 1;
            exponent = exponent.saturating_add(1);
            if exponent > 127 {
                return f32::from_bits(sign | 0x7f80_0000);
            }
        }
        let target_exponent = u32::try_from(exponent.saturating_add(127)).unwrap_or_default();
        let target_fraction = u32::try_from(rounded & 0x007f_ffff).unwrap_or_default();
        return f32::from_bits(sign | (target_exponent << 23) | target_fraction);
    }

    let shift = u32::try_from((-97i32).saturating_sub(exponent)).unwrap_or_default();
    let rounded = round_shift_right(significand, shift);
    let target_fraction = u32::try_from(rounded).unwrap_or(0x0080_0000);
    f32::from_bits(sign | target_fraction)
}

fn round_shift_right(value: u64, shift: u32) -> u64 {
    let quotient = value >> shift;
    let mask = (1u64 << shift).saturating_sub(1);
    let remainder = value & mask;
    let halfway = 1u64 << shift.saturating_sub(1);
    if remainder > halfway || (remainder == halfway && quotient & 1 == 1) {
        quotient.saturating_add(1)
    } else {
        quotient
    }
}

impl Enlargeable for f32 {
    type Larger = f64;

    #[inline]
    fn clamp_from(n: Self::Larger) -> Self {
        f64_to_f32(n.clamp(f64::from(f32::MIN), f64::from(f32::MAX)))
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
        bytemuck::cast_slice(self)
    }
}

impl EncodableLayout for [f32] {
    fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(self)
    }
}

mod seals {
    pub trait EncodableLayout {}
    impl EncodableLayout for [u8] {}
    impl EncodableLayout for [u16] {}
    impl EncodableLayout for [f32] {}
}
