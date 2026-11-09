use crate::error::PilError;

/// Numeric component kind retained while P Draw resolves a palette tuple.
#[derive(Debug, Clone, PartialEq)]
pub enum PaletteColorComponent {
    /// An integer that fits the core tuple carrier.
    Integer(i64),
    /// An arbitrary-size integer outside the core tuple carrier, stored as a
    /// normalized sign and big-endian magnitude.
    WideInteger {
        /// Whether the original integer was negative.
        negative: bool,
        /// Absolute value as a normalized big-endian byte sequence.
        magnitude: Vec<u8>,
    },
    /// A floating-point component whose kind affects `bytes(tuple)` errors.
    Float(f64),
}

impl PaletteColorComponent {
    /// Converts an integral floating-point input to its exact integer value.
    #[doc(hidden)]
    pub fn from_integral_f64(value: f64) -> Option<Self> {
        if !value.is_finite() || value.fract() != 0.0 {
            return None;
        }
        if value >= i64::MIN as f64 && value < -(i64::MIN as f64) {
            return Some(Self::Integer(value as i64));
        }
        let (negative, magnitude) = float_integer_magnitude(value)?;
        Some(Self::WideInteger {
            negative,
            magnitude,
        })
    }
}

fn float_integer_magnitude(value: f64) -> Option<(bool, Vec<u8>)> {
    if !value.is_finite() || value.fract() != 0.0 {
        return None;
    }
    if value == 0.0 {
        return Some((false, Vec::new()));
    }

    let bits = value.abs().to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32 - 1023;
    let significand = (bits & ((1_u64 << 52) - 1)) | (1_u64 << 52);
    let (integer, shift) = if exponent < 52 {
        (significand >> (52 - exponent) as u32, 0)
    } else {
        (significand, (exponent - 52) as usize)
    };
    let mut magnitude = integer.to_be_bytes().to_vec();
    while magnitude.first() == Some(&0) {
        magnitude.remove(0);
    }
    if shift != 0 {
        let byte_shift = shift / 8;
        let bit_shift = shift % 8;
        if bit_shift != 0 {
            let mut carry = 0_u16;
            for byte in magnitude.iter_mut().rev() {
                let shifted = (u16::from(*byte) << bit_shift) | carry;
                *byte = shifted as u8;
                carry = shifted >> 8;
            }
            if carry != 0 {
                magnitude.insert(0, carry as u8);
            }
        }
        magnitude.resize(magnitude.len() + byte_shift, 0);
    }
    Some((value.is_sign_negative(), magnitude))
}

fn integer_magnitude(value: i64) -> (bool, Vec<u8>) {
    if value == 0 {
        return (false, Vec::new());
    }
    let magnitude = value.unsigned_abs().to_be_bytes();
    let first = magnitude
        .iter()
        .position(|byte| *byte != 0)
        .expect("nonzero integer has a nonzero magnitude byte");
    (value.is_negative(), magnitude[first..].to_vec())
}

fn wide_integer_equals(
    negative: bool,
    magnitude: &[u8],
    other_negative: bool,
    other_magnitude: &[u8],
) -> bool {
    negative == other_negative && magnitude == other_magnitude
}

pub(super) fn palette_components_equal(
    left: &[PaletteColorComponent],
    right: &[PaletteColorComponent],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| match (left, right) {
            (PaletteColorComponent::Integer(left), PaletteColorComponent::Integer(right)) => {
                left == right
            }
            (
                PaletteColorComponent::WideInteger {
                    negative: left_negative,
                    magnitude: left_magnitude,
                },
                PaletteColorComponent::WideInteger {
                    negative: right_negative,
                    magnitude: right_magnitude,
                },
            ) => wide_integer_equals(
                *left_negative,
                left_magnitude,
                *right_negative,
                right_magnitude,
            ),
            (PaletteColorComponent::Float(left), PaletteColorComponent::Float(right)) => {
                left == right
            }
            (PaletteColorComponent::Integer(integer), PaletteColorComponent::Float(float))
            | (PaletteColorComponent::Float(float), PaletteColorComponent::Integer(integer)) => {
                float.is_finite()
                    && float.fract() == 0.0
                    && *float >= i64::MIN as f64
                    && *float < -(i64::MIN as f64)
                    && (*float as i64) == *integer
            }
            (
                PaletteColorComponent::WideInteger {
                    negative,
                    magnitude,
                },
                PaletteColorComponent::Float(float),
            )
            | (
                PaletteColorComponent::Float(float),
                PaletteColorComponent::WideInteger {
                    negative,
                    magnitude,
                },
            ) => float_integer_magnitude(*float).is_some_and(|(float_negative, float_magnitude)| {
                wide_integer_equals(*negative, magnitude, float_negative, &float_magnitude)
            }),
            (
                PaletteColorComponent::WideInteger {
                    negative,
                    magnitude,
                },
                PaletteColorComponent::Integer(integer),
            )
            | (
                PaletteColorComponent::Integer(integer),
                PaletteColorComponent::WideInteger {
                    negative,
                    magnitude,
                },
            ) => {
                let (integer_negative, integer_magnitude) = integer_magnitude(*integer);
                wide_integer_equals(*negative, magnitude, integer_negative, &integer_magnitude)
            }
        })
}

pub(super) fn palette_component_matches_byte(component: &PaletteColorComponent, byte: u8) -> bool {
    match component {
        PaletteColorComponent::Integer(value) => *value == i64::from(byte),
        PaletteColorComponent::WideInteger { .. } => false,
        PaletteColorComponent::Float(value) => *value == f64::from(byte),
    }
}

pub(super) fn palette_component_to_byte(component: &PaletteColorComponent) -> Result<u8, PilError> {
    match component {
        PaletteColorComponent::Integer(value) => u8::try_from(*value)
            .map_err(|_| PilError::ValueError("bytes must be in range(0, 256)".into())),
        PaletteColorComponent::WideInteger { .. } => Err(PilError::ValueError(
            "bytes must be in range(0, 256)".into(),
        )),
        PaletteColorComponent::Float(_) => Err(PilError::TypeError(
            "'float' object cannot be interpreted as an integer".into(),
        )),
    }
}

pub(super) fn expand_palette_metadata(palette: Vec<u8>, rgba: bool) -> (Vec<u8>, Option<Vec<u8>>) {
    if !rgba {
        return (palette, None);
    }
    let mut rgb = Vec::with_capacity(palette.len() / 4 * 3);
    let mut alpha = Vec::with_capacity(palette.len() / 4);
    for color in palette.chunks_exact(4) {
        rgb.extend_from_slice(&color[..3]);
        alpha.push(color[3]);
    }
    (rgb, Some(alpha))
}
