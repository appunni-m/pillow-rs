//! Pillow-compatible byte interpretation for native `I;16*` samples.

/// Reads the two native sample bytes using Pillow's selected byte order.
#[inline(always)]
pub(crate) fn read<const BIG_ENDIAN: bool>(sample: u16) -> u16 {
    let native_bytes = sample.to_ne_bytes();
    if BIG_ENDIAN {
        u16::from_be_bytes(native_bytes)
    } else {
        u16::from_le_bytes(native_bytes)
    }
}

/// Applies Pillow's ordered round and full-sample clipping to one u16 sample.
#[inline(always)]
pub(crate) fn write<const BIG_ENDIAN: bool>(value: f64) -> u16 {
    let rounded = super::rounding::round_up(value) as i64;
    let sample = rounded.clamp(0, i64::from(u16::MAX)) as u16;
    let bytes = if BIG_ENDIAN {
        sample.to_be_bytes()
    } else {
        sample.to_le_bytes()
    };
    u16::from_ne_bytes(bytes)
}
