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
