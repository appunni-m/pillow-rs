//! Native typed affine Transform route for exact mode ImageI16.

use crate::compute::pool_cpu::ops::transform::helpers::luma16_mode_vector;

luma16_mode_vector!(
    ImageI16,
    Native,
    "I;16",
    "Transform I;16 output shape mismatch"
);
