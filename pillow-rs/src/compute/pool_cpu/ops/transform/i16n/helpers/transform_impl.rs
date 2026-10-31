//! Native typed affine Transform route for exact mode ImageI16N.

use crate::compute::pool_cpu::ops::transform::helpers::luma16_mode_vector;

luma16_mode_vector!(ImageI16N, Native, "I;16N", "Transform I;16N output shape mismatch");
