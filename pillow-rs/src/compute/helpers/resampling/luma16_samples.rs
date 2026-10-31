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

fn clip_u8(value: i64) -> u8 {
    value.clamp(0, 255) as u8
}

/// Applies Pillow's ordered round and per-byte clipping to one sample.
#[inline(always)]
pub(crate) fn write<const BIG_ENDIAN: bool>(value: f64) -> u16 {
    let rounded = super::rounding::round_up(value) as i64;
    // Resample.c clips each output byte independently rather than clipping the
    // complete u16 value, including for negative-filter overshoot.
    let low = clip_u8(rounded % 256);
    let high = clip_u8(rounded >> 8);
    let bytes = if BIG_ENDIAN { [high, low] } else { [low, high] };
    u16::from_ne_bytes(bytes)
}
