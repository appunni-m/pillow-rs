//! CMYK ssse3 strategy with validated native storage.

use super::super::default::grayscale_impl as scalar;
use super::super::super::helpers::cmyk;

pub(crate) type Kernel<'a> = scalar::Kernel<'a>;

pub(crate) fn prepare(input: scalar::Kernel<'_>) -> Kernel<'_> {
    input
}

pub(crate) fn dimensions(input: &Kernel<'_>) -> (u32, u32) {
    input.input.dimensions()
}

pub(crate) fn execute(input: Kernel<'_>) -> (Vec<u8>, u64, u64) {
    cmyk::execute(input.input.as_raw())
}
