//! I-mode orchestration over signed little-endian 32-bit samples.

use crate::compute::{Backend, ExecutionContext};
use crate::error::PilError;
#[cfg(target_arch = "x86_64")]
use crate::ops::pil_resize::X86FmaToken;
use crate::pipeline::ResampleFilter;
use crate::raster::{DynamicImage, GenericImageView};

pub(crate) use super::kernels::default::{
    i32_sample_bytes_from_native_storage, i32_samples_from_native_storage,
    resize_i_contiguous_eight_tap_weights, resize_i_round_up_to_i32, resize_i_sum_eight,
    resize_i_with_fma,
};

pub(crate) fn resize_i(
    image: &DynamicImage,
    width: u32,
    height: u32,
    filter: &ResampleFilter,
) -> Result<DynamicImage, PilError> {
    resize_i_in_context(
        &ExecutionContext::for_backend(Backend::Cpu),
        image,
        width,
        height,
        filter,
    )
}

pub(crate) fn resize_i_in_context(
    context: &ExecutionContext,
    image: &DynamicImage,
    width: u32,
    height: u32,
    filter: &ResampleFilter,
) -> Result<DynamicImage, PilError> {
    #[cfg(target_arch = "x86_64")]
    if let Some(fma) = X86FmaToken::from_context(context) {
        return super::kernels::fma::resize(image, width, height, filter, &fma);
    }
    let _ = context;
    super::kernels::default::resize_i_default(image, width, height, filter)
}

/// Resize an `I` image through a fractional source box.
///
/// Pillow's reducing-gap thumbnail path passes the original source box after
/// integer reduction, preserving the same two-pass rounding as ordinary I resize.
pub(crate) fn resize_i_boxed(
    image: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    box_left: f64,
    box_top: f64,
    box_right: f64,
    box_bottom: f64,
    filter: ResampleFilter,
) -> Result<DynamicImage, PilError> {
    resize_i_boxed_in_context(
        &ExecutionContext::for_backend(Backend::Cpu),
        image,
        dst_w,
        dst_h,
        box_left,
        box_top,
        box_right,
        box_bottom,
        filter,
    )
}

pub(crate) fn resize_i_boxed_in_context(
    context: &ExecutionContext,
    image: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    box_left: f64,
    box_top: f64,
    box_right: f64,
    box_bottom: f64,
    filter: ResampleFilter,
) -> Result<DynamicImage, PilError> {
    let (source_width, source_height) = image.dimensions();
    if dst_w == 0 || dst_h == 0 || source_width == 0 || source_height == 0 {
        return Ok(DynamicImage::new_rgba8(dst_w, dst_h));
    }
    if is_full_image_box(box_left, box_top, box_right, box_bottom, source_width, source_height) {
        return resize_i_in_context(context, image, dst_w, dst_h, &filter);
    }
    super::kernels::default::resize_i_boxed_native(
        image,
        (dst_w, dst_h),
        (box_left, box_top, box_right, box_bottom),
        filter,
    )
}

fn is_full_image_box(left: f64, top: f64, right: f64, bottom: f64, width: u32, height: u32) -> bool {
    left == 0.0 && top == 0.0 && right == f64::from(width) && bottom == f64::from(height)
}

pub(super) fn resize(
    context: &ExecutionContext,
    image: &DynamicImage,
    width: u32,
    height: u32,
    filter: &ResampleFilter,
) -> Result<DynamicImage, PilError> {
    resize_i_in_context(context, image, width, height, filter)
}

pub(super) fn resize_boxed(
    context: &ExecutionContext,
    image: &DynamicImage,
    width: u32,
    height: u32,
    bounds: (f64, f64, f64, f64),
    filter: &ResampleFilter,
) -> Result<DynamicImage, PilError> {
    resize_i_boxed_in_context(
        context, image, width, height, bounds.0, bounds.1, bounds.2, bounds.3, *filter,
    )
}
