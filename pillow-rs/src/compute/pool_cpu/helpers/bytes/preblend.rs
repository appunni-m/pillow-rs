/// Apply Pillow's byte `PREBLEND` operation to a premultiplied source sample.
#[expect(
    clippy::arithmetic_side_effects,
    reason = "byte PREBLEND intermediates are bounded below u32::MAX"
)]
#[inline]
pub(crate) fn apply(source: u8, destination: u8, mask: u8) -> u8 {
    let inverse = 255 - u32::from(mask);
    let scaled = u32::from(destination) * inverse + 128;
    let value = (((scaled >> 8) + scaled) >> 8) + u32::from(source);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "Pillow PREBLEND stores the resulting byte modulo 256"
    )]
    {
        value as u8
    }
}
