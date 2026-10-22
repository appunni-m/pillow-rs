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
mod duplicate;
pub(crate) use self::alpha_composite::{simd_alpha_composite, simd_alpha_composite_mirror};
use self::alpha_composite::{
    simd_alpha_composite_operands_supported, simd_alpha_composite_shape_supported,
};
pub(crate) use self::box_blur::simd_box_blur;
use self::box_blur::simd_pil_box_blur;
pub use self::chops::simd_constant;
pub(crate) use self::chops::{
    simd_add, simd_add_modulo, simd_darker, simd_difference, simd_fused_multiply_screen,
    simd_hard_light, simd_lighter, simd_logical_and, simd_logical_or, simd_logical_xor,
    simd_multiply, simd_overlay, simd_screen, simd_soft_light, simd_subtract, simd_subtract_modulo,
};
pub(crate) use self::crop::{simd_crop, simd_crop_border};
#[cfg(all(test, target_arch = "aarch64", target_feature = "neon"))]
use self::duplicate::simd_copy_native_bytes;
pub use self::duplicate::simd_duplicate;
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
    native_filter_3x3_vector,
};
pub(crate) use self::filter::{
    simd_filter_3x3, simd_filter_5x5, simd_unsharp_blend_rgb_150,
    uniform_i32_filter3x3_is_identity_simd,
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
mod put_data;
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
pub(crate) use self::merge::simd_merge;
pub(crate) use self::module_blend::simd_blend_module;
pub(crate) use self::offset::simd_offset;
pub(crate) use self::pad::simd_pad;
pub use self::paste::simd_composite_module;
pub(crate) use self::paste::simd_paste;
use self::paste::{
    native_paste_in_place, native_paste_plan_for_image, native_paste_plan_for_shape,
    native_paste_rgb_to_rgba_supported, native_paste_rgb_to_rgba_supported_for_shape,
    native_paste_rgba_to_rgb_supported, native_paste_rgba_to_rgb_supported_for_shape,
};
pub(crate) use self::projection::{
    simd_cmyk_projection, simd_cmyk_projection_from_positive_writes, simd_rgb_projection,
};
pub(crate) use self::put_alpha::{simd_put_alpha, simd_put_alpha_data};
pub use self::put_data::simd_put_data;
pub(crate) use self::put_pixel::{
    simd_put_pixel, simd_rgba_putpixel_batch, simd_rgba_putpixel_batch_supported,
};
pub(crate) use self::reduce::simd_reduce;
use self::reduce::{
    native_reduce_bytes, native_reduce_sparse_rgb_2x2, native_reduce_supported_for_image,
    native_reduce_supported_for_shape,
};
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
pub(crate) use self::transpose::simd_transpose;
#[cfg(all(not(feature = "parallel"), target_arch = "aarch64"))]
use self::transpose::transpose_native_tiled;
use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::{Image, preserve_mode};
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
        } => put_data::supports_for_image(img, *data_mode, mode),
        PipelineOp::ExtractBand { index } => extract_band::supports_for_image(img, *index, mode),
        // These operations are native memory movement. They do not claim
        // arithmetic vectorization, but they never enter a CPU adapter.
        PipelineOp::Offset { x, .. } => offset::supports_for_image(img, *x, mode),
        PipelineOp::Flip | PipelineOp::Mirror => flip::supports_for_image(img, op, mode),
        PipelineOp::Transpose { .. } => transpose::supports_for_image(img, mode),
        PipelineOp::Crop { .. } | PipelineOp::CropBorder { .. } => {
            crop::supports_for_image(img, op, mode)
        }
        PipelineOp::Expand { .. } => expand::supports_for_image(img, op, mode),
        PipelineOp::Constant { .. } => img.width() != 0 && img.height() != 0,
        PipelineOp::Duplicate => duplicate::supports_for_image(img, mode),
        PipelineOp::Filter3x3 { .. } | PipelineOp::Filter5x5 { .. } => {
            filter::supports_for_image(img, op, mode)
        }
        PipelineOp::BoxBlur { .. } | PipelineOp::BoxBlurXY { .. } => {
            box_blur::supports_for_image(img, op, mode)
        }
        PipelineOp::GaussianBlur { sigma } => gaussian_blur::supports_for_image(img, *sigma, mode),
        PipelineOp::MaxFilter { .. }
        | PipelineOp::MinFilter { .. }
        | PipelineOp::MedianFilter { .. }
        | PipelineOp::RankFilter { .. } => median_filter::supports_for_image(img, op, mode),
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
        } => paste::native_composite_plan_for_image(img, other, mask, *mask_alpha, mode).is_some(),
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
        } => put_data::supports_for_shape(shape, *data_mode, mode),
        PipelineOp::ExtractBand { index } => {
            extract_band::supports_for_shape(shape, *index, mode, pixel_count)
        }
        PipelineOp::Offset { x, .. } => offset::supports_for_shape(shape, *x, mode),
        PipelineOp::Flip | PipelineOp::Mirror => flip::supports_for_shape(shape, op, mode),
        PipelineOp::Transpose { .. } => transpose::supports_for_shape(shape, mode),
        PipelineOp::Crop { .. } | PipelineOp::CropBorder { .. } => {
            crop::supports_for_shape(shape, op, mode)
        }
        PipelineOp::Expand { .. } => expand::supports_for_shape(shape, op, mode),
        PipelineOp::Pad { .. } => pad::supports_for_shape(shape, op, mode),
        PipelineOp::Constant { .. } => shape.width != 0 && shape.height != 0,
        PipelineOp::Duplicate => duplicate::supports_for_shape(shape, mode),
        PipelineOp::Filter3x3 { .. } | PipelineOp::Filter5x5 { .. } => {
            filter::supports_for_shape(shape, op, mode)
        }
        PipelineOp::BoxBlur { .. } | PipelineOp::BoxBlurXY { .. } => {
            box_blur::supports_for_shape(shape, op, mode)
        }
        PipelineOp::GaussianBlur { sigma } => {
            gaussian_blur::supports_for_shape(shape, *sigma, mode)
        }
        PipelineOp::MaxFilter { .. }
        | PipelineOp::MinFilter { .. }
        | PipelineOp::MedianFilter { .. }
        | PipelineOp::RankFilter { .. } => median_filter::supports_for_shape(shape, op, mode),
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
        } => {
            paste::native_composite_plan_for_shape(shape, other, mask, *mask_alpha, mode).is_some()
        }
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

    use super::effects::native_invert;
    use super::rotate::rotate_discrete_fast_angle;
    use super::{native_lut_apply, native_lut_map_rows, native_lut_tables_for_channels};
    use crate::raster::{DynamicImage, GrayImage};

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
}
