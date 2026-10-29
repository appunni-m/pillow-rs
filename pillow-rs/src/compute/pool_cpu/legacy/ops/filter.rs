//! Filter operations extracted from image.rs execute_op().
//!
//! These functions are standalone implementations of PIL-compatible filter
//! operations that operate on DynamicImage and return new DynamicImage instances.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::image_utils::raw_bytes_to_image;
use crate::raster::DynamicImage;

#[path = "filter/uniform_image.rs"]
mod uniform_image;

#[path = "filter/blur.rs"]
mod blur;
pub(crate) use blur::{execute_box_blur, execute_box_blur_xy_with_passes, execute_gaussian_blur};

#[path = "filter/rank.rs"]
mod rank;
pub(crate) use rank::{
    execute_max_filter_with_mode, execute_median_filter_with_mode, execute_min_filter_with_mode,
    execute_rank_filter_with_mode,
};

#[path = "filter/convolution.rs"]
mod convolution;
#[path = "filter/filter3x3.rs"]
mod filter3x3;
pub(crate) use filter3x3::I32_UNIFORM_FILTER3X3_MAX_PIXELS;
pub(crate) use filter3x3::{
    binomial3x3_parameters_match, execute_filter3x3, uniform_i32_filter3x3_is_identity,
    uniform_i32_filter3x3_sample_is_identity,
};
#[path = "filter/filter5x5.rs"]
mod filter5x5;
#[cfg(all(test, not(feature = "parallel")))]
pub(crate) use filter5x5::filter_5x5_byte_row;
#[cfg(not(feature = "parallel"))]
pub(crate) use filter5x5::rgb_blur_ring5x5_parameters_match;
pub(crate) use filter5x5::{
    binomial5x5_parameters_match, execute_filter5x5, la_single_outlier5x5_parameters_match,
    uniform_i32_filter5x5_is_identity, uniform5x5_parameters_match,
};
