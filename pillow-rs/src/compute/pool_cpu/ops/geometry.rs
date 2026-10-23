//! Compatibility facade for CPU geometry operation registration and callers.

pub(crate) use super::crop::execute_crop;
pub(crate) use super::reduce::execute_reduce;
pub(crate) use super::resize::{execute_resize, execute_resize_boxed, resample_kernel};
pub(crate) use super::rotate::{
    execute_rotate, rotate_cubic_horizontal_i32, rotate_cubic_vertical_f64,
};
pub(crate) use super::thumbnail::{
    execute_rgb_putpixel_thumbnail_fusion, execute_thumbnail,
    rgb_putpixel_thumbnail_fusion_supported,
};
pub(crate) use super::transpose::execute_transpose;
