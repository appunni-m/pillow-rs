//! Original RGB wide loop in the 32-bit x86 AVX2 build configuration.

use super::super::super::helpers::interleaved::grayscale_interleaved;
use super::super::default::grayscale_impl as scalar;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(input: scalar::Kernel<'_>) -> (Vec<u8>, u64, u64) {
    let output = grayscale_interleaved::<3>(input.input.as_raw());
    (
        output,
        input.dimensions.total_pixels().div_ceil(16) as u64,
        0,
    )
}
