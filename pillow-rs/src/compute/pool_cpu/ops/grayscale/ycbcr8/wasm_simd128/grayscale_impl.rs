//! Wasm SIMD128 gathers native Y samples without changing mode or layout.

use super::super::default::grayscale_impl as scalar;
use crate::compute::pool_cpu::helpers::channel::wide as channel;

pub(crate) fn execute(input: scalar::Kernel<'_>) -> (Vec<u8>, u64, u64) {
    channel::gather::<3, 0>(input.as_raw())
}
