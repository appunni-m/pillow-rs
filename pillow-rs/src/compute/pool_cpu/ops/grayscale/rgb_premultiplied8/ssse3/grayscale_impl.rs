//! Native rgb_premultiplied8 ssse3 vector traversal with its mode semantics already selected.

use super::super::default::grayscale_impl as scalar;
use super::super::super::helpers::interleaved::grayscale_interleaved;

pub(crate) fn execute(input: scalar::Kernel<'_>) -> (Vec<u8>, u64, u64) {
    let straight = crate::ops::pil_resize::unpremultiply_rgba(input.input);
    let source = straight.as_raw();
    let output = grayscale_interleaved::<4>(source);
    (output, input.dimensions.total_pixels().div_ceil(16) as u64, 0)
}
