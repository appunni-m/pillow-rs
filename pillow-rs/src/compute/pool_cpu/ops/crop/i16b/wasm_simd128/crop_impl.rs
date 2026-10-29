//! wasm_simd128 selected native I;16 crop copy.

use crate::compute::pool_cpu::ops::crop::helpers::{crop::CropRegion, vector_copy};
use crate::error::PilError;

pub(crate) type Input = super::super::default::crop_impl::Input;

pub(crate) fn execute(image: &Input, region: CropRegion) -> Result<Input, PilError> {
    vector_copy::crop_luma16(image, region)
}
