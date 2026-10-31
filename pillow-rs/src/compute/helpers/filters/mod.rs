//! Filter request facts shared across compute backends.

mod parameters;

pub(crate) use parameters::{
    binomial3x3_parameters_match, binomial5x5_parameters_match,
    box_blur_parameters, gaussian_blur_box_parameters, gaussian_blur_three_pass_radius,
    la_single_outlier5x5_parameters_match, uniform5x5_parameters_match,
};
