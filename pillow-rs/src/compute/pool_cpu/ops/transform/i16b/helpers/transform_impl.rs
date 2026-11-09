//! Native typed affine Transform route for exact mode ImageI16B.

use crate::compute::pool_cpu::ops::transform::helpers::luma16_mode_vector;

luma16_mode_vector!(
    ImageI16B,
    Big,
    "I;16B",
    "Transform I;16B output shape mismatch"
);
