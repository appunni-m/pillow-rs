//! SIMD Paste family admission, kernels, and output materialization.

#[path = "kernels/mod.rs"]
mod kernels;
#[path = "plan.rs"]
mod plan;
mod rgb_to_rgba;
#[cfg(test)]
mod tests;

use super::{
    NativeConvertLayout, copy_native_bytes, native_convert_layout_block, simd_unsupported,
};
use crate::error::PilError;
use crate::image::{Image, preserve_mode};
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, GenericImageView, ImageBuffer, RgbImage};
use std::sync::Arc;

pub(super) use self::kernels::native_paste_apply;
use self::kernels::native_paste_apply_with_source_layout;
use self::kernels::native_paste_luma16_apply;
#[cfg(test)]
pub(super) use self::kernels::{
    native_paste_cmyk_l_masked_row, native_paste_la_masked_row, native_paste_rgb_l_masked_row,
};
pub(super) use self::plan::NativePasteLayout;
pub(super) use self::plan::{
    NativePasteMaskLayout, NativePastePlan, NativePasteRegion, NativePasteSourceLayout,
    native_composite_plan_for_image, native_composite_plan_for_shape, native_paste_actual_layout,
    native_paste_actual_mask_layout, native_paste_layout, native_paste_plan_for_image,
    native_paste_plan_for_image_with_source_layout, native_paste_plan_for_shape,
    native_paste_region, native_paste_rgb_to_rgba_supported,
    native_paste_rgb_to_rgba_supported_for_shape, native_paste_rgba_to_rgb_supported,
    native_paste_rgba_to_rgb_supported_for_shape, native_paste_shape_layout,
};
pub(super) use self::rgb_to_rgba::simd_paste_rgb_to_rgba;

/// Composite two images through their native byte planes.
///
/// Pillow implements composite as a copy of `image2` followed by a masked
/// paste of `image1` at the origin. The output copy and the masked arithmetic
/// both stay on the host in native storage; no packed-RGBA conversion or CPU
/// adapter is used. A padded final vector handles narrow rows, which is
/// important for indexed images and small public inputs.
pub fn simd_composite_module(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::CompositeModule {
        other,
        mask,
        mask_alpha,
    } = op
    else {
        return Err(PilError::ValueError("expected CompositeModule op".into()));
    };
    let plan = native_composite_plan_for_image(img, other, mask, *mask_alpha, mode)
        .ok_or_else(|| simd_unsupported("CompositeModule"))?;
    let destination = other.materialized_shared()?;
    if !native_paste_actual_layout(img, plan.layout)
        || !native_paste_actual_layout(destination.as_ref(), plan.layout)
        || destination.dimensions() != other.size()?
    {
        return Err(simd_unsupported("CompositeModule"));
    }
    let mask_image = mask.materialized_shared()?;
    let mask_layout = plan
        .mask
        .ok_or_else(|| simd_unsupported("CompositeModule"))?;
    if !native_paste_actual_mask_layout(mask_image.as_ref(), mask_layout)
        || mask_image.dimensions() != img.dimensions()
    {
        return Err(simd_unsupported("CompositeModule"));
    }

    // Composite starts from an image2 copy. For native L, clone the existing
    // byte plane directly: zero-filling a new output before copying every
    // destination byte adds a full redundant write pass. Keep the explicit
    // vector copy and its telemetry for the other layouts.
    let mut output = if plan.layout.mode == "L" {
        destination.as_bytes().to_vec()
    } else {
        let mut output = vec![0u8; destination.as_bytes().len()];
        let (copy_blocks, copy_tail) = copy_native_bytes(destination.as_bytes(), &mut output)
            .ok_or_else(|| {
                PilError::InternalError("SIMD composite buffer shape mismatch".into())
            })?;
        crate::compute::record_pipeline_operation_vector_blocks(copy_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(copy_tail);
        output
    };
    if !native_paste_apply(
        &mut output,
        img.as_bytes(),
        Some(mask_image.as_bytes()),
        destination.width() as usize,
        destination.height() as usize,
        plan,
        true,
    ) {
        return Err(simd_unsupported("CompositeModule"));
    }
    let result = crate::image_utils::raw_bytes_to_image_allow_empty(
        destination.width(),
        destination.height(),
        output,
        plan.layout.channels,
    )?;
    Ok(preserve_mode(destination.as_ref(), result))
}

/// Copy an RGBA source into an RGB destination using fixed SIMD shuffles to
/// drop alpha. This is the unmasked Pillow contract; a mask would require a
/// separate blend contract and is deliberately not admitted here.
fn simd_paste_rgba_to_rgb(
    img: &DynamicImage,
    source: &DynamicImage,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Result<DynamicImage, PilError> {
    let (destination, source) = match (img, source) {
        (DynamicImage::ImageRgb8(destination), DynamicImage::ImageRgba8(source)) => {
            (destination, source)
        }
        _ => return Err(simd_unsupported("Paste")),
    };
    let source_width = u32::try_from(width).map_err(|_| simd_unsupported("Paste"))?;
    let source_height = u32::try_from(height).map_err(|_| simd_unsupported("Paste"))?;
    if source.dimensions() != (source_width, source_height) {
        return Err(simd_unsupported("Paste"));
    }
    let expected_destination = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(3))
        .ok_or_else(|| simd_unsupported("Paste"))?;
    let expected_source = (source_width as usize)
        .checked_mul(source_height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| simd_unsupported("Paste"))?;
    if destination.as_raw().len() != expected_destination
        || source.as_raw().len() != expected_source
    {
        return Err(simd_unsupported("Paste"));
    }

    let region = native_paste_region(img.width(), img.height(), source_width, source_height, x, y)
        .unwrap_or(NativePasteRegion {
            source_left: 0,
            source_top: 0,
            destination_left: 0,
            destination_top: 0,
            width: 0,
            height: 0,
        });
    let source_row_stride = source_width as usize * 4;
    let destination_row_stride = img.width() as usize * 3;
    let layout = NativeConvertLayout {
        source_channels: 4,
        target_channels: 3,
        source_is_luma: false,
        source_is_rgbx: false,
        target_is_luma: false,
        target_is_cmyk: false,
        target_is_hsv: false,
        target_is_ycbcr: false,
        target_is_integer: false,
        target_is_float: false,
    };
    let mut output = destination.as_raw().clone();
    let mut vector_blocks = 0u64;
    for row in 0..region.height {
        let source_row_start =
            (region.source_top + row) * source_row_stride + region.source_left * 4;
        let destination_row_start =
            (region.destination_top + row) * destination_row_stride + region.destination_left * 3;
        for start in (0..region.width).step_by(4) {
            let active = (region.width - start).min(4);
            let source_start = source_row_start + start * 4;
            let mut input = [0u8; 16];
            input[..active * 4]
                .copy_from_slice(&source.as_raw()[source_start..source_start + active * 4]);
            let converted = native_convert_layout_block(&input, layout, 4)
                .ok_or_else(|| simd_unsupported("Paste"))?;
            let destination_start = destination_row_start + start * 3;
            output[destination_start..destination_start + active * 3]
                .copy_from_slice(&converted[..active * 3]);
            vector_blocks = vector_blocks.saturating_add(1);
        }
    }
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    crate::compute::record_pipeline_operation_path("vector");
    let result = RgbImage::from_raw(img.width(), img.height(), output)
        .ok_or_else(|| PilError::InternalError("SIMD Paste RGB buffer shape mismatch".into()))?;
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(result)))
}

/// Copy RGB pixels into an RGBA paste region and insert opaque alpha in the
/// same four-pixel shuffle that writes each destination block.
pub fn simd_paste(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Paste {
        source,
        x,
        y,
        w,
        h,
        mask,
        mask_alpha,
    } = op
    else {
        return Err(PilError::ValueError("expected Paste op".into()));
    };
    if native_paste_rgba_to_rgb_supported(img, source, *w, *h, mask.as_ref(), mode) {
        let source_image = source.materialized_shared()?;
        return simd_paste_rgba_to_rgb(img, source_image.as_ref(), *x, *y, *w, *h);
    }
    if native_paste_rgb_to_rgba_supported(img, source, *w, *h, mask.as_ref(), mode) {
        let source_image = source.materialized_shared()?;
        return simd_paste_rgb_to_rgba(img, source_image.as_ref(), *x, *y, *w, *h);
    }
    let (plan, source_layout) = native_paste_plan_for_image_with_source_layout(
        img,
        source,
        *x,
        *y,
        *w,
        *h,
        mask.as_ref(),
        *mask_alpha,
        mode,
    )
    .ok_or_else(|| simd_unsupported("Paste"))?;
    let source_image = source.materialized_shared()?;
    if !native_paste_actual_layout(source_image.as_ref(), plan.layout) {
        return Err(simd_unsupported("Paste"));
    }
    let mask_image = match mask {
        Some(mask) => {
            let mask_image = mask.materialized_shared()?;
            let Some(mask_layout) = plan.mask else {
                return Err(simd_unsupported("Paste"));
            };
            if !native_paste_actual_mask_layout(mask_image.as_ref(), mask_layout) {
                return Err(simd_unsupported("Paste"));
            }
            Some(mask_image)
        }
        None => None,
    };
    if plan.layout.mode == "I;16" {
        let DynamicImage::ImageLuma16(source_image) = source_image.as_ref() else {
            return Err(simd_unsupported("Paste"));
        };
        let DynamicImage::ImageLuma16(destination_image) = img else {
            return Err(simd_unsupported("Paste"));
        };
        let mut output = destination_image.as_raw().clone();
        if !native_paste_luma16_apply(
            &mut output,
            source_image.as_raw(),
            mask_image.as_ref().map(|image| image.as_bytes()),
            img.width() as usize,
            img.height() as usize,
            plan,
        ) {
            return Err(simd_unsupported("Paste"));
        }
        let result = ImageBuffer::from_raw(img.width(), img.height(), output).ok_or_else(|| {
            PilError::InternalError("SIMD Paste I;16 buffer shape mismatch".into())
        })?;
        return Ok(preserve_mode(img, DynamicImage::ImageLuma16(result)));
    }
    let mut output = img.as_bytes().to_vec();
    if !native_paste_apply_with_source_layout(
        &mut output,
        source_image.as_bytes(),
        mask_image.as_ref().map(|image| image.as_bytes()),
        img.width() as usize,
        img.height() as usize,
        plan,
        source_layout,
        true,
    ) {
        return Err(simd_unsupported("Paste"));
    }
    let result = crate::image_utils::raw_bytes_to_image(
        img.width(),
        img.height(),
        output,
        plan.layout.channels,
    )?;
    Ok(preserve_mode(img, result))
}

pub(super) fn native_paste_in_place(
    img: &mut DynamicImage,
    source: &Arc<Image>,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    mask: Option<&Arc<Image>>,
    mask_alpha: bool,
    mode: Option<&str>,
) -> Result<bool, PilError> {
    let plan = native_paste_plan_for_image_with_source_layout(
        img, source, x, y, width, height, mask, mask_alpha, mode,
    );
    let Some((plan, source_layout)) = plan else {
        return Ok(false);
    };
    let source_image = source.materialized_shared()?;
    if !native_paste_actual_layout(source_image.as_ref(), plan.layout) {
        return Ok(false);
    }
    let mask_image = match mask {
        Some(mask) => {
            let mask_image = mask.materialized_shared()?;
            let Some(mask_layout) = plan.mask else {
                return Ok(false);
            };
            if !native_paste_actual_mask_layout(mask_image.as_ref(), mask_layout) {
                return Ok(false);
            }
            Some(mask_image)
        }
        None => None,
    };
    if plan.layout.mode == "I;16" {
        let DynamicImage::ImageLuma16(source_image) = source_image.as_ref() else {
            return Ok(false);
        };
        let destination_width = img.width() as usize;
        let destination_height = img.height() as usize;
        let Some(destination_image) = img.as_mut_luma16() else {
            return Ok(false);
        };
        return Ok(native_paste_luma16_apply(
            destination_image.as_mut(),
            source_image.as_raw(),
            mask_image.as_ref().map(|image| image.as_bytes()),
            destination_width,
            destination_height,
            plan,
        ));
    }
    let destination_width = img.width() as usize;
    let destination_height = img.height() as usize;
    let Some(output) = img.as_bytes_mut() else {
        return Ok(false);
    };
    Ok(native_paste_apply_with_source_layout(
        output,
        source_image.as_bytes(),
        mask_image.as_ref().map(|image| image.as_bytes()),
        destination_width,
        destination_height,
        plan,
        source_layout,
        true,
    ))
}
