//! Native-sample algorithms shared by the filter mode strategies.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;

pub(crate) const BOX_BLUR_BIAS: u32 = 1 << 23;

#[path = "blur.rs"]
mod blur;
#[path = "byte_filter.rs"]
mod byte_filter;
#[path = "convolution.rs"]
mod convolution;
#[path = "filter3x3.rs"]
mod filter3x3;
#[path = "kernels/filter3x3_vector.rs"]
mod filter3x3_vector;
#[path = "kernels/filter5x5_vector.rs"]
mod filter5x5_vector;
#[path = "kernels/filter5x5_rgb_vector.rs"]
mod filter5x5_rgb_vector;
#[path = "kernels/rank_vector.rs"]
mod rank_vector_kernel;
#[path = "filter5x5.rs"]
mod filter5x5;
mod mode_leaf;
mod mode_filter;
#[path = "rank.rs"]
mod rank;
#[path = "rank_vector.rs"]
mod rank_vector;
mod extrema;
mod unsharp;
#[path = "uniform_image.rs"]
mod uniform_image;

pub(crate) use convolution::{pillow_kernel_row_3, pillow_kernel_row_5};
pub(crate) use blur::gaussian_blur_box_parameters;
pub(crate) use crate::compute::helpers::filters::binomial3x3_parameters_match;
pub(crate) use filter3x3::{
    execute_filter3x3_la_binomial_bytes, execute_filter3x3_la_binomial_vector_bytes,
    execute_filter3x3_native_bytes, execute_filter3x3_small_uniform_bytes,
};
pub(crate) use filter3x3_vector::apply as apply_filter3x3_vector;
pub(crate) use filter5x5_vector::apply as apply_filter5x5_vector;
pub(crate) use filter5x5::{
    apply_native_rows as apply_filter5x5_native_rows,
    binomial5x5_parameters_match, execute_filter5x5_native_bytes,
    execute_filter5x5_rgb_vector_fast_path, execute_filter5x5_small_uniform_bytes,
    filter_5x5_constant_bytes, filter_5x5_la_single_outlier_rows,
    filter_5x5_uniform_luma_rows, filter_5x5_uniform_rgba_identity,
    filter_5x5_uniform_rgba_rows, is_small_uniform_bytes as is_small_uniform_filter5x5_bytes,
    la_single_outlier5x5_parameters_match,
    normalized_kernel as normalized_filter5x5_kernel,
    uniform5x5_parameters_match,
};
pub(in crate::compute::pool_cpu::ops::filter) use filter5x5::{
    filter_5x5_binomial_la_rows, filter_5x5_binomial_luma_rows,
};
pub(crate) use filter5x5::rgb_blur_ring as filter5x5_rgb_blur_ring;
pub(crate) use rank::SMALL_RANK_AREA;
pub(in crate::compute::pool_cpu::ops::filter) use extrema::{
    execute_native_byte_extrema, native_byte_blur_architectures,
    native_byte_filter_architectures,
};
pub(in crate::compute::pool_cpu::ops::filter) use blur::{
    execute_native_byte_blur, validate_native_byte_blur_selection,
};
pub(crate) use rank_vector::execute_native_byte_rank;
pub(in crate::compute::pool_cpu::ops::filter) use uniform_image::{
    native_small_uniform_bytes, native_zero_bytes, SMALL_UNIFORM_BYTE_MAX_PIXELS,
};
pub(crate) use unsharp::blend_native_bytes as blend_unsharp_native_bytes;
pub(in crate::compute::pool_cpu::ops::filter) use byte_filter::execute as execute_native_bytes_filter;
pub(crate) use mode_leaf::{
    native_filter3x3_arch_leaf, native_filter3x3_architectures,
    native_filter5x5_arch_dispatch, native_filter5x5_arch_leaf,
    native_filter5x5_architectures, native_filter5x5_mode_fast_path,
    native_filter_mode_leaf,
};
pub(crate) use mode_filter::execute_mode_filter;
