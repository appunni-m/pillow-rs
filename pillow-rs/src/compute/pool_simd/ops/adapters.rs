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
pub use self::duplicate::simd_duplicate;
mod draw;
use self::draw::{draw_line_vector_block, native_draw_layout, shape_draw_channels};
mod effects;
mod enhance;
mod expand;
mod extract_band;
mod filter;
mod histogram;
mod layout;
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
mod native_arithmetic;
mod native_bytes;
mod neighborhood;
mod offset;
mod pad;
mod paste;
mod pipeline;
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
use self::layout::{
    SIMD_BYTE_VECTOR_LANES, SimdImageShape, SimdLayout, has_empty_native_bytes,
    has_nonempty_byte_data, has_valid_byte_data, has_valid_luma16_copy_bytes,
    has_vectorized_byte_rows, has_vectorized_luma16_offset, has_vectorized_mirror_rows,
    has_vectorized_native_identity_copy, has_vectorized_offset_rows, native_byte_layout,
    native_copy_layout, native_expand_contract_for_image, native_expand_contract_for_shape,
    native_filter_byte_layout, native_luma16_transpose_layout, native_typed_filter_layout,
    shape_channels, shape_has_empty_native_bytes, shape_has_nonempty_byte_data,
    shape_has_vector_rows, shape_has_vectorized_luma16_offset, shape_has_vectorized_mirror_rows,
    shape_has_vectorized_native_identity_copy, shape_has_vectorized_offset_rows,
    shape_luma16_copy_layout, shape_luma16_transpose_layout, shape_native_byte_channels,
    shape_native_copy_channels, shape_native_identity_copy_channels,
    shape_native_typed_filter_channels,
};
pub(crate) use self::lut::{native_point_lut, simd_eval, simd_remap_palette};
pub(crate) use self::mandelbrot::simd_mandelbrot_generate;
pub(crate) use self::median_filter::{
    simd_max_filter, simd_median_filter, simd_min_filter, simd_rank_filter,
};
pub(crate) use self::merge::simd_merge;
pub(crate) use self::module_blend::simd_blend_module;
use self::native_arithmetic::{simd_div255, simd_div255_u16x8, simd_pack_u16x16};
pub(crate) use self::native_bytes::simd_rows_mut_serial;
use self::native_bytes::{
    NATIVE_BYTE_ALL_MASK, NATIVE_BYTE_LA_MASK, NATIVE_BYTE_RGBA_MASK, NATIVE_BYTE_ZERO_MASK,
    apply_native_rows, copy_native_bytes, mirror_native_row, native_all_channel_transform_bytes,
    native_byte_transform_bytes, native_byte_transform_copy_bytes, native_copy_image_bytes,
    native_expand_fill_sample, native_fill_row, native_luma16_image_from_bytes,
    record_padded_native_row_work, reverse_pixel_block,
};
use self::neighborhood::{
    SIMD_BOX_BLUR_BIAS, SIMD_BOX_BLUR_SCALE, SIMD_UNIFORM_NEIGHBORHOOD_MAX_PIXELS,
    native_small_uniform_byte_image, native_uniform_byte_image_up_to, native_zero_byte_image,
    simd_blur_load_u8x16, simd_blur_pack_u32x16_to_bytes,
};
pub(crate) use self::offset::simd_offset;
pub(crate) use self::pad::simd_pad;
pub use self::paste::simd_composite_module;
pub(crate) use self::paste::simd_paste;
use self::paste::{
    native_paste_in_place, native_paste_plan_for_image, native_paste_plan_for_shape,
    native_paste_rgb_to_rgba_supported, native_paste_rgb_to_rgba_supported_for_shape,
    native_paste_rgba_to_rgb_supported, native_paste_rgba_to_rgb_supported_for_shape,
};
use self::pipeline::pixel_mode_name;
pub(crate) use self::pipeline::{first_unsupported_simd_op, simd_initial_mode, simd_mode_after_op};
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
use crate::image::preserve_mode;
use crate::ops::pil_resize::{
    compact_resize_coeffs_to_source_span, precompute_coeffs_boxed_for_filter,
};
use crate::pipeline::{PipelineOp, TransposeMethod};
use crate::raster::DynamicImage;
use wide::{f64x8, u8x16, u16x16, u32x16};

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

// ── Native rank-family extrema ─────────────────────────────────────────────

const SIMD_RESIZE_LANES: usize = 8;
const SIMD_RESIZE_NEAREST_BYTES: usize = 16;
// Transpose dispatch is implemented in `adapters/transpose.rs`.

// ═══════════════════════════════════════════════════════════════════════
// Section G: New-buffer ops with PipelineOp dispatch
// ═══════════════════════════════════════════════════════════════════════

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
