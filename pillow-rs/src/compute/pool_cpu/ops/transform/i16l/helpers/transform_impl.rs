//! Native typed affine Transform route for exact mode ImageI16L.

use crate::compute::pool_cpu::ops::transform::helpers::luma16_mode_vector;

luma16_mode_vector!(ImageI16L, Little, "I;16L", "Transform I;16L output shape mismatch");
