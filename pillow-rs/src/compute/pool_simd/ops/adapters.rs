//! SIMD adapter wrappers for the registry's `SimdOpFn` signature.
//!
//! Each admitted adapter validates the concrete image layout and parameters
//! before execution, then keeps the hot pixel/data path in a native byte
//! kernel. Scalar work is limited to control/validation and unavoidable edge
//! or tail handling; unsupported contracts return a capability error.

use super::native_channel;
use super::native_filter::native_filter_load_byte_block;

mod alpha_composite;
mod box_blur;
mod chops;
mod color3dlut;
mod colorize;
mod crop;
pub(crate) use self::alpha_composite::{simd_alpha_composite, simd_alpha_composite_mirror};
use self::alpha_composite::{
    simd_alpha_composite_operands_supported, simd_alpha_composite_shape_supported,
};
pub(crate) use self::box_blur::simd_box_blur;
use self::box_blur::simd_pil_box_blur;
#[cfg(test)]
use self::box_blur::{simd_blur_rows, simd_transpose_interleaved_rows};
pub(crate) use self::chops::{
    simd_add, simd_add_modulo, simd_darker, simd_difference, simd_hard_light, simd_lighter,
    simd_logical_and, simd_logical_or, simd_logical_xor, simd_multiply, simd_overlay, simd_screen,
    simd_soft_light, simd_subtract, simd_subtract_modulo,
};
pub(crate) use self::crop::{simd_crop, simd_crop_border};
mod draw;
use self::draw::{draw_line_vector_block, native_draw_layout, shape_draw_channels};
mod effects;
mod enhance;
mod expand;
mod extract_band;
mod filter;
mod histogram;
mod lut;
pub(crate) use self::color3dlut::simd_color3dlut;
use self::color3dlut::{color3dlut_supported_for_image, color3dlut_supported_for_shape};
pub(crate) use self::colorize::simd_colorize;
mod flip;
#[cfg(all(
    not(feature = "parallel"),
    target_arch = "aarch64",
    target_feature = "neon"
))]
pub(crate) use self::filter::native_filter_3x3_la_binomial_rows;
use self::filter::{
    native_filter_3x3_channel_block_integer, native_filter_3x3_luma_block_integer,
    native_filter_3x3_pixel_block_integer, native_filter_3x3_rgb_byte_block_integer,
    native_filter_3x3_vector, native_filter_identity_supported_for_image,
    shape_native_filter_byte_channels, shape_native_filter_identity_supported,
    shape_native_i32_convolution_path, use_native_byte_convolution_path,
    use_native_i32_convolution_path,
};
pub(crate) use self::filter::{
    simd_filter_3x3, simd_filter_5x5, uniform_i32_filter3x3_is_identity_simd,
};
pub(crate) use self::flip::{simd_flip, simd_mirror};
mod gaussian_blur;
pub(crate) use self::gaussian_blur::simd_gaussian_blur;
#[path = "adapters/gaussian_blur/kernels/rgba_uniform.rs"]
mod gaussian_rgba_uniform;
mod gradient;
mod grayscale;
mod mandelbrot;
mod median_filter;
mod merge;
mod module_blend;
mod offset;
mod pad;
mod paste;
mod projection;
mod put_alpha;
mod put_pixel;
mod reduce;
mod resize;
mod rotate;
pub(crate) use self::rotate::simd_rotate;
mod thumbnail;
mod transform;
mod transpose;
pub(crate) use self::draw::{
    simd_draw_arc, simd_draw_chord, simd_draw_circle, simd_draw_ellipse, simd_draw_line,
    simd_draw_pieslice, simd_draw_point, simd_draw_polygon, simd_draw_rectangle,
    simd_draw_rounded_rect,
};
pub(crate) use self::effects::{
    simd_effect_noise, simd_effect_spread, simd_invert, simd_invert_chops,
};
pub(crate) use self::enhance::{
    simd_brightness, simd_color_saturation, simd_contrast, simd_posterize, simd_sharpness,
    simd_solarize,
};
pub(crate) use self::expand::simd_expand;
pub(crate) use self::extract_band::{simd_extract_band, simd_extract_band_into};
pub(crate) use self::gradient::{simd_linear_gradient_generate, simd_radial_gradient_generate};
pub(crate) use self::grayscale::{simd_grayscale, simd_grayscale_with_evidence};
pub(crate) use self::histogram::{simd_autocontrast, simd_equalize};
#[cfg(test)]
use self::lut::{native_lut_apply, native_lut_map_rows, native_lut_tables_for_channels};
pub(crate) use self::lut::{native_point_lut, simd_eval, simd_remap_palette};
pub(crate) use self::mandelbrot::simd_mandelbrot_generate;
pub(crate) use self::median_filter::{
    simd_max_filter, simd_median_filter, simd_min_filter, simd_rank_filter,
};
#[cfg(all(test, target_arch = "x86_64"))]
use self::merge::native_merge_interleave;
pub(crate) use self::merge::simd_merge;
#[cfg(test)]
use self::merge::{native_merge_bytes, native_merge_rgb_luma_owners};
pub(crate) use self::module_blend::simd_blend_module;
pub(crate) use self::offset::simd_offset;
pub(crate) use self::pad::simd_pad;
pub(crate) use self::paste::simd_paste;
#[cfg(test)]
use self::paste::{
    NativePasteLayout, NativePasteMaskLayout, NativePastePlan, native_paste_cmyk_l_masked_row,
    native_paste_la_masked_row, native_paste_region, native_paste_rgb_l_masked_row,
    simd_paste_rgb_to_rgba,
};
use self::paste::{
    native_composite_plan_for_image, native_composite_plan_for_shape, native_paste_actual_layout,
    native_paste_actual_mask_layout, native_paste_apply, native_paste_in_place,
    native_paste_plan_for_image, native_paste_plan_for_shape, native_paste_rgb_to_rgba_supported,
    native_paste_rgb_to_rgba_supported_for_shape, native_paste_rgba_to_rgb_supported,
    native_paste_rgba_to_rgb_supported_for_shape, native_put_data_layout,
    native_put_data_shape_layout,
};
pub(crate) use self::projection::{
    simd_cmyk_projection, simd_cmyk_projection_from_positive_writes, simd_rgb_projection,
};
pub(crate) use self::put_alpha::{simd_put_alpha, simd_put_alpha_data};
pub(crate) use self::put_pixel::{
    simd_put_pixel, simd_rgba_putpixel_batch, simd_rgba_putpixel_batch_supported,
};
pub(crate) use self::reduce::simd_reduce;
use self::reduce::{
    native_reduce_bytes, native_reduce_sparse_rgb_2x2, native_reduce_supported_for_image,
    native_reduce_supported_for_shape,
};
#[cfg(test)]
use self::resize::{build_resize_horizontal_plan, resize_u8_coefficients_fit_i32};
pub(crate) use self::resize::{simd_contain, simd_cover, simd_fit, simd_resize, simd_scale};
use self::resize::{
    simd_resize_convolution, simd_resize_convolution_boxed, simd_resize_convolution_into,
    simd_resize_convolution_with_zero_row_elision, simd_resize_f, simd_resize_f_boxed,
    simd_resize_i32, simd_resize_nearest,
};
pub(crate) use self::thumbnail::{
    simd_rgb_putpixel_thumbnail_fusion_supported, simd_rgb_putpixel_thumbnail_prepare,
    simd_rgb_putpixel_thumbnail_resize, simd_thumbnail,
};
pub(crate) use self::transform::simd_transform;
#[cfg(test)]
use self::transpose::native_transpose_bytes;
pub(crate) use self::transpose::simd_transpose;
#[cfg(all(not(feature = "parallel"), target_arch = "aarch64"))]
use self::transpose::transpose_native_tiled;
#[cfg(all(test, feature = "parallel"))]
use self::transpose::{
    TransposeNativeCollectCounters, TransposeNativeOddCollectCounters, TransposeNativeOddRowCache,
    TransposeNativeRowCache, transpose_native_collect, transpose_native_collect_admitted,
    transpose_native_odd_collect, transpose_native_odd_collect_admitted,
};
use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::{Image, preserve_mode};
#[cfg(test)]
use crate::ops::pil_resize::FilterCoeffs;
#[cfg(test)]
use crate::ops::pil_resize::precompute_coeffs;
use crate::ops::pil_resize::{
    compact_resize_coeffs_to_source_span, precompute_coeffs_boxed_for_filter,
};
use crate::pipeline::{ColorMode, PipelineOp, PixelMode, TransposeMethod};
use crate::raster::{DynamicImage, GenericImageView, ImageBuffer, Luma};
use std::sync::Arc;
use wide::{f64x8, i16x8, i32x8, u8x16, u16x8, u16x16, u32x16};

// `parallel` may be enabled to select Parallel CPU kernels, but it must not
// silently add Rayon scheduling to the SIMD backend. Keep SIMD routes
// row-serial while retaining their vectorized inner loops.
macro_rules! simd_rows_mut_serial {
    ($data:expr, $stride:expr, $height:expr,
     |$row_start:ident, $row_end:ident, $y:ident, $row:ident| $body:block) => {{
        let _data: &mut [u8] = $data;
        let _stride: usize = $stride;
        let _height: usize = $height;
        _data
            .chunks_mut(_stride)
            .take(_height)
            .enumerate()
            .for_each(|(_y_idx, _row)| {
                let $row_start: usize = _y_idx * _stride;
                let $row_end: usize = $row_start + _stride;
                let $y: u32 = _y_idx as u32;
                let $row: &mut [u8] = _row;
                $body
            });
    }};
}

pub(crate) use simd_rows_mut_serial;

const SIMD_BYTE_VECTOR_LANES: usize = 16;

fn native_byte_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("L")) => Some(1),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA")) => Some(2),
        DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB")) => Some(3),
        DynamicImage::ImageRgba8(_) if matches!(mode, None | Some("RGBA" | "RGBa" | "RGBX")) => {
            Some(4)
        }
        _ => None,
    }
}

/// Return the native byte layout accepted by Pillow's filter kernels.
///
/// Filter.c operates on the stored samples rather than interpreting the
/// fourth byte as alpha.  That makes the filter contract broader than the
/// ordinary RGBA arithmetic adapters: `1`, CMYK, and PA stay in their native
/// packed layouts.  Palette-only `P` is intentionally excluded because
/// Pillow rejects filtering a palette image.
fn native_filter_byte_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("1" | "L")) => Some(1),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA" | "PA")) => Some(2),
        DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) => {
            Some(3)
        }
        DynamicImage::ImageRgba8(_)
            if matches!(mode, None | Some("RGBA" | "RGBa" | "RGBX" | "CMYK")) =>
        {
            Some(4)
        }
        _ => None,
    }
}

fn native_typed_filter_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    matches!(img, DynamicImage::ImageRgba8(_))
        .then_some(())
        .filter(|_| matches!(mode, Some("I" | "F")))
        .map(|_| 4)
}

fn native_expand_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("1" | "L" | "P")) => Some(1),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA" | "PA")) => Some(2),
        DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) => {
            Some(3)
        }
        DynamicImage::ImageRgba8(_)
            if matches!(mode, None | Some("RGBA" | "CMYK" | "RGBa" | "RGBX")) =>
        {
            Some(4)
        }
        _ => None,
    }
}

fn native_expand_contract_for_image(
    img: &DynamicImage,
    border: u32,
    mode: Option<&str>,
) -> Option<(usize, u32, u32)> {
    let channels = native_expand_layout(img, mode)?;
    let border_twice = border.checked_mul(2)?;
    let width = img.width().checked_add(border_twice)?;
    let height = img.height().checked_add(border_twice)?;
    let source_len = (img.width() as usize)
        .checked_mul(img.height() as usize)?
        .checked_mul(channels)?;
    let output_len = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(channels)?;
    if output_len == 0 || img.as_bytes().len() != source_len {
        return None;
    }
    Some((channels, width, height))
}

fn native_expand_contract_for_shape(
    shape: SimdImageShape,
    border: u32,
    mode: Option<&str>,
) -> Option<(usize, u32, u32)> {
    let channels = match shape.layout {
        SimdLayout::Luma8 if matches!(mode, None | Some("1" | "L" | "P")) => 1,
        SimdLayout::LumaA8 if matches!(mode, None | Some("LA" | "PA")) => 2,
        SimdLayout::Rgb8 if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) => 3,
        SimdLayout::Rgba8 if matches!(mode, None | Some("RGBA" | "CMYK" | "RGBa" | "RGBX")) => 4,
        _ => return None,
    };
    let border_twice = border.checked_mul(2)?;
    let width = shape.width.checked_add(border_twice)?;
    let height = shape.height.checked_add(border_twice)?;
    let output_len = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(channels)?;
    if output_len == 0 {
        return None;
    }
    Some((channels, width, height))
}

/// Return a native interleaved byte layout for operations whose data plane is
/// memory movement rather than color arithmetic. Indexed and color-space
/// tags are included here because their stored samples are still raw bytes;
/// callers that interpret alpha or color arithmetic must use the narrower
/// helpers above instead.
fn native_copy_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("1" | "L" | "P")) => Some(1),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA" | "PA")) => Some(2),
        DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) => {
            Some(3)
        }
        DynamicImage::ImageRgba8(_)
            if matches!(
                mode,
                None | Some("RGBA" | "CMYK" | "RGBa" | "RGBX" | "I" | "F")
            ) =>
        {
            Some(4)
        }
        _ => None,
    }
}

/// Return whether an `I;16*` image can use the transpose byte kernel without
/// widening its samples into a packed color representation. This is kept
/// separate from `native_copy_layout`: other memory operations still need
/// their own two-byte sample contracts.
fn native_luma16_transpose_layout(img: &DynamicImage, mode: Option<&str>) -> bool {
    matches!(img, DynamicImage::ImageLuma16(_))
        && matches!(mode, None | Some("I;16" | "I;16L" | "I;16B" | "I;16N"))
}

/// Return whether an `I;16*` image can use native-copy geometry operations.
/// The two-byte samples stay opaque throughout the copy; reconstructing the
/// `ImageLuma16` buffer at the boundary preserves the typed image contract
/// without widening it through an RGBA representation.
fn native_luma16_copy_layout(img: &DynamicImage, mode: Option<&str>) -> bool {
    matches!(img, DynamicImage::ImageLuma16(_))
        && matches!(mode, None | Some("I;16" | "I;16L" | "I;16B" | "I;16N"))
}

fn has_valid_luma16_copy_bytes(img: &DynamicImage, mode: Option<&str>) -> bool {
    if !native_luma16_copy_layout(img, mode) {
        return false;
    }
    let Some(expected) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(2))
    else {
        return false;
    };
    img.as_bytes().len() == expected
}

/// Return whether a row-oriented byte kernel has at least one complete
/// sixteen-byte vector block per row.  A large image made of very narrow rows
/// can have many total bytes while still exercising only scalar tails; those
/// inputs must not be advertised as SIMD-capable.
fn has_vectorized_byte_rows(img: &DynamicImage, channels: usize) -> bool {
    img.height() != 0
        && img
            .width()
            .checked_mul(channels as u32)
            .is_some_and(|row_bytes| row_bytes >= 16)
}

/// A zero-pixel native image is a valid Chops data path even though it has no
/// vector block to execute. Returning its already-typed buffer is the native
/// no-work equivalent of Pillow's empty output; it must not be reported as a
/// scalar or arithmetic vector kernel.
fn has_empty_native_bytes(img: &DynamicImage, channels: usize) -> bool {
    (img.width() == 0 || img.height() == 0)
        && img
            .width()
            .checked_mul(img.height())
            .and_then(|pixels| pixels.checked_mul(channels as u32))
            == Some(0)
        && img.as_bytes().is_empty()
}

/// Offset copies wrapped rows through vector loads and lane shuffles. Rows
/// narrower than one vector can still be batched across row boundaries; the
/// capability gate therefore requires one complete vector in the image, not
/// in every individual row.
fn has_vectorized_offset_rows(img: &DynamicImage, channels: usize, xoffset: i32) -> bool {
    if img.width() == 0 || img.height() == 0 {
        return false;
    }
    let Some(row_bytes) = img.width().checked_mul(channels as u32) else {
        return false;
    };
    let _ = xoffset;
    row_bytes
        .checked_mul(img.height())
        .is_some_and(|total_bytes| total_bytes >= 16)
}

/// Pillow's `I;16*` ImageChops offset path is byte-oriented: the native
/// implementation copies only the first `width` bytes of each `width * 2`
/// byte row, leaving the second half zero-filled. Narrow images use a masked
/// vector block across adjacent rows, so a small frame does not need to be
/// handed to the scalar backend merely because one row is shorter than a
/// register.
fn has_vectorized_luma16_offset(img: &DynamicImage, mode: Option<&str>) -> bool {
    matches!(img, DynamicImage::ImageLuma16(_))
        && matches!(mode, None | Some("I;16" | "I;16L" | "I;16B" | "I;16N"))
        && (has_empty_native_bytes(img, 2)
            || (img.width() != 0
                && img.height() != 0
                && img
                    .width()
                    .checked_mul(img.height())
                    .and_then(|pixels| pixels.checked_mul(2))
                    .is_some_and(|bytes| img.as_bytes().len() == bytes as usize)))
}

/// Mirror uses one padded vector group for a partial final pixel group, so
/// even narrow rows can execute the native reverse kernel. RGB still needs
/// three vectors for one complete sixteen-pixel group, while the other layouts
/// use one vector group sized to their channel stride.
fn has_vectorized_mirror_rows(img: &DynamicImage, channels: usize) -> bool {
    img.height() != 0 && img.width() != 0 && matches!(channels, 1..=4)
}

/// A masked final block is still a real SIMD data path. Use this admission
/// predicate for byte-wise kernels whose operation is independent per sample;
/// it permits narrow rows without routing the whole operation to CPU merely
/// because the row has fewer than sixteen bytes.
fn has_nonempty_byte_data(img: &DynamicImage, channels: usize) -> bool {
    img.height() != 0
        && img.width() != 0
        && img
            .width()
            .checked_mul(img.height())
            .and_then(|pixels| pixels.checked_mul(channels as u32))
            .is_some_and(|bytes| bytes != 0)
}

/// Validate the native byte buffer without imposing a minimum vector width.
/// A partial final block is padded by the data kernel, and an empty image is
/// handled as a valid native no-work path by operations whose Pillow contract
/// preserves it.
fn has_valid_byte_data(img: &DynamicImage, channels: usize) -> bool {
    let Some(expected) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return false;
    };
    img.as_bytes().len() == expected
}

/// Return whether an identity operation can use at least one complete native
/// vector copy.  Zero-radius blur is a valid Pillow no-op, but it must not be
/// admitted as SIMD merely because the image has byte storage: a one-byte
/// scalar-only copy is still an unsupported strict-SIMD data path.
fn has_vectorized_native_identity_copy(img: &DynamicImage, mode: Option<&str>) -> bool {
    let Some(channels) = native_byte_layout(img, mode) else {
        return false;
    };
    let Some(expected_len) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return false;
    };
    expected_len >= 16 && img.as_bytes().len() == expected_len
}

#[inline]
fn record_padded_native_row_work(width: usize, height: usize, channels: usize) {
    let row_bytes = width.saturating_mul(channels);
    if row_bytes == 0 || height == 0 {
        return;
    }
    let vector_blocks = row_bytes.div_ceil(16).saturating_mul(height);
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks as u64);
}

fn valid_convolution_parameters(kernel: &[f32], scale: f32) -> bool {
    scale.is_finite() && scale != 0.0 && kernel.iter().all(|value| value.is_finite())
}

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

/// Replace the canonical `putdata` prefix through the image's native byte
/// layout. The binding/core boundary has already normalized scalar and tuple
/// values into `data`; the SIMD adapter only performs the defined byte copy,
/// preserving untouched pixels when Pillow receives a short sequence.
pub fn simd_put_data(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::PutData {
        data,
        mode: data_mode,
    } = op
    else {
        return Err(PilError::ValueError("expected PutData op".into()));
    };
    let layout =
        native_put_data_layout(img, *data_mode, mode).ok_or_else(|| simd_unsupported("PutData"))?;
    if !native_paste_actual_layout(img, layout) {
        return Err(simd_unsupported("PutData"));
    }
    let expected_len = img
        .width()
        .checked_mul(img.height())
        .and_then(|pixels| pixels.checked_mul(layout.channels as u32))
        .map(|bytes| bytes as usize)
        .ok_or_else(|| PilError::ValueError("SIMD PutData byte count overflow".into()))?;
    if img.as_bytes().len() != expected_len {
        return Err(simd_unsupported("PutData"));
    }

    let copy_len = data.len().min(expected_len);
    let (output, vector_blocks, scalar_tail) = if copy_len == expected_len {
        // A complete payload replaces the entire image. Copy it once into
        // the result instead of cloning source samples that are immediately
        // overwritten, then copying the payload a second time.
        (data[..copy_len].to_vec(), 0, 0)
    } else {
        let mut output = vec![0u8; expected_len];
        let (mut vector_blocks, mut scalar_tail) = copy_native_bytes(img.as_bytes(), &mut output)
            .ok_or_else(|| {
            PilError::InternalError("SIMD PutData buffer shape mismatch".into())
        })?;
        if copy_len != 0 {
            let (blocks, tail) = copy_native_bytes(&data[..copy_len], &mut output[..copy_len])
                .ok_or_else(|| {
                    PilError::InternalError("SIMD PutData prefix shape mismatch".into())
                })?;
            vector_blocks = vector_blocks.saturating_add(blocks);
            scalar_tail = scalar_tail.saturating_add(tail);
        }
        (output, vector_blocks, scalar_tail)
    };
    crate::compute::record_pipeline_operation_path("native-copy");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    let result = crate::image_utils::raw_bytes_to_image_allow_empty(
        img.width(),
        img.height(),
        output,
        layout.channels,
    )?;
    Ok(preserve_mode(img, result))
}

pub(super) fn simd_unsupported(operation: &str) -> PilError {
    PilError::NotImplementedError(format!(
        "SIMD {operation} is unsupported for the validated image layout/parameters"
    ))
}

/// Check the contextual conditions for a real SIMD/native-copy operation.
///
/// The registry answers the cheap operation-only question.  This second
/// check runs after the source image is available. Returning `false` here is
/// important: an explicit SIMD request must fail before entering an adapter
/// that would use a scalar-only data path, while automatic routing can choose
/// CPU before any pixel work starts.
pub(crate) fn simd_supports_for_image(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> bool {
    // The operation-level allow-list is the first admission gate. Some
    // scalar adapters remain in the registry for legacy ABI/workbench
    // compatibility, but they are never allowed to reach this contextual
    // layout check or an execution function under the SIMD backend.
    if !crate::compute::registry::simd_supports(op).unwrap_or(false) {
        return false;
    }
    let pixel_count = (img.width() as usize).saturating_mul(img.height() as usize);
    let native_byte_channels = native_byte_layout(img, mode);
    let native_filter_byte_channels = native_filter_byte_layout(img, mode);
    let native_typed_filter_channels = native_typed_filter_layout(img, mode);
    let native_copy = native_copy_layout(img, mode).is_some();
    let native_luma16_transpose = native_luma16_transpose_layout(img, mode);
    let native_luma16_copy =
        has_valid_luma16_copy_bytes(img, mode) && img.width() != 0 && img.height() != 0;
    match op {
        // These point operations have native byte kernels. Typed, indexed,
        // and explicitly mode-converted images are deliberately rejected
        // until their sample-domain kernels exist.
        PipelineOp::Invert | PipelineOp::InvertChops => effects::supports_for_image(img, op, mode),
        PipelineOp::ResizeBoxed { .. }
        | PipelineOp::Resize { .. }
        | PipelineOp::Scale { .. }
        | PipelineOp::Contain { .. }
        | PipelineOp::Cover { .. }
        | PipelineOp::Fit { .. } => resize::supports_for_image(img, op, mode),
        PipelineOp::Thumbnail { w, h, filter } => {
            thumbnail::native_thumbnail_supported_for_image(img, *w, *h, *filter, mode)
        }
        PipelineOp::Transform {
            w,
            h,
            method,
            data,
            filter,
            ..
        } => transform::supports_for_image(img, *w, *h, method, data, *filter, mode),
        PipelineOp::Pad { .. } => pad::supports_for_image(img, op, mode),
        PipelineOp::Convert {
            mode: target,
            matrix,
            dither: _,
        } => native_convert_supported_for_image(img, target, matrix.as_deref(), mode),
        PipelineOp::ConvertLab { .. } => {
            matches!(img, DynamicImage::ImageRgb8(_))
                && matches!(mode, None | Some("RGB"))
                && (img.width() as usize)
                    .checked_mul(img.height() as usize)
                    .is_some_and(|pixels| img.as_bytes().len() == pixels.saturating_mul(3))
        }
        PipelineOp::Reduce { x_factor, y_factor } => {
            native_reduce_supported_for_image(img, *x_factor, *y_factor, mode)
        }
        PipelineOp::Grayscale => grayscale::supports_image(img, mode),
        PipelineOp::Colorize { .. } => colorize::supports_for_image(img, mode),
        PipelineOp::Solarize { .. }
        | PipelineOp::Posterize { .. }
        | PipelineOp::Brightness { .. }
        | PipelineOp::Contrast { .. }
        | PipelineOp::ColorSaturation { .. }
        | PipelineOp::Sharpness { .. } => enhance::supports_for_image(img, op, mode),
        PipelineOp::Autocontrast { .. }
        | PipelineOp::Equalize
        | PipelineOp::EqualizeMasked { .. } => histogram::supports_for_image(img, op, mode),
        PipelineOp::RemapPalette { .. } | PipelineOp::Eval { .. } => {
            lut::supports_for_image(img, op, mode)
        }
        PipelineOp::PutData {
            mode: data_mode, ..
        } => native_put_data_layout(img, *data_mode, mode)
            .is_some_and(|layout| native_paste_actual_layout(img, layout)),
        PipelineOp::ExtractBand { index } => extract_band::supports_for_image(img, *index, mode),
        // These operations are native memory movement. They do not claim
        // arithmetic vectorization, but they never enter a CPU adapter.
        PipelineOp::Offset { x, .. } => {
            has_vectorized_luma16_offset(img, mode)
                || (native_copy
                    && native_copy_layout(img, mode).is_some_and(|channels| {
                        has_empty_native_bytes(img, channels)
                            || has_vectorized_offset_rows(img, channels, *x)
                    }))
        }
        PipelineOp::Flip => {
            native_copy_layout(img, mode)
                .is_some_and(|channels| has_nonempty_byte_data(img, channels))
                || native_luma16_copy
        }
        PipelineOp::Mirror => {
            native_copy_layout(img, mode)
                .is_some_and(|channels| has_vectorized_mirror_rows(img, channels))
                || native_luma16_copy
        }
        PipelineOp::Transpose { .. } => {
            (native_copy || native_luma16_transpose)
                && (pixel_count != 0
                    || native_copy_layout(img, mode)
                        .is_some_and(|channels| has_empty_native_bytes(img, channels)))
        }
        PipelineOp::Crop {
            left,
            top,
            right,
            bottom,
        } => {
            (native_copy || native_luma16_copy)
                && pixel_count != 0
                && *left < *right
                && *top < *bottom
                && *right <= img.width()
                && *bottom <= img.height()
        }
        PipelineOp::CropBorder { .. } => {
            // Let the adapter's scalar geometry check report Pillow's public
            // error for an oversized border. Capability preflight must not
            // replace that validation error with NotImplementedError.
            native_copy
        }
        PipelineOp::Expand { border, .. } => {
            native_expand_contract_for_image(img, *border, mode).is_some()
        }
        PipelineOp::Constant { .. } => img.width() != 0 && img.height() != 0,
        PipelineOp::Duplicate => native_copy_layout(img, mode)
            .is_some_and(|channels| has_nonempty_byte_data(img, channels)),
        // The convolution kernels use wide vector blocks for the maintained
        // native-byte layout. Images narrower than one complete interior
        // vector remain automatic CPU work; image *area* is not a capability
        // condition because a small image can still contain a real vector
        // block.
        PipelineOp::Filter3x3 { kernel, scale, .. } => {
            valid_convolution_parameters(kernel, *scale)
                && (use_native_byte_convolution_path(img, mode, 1)
                    || (mode == Some("I")
                        && native_typed_filter_channels.is_some()
                        && use_native_i32_convolution_path(img, mode, 1))
                    || native_filter_identity_supported_for_image(img, mode, 1))
        }
        PipelineOp::Filter5x5 { kernel, scale, .. } => {
            valid_convolution_parameters(kernel, *scale)
                && (use_native_byte_convolution_path(img, mode, 2)
                    || (mode == Some("I")
                        && native_typed_filter_channels.is_some()
                        && use_native_i32_convolution_path(img, mode, 2))
                    || native_filter_identity_supported_for_image(img, mode, 2))
        }
        PipelineOp::BoxBlur { radius } => {
            if *radius == 0 {
                has_vectorized_native_identity_copy(img, mode)
            } else {
                native_byte_channels.is_some() && img.width() != 0 && img.height() != 0
            }
        }
        PipelineOp::BoxBlurXY {
            radius_x,
            radius_y,
            passes,
        } => {
            native_byte_channels.is_some()
                && *passes != 0
                && radius_x.is_finite()
                && radius_y.is_finite()
                && *radius_x >= 0.0
                && *radius_y >= 0.0
                && ((*radius_x != 0.0 || *radius_y != 0.0) && img.width() != 0 && img.height() != 0
                    || (*radius_x == 0.0
                        && *radius_y == 0.0
                        && has_vectorized_native_identity_copy(img, mode)))
        }
        PipelineOp::GaussianBlur { sigma } => gaussian_blur::supports_for_image(img, *sigma, mode),
        PipelineOp::MaxFilter { size } | PipelineOp::MinFilter { size } => {
            (native_filter_byte_channels.is_some()
                && *size != 0
                && *size % 2 == 1
                && img.width() != 0
                && img.height() != 0)
                || median_filter::native_float_rank_supported_for_image(
                    img,
                    mode,
                    *size,
                    if matches!(op, PipelineOp::MaxFilter { .. }) {
                        size.saturating_mul(*size).saturating_sub(1)
                    } else {
                        0
                    },
                )
        }
        PipelineOp::MedianFilter { size } => {
            (native_filter_byte_channels.is_some()
                && *size != 0
                && *size % 2 == 1
                && *size <= 15
                && img.width() != 0
                && img.height() != 0)
                || median_filter::native_float_rank_supported_for_image(
                    img,
                    mode,
                    *size,
                    size.saturating_mul(*size) / 2,
                )
        }
        PipelineOp::RankFilter { size, rank } => {
            let area = u64::from(*size).saturating_mul(u64::from(*size));
            (native_filter_byte_channels.is_some()
                && *size != 0
                && *size % 2 == 1
                && area <= 225
                && u64::from(*rank) < area
                && img.width() != 0
                && img.height() != 0)
                || median_filter::native_float_rank_supported_for_image(img, mode, *size, *rank)
        }
        PipelineOp::PutAlpha { .. } | PipelineOp::PutAlphaData { .. } => {
            put_alpha::supports_for_image(img, op, mode)
        }
        PipelineOp::EffectNoise { .. } | PipelineOp::EffectSpread { .. } => {
            effects::supports_for_image(img, op, mode)
        }
        PipelineOp::Color3DLut {
            size,
            table,
            channels,
            source_mode,
            target_mode,
        } => color3dlut_supported_for_image(
            img,
            *size,
            table.len(),
            *channels,
            *source_mode,
            *target_mode,
            mode,
        ),
        PipelineOp::Rotate { .. } => rotate::supports_for_image(img, op, mode),
        PipelineOp::DrawLine { .. }
        | PipelineOp::DrawPoint { .. }
        | PipelineOp::DrawEllipse { .. }
        | PipelineOp::DrawCircle { .. }
        | PipelineOp::DrawArc { .. }
        | PipelineOp::DrawChord { .. }
        | PipelineOp::DrawPieslice { .. }
        | PipelineOp::DrawRoundedRect { .. }
        | PipelineOp::DrawPolygon { .. }
        | PipelineOp::DrawRectangle { .. } => draw::supports_for_image(img, op, mode),
        PipelineOp::PutPixel {
            x,
            y,
            palette_index: _,
            ..
        } => put_pixel::native_putpixel_supported_for_image(img, *x, *y, mode),
        PipelineOp::AlphaComposite { source, dest, src } => {
            *dest == (0, 0)
                && *src == (0, 0)
                && simd_alpha_composite_operands_supported(img, source, mode)
        }
        PipelineOp::CompositeModule {
            other,
            mask,
            mask_alpha,
        } => native_composite_plan_for_image(img, other, mask, *mask_alpha, mode).is_some(),
        PipelineOp::Paste {
            source,
            x,
            y,
            w,
            h,
            mask,
            mask_alpha,
        } => {
            native_paste_plan_for_image(
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
            .is_some()
                || native_paste_rgba_to_rgb_supported(img, source, *w, *h, mask.as_ref(), mode)
                || native_paste_rgb_to_rgba_supported(img, source, *w, *h, mask.as_ref(), mode)
        }
        PipelineOp::Merge {
            mode: target_mode,
            bands,
            logical_mode,
        } => merge::supports_image_contract(img, target_mode, bands, mode, logical_mode),
        PipelineOp::BlendModule { .. } => module_blend::supports_for_image(img, op, mode),
        // ImageChops owns stored-sample admission and paired-operand validation.
        PipelineOp::Multiply { .. }
        | PipelineOp::Screen { .. }
        | PipelineOp::Darker { .. }
        | PipelineOp::Lighter { .. }
        | PipelineOp::Difference { .. }
        | PipelineOp::AddModulo { .. }
        | PipelineOp::SubtractModulo { .. }
        | PipelineOp::Overlay { .. }
        | PipelineOp::HardLight { .. }
        | PipelineOp::SoftLight { .. }
        | PipelineOp::LogicalAnd { .. }
        | PipelineOp::LogicalOr { .. }
        | PipelineOp::LogicalXor { .. }
        | PipelineOp::Add { .. }
        | PipelineOp::Subtract { .. } => chops::supports_for_image(img, op, mode),
    }
}

pub(crate) fn preserves_native_contract(op: &PipelineOp) -> bool {
    draw::preserves_native_contract(op)
        || matches!(
            op,
            PipelineOp::Filter3x3 { .. }
                | PipelineOp::Filter5x5 { .. }
                | PipelineOp::GaussianBlur { .. }
                | PipelineOp::BoxBlur { .. }
                | PipelineOp::BoxBlurXY { .. }
                | PipelineOp::MedianFilter { .. }
                | PipelineOp::MaxFilter { .. }
                | PipelineOp::MinFilter { .. }
                | PipelineOp::RankFilter { .. }
                | PipelineOp::Autocontrast { .. }
                | PipelineOp::Equalize
                | PipelineOp::EqualizeMasked { .. }
                | PipelineOp::RemapPalette { .. }
                | PipelineOp::Invert
                | PipelineOp::Flip
                | PipelineOp::Mirror
                | PipelineOp::Posterize { .. }
                | PipelineOp::Solarize { .. }
                | PipelineOp::Add { .. }
                | PipelineOp::Subtract { .. }
                | PipelineOp::Multiply { .. }
                | PipelineOp::Screen { .. }
                | PipelineOp::Darker { .. }
                | PipelineOp::Lighter { .. }
                | PipelineOp::Difference { .. }
                | PipelineOp::Overlay { .. }
                | PipelineOp::HardLight { .. }
                | PipelineOp::SoftLight { .. }
                | PipelineOp::AddModulo { .. }
                | PipelineOp::SubtractModulo { .. }
                | PipelineOp::LogicalAnd { .. }
                | PipelineOp::LogicalOr { .. }
                | PipelineOp::LogicalXor { .. }
                | PipelineOp::Offset { .. }
                | PipelineOp::Duplicate
                | PipelineOp::InvertChops
                | PipelineOp::AlphaComposite { .. }
                | PipelineOp::Brightness { .. }
                | PipelineOp::Contrast { .. }
                | PipelineOp::ColorSaturation { .. }
                | PipelineOp::Sharpness { .. }
                | PipelineOp::EffectSpread { .. }
                | PipelineOp::Paste { .. }
                | PipelineOp::BlendModule { .. }
                | PipelineOp::Eval { .. }
                | PipelineOp::PutData { .. }
                | PipelineOp::PutPixel { .. }
        )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SimdLayout {
    Luma8,
    LumaA8,
    Rgb8,
    Rgba8,
    Luma16,
    Unsupported,
}

#[derive(Clone, Copy)]
pub(super) struct SimdImageShape {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) layout: SimdLayout,
}

fn simd_shape(img: &DynamicImage) -> SimdImageShape {
    let layout = match img {
        DynamicImage::ImageLuma8(_) => SimdLayout::Luma8,
        DynamicImage::ImageLumaA8(_) => SimdLayout::LumaA8,
        DynamicImage::ImageRgb8(_) => SimdLayout::Rgb8,
        DynamicImage::ImageRgba8(_) => SimdLayout::Rgba8,
        DynamicImage::ImageLuma16(_) => SimdLayout::Luma16,
        _ => SimdLayout::Unsupported,
    };
    SimdImageShape {
        width: img.width(),
        height: img.height(),
        layout,
    }
}

fn shape_channels(shape: SimdImageShape) -> Option<usize> {
    match shape.layout {
        SimdLayout::Luma8 => Some(1),
        SimdLayout::LumaA8 => Some(2),
        SimdLayout::Rgb8 => Some(3),
        SimdLayout::Rgba8 => Some(4),
        SimdLayout::Luma16 | SimdLayout::Unsupported => None,
    }
}

fn shape_mode_matches(shape: SimdImageShape, mode: Option<&str>) -> bool {
    match shape.layout {
        SimdLayout::Luma8 => matches!(mode, None | Some("1" | "L" | "P")),
        SimdLayout::LumaA8 => matches!(mode, None | Some("LA" | "PA")),
        SimdLayout::Rgb8 => matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")),
        SimdLayout::Rgba8 => {
            matches!(
                mode,
                None | Some("RGBA" | "CMYK" | "RGBa" | "RGBX" | "I" | "F")
            )
        }
        SimdLayout::Luma16 => false,
        SimdLayout::Unsupported => false,
    }
}

fn shape_native_byte_channels(shape: SimdImageShape, mode: Option<&str>) -> Option<usize> {
    let channels = shape_channels(shape)?;
    let valid = match shape.layout {
        SimdLayout::Luma8 => matches!(mode, None | Some("L")),
        SimdLayout::LumaA8 => matches!(mode, None | Some("LA")),
        SimdLayout::Rgb8 => matches!(mode, None | Some("RGB")),
        SimdLayout::Rgba8 => matches!(mode, None | Some("RGBA" | "RGBa" | "RGBX")),
        SimdLayout::Luma16 => false,
        SimdLayout::Unsupported => false,
    };
    valid.then_some(channels)
}

fn shape_native_typed_filter_channels(shape: SimdImageShape, mode: Option<&str>) -> Option<usize> {
    (shape.layout == SimdLayout::Rgba8 && matches!(mode, Some("I" | "F"))).then_some(4)
}

fn shape_has_nonempty_byte_data(shape: SimdImageShape, channels: usize) -> bool {
    shape.width != 0
        && shape.height != 0
        && shape
            .width
            .checked_mul(shape.height)
            .and_then(|pixels| pixels.checked_mul(channels as u32))
            .is_some_and(|bytes| bytes != 0)
}

fn shape_has_vectorized_native_identity_copy(shape: SimdImageShape, channels: usize) -> bool {
    shape
        .width
        .checked_mul(shape.height)
        .and_then(|pixels| pixels.checked_mul(channels as u32))
        .is_some_and(|bytes| bytes >= 16)
}

fn shape_native_copy_channels(shape: SimdImageShape, mode: Option<&str>) -> Option<usize> {
    let channels = shape_channels(shape)?;
    shape_mode_matches(shape, mode).then_some(channels)
}

fn shape_luma16_transpose_layout(shape: SimdImageShape, mode: Option<&str>) -> bool {
    shape.layout == SimdLayout::Luma16
        && matches!(mode, None | Some("I;16" | "I;16L" | "I;16B" | "I;16N"))
}

fn shape_luma16_copy_layout(shape: SimdImageShape, mode: Option<&str>) -> bool {
    shape_luma16_transpose_layout(shape, mode)
}

fn shape_native_identity_copy_channels(shape: SimdImageShape, mode: Option<&str>) -> Option<usize> {
    match shape.layout {
        SimdLayout::Luma8 if matches!(mode, None | Some("1" | "L" | "P")) => Some(1),
        SimdLayout::LumaA8 if matches!(mode, None | Some("LA" | "PA")) => Some(2),
        SimdLayout::Rgb8 if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) => Some(3),
        SimdLayout::Rgba8 if matches!(mode, None | Some("RGBA" | "CMYK" | "RGBa" | "I" | "F")) => {
            Some(4)
        }
        _ => None,
    }
}

fn shape_has_empty_native_bytes(shape: SimdImageShape, channels: usize) -> bool {
    (shape.width == 0 || shape.height == 0)
        && shape
            .width
            .checked_mul(shape.height)
            .and_then(|pixels| pixels.checked_mul(channels as u32))
            == Some(0)
}

fn shape_has_vector_rows(shape: SimdImageShape, channels: usize) -> bool {
    shape.height != 0
        && shape
            .width
            .checked_mul(channels as u32)
            .is_some_and(|row_bytes| row_bytes >= 16)
}

fn shape_has_vectorized_offset_rows(shape: SimdImageShape, channels: usize, xoffset: i32) -> bool {
    if shape.width == 0 || shape.height == 0 {
        return false;
    }
    let Some(row_bytes) = shape.width.checked_mul(channels as u32) else {
        return false;
    };
    let _ = xoffset;
    row_bytes
        .checked_mul(shape.height)
        .is_some_and(|total_bytes| total_bytes >= 16)
}

fn shape_has_vectorized_luma16_offset(shape: SimdImageShape, mode: Option<&str>) -> bool {
    shape.layout == SimdLayout::Luma16
        && matches!(mode, None | Some("I;16" | "I;16L" | "I;16B" | "I;16N"))
        && (shape_has_empty_native_bytes(shape, 2) || (shape.width != 0 && shape.height != 0))
}

fn shape_has_vectorized_mirror_rows(shape: SimdImageShape, channels: usize) -> bool {
    shape.height != 0 && shape.width != 0 && matches!(channels, 1..=4)
}

fn shape_after_simd_op(shape: SimdImageShape, op: &PipelineOp) -> Option<SimdImageShape> {
    let mut next = shape;
    match op {
        PipelineOp::Crop {
            left,
            top,
            right,
            bottom,
        } => {
            if *left >= *right || *top >= *bottom || *right > shape.width || *bottom > shape.height
            {
                return None;
            }
            next.width = *right - *left;
            next.height = *bottom - *top;
        }
        PipelineOp::CropBorder { border } => {
            let twice = border.saturating_mul(2);
            if twice > shape.width || twice > shape.height {
                return None;
            }
            next.width = shape.width - twice;
            next.height = shape.height - twice;
        }
        PipelineOp::Expand { border, .. } => {
            let twice = border.checked_mul(2)?;
            next.width = shape.width.checked_add(twice)?;
            next.height = shape.height.checked_add(twice)?;
        }
        PipelineOp::Constant { .. } => next.layout = SimdLayout::Luma8,
        PipelineOp::Grayscale => next.layout = SimdLayout::Luma8,
        PipelineOp::Colorize { .. } => next.layout = SimdLayout::Rgb8,
        PipelineOp::Transpose { method } => {
            if matches!(
                method,
                TransposeMethod::Rotate90
                    | TransposeMethod::Rotate270
                    | TransposeMethod::Transpose
                    | TransposeMethod::Transverse
            ) {
                std::mem::swap(&mut next.width, &mut next.height);
            }
        }
        PipelineOp::ExtractBand { .. } => next.layout = SimdLayout::Luma8,
        PipelineOp::EffectNoise { .. } => next.layout = effects::output_layout(),
        PipelineOp::Convert { mode, .. } => {
            next.layout = match mode {
                ColorMode::L => SimdLayout::Luma8,
                ColorMode::LA => SimdLayout::LumaA8,
                ColorMode::RGB | ColorMode::YCbCr | ColorMode::HSV => SimdLayout::Rgb8,
                ColorMode::RGBA | ColorMode::CMYK => SimdLayout::Rgba8,
                _ => SimdLayout::Unsupported,
            };
        }
        PipelineOp::ConvertLab { .. } => next.layout = SimdLayout::Rgb8,
        PipelineOp::Reduce { x_factor, y_factor } => {
            next.width = shape.width.div_ceil((*x_factor).max(1));
            next.height = shape.height.div_ceil((*y_factor).max(1));
        }
        PipelineOp::Resize { w, h, .. } => {
            next.width = *w;
            next.height = *h;
        }
        PipelineOp::ResizeBoxed { w, h, .. } => {
            next.width = *w;
            next.height = *h;
        }
        PipelineOp::Scale { factor, .. } => {
            (next.width, next.height) =
                resize::native_scale_dimensions(shape.width, shape.height, *factor)?;
        }
        PipelineOp::Thumbnail { w, h, .. } => {
            let (output_width, output_height) =
                thumbnail::native_thumbnail_dimensions(shape.width, shape.height, *w, *h)?;
            next.width = output_width;
            next.height = output_height;
        }
        PipelineOp::Contain { w, h, .. } => {
            if shape.width != 0 && shape.height != 0 {
                let (output_width, output_height) =
                    resize::native_pad_contained_dimensions(shape.width, shape.height, *w, *h)?;
                next.width = output_width;
                next.height = output_height;
            }
        }
        PipelineOp::Cover { w, h, .. } => {
            let (output_width, output_height) =
                resize::native_cover_dimensions(shape.width, shape.height, *w, *h)?;
            next.width = output_width;
            next.height = output_height;
        }
        PipelineOp::Fit { w, h, .. } => {
            next.width = (*w).max(1);
            next.height = (*h).max(1);
        }
        PipelineOp::Transform { w, h, .. } => {
            next.width = *w;
            next.height = *h;
        }
        PipelineOp::Pad { w, h, .. } => {
            next.width = *w;
            next.height = *h;
        }
        PipelineOp::Color3DLut { target_mode, .. } => {
            next.layout = match target_mode {
                PixelMode::RGB => SimdLayout::Rgb8,
                PixelMode::RGBA | PixelMode::CMYK => SimdLayout::Rgba8,
                _ => SimdLayout::Unsupported,
            };
        }
        PipelineOp::Merge { mode, .. } => {
            next.layout = match mode {
                ColorMode::L | ColorMode::Mode1 => SimdLayout::Luma8,
                ColorMode::LA => SimdLayout::LumaA8,
                ColorMode::RGB => SimdLayout::Rgb8,
                ColorMode::RGBA | ColorMode::CMYK => SimdLayout::Rgba8,
                _ => SimdLayout::Unsupported,
            };
        }
        PipelineOp::CompositeModule { other, .. } => {
            let output_mode = other.mode().ok()?;
            (next.width, next.height) = other.size().ok()?;
            next.layout = match output_mode.as_str() {
                "1" | "L" | "P" => SimdLayout::Luma8,
                "LA" | "PA" => SimdLayout::LumaA8,
                "RGB" | "HSV" | "YCbCr" => SimdLayout::Rgb8,
                "RGBA" | "CMYK" | "RGBa" | "RGBX" | "I" | "F" => SimdLayout::Rgba8,
                _ => SimdLayout::Unsupported,
            };
        }
        PipelineOp::PutAlpha { .. } | PipelineOp::PutAlphaData { .. } => {
            put_alpha::update_shape(&mut next, op);
        }
        _ => {}
    }
    Some(next)
}

fn color_mode_name(mode: &ColorMode) -> &'static str {
    match mode {
        ColorMode::L => "L",
        ColorMode::LA => "LA",
        ColorMode::RGB => "RGB",
        ColorMode::RGBA => "RGBA",
        ColorMode::CMYK => "CMYK",
        ColorMode::YCbCr => "YCbCr",
        ColorMode::HSV => "HSV",
        ColorMode::I => "I",
        ColorMode::F => "F",
        ColorMode::P => "P",
        ColorMode::Mode1 => "1",
    }
}

fn pixel_mode_name(mode: PixelMode) -> &'static str {
    match mode {
        PixelMode::L => "L",
        PixelMode::LA => "LA",
        PixelMode::RGB => "RGB",
        PixelMode::RGBA => "RGBA",
        PixelMode::P => "P",
        PixelMode::PA => "PA",
        PixelMode::CMYK => "CMYK",
        PixelMode::Mode1 => "1",
        PixelMode::YCbCr => "YCbCr",
        PixelMode::HSV => "HSV",
        PixelMode::I => "I",
        PixelMode::F => "F",
    }
}

fn concrete_simd_mode(img: &DynamicImage) -> Option<&'static str> {
    match img {
        DynamicImage::ImageLuma8(_) => Some("L"),
        DynamicImage::ImageLumaA8(_) => Some("LA"),
        DynamicImage::ImageRgb8(_) => Some("RGB"),
        DynamicImage::ImageRgba8(_) => Some("RGBA"),
        _ => None,
    }
}

fn operation_target_mode(op: &PipelineOp) -> Option<&str> {
    match op {
        PipelineOp::Convert { mode, .. } => Some(color_mode_name(mode)),
        PipelineOp::ConvertLab { .. } => Some("LAB"),
        // A merge's storage family can differ from its logical mode (LAB
        // uses RGB storage). Subsequent operations need the semantic tag.
        PipelineOp::Merge { logical_mode, .. } => Some(logical_mode),
        PipelineOp::Color3DLut { target_mode, .. } => Some(pixel_mode_name(*target_mode)),
        PipelineOp::PutAlpha { .. } | PipelineOp::PutAlphaData { .. } => put_alpha::output_mode(op),
        PipelineOp::Grayscale => Some("L"),
        PipelineOp::EffectNoise { .. } => Some(effects::output_mode()),
        PipelineOp::Colorize { .. } => Some("RGB"),
        PipelineOp::Constant { .. } => Some("L"),
        PipelineOp::ExtractBand { .. } => Some("L"),
        _ => None,
    }
}

/// Resolve the logical mode at the first operation boundary.
///
/// Pipeline metadata carries the final output mode, while the concrete input
/// buffer still has the source layout. If a pipeline ends in a nonstandard
/// conversion (for example RGB -> CMYK), passing that final tag to the first
/// `PutPixel` or `Convert` operation would make a valid native source look
/// unsupported. Use the concrete source mode whenever the operation list
/// contains the final mode transition; preserving a nonstandard mode without
/// such a transition remains necessary for raw HSV/YCbCr/CMYK samples.
pub(crate) fn simd_initial_mode(
    img: &DynamicImage,
    ops: &[PipelineOp],
    final_mode: Option<&str>,
) -> Option<String> {
    let Some(final_mode) = final_mode else {
        return None;
    };
    // `PA` shares the two-byte `LumaA8` storage with `LA`, but its first byte
    // is a palette index rather than luma.  Keep the logical tag at this
    // boundary; replacing it with the concrete raster mode would make a
    // valid PA-only adapter look unsupported before execution begins.
    if final_mode == "PA" {
        return Some(final_mode.to_owned());
    }
    if let Some(PipelineOp::Color3DLut { source_mode, .. }) = ops.first() {
        return Some(pixel_mode_name(*source_mode).to_owned());
    }
    if ops
        .iter()
        .any(|op| operation_target_mode(op) == Some(final_mode))
    {
        return concrete_simd_mode(img)
            .map(str::to_owned)
            .or_else(|| Some(final_mode.to_owned()));
    }
    Some(final_mode.to_owned())
}

/// Advance the logical mode after one pipeline operation.
pub(crate) fn simd_mode_after_op(op: &PipelineOp, current: Option<&str>) -> Option<String> {
    if let PipelineOp::CompositeModule { other, .. } = op {
        return other.mode().ok();
    }
    if let Some(target) = operation_target_mode(op) {
        // Equalize expands indexed samples before its histogram operation.
        if matches!(op, PipelineOp::Equalize | PipelineOp::EqualizeMasked { .. }) {
            return Some(target.to_owned());
        }
        return Some(target.to_owned());
    }
    if matches!(op, PipelineOp::Equalize | PipelineOp::EqualizeMasked { .. })
        && matches!(current, Some("P" | "PA"))
    {
        return Some("RGB".to_owned());
    }
    current.map(str::to_owned)
}

/// Find the first operation that would be unsupported for the actual
/// intermediate layout, without allocating or touching pixel data.
///
/// Strict SIMD uses this shape-only pass before entering the first adapter.
/// It closes the subtle gap where an early crop/extraction/alpha promotion
/// changes the layout enough to make a later operation unsupported.
pub(crate) fn first_unsupported_simd_op<'a>(
    img: &DynamicImage,
    ops: &'a [PipelineOp],
    mode: Option<&str>,
) -> Option<&'a PipelineOp> {
    let mut shape = simd_shape(img);
    let mut current_mode = simd_initial_mode(img, ops, mode);
    for op in ops {
        let op_mode = current_mode.as_deref();
        if !crate::compute::registry::simd_supports(op).unwrap_or(false) {
            return Some(op);
        }
        if !simd_supports_for_shape(shape, op, op_mode) {
            return Some(op);
        }
        shape = shape_after_simd_op(shape, op)?;
        current_mode = simd_mode_after_op(op, op_mode);
    }
    None
}

fn simd_supports_for_shape(shape: SimdImageShape, op: &PipelineOp, mode: Option<&str>) -> bool {
    let pixel_count = (shape.width as usize).saturating_mul(shape.height as usize);
    let native_byte_channels = shape_native_byte_channels(shape, mode);
    let native_filter_byte_channels = shape_native_filter_byte_channels(shape, mode);
    let native_copy = shape_native_copy_channels(shape, mode).is_some();
    let native_luma16_transpose = shape_luma16_transpose_layout(shape, mode);
    let native_luma16_copy =
        shape_luma16_copy_layout(shape, mode) && shape.width != 0 && shape.height != 0;
    match op {
        PipelineOp::Invert | PipelineOp::InvertChops => {
            effects::supports_for_shape(shape, op, mode)
        }
        PipelineOp::ResizeBoxed { .. }
        | PipelineOp::Resize { .. }
        | PipelineOp::Scale { .. }
        | PipelineOp::Contain { .. }
        | PipelineOp::Cover { .. }
        | PipelineOp::Fit { .. } => resize::supports_for_shape(shape, op, mode),
        PipelineOp::Thumbnail { w, h, filter } => {
            thumbnail::native_thumbnail_supported_for_shape(shape, *w, *h, *filter, mode)
        }
        PipelineOp::Transform {
            w,
            h,
            method,
            data,
            filter,
            ..
        } => transform::supports_for_shape(shape, *w, *h, method, data, *filter, mode),
        PipelineOp::Reduce { x_factor, y_factor } => {
            native_reduce_supported_for_shape(shape, *x_factor, *y_factor, mode)
        }
        PipelineOp::Convert {
            mode: target,
            matrix,
            dither: _,
        } => native_convert_supported_for_shape(shape, target, matrix.as_deref(), mode),
        PipelineOp::ConvertLab { .. } => {
            matches!(shape.layout, SimdLayout::Rgb8)
                && matches!(mode, None | Some("RGB"))
                && (shape.width as usize)
                    .checked_mul(shape.height as usize)
                    .is_some()
        }
        PipelineOp::Grayscale => grayscale::supports_shape(shape, mode),
        PipelineOp::Colorize { .. } => colorize::supports_for_shape(shape, mode),
        PipelineOp::Solarize { .. }
        | PipelineOp::Posterize { .. }
        | PipelineOp::Brightness { .. }
        | PipelineOp::Contrast { .. }
        | PipelineOp::ColorSaturation { .. }
        | PipelineOp::Sharpness { .. } => enhance::supports_for_shape(shape, op, mode),
        PipelineOp::Autocontrast { .. }
        | PipelineOp::Equalize
        | PipelineOp::EqualizeMasked { .. } => histogram::supports_for_shape(shape, op, mode),
        PipelineOp::RemapPalette { .. } | PipelineOp::Eval { .. } => {
            lut::supports_for_shape(shape, op, mode)
        }
        PipelineOp::PutData {
            mode: data_mode, ..
        } => native_put_data_shape_layout(shape, *data_mode, mode).is_some(),
        PipelineOp::ExtractBand { index } => {
            extract_band::supports_for_shape(shape, *index, mode, pixel_count)
        }
        PipelineOp::Offset { x, .. } => {
            shape_has_vectorized_luma16_offset(shape, mode)
                || shape_native_copy_channels(shape, mode).is_some_and(|channels| {
                    shape_has_empty_native_bytes(shape, channels)
                        || shape_has_vectorized_offset_rows(shape, channels, *x)
                })
        }
        PipelineOp::Flip => {
            shape_native_copy_channels(shape, mode)
                .is_some_and(|channels| shape_has_nonempty_byte_data(shape, channels))
                || native_luma16_copy
        }
        PipelineOp::Mirror => {
            shape_native_copy_channels(shape, mode)
                .is_some_and(|channels| shape_has_vectorized_mirror_rows(shape, channels))
                || native_luma16_copy
        }
        PipelineOp::Transpose { .. } => {
            (native_copy || native_luma16_transpose)
                && (pixel_count != 0
                    || native_copy
                        && shape_has_empty_native_bytes(shape, shape_channels(shape).unwrap_or(0)))
        }
        PipelineOp::Crop {
            left,
            top,
            right,
            bottom,
        } => {
            (native_copy || native_luma16_copy)
                && pixel_count != 0
                && *left < *right
                && *top < *bottom
                && *right <= shape.width
                && *bottom <= shape.height
        }
        PipelineOp::CropBorder { .. } => {
            // Invalid geometry is still a scalar control-plane concern; the
            // adapter must be allowed to return Pillow's ValueError instead
            // of strict SIMD masking it as an unsupported operation.
            native_copy
        }
        PipelineOp::Expand { border, .. } => {
            native_expand_contract_for_shape(shape, *border, mode).is_some()
        }
        PipelineOp::Pad { .. } => pad::supports_for_shape(shape, op, mode),
        PipelineOp::Constant { .. } => shape.width != 0 && shape.height != 0,
        PipelineOp::Duplicate => shape_native_copy_channels(shape, mode).is_some_and(|channels| {
            shape.width != 0
                && shape.height != 0
                && shape
                    .width
                    .checked_mul(shape.height)
                    .and_then(|pixels| pixels.checked_mul(channels as u32))
                    .is_some_and(|bytes| bytes != 0)
        }),
        PipelineOp::Filter3x3 { kernel, scale, .. } => {
            valid_convolution_parameters(kernel, *scale)
                && (native_filter_byte_channels.is_some()
                    && shape.width as usize >= 3
                    && shape.height as usize > 2
                    || shape_native_i32_convolution_path(shape, mode, 1)
                    || shape_native_filter_identity_supported(shape, mode, 1))
        }
        PipelineOp::Filter5x5 { kernel, scale, .. } => {
            valid_convolution_parameters(kernel, *scale)
                && (native_filter_byte_channels.is_some()
                    && shape.width as usize >= 5
                    && shape.height as usize > 4
                    || shape_native_i32_convolution_path(shape, mode, 2)
                    || shape_native_filter_identity_supported(shape, mode, 2))
        }
        PipelineOp::BoxBlur { radius } => {
            if *radius == 0 {
                shape_native_byte_channels(shape, mode).is_some_and(|channels| {
                    shape_has_vectorized_native_identity_copy(shape, channels)
                })
            } else {
                native_byte_channels.is_some() && shape.width != 0 && shape.height != 0
            }
        }
        PipelineOp::BoxBlurXY {
            radius_x,
            radius_y,
            passes,
        } => {
            native_byte_channels.is_some()
                && *passes != 0
                && radius_x.is_finite()
                && radius_y.is_finite()
                && *radius_x >= 0.0
                && *radius_y >= 0.0
                && ((*radius_x != 0.0 || *radius_y != 0.0) && shape.width != 0 && shape.height != 0
                    || (*radius_x == 0.0
                        && *radius_y == 0.0
                        && shape_native_byte_channels(shape, mode).is_some_and(|channels| {
                            shape_has_vectorized_native_identity_copy(shape, channels)
                        })))
        }
        PipelineOp::GaussianBlur { sigma } => {
            gaussian_blur::supports_for_shape(shape, *sigma, mode)
        }
        PipelineOp::MaxFilter { size } | PipelineOp::MinFilter { size } => {
            (native_filter_byte_channels.is_some()
                && *size != 0
                && *size % 2 == 1
                && shape.width != 0
                && shape.height != 0)
                || median_filter::shape_native_float_rank_supported(
                    shape,
                    mode,
                    *size,
                    if matches!(op, PipelineOp::MaxFilter { .. }) {
                        size.saturating_mul(*size).saturating_sub(1)
                    } else {
                        0
                    },
                )
        }
        PipelineOp::MedianFilter { size } => {
            (native_filter_byte_channels.is_some()
                && *size != 0
                && *size % 2 == 1
                && *size <= 15
                && shape.width != 0
                && shape.height != 0)
                || median_filter::shape_native_float_rank_supported(
                    shape,
                    mode,
                    *size,
                    size.saturating_mul(*size) / 2,
                )
        }
        PipelineOp::RankFilter { size, rank } => {
            let area = u64::from(*size).saturating_mul(u64::from(*size));
            (native_filter_byte_channels.is_some()
                && *size != 0
                && *size % 2 == 1
                && area <= 225
                && u64::from(*rank) < area
                && shape.width != 0
                && shape.height != 0)
                || median_filter::shape_native_float_rank_supported(shape, mode, *size, *rank)
        }
        PipelineOp::PutAlpha { .. } | PipelineOp::PutAlphaData { .. } => {
            put_alpha::supports_for_shape(shape, op, mode)
        }
        PipelineOp::EffectNoise { .. } | PipelineOp::EffectSpread { .. } => {
            effects::supports_for_shape(shape, op, mode)
        }
        PipelineOp::Color3DLut {
            size,
            table,
            channels,
            source_mode,
            target_mode,
        } => color3dlut_supported_for_shape(
            shape,
            *size,
            table.len(),
            *channels,
            *source_mode,
            *target_mode,
            mode,
        ),
        PipelineOp::Rotate { .. } => rotate::supports_for_shape(shape, op, mode),
        PipelineOp::DrawLine { .. }
        | PipelineOp::DrawPoint { .. }
        | PipelineOp::DrawEllipse { .. }
        | PipelineOp::DrawCircle { .. }
        | PipelineOp::DrawArc { .. }
        | PipelineOp::DrawChord { .. }
        | PipelineOp::DrawPieslice { .. }
        | PipelineOp::DrawRoundedRect { .. }
        | PipelineOp::DrawPolygon { .. }
        | PipelineOp::DrawRectangle { .. } => draw::supports_for_shape(shape, op, mode),
        PipelineOp::PutPixel {
            x,
            y,
            palette_index: _,
            ..
        } => put_pixel::native_putpixel_supported_for_shape(shape, *x, *y, mode),
        PipelineOp::AlphaComposite { source, dest, src } => {
            *dest == (0, 0)
                && *src == (0, 0)
                && simd_alpha_composite_shape_supported(shape, source, mode)
        }
        PipelineOp::CompositeModule {
            other,
            mask,
            mask_alpha,
        } => native_composite_plan_for_shape(shape, other, mask, *mask_alpha, mode).is_some(),
        PipelineOp::Paste {
            source,
            x,
            y,
            w,
            h,
            mask,
            mask_alpha,
        } => {
            native_paste_plan_for_shape(
                shape,
                source,
                *x,
                *y,
                *w,
                *h,
                mask.as_ref(),
                *mask_alpha,
                mode,
            )
            .is_some()
                || native_paste_rgba_to_rgb_supported_for_shape(
                    shape,
                    source,
                    *w,
                    *h,
                    mask.as_ref(),
                    mode,
                )
                || native_paste_rgb_to_rgba_supported_for_shape(
                    shape,
                    source,
                    *w,
                    *h,
                    mask.as_ref(),
                    mode,
                )
        }
        PipelineOp::Merge {
            mode: target_mode,
            bands,
            logical_mode,
        } => merge::supports_shape_contract(shape, target_mode, bands, mode, logical_mode),
        PipelineOp::BlendModule { .. } => module_blend::supports_for_shape(shape, op, mode),
        // ImageChops owns stored-sample admission and paired-operand validation.
        PipelineOp::Multiply { .. }
        | PipelineOp::Screen { .. }
        | PipelineOp::Darker { .. }
        | PipelineOp::Lighter { .. }
        | PipelineOp::Difference { .. }
        | PipelineOp::AddModulo { .. }
        | PipelineOp::SubtractModulo { .. }
        | PipelineOp::Overlay { .. }
        | PipelineOp::HardLight { .. }
        | PipelineOp::SoftLight { .. }
        | PipelineOp::LogicalAnd { .. }
        | PipelineOp::LogicalOr { .. }
        | PipelineOp::LogicalXor { .. }
        | PipelineOp::Add { .. }
        | PipelineOp::Subtract { .. } => chops::supports_for_shape(shape, op, mode),
    }
}

// Native byte transforms always operate on L/LA/RGB/RGBA layouts. Keeping
// their active-channel masks in one table avoids duplicating an input-driven
// branch in every closure monomorphization of `native_byte_transform`.
const NATIVE_BYTE_ZERO_MASK: [u8; 16] = [0; 16];
const NATIVE_BYTE_ALL_MASK: [u8; 16] = [u8::MAX; 16];
const NATIVE_BYTE_LA_MASK: [u8; 16] = [
    u8::MAX,
    0,
    u8::MAX,
    0,
    u8::MAX,
    0,
    u8::MAX,
    0,
    u8::MAX,
    0,
    u8::MAX,
    0,
    u8::MAX,
    0,
    u8::MAX,
    0,
];
const NATIVE_BYTE_RGBA_MASK: [u8; 16] = [
    u8::MAX,
    u8::MAX,
    u8::MAX,
    0,
    u8::MAX,
    u8::MAX,
    u8::MAX,
    0,
    u8::MAX,
    u8::MAX,
    u8::MAX,
    0,
    u8::MAX,
    u8::MAX,
    u8::MAX,
    0,
];

const NATIVE_BYTE_ACTIVE_MASKS: [[u8; 16]; 5] = [
    NATIVE_BYTE_ZERO_MASK,
    NATIVE_BYTE_ALL_MASK,
    NATIVE_BYTE_LA_MASK,
    NATIVE_BYTE_ALL_MASK,
    NATIVE_BYTE_RGBA_MASK,
];

#[inline]
fn native_byte_transform_bytes<F>(bytes: &mut [u8], channels: usize, transform: &F)
where
    F: Fn(u8x16) -> u8x16,
{
    // Native-byte callers pass only channel counts 1..=4, so this lookup is
    // total for every supported native byte image.
    let active = NATIVE_BYTE_ACTIVE_MASKS[channels];
    let active_vector = u8x16::new(active);
    let inactive = u8x16::splat(u8::MAX) - active_vector;
    let mut chunks = bytes.chunks_exact_mut(16);
    for chunk in &mut chunks {
        let input = u8x16::new(
            <[u8; 16]>::try_from(&*chunk).expect("chunks_exact_mut yields 16-byte chunks"),
        );
        let transformed = transform(input);
        let output = (transformed & active_vector) | (input & inactive);
        chunk.copy_from_slice(&output.to_array());
    }
    let remainder = chunks.into_remainder();
    if !remainder.is_empty() {
        // A short row/image still has a genuine vector data path: copy only
        // the valid bytes into a stack-padded block, run the same SIMD kernel,
        // then commit only those lanes. This avoids pretending that a scalar
        // per-byte tail is SIMD while also avoiding an out-of-bounds load.
        let mut padded = [0u8; 16];
        padded[..remainder.len()].copy_from_slice(remainder);
        let input = u8x16::new(padded);
        let transformed = transform(input);
        let output = (transformed & active_vector) | (input & inactive);
        remainder.copy_from_slice(&output.to_array()[..remainder.len()]);
    }
}

#[inline]
fn native_all_channel_transform_bytes<F>(bytes: &mut [u8], transform: &F)
where
    F: Fn(u8x16) -> u8x16,
{
    let mut chunks = bytes.chunks_exact_mut(16);
    for chunk in &mut chunks {
        let input = u8x16::new(
            <[u8; 16]>::try_from(&*chunk).expect("chunks_exact_mut yields 16-byte chunks"),
        );
        chunk.copy_from_slice(&transform(input).to_array());
    }
    let remainder = chunks.into_remainder();
    if !remainder.is_empty() {
        let mut padded = [0u8; 16];
        padded[..remainder.len()].copy_from_slice(remainder);
        remainder.copy_from_slice(&transform(u8x16::new(padded)).to_array()[..remainder.len()]);
    }
}

/// Transform native posterize samples directly from the immutable source into
/// the one output allocation. Cloning and then mutating the clone makes two
/// full-frame passes for a point operation whose output depends only on each
/// input byte.
#[inline]
fn native_byte_transform_copy_bytes<F>(input: &[u8], output: &mut [u8], transform: &F)
where
    F: Fn(u8x16) -> u8x16,
{
    let mut input_chunks = input.chunks_exact(16);
    let mut output_chunks = output.chunks_exact_mut(16);
    for (source, destination) in input_chunks.by_ref().zip(output_chunks.by_ref()) {
        let vector =
            u8x16::new(<[u8; 16]>::try_from(source).expect("chunks_exact yields 16-byte chunks"));
        destination.copy_from_slice(&transform(vector).to_array());
    }
    let input_tail = input_chunks.remainder();
    let output_tail = output_chunks.into_remainder();
    if !input_tail.is_empty() {
        let mut padded = [0u8; 16];
        padded[..input_tail.len()].copy_from_slice(input_tail);
        let transformed = transform(u8x16::new(padded)).to_array();
        output_tail.copy_from_slice(&transformed[..input_tail.len()]);
    }
}

#[inline]
fn apply_native_rows<F>(
    bytes: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    transform: F,
) where
    F: Fn(&mut [u8]) + Send + Sync,
{
    #[cfg(feature = "parallel")]
    let row_stride = width.saturating_mul(channels);
    #[cfg(feature = "parallel")]
    if bytes.len() >= 256 * 1024 && row_stride != 0 {
        simd_rows_mut_serial!(
            bytes,
            row_stride,
            height,
            |_row_start, _row_end, _y, row| {
                transform(row);
            }
        );
    } else {
        if row_stride != 0 {
            for row in bytes.chunks_mut(row_stride).take(height) {
                transform(row);
            }
        }
    }
    #[cfg(not(feature = "parallel"))]
    {
        let row_stride = width.saturating_mul(channels);
        if row_stride != 0 {
            for row in bytes.chunks_mut(row_stride).take(height) {
                transform(row);
            }
        }
    }
}

/// Whether an admitted operation can reuse an owned intermediate without
/// changing its concrete layout.  This is deliberately narrower than SIMD
/// capability: native-copy, blur, convolution, and other shape-changing
/// kernels still need their own output/scratch storage.
pub(crate) fn simd_in_place_supported(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> bool {
    match op {
        PipelineOp::Invert | PipelineOp::InvertChops => effects::supports_in_place(img, mode),
        PipelineOp::Solarize { .. }
        | PipelineOp::Posterize { .. }
        | PipelineOp::Brightness { .. } => enhance::supports_in_place(img, op, mode),
        PipelineOp::RemapPalette { .. } | PipelineOp::Eval { .. } => {
            lut::in_place_supported(img, op, mode)
        }
        PipelineOp::Paste {
            source,
            x,
            y,
            w,
            h,
            mask,
            mask_alpha,
        } => native_paste_plan_for_image(
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
        .is_some(),
        PipelineOp::Add { .. } | PipelineOp::Subtract { .. } => {
            chops::supports_in_place(img, op, mode)
        }
        _ => false,
    }
}

/// Reuse an owned SIMD-segment buffer for operations whose native byte
/// contract is unchanged.  Shape-changing operations deliberately return
/// `false` and continue through their allocating native kernel.
pub(crate) fn simd_execute_in_place(
    img: &mut DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<bool, PilError> {
    match op {
        PipelineOp::Invert => Ok(effects::execute_invert_in_place(img, mode, false)),
        PipelineOp::InvertChops => Ok(effects::execute_invert_in_place(img, mode, true)),
        PipelineOp::Solarize { .. }
        | PipelineOp::Posterize { .. }
        | PipelineOp::Brightness { .. } => enhance::execute_in_place(img, op, mode),
        PipelineOp::RemapPalette { .. } | PipelineOp::Eval { .. } => {
            Ok(lut::execute_in_place(img, op, mode))
        }
        PipelineOp::Paste {
            source,
            x,
            y,
            w,
            h,
            mask,
            mask_alpha,
        } => native_paste_in_place(
            img,
            source,
            *x,
            *y,
            *w,
            *h,
            mask.as_ref(),
            *mask_alpha,
            mode,
        ),
        PipelineOp::Multiply { .. }
        | PipelineOp::Screen { .. }
        | PipelineOp::Darker { .. }
        | PipelineOp::Lighter { .. }
        | PipelineOp::Difference { .. }
        | PipelineOp::AddModulo { .. }
        | PipelineOp::SubtractModulo { .. }
        | PipelineOp::LogicalAnd { .. }
        | PipelineOp::LogicalOr { .. }
        | PipelineOp::LogicalXor { .. }
        | PipelineOp::Add { .. }
        | PipelineOp::Subtract { .. } => chops::execute_in_place(img, op, mode),
        _ => Ok(false),
    }
}

fn simd_div255(value: u16x16) -> u16x16 {
    // For 0 <= value <= 65025, this is exactly floor(value / 255), including
    // both endpoints.  It replaces an integer divide without changing the
    // intermediate truncation required by ImageChops.multiply/screen.
    // A runtime splat can emit memset_pattern16 in every vector iteration.
    const ONE: u16x16 = u16x16::splat(1);
    let incremented = value + ONE;
    (incremented + (incremented >> 8u32)) >> 8u32
}

#[inline]
fn simd_div255_u16x8(value: u16x8) -> u16x8 {
    const ONE: u16x8 = u16x8::splat(1);
    let incremented = value + ONE;
    (incremented + (incremented >> 8u32)) >> 8u32
}

#[inline]
pub(super) fn simd_pack_u16x16(value: u16x16) -> u8x16 {
    let [low, high]: [u16x8; 2] = bytemuck::cast(value);
    let low: i16x8 = bytemuck::cast(low);
    let high: i16x8 = bytemuck::cast(high);
    u8x16::narrow_i16x8(low, high)
}

/// Apply Pillow's exact 150% UnsharpMask blend to packed native RGB bytes.
///
/// At 150%, `diff * percent / 100` is truncating `diff * 3 / 2`. Computing
/// the magnitude first makes the signed truncation exact with unsigned lanes:
/// `sign(diff) * floor(abs(diff) * 3 / 2)`. The input is interleaved RGB, but
/// the operation is independent per byte and therefore needs no channel
/// conversion or shuffle.
pub(crate) fn simd_unsharp_blend_rgb_150(
    original: &[u8],
    blurred_output: &mut [u8],
    threshold: i32,
) -> bool {
    if original.len() != blurred_output.len() {
        return false;
    }

    let mut original_chunks = original.chunks_exact(16);
    let mut output_chunks = blurred_output.chunks_exact_mut(16);
    for output_chunk in output_chunks.by_ref() {
        let Some(original_chunk) = original_chunks.next() else {
            return false;
        };
        let Ok(original_lanes) = <[u8; 16]>::try_from(original_chunk) else {
            return false;
        };
        let Ok(blurred_lanes) = <[u8; 16]>::try_from(&*output_chunk) else {
            return false;
        };
        let original_lanes = u16x16::from(u8x16::new(original_lanes));
        let blurred_lanes = u16x16::from(u8x16::new(blurred_lanes));
        let original_is_greater = original_lanes.simd_ge(blurred_lanes);
        let distance = original_is_greater.select(
            original_lanes - blurred_lanes,
            blurred_lanes - original_lanes,
        );
        let adjustment = (distance * u16x16::splat(3)) >> 1u32;
        // Preserve the signed subtraction through the u16 -> i16 reinterpret
        // in `simd_pack_u16x16`; its saturating narrow clamps both underflow
        // and overflow to the byte range. The adjustment is at most 382, so
        // wrapped negative lanes remain within i16's signed range.
        let sharpened =
            original_is_greater.select(original_lanes + adjustment, original_lanes - adjustment);
        let result = if threshold < 0 {
            sharpened
        } else {
            distance
                .simd_gt(u16x16::splat(threshold.min(255) as u16))
                .select(sharpened, original_lanes)
        };
        output_chunk.copy_from_slice(&simd_pack_u16x16(result).to_array());
    }

    for (original, output) in original_chunks
        .remainder()
        .iter()
        .zip(output_chunks.into_remainder())
    {
        let blurred = *output;
        let difference = i32::from(*original) - i32::from(blurred);
        let sharpened = if difference.abs() > threshold {
            let adjustment = difference * 150 / 100;
            (i32::from(*original) + adjustment).clamp(0, 255) as u8
        } else {
            *original
        };
        *output = sharpened;
    }

    true
}

#[inline]
fn simd_fused_multiply_screen_row(
    left_bytes: &[u8],
    right_bytes: &[u8],
    output: &mut [u8],
) -> bool {
    if left_bytes.len() != right_bytes.len() || left_bytes.len() != output.len() {
        return false;
    }

    let mut left_chunks = left_bytes.chunks_exact(16);
    let mut right_chunks = right_bytes.chunks_exact(16);
    let mut output_chunks = output.chunks_exact_mut(16);
    for ((left_chunk, right_chunk), output_chunk) in left_chunks
        .by_ref()
        .zip(right_chunks.by_ref())
        .zip(output_chunks.by_ref())
    {
        let Ok(left) = <[u8; 16]>::try_from(left_chunk) else {
            return false;
        };
        let Ok(right) = <[u8; 16]>::try_from(right_chunk) else {
            return false;
        };
        let left = u16x16::from(u8x16::new(left));
        let right = u16x16::from(u8x16::new(right));
        let multiplied = simd_div255(left * right);
        let result = multiplied + right - simd_div255(multiplied * right);
        output_chunk.copy_from_slice(&simd_pack_u16x16(result).to_array());
    }
    for ((&left, &right), output) in left_chunks
        .remainder()
        .iter()
        .zip(right_chunks.remainder())
        .zip(output_chunks.into_remainder())
    {
        let multiplied = (left as u32 * right as u32 / 255) as u8;
        *output = (255u32 - ((255 - multiplied as u32) * (255 - right as u32) / 255)) as u8;
    }
    true
}

pub(crate) fn simd_fused_multiply_screen(
    img: &DynamicImage,
    first_other: &Arc<Image>,
    second_other: &Arc<Image>,
    mode: Option<&str>,
) -> Result<Option<(DynamicImage, u64, u64)>, PilError> {
    if mode.is_some() || !first_other.shares_execution_source(second_other) {
        return Ok(None);
    }
    let other = materialize_chops_operand(first_other, mode)?;
    let channels = match (img, other.as_ref()) {
        (DynamicImage::ImageLuma8(_), DynamicImage::ImageLuma8(_)) => 1usize,
        (DynamicImage::ImageLumaA8(_), DynamicImage::ImageLumaA8(_)) => 2,
        (DynamicImage::ImageRgb8(_), DynamicImage::ImageRgb8(_)) => 3,
        (DynamicImage::ImageRgba8(_), DynamicImage::ImageRgba8(_)) => 4,
        _ => return Ok(None),
    };
    if img.dimensions() != other.dimensions() {
        return Ok(None);
    }

    let left_bytes = img.as_bytes();
    let right_bytes = other.as_bytes();
    let mut output = vec![0u8; left_bytes.len()];
    let (width, height) = img.dimensions();
    let row_stride = (width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("SIMD fused Chops row stride overflow".into()))?;
    if row_stride == 0 || row_stride.checked_mul(height as usize) != Some(output.len()) {
        return Ok(None);
    }
    // Both operands are tightly packed and each byte is independent. Stream
    // across rows; only the final tile needs a tail. Cheap fused arithmetic
    // must amortize scheduling just like the standalone native blend path.
    let vector_blocks = output.len() / 16;
    let scalar_tail = output.len() % 16;
    #[cfg(feature = "parallel")]
    if output.len() >= 4 * 1024 * 1024 {
        const TILE_BYTES: usize = 64 * 1024;
        let tiles = output.len().div_ceil(TILE_BYTES);
        simd_rows_mut_serial!(
            &mut output,
            TILE_BYTES,
            tiles,
            |row_start, _row_end, _y, row| {
                let row_end = row_start + row.len();
                let _ = simd_fused_multiply_screen_row(
                    &left_bytes[row_start..row_end],
                    &right_bytes[row_start..row_end],
                    row,
                );
            }
        );
    } else if !simd_fused_multiply_screen_row(left_bytes, right_bytes, &mut output) {
        return Ok(None);
    }

    #[cfg(not(feature = "parallel"))]
    if !simd_fused_multiply_screen_row(left_bytes, right_bytes, &mut output) {
        return Ok(None);
    }
    let result =
        crate::image_utils::raw_bytes_to_image_allow_empty(width, height, output, channels)?;
    Ok(Some((
        preserve_mode(img, result),
        vector_blocks as u64,
        scalar_tail as u64,
    )))
}

/// Reverse rows in native 8-bit storage without packing pixels into RGBA.
#[inline]
fn reverse_pixel_block(input: &[u8], channels: usize) -> Option<[u8; 16]> {
    let indices = match channels {
        1 => [15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0],
        2 => [14, 15, 12, 13, 10, 11, 8, 9, 6, 7, 4, 5, 2, 3, 0, 1],
        4 => [12, 13, 14, 15, 8, 9, 10, 11, 4, 5, 6, 7, 0, 1, 2, 3],
        _ => return None,
    };
    let input = u8x16::new(<[u8; 16]>::try_from(input).ok()?);
    Some(input.swizzle_relaxed(u8x16::new(indices)).to_array())
}

/// Reverse sixteen RGB pixels with three native byte vectors.
///
/// RGB pixels are three bytes wide, so a single `u8x16` cannot contain an
/// integral number of pixels. Three source vectors and fixed lane selects
/// still cover the complete 48-byte group without scalar pixel arithmetic.
#[inline]
fn reverse_rgb_block(input: &[u8]) -> Option<[u8; 48]> {
    let first = u8x16::new(<[u8; 16]>::try_from(input.get(..16)?).ok()?);
    let second = u8x16::new(<[u8; 16]>::try_from(input.get(16..32)?).ok()?);
    let third = u8x16::new(<[u8; 16]>::try_from(input.get(32..48)?).ok()?);

    let block0_from_third = third.swizzle_relaxed(u8x16::new([
        13, 14, 15, 10, 11, 12, 7, 8, 9, 4, 5, 6, 1, 2, 3, 0,
    ]));
    let block0_from_second = second.swizzle_relaxed(u8x16::splat(14));
    let block0 = u8x16::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, u8::MAX])
        .select(block0_from_second, block0_from_third);

    let block1_from_second = second.swizzle_relaxed(u8x16::new([
        15, 0, 11, 12, 13, 8, 9, 10, 5, 6, 7, 2, 3, 4, 0, 0,
    ]));
    let block1_from_third = third.swizzle_relaxed(u8x16::splat(0));
    let block1_from_first = first.swizzle_relaxed(u8x16::splat(15));
    let block1 = u8x16::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, u8::MAX, 0]).select(
        block1_from_first,
        u8x16::new([0, u8::MAX, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
            .select(block1_from_third, block1_from_second),
    );

    let block2_from_first = first.swizzle_relaxed(u8x16::new([
        0, 12, 13, 14, 9, 10, 11, 6, 7, 8, 3, 4, 5, 0, 1, 2,
    ]));
    let block2_from_second = second.swizzle_relaxed(u8x16::splat(1));
    let block2 = u8x16::new([u8::MAX, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
        .select(block2_from_second, block2_from_first);

    let mut output = [0u8; 48];
    output[..16].copy_from_slice(&block0.to_array());
    output[16..32].copy_from_slice(&block1.to_array());
    output[32..].copy_from_slice(&block2.to_array());
    Some(output)
}

/// Copy a byte span through the native vector type and account for its tail.
#[inline]
fn copy_native_bytes(source: &[u8], output: &mut [u8]) -> Option<(u64, u64)> {
    if source.len() != output.len() {
        return None;
    }
    if source.is_empty() {
        return Some((0, 0));
    }
    let vector_len = source.len() / 16 * 16;
    let mut vector_blocks = 0u64;
    for offset in (0..vector_len).step_by(16) {
        let block = u8x16::new(<[u8; 16]>::try_from(&source[offset..offset + 16]).ok()?);
        output[offset..offset + 16].copy_from_slice(&block.to_array());
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let scalar_tail = source.len() - vector_len;
    if scalar_tail != 0 {
        let mut padded = [0u8; 16];
        padded[..scalar_tail].copy_from_slice(&source[vector_len..]);
        let block = u8x16::new(padded);
        output[vector_len..].copy_from_slice(&block.to_array()[..scalar_tail]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    Some((vector_blocks, scalar_tail as u64))
}

/// Write a horizontally reversed row into a separate native output buffer.
///
/// Using a separate row is intentional: it avoids the source/destination
/// overlap constraints of an in-place swap and lets every complete group use
/// a fixed vector load/store. Only incomplete pixel groups use scalar copies.
fn mirror_native_row(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    channels: usize,
) -> Option<(u64, u64)> {
    let row_len = width.checked_mul(channels)?;
    if source.len() != row_len || output.len() != row_len {
        return None;
    }
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    if channels == 3 {
        let vector_pixels = width / 16 * 16;
        for output_pixel in (0..vector_pixels).step_by(16) {
            let source_pixel = width - output_pixel - 16;
            let source_start = source_pixel.checked_mul(3)?;
            let output_start = output_pixel.checked_mul(3)?;
            let block = reverse_rgb_block(source.get(source_start..source_start + 48)?)?;
            output[output_start..output_start + 48].copy_from_slice(&block);
            vector_blocks = vector_blocks.saturating_add(3);
        }
        let remainder_pixels = width - vector_pixels;
        if remainder_pixels != 0 {
            let remainder_len = remainder_pixels.checked_mul(3)?;
            let mut padded = [0u8; 48];
            let pad = 48usize.checked_sub(remainder_len)?;
            padded[pad..].copy_from_slice(source.get(..remainder_len)?);
            let block = reverse_rgb_block(&padded)?;
            let output_start = vector_pixels.checked_mul(3)?;
            output[output_start..output_start + remainder_len]
                .copy_from_slice(&block[..remainder_len]);
            vector_blocks = vector_blocks.saturating_add(3);
            scalar_tail = scalar_tail.saturating_add(remainder_pixels as u64);
        }
        return Some((vector_blocks, scalar_tail));
    }
    let pixels_per_vector = 16usize.checked_div(channels)?;
    let vector_pixels = width / pixels_per_vector * pixels_per_vector;
    for output_pixel in (0..vector_pixels).step_by(pixels_per_vector) {
        let source_pixel = width - output_pixel - pixels_per_vector;
        let source_start = source_pixel.checked_mul(channels)?;
        let output_start = output_pixel.checked_mul(channels)?;
        let block = reverse_pixel_block(source.get(source_start..source_start + 16)?, channels)?;
        output[output_start..output_start + 16].copy_from_slice(&block);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let remainder_pixels = width - vector_pixels;
    if remainder_pixels != 0 {
        let remainder_len = remainder_pixels.checked_mul(channels)?;
        let mut padded = [0u8; 16];
        let pad = 16usize.checked_sub(remainder_len)?;
        padded[pad..].copy_from_slice(source.get(..remainder_len)?);
        let block = reverse_pixel_block(&padded, channels)?;
        let output_start = vector_pixels.checked_mul(channels)?;
        output[output_start..output_start + remainder_len].copy_from_slice(&block[..remainder_len]);
        vector_blocks = vector_blocks.saturating_add(1);
        scalar_tail = scalar_tail.saturating_add(remainder_pixels as u64);
    }
    Some((vector_blocks, scalar_tail))
}

fn native_luma16_image_from_bytes(width: u32, height: u32, bytes: Vec<u8>) -> Option<DynamicImage> {
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(2))?;
    if bytes.len() != expected {
        return None;
    }
    let samples = bytes
        .chunks_exact(2)
        .map(|sample| u16::from_ne_bytes([sample[0], sample[1]]))
        .collect::<Vec<_>>();
    let image = ImageBuffer::<Luma<u16>, Vec<u16>>::from_raw(width, height, samples)?;
    Some(DynamicImage::ImageLuma16(image))
}

fn materialize_chops_operand(
    arc: &Arc<Image>,
    _mode: Option<&str>,
) -> Result<Arc<DynamicImage>, PilError> {
    // Native Chops and blend read stored samples, never expanded palette
    // colors. The shared handle keeps a secondary operand alive without a
    // full-image copy, including when it aliases an input pipeline branch.
    arc.materialized_shared()
}

// ── Native rank-family extrema ─────────────────────────────────────────────

const SIMD_UNIFORM_NEIGHBORHOOD_MAX_PIXELS: usize = 64 * 64;
const SIMD_ZERO_NEIGHBORHOOD_MAX_PIXELS: usize = 256 * 256;

/// Detect a bounded all-zero native-byte image before a neighborhood pass.
///
/// A zero image is preserved exactly by replicated-edge extrema and by
/// reduction in every admitted byte layout, including premultiplied alpha.
/// Keep this larger scan separate from the general uniform-image check: the
/// zero-byte comparison is cheap and covers the standard 256x256 benchmark,
/// while arbitrary uniform tuples still use the smaller guard below.
fn native_zero_byte_image(img: &DynamicImage, channels: usize) -> bool {
    let pixels = (img.width() as usize).saturating_mul(img.height() as usize);
    pixels != 0
        && pixels <= SIMD_ZERO_NEIGHBORHOOD_MAX_PIXELS
        && img.as_bytes().len() == pixels.saturating_mul(channels)
        && img.as_bytes().iter().all(|&byte| byte == 0)
}

/// Detect a constant native-byte image before allocating neighborhood buffers.
///
/// Replicated-edge blur and extrema filters preserve a constant channel tuple
/// exactly. Limit the scan to small images, where avoiding the kernel work is
/// cheaper than adding a full read pass to large non-uniform frames.
fn native_small_uniform_byte_image(img: &DynamicImage, channels: usize) -> bool {
    native_uniform_byte_image_up_to(img, channels, SIMD_UNIFORM_NEIGHBORHOOD_MAX_PIXELS)
}

/// Detect a constant native-byte image with an operation-specific bound.
///
/// Convolution can amortize a larger proof scan because a 5x5 pass evaluates
/// twenty-five taps for every interior pixel. Keep the larger bound private to
/// the identity proof so rank/blur paths retain their existing 64x64 guard.
fn native_uniform_byte_image_up_to(img: &DynamicImage, channels: usize, max_pixels: usize) -> bool {
    let pixels = (img.width() as usize).saturating_mul(img.height() as usize);
    if pixels == 0 || pixels > max_pixels {
        return false;
    }
    let raw = img.as_bytes();
    let mut pixel_chunks = raw.chunks_exact(channels);
    pixel_chunks.next().is_some_and(|first_pixel| {
        pixel_chunks.remainder().is_empty() && pixel_chunks.all(|pixel| pixel == first_pixel)
    })
}

const SIMD_BOX_BLUR_SCALE: u32 = 1 << 24;
const SIMD_BOX_BLUR_BIAS: u32 = 1 << 23;

#[inline(always)]
fn simd_luma_load_radius_one_block(source: &[u8], start: usize, edge: u8) -> u8x16 {
    if start >= source.len() {
        return u8x16::splat(edge);
    }
    let count = (source.len() - start).min(16);
    if count == 16 {
        return simd_blur_load_u8x16(source, start);
    }
    let mut bytes = [edge; 16];
    bytes[..count].copy_from_slice(&source[start..start + count]);
    u8x16::new(bytes)
}

/// Apply one native-L horizontal radius-one box pass with adjacent SIMD lanes.
///
/// Three overlapping unaligned vectors provide each block's left, center, and
/// right samples. The byte sum is widened before arithmetic, and the rounded
/// average uses an exact reciprocal multiply for every radius-one byte sum.
fn simd_luma_horizontal_radius_one_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
) {
    debug_assert_eq!(source.len(), width * height);
    debug_assert_eq!(destination.len(), source.len());
    if width == 0 || height == 0 {
        return;
    }

    let vector_blocks = width.div_ceil(16).saturating_mul(height) as u64;
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);

    let reciprocal = u32x16::splat(21_846);
    let one = u32x16::splat(1);

    for y in 0..height {
        let row_start = y * width;
        let row = &source[row_start..row_start + width];
        let output_row = &mut destination[row_start..row_start + width];
        let left_edge = row[0];
        let right_edge = row[width - 1];

        for x in (0..width).step_by(16) {
            let left = if x == 0 {
                let mut bytes = [left_edge; 16];
                let count = (width - 1).min(15);
                bytes[1..count + 1].copy_from_slice(&row[..count]);
                u8x16::new(bytes)
            } else {
                simd_luma_load_radius_one_block(row, x - 1, right_edge)
            };
            let center = simd_luma_load_radius_one_block(row, x, right_edge);
            let right = simd_luma_load_radius_one_block(row, x.saturating_add(1), right_edge);
            let left = u32x16::from(u16x16::from(left));
            let center = u32x16::from(u16x16::from(center));
            let right = u32x16::from(u16x16::from(right));
            let sum = left + center + right;
            let average = ((sum + one) * reciprocal) >> 16u32;
            let packed = simd_blur_pack_u32x16_to_bytes(average);
            let count = (width - x).min(16);
            output_row[x..x + count].copy_from_slice(&packed[..count]);
        }
    }
}

/// Apply one weighted radius-one horizontal GaussianBlur pass with SIMD lanes
/// spanning adjacent L pixels, including Pillow's fractional far-edge taps.
fn simd_luma_horizontal_radius_one_weighted_row(
    source: &[u8],
    destination: &mut [u8],
    whole_weight: u32,
    fractional_weight: u32,
) {
    let width = source.len();
    debug_assert_eq!(destination.len(), width);
    if width == 0 {
        return;
    }

    let right_edge = source[width - 1];
    let left_one_indices = u8x16::new([0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]);
    let left_two_indices = u8x16::new([0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]);
    let whole_weight = u32x16::splat(whole_weight);
    let fractional_weight = u32x16::splat(fractional_weight);
    let rounding_bias = u32x16::splat(SIMD_BOX_BLUR_BIAS);
    crate::compute::record_pipeline_operation_vector_blocks(width.div_ceil(16) as u64);

    for x in (0..width).step_by(16) {
        let center_bytes = simd_luma_load_radius_one_block(source, x, right_edge);
        let left_one_bytes = if x == 0 {
            center_bytes.swizzle_relaxed(left_one_indices)
        } else {
            simd_luma_load_radius_one_block(source, x - 1, right_edge)
        };
        let left_two_bytes = if x == 0 {
            center_bytes.swizzle_relaxed(left_two_indices)
        } else {
            simd_luma_load_radius_one_block(source, x - 2, right_edge)
        };
        let right_one_bytes = simd_luma_load_radius_one_block(source, x + 1, right_edge);
        let right_two_bytes = simd_luma_load_radius_one_block(source, x + 2, right_edge);

        let center = u32x16::from(u16x16::from(center_bytes));
        let neighbors = u32x16::from(u16x16::from(left_one_bytes))
            + center
            + u32x16::from(u16x16::from(right_one_bytes));
        let far_edges = u32x16::from(u16x16::from(left_two_bytes))
            + u32x16::from(u16x16::from(right_two_bytes));
        let weighted = neighbors * whole_weight + far_edges * fractional_weight + rounding_bias;
        let packed = simd_blur_pack_u32x16_to_bytes(weighted >> 24u32);
        let count = (width - x).min(16);
        destination[x..x + count].copy_from_slice(&packed[..count]);
    }
}

/// Run Pillow's three rounded L horizontal GaussianBlur passes one row at a
/// time, keeping the intermediate rows hot instead of writing two full-frame
/// images before the third pass.
fn simd_luma_horizontal_radius_one_three_passes_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    whole_weight: u32,
    fractional_weight: u32,
    scratch_a: &mut [u8],
    scratch_b: &mut [u8],
) {
    debug_assert_eq!(source.len(), width * height);
    debug_assert_eq!(destination.len(), source.len());
    debug_assert_eq!(scratch_a.len(), width);
    debug_assert_eq!(scratch_b.len(), width);
    if width == 0 || height == 0 {
        return;
    }

    for y in 0..height {
        let start = y * width;
        let end = start + width;
        simd_luma_horizontal_radius_one_weighted_row(
            &source[start..end],
            scratch_a,
            whole_weight,
            fractional_weight,
        );
        simd_luma_horizontal_radius_one_weighted_row(
            scratch_a,
            scratch_b,
            whole_weight,
            fractional_weight,
        );
        simd_luma_horizontal_radius_one_weighted_row(
            scratch_b,
            &mut destination[start..end],
            whole_weight,
            fractional_weight,
        );
    }
}

/// Apply one native-L vertical radius-one box pass with vector lanes spanning
/// adjacent columns. Keep Pillow's sliding window across output rows: the
/// previous implementation reloaded three window rows plus both fractional
/// edge rows for every output row. The rolling row state reads only the
/// newly entering rows while preserving the unsigned fixed-point recurrence.
fn simd_luma_vertical_radius_one_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    debug_assert_eq!(source.len(), width * height);
    debug_assert_eq!(destination.len(), source.len());
    let vector_blocks = width.div_ceil(16).saturating_mul(height) as u64;
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }

    if width == 0 || height == 0 {
        return;
    }

    let last_y = height - 1;
    let whole_weight = u32x16::splat(whole_weight);
    let fractional_weight = u32x16::splat(fractional_weight);
    let rounding_bias = u32x16::splat(SIMD_BOX_BLUR_BIAS);

    for x in (0..width).step_by(16) {
        let count = (width - x).min(16);
        let load_row = |y: usize| {
            let mut bytes = [0u8; 16];
            let start = y * width + x;
            bytes[..count].copy_from_slice(&source[start..start + count]);
            u8x16::new(bytes)
        };

        // For output y, Pillow's leaving, center, entering, and fractional
        // far-edge samples are respectively max(y-2, 0), max(y-1, 0),
        // min(y+1, last), and min(y+2, last). Three initial copies of row 0
        // reproduce blur_line's radius-one accumulator before its first step.
        // Retain those byte rows as bytes: keeping five widened u32x16 rows
        // live beside the accumulator creates excessive vector-register
        // pressure on ARM64.
        let mut leaving = load_row(0);
        let mut center = load_row(0);
        let mut current = load_row(0);
        let mut entering = load_row(1.min(last_y));
        let mut far_edge = load_row(2.min(last_y));
        let mut accumulator = u32x16::from(u16x16::from(leaving)) * u32x16::splat(3);

        for y in 0..height {
            let leaving_wide = u32x16::from(u16x16::from(leaving));
            accumulator = accumulator - leaving_wide + u32x16::from(u16x16::from(entering));
            let edge_sum = leaving_wide + u32x16::from(u16x16::from(far_edge));
            let weighted =
                accumulator * whole_weight + edge_sum * fractional_weight + rounding_bias;
            let packed = simd_blur_pack_u32x16_to_bytes(weighted >> 24u32);
            let output_start = y * width + x;
            destination[output_start..output_start + count].copy_from_slice(&packed[..count]);

            leaving = center;
            center = current;
            current = entering;
            entering = far_edge;
            far_edge = load_row(y.saturating_add(3).min(last_y));
        }
    }
}

/// Apply one RGBA vertical radius-one box pass directly in row-major storage.
///
/// GaussianBlur's three box passes use the same vertical sample on each of
/// the four native RGBA bytes. Process complete packed pixels in 16-byte
/// vectors and retain Pillow's fixed-point weights and byte rounding. This
/// avoids transposing the full RGBA frame just to make columns contiguous.
#[inline(always)]
fn simd_rgba_blur_scalar_pixel(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    x: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let left = x.saturating_sub(1);
    let right = x.saturating_add(1).min(width - 1);
    let far_left = x.saturating_sub(2);
    let far_right = x.saturating_add(2).min(width - 1);
    for channel in 0..4 {
        let sum = u32::from(source[left * 4 + channel])
            + u32::from(source[x * 4 + channel])
            + u32::from(source[right * 4 + channel]);
        let edge =
            u32::from(source[far_left * 4 + channel]) + u32::from(source[far_right * 4 + channel]);
        let weighted = sum
            .wrapping_mul(whole_weight)
            .wrapping_add(edge.wrapping_mul(fractional_weight))
            .wrapping_add(SIMD_BOX_BLUR_BIAS);
        destination[x * 4 + channel] = (weighted >> 24) as u8;
    }
}

#[inline(always)]
fn simd_blur_load_u8x16(bytes: &[u8], start: usize) -> u8x16 {
    bytemuck::pod_read_unaligned(&bytes[start..start + 16])
}

#[inline(always)]
fn simd_blur_pack_u32x16_to_bytes(values: u32x16) -> [u8; 16] {
    // The rounded blur lanes are in 0..=255, so signed saturating packs are
    // exact and avoid extracting and narrowing every lane in scalar code.
    let [low, high]: [i32x8; 2] = bytemuck::cast(values);
    let low = i16x8::from_i32x8_saturate(low);
    let high = i16x8::from_i32x8_saturate(high);
    bytemuck::cast(u8x16::narrow_i16x8(low, high))
}

/// Apply one RGBA horizontal radius-one box pass with direct vector loads.
///
/// Four neighboring pixels occupy one 16-byte vector. The interior therefore
/// uses aligned channel correspondence for all four RGBA bytes, while the
/// two-pixel borders and the final incomplete group use the exact clamped
/// scalar formula. This removes the per-pixel rolling-recurrence setup from
/// the three horizontal Gaussian box passes.
fn simd_rgba_horizontal_radius_one_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let row_stride = width
        .checked_mul(4)
        .expect("validated RGBA dimensions fit the native row stride");
    debug_assert!(width > 0);
    debug_assert!(height > 0);
    debug_assert_eq!(source.len(), row_stride * height);
    debug_assert_eq!(destination.len(), source.len());

    let interior_end = width.saturating_sub(2);
    let vector_blocks_per_row = interior_end.saturating_sub(2) / 4;
    let vector_blocks = vector_blocks_per_row.saturating_mul(height) as u64;
    let scalar_tail =
        (width.saturating_sub(vector_blocks_per_row * 4)).saturating_mul(height) as u64;
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }

    for y in 0..height {
        let row_start = y * row_stride;
        let input = &source[row_start..row_start + row_stride];
        let output = &mut destination[row_start..row_start + row_stride];
        let prefix_end = width.min(2);
        for x in 0..prefix_end {
            simd_rgba_blur_scalar_pixel(input, output, width, x, whole_weight, fractional_weight);
        }

        let mut x = 2usize;
        while x + 4 <= interior_end {
            let byte_start = x * 4;
            let load = |pixel: usize| {
                let start = pixel * 4;
                u32x16::from(u16x16::from(simd_blur_load_u8x16(input, start)))
            };
            let sum = load(x - 1) + load(x) + load(x + 1);
            let edge = if fractional_weight == 0 {
                u32x16::splat(0)
            } else {
                load(x - 2) + load(x + 2)
            };
            let weighted = sum * u32x16::splat(whole_weight)
                + edge * u32x16::splat(fractional_weight)
                + u32x16::splat(SIMD_BOX_BLUR_BIAS);
            let packed = simd_blur_pack_u32x16_to_bytes(weighted >> 24u32);
            output[byte_start..byte_start + 16].copy_from_slice(&packed);
            x += 4;
        }

        for x in x..interior_end {
            simd_rgba_blur_scalar_pixel(input, output, width, x, whole_weight, fractional_weight);
        }
        for x in interior_end.max(prefix_end)..width {
            simd_rgba_blur_scalar_pixel(input, output, width, x, whole_weight, fractional_weight);
        }
    }
}

#[inline(always)]
fn simd_rgb_blur_scalar_sample(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    byte_index: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let x = byte_index / 3;
    let channel = byte_index % 3;
    let left = x.saturating_sub(1);
    let right = x.saturating_add(1).min(width - 1);
    let far_left = x.saturating_sub(2);
    let far_right = x.saturating_add(2).min(width - 1);
    let sample = |pixel: usize| u32::from(source[pixel * 3 + channel]);
    let sum = sample(left) + sample(x) + sample(right);
    let edge = if fractional_weight == 0 {
        0
    } else {
        sample(far_left) + sample(far_right)
    };
    let weighted = sum
        .wrapping_mul(whole_weight)
        .wrapping_add(edge.wrapping_mul(fractional_weight))
        .wrapping_add(SIMD_BOX_BLUR_BIAS);
    destination[byte_index] = (weighted >> 24) as u8;
}

#[inline]
fn simd_rgb_horizontal_radius_one_row(
    input: &[u8],
    output: &mut [u8],
    width: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    simd_rgb_horizontal_radius_one_row_with_vectors(
        input,
        output,
        width,
        whole_weight,
        fractional_weight,
        u32x16::splat(whole_weight),
        u32x16::splat(fractional_weight),
        u32x16::splat(SIMD_BOX_BLUR_BIAS),
    );
}

#[inline]
fn simd_rgb_horizontal_radius_one_row_with_vectors(
    input: &[u8],
    output: &mut [u8],
    width: usize,
    whole_weight: u32,
    fractional_weight: u32,
    whole_weight_vector: u32x16,
    fractional_weight_vector: u32x16,
    rounding_bias: u32x16,
) {
    let row_stride = width
        .checked_mul(3)
        .expect("validated RGB dimensions fit the native row stride");
    debug_assert!(width > 0);
    debug_assert_eq!(input.len(), row_stride);
    debug_assert_eq!(output.len(), row_stride);

    let interior_end = width.saturating_sub(2) * 3;
    let prefix_end = width.min(2) * 3;
    for byte_index in 0..prefix_end {
        simd_rgb_blur_scalar_sample(
            input,
            output,
            width,
            byte_index,
            whole_weight,
            fractional_weight,
        );
    }

    let mut byte_index = prefix_end;
    if fractional_weight == 0 {
        while byte_index + 16 <= interior_end {
            // Three byte samples sum to at most 765, so add them as u16 lanes
            // and widen only the completed sum before Pillow's fixed-point
            // multiply. Radius one has no fractional edge contribution.
            let load = |offset: usize| u16x16::from(simd_blur_load_u8x16(input, offset));
            let sum = u32x16::from(load(byte_index - 3) + load(byte_index) + load(byte_index + 3));
            let weighted = sum * whole_weight_vector + rounding_bias;
            let packed = simd_blur_pack_u32x16_to_bytes(weighted >> 24u32);
            output[byte_index..byte_index + 16].copy_from_slice(&packed);
            byte_index += 16;
        }
    } else {
        while byte_index + 16 <= interior_end {
            let load = |offset: usize| u16x16::from(simd_blur_load_u8x16(input, offset));
            let sum = u32x16::from(load(byte_index - 3) + load(byte_index) + load(byte_index + 3));
            let edge = u32x16::from(load(byte_index - 6) + load(byte_index + 6));
            let weighted =
                sum * whole_weight_vector + edge * fractional_weight_vector + rounding_bias;
            let packed = simd_blur_pack_u32x16_to_bytes(weighted >> 24u32);
            output[byte_index..byte_index + 16].copy_from_slice(&packed);
            byte_index += 16;
        }
    }

    for byte_index in byte_index..interior_end {
        simd_rgb_blur_scalar_sample(
            input,
            output,
            width,
            byte_index,
            whole_weight,
            fractional_weight,
        );
    }
    for byte_index in interior_end.max(prefix_end)..row_stride {
        simd_rgb_blur_scalar_sample(
            input,
            output,
            width,
            byte_index,
            whole_weight,
            fractional_weight,
        );
    }
}

/// Apply one horizontal radius-one RGB pass without the generic scalar
/// rolling recurrence. Three-byte neighbors are 3/6 bytes apart, so vector
/// loads may cross pixel boundaries while every lane still retains its own
/// channel. Replicated borders and the short vector tail use the scalar path.
fn simd_rgb_horizontal_radius_one_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let row_stride = width
        .checked_mul(3)
        .expect("validated RGB dimensions fit the native row stride");
    debug_assert!(width > 0);
    debug_assert!(height > 0);
    debug_assert_eq!(source.len(), row_stride * height);
    debug_assert_eq!(destination.len(), source.len());

    let interior_end = width.saturating_sub(2) * 3;
    let prefix_end = width.min(2) * 3;
    let vector_blocks_per_row = interior_end.saturating_sub(prefix_end) / 16;
    let scalar_samples_per_row = row_stride.saturating_sub(vector_blocks_per_row * 16);
    crate::compute::record_pipeline_operation_vector_blocks(
        vector_blocks_per_row.saturating_mul(height) as u64,
    );
    crate::compute::record_pipeline_operation_scalar_tail(
        scalar_samples_per_row.saturating_mul(height) as u64,
    );

    for y in 0..height {
        let row_start = y * row_stride;
        simd_rgb_horizontal_radius_one_row(
            &source[row_start..row_start + row_stride],
            &mut destination[row_start..row_start + row_stride],
            width,
            whole_weight,
            fractional_weight,
        );
    }
}

fn simd_rgba_vertical_radius_one_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let row_stride = width
        .checked_mul(4)
        .expect("validated RGBA dimensions fit the native row stride");
    debug_assert!(width > 0);
    debug_assert!(height > 0);
    debug_assert_eq!(source.len(), row_stride * height);
    debug_assert_eq!(destination.len(), source.len());

    let last_y = height - 1;
    let vector_blocks = row_stride.div_ceil(16).saturating_mul(height) as u64;
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);

    for y in 0..height {
        let sum_top_base = y.saturating_sub(1) * row_stride;
        let sum_middle_base = y * row_stride;
        let sum_bottom_base = y.saturating_add(1).min(last_y) * row_stride;
        let edge_top_base = y.saturating_sub(2) * row_stride;
        let edge_bottom_base = y.saturating_add(2).min(last_y) * row_stride;
        let output_row = y * row_stride;

        for x in (0..row_stride).step_by(16) {
            let count = (row_stride - x).min(16);
            let load = |base: usize| {
                if count == 16 {
                    u32x16::from(u16x16::from(simd_blur_load_u8x16(source, base + x)))
                } else {
                    let mut padded = [0u8; 16];
                    padded[..count].copy_from_slice(&source[base + x..base + x + count]);
                    u32x16::from(u16x16::from(u8x16::new(padded)))
                }
            };
            let sum = load(sum_top_base) + load(sum_middle_base) + load(sum_bottom_base);
            let edge = if fractional_weight == 0 {
                u32x16::splat(0)
            } else {
                load(edge_top_base) + load(edge_bottom_base)
            };
            let weighted = sum * u32x16::splat(whole_weight)
                + edge * u32x16::splat(fractional_weight)
                + u32x16::splat(SIMD_BOX_BLUR_BIAS);
            let packed = simd_blur_pack_u32x16_to_bytes(weighted >> 24u32);
            destination[output_row + x..output_row + x + count].copy_from_slice(&packed[..count]);
        }
    }
}

/// Apply one RGB vertical radius-one pass in row-major three-byte storage.
///
/// A vertical blur treats every channel byte as an independent sample, so a
/// 16-byte vector may begin or end inside an RGB pixel. The same byte offset
/// is loaded from each neighboring row; no channel reordering or transpose is
/// needed. Tail vectors retain the exact fixed-point rounding.
fn simd_rgb_vertical_radius_one_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let row_stride = width
        .checked_mul(3)
        .expect("validated RGB dimensions fit the native row stride");
    debug_assert!(width > 0);
    debug_assert!(height > 0);
    debug_assert_eq!(source.len(), row_stride * height);
    debug_assert_eq!(destination.len(), source.len());

    let last_y = height - 1;
    let vector_blocks = row_stride.div_ceil(16).saturating_mul(height) as u64;
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);

    for y in 0..height {
        let sum_top_base = y.saturating_sub(1) * row_stride;
        let sum_middle_base = y * row_stride;
        let sum_bottom_base = y.saturating_add(1).min(last_y) * row_stride;
        let edge_top_base = y.saturating_sub(2) * row_stride;
        let edge_bottom_base = y.saturating_add(2).min(last_y) * row_stride;
        let output_row = y * row_stride;

        for x in (0..row_stride).step_by(16) {
            let count = (row_stride - x).min(16);
            let load = |base: usize| {
                if count == 16 {
                    u32x16::from(u16x16::from(simd_blur_load_u8x16(source, base + x)))
                } else {
                    let mut padded = [0u8; 16];
                    padded[..count].copy_from_slice(&source[base + x..base + x + count]);
                    u32x16::from(u16x16::from(u8x16::new(padded)))
                }
            };
            let sum = load(sum_top_base) + load(sum_middle_base) + load(sum_bottom_base);
            let edge = if fractional_weight == 0 {
                u32x16::splat(0)
            } else {
                load(edge_top_base) + load(edge_bottom_base)
            };
            let weighted = sum * u32x16::splat(whole_weight)
                + edge * u32x16::splat(fractional_weight)
                + u32x16::splat(SIMD_BOX_BLUR_BIAS);
            let packed = simd_blur_pack_u32x16_to_bytes(weighted >> 24u32);
            destination[output_row + x..output_row + x + count].copy_from_slice(&packed[..count]);
        }
    }
}

/// Stream horizontal RGB BoxBlur(1) rows through a three-row SIMD ring before
/// applying Pillow's rounded vertical pass. The horizontal bytes stay in
/// cache-sized row storage instead of a full-frame intermediate.
fn simd_rgb_box_blur_radius_one_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    horizontal_weight: u32,
    horizontal_fractional_weight: u32,
    vertical_weight: u32,
    vertical_fractional_weight: u32,
) {
    let row_stride = width
        .checked_mul(3)
        .expect("validated RGB dimensions fit the native row stride");
    debug_assert!(width > 0);
    debug_assert!(height > 0);
    debug_assert_eq!(source.len(), row_stride * height);
    debug_assert_eq!(destination.len(), source.len());
    debug_assert_eq!(horizontal_weight, SIMD_BOX_BLUR_SCALE / 3);
    debug_assert_eq!(horizontal_fractional_weight, 0);
    debug_assert_eq!(vertical_weight, SIMD_BOX_BLUR_SCALE / 3);
    debug_assert_eq!(vertical_fractional_weight, 0);

    let interior_end = width.saturating_sub(2) * 3;
    let prefix_end = width.min(2) * 3;
    let horizontal_blocks_per_row = interior_end.saturating_sub(prefix_end) / 16;
    let horizontal_scalar_bytes = row_stride.saturating_sub(horizontal_blocks_per_row * 16);
    crate::compute::record_pipeline_operation_vector_blocks(
        horizontal_blocks_per_row.saturating_mul(height) as u64,
    );
    crate::compute::record_pipeline_operation_scalar_tail(
        horizontal_scalar_bytes.saturating_mul(height) as u64,
    );

    if height == 1 {
        simd_rgb_horizontal_radius_one_row(
            source,
            destination,
            width,
            horizontal_weight,
            horizontal_fractional_weight,
        );
        return;
    }

    crate::compute::record_pipeline_operation_vector_blocks(
        row_stride.div_ceil(16).saturating_mul(height) as u64,
    );
    let ring_rows = height.min(3);
    let ring_dimensions = CheckedDims::new(
        u32::try_from(width).expect("validated RGB width fits u32"),
        u32::try_from(ring_rows).expect("three-row RGB ring height fits u32"),
        3,
    )
    .expect("validated RGB row ring dimensions fit image limits");
    let mut ring = ring_dimensions.alloc_buffer();
    let horizontal_weight_vector = u32x16::splat(horizontal_weight);
    let horizontal_fractional_weight_vector = u32x16::splat(horizontal_fractional_weight);
    let horizontal_rounding_bias = u32x16::splat(SIMD_BOX_BLUR_BIAS);
    simd_rgb_horizontal_radius_one_row_with_vectors(
        &source[..row_stride],
        &mut ring[..row_stride],
        width,
        horizontal_weight,
        horizontal_fractional_weight,
        horizontal_weight_vector,
        horizontal_fractional_weight_vector,
        horizontal_rounding_bias,
    );
    simd_rgb_horizontal_radius_one_row_with_vectors(
        &source[row_stride..row_stride * 2],
        &mut ring[row_stride..row_stride * 2],
        width,
        horizontal_weight,
        horizontal_fractional_weight,
        horizontal_weight_vector,
        horizontal_fractional_weight_vector,
        horizontal_rounding_bias,
    );

    let mut previous_slot = 0usize;
    let mut current_slot = 0usize;
    let mut next_slot = 1usize;
    let weight = u32x16::splat(vertical_weight);
    let bias = u32x16::splat(SIMD_BOX_BLUR_BIAS);
    for y in 0..height {
        let previous = previous_slot * row_stride;
        let current = current_slot * row_stride;
        let next = next_slot * row_stride;
        let output = y * row_stride;
        for x in (0..row_stride).step_by(16) {
            let count = (row_stride - x).min(16);
            let load = |start: usize| {
                if count == 16 {
                    u16x16::from(simd_blur_load_u8x16(&ring, start + x))
                } else {
                    let mut padded = [0u8; 16];
                    padded[..count].copy_from_slice(&ring[start + x..start + x + count]);
                    u16x16::from(u8x16::new(padded))
                }
            };
            let sum = u32x16::from(load(previous) + load(current) + load(next));
            let weighted = sum * weight + bias;
            let packed = simd_blur_pack_u32x16_to_bytes(weighted >> 24u32);
            destination[output + x..output + x + count].copy_from_slice(&packed[..count]);
        }

        if y + 1 < height {
            previous_slot = current_slot;
            current_slot = next_slot;
            if y + 2 < height {
                next_slot = (next_slot + 1) % ring_rows;
                let source_y = y + 2;
                let source_start = source_y * row_stride;
                let ring_start = next_slot * row_stride;
                simd_rgb_horizontal_radius_one_row_with_vectors(
                    &source[source_start..source_start + row_stride],
                    &mut ring[ring_start..ring_start + row_stride],
                    width,
                    horizontal_weight,
                    horizontal_fractional_weight,
                    horizontal_weight_vector,
                    horizontal_fractional_weight_vector,
                    horizontal_rounding_bias,
                );
            } else {
                next_slot = current_slot;
            }
        }
    }
}

#[inline(always)]
fn simd_la_blur_scalar_pixel(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    x: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let left = x.saturating_sub(1);
    let right = x.saturating_add(1).min(width - 1);
    let far_left = x.saturating_sub(2);
    let far_right = x.saturating_add(2).min(width - 1);
    let center = x * 2;
    for channel in 0..2 {
        let sum = u32::from(source[left * 2 + channel])
            + u32::from(source[center + channel])
            + u32::from(source[right * 2 + channel]);
        let edge =
            u32::from(source[far_left * 2 + channel]) + u32::from(source[far_right * 2 + channel]);
        let weighted = sum
            .wrapping_mul(whole_weight)
            .wrapping_add(edge.wrapping_mul(fractional_weight))
            .wrapping_add(SIMD_BOX_BLUR_BIAS);
        destination[center + channel] = (weighted >> 24) as u8;
    }
}

/// Apply one native-LA vertical radius-one box pass directly in row-major
/// storage. Eight interleaved LA pixels fit each vector; border rows and an
/// incomplete final vector retain Pillow's clamped-edge and byte rounding.
fn simd_la_vertical_radius_one_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let row_stride = width
        .checked_mul(2)
        .expect("validated LA dimensions fit the native row stride");
    debug_assert!(width > 0);
    debug_assert!(height > 0);
    debug_assert_eq!(source.len(), row_stride * height);
    debug_assert_eq!(destination.len(), source.len());

    let last_y = height - 1;
    let vector_blocks = row_stride.div_ceil(16).saturating_mul(height) as u64;
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);

    for y in 0..height {
        let sum_top_base = y.saturating_sub(1) * row_stride;
        let sum_middle_base = y * row_stride;
        let sum_bottom_base = y.saturating_add(1).min(last_y) * row_stride;
        let edge_top_base = y.saturating_sub(2) * row_stride;
        let edge_bottom_base = y.saturating_add(2).min(last_y) * row_stride;
        let output_row = y * row_stride;

        for x in (0..row_stride).step_by(16) {
            let count = (row_stride - x).min(16);
            let load = |base: usize| {
                if count == 16 {
                    u32x16::from(u16x16::from(simd_blur_load_u8x16(source, base + x)))
                } else {
                    let mut padded = [0u8; 16];
                    padded[..count].copy_from_slice(&source[base + x..base + x + count]);
                    u32x16::from(u16x16::from(u8x16::new(padded)))
                }
            };
            let sum = load(sum_top_base) + load(sum_middle_base) + load(sum_bottom_base);
            let edge = if fractional_weight == 0 {
                u32x16::splat(0)
            } else {
                load(edge_top_base) + load(edge_bottom_base)
            };
            let weighted = sum * u32x16::splat(whole_weight)
                + edge * u32x16::splat(fractional_weight)
                + u32x16::splat(SIMD_BOX_BLUR_BIAS);
            let packed = simd_blur_pack_u32x16_to_bytes(weighted >> 24u32);
            destination[output_row + x..output_row + x + count].copy_from_slice(&packed[..count]);
        }
    }
}

fn simd_native_blur_channels(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    let channels = native_byte_layout(img, mode)?;
    (1..=4).contains(&channels).then_some(channels)
}

const SIMD_RESIZE_LANES: usize = 8;
const SIMD_RESIZE_NEAREST_BYTES: usize = 16;
// Transpose dispatch is implemented in `adapters/transpose.rs`.

// ═══════════════════════════════════════════════════════════════════════
// Section G: New-buffer ops with PipelineOp dispatch
// ═══════════════════════════════════════════════════════════════════════

#[inline]
fn native_expand_fill_sample(fill: (u8, u8, u8, u8), channels: usize, index: usize) -> u8 {
    match channels {
        1 => fill.0,
        2 => {
            if index % 2 == 0 {
                fill.0
            } else {
                fill.3
            }
        }
        3 => match index % 3 {
            0 => fill.0,
            1 => fill.1,
            _ => fill.2,
        },
        4 => match index % 4 {
            0 => fill.0,
            1 => fill.1,
            2 => fill.2,
            _ => fill.3,
        },
        _ => 0,
    }
}

fn native_fill_row(row: &mut [u8], fill: (u8, u8, u8, u8), channels: usize) -> Option<(u64, u64)> {
    if !(1..=4).contains(&channels) {
        return None;
    }
    if channels == 1 {
        let pattern = u8x16::splat(fill.0).to_array();
        let vector_len = row.len() / pattern.len() * pattern.len();
        for chunk in row[..vector_len].chunks_exact_mut(pattern.len()) {
            chunk.copy_from_slice(&pattern);
        }
        let scalar_tail = row.len() - vector_len;
        row[vector_len..].copy_from_slice(&pattern[..scalar_tail]);
        let vector_blocks = (vector_len / pattern.len()) as u64 + u64::from(scalar_tail != 0);
        return Some((vector_blocks, scalar_tail as u64));
    }
    let vector_len = row.len() / 16 * 16;
    let mut vector_blocks = 0u64;
    for start in (0..vector_len).step_by(16) {
        let block = u8x16::new(std::array::from_fn(|lane| {
            native_expand_fill_sample(fill, channels, start + lane)
        }));
        row[start..start + 16].copy_from_slice(&block.to_array());
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let scalar_tail = row.len() - vector_len;
    if scalar_tail != 0 {
        let block = u8x16::new(std::array::from_fn(|lane| {
            native_expand_fill_sample(fill, channels, vector_len + lane)
        }));
        row[vector_len..].copy_from_slice(&block.to_array()[..scalar_tail]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    Some((vector_blocks, scalar_tail as u64))
}

/// Fill the ImageChops.constant result with one byte per output pixel.
///
/// Pillow always returns a new single-band `L` image for this operation; the
/// source samples and logical source mode do not affect the result. The
/// backend telemetry retains the vector-block accounting for this SIMD route,
/// while the allocator's repeated-byte fill avoids a redundant second pass.
pub fn simd_constant(
    img: &DynamicImage,
    op: &PipelineOp,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Constant { value } = op else {
        return Err(PilError::ValueError("expected Constant op".into()));
    };
    let width = img.width();
    let height = img.height();
    if width == 0 || height == 0 {
        return Err(simd_unsupported("Constant"));
    }
    let output_len = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| PilError::ValueError("SIMD constant output length overflow".into()))?;
    if output_len == 0 {
        return Err(simd_unsupported("Constant"));
    }
    // The destination is constant for the whole operation. Allocate it with
    // the final byte value so the bulk fill performs one pass; the previous
    // zero-fill followed by per-block copies traversed the full frame twice
    // without changing any output byte.
    let output = vec![*value; output_len];
    let vector_len = output_len / 16 * 16;
    let scalar_tail = output_len - vector_len;
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(
        (vector_len / 16 + usize::from(scalar_tail != 0)) as u64,
    );
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail as u64);
    crate::image_utils::raw_bytes_to_image(width, height, output, 1)
}

/// Duplicate an image through its native byte layout.
///
/// Unlike the packed scalar adapter, this preserves raw P/PA samples and the
/// fourth CMYK byte. The NEON path copies directly into uninitialized output
/// storage, so it does not first zero every byte before overwriting it.
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[allow(unsafe_code)]
fn simd_copy_native_bytes(source: &[u8]) -> (Vec<u8>, u64, u64) {
    use core::arch::aarch64 as neon;

    let vector_len = source.len() / 64 * 64;
    let scalar_tail = source.len() - vector_len;
    let mut output = Vec::<u8>::with_capacity(source.len());

    // SAFETY: `output` has capacity for the full source but its length is
    // still zero. Each NEON load/store covers a complete 16-byte block within
    // `vector_len`; the remaining bytes are copied with the exact bounded
    // scalar-tail length. The source is immutable and cannot overlap the new
    // allocation. After these writes, every byte in `0..source.len()` is
    // initialized before the vector length is published.
    unsafe {
        let source_ptr = source.as_ptr();
        let output_ptr = output.as_mut_ptr();
        for offset in (0..vector_len).step_by(64) {
            let block = neon::vld1q_u8_x4(source_ptr.add(offset));
            neon::vst1q_u8_x4(output_ptr.add(offset), block);
        }
        std::ptr::copy_nonoverlapping(
            source_ptr.add(vector_len),
            output_ptr.add(vector_len),
            scalar_tail,
        );
        output.set_len(source.len());
    }

    (output, (vector_len / 16) as u64, scalar_tail as u64)
}

/// Duplicate an image through its native byte layout.
///
/// The NEON copy handles full blocks without a zero-fill pass; other targets
/// retain the portable vector implementation and its bounded final block.
pub fn simd_duplicate(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if !matches!(op, PipelineOp::Duplicate) {
        return Err(PilError::ValueError("expected Duplicate op".into()));
    }
    let Some(channels) = native_copy_layout(img, mode) else {
        return Err(simd_unsupported("Duplicate"));
    };
    let (width, height) = img.dimensions();
    if width == 0 || height == 0 {
        return Err(simd_unsupported("Duplicate"));
    }
    let expected_len = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| PilError::ValueError("SIMD duplicate image length overflow".into()))?;
    let source = img.as_bytes();
    if source.len() != expected_len {
        return Err(simd_unsupported("Duplicate"));
    }
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    let (output, vector_blocks, scalar_tail) = simd_copy_native_bytes(source);
    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    let (output, vector_blocks, scalar_tail) = {
        let mut output = vec![0u8; expected_len];
        let (vector_blocks, scalar_tail) =
            copy_native_bytes(source, &mut output).ok_or_else(|| {
                PilError::InternalError("SIMD duplicate buffer shape mismatch".into())
            })?;
        (output, vector_blocks, scalar_tail)
    };
    let result = crate::image_utils::raw_bytes_to_image(width, height, output, channels)?;
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    crate::compute::record_pipeline_operation_path("neon-copy");
    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    crate::compute::record_pipeline_operation_path("native-copy");
    Ok(preserve_mode(img, result))
}

/// Copy an identity-like operation in the source's native byte layout.
///
/// Callers validate their complete public contract before this fast path is
/// selected. Once that validation proves that every output pixel maps to the
/// same source pixel, the data plane is simply a contiguous copy. Keep it as
/// a real SIMD operation so strict SIMD never enters a scalar implementation
/// by accident.
fn native_copy_image_bytes(
    img: &DynamicImage,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_copy_layout(img, mode) else {
        return Ok(None);
    };
    let (width, height) = img.dimensions();
    let Some(expected_len) = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return Ok(None);
    };
    let source = img.as_bytes();
    if source.len() != expected_len {
        return Ok(None);
    }

    let mut output = vec![0u8; source.len()];
    let (vector_blocks, scalar_tail) = copy_native_bytes(source, &mut output)
        .ok_or_else(|| PilError::InternalError("SIMD identity copy shape mismatch".into()))?;

    crate::compute::record_pipeline_operation_path("native-copy");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(width, height, output, channels).map(Some)
}

const SIMD_F64_LANES: usize = 8;

// ═══════════════════════════════════════════════════════════════════════
// Section H: Special/mutating ops
// ═══════════════════════════════════════════════════════════════════════

#[path = "adapters/convert.rs"]
mod convert;
#[cfg(feature = "parallel")]
use self::convert::SIMD_RGB_TO_RGBA_PARALLEL_PIXEL_THRESHOLD;
use self::convert::{
    NativeConvertLayout, native_convert_layout_block, native_convert_supported_for_image,
    native_convert_supported_for_shape, native_rgba_color_degenerate,
};
#[cfg(test)]
use self::convert::{
    native_convert_bytes, native_luma_to_rgb_bytes, native_luma_to_rgba_bytes,
    native_rgb_to_rgba_bytes,
};
pub(crate) use self::convert::{simd_convert, simd_convert_lab};

#[cfg(test)]
mod tests {
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    #[test]
    fn neon_duplicate_copy_writes_complete_blocks_and_bounded_tails() {
        for length in [0usize, 1, 15, 16, 63, 64, 65, 127, 128, 129] {
            let source = (0..length)
                .map(|index| (index.wrapping_mul(73).wrapping_add(19)) as u8)
                .collect::<Vec<_>>();
            let (output, vector_blocks, scalar_tail) = super::simd_copy_native_bytes(&source);
            let vector_len = length / 64 * 64;

            assert_eq!(output, source, "duplicate copy length {length}");
            assert_eq!(vector_blocks, (vector_len / 16) as u64);
            assert_eq!(scalar_tail, (length - vector_len) as u64);
        }
    }

    #[test]
    fn la_brightness_binary_shifts_match_float_truncation_and_preserve_alpha() {
        use crate::pipeline::PipelineOp;
        use crate::raster::{DynamicImage, GrayAlphaImage};

        for (width, height) in [(256u32, 256u32), (17, 3), (3, 17)] {
            let pixels = (width * height) as usize;
            let source = if pixels == 256 * 256 {
                (0..pixels)
                    .flat_map(|index| [(index >> 8) as u8, index as u8])
                    .collect::<Vec<_>>()
            } else {
                (0..pixels)
                    .flat_map(|index| {
                        [
                            (index.wrapping_mul(71).wrapping_add(13)) as u8,
                            (index.wrapping_mul(43).wrapping_add(29)) as u8,
                        ]
                    })
                    .collect::<Vec<_>>()
            };
            let image = DynamicImage::ImageLumaA8(
                GrayAlphaImage::from_raw(width, height, source.clone())
                    .expect("LA brightness source shape"),
            );

            for (factor, shift) in [(0.5, 1), (0.25, 2), (0.125, 3)] {
                let operation = PipelineOp::Brightness { factor };
                let expected = source
                    .chunks_exact(2)
                    .flat_map(|pixel| [(pixel[0] >> shift), pixel[1]])
                    .collect::<Vec<_>>();
                let direct = super::simd_brightness(&image, &operation, Some("LA"))
                    .expect("direct SIMD LA brightness");
                assert!(matches!(direct, DynamicImage::ImageLumaA8(_)));
                assert_eq!(
                    direct.as_bytes(),
                    expected,
                    "direct {factor} {width}x{height}"
                );

                let mut in_place = image.clone();
                assert!(
                    super::simd_execute_in_place(&mut in_place, &operation, Some("LA"))
                        .expect("in-place SIMD LA brightness dispatch")
                );
                assert_eq!(
                    in_place.as_bytes(),
                    expected,
                    "in-place {factor} {width}x{height}"
                );
                assert_eq!(image.as_bytes(), source, "source remains unchanged");
            }
        }
    }

    #[test]
    fn rgb_box_blur_radius_one_direct_passes_match_cpu_at_edges_and_tails() {
        use crate::compute::pool_cpu::ops::filter::execute_box_blur;
        use crate::pipeline::PipelineOp;
        use crate::raster::{DynamicImage, RgbImage};

        let operation = PipelineOp::BoxBlur { radius: 1 };
        for (width, height) in [
            (1u32, 1u32),
            (2, 3),
            (3, 2),
            (4, 5),
            (5, 9),
            (8, 3),
            (9, 4),
            (19, 7),
            (65, 47),
            (67, 53),
            (1024, 3),
        ] {
            let source: Vec<u8> = (0..width as usize * height as usize * 3)
                .map(|index| (index.wrapping_mul(73).wrapping_add(29) % 256) as u8)
                .collect();
            let image = DynamicImage::ImageRgb8(
                RgbImage::from_raw(width, height, source).expect("RGB blur source shape"),
            );
            let expected = execute_box_blur(&image, 1).expect("CPU RGB BoxBlur");
            let actual =
                super::simd_box_blur(&image, &operation, Some("RGB")).expect("SIMD RGB BoxBlur");
            assert_eq!(
                actual.as_bytes(),
                expected.as_bytes(),
                "size={width}x{height}"
            );
        }
    }

    #[test]
    fn luma_box_blur_radius_one_matches_cpu_at_edges_and_vector_tails() {
        use crate::compute::pool_cpu::ops::filter::execute_box_blur;
        use crate::pipeline::PipelineOp;
        use crate::raster::{DynamicImage, GrayImage};

        let operation = PipelineOp::BoxBlur { radius: 1 };
        for (width, height) in [
            (1u32, 1u32),
            (2, 3),
            (3, 2),
            (7, 5),
            (15, 2),
            (16, 3),
            (17, 5),
            (65, 47),
            (67, 53),
            (1024, 3),
        ] {
            let source: Vec<u8> = (0..width as usize * height as usize)
                .map(|index| (index.wrapping_mul(73).wrapping_add(29) % 256) as u8)
                .collect();
            let image = DynamicImage::ImageLuma8(
                GrayImage::from_raw(width, height, source).expect("L blur source shape"),
            );
            let expected = execute_box_blur(&image, 1).expect("CPU L BoxBlur");
            let actual =
                super::simd_box_blur(&image, &operation, Some("L")).expect("SIMD L BoxBlur");
            assert_eq!(
                actual.as_bytes(),
                expected.as_bytes(),
                "size={width}x{height}"
            );
        }
    }

    #[test]
    fn luma_radius_one_reciprocal_matches_pillow_fixed_point_for_all_byte_sums() {
        let whole_weight = super::SIMD_BOX_BLUR_SCALE / 3;
        for sum in 0..=3 * u32::from(u8::MAX) {
            let pillow = sum
                .wrapping_mul(whole_weight)
                .wrapping_add(super::SIMD_BOX_BLUR_BIAS)
                >> 24;
            let reciprocal = ((sum + 1) * 21_846) >> 16;
            assert_eq!(reciprocal, pillow, "byte sum={sum}");
        }
    }

    #[test]
    fn gaussian_blur_l_sliding_vertical_matches_cpu_edges_and_vector_tails() {
        use crate::compute::pool_cpu::ops::filter::execute_gaussian_blur;
        use crate::pipeline::PipelineOp;
        use crate::raster::{DynamicImage, GrayImage};

        for (width, height) in [(1u32, 1u32), (15, 2), (16, 3), (17, 5), (65, 47)] {
            let source = (0..width as usize * height as usize)
                .map(|index| (index.wrapping_mul(67).wrapping_add(index / 7 * 23 + 17) % 256) as u8)
                .collect();
            let image = DynamicImage::ImageLuma8(
                GrayImage::from_raw(width, height, source).expect("L blur source shape"),
            );
            let expected = execute_gaussian_blur(&image, 2.0).expect("CPU L GaussianBlur");
            let actual =
                super::gaussian_blur::simd_pil_gaussian_blur_l_radius_one(&image, 1.375, Some("L"))
                    .expect("SIMD L radius-one GaussianBlur");
            let operation = PipelineOp::GaussianBlur { sigma: 2.0 };
            let dispatched = super::simd_gaussian_blur(&image, &operation, Some("L"))
                .expect("SIMD L GaussianBlur adapter");

            assert_eq!(
                actual.as_bytes(),
                expected.as_bytes(),
                "size={width}x{height}"
            );
            assert_eq!(
                dispatched.as_bytes(),
                expected.as_bytes(),
                "adapter size={width}x{height}"
            );
        }
    }

    #[test]
    fn f_boxed_resize_sample_view_matches_cpu_reference() {
        use crate::ops::pil_resize::pil_resize_boxed;
        use crate::pipeline::ResampleFilter;
        use crate::raster::{DynamicImage, RgbaImage};

        let cases = [
            (
                "bicubic scale two",
                512,
                384,
                256,
                192,
                0.0,
                0.0,
                512.0,
                384.0,
                ResampleFilter::Bicubic,
            ),
            (
                "odd bicubic tail",
                511,
                383,
                255,
                191,
                0.0,
                0.0,
                511.0,
                383.0,
                ResampleFilter::Bicubic,
            ),
            (
                "fractional bicubic box",
                512,
                384,
                256,
                192,
                0.25,
                0.5,
                511.5,
                383.5,
                ResampleFilter::Bicubic,
            ),
            (
                "lanczos fallback",
                512,
                384,
                256,
                192,
                0.0,
                0.0,
                512.0,
                384.0,
                ResampleFilter::Lanczos,
            ),
        ];

        for (
            case,
            source_width,
            source_height,
            output_width,
            output_height,
            box_left,
            box_top,
            box_right,
            box_bottom,
            filter,
        ) in cases
        {
            let samples = (0..source_width as usize * source_height as usize)
                .map(|index| (((index * 41 % 1021) as i32 - 510) as f32) / 37.0)
                .collect::<Vec<_>>();
            let bytes = samples
                .iter()
                .flat_map(|sample| sample.to_le_bytes())
                .collect();
            let image = DynamicImage::ImageRgba8(
                RgbaImage::from_raw(source_width, source_height, bytes).expect("packed F samples"),
            );
            let expected = pil_resize_boxed(
                &image,
                output_width,
                output_height,
                box_left,
                box_top,
                box_right,
                box_bottom,
                filter,
                Some("F"),
            );
            let actual = super::simd_resize_f_boxed(
                &image,
                output_width,
                output_height,
                box_left,
                box_top,
                box_right,
                box_bottom,
                filter,
            )
            .expect("SIMD boxed F resize");

            let actual_bytes = actual.as_bytes();
            let expected_bytes = expected.as_bytes();
            let first_mismatch = actual_bytes
                .iter()
                .zip(expected_bytes)
                .position(|(actual, expected)| actual != expected);
            assert!(
                actual_bytes.len() == expected_bytes.len() && first_mismatch.is_none(),
                "{case}: first mismatching byte {first_mismatch:?}; actual={:?}, expected={:?}",
                first_mismatch.and_then(|index| actual_bytes.get(index..index + 4)),
                first_mismatch.and_then(|index| expected_bytes.get(index..index + 4)),
            );
        }
    }

    #[test]
    fn native_rgb_merge_borrows_auxiliary_bands_from_full_channel_list() {
        use crate::pipeline::ColorMode;
        use crate::raster::{DynamicImage, GrayImage};

        let red = vec![1, 2, 3, 4, 5];
        let green = vec![11, 22, 33, 44, 55];
        let blue = vec![101, 102, 103, 104, 105];
        let first = DynamicImage::ImageLuma8(
            GrayImage::from_raw(5, 1, red.clone()).expect("first L band shape"),
        );
        let bands = vec![
            crate::image::Image::from_luma_mask(5, 1, red).expect("first L band"),
            crate::image::Image::from_luma_mask(5, 1, green.clone()).expect("green L band"),
            crate::image::Image::from_luma_mask(5, 1, blue).expect("blue L band"),
        ];
        let shared_green = bands[1]
            .materialized_shared_for_ops()
            .expect("green L band materialization");
        let owners = super::native_merge_rgb_luma_owners(&first, &ColorMode::RGB, &bands, "RGB", 5)
            .expect("RGB merge owner lookup")
            .expect("all three channel bands should select the borrowed path");

        assert_eq!(owners.len(), 2, "the first band is already `img`");
        assert!(std::sync::Arc::ptr_eq(&owners[0], &shared_green));

        let (merged, vector_blocks, scalar_tail, path) =
            super::native_merge_bytes(&first, &ColorMode::RGB, &bands, Some("L"), "RGB")
                .expect("native SIMD merge")
                .expect("native RGB/L merge should be admitted");
        assert_eq!(
            merged.as_bytes(),
            [1, 11, 101, 2, 22, 102, 3, 33, 103, 4, 44, 104, 5, 55, 105]
        );
        assert_eq!(vector_blocks, 1);
        assert_eq!(scalar_tail, 3);
        assert_eq!(path, "wide-shuffle");
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn ssse3_native_rgb_merge_interleaves_exact_blocks_and_tails() {
        if !std::is_x86_feature_detected!("ssse3") {
            return;
        }

        for pixels in [0usize, 1, 3, 4, 15, 16, 17, 31, 32, 33, 1024] {
            let red = (0..pixels)
                .map(|index| ((index * 7 + 1) & 255) as u8)
                .collect::<Vec<_>>();
            let green = (0..pixels)
                .map(|index| ((index * 11 + 3) & 255) as u8)
                .collect::<Vec<_>>();
            let blue = (0..pixels)
                .map(|index| ((index * 13 + 5) & 255) as u8)
                .collect::<Vec<_>>();
            let expected = (0..pixels)
                .flat_map(|index| [red[index], green[index], blue[index]])
                .collect::<Vec<_>>();
            let bands = [&red[..], &green[..], &blue[..]];

            let (actual, vector_blocks, scalar_tail, path) =
                super::native_merge_interleave(pixels as u32, 1, pixels, 3, &bands)
                    .expect("native RGB merge should succeed");

            assert_eq!(actual.as_bytes(), expected, "pixel count {pixels}");
            if pixels >= 16 {
                assert_eq!(vector_blocks, (pixels / 16) as u64);
                assert_eq!(scalar_tail, ((pixels % 16) * 3) as u64);
                assert_eq!(path, "ssse3-interleave");
            } else {
                assert_eq!(vector_blocks, (pixels / 4) as u64);
                assert_eq!(scalar_tail, ((pixels % 4) * 3) as u64);
                assert_eq!(path, "wide-shuffle");
            }
        }
    }

    #[test]
    fn rgba_blur_vector_pack_preserves_byte_lanes() {
        let values = super::u32x16::new([
            0, 255, 1, 254, 127, 128, 17, 239, 33, 222, 64, 191, 85, 170, 100, 200,
        ]);
        assert_eq!(
            super::simd_blur_pack_u32x16_to_bytes(values),
            [
                0, 255, 1, 254, 127, 128, 17, 239, 33, 222, 64, 191, 85, 170, 100, 200
            ]
        );
    }

    #[test]
    fn rgba_radius_one_direct_passes_match_generic_simd() {
        for radius in [1.0f32, 1.25, 1.9] {
            let whole_weight = (super::SIMD_BOX_BLUR_SCALE as f32 / (radius * 2.0 + 1.0)) as u32;
            let fractional_weight = super::SIMD_BOX_BLUR_SCALE.wrapping_sub(3 * whole_weight) / 2;

            for (width, height) in [
                (1, 1),
                (2, 3),
                (3, 2),
                (4, 5),
                (5, 9),
                (8, 3),
                (9, 4),
                (19, 7),
                (256, 3),
            ] {
                let dimensions = crate::checked_dims::CheckedDims::new(width, height, 4)
                    .expect("test RGBA dimensions fit");
                let source: Vec<u8> = (0..dimensions.total_bytes())
                    .map(|index| (index.wrapping_mul(73).wrapping_add(29) % 256) as u8)
                    .collect();

                let mut horizontal_expected = dimensions.alloc_buffer();
                super::simd_blur_rows::<false>(
                    &source,
                    &mut horizontal_expected,
                    width as usize,
                    height as usize,
                    4,
                    1,
                    whole_weight,
                    fractional_weight,
                );
                let mut horizontal_actual = dimensions.alloc_buffer();
                super::simd_rgba_horizontal_radius_one_rows(
                    &source,
                    &mut horizontal_actual,
                    width as usize,
                    height as usize,
                    whole_weight,
                    fractional_weight,
                );
                assert_eq!(
                    horizontal_actual, horizontal_expected,
                    "horizontal dimensions={width}x{height} radius={radius}"
                );

                let mut transposed = dimensions.alloc_buffer();
                super::simd_transpose_interleaved_rows(
                    &source,
                    &mut transposed,
                    width as usize,
                    height as usize,
                    4,
                );
                let mut transposed_blurred = dimensions.alloc_buffer();
                super::simd_blur_rows::<false>(
                    &transposed,
                    &mut transposed_blurred,
                    height as usize,
                    width as usize,
                    4,
                    1,
                    whole_weight,
                    fractional_weight,
                );
                let mut vertical_expected = dimensions.alloc_buffer();
                super::simd_transpose_interleaved_rows(
                    &transposed_blurred,
                    &mut vertical_expected,
                    height as usize,
                    width as usize,
                    4,
                );

                let mut vertical_actual = dimensions.alloc_buffer();
                super::simd_rgba_vertical_radius_one_rows(
                    &source,
                    &mut vertical_actual,
                    width as usize,
                    height as usize,
                    whole_weight,
                    fractional_weight,
                );
                assert_eq!(
                    vertical_actual, vertical_expected,
                    "vertical dimensions={width}x{height} radius={radius}"
                );
            }
        }
    }

    #[test]
    fn rgb_radius_one_direct_vertical_passes_match_transposed_simd() {
        for radius in [1.0f32, 1.25, 1.9] {
            let whole_weight = (super::SIMD_BOX_BLUR_SCALE as f32 / (radius * 2.0 + 1.0)) as u32;
            let fractional_weight = super::SIMD_BOX_BLUR_SCALE.wrapping_sub(3 * whole_weight) / 2;

            for (width, height) in [
                (1, 1),
                (2, 3),
                (3, 2),
                (4, 5),
                (5, 9),
                (8, 3),
                (9, 4),
                (19, 7),
                (65, 47),
                (67, 53),
            ] {
                let dimensions = crate::checked_dims::CheckedDims::new(width, height, 3)
                    .expect("test RGB dimensions fit");
                let source: Vec<u8> = (0..dimensions.total_bytes())
                    .map(|index| (index.wrapping_mul(73).wrapping_add(29) % 256) as u8)
                    .collect();

                let mut expected = dimensions.alloc_buffer();
                super::simd_transpose_interleaved_rows(
                    &source,
                    &mut expected,
                    width as usize,
                    height as usize,
                    3,
                );
                let mut expected_scratch = dimensions.alloc_buffer();
                for _ in 0..3 {
                    super::simd_blur_rows::<false>(
                        &expected,
                        &mut expected_scratch,
                        height as usize,
                        width as usize,
                        3,
                        1,
                        whole_weight,
                        fractional_weight,
                    );
                    std::mem::swap(&mut expected, &mut expected_scratch);
                }
                super::simd_transpose_interleaved_rows(
                    &expected,
                    &mut expected_scratch,
                    height as usize,
                    width as usize,
                    3,
                );

                let mut actual = source;
                let mut actual_scratch = dimensions.alloc_buffer();
                for _ in 0..3 {
                    super::simd_rgb_vertical_radius_one_rows(
                        &actual,
                        &mut actual_scratch,
                        width as usize,
                        height as usize,
                        whole_weight,
                        fractional_weight,
                    );
                    std::mem::swap(&mut actual, &mut actual_scratch);
                }
                assert_eq!(
                    actual, expected_scratch,
                    "vertical dimensions={width}x{height} radius={radius}"
                );
            }
        }
    }

    #[test]
    fn rgb_radius_one_direct_horizontal_passes_match_generic_simd() {
        for radius in [1.0f32, 1.25, 1.9] {
            let whole_weight = (super::SIMD_BOX_BLUR_SCALE as f32 / (radius * 2.0 + 1.0)) as u32;
            let fractional_weight = super::SIMD_BOX_BLUR_SCALE.wrapping_sub(3 * whole_weight) / 2;

            for (width, height) in [
                (1, 1),
                (2, 3),
                (3, 2),
                (4, 5),
                (5, 9),
                (8, 3),
                (9, 4),
                (19, 7),
                (65, 47),
                (67, 53),
                (1024, 3),
            ] {
                let dimensions = crate::checked_dims::CheckedDims::new(width, height, 3)
                    .expect("test RGB dimensions fit");
                let source: Vec<u8> = (0..dimensions.total_bytes())
                    .map(|index| (index.wrapping_mul(73).wrapping_add(29) % 256) as u8)
                    .collect();
                let mut expected = dimensions.alloc_buffer();
                super::simd_blur_rows::<false>(
                    &source,
                    &mut expected,
                    width as usize,
                    height as usize,
                    3,
                    1,
                    whole_weight,
                    fractional_weight,
                );
                let mut actual = dimensions.alloc_buffer();
                super::simd_rgb_horizontal_radius_one_rows(
                    &source,
                    &mut actual,
                    width as usize,
                    height as usize,
                    whole_weight,
                    fractional_weight,
                );
                assert_eq!(
                    actual, expected,
                    "horizontal dimensions={width}x{height} radius={radius}"
                );
            }
        }
    }

    #[test]
    fn la_radius_one_direct_passes_match_generic_simd() {
        for radius in [1.0f32, 1.25, 1.9] {
            let whole_weight = (super::SIMD_BOX_BLUR_SCALE as f32 / (radius * 2.0 + 1.0)) as u32;
            let fractional_weight = super::SIMD_BOX_BLUR_SCALE.wrapping_sub(3 * whole_weight) / 2;

            for (width, height) in [
                (1, 1),
                (2, 3),
                (3, 2),
                (4, 5),
                (5, 9),
                (8, 3),
                (9, 4),
                (19, 7),
                (65, 47),
            ] {
                let dimensions = crate::checked_dims::CheckedDims::new(width, height, 2)
                    .expect("test LA dimensions fit");
                let source: Vec<u8> = (0..dimensions.total_bytes())
                    .map(|index| (index.wrapping_mul(59).wrapping_add(17) % 256) as u8)
                    .collect();

                let mut horizontal_expected = dimensions.alloc_buffer();
                super::simd_blur_rows::<false>(
                    &source,
                    &mut horizontal_expected,
                    width as usize,
                    height as usize,
                    2,
                    1,
                    whole_weight,
                    fractional_weight,
                );
                let transposed_dimensions = crate::checked_dims::CheckedDims::new(height, width, 2)
                    .expect("transposed LA dimensions fit");
                let mut transposed = transposed_dimensions.alloc_buffer();
                super::simd_transpose_interleaved_rows(
                    &source,
                    &mut transposed,
                    width as usize,
                    height as usize,
                    2,
                );
                let mut transposed_horizontal_actual = transposed_dimensions.alloc_buffer();
                super::simd_la_vertical_radius_one_rows(
                    &transposed,
                    &mut transposed_horizontal_actual,
                    height as usize,
                    width as usize,
                    whole_weight,
                    fractional_weight,
                );
                let mut horizontal_actual = dimensions.alloc_buffer();
                super::simd_transpose_interleaved_rows(
                    &transposed_horizontal_actual,
                    &mut horizontal_actual,
                    height as usize,
                    width as usize,
                    2,
                );
                assert_eq!(
                    horizontal_actual, horizontal_expected,
                    "horizontal dimensions={width}x{height} radius={radius}"
                );

                let mut transposed = dimensions.alloc_buffer();
                super::simd_transpose_interleaved_rows(
                    &source,
                    &mut transposed,
                    width as usize,
                    height as usize,
                    2,
                );
                let mut transposed_blurred = dimensions.alloc_buffer();
                super::simd_blur_rows::<false>(
                    &transposed,
                    &mut transposed_blurred,
                    height as usize,
                    width as usize,
                    2,
                    1,
                    whole_weight,
                    fractional_weight,
                );
                let mut vertical_expected = dimensions.alloc_buffer();
                super::simd_transpose_interleaved_rows(
                    &transposed_blurred,
                    &mut vertical_expected,
                    height as usize,
                    width as usize,
                    2,
                );

                let mut vertical_actual = dimensions.alloc_buffer();
                super::simd_la_vertical_radius_one_rows(
                    &source,
                    &mut vertical_actual,
                    width as usize,
                    height as usize,
                    whole_weight,
                    fractional_weight,
                );
                assert_eq!(
                    vertical_actual, vertical_expected,
                    "vertical dimensions={width}x{height} radius={radius}"
                );
            }
        }
    }

    #[test]
    fn hsv_masked_paste_uses_native_three_channel_simd_path() {
        use crate::pipeline::PipelineOp;
        use crate::raster::{DynamicImage, GrayImage, RgbImage};
        use std::sync::Arc;

        let destination = DynamicImage::ImageRgb8(
            RgbImage::from_raw(1, 1, vec![10, 80, 190]).expect("HSV destination bytes"),
        );
        let source = Arc::new(crate::Image::from_dynamic(
            DynamicImage::ImageRgb8(
                RgbImage::from_raw(1, 1, vec![200, 60, 10]).expect("HSV source bytes"),
            ),
            Some("HSV".to_owned()),
        ));
        let mask = Arc::new(crate::Image::from_dynamic(
            DynamicImage::ImageLuma8(GrayImage::from_raw(1, 1, vec![127]).expect("L mask bytes")),
            Some("L".to_owned()),
        ));
        let op = PipelineOp::Paste {
            source,
            x: 0,
            y: 0,
            w: 1,
            h: 1,
            mask: Some(mask),
            mask_alpha: false,
        };

        let result = super::simd_paste(&destination, &op, Some("HSV"))
            .expect("HSV masked Paste must use the native SIMD byte path");
        assert_eq!(result.as_bytes(), [105, 70, 100]);
    }

    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    #[test]
    fn simd_l_masked_paste_uses_neon_and_keeps_exact_rounding_and_tail() {
        use crate::pipeline::PipelineOp;
        use crate::raster::{DynamicImage, GrayImage};
        use std::sync::Arc;

        let (width, height) = (257usize, 2usize);
        let destination_bytes = (0..width * height)
            .map(|index| (index * 37 + 13) as u8)
            .collect::<Vec<_>>();
        let source_bytes = (0..width * height)
            .map(|index| (index * 91 + 7) as u8)
            .collect::<Vec<_>>();
        let mask_bytes = (0..width * height)
            .map(|index| index as u8)
            .collect::<Vec<_>>();
        let destination = DynamicImage::ImageLuma8(
            GrayImage::from_raw(width as u32, height as u32, destination_bytes.clone())
                .expect("L destination dimensions must be valid"),
        );
        let source = Arc::new(crate::Image::from_dynamic(
            DynamicImage::ImageLuma8(
                GrayImage::from_raw(width as u32, height as u32, source_bytes.clone())
                    .expect("L source dimensions must be valid"),
            ),
            Some("L".to_owned()),
        ));
        let mask = Arc::new(crate::Image::from_dynamic(
            DynamicImage::ImageLuma8(
                GrayImage::from_raw(width as u32, height as u32, mask_bytes.clone())
                    .expect("L mask dimensions must be valid"),
            ),
            Some("L".to_owned()),
        ));
        let op = PipelineOp::Paste {
            source,
            x: 0,
            y: 0,
            w: width as i32,
            h: height as i32,
            mask: Some(mask),
            mask_alpha: false,
        };

        let previous = crate::compute::Backend::set_pipeline_telemetry_enabled(true);
        crate::compute::begin_pipeline_operation_telemetry("Paste");
        let output = super::simd_paste(&destination, &op, Some("L"))
            .expect("SIMD L masked Paste must use its native NEON path");
        crate::compute::finish_pipeline_operation_telemetry();
        let telemetry = crate::compute::Backend::take_pipeline_operation_telemetry();
        crate::compute::Backend::set_pipeline_telemetry_enabled(previous);

        let expected = source_bytes
            .iter()
            .copied()
            .zip(destination_bytes.iter().copied())
            .zip(mask_bytes.iter().copied())
            .map(|((source, destination), mask)| {
                ((u32::from(source) * u32::from(mask)
                    + u32::from(destination) * (255 - u32::from(mask))
                    + 127)
                    / 255) as u8
            })
            .collect::<Vec<_>>();
        assert_eq!(output.as_bytes(), expected);

        let operation = telemetry
            .first()
            .expect("SIMD Paste must publish operation telemetry");
        assert_eq!(operation.path, "neon-blend");
        assert_eq!(operation.vector_block_count, 32);
        assert_eq!(operation.scalar_tail_count, 2);
    }

    #[test]
    fn simd_rgba_l_masked_paste_uses_native_mask_expansion_and_exact_clipped_tail() {
        use crate::pipeline::PipelineOp;
        use crate::raster::{DynamicImage, GrayImage, RgbaImage};
        use std::sync::Arc;

        let (source_width, source_height) = (27usize, 4usize);
        let (destination_width, destination_height) = (23usize, 3usize);
        let source_bytes = (0..source_width * source_height * 4)
            .map(|index| {
                u8::try_from(index % 256)
                    .expect("wrapped source sample fits in a byte")
                    .wrapping_mul(37)
                    .wrapping_add(13)
            })
            .collect::<Vec<_>>();
        let destination_bytes = (0..destination_width * destination_height * 4)
            .map(|index| {
                u8::try_from(index % 256)
                    .expect("wrapped destination sample fits in a byte")
                    .wrapping_mul(71)
                    .wrapping_add(29)
            })
            .collect::<Vec<_>>();
        let mask_pattern = [0u8, 1, 127, 128, 254, 255];
        let mask_bytes = (0..source_width * source_height)
            .map(|index| mask_pattern[index % mask_pattern.len()])
            .collect::<Vec<_>>();
        let destination = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(
                destination_width as u32,
                destination_height as u32,
                destination_bytes.clone(),
            )
            .expect("RGBA destination dimensions must be valid"),
        );
        let source = Arc::new(crate::Image::from_dynamic(
            DynamicImage::ImageRgba8(
                RgbaImage::from_raw(
                    source_width as u32,
                    source_height as u32,
                    source_bytes.clone(),
                )
                .expect("RGBA source dimensions must be valid"),
            ),
            Some("RGBA".to_owned()),
        ));
        let mask = Arc::new(crate::Image::from_dynamic(
            DynamicImage::ImageLuma8(
                GrayImage::from_raw(
                    source_width as u32,
                    source_height as u32,
                    mask_bytes.clone(),
                )
                .expect("L mask dimensions must be valid"),
            ),
            Some("L".to_owned()),
        ));
        let op = PipelineOp::Paste {
            source,
            x: -2,
            y: 1,
            w: source_width as i32,
            h: source_height as i32,
            mask: Some(mask),
            mask_alpha: false,
        };

        let previous = crate::compute::Backend::set_pipeline_telemetry_enabled(true);
        crate::compute::begin_pipeline_operation_telemetry("Paste");
        let output = super::simd_paste(&destination, &op, Some("RGBA"))
            .expect("SIMD RGBA/L Paste must use its native mask expansion path");
        crate::compute::finish_pipeline_operation_telemetry();
        let telemetry = crate::compute::Backend::take_pipeline_operation_telemetry();
        crate::compute::Backend::set_pipeline_telemetry_enabled(previous);

        let mut expected = destination_bytes;
        // x=-2 shifts source and L mask by two pixels; y=1 leaves only two
        // complete source rows visible in the destination.
        for destination_y in 1..destination_height {
            let source_y = destination_y - 1;
            for destination_x in 0..destination_width {
                let source_x = destination_x + 2;
                let mask_value = u32::from(mask_bytes[source_y * source_width + source_x]);
                for channel in 0..4 {
                    let source_index = (source_y * source_width + source_x) * 4 + channel;
                    let destination_index =
                        (destination_y * destination_width + destination_x) * 4 + channel;
                    let source_value = u32::from(source_bytes[source_index]);
                    let destination_value = u32::from(expected[destination_index]);
                    expected[destination_index] =
                        ((source_value * mask_value + destination_value * (255 - mask_value) + 127)
                            / 255) as u8;
                }
            }
        }
        assert_eq!(
            output.as_bytes(),
            expected,
            "RGBA alpha is blended as stored data"
        );

        let operation = telemetry
            .first()
            .expect("SIMD Paste must publish operation telemetry");
        let expected_path = if cfg!(all(target_arch = "aarch64", target_feature = "neon")) {
            "neon-rgba-l-blend"
        } else {
            "rgba-l-mask-vector"
        };
        assert_eq!(operation.path, expected_path);
        assert_eq!(operation.vector_block_count, 14);
        assert_eq!(operation.scalar_tail_count, 8);
    }

    #[test]
    fn native_pa_masked_paste_plan_uses_packed_pa_bytes_and_clipped_mask() {
        let source_bytes = vec![17, 201, 83, 149, 26, 222, 91, 163, 240, 7];
        let destination_bytes = vec![31, 42, 53, 64, 75, 86, 97, 108];
        let mask_bytes = vec![0, 1, 127, 128, 254];
        let source_image = crate::raster::DynamicImage::ImageLumaA8(
            crate::raster::GrayAlphaImage::from_raw(5, 1, source_bytes.clone())
                .expect("PA source shape"),
        );
        let destination = crate::raster::DynamicImage::ImageLumaA8(
            crate::raster::GrayAlphaImage::from_raw(4, 1, destination_bytes.clone())
                .expect("PA destination shape"),
        );
        let source = std::sync::Arc::new(crate::Image::from_dynamic(
            source_image,
            Some("PA".to_owned()),
        ));
        let mask = std::sync::Arc::new(crate::Image::from_dynamic(
            crate::raster::DynamicImage::ImageLuma8(
                crate::raster::GrayImage::from_raw(5, 1, mask_bytes.clone()).expect("L mask shape"),
            ),
            Some("L".to_owned()),
        ));

        let plan = super::native_paste_plan_for_image(
            &destination,
            &source,
            -1,
            0,
            5,
            1,
            Some(&mask),
            false,
            Some("PA"),
        )
        .expect("PA/L plan is admitted directly");
        assert_eq!(plan.layout.mode, "PA");
        assert_eq!(plan.layout.channels, 2);
        assert_eq!((plan.region.source_left, plan.region.width), (1, 4));

        let source_pixels = source.materialized_shared().expect("native PA bytes");
        let mask_pixels = mask.materialized_shared().expect("native L mask bytes");
        let mut actual = destination.as_bytes().to_vec();
        assert!(super::native_paste_apply(
            &mut actual,
            source_pixels.as_bytes(),
            Some(mask_pixels.as_bytes()),
            4,
            1,
            plan,
            true,
        ));

        let mut expected = destination_bytes;
        for destination_pixel in 0..4 {
            let source_pixel = destination_pixel + 1;
            let weight = u32::from(mask_bytes[source_pixel]);
            for channel in 0..2 {
                let byte = destination_pixel * 2 + channel;
                let source_value = u32::from(source_bytes[source_pixel * 2 + channel]);
                let destination_value = u32::from(expected[byte]);
                expected[byte] =
                    ((source_value * weight + destination_value * (255 - weight) + 127) / 255)
                        as u8;
            }
        }
        assert_eq!(actual, expected);
    }

    #[test]
    fn native_l_masked_paste_rows_keep_positive_clipped_short_tail_exact() {
        let destination = vec![11, 22, 33, 44, 55, 66];
        let source = (101..116).collect::<Vec<u8>>();
        let mask = vec![
            0, 128, 255, 254, 1, 127, 128, 254, 255, 0, 1, 127, 128, 254, 255,
        ];
        let region = super::native_paste_region(3, 2, 5, 3, 1, 1)
            .expect("positive offset should leave a clipped two-pixel row");
        assert_eq!((region.width, region.height), (2, 1));
        let plan = super::NativePastePlan {
            layout: super::NativePasteLayout {
                channels: 1,
                mode: "L",
            },
            mask: Some(super::NativePasteMaskLayout {
                channels: 1,
                value_index: 0,
                premultiplied: false,
                binary: false,
            }),
            region,
            source_width: 5,
            source_height: 3,
            luma16_mask_byte_shift: 0,
        };

        let mut actual = destination.clone();
        assert!(super::native_paste_apply(
            &mut actual,
            &source,
            Some(&mask),
            3,
            2,
            plan,
            true,
        ));

        let mut expected = destination;
        for index in 0..2 {
            let source_value = u32::from(source[index]);
            let destination_value = u32::from(expected[4 + index]);
            let mask_value = u32::from(mask[index]);
            expected[4 + index] =
                ((source_value * mask_value + destination_value * (255 - mask_value) + 127) / 255)
                    as u8;
        }
        assert_eq!(actual.as_slice(), expected.as_slice());
    }

    #[test]
    fn native_la_masked_paste_row_duplicates_l_mask_per_pixel_across_vector_tails() {
        for width in 1..=256 {
            let source = (0..width * 2)
                .map(|index| (index * 37 + 11) as u8)
                .collect::<Vec<_>>();
            let destination = (0..width * 2)
                .map(|index| (index * 53 + 7) as u8)
                .collect::<Vec<_>>();
            let mask = (0..width).map(|index| index as u8).collect::<Vec<_>>();
            let mut expected = destination.clone();
            for byte_index in 0..expected.len() {
                let mask_value = u32::from(mask[byte_index / 2]);
                let source_value = u32::from(source[byte_index]);
                let destination_value = u32::from(destination[byte_index]);
                expected[byte_index] =
                    ((source_value * mask_value + destination_value * (255 - mask_value) + 127)
                        / 255) as u8;
            }

            for allow_short_masked_tail in [false, true] {
                let mut actual = destination.clone();
                let used_neon = super::native_paste_la_masked_row(
                    &source,
                    &mut actual,
                    &mask,
                    allow_short_masked_tail,
                );
                assert_eq!(
                    actual, expected,
                    "width {width}, padded tail {allow_short_masked_tail}"
                );
                assert_eq!(
                    used_neon,
                    cfg!(all(target_arch = "aarch64", target_feature = "neon")) && width >= 8,
                    "width {width}, padded tail {allow_short_masked_tail} NEON route"
                );
            }
        }
    }

    #[test]
    fn native_rgb_l_masked_paste_row_expands_mask_across_vector_tails() {
        let masks = [0u8, 1, 127, 128, 254, 255];
        for width in 1..=24 {
            let source = (0..width * 3)
                .map(|index| (index * 37 + 11) as u8)
                .collect::<Vec<_>>();
            let destination = (0..width * 3)
                .map(|index| (index * 53 + 7) as u8)
                .collect::<Vec<_>>();
            let mask = (0..width)
                .map(|index| masks[index % masks.len()])
                .collect::<Vec<_>>();
            let mut expected = destination.clone();
            for (pixel, mask_value) in mask.iter().copied().enumerate() {
                for channel in 0..3 {
                    let byte = pixel * 3 + channel;
                    let source_value = u32::from(source[byte]);
                    let destination_value = u32::from(destination[byte]);
                    let mask_value = u32::from(mask_value);
                    expected[byte] =
                        ((source_value * mask_value + destination_value * (255 - mask_value) + 127)
                            / 255) as u8;
                }
            }

            for allow_short_masked_tail in [false, true] {
                let mut actual = destination.clone();
                super::native_paste_rgb_l_masked_row(
                    &source,
                    &mut actual,
                    &mask,
                    allow_short_masked_tail,
                );
                assert_eq!(
                    actual, expected,
                    "width {width}, padded tail {allow_short_masked_tail}"
                );
            }
        }
    }

    #[test]
    fn native_cmyk_l_masked_paste_row_expands_mask_across_vector_tails() {
        let masks = [0u8, 1, 127, 128, 254, 255];
        for width in 1..=131 {
            let source = (0..width * 4)
                .map(|index| (index * 37 + 11) as u8)
                .collect::<Vec<_>>();
            let destination = (0..width * 4)
                .map(|index| (index * 53 + 7) as u8)
                .collect::<Vec<_>>();
            let mask = (0..width)
                .map(|index| masks[index % masks.len()])
                .collect::<Vec<_>>();
            let mut expected = destination.clone();
            for (pixel, mask_value) in mask.iter().copied().enumerate() {
                for channel in 0..4 {
                    let byte = pixel * 4 + channel;
                    let source_value = u32::from(source[byte]);
                    let destination_value = u32::from(destination[byte]);
                    let mask_value = u32::from(mask_value);
                    expected[byte] =
                        ((source_value * mask_value + destination_value * (255 - mask_value) + 127)
                            / 255) as u8;
                }
            }

            for allow_short_masked_tail in [false, true] {
                let mut actual = destination.clone();
                let (_, _, used_neon) = super::native_paste_cmyk_l_masked_row(
                    &source,
                    &mut actual,
                    &mask,
                    allow_short_masked_tail,
                );
                assert_eq!(
                    used_neon,
                    cfg!(all(target_arch = "aarch64", target_feature = "neon")) && width >= 16,
                    "width {width}, neon dispatch"
                );
                assert_eq!(
                    actual, expected,
                    "width {width}, padded tail {allow_short_masked_tail}"
                );
            }
        }
    }

    #[test]
    fn native_rgb_to_rgba_paste_returns_unchanged_destination_for_empty_or_disjoint_regions() {
        use crate::raster::{DynamicImage, RgbImage, RgbaImage};

        let destination_bytes = vec![17, 31, 47, 83, 19, 37, 53, 97];
        let destination = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(2, 1, destination_bytes.clone())
                .expect("test destination shape must be valid"),
        );
        let cases = [
            (RgbImage::new(0, 1), 0, 0, 0, 1),
            (RgbImage::new(1, 0), 0, 0, 1, 0),
            (RgbImage::new(1, 1), i32::MAX, 0, 1, 1),
        ];
        for (source, x, y, width, height) in cases {
            let source = DynamicImage::ImageRgb8(source);
            let actual = super::simd_paste_rgb_to_rgba(&destination, &source, x, y, width, height)
                .expect("empty and disjoint SIMD paste must preserve the receiver");
            assert_eq!(actual.as_bytes(), destination_bytes);
        }

        let empty_destination = DynamicImage::ImageRgba8(RgbaImage::new(0, 1));
        let source = DynamicImage::ImageRgb8(
            RgbImage::from_raw(1, 1, vec![101, 127, 149]).expect("test source shape must be valid"),
        );
        let actual = super::simd_paste_rgb_to_rgba(&empty_destination, &source, 0, 0, 1, 1)
            .expect("zero-width destination paste must preserve the receiver");
        assert!(actual.as_bytes().is_empty());
    }

    #[test]
    fn pad_rgbx_preserves_x_samples_and_fills_new_pixels_from_fill_x() {
        let image = DynamicImage::ImageRgba8(
            crate::raster::RgbaImage::from_raw(2, 1, vec![23, 47, 89, 131, 29, 31, 37, 41])
                .expect("RGBX source shape must be valid"),
        );
        assert_eq!(
            super::resize::native_pad_channels_for_image(&image, Some("RGBX")),
            Some(4)
        );
        let mismatched = DynamicImage::ImageRgb8(
            crate::raster::RgbImage::from_raw(2, 1, vec![23, 47, 89, 29, 31, 37])
                .expect("mismatched RGBX storage shape must be valid"),
        );
        assert_eq!(
            super::resize::native_pad_channels_for_image(&mismatched, Some("RGBX")),
            None
        );

        let fill = (7, 11, 13, 17);
        let operation = crate::pipeline::PipelineOp::Pad {
            w: 2,
            h: 4,
            filter: crate::pipeline::ResampleFilter::Nearest,
            color: Some(fill),
            centering: (0.0, 0.5),
        };
        let result =
            super::simd_pad(&image, &operation, Some("RGBX")).expect("SIMD RGBX pad must succeed");

        let fill_row = [7, 11, 13, 17, 7, 11, 13, 17];
        let mut expected = Vec::with_capacity(2 * 4 * 4);
        expected.extend_from_slice(&fill_row);
        expected.extend_from_slice(&fill_row);
        expected.extend_from_slice(&[23, 47, 89, 131, 29, 31, 37, 41]);
        expected.extend_from_slice(&fill_row);
        assert_eq!(result.as_bytes(), expected);
    }

    #[test]
    fn pad_rgba_appends_native_source_rows_between_alpha_fill_rows() {
        use crate::raster::{DynamicImage, RgbaImage};

        let source = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(2, 1, vec![23, 47, 89, 131, 29, 31, 37, 41])
                .expect("RGBA source shape must be valid"),
        );
        let fill = (7, 11, 13, 17);
        let operation = crate::pipeline::PipelineOp::Pad {
            w: 2,
            h: 4,
            filter: crate::pipeline::ResampleFilter::Nearest,
            color: Some(fill),
            centering: (0.0, 0.5),
        };
        let result = super::simd_pad(&source, &operation, Some("RGBA")).expect("SIMD RGBA pad");

        let fill_row = [7, 11, 13, 17, 7, 11, 13, 17];
        let mut expected = Vec::with_capacity(2 * 4 * 4);
        expected.extend_from_slice(&fill_row);
        expected.extend_from_slice(&fill_row);
        expected.extend_from_slice(&[23, 47, 89, 131, 29, 31, 37, 41]);
        expected.extend_from_slice(&fill_row);
        assert_eq!(result.as_bytes(), expected);
    }

    #[test]
    fn pad_rgb_appends_native_source_rows_between_fill_rows() {
        use crate::raster::{DynamicImage, RgbImage};

        let source = DynamicImage::ImageRgb8(
            RgbImage::from_raw(2, 1, vec![23, 47, 89, 29, 31, 37])
                .expect("RGB source shape must be valid"),
        );
        let fill = (7, 11, 13, 17);
        let operation = crate::pipeline::PipelineOp::Pad {
            w: 2,
            h: 4,
            filter: crate::pipeline::ResampleFilter::Nearest,
            color: Some(fill),
            centering: (0.0, 0.5),
        };
        let result = super::simd_pad(&source, &operation, Some("RGB")).expect("SIMD RGB pad");

        let fill_row = [7, 11, 13, 7, 11, 13];
        let mut expected = Vec::with_capacity(2 * 4 * 3);
        expected.extend_from_slice(&fill_row);
        expected.extend_from_slice(&fill_row);
        expected.extend_from_slice(&[23, 47, 89, 29, 31, 37]);
        expected.extend_from_slice(&fill_row);
        assert_eq!(result.as_bytes(), expected);
    }

    #[test]
    fn expand_rgbx_appends_rows_without_changing_x_samples() {
        let source = DynamicImage::ImageRgba8(
            crate::raster::RgbaImage::from_raw(2, 1, vec![23, 47, 89, 131, 29, 31, 37, 41])
                .expect("RGBX source shape must be valid"),
        );
        let fill = (7, 11, 13, 17);
        let operation = PipelineOp::Expand { border: 1, fill };
        let actual = super::simd_expand(&source, &operation, Some("RGBX"))
            .expect("SIMD RGBX expansion must succeed");
        let expected = crate::compute::registry::execute_cpu(&operation, &source, Some("RGBX"))
            .expect("CPU RGBX expansion must succeed");

        assert_eq!(actual.as_bytes(), expected.as_bytes());
        let fill_pixel = [7, 11, 13, 17];
        let mut expected_bytes = Vec::with_capacity(4 * 3 * 4);
        expected_bytes.extend_from_slice(&fill_pixel.repeat(4));
        expected_bytes.extend_from_slice(&fill_pixel);
        expected_bytes.extend_from_slice(&[23, 47, 89, 131, 29, 31, 37, 41]);
        expected_bytes.extend_from_slice(&fill_pixel);
        expected_bytes.extend_from_slice(&fill_pixel.repeat(4));
        assert_eq!(actual.as_bytes(), expected_bytes);
    }

    #[test]
    fn expand_single_byte_modes_append_vector_filled_rows_without_zeroing() {
        use crate::pipeline::PipelineOp;
        use crate::raster::{DynamicImage, GrayImage};

        for (width, height, border) in [(1u32, 1u32, 1u32), (17, 3, 2), (33, 32, 1), (16, 16, 0)] {
            let pixels = (0..width * height)
                .map(|index| (index.wrapping_mul(67).wrapping_add(19)) as u8)
                .collect::<Vec<_>>();
            let source = DynamicImage::ImageLuma8(
                GrayImage::from_raw(width, height, pixels).expect("L source shape must be valid"),
            );
            let operation = PipelineOp::Expand {
                border,
                fill: (7, 11, 13, 17),
            };
            for mode in [Some("L"), Some("P")] {
                let actual = super::simd_expand(&source, &operation, mode)
                    .expect("SIMD single-byte expansion must succeed");
                let expected = crate::compute::registry::execute_cpu(&operation, &source, mode)
                    .expect("CPU single-byte expansion must succeed");

                assert_eq!(
                    (actual.width(), actual.height()),
                    (expected.width(), expected.height())
                );
                assert_eq!(actual.as_bytes(), expected.as_bytes());
            }
        }
    }

    #[test]
    fn native_rgb_sharpness_vector_stream_matches_cpu_across_tails() {
        for (width, height) in [(3, 3), (4, 5), (5, 7), (8, 4), (9, 9), (17, 6)] {
            let pixels = (width * height) as usize;
            let bytes = (0..pixels * 3)
                .map(|index| (index.wrapping_mul(61).wrapping_add(index / 7 + 23)) as u8)
                .collect::<Vec<_>>();
            let source = DynamicImage::ImageRgb8(
                crate::raster::RgbImage::from_raw(width, height, bytes).unwrap(),
            );
            for factor in [0.0, 0.5, 1.5, 2.0, f64::MAX] {
                let op = PipelineOp::Sharpness { factor };
                let expected =
                    crate::compute::registry::execute_cpu(&op, &source, Some("RGB")).unwrap();
                let actual = super::simd_sharpness(&source, &op, Some("RGB")).unwrap();
                assert_eq!(
                    actual.color(),
                    expected.color(),
                    "{width}x{height}, {factor}"
                );
                assert_eq!(
                    actual.as_bytes(),
                    expected.as_bytes(),
                    "{width}x{height}, {factor}"
                );
            }
        }
    }

    #[test]
    fn native_sharpness_integer_channels_match_cpu_for_all_byte_modes() {
        for (mode, channels) in [
            ("L", 1usize),
            ("LA", 2),
            ("RGB", 3),
            ("RGBA", 4),
            ("RGBX", 4),
            ("CMYK", 4),
        ] {
            for (width, height) in [
                (3, 3),
                (4, 5),
                (5, 7),
                (9, 4),
                (10, 9),
                (17, 6),
                (18, 5),
                (25, 8),
                (33, 4),
            ] {
                let pixels = width as usize * height as usize;
                let bytes = (0..pixels * channels)
                    .map(|index| (index.wrapping_mul(61).wrapping_add(index / 7 + 23)) as u8)
                    .collect::<Vec<_>>();
                let image = match channels {
                    1 => {
                        DynamicImage::ImageLuma8(GrayImage::from_raw(width, height, bytes).unwrap())
                    }
                    2 => DynamicImage::ImageLumaA8(
                        GrayAlphaImage::from_raw(width, height, bytes).unwrap(),
                    ),
                    3 => DynamicImage::ImageRgb8(RgbImage::from_raw(width, height, bytes).unwrap()),
                    4 => {
                        DynamicImage::ImageRgba8(RgbaImage::from_raw(width, height, bytes).unwrap())
                    }
                    _ => unreachable!(),
                };
                for factor in [0.0, 0.5, 1.5, 2.0, f64::MAX] {
                    let operation = PipelineOp::Sharpness { factor };
                    let expected =
                        crate::compute::registry::execute_cpu(&operation, &image, Some(mode))
                            .unwrap();
                    let actual = super::simd_sharpness(&image, &operation, Some(mode)).unwrap();
                    assert_eq!(
                        actual.color(),
                        expected.color(),
                        "{mode} {width}x{height} {factor}"
                    );
                    assert_eq!(
                        actual.as_bytes(),
                        expected.as_bytes(),
                        "{mode} {width}x{height} {factor}"
                    );
                }
            }
        }
    }

    #[test]
    fn posterize_direct_copy_matches_all_bits_and_vector_tails() {
        for (width, height) in [(1, 1), (5, 1), (17, 3), (256, 1), (513, 512)] {
            let pixels = (width * height) as usize;
            let luma_bytes = (0..pixels)
                .map(|index| (index.wrapping_mul(73).wrapping_add(19)) as u8)
                .collect::<Vec<_>>();
            let rgb_bytes = (0..pixels * 3)
                .map(|index| (index.wrapping_mul(47).wrapping_add(31)) as u8)
                .collect::<Vec<_>>();
            let luma = DynamicImage::ImageLuma8(
                GrayImage::from_raw(width, height, luma_bytes.clone()).unwrap(),
            );
            let rgb = DynamicImage::ImageRgb8(
                RgbImage::from_raw(width, height, rgb_bytes.clone()).unwrap(),
            );

            for bits in 1..=8u8 {
                let mask = !((1u8 << (8 - bits)) - 1);
                let operation = PipelineOp::Posterize { bits };
                let expected_luma = luma_bytes
                    .iter()
                    .map(|value| value & mask)
                    .collect::<Vec<_>>();
                let expected_rgb = rgb_bytes
                    .iter()
                    .map(|value| value & mask)
                    .collect::<Vec<_>>();
                let actual_luma = simd_posterize(&luma, &operation, Some("L")).unwrap();
                let actual_rgb = simd_posterize(&rgb, &operation, Some("RGB")).unwrap();

                assert!(matches!(actual_luma, DynamicImage::ImageLuma8(_)));
                assert!(matches!(actual_rgb, DynamicImage::ImageRgb8(_)));
                assert_eq!(actual_luma.as_bytes(), expected_luma);
                assert_eq!(actual_rgb.as_bytes(), expected_rgb);
                assert_eq!(luma.as_bytes(), luma_bytes);
                assert_eq!(rgb.as_bytes(), rgb_bytes);
            }
        }
    }

    use super::effects::native_invert;
    use super::rotate::rotate_discrete_fast_angle;
    #[cfg(feature = "parallel")]
    use super::{
        TransposeNativeCollectCounters, TransposeNativeOddCollectCounters,
        TransposeNativeOddRowCache, TransposeNativeRowCache, transpose_native_collect_admitted,
        transpose_native_odd_collect_admitted,
    };
    use super::{
        native_lut_apply, native_lut_map_rows, native_lut_tables_for_channels,
        native_transpose_bytes, simd_posterize, simd_resize_f,
    };
    use crate::pipeline::{PipelineOp, ResampleFilter, TransposeMethod};
    use crate::raster::{DynamicImage, GrayAlphaImage, GrayImage, RgbImage, RgbaImage};

    #[test]
    fn cover_luma_i32_coefficient_guard_fails_closed() {
        let horizontal = super::precompute_coeffs(1365, 1024, ResampleFilter::Bicubic);
        let vertical = super::precompute_coeffs(1024, 768, ResampleFilter::Bicubic);
        assert!(super::resize_u8_coefficients_fit_i32(&horizontal));
        assert!(super::resize_u8_coefficients_fit_i32(&vertical));

        let at_bound = super::FilterCoeffs {
            xmin: vec![0],
            count: vec![1],
            offsets: vec![0],
            weights: vec![8_413_280],
        };
        assert!(super::resize_u8_coefficients_fit_i32(&at_bound));
        let above_bound = super::FilterCoeffs {
            xmin: vec![0],
            count: vec![1],
            offsets: vec![0],
            weights: vec![8_413_281],
        };
        assert!(!super::resize_u8_coefficients_fit_i32(&above_bound));
        let outside_i32 = super::FilterCoeffs {
            xmin: vec![0],
            count: vec![1],
            offsets: vec![0],
            weights: vec![i64::MAX],
        };
        assert!(!super::resize_u8_coefficients_fit_i32(&outside_i32));
    }

    #[test]
    fn alpha_source_row_precompute_matches_per_tap_simd_premultiplication() {
        for channels in [2usize, 4] {
            let source_width = 37usize;
            let output_width = 53usize;
            let source = (0..source_width * channels)
                .map(|index| {
                    let pixel = index / channels;
                    let channel = index % channels;
                    if channel == channels - 1 {
                        match pixel % 4 {
                            0 => 0,
                            1 => 255,
                            _ => (pixel * 47 + 13) as u8,
                        }
                    } else {
                        (pixel * 83 + channel * 61 + 19) as u8
                    }
                })
                .collect::<Vec<_>>();
            let coeffs = super::precompute_coeffs(
                output_width as u32,
                source_width as u32,
                ResampleFilter::Bicubic,
            );
            let plan = super::build_resize_horizontal_plan(&coeffs, output_width, channels, true)
                .expect("horizontal plan");
            let mut expected = vec![0; output_width * channels];
            super::resize::resize_horizontal_vector_row(
                &source,
                channels,
                &coeffs,
                &plan,
                output_width,
                &mut expected,
                true,
            )
            .expect("reference alpha row");

            let mut scratch = vec![0; source.len()];
            let mut actual = vec![0; output_width * channels];
            super::resize::resize_horizontal_convolution_row(
                &source,
                channels,
                &coeffs,
                &plan,
                output_width,
                &mut actual,
                true,
                false,
                &mut scratch,
            )
            .expect("precomputed alpha row");
            assert_eq!(actual, expected, "channels={channels}");
        }
    }

    #[test]
    fn cover_luma_i32_two_pass_matches_widened_for_extreme_samples() {
        let width = 37u32;
        let height = 17u32;
        let output_width = 53u32;
        let output_height = 23u32;
        for pattern in 0..4u32 {
            let samples = (0..width * height)
                .map(|index| match pattern {
                    0 => 0,
                    1 => 255,
                    2 => {
                        let x = index % width;
                        let y = index / width;
                        if (x + y) % 2 == 0 { 0 } else { 255 }
                    }
                    _ => index.wrapping_mul(97).wrapping_add(index / width * 53) as u8,
                })
                .collect::<Vec<_>>();
            let image = DynamicImage::ImageLuma8(
                GrayImage::from_raw(width, height, samples).expect("test L image shape"),
            );
            let output_len = (output_width * output_height) as usize;
            let mut widened = vec![0u8; output_len];
            let mut narrow = vec![0u8; output_len];
            super::simd_resize_convolution_into(
                &image,
                output_width,
                output_height,
                ResampleFilter::Bicubic,
                1,
                false,
                false,
                0,
                false,
                &mut widened,
            )
            .expect("widened Cover resize must succeed");
            super::simd_resize_convolution_into(
                &image,
                output_width,
                output_height,
                ResampleFilter::Bicubic,
                1,
                false,
                true,
                0,
                false,
                &mut narrow,
            )
            .expect("bounded i32 Cover resize must succeed");
            assert_eq!(narrow, widened, "sample pattern {pattern}");
        }
    }

    #[test]
    fn cover_la_i32_two_pass_matches_widened_for_alpha_patterns() {
        let width = 37u32;
        let height = 17u32;
        let output_width = 53u32;
        let output_height = 23u32;
        let horizontal = super::precompute_coeffs(output_width, width, ResampleFilter::Bicubic);
        let vertical = super::precompute_coeffs(output_height, height, ResampleFilter::Bicubic);
        assert!(super::resize_u8_coefficients_fit_i32(&horizontal));
        assert!(super::resize_u8_coefficients_fit_i32(&vertical));

        for pattern in 0..4u32 {
            let samples = (0..width * height)
                .flat_map(|index| {
                    let luma = index.wrapping_mul(97).wrapping_add(index / width * 53) as u8;
                    let alpha = match pattern {
                        0 => 0,
                        1 => 255,
                        2 if (index + index / width) % 2 == 0 => 0,
                        2 => 255,
                        _ => index.wrapping_mul(71).wrapping_add(index / width * 29) as u8,
                    };
                    [luma, alpha]
                })
                .collect::<Vec<_>>();
            let image = DynamicImage::ImageLumaA8(
                GrayAlphaImage::from_raw(width, height, samples).expect("test LA image shape"),
            );
            let output_len = (output_width * output_height * 2) as usize;
            let mut widened = vec![0u8; output_len];
            let mut narrow = vec![0u8; output_len];
            super::simd_resize_convolution_into(
                &image,
                output_width,
                output_height,
                ResampleFilter::Bicubic,
                2,
                true,
                false,
                0,
                false,
                &mut widened,
            )
            .expect("widened LA Cover resize must succeed");
            super::simd_resize_convolution_into(
                &image,
                output_width,
                output_height,
                ResampleFilter::Bicubic,
                2,
                true,
                true,
                0,
                false,
                &mut narrow,
            )
            .expect("bounded i32 LA Cover resize must succeed");
            assert_eq!(narrow, widened, "alpha pattern {pattern}");
        }
    }

    #[test]
    fn cover_hsv_i32_two_pass_matches_widened_at_edges_and_tails() {
        for (width, height, output_width, output_height) in
            [(5, 3, 7, 5), (37, 17, 53, 23), (67, 29, 31, 41)]
        {
            let horizontal = super::precompute_coeffs(output_width, width, ResampleFilter::Bicubic);
            let vertical = super::precompute_coeffs(output_height, height, ResampleFilter::Bicubic);
            assert!(super::resize_u8_coefficients_fit_i32(&horizontal));
            assert!(super::resize_u8_coefficients_fit_i32(&vertical));
            let plan =
                super::build_resize_horizontal_plan(&horizontal, output_width as usize, 3, false)
                    .expect("HSV horizontal plan");

            for pattern in 0..4u32 {
                let source = (0..width * height * 3)
                    .map(|index| match pattern {
                        0 => 0,
                        1 => 255,
                        2 if (index + index / (width * 3)) % 2 == 0 => 0,
                        2 => 255,
                        _ => index
                            .wrapping_mul(97)
                            .wrapping_add(index / (width * 3) * 53)
                            as u8,
                    })
                    .collect::<Vec<_>>();
                let mut widened_intermediate =
                    vec![0u8; height as usize * output_width as usize * 3];
                let mut narrow_intermediate = widened_intermediate.clone();
                for y in 0..height as usize {
                    let source_row = &source[y * width as usize * 3..(y + 1) * width as usize * 3];
                    let row_start = y * output_width as usize * 3;
                    let row_end = row_start + output_width as usize * 3;
                    super::resize::resize_horizontal_vector_row(
                        source_row,
                        3,
                        &horizontal,
                        &plan,
                        output_width as usize,
                        &mut widened_intermediate[row_start..row_end],
                        false,
                    )
                    .expect("wide HSV horizontal pass");
                    super::resize::resize_horizontal_hsv_i32_vector_row(
                        source_row,
                        &horizontal,
                        &plan,
                        output_width as usize,
                        &mut narrow_intermediate[row_start..row_end],
                    )
                    .expect("bounded HSV horizontal pass");
                }
                assert_eq!(
                    narrow_intermediate, widened_intermediate,
                    "horizontal {pattern}"
                );

                let mut widened = vec![0u8; output_width as usize * output_height as usize * 3];
                let mut narrow = widened.clone();
                for y in 0..output_height as usize {
                    let row_start = y * output_width as usize * 3;
                    let row_end = row_start + output_width as usize * 3;
                    super::resize::resize_vertical_vector_row(
                        &widened_intermediate,
                        output_width as usize,
                        height as usize,
                        3,
                        &vertical,
                        y,
                        &mut widened[row_start..row_end],
                        false,
                        false,
                    )
                    .expect("wide HSV vertical pass");
                    super::resize::resize_vertical_hsv_i32_vector_row(
                        &narrow_intermediate,
                        output_width as usize,
                        height as usize,
                        &vertical,
                        y,
                        &mut narrow[row_start..row_end],
                    )
                    .expect("bounded HSV vertical pass");
                }
                assert_eq!(narrow, widened, "vertical {pattern}");
            }
        }
    }

    #[test]
    fn cover_cmyk_i32_two_pass_matches_widened_at_edges_and_tails() {
        for (width, height, output_width, output_height) in
            [(5u32, 3u32, 7u32, 5u32), (37, 17, 53, 23), (67, 29, 31, 41)]
        {
            let horizontal = super::precompute_coeffs(output_width, width, ResampleFilter::Bicubic);
            let vertical = super::precompute_coeffs(output_height, height, ResampleFilter::Bicubic);
            assert!(super::resize_u8_coefficients_fit_i32(&horizontal));
            assert!(super::resize_u8_coefficients_fit_i32(&vertical));

            for pattern in 0..4u32 {
                let source = (0..width as usize * height as usize * 4)
                    .map(|index| match pattern {
                        0 => 0,
                        1 => 255,
                        2 => {
                            let pixel = index / 4;
                            if ((pixel % width as usize) ^ (pixel / width as usize)) & 1 == 0 {
                                0
                            } else {
                                255
                            }
                        }
                        _ => index
                            .wrapping_mul(97)
                            .wrapping_add((index / (width as usize * 4)).wrapping_mul(53))
                            .to_le_bytes()[0],
                    })
                    .collect::<Vec<_>>();
                let image = DynamicImage::ImageRgba8(
                    RgbaImage::from_raw(width, height, source)
                        .expect("test CMYK byte-storage image shape"),
                );
                let output_len = (output_width * output_height * 4) as usize;
                let mut widened = vec![0u8; output_len];
                let mut narrow = vec![0u8; output_len];
                super::simd_resize_convolution_into(
                    &image,
                    output_width,
                    output_height,
                    ResampleFilter::Bicubic,
                    4,
                    false,
                    false,
                    0,
                    false,
                    &mut widened,
                )
                .expect("widened CMYK Cover resize must succeed");
                super::simd_resize_convolution_into(
                    &image,
                    output_width,
                    output_height,
                    ResampleFilter::Bicubic,
                    4,
                    false,
                    true,
                    0,
                    false,
                    &mut narrow,
                )
                .expect("bounded i32 CMYK Cover resize must succeed");
                assert_eq!(
                    narrow, widened,
                    "sample pattern {pattern}, {width}x{height} -> {output_width}x{output_height}"
                );
            }
        }
    }

    #[test]
    fn rgba_i32_resize_matches_widened_for_filters_and_alpha_patterns() {
        for (width, height, output_width, output_height) in
            [(5u32, 3u32, 7u32, 5u32), (37, 17, 53, 23), (73, 41, 13, 9)]
        {
            for filter in [
                ResampleFilter::Bilinear,
                ResampleFilter::Bicubic,
                ResampleFilter::Lanczos,
                ResampleFilter::Hamming,
                ResampleFilter::Box,
            ] {
                let horizontal = super::precompute_coeffs(output_width, width, filter);
                let vertical = super::precompute_coeffs(output_height, height, filter);
                assert!(super::resize_u8_coefficients_fit_i32(&horizontal));
                assert!(super::resize_u8_coefficients_fit_i32(&vertical));
                let operation = PipelineOp::Resize {
                    w: output_width,
                    h: output_height,
                    filter,
                };

                for pattern in 0..4u32 {
                    let source = (0..width * height)
                        .flat_map(|index| {
                            let alpha = match pattern {
                                0 => 0,
                                1 => 255,
                                2 if (index + index / width) % 2 == 0 => 0,
                                2 => 255,
                                _ => index.wrapping_mul(47).wrapping_add(index / width * 61) as u8,
                            };
                            [
                                index.wrapping_mul(73).wrapping_add(31) as u8,
                                index.wrapping_mul(29).wrapping_add(19) as u8,
                                index.wrapping_mul(97).wrapping_add(11) as u8,
                                alpha,
                            ]
                        })
                        .collect::<Vec<_>>();
                    let image = DynamicImage::ImageRgba8(
                        RgbaImage::from_raw(width, height, source).expect("test RGBA image shape"),
                    );
                    let widened = super::simd_resize_convolution(
                        &image,
                        output_width,
                        output_height,
                        filter,
                        4,
                        true,
                        false,
                    )
                    .expect("wide SIMD RGBA resize");
                    let narrow = super::simd_resize(&image, &operation, Some("RGBA"))
                        .expect("bounded i32 SIMD RGBA resize");
                    assert!(matches!(narrow, DynamicImage::ImageRgba8(_)));
                    assert_eq!(
                        narrow.as_bytes(),
                        widened.as_bytes(),
                        "{width}x{height}->{output_width}x{output_height} {filter:?} pattern {pattern}"
                    );
                }
            }
        }
    }

    #[test]
    fn native_invert_writes_owned_p_indices_in_one_vector_pass() {
        for width in [1u32, 15, 16, 17, 31] {
            let height = 3;
            let source = (0..width * height)
                .map(|index| (index.wrapping_mul(73).wrapping_add(19)) as u8)
                .collect::<Vec<_>>();
            let image = DynamicImage::ImageLuma8(
                GrayImage::from_raw(width, height, source.clone()).unwrap(),
            );
            let inverted = native_invert(&image, Some("P"), false).unwrap();
            let expected = source.iter().map(|value| 255 - value).collect::<Vec<_>>();

            assert!(matches!(inverted, DynamicImage::ImageLuma8(_)));
            assert_eq!(inverted.as_bytes(), expected);
            assert_eq!(image.as_bytes(), source, "input remains immutable");
        }
    }

    #[test]
    fn native_cmyk_grayscale_matches_exact_conversion_across_vector_tails() {
        for width in [1u32, 15, 16, 17, 31, 32, 33] {
            let height = 3u32;
            let source: Vec<u8> = (0..width * height * 4)
                .map(|index| ((index * 97 + index / 3 + 11) & 255) as u8)
                .collect();
            let expected: Vec<u8> = source
                .chunks_exact(4)
                .map(|pixel| {
                    let nk = 255u32 - u32::from(pixel[3]);
                    let red = (nk - crate::color::muldiv255(u32::from(pixel[0]), nk)) as u8;
                    let green = (nk - crate::color::muldiv255(u32::from(pixel[1]), nk)) as u8;
                    let blue = (nk - crate::color::muldiv255(u32::from(pixel[2]), nk)) as u8;
                    crate::color::rgb_to_luma_u8(red, green, blue)
                })
                .collect();
            let image = DynamicImage::ImageRgba8(
                RgbaImage::from_raw(width, height, source).expect("valid CMYK storage"),
            );
            let (actual, vector_blocks, scalar_tail) =
                super::grayscale::native_cmyk_grayscale_bytes(&image)
                    .expect("native CMYK grayscale");

            assert_eq!(actual, expected, "CMYK grayscale width {width}");
            assert_eq!(vector_blocks, (width as u64 * height as u64).div_ceil(16));
            assert_eq!(scalar_tail, 0);
        }
    }

    #[test]
    fn native_rgb_grayscale_matches_exact_conversion_across_vector_tails() {
        for (width, height) in [
            (1u32, 3u32),
            (15, 3),
            (16, 3),
            (17, 3),
            (31, 3),
            (32, 3),
            (33, 3),
            (1023, 512),
        ] {
            let pixels = width as usize * height as usize;
            let source: Vec<u8> = (0..pixels * 3)
                .map(|index| ((index * 113 + index / 5 + 29) & 255) as u8)
                .collect();
            let image = DynamicImage::ImageRgb8(
                RgbImage::from_raw(width, height, source).expect("valid RGB storage"),
            );
            let expected = crate::color::pil_grayscale(&image).expect("reference grayscale");
            let (actual, vector_blocks, scalar_tail) =
                super::grayscale::native_grayscale_bytes(&image, 3).expect("native RGB grayscale");

            assert_eq!(
                actual.as_slice(),
                expected.as_raw().as_slice(),
                "RGB grayscale {width}x{height}"
            );
            assert_eq!(vector_blocks, pixels.div_ceil(16) as u64);
            assert_eq!(scalar_tail, 0);
        }
    }

    #[test]
    fn native_la_and_rgba_grayscale_match_exact_conversion_across_vector_tails() {
        use crate::raster::GrayAlphaImage;

        for width in [1u32, 15, 16, 17, 31, 32, 33] {
            let height = 3u32;
            let pixels = width as usize * height as usize;

            let la_source: Vec<u8> = (0..pixels * 2)
                .map(|index| ((index * 89 + index / 7 + 13) & 255) as u8)
                .collect();
            let la_image = DynamicImage::ImageLumaA8(
                GrayAlphaImage::from_raw(width, height, la_source).expect("valid LA storage"),
            );
            let la_expected = crate::color::pil_grayscale(&la_image).expect("LA reference");
            let (la_actual, la_vector_blocks, la_scalar_tail) =
                super::grayscale::native_grayscale_bytes(&la_image, 2)
                    .expect("native LA grayscale");
            assert_eq!(
                la_actual.as_slice(),
                la_expected.as_raw().as_slice(),
                "LA grayscale width {width}"
            );
            assert_eq!(la_vector_blocks, pixels.div_ceil(16) as u64);
            assert_eq!(la_scalar_tail, 0);

            let rgba_source: Vec<u8> = (0..pixels * 4)
                .map(|index| ((index * 71 + index / 9 + 31) & 255) as u8)
                .collect();
            let rgba_image = DynamicImage::ImageRgba8(
                RgbaImage::from_raw(width, height, rgba_source).expect("valid RGBA storage"),
            );
            let rgba_expected = crate::color::pil_grayscale(&rgba_image).expect("RGBA reference");
            let (rgba_actual, rgba_vector_blocks, rgba_scalar_tail) =
                super::grayscale::native_grayscale_bytes(&rgba_image, 4)
                    .expect("native RGBA grayscale");
            assert_eq!(
                rgba_actual.as_slice(),
                rgba_expected.as_raw(),
                "RGBA grayscale width {width}"
            );
            assert_eq!(rgba_vector_blocks, pixels.div_ceil(16) as u64);
            assert_eq!(rgba_scalar_tail, 0);
        }
    }

    #[test]
    fn native_ycbcr_grayscale_gathers_y_across_vector_tails() {
        for (width, height) in [(3u32, 1u32), (15, 3), (16, 3), (17, 3), (1024, 768)] {
            let pixels = width as usize * height as usize;
            let source: Vec<u8> = (0..pixels * 3)
                .map(|index| ((index * 113 + index / 5 + 29) & 255) as u8)
                .collect();
            let expected: Vec<u8> = source.chunks_exact(3).map(|pixel| pixel[0]).collect();
            let image = DynamicImage::ImageRgb8(
                RgbImage::from_raw(width, height, source).expect("valid YCbCr storage"),
            );
            let (actual, vector_blocks, scalar_tail) =
                super::grayscale::native_ycbcr_grayscale_bytes(&image)
                    .expect("native YCbCr grayscale");

            assert_eq!(actual, expected, "YCbCr grayscale {width}x{height}");
            assert_eq!(vector_blocks, (pixels / 16) as u64);
            assert_eq!(scalar_tail, (pixels % 16) as u64);

            let result = super::simd_grayscale(&image, &PipelineOp::Grayscale, Some("YCbCr"))
                .expect("SIMD YCbCr grayscale must be admitted");
            assert_eq!(result.as_bytes(), actual, "SIMD output {width}x{height}");
        }
        #[cfg(target_arch = "x86_64")]
        if std::is_x86_feature_detected!("ssse3") {
            assert_eq!(
                super::grayscale::native_ycbcr_grayscale_path(),
                "ssse3-deinterleave"
            );
        }
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        assert_eq!(
            super::grayscale::native_ycbcr_grayscale_path(),
            "neon-deinterleave"
        );
    }

    #[test]
    fn native_luma_to_rgb_matches_exact_conversion_across_vector_tails() {
        for (width, height) in [
            (1u32, 1u32),
            (5, 3),
            (15, 3),
            (16, 3),
            (17, 3),
            (31, 3),
            (32, 3),
            (33, 3),
            (1023, 512),
        ] {
            let pixels = width as usize * height as usize;
            let source: Vec<u8> = (0..pixels)
                .map(|index| ((index * 73 + index / 11 + 19) & 255) as u8)
                .collect();
            let image = DynamicImage::ImageLuma8(
                GrayImage::from_raw(width, height, source).expect("valid L storage"),
            );
            let expected = image.to_rgb8();
            let (actual, vector_blocks, scalar_tail) =
                super::native_luma_to_rgb_bytes(&image).expect("native L to RGB");

            assert_eq!(
                actual.as_slice(),
                expected.as_raw().as_slice(),
                "{width}x{height}"
            );
            let expected_vector_blocks = pixels.div_ceil(16) as u64;
            assert_eq!(vector_blocks, expected_vector_blocks);
            assert_eq!(scalar_tail, 0);
        }
    }

    #[test]
    fn native_luma_to_typed_samples_does_not_use_rgba_expansion() {
        use crate::ColorMode;

        for width in [1u32, 7, 8, 9, 17, 33] {
            for mode in ["L", "LA"] {
                let luma: Vec<u8> = (0..width as usize * 3)
                    .map(|index| [0, 1, 127, 128, 254, 255][index % 6])
                    .collect();
                let image = if mode == "L" {
                    DynamicImage::ImageLuma8(
                        GrayImage::from_raw(width, 3, luma.clone()).expect("L samples"),
                    )
                } else {
                    let bytes = luma
                        .iter()
                        .enumerate()
                        .flat_map(|(index, &sample)| [sample, index as u8])
                        .collect();
                    DynamicImage::ImageLumaA8(
                        crate::raster::GrayAlphaImage::from_raw(width, 3, bytes)
                            .expect("LA samples"),
                    )
                };
                for target in [ColorMode::I, ColorMode::F] {
                    let expected: Vec<u8> = luma
                        .iter()
                        .flat_map(|&sample| match target {
                            ColorMode::I => i32::from(sample).to_le_bytes(),
                            ColorMode::F => f32::from(sample).to_le_bytes(),
                            _ => unreachable!(),
                        })
                        .collect();
                    let op = PipelineOp::Convert {
                        mode: target.clone(),
                        matrix: None,
                        dither: None,
                    };
                    let actual = super::simd_convert(&image, &op, Some(mode))
                        .expect("typed native SIMD conversion");
                    assert_eq!(
                        actual.as_bytes(),
                        expected,
                        "{mode} to {target:?}, width {width}"
                    );
                }
            }
        }
    }

    #[test]
    fn native_luma_to_rgba_matches_exact_conversion_across_vector_tails() {
        for (width, height) in [
            (1u32, 1u32),
            (3, 5),
            (15, 3),
            (16, 3),
            (17, 3),
            (31, 3),
            (32, 3),
            (33, 3),
            (1023, 512),
        ] {
            let pixels = width as usize * height as usize;
            let source: Vec<u8> = (0..pixels)
                .map(|index| ((index * 73 + index / 11 + 19) & 255) as u8)
                .collect();
            let image = DynamicImage::ImageLuma8(
                GrayImage::from_raw(width, height, source).expect("valid L storage"),
            );
            let expected = image.to_rgba8();
            let (actual, vector_blocks, scalar_tail) =
                super::native_luma_to_rgba_bytes(&image).expect("native L to RGBA");

            assert_eq!(
                actual.as_slice(),
                expected.as_raw().as_slice(),
                "{width}x{height}"
            );
            assert_eq!(vector_blocks, pixels.div_ceil(16) as u64);
            assert_eq!(scalar_tail, 0);
        }
    }

    #[test]
    fn native_luma_alpha_to_rgba_matches_exact_conversion_across_vector_tails() {
        for (width, height) in [
            (1u32, 1u32),
            (3, 5),
            (15, 3),
            (16, 3),
            (17, 3),
            (31, 3),
            (32, 3),
            (33, 3),
            (1023, 512),
        ] {
            let pixels = width as usize * height as usize;
            let source: Vec<u8> = (0..pixels * 2)
                .map(|index| ((index * 73 + index / 11 + 19) & 255) as u8)
                .collect();
            let image = DynamicImage::ImageLumaA8(
                crate::raster::GrayAlphaImage::from_raw(width, height, source)
                    .expect("valid LA storage"),
            );
            let expected = image.to_rgba8();
            let layout = super::NativeConvertLayout {
                source_channels: 2,
                target_channels: 4,
                source_is_luma: true,
                source_is_rgbx: false,
                target_is_luma: false,
                target_is_cmyk: false,
                target_is_hsv: false,
                target_is_ycbcr: false,
                target_is_integer: false,
                target_is_float: false,
            };
            let (actual, vector_blocks, scalar_tail) =
                super::native_convert_bytes(&image, layout).expect("native LA to RGBA");

            assert_eq!(
                actual.as_slice(),
                expected.as_raw().as_slice(),
                "{width}x{height}"
            );
            assert_eq!(vector_blocks, pixels.div_ceil(4) as u64);
            assert_eq!(scalar_tail, 0);
        }
    }

    #[test]
    fn native_rgba_to_la_matches_exact_conversion_across_vector_tails() {
        for (width, height) in [
            (1u32, 1u32),
            (15, 3),
            (16, 3),
            (17, 3),
            (31, 3),
            (32, 3),
            (33, 3),
            (1023, 512),
        ] {
            let pixels = width as usize * height as usize;
            let source: Vec<u8> = (0..pixels * 4)
                .map(|index| ((index * 113 + index / 5 + 29) & 255) as u8)
                .collect();
            let image = DynamicImage::ImageRgba8(
                RgbaImage::from_raw(width, height, source).expect("valid RGBA storage"),
            );
            let gray = crate::color::pil_grayscale(&image).expect("reference luma");
            let expected: Vec<u8> = gray
                .as_raw()
                .iter()
                .copied()
                .zip(image.as_bytes().chunks_exact(4).map(|pixel| pixel[3]))
                .flat_map(|(luma, alpha)| [luma, alpha])
                .collect();
            let layout = super::NativeConvertLayout {
                source_channels: 4,
                target_channels: 2,
                source_is_luma: false,
                source_is_rgbx: false,
                target_is_luma: true,
                target_is_cmyk: false,
                target_is_hsv: false,
                target_is_ycbcr: false,
                target_is_integer: false,
                target_is_float: false,
            };
            let (actual, vector_blocks, scalar_tail) =
                super::native_convert_bytes(&image, layout).expect("native RGBA to LA");

            assert_eq!(actual, expected, "RGBA to LA {width}x{height}");
            assert_eq!(vector_blocks, pixels.div_ceil(8) as u64);
            assert_eq!(scalar_tail, 0);
        }
    }

    #[test]
    fn native_rgb_to_rgba_matches_exact_conversion_across_vector_tails() {
        for (width, height) in [
            (0u32, 3u32),
            (3, 0),
            (1, 1),
            (3, 1),
            (4, 1),
            (5, 1),
            (15, 3),
            (16, 3),
            (17, 3),
            (31, 3),
            (32, 3),
            (33, 3),
            (1023, 512),
        ] {
            let pixels = width as usize * height as usize;
            let source: Vec<u8> = (0..pixels * 3)
                .map(|index| ((index * 73 + index / 11 + 19) & 255) as u8)
                .collect();
            let image = DynamicImage::ImageRgb8(
                RgbImage::from_raw(width, height, source).expect("valid RGB storage"),
            );
            let expected = image.to_rgba8();
            let (actual, vector_blocks, scalar_tail) =
                super::native_rgb_to_rgba_bytes(&image).expect("native RGB to RGBA");

            assert_eq!(
                actual.as_slice(),
                expected.as_raw().as_slice(),
                "{width}x{height}"
            );
            #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
            let pixels_per_vector = 16;
            #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
            let pixels_per_vector = 4;
            assert_eq!(
                vector_blocks,
                (pixels / pixels_per_vector) as u64,
                "vector blocks for {width}x{height}"
            );
            assert_eq!(
                scalar_tail,
                (pixels % pixels_per_vector) as u64,
                "scalar tail for {width}x{height}"
            );
        }
    }

    #[test]
    fn native_lut_preserves_channel_order_and_vector_tails() {
        for channels in 1..=4 {
            let lut: Vec<u8> = (0..channels)
                .flat_map(|channel| {
                    (0..256).map(move |value| ((value * 73 + channel * 47 + 19) & 255) as u8)
                })
                .collect();
            for width in [1, 15, 16, 17, 31, 32, 33, 257] {
                for height in [1, 3, 1025] {
                    let mut bytes: Vec<u8> = (0..width * height * channels)
                        .map(|index| ((index * 113 + index / 7) & 255) as u8)
                        .collect();
                    let expected: Vec<u8> = bytes
                        .iter()
                        .enumerate()
                        .map(|(index, &value)| lut[(index % channels) * 256 + usize::from(value)])
                        .collect();
                    let original = bytes.clone();
                    let tables = native_lut_tables_for_channels(&lut, channels)
                        .expect("valid native byte lookup tables");
                    let mut mapped = vec![0u8; bytes.len()];
                    native_lut_map_rows(
                        &bytes,
                        &mut mapped,
                        width,
                        height,
                        channels,
                        &lut,
                        &tables,
                    )
                    .expect("valid out-of-place native byte lookup");
                    assert_eq!(
                        mapped, expected,
                        "out of place {width}x{height}, {channels}"
                    );
                    assert_eq!(
                        bytes, original,
                        "source mutated {width}x{height}, {channels}"
                    );
                    let (_, scalar_tail) =
                        native_lut_apply(&mut bytes, width, height, channels, &lut)
                            .expect("valid native byte lookup");
                    assert_eq!(bytes, expected, "{width}x{height}, {channels} channels");
                    assert_eq!(scalar_tail, 0);
                }
            }
        }
    }

    #[test]
    fn native_rgb_affine_point_lut_matches_table_math_across_tails() {
        use crate::raster::{DynamicImage, RgbImage};

        let coefficients = [(3u8, 5u8), (5, 13), (255, 255)];
        let lut = coefficients
            .iter()
            .flat_map(|&(multiplier, offset)| {
                (0..=u8::MAX)
                    .map(move |sample| sample.wrapping_mul(multiplier).wrapping_add(offset))
            })
            .collect::<Vec<_>>();

        for width in [1u32, 15, 16, 17, 31, 32, 33, 257] {
            let height = 3u32;
            let source = (0..width * height * 3)
                .map(|index| index.wrapping_mul(73).wrapping_add(index / 7) as u8)
                .collect::<Vec<_>>();
            let expected = source
                .chunks_exact(3)
                .flat_map(|pixel| {
                    [
                        lut[usize::from(pixel[0])],
                        lut[256 + usize::from(pixel[1])],
                        lut[512 + usize::from(pixel[2])],
                    ]
                })
                .collect::<Vec<_>>();
            let image = DynamicImage::ImageRgb8(
                RgbImage::from_raw(width, height, source.clone()).expect("valid RGB source"),
            );
            let result =
                super::native_point_lut(&image, Some("RGB"), &lut).expect("affine RGB point LUT");

            assert_eq!(result.as_bytes(), expected, "{width}x{height}");
            assert_eq!(image.as_bytes(), source, "source remains unchanged");
        }
    }

    fn assert_native_transpose_matches_forward_mapping(
        source: &[u8],
        width: usize,
        height: usize,
        channels: usize,
    ) {
        for method in [
            TransposeMethod::FlipLeftRight,
            TransposeMethod::FlipTopBottom,
            TransposeMethod::Rotate90,
            TransposeMethod::Rotate180,
            TransposeMethod::Rotate270,
            TransposeMethod::Transpose,
            TransposeMethod::Transverse,
        ] {
            let swapped = matches!(
                method,
                TransposeMethod::Rotate90
                    | TransposeMethod::Rotate270
                    | TransposeMethod::Transpose
                    | TransposeMethod::Transverse
            );
            let (expected_width, expected_height) = if swapped {
                (height, width)
            } else {
                (width, height)
            };
            let mut expected = vec![0; source.len()];
            // Scatter the original samples using the public forward
            // coordinate mapping, independent of the kernel's inverse
            // mapping and tile layout.
            for y in 0..height {
                for x in 0..width {
                    let (target_x, target_y) = match method {
                        TransposeMethod::FlipLeftRight => (width - 1 - x, y),
                        TransposeMethod::FlipTopBottom => (x, height - 1 - y),
                        TransposeMethod::Rotate90 => (y, width - 1 - x),
                        TransposeMethod::Rotate180 => (width - 1 - x, height - 1 - y),
                        TransposeMethod::Rotate270 => (height - 1 - y, x),
                        TransposeMethod::Transpose => (y, x),
                        TransposeMethod::Transverse => (height - 1 - y, width - 1 - x),
                    };
                    let source_start = (y * width + x) * channels;
                    let target_start = (target_y * expected_width + target_x) * channels;
                    expected[target_start..target_start + channels]
                        .copy_from_slice(&source[source_start..source_start + channels]);
                }
            }
            let (actual, actual_width, actual_height, blocks, tail, kernel_path) =
                native_transpose_bytes(
                    source,
                    width as u32,
                    height as u32,
                    channels,
                    method.clone(),
                )
                .expect("valid native transpose");
            assert_eq!(
                (actual_width, actual_height),
                (expected_width as u32, expected_height as u32)
            );
            assert_eq!(
                actual, expected,
                "{method:?}: {width}x{height}, {channels} channels"
            );
            #[cfg(all(target_arch = "x86_64", not(feature = "parallel")))]
            if channels == 4
                && swapped
                && width >= 8
                && height >= 8
                && std::is_x86_feature_detected!("avx2")
            {
                assert_eq!(kernel_path, "avx2-transpose");
            }
            let tile = if kernel_path == "avx2-transpose" {
                8
            } else {
                match channels {
                    1 => 16,
                    2 => 8,
                    _ => 4,
                }
            };
            if swapped && width >= tile && height >= tile {
                let tiles = (width / tile) * (height / tile);
                #[cfg(feature = "parallel")]
                let uses_collect = transpose_native_collect_admitted(
                    width,
                    height,
                    channels,
                    source.len(),
                    &method,
                );
                #[cfg(not(feature = "parallel"))]
                let uses_collect = false;
                if uses_collect {
                    // Work includes complete groups rebuilt at task splits.
                    // The forced-split test checks that exact extra count.
                    assert!(blocks >= (tiles * tile) as u64);
                    assert_eq!(blocks % height as u64, 0);
                } else {
                    assert_eq!(blocks, (tiles * tile) as u64);
                }
                assert_eq!(
                    tail,
                    ((width * height - tiles * tile * tile) * channels) as u64
                );
            }
        }
    }

    #[test]
    fn transpose_native_tiles_preserve_pixels_at_odd_boundaries() {
        for (width, height) in [
            (0, 0),
            (0, 33),
            (33, 0),
            (1, 37),
            (37, 1),
            (3, 5),
            (4, 4),
            (7, 9),
            (8, 8),
            (15, 17),
            (16, 16),
            (31, 33),
            (33, 31),
            (65, 67),
            (513, 515),
        ] {
            for channels in 1..=4 {
                let source: Vec<u8> = (0..width * height * channels)
                    .map(|index| ((index * 73 + index / 17 * 11 + 29) % 256) as u8)
                    .collect();
                assert_native_transpose_matches_forward_mapping(&source, width, height, channels);
            }
        }
    }

    #[test]
    fn transpose_native_tiles_preserve_float_payload_bits() {
        let patterns = [0x7f80_0001u32, 0x7fc1_2345, 0x8000_0000, 0xff80_0000];
        let source: Vec<u8> = (0..35)
            .flat_map(|index| patterns[index % patterns.len()].to_ne_bytes())
            .collect();
        let (transposed, width, height, _, _, _) =
            native_transpose_bytes(&source, 7, 5, 4, TransposeMethod::Transpose)
                .expect("packed F transpose");
        let (restored, _, _, _, _, _) =
            native_transpose_bytes(&transposed, width, height, 4, TransposeMethod::Transpose)
                .expect("inverse packed F transpose");
        assert_eq!(restored, source);
    }

    #[test]
    fn transpose_native_collect_preserves_aligned_pixels() {
        for (width, height) in [(768, 768), (768, 772)] {
            for channels in [3, 4] {
                let source: Vec<u8> = (0..width * height * channels)
                    .map(|index| ((index * 73 + index / 17 * 11 + 29) % 256) as u8)
                    .collect();
                #[cfg(feature = "parallel")]
                assert!(transpose_native_collect_admitted(
                    width,
                    height,
                    channels,
                    source.len(),
                    &TransposeMethod::Transpose,
                ));
                assert_native_transpose_matches_forward_mapping(&source, width, height, channels);
            }
        }
    }

    #[test]
    fn transpose_native_collect_preserves_typed_payload_bits() {
        let (width, height) = (772, 768);
        let patterns = [
            0x7f80_0001u32,
            0x7fc1_2345,
            0x8000_0000,
            0x0000_0000,
            0xff80_0000,
            0x3f81_2345,
            0xffc5_4321,
        ];
        let source: Vec<u8> = (0..width * height)
            .flat_map(|index| patterns[(index * 3 + index / 19) % patterns.len()].to_ne_bytes())
            .collect();
        // Packed I/F storage uses the same four-byte permutation without
        // interpreting or canonicalizing NaNs, infinities, or signed zero.
        assert_native_transpose_matches_forward_mapping(&source, width, height, 4);
    }

    #[test]
    fn transpose_native_collect_cutoffs_preserve_fallback_pixels() {
        // Exactly 1 MiB and just above 8 MiB both keep the direct-write path.
        for (width, height) in [(512, 512), (1024, 2052)] {
            let source: Vec<u8> = (0..width * height * 4)
                .map(|index| ((index * 73 + index / 17 * 11 + 29) % 256) as u8)
                .collect();
            #[cfg(feature = "parallel")]
            assert!(!transpose_native_collect_admitted(
                width,
                height,
                4,
                source.len(),
                &TransposeMethod::Transpose,
            ));
            assert_native_transpose_matches_forward_mapping(&source, width, height, 4);
        }
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn transpose_native_collect_admission_requires_complete_bounded_tiles() {
        for (width, height, channels, expected) in [
            (512, 512, 4, false),
            (512, 516, 4, true),
            (1024, 2048, 4, true),
            (1024, 2052, 4, false),
            (768, 769, 3, false),
            (769, 768, 4, false),
            (2048, 2048, 1, false),
            (1024, 1024, 2, false),
            (0, 768, 4, false),
        ] {
            assert_eq!(
                transpose_native_collect_admitted(
                    width,
                    height,
                    channels,
                    width * height * channels,
                    &TransposeMethod::Transpose,
                ),
                expected,
                "{width}x{height}, {channels} channels"
            );
        }
        for method in [
            TransposeMethod::FlipLeftRight,
            TransposeMethod::FlipTopBottom,
            TransposeMethod::Rotate180,
        ] {
            assert!(!transpose_native_collect_admitted(
                768,
                768,
                4,
                768 * 768 * 4,
                &method,
            ));
        }
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn transpose_native_collect_counts_split_cache_work_and_allocations() {
        use crate::checked_dims::CheckedDims;
        use crate::compute::Backend;
        use std::sync::atomic::Ordering;

        struct RestoreTelemetry(bool);
        impl Drop for RestoreTelemetry {
            fn drop(&mut self) {
                Backend::set_pipeline_telemetry_enabled(self.0);
            }
        }

        let (width, height) = (772usize, 768usize);
        let source: Vec<u8> = (0..width * height * 3)
            .map(|index| ((index * 73 + index / 17 * 11 + 29) % 256) as u8)
            .collect();
        let output_dims = CheckedDims::new(height as u32, width as u32, 3).unwrap();
        let scratch_dims = CheckedDims::new(height as u32, 4, 3).unwrap();
        let counters = TransposeNativeCollectCounters::default();
        let block_count = output_dims.total_bytes() / 12;
        let midpoint = block_count / 2;
        assert_eq!(midpoint / (height / 4), 386);

        let _telemetry = RestoreTelemetry(Backend::set_pipeline_telemetry_enabled(true));
        let _ = Backend::take_pipeline_allocation_telemetry();
        let mut blocks = Vec::with_capacity(block_count);
        // Force two indexed tasks to meet halfway through rows 384..388.
        // Each task must build that complete group in its private cache.
        for indices in [0..midpoint, midpoint..block_count] {
            let mut cache = TransposeNativeRowCache::new(&scratch_dims, &counters);
            for index in indices {
                blocks.push(cache.block::<3, 12>(&source, width, height, index, false, false));
            }
        }
        assert_eq!(counters.scratch_allocations.load(Ordering::Relaxed), 2);
        assert_eq!(counters.cache_rebuilds.load(Ordering::Relaxed), 194);
        let before_finish = Backend::take_pipeline_allocation_telemetry();
        assert_eq!(before_finish.allocation_count, 0);
        assert_eq!(before_finish.allocated_bytes, 0);

        let (actual, vector_blocks) = counters.finish(blocks, height, scratch_dims.total_bytes());
        assert_eq!(vector_blocks, 194 * height as u64);
        assert_eq!(vector_blocks - (width * height / 4) as u64, height as u64);
        let allocations = Backend::take_pipeline_allocation_telemetry();
        assert_eq!(allocations.allocation_count, 3);
        assert_eq!(
            allocations.allocated_bytes,
            (output_dims.total_bytes() + 2 * scratch_dims.total_bytes()) as u64
        );
        let mut expected = vec![0; output_dims.total_bytes()];
        for y in 0..height {
            for x in 0..width {
                let source_start = (y * width + x) * 3;
                let target_start = (x * height + y) * 3;
                expected[target_start..target_start + 3]
                    .copy_from_slice(&source[source_start..source_start + 3]);
            }
        }
        assert_eq!(actual, expected);
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn transpose_native_collect_cache_handles_arbitrary_block_access() {
        use crate::checked_dims::CheckedDims;
        use std::sync::atomic::Ordering;

        fn check<const CHANNELS: usize, const BLOCK_BYTES: usize>() {
            let (width, height) = (16usize, 20usize);
            let source: Vec<u8> = (0..width * height * CHANNELS)
                .map(|index| ((index * 73 + index / 17 * 11 + 29) % 256) as u8)
                .collect();
            let output_dims =
                CheckedDims::new(height as u32, width as u32, CHANNELS as u8).unwrap();
            let scratch_dims = CheckedDims::new(height as u32, 4, CHANNELS as u8).unwrap();
            for method in [
                TransposeMethod::Transpose,
                TransposeMethod::Rotate90,
                TransposeMethod::Rotate270,
                TransposeMethod::Transverse,
            ] {
                let mut expected = vec![0; output_dims.total_bytes()];
                for y in 0..height {
                    for x in 0..width {
                        let (target_x, target_y) = match method {
                            TransposeMethod::Transpose => (y, x),
                            TransposeMethod::Rotate90 => (y, width - 1 - x),
                            TransposeMethod::Rotate270 => (height - 1 - y, x),
                            TransposeMethod::Transverse => (height - 1 - y, width - 1 - x),
                            _ => unreachable!(),
                        };
                        let from = (y * width + x) * CHANNELS;
                        let to = (target_y * height + target_x) * CHANNELS;
                        expected[to..to + CHANNELS].copy_from_slice(&source[from..from + CHANNELS]);
                    }
                }
                let reverse_x = matches!(
                    method,
                    TransposeMethod::Rotate90 | TransposeMethod::Transverse
                );
                let reverse_y = matches!(
                    method,
                    TransposeMethod::Rotate270 | TransposeMethod::Transverse
                );
                let counters = TransposeNativeCollectCounters::default();
                {
                    let mut cache = TransposeNativeRowCache::new(&scratch_dims, &counters);
                    // Start within a group, visit both edges, jump forward,
                    // then revisit earlier groups. Group sequence is
                    // 0,0,0,1,1,1,3,0,3,0: exactly six complete rebuilds.
                    for index in [
                        height / 2,
                        0,
                        height - 1,
                        height,
                        height + 1,
                        2 * height - 1,
                        3 * height + 2,
                        height / 2,
                        4 * height - 1,
                        0,
                    ] {
                        let actual = cache.block::<CHANNELS, BLOCK_BYTES>(
                            &source, width, height, index, reverse_x, reverse_y,
                        );
                        let start = index * BLOCK_BYTES;
                        assert_eq!(
                            actual.as_slice(),
                            &expected[start..start + BLOCK_BYTES],
                            "{method:?}, {CHANNELS} channels, block {index}"
                        );
                    }
                }
                assert_eq!(counters.scratch_allocations.load(Ordering::Relaxed), 1);
                assert_eq!(counters.cache_rebuilds.load(Ordering::Relaxed), 6);
            }
        }

        check::<3, 12>();
        check::<4, 16>();
    }

    #[cfg(feature = "parallel")]
    mod transpose_native_collect_sixteen_pixel_tests {
        use super::super::{
            TransposeNativeCollectCounters, TransposeNativeRowCache, transpose_native_collect,
        };
        use crate::checked_dims::CheckedDims;
        use crate::compute::Backend;
        use crate::pipeline::TransposeMethod;
        use std::sync::atomic::Ordering;

        fn reference<const CHANNELS: usize>(
            source: &[u8],
            width: usize,
            height: usize,
            method: &TransposeMethod,
        ) -> Vec<u8> {
            let dimensions = CheckedDims::new(height as u32, width as u32, CHANNELS as u8).unwrap();
            let mut output = vec![0; dimensions.total_bytes()];
            for y in 0..height {
                for x in 0..width {
                    let (tx, ty) = match method {
                        TransposeMethod::Transpose => (y, x),
                        TransposeMethod::Rotate90 => (y, width - 1 - x),
                        TransposeMethod::Rotate270 => (height - 1 - y, x),
                        TransposeMethod::Transverse => (height - 1 - y, width - 1 - x),
                        _ => unreachable!(),
                    };
                    let from = (y * width + x) * CHANNELS;
                    let to = (ty * height + tx) * CHANNELS;
                    output[to..to + CHANNELS].copy_from_slice(&source[from..from + CHANNELS]);
                }
            }
            output
        }

        fn patterned<const CHANNELS: usize>(width: usize, height: usize) -> Vec<u8> {
            let dimensions = CheckedDims::new(width as u32, height as u32, CHANNELS as u8).unwrap();
            let exceptional = [0x7f80_0001u32, 0x7fc1_2345, 0x8000_0000, 0, 0xff80_0000];
            let mut source: Vec<u8> = (0..dimensions.total_bytes().div_ceil(4))
                .flat_map(|index| {
                    let bits = if index % 11 < exceptional.len() {
                        exceptional[index % 11]
                    } else {
                        (index as u32)
                            .wrapping_mul(0x9e37_79b9)
                            .rotate_left((index % 31) as u32)
                    };
                    bits.to_ne_bytes()
                })
                .collect();
            source.truncate(dimensions.total_bytes());
            // Unused ImageBuffer suffix must never become a pixel.
            source.extend_from_slice(&[0xA5; 19]);
            source
        }

        fn verify_allocations(
            logical_bytes: usize,
            height: usize,
            channels: usize,
        ) -> crate::compute::PipelineAllocationTelemetry {
            let allocations = Backend::take_pipeline_allocation_telemetry();
            assert!(allocations.allocation_count >= 2);
            assert_eq!(
                allocations.allocated_bytes,
                logical_bytes as u64
                    + (allocations.allocation_count - 1) * (height * 4 * channels) as u64,
            );
            allocations
        }

        fn check_pixels<const CHANNELS: usize, const BLOCK_BYTES: usize>() {
            for (width, height) in [
                (4, 4),
                (4, 12),
                (12, 4),
                (8, 12),
                (12, 20),
                (20, 12),
                (32, 36),
                (36, 32),
            ] {
                let source = patterned::<CHANNELS>(width, height);
                let original = source.clone();
                let dims = CheckedDims::new(height as u32, width as u32, CHANNELS as u8).unwrap();
                for (method, rx, ry) in [
                    (TransposeMethod::Transpose, false, false),
                    (TransposeMethod::Rotate90, true, false),
                    (TransposeMethod::Rotate270, false, true),
                    (TransposeMethod::Transverse, true, true),
                ] {
                    let expected = reference::<CHANNELS>(&source, width, height, &method);
                    let _ = Backend::take_pipeline_allocation_telemetry();
                    let (actual, vectors) = transpose_native_collect::<CHANNELS, BLOCK_BYTES>(
                        &source, width, height, rx, ry, &dims,
                    )
                    .unwrap();
                    verify_allocations(dims.total_bytes(), height, CHANNELS);
                    assert_eq!(actual, expected);
                    assert!(vectors >= (width * height / 4) as u64);
                    assert_eq!(vectors % height as u64, 0);
                    assert_eq!(source, original);
                }
            }
        }

        fn arbitrary_cache_checks<const CHANNELS: usize, const BLOCK_BYTES: usize>() {
            let (width, height) = (16usize, 20usize);
            let source = patterned::<CHANNELS>(width, height);
            let group_blocks = height / 4;
            let count = width * height / 16;
            let scratch_dims = CheckedDims::new(height as u32, 4, CHANNELS as u8).unwrap();
            for (method, rx, ry) in [
                (TransposeMethod::Transpose, false, false),
                (TransposeMethod::Rotate90, true, false),
                (TransposeMethod::Rotate270, false, true),
                (TransposeMethod::Transverse, true, true),
            ] {
                let expected = reference::<CHANNELS>(&source, width, height, &method);
                let counters = TransposeNativeCollectCounters::default();
                {
                    let mut cache = TransposeNativeRowCache::new(&scratch_dims, &counters);
                    cache.scratch.fill(0xA5);
                    for index in [
                        count - 1,
                        0,
                        group_blocks - 1,
                        group_blocks,
                        group_blocks + 1,
                        count - 1,
                        0,
                        count - 2,
                        count - 1,
                        group_blocks - 1,
                    ] {
                        let actual = cache
                            .block::<CHANNELS, BLOCK_BYTES>(&source, width, height, index, rx, ry);
                        assert_eq!(
                            actual.as_slice(),
                            &expected[index * BLOCK_BYTES..(index + 1) * BLOCK_BYTES]
                        );
                    }
                }
                assert_eq!(counters.scratch_allocations.load(Ordering::Relaxed), 1);
                assert_eq!(counters.cache_rebuilds.load(Ordering::Relaxed), 7);
            }
        }

        fn forced_split_checks<const CHANNELS: usize, const BLOCK_BYTES: usize>() {
            let (width, height) = (772usize, 768usize);
            let source = patterned::<CHANNELS>(width, height);
            let count = width * height / 16;
            let midpoint = count / 2;
            assert_eq!(midpoint * 16 / height, 386);
            let scratch_dims = CheckedDims::new(height as u32, 4, CHANNELS as u8).unwrap();
            for (method, rx, ry) in [
                (TransposeMethod::Transpose, false, false),
                (TransposeMethod::Rotate90, true, false),
                (TransposeMethod::Rotate270, false, true),
                (TransposeMethod::Transverse, true, true),
            ] {
                let expected = reference::<CHANNELS>(&source, width, height, &method);
                let counters = TransposeNativeCollectCounters::default();
                let _ = Backend::take_pipeline_allocation_telemetry();
                let mut blocks = Vec::with_capacity(count);
                for indices in [0..midpoint, midpoint..count] {
                    let mut cache = TransposeNativeRowCache::new(&scratch_dims, &counters);
                    cache.scratch.fill(0xA5);
                    for index in indices {
                        blocks.push(
                            cache.block::<CHANNELS, BLOCK_BYTES>(
                                &source, width, height, index, rx, ry,
                            ),
                        );
                    }
                }
                assert_eq!(counters.cache_rebuilds.load(Ordering::Relaxed), 194);
                assert_eq!(counters.scratch_allocations.load(Ordering::Relaxed), 2);
                let before_finish = Backend::take_pipeline_allocation_telemetry();
                assert_eq!(before_finish.allocation_count, 0);
                assert_eq!(before_finish.allocated_bytes, 0);
                let (output, vectors) = counters.finish(blocks, height, scratch_dims.total_bytes());
                let allocations = Backend::take_pipeline_allocation_telemetry();
                assert_eq!(output, expected);
                assert_eq!(vectors, 194 * height as u64);
                assert_eq!(vectors - (width * height / 4) as u64, height as u64);
                assert_eq!(allocations.allocation_count, 3);
                assert_eq!(
                    allocations.allocated_bytes,
                    (width * height * CHANNELS + 2 * height * 4 * CHANNELS) as u64
                );
            }
        }

        struct RestoreTelemetry(bool);
        impl Drop for RestoreTelemetry {
            fn drop(&mut self) {
                Backend::set_pipeline_telemetry_enabled(self.0);
            }
        }

        #[test]
        fn transpose_native_collect_sixteen_pixel_preserves_payloads_and_edges() {
            let _telemetry = RestoreTelemetry(Backend::set_pipeline_telemetry_enabled(true));
            check_pixels::<3, 48>();
            check_pixels::<4, 64>();
        }

        #[test]
        fn transpose_native_collect_sixteen_pixel_handles_arbitrary_cache_access() {
            arbitrary_cache_checks::<3, 48>();
            arbitrary_cache_checks::<4, 64>();
        }

        #[test]
        fn transpose_native_collect_sixteen_pixel_counts_split_work_and_allocations() {
            let _telemetry = RestoreTelemetry(Backend::set_pipeline_telemetry_enabled(true));
            forced_split_checks::<3, 48>();
            forced_split_checks::<4, 64>();
        }
    }

    #[cfg(feature = "parallel")]
    fn odd_transpose_reference(
        source: &[u8],
        width: usize,
        height: usize,
        method: &TransposeMethod,
    ) -> Vec<u8> {
        let output_width = if matches!(
            method,
            TransposeMethod::Rotate90
                | TransposeMethod::Rotate270
                | TransposeMethod::Transpose
                | TransposeMethod::Transverse
        ) {
            height
        } else {
            width
        };
        let mut expected = vec![0; width * height * 4];
        for y in 0..height {
            for x in 0..width {
                let (target_x, target_y) = match method {
                    TransposeMethod::FlipLeftRight => (width - 1 - x, y),
                    TransposeMethod::FlipTopBottom => (x, height - 1 - y),
                    TransposeMethod::Rotate90 => (y, width - 1 - x),
                    TransposeMethod::Rotate180 => (width - 1 - x, height - 1 - y),
                    TransposeMethod::Rotate270 => (height - 1 - y, x),
                    TransposeMethod::Transpose => (y, x),
                    TransposeMethod::Transverse => (height - 1 - y, width - 1 - x),
                };
                let from = (y * width + x) * 4;
                let to = (target_y * output_width + target_x) * 4;
                expected[to..to + 4].copy_from_slice(&source[from..from + 4]);
            }
        }
        expected
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn transpose_native_odd_collect_admission_bounds() {
        let method = TransposeMethod::Transpose;
        for (width, height, expected) in [
            (767, 683, false), // Immediately below two MiB.
            (768, 683, true),
            (1024, 512, false), // Exactly two MiB, aligned.
            (2047, 1024, true),
            (2049, 1024, false), // Above eight MiB.
            (1024, 2048, false), // Aligned eight-MiB collector stays separate.
            (769, 2048, true),
            (769, 2049, false), // Four-row scratch would exceed 32 KiB.
            (1604, 401, true),
            (1605, 401, false),
            (401, 1604, true),
            (401, 1605, false),
            (255, 2048, false),
            (2048, 255, false),
            (33, 31777, false),
            (31777, 33, false),
            (768, 769, true),
            (769, 768, true),
            (1024, 1024, false),
            (513, 515, false),
            (0, 1025, false),
            (1025, 0, false),
        ] {
            assert_eq!(
                transpose_native_odd_collect_admitted(
                    width,
                    height,
                    4,
                    width * height * 4,
                    &method,
                ),
                expected,
                "{width}x{height}"
            );
        }
        for channels in 1..=4 {
            for method in [
                TransposeMethod::FlipLeftRight,
                TransposeMethod::FlipTopBottom,
                TransposeMethod::Rotate90,
                TransposeMethod::Rotate180,
                TransposeMethod::Rotate270,
                TransposeMethod::Transpose,
                TransposeMethod::Transverse,
            ] {
                let swapped = matches!(
                    method,
                    TransposeMethod::Rotate90
                        | TransposeMethod::Rotate270
                        | TransposeMethod::Transpose
                        | TransposeMethod::Transverse
                );
                assert_eq!(
                    transpose_native_odd_collect_admitted(
                        1023,
                        1025,
                        channels,
                        1023 * 1025 * channels,
                        &method,
                    ),
                    channels == 4 && swapped,
                );
            }
        }
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn transpose_native_odd_collect_preserves_all_methods_and_payload_bits() {
        use crate::compute::Backend;

        struct RestoreTelemetry(bool);
        impl Drop for RestoreTelemetry {
            fn drop(&mut self) {
                Backend::set_pipeline_telemetry_enabled(self.0);
            }
        }
        let _telemetry = RestoreTelemetry(Backend::set_pipeline_telemetry_enabled(true));
        let typed_bits = [
            0x7f80_0001u32,
            0x7fc1_2345,
            0x8000_0000,
            0x0000_0000,
            0xff80_0000,
            0xffc5_4321,
        ];
        for (width, height) in [(768usize, 769usize), (1023, 1025)] {
            let mut source: Vec<u8> = (0..width * height)
                .flat_map(|index| {
                    let bits = if index % 11 < typed_bits.len() {
                        typed_bits[index % 11]
                    } else {
                        (index as u32)
                            .wrapping_mul(0x9e37_79b9)
                            .rotate_left((index % 31) as u32)
                    };
                    bits.to_ne_bytes()
                })
                .collect();
            // Valid ImageBuffer storage may include unused trailing bytes.
            source.extend_from_slice(&[0xA5; 19]);
            let original = source.clone();
            for method in [
                TransposeMethod::FlipLeftRight,
                TransposeMethod::FlipTopBottom,
                TransposeMethod::Rotate90,
                TransposeMethod::Rotate180,
                TransposeMethod::Rotate270,
                TransposeMethod::Transpose,
                TransposeMethod::Transverse,
            ] {
                let swapped = matches!(
                    method,
                    TransposeMethod::Rotate90
                        | TransposeMethod::Rotate270
                        | TransposeMethod::Transpose
                        | TransposeMethod::Transverse
                );
                let expected = odd_transpose_reference(&source, width, height, &method);
                let _ = Backend::take_pipeline_allocation_telemetry();
                let (actual, out_width, out_height, vectors, tail, _) =
                    native_transpose_bytes(&source, width as u32, height as u32, 4, method.clone())
                        .expect("odd native four-byte transpose");
                let allocations = Backend::take_pipeline_allocation_telemetry();
                assert_eq!(actual, expected, "{width}x{height} {method:?}");
                assert_eq!(
                    (out_width, out_height),
                    if swapped {
                        (height as u32, width as u32)
                    } else {
                        (width as u32, height as u32)
                    }
                );
                if swapped {
                    assert!(transpose_native_odd_collect_admitted(
                        width,
                        height,
                        4,
                        expected.len(),
                        &method,
                    ));
                    let tile_pixels = (width / 4 * 4) * (height / 4 * 4);
                    // Indexed task splits may repeat complete cache fills.
                    // The deterministic split test below proves exact work.
                    assert!(vectors >= (tile_pixels / 4) as u64);
                    assert!(tail >= ((width * height - tile_pixels) * 4) as u64);
                    let padded_bytes = (width * height).div_ceil(4) * 16;
                    let scratch_bytes = height * 4 * 4;
                    assert!(allocations.allocation_count >= 2);
                    assert_eq!(
                        allocations.allocated_bytes,
                        padded_bytes as u64
                            + (allocations.allocation_count - 1) * scratch_bytes as u64,
                    );
                }
                assert_eq!(source, original);
            }
        }
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn transpose_native_odd_collect_boundaries_keep_fallback_exact() {
        for (width, height, admitted) in [
            (767usize, 683usize, false),
            (768, 683, true),
            (2047, 1024, true),
            (2049, 1024, false),
            (769, 2048, true),
            (769, 2049, false),
            (1604, 401, true),
            (1605, 401, false),
        ] {
            let source: Vec<u8> = (0..width * height * 4)
                .map(|index| ((index * 73 + index / 17 * 29) % 256) as u8)
                .collect();
            let method = TransposeMethod::Transverse;
            assert_eq!(
                transpose_native_odd_collect_admitted(width, height, 4, source.len(), &method),
                admitted,
            );
            let expected = odd_transpose_reference(&source, width, height, &method);
            let (actual, out_width, out_height, vectors, tail, _) =
                native_transpose_bytes(&source, width as u32, height as u32, 4, method)
                    .expect("odd transpose boundary");
            assert_eq!(actual, expected, "{width}x{height}");
            assert_eq!((out_width, out_height), (height as u32, width as u32));
            if !admitted {
                let tile_pixels = (width / 4 * 4) * (height / 4 * 4);
                assert_eq!(vectors, (tile_pixels / 4) as u64);
                assert_eq!(tail, ((width * height - tile_pixels) * 4) as u64);
            }
        }
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn transpose_native_odd_collect_all_padding_lengths() {
        use crate::checked_dims::CheckedDims;
        use crate::compute::Backend;

        struct RestoreTelemetry(bool);
        impl Drop for RestoreTelemetry {
            fn drop(&mut self) {
                Backend::set_pipeline_telemetry_enabled(self.0);
            }
        }
        let _telemetry = RestoreTelemetry(Backend::set_pipeline_telemetry_enabled(true));
        let height = 13usize;
        for width in [13usize, 14, 15, 16] {
            let source: Vec<u8> = (0..width * height * 4)
                .map(|index| ((index * 73 + index / 17 * 29) % 256) as u8)
                .collect();
            let dimensions = CheckedDims::new(height as u32, width as u32, 4).unwrap();
            for (method, reverse_x, reverse_y) in [
                (TransposeMethod::Transpose, false, false),
                (TransposeMethod::Rotate90, true, false),
                (TransposeMethod::Rotate270, false, true),
                (TransposeMethod::Transverse, true, true),
            ] {
                let expected = odd_transpose_reference(&source, width, height, &method);
                let _ = Backend::take_pipeline_allocation_telemetry();
                // Exercise every final-block length directly below admission;
                // admission tests separately preserve the scheduling bounds.
                let (actual, _, _) = super::transpose_native_odd_collect(
                    &source,
                    width,
                    height,
                    reverse_x,
                    reverse_y,
                    &dimensions,
                )
                .expect("complete initialized odd transpose blocks");
                let allocations = Backend::take_pipeline_allocation_telemetry();
                assert_eq!(actual, expected);
                let padded_bytes = (width * height).div_ceil(4) * 16;
                assert_eq!(padded_bytes - actual.len(), (4 - width % 4) % 4 * 4);
                assert!(allocations.allocation_count >= 2);
                assert_eq!(
                    allocations.allocated_bytes,
                    padded_bytes as u64
                        + (allocations.allocation_count - 1) * (height * 4 * 4) as u64,
                );
            }
        }
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn transpose_native_odd_collect_final_cache_revisits_and_split_counters() {
        use crate::checked_dims::CheckedDims;
        use crate::compute::Backend;
        use std::sync::atomic::Ordering;

        struct RestoreTelemetry(bool);
        impl Drop for RestoreTelemetry {
            fn drop(&mut self) {
                Backend::set_pipeline_telemetry_enabled(self.0);
            }
        }
        let _telemetry = RestoreTelemetry(Backend::set_pipeline_telemetry_enabled(true));
        let (width, height) = (15usize, 13usize);
        let source: Vec<u8> = (0..width * height * 4)
            .map(|index| ((index * 73 + index / 17 * 29) % 256) as u8)
            .collect();
        let scratch_dims = CheckedDims::new(height as u32, 4, 4).unwrap();
        let count = (width * height).div_ceil(4);
        for (method, reverse_x, reverse_y) in [
            (TransposeMethod::Transpose, false, false),
            (TransposeMethod::Rotate90, true, false),
            (TransposeMethod::Rotate270, false, true),
            (TransposeMethod::Transverse, true, true),
        ] {
            let expected = odd_transpose_reference(&source, width, height, &method);
            let counters = TransposeNativeOddCollectCounters::default();
            {
                let mut cache = TransposeNativeOddRowCache::new(&scratch_dims, &counters);
                cache.scratch.fill(0xA5);
                // Four complete and three partial cache fills, including a
                // first access to the padded final block and backward visits.
                for index in [
                    count - 1,
                    0,
                    height - 1,
                    height,
                    height + 1,
                    count - 1,
                    0,
                    count - 2,
                    count - 1,
                    height - 1,
                ] {
                    let actual =
                        cache.block(&source, width, height, count, index, reverse_x, reverse_y);
                    let start = index * 16;
                    let valid = (expected.len() - start).min(16);
                    assert_eq!(&actual[..valid], &expected[start..start + valid]);
                }
            }
            assert_eq!(counters.scratch_allocations.load(Ordering::Relaxed), 1);
            assert_eq!(counters.vector_blocks.load(Ordering::Relaxed), 48);
            assert_eq!(counters.scalar_tail.load(Ordering::Relaxed), 532);

            let counters = TransposeNativeOddCollectCounters::default();
            let mut blocks = Vec::with_capacity(count);
            let split = height + height / 2;
            for range in [0..split, split..count] {
                let mut cache = TransposeNativeOddRowCache::new(&scratch_dims, &counters);
                cache.scratch.fill(0xA5);
                for index in range {
                    blocks.push(
                        cache.block(&source, width, height, count, index, reverse_x, reverse_y),
                    );
                }
            }
            let _ = Backend::take_pipeline_allocation_telemetry();
            let (actual, vectors, tail) =
                counters.finish(blocks, expected.len(), scratch_dims.total_bytes());
            let allocations = Backend::take_pipeline_allocation_telemetry();
            assert_eq!(actual, expected);
            // The middle group is computed by both tasks: 48 actual vectors
            // instead of 36 unique vectors, and 220 rather than 204 tails.
            assert_eq!(vectors, 48);
            assert_eq!(tail, 220);
            assert_eq!(allocations.allocation_count, 3);
            assert_eq!(allocations.allocated_bytes, 1200);
            assert_eq!(actual.len(), 780); // Four initialized padding bytes removed.
        }
    }

    #[test]
    fn rotate_discrete_fast_path_requires_exact_angle() {
        assert_eq!(rotate_discrete_fast_angle(90.0, None, None), Some(90));
        assert_eq!(rotate_discrete_fast_angle(-90.0, None, None), Some(270));
        assert_eq!(rotate_discrete_fast_angle(450.0, None, None), Some(90));
        for angle in [88.9, 89.9, 90.1, 91.1, 179.9, 180.1, 269.9, 270.1] {
            assert_eq!(
                rotate_discrete_fast_angle(angle, None, None),
                None,
                "fractional angle {angle} must retain affine sampling"
            );
        }
    }

    #[cfg(target_endian = "little")]
    #[test]
    fn f_resize_preserves_simd_negative_zero() {
        // Pillow's 32bpc resampler keeps the sign of a zero produced by
        // cancellation. The SIMD adapter used to erase that sign while
        // converting each f64 lane back to f32.
        let source_words = [
            0x0000_0001,
            0x8000_0001,
            0x0000_0002,
            0x8000_0002,
            0x0000_0008,
            0x0000_0001,
        ];
        let source_bytes: Vec<u8> = source_words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        let source = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(3, 2, source_bytes).expect("source shape must be valid"),
        );

        let output = simd_resize_f(&source, 7, 5, &ResampleFilter::Bilinear)
            .expect("finite F-mode SIMD resize must succeed");
        let DynamicImage::ImageRgba8(output) = output else {
            panic!("F-mode resize must retain packed float storage");
        };
        let word_at = |index: usize| {
            let offset = index * 4;
            u32::from_le_bytes(output.as_raw()[offset..offset + 4].try_into().unwrap())
        };
        assert_eq!(word_at(10), 0x8000_0000);
        assert_eq!(word_at(14), 0x8000_0000);
    }

    #[test]
    fn unsharp_mask_150_blend_matches_scalar_for_every_byte_pair() {
        use super::simd_unsharp_blend_rgb_150;

        let mut original = Vec::with_capacity(256 * 256);
        let mut blurred = Vec::with_capacity(256 * 256);
        for source in 0..=u8::MAX {
            for smooth in 0..=u8::MAX {
                original.push(source);
                blurred.push(smooth);
            }
        }

        for threshold in [-1, 0, 3, i32::from(u8::MAX), 256] {
            let expected: Vec<u8> = original
                .iter()
                .zip(&blurred)
                .map(|(&source, &smooth)| {
                    let difference = i32::from(source) - i32::from(smooth);
                    if difference.abs() > threshold {
                        (i32::from(source) + difference * 150 / 100).clamp(0, 255) as u8
                    } else {
                        source
                    }
                })
                .collect();

            let mut actual = blurred.clone();
            assert_eq!(
                simd_unsharp_blend_rgb_150(&original, &mut actual, threshold),
                true,
                "matching byte buffers use the exact SIMD blend"
            );
            assert_eq!(actual, expected, "threshold={threshold}");
        }
    }

    #[test]
    fn unsharp_mask_150_blend_handles_scalar_tail_and_mismatched_buffers() {
        use super::simd_unsharp_blend_rgb_150;

        let original = [
            0, 1, 2, 3, 128, 254, 255, 100, 101, 200, 10, 11, 12, 250, 7, 9, 16,
        ];
        let blurred = [
            255, 255, 255, 0, 127, 255, 0, 105, 96, 190, 20, 8, 9, 0, 10, 11, 20,
        ];
        let expected: Vec<u8> = original
            .iter()
            .zip(blurred)
            .map(|(&source, smooth)| {
                let difference = i32::from(source) - i32::from(smooth);
                if difference.abs() > 3 {
                    (i32::from(source) + difference * 150 / 100).clamp(0, 255) as u8
                } else {
                    source
                }
            })
            .collect();
        let mut actual = blurred;
        assert!(simd_unsharp_blend_rgb_150(&original, &mut actual, 3));
        assert_eq!(actual.as_slice(), expected.as_slice());
        let mut mismatched = [0; 16];
        assert!(!simd_unsharp_blend_rgb_150(&original, &mut mismatched, 3));
    }

    #[test]
    fn simd_native_l_rank_one_matches_cpu_for_ties_edges_and_vector_tails() {
        use crate::compute::registry;
        use crate::pipeline::PipelineOp;
        use crate::raster::{DynamicImage, GrayImage};

        let operation = PipelineOp::RankFilter { size: 3, rank: 1 };
        for (width, height) in [(17u32, 1u32), (31, 9), (33, 35)] {
            let raw = (0..width as usize * height as usize)
                .map(|index| ((index * 37 + index / width as usize * 11 + 3) % 5) as u8)
                .collect::<Vec<_>>();
            let image = DynamicImage::ImageLuma8(
                GrayImage::from_raw(width, height, raw).expect("valid native L image"),
            );
            let expected = registry::execute_cpu(&operation, &image, Some("L"))
                .expect("CPU L RankFilter reference");
            let actual = super::median_filter::simd_order_statistic_filter(&image, 3, 1, Some("L"))
                .expect("SIMD native L RankFilter");

            assert!(matches!(actual, DynamicImage::ImageLuma8(_)));
            assert_eq!(actual.as_bytes(), expected.as_bytes(), "{width}x{height}");
        }
    }
}
