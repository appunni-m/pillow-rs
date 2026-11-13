//! Native rgb_premultiplied8 ssse3 vector traversal with its mode semantics already selected.

use super::super::super::helpers::interleaved::grayscale_premultiplied_rgba;
use super::super::default::grayscale_impl as scalar;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(input: scalar::Kernel<'_>) -> (Vec<u8>, u64, u64) {
    grayscale_premultiplied_rgba(input.input.as_raw())
}
