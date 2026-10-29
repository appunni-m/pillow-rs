//! CPU implementation for Pillow's 5x5 convolution filter.

use super::uniform_image::{SMALL_UNIFORM_BYTE_MAX_PIXELS, native_small_uniform_byte_image};
use crate::compute::{Backend, ExecutionContext};
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::image_utils::raw_bytes_to_image;
use crate::raster::DynamicImage;

#[path = "filter5x5/int32.rs"]
mod int32;
#[path = "filter5x5/native_bytes.rs"]
mod native_bytes;

#[path = "rgb_blur_ring.rs"]
mod rgb_blur_ring;

#[path = "uniform_box.rs"]
mod uniform_box;
#[cfg(test)]
#[path = "uniform_box_tests.rs"]
mod uniform_box_tests;

pub(crate) use int32::uniform_i32_filter5x5_is_identity;
#[cfg(test)]
pub(crate) use native_bytes::filter_5x5_byte_row;
pub(crate) use native_bytes::la_single_outlier5x5_parameters_match;

pub(crate) use native_bytes::rgb_blur_ring5x5_parameters_match;
pub(crate) use native_bytes::{binomial5x5_parameters_match, uniform5x5_parameters_match};

use native_bytes::{
    filter_5x5_binomial_la_rows, filter_5x5_binomial_luma_rows, filter_5x5_la_single_outlier_rows,
    filter_5x5_uniform_rgba_identity, filter_5x5_uniform_rgba_rows,
};
use native_bytes::{filter_5x5_byte_rows, filter_5x5_constant_bytes, filter_5x5_uniform_luma_rows};

struct Filter5x5Context<'a> {
    img: &'a DynamicImage,
    raw: &'a [u8],
    kernel: &'a [f32; 25],
    scale: f32,
    offset: f32,
    explicit_mode: Option<&'a str>,
    width: u32,
    height: u32,
    channels: usize,
    normalized_kernel: [f32; 25],
    rounding_bias: f32,
}

#[inline(always)]
fn finish_filter5x5(
    context: &Filter5x5Context<'_>,
    output: Vec<u8>,
) -> Result<DynamicImage, PilError> {
    let result = raw_bytes_to_image(context.width, context.height, output, context.channels)?;
    Ok(preserve_mode(context.img, result))
}

#[inline(always)]
fn try_filter5x5_before_output_allocation(
    context: &Filter5x5Context<'_>,
) -> Result<Option<DynamicImage>, PilError> {
    if matches!(context.img, DynamicImage::ImageRgb8(_))
        && matches!(context.explicit_mode, None | Some("RGB"))
        && rgb_blur_ring::parameters_match(context.kernel, context.scale, context.offset)
        && let Some(output) =
            rgb_blur_ring::apply(context.raw, context.width as usize, context.height as usize)
    {
        return Ok(Some(finish_filter5x5(context, output)?));
    }

    if matches!(context.img, DynamicImage::ImageRgba8(_))
        && matches!(context.explicit_mode, None | Some("RGBA"))
        && uniform5x5_parameters_match(context.kernel, context.scale, context.offset)
        && filter_5x5_uniform_rgba_identity(
            context.raw,
            context.width as usize,
            context.height as usize,
            &context.normalized_kernel,
            context.rounding_bias,
        )
    {
        return Ok(Some(finish_filter5x5(context, context.raw.to_vec())?));
    }
    Ok(None)
}

#[inline(always)]
fn try_filter5x5_native_byte_rows(
    context: &Filter5x5Context<'_>,
    output: &mut Vec<u8>,
) -> Result<Option<DynamicImage>, PilError> {
    if matches!(context.explicit_mode, None | Some("LA"))
        && matches!(context.img, DynamicImage::ImageLumaA8(_))
        && la_single_outlier5x5_parameters_match(context.kernel, context.scale, context.offset)
        && filter_5x5_la_single_outlier_rows(
            context.raw,
            output,
            context.width as usize,
            context.height as usize,
        )
    {
        return Ok(Some(finish_filter5x5(context, std::mem::take(output))?));
    }

    if matches!(context.img, DynamicImage::ImageRgba8(_))
        && matches!(context.explicit_mode, None | Some("RGBA"))
        && uniform5x5_parameters_match(context.kernel, context.scale, context.offset)
        && filter_5x5_uniform_rgba_rows(
            context.raw,
            output,
            context.width as usize,
            context.height as usize,
        )
    {
        return Ok(Some(finish_filter5x5(context, std::mem::take(output))?));
    }
    let native_byte_layout = matches!(
        context.img,
        DynamicImage::ImageLuma8(_)
            | DynamicImage::ImageLumaA8(_)
            | DynamicImage::ImageRgb8(_)
            | DynamicImage::ImageRgba8(_)
    );
    if native_byte_layout
        && native_small_uniform_byte_image(
            context.img,
            context.channels,
            SMALL_UNIFORM_BYTE_MAX_PIXELS,
        )
    {
        filter_5x5_constant_bytes(
            context.raw,
            output,
            context.width as usize,
            context.height as usize,
            context.channels,
            &context.normalized_kernel,
            context.rounding_bias,
        );
        return Ok(Some(finish_filter5x5(context, std::mem::take(output))?));
    }
    Ok(None)
}

#[inline(always)]
fn try_filter5x5_luma_byte_rows(
    context: &Filter5x5Context<'_>,
    output: &mut Vec<u8>,
) -> Result<Option<DynamicImage>, PilError> {
    if matches!(context.explicit_mode, None | Some("L"))
        && matches!(context.img, DynamicImage::ImageLuma8(_))
        && uniform5x5_parameters_match(context.kernel, context.scale, context.offset)
        && filter_5x5_uniform_luma_rows(
            context.raw,
            output,
            context.width as usize,
            context.height as usize,
        )
    {
        return Ok(Some(finish_filter5x5(context, std::mem::take(output))?));
    }

    if matches!(context.explicit_mode, None | Some("L"))
        && matches!(context.img, DynamicImage::ImageLuma8(_))
        && binomial5x5_parameters_match(context.kernel, context.scale, context.offset)
        && filter_5x5_binomial_luma_rows(
            context.raw,
            output,
            context.width as usize,
            context.height as usize,
        )
    {
        return Ok(Some(finish_filter5x5(context, std::mem::take(output))?));
    }
    Ok(None)
}

#[inline(always)]
fn try_filter5x5_luma_rows(
    context: &Filter5x5Context<'_>,
    output: &mut Vec<u8>,
) -> Result<Option<DynamicImage>, PilError> {
    if let Some(result) = try_filter5x5_luma_byte_rows(context, output)? {
        return Ok(Some(result));
    }

    #[cfg(target_arch = "aarch64")]
    #[cfg(target_feature = "neon")]
    if matches!(context.img, DynamicImage::ImageLumaA8(_))
        && context.width >= 13
        && context.height >= 5
        && binomial5x5_parameters_match(context.kernel, context.scale, context.offset)
    {
        // Reuse the exact packed-LA vector kernel for serial CPU on AArch64.
        let mut vector_output = context.raw.to_vec();
        if let Some((vector_blocks, scalar_tail)) =
            crate::compute::pool_simd::ops::native_filter::native_filter_5x5_binomial_la_rows(
                context.raw,
                &mut vector_output,
                context.width as usize,
                context.height as usize,
            )
        {
            crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
            crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
            return Ok(Some(finish_filter5x5(context, vector_output)?));
        }
    }

    if matches!(context.explicit_mode, None | Some("LA"))
        && matches!(context.img, DynamicImage::ImageLumaA8(_))
        && binomial5x5_parameters_match(context.kernel, context.scale, context.offset)
        && filter_5x5_binomial_la_rows(
            context.raw,
            output,
            context.width as usize,
            context.height as usize,
        )
    {
        return Ok(Some(finish_filter5x5(context, std::mem::take(output))?));
    }
    Ok(None)
}

/// Execute a 5x5 convolution filter.
pub fn execute_filter5x5(
    img: &DynamicImage,
    kernel: &[f32; 25],
    scale: f32,
    offset: f32,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    execute_filter5x5_in_context(
        &ExecutionContext::for_backend(Backend::Cpu),
        img,
        kernel,
        scale,
        offset,
        explicit_mode,
    )
}

pub fn execute_filter5x5_in_context(
    execution_context: &ExecutionContext,
    img: &DynamicImage,
    kernel: &[f32; 25],
    scale: f32,
    offset: f32,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // I-mode: operate directly on int32 pixel values (no [0,255] clipping)
    if explicit_mode == Some("I") {
        return int32::filter_5x5_i32_in_context(execution_context, img, kernel, scale, offset);
    }
    // Normalize once, outside the pixel loop. The row helper keeps all five
    // tap groups in the same order as the original scalar implementation.
    let context = Filter5x5Context {
        img,
        raw: img.as_bytes(),
        kernel,
        scale,
        offset,
        explicit_mode,
        width: img.width(),
        height: img.height(),
        channels: img.color().channel_count() as usize,
        normalized_kernel: std::array::from_fn(|index| kernel[index] / scale),
        rounding_bias: offset + 0.5,
    };
    if let Some(output) = try_filter5x5_before_output_allocation(&context)? {
        return Ok(output);
    }
    let mut output = context.raw.to_vec();
    if let Some(result) = try_filter5x5_native_byte_rows(&context, &mut output)? {
        return Ok(result);
    }
    if let Some(result) = try_filter5x5_luma_rows(&context, &mut output)? {
        return Ok(result);
    }
    filter_5x5_byte_rows(
        context.raw,
        &mut output,
        context.width as i32,
        context.height as i32,
        context.channels,
        &context.normalized_kernel,
        context.rounding_bias,
    );
    finish_filter5x5(&context, output)
}
