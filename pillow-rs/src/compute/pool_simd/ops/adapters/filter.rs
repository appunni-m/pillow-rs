//! SIMD filter-family admission for narrow mode-specific fast paths.

mod kernels;

pub(crate) use self::kernels::simd_unsharp_blend_rgb_150;
use self::kernels::{
    native_filter_3x3_i32_rows, native_filter_3x3_rgb_byte_stream, native_filter_3x3_rows,
};
use super::{
    SIMD_UNIFORM_NEIGHBORHOOD_MAX_PIXELS, SimdImageShape, SimdLayout, copy_native_bytes,
    native_filter_byte_layout, native_small_uniform_byte_image, native_typed_filter_layout,
    native_uniform_byte_image_up_to, shape_channels, shape_native_typed_filter_channels,
    simd_unsupported,
};
const SIMD_UNIFORM_CONVOLUTION_MAX_PIXELS: usize = 256 * 256;

fn valid_convolution_parameters(kernel: &[f32], scale: f32) -> bool {
    scale.is_finite() && scale != 0.0 && kernel.iter().all(|value| value.is_finite())
}

pub(super) fn supports_for_image(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    match op {
        PipelineOp::Filter3x3 { kernel, scale, .. } => {
            valid_convolution_parameters(kernel, *scale)
                && (use_native_byte_convolution_path(img, mode, 1)
                    || (mode == Some("I")
                        && native_typed_filter_layout(img, mode).is_some()
                        && use_native_i32_convolution_path(img, mode, 1))
                    || native_filter_identity_supported_for_image(img, mode, 1))
        }
        PipelineOp::Filter5x5 { kernel, scale, .. } => {
            valid_convolution_parameters(kernel, *scale)
                && (use_native_byte_convolution_path(img, mode, 2)
                    || (mode == Some("I")
                        && native_typed_filter_layout(img, mode).is_some()
                        && use_native_i32_convolution_path(img, mode, 2))
                    || native_filter_identity_supported_for_image(img, mode, 2))
        }
        _ => false,
    }
}

pub(super) fn supports_for_shape(
    shape: SimdImageShape,
    op: &PipelineOp,
    mode: Option<&str>,
) -> bool {
    let native_filter_byte_channels = shape_native_filter_byte_channels(shape, mode);
    match op {
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
        _ => false,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Section D: Filter/window ops (convolution and blur)
// ═══════════════════════════════════════════════════════════════════════

/// The packed convolution adapter is scalar and widens every non-native image
/// to RGBA before evaluating its window. Native byte layouts use the row
/// kernels below, including small images, so SIMD does not pay a conversion
/// boundary or hide a CPU fallback behind an area threshold. The width and
/// height checks ensure that the adapter has at least one complete vector block
/// in the interior; the remaining pixels are handled as scalar tails.
pub(super) fn use_native_byte_convolution_path(
    img: &DynamicImage,
    mode: Option<&str>,
    border: usize,
) -> bool {
    native_filter_byte_layout(img, mode).is_some()
        && img.width() as usize >= border.saturating_mul(2).saturating_add(1)
        && img.height() as usize > border.saturating_mul(2)
}

pub(super) fn use_native_i32_convolution_path(
    img: &DynamicImage,
    mode: Option<&str>,
    border: usize,
) -> bool {
    if mode != Some("I") || !matches!(img, DynamicImage::ImageRgba8(_)) {
        return false;
    }
    let Some(expected_len) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(4))
    else {
        return false;
    };
    img.width() as usize >= border.saturating_mul(2).saturating_add(1)
        && img.height() as usize > border.saturating_mul(2)
        && img.as_bytes().len() == expected_len
}

pub(super) fn native_filter_identity_supported_for_image(
    img: &DynamicImage,
    mode: Option<&str>,
    border: usize,
) -> bool {
    let valid_layout = native_filter_byte_layout(img, mode).is_some()
        || (mode == Some("I") && native_typed_filter_layout(img, mode).is_some());
    let no_interior = img.width() as usize <= border.saturating_mul(2)
        || img.height() as usize <= border.saturating_mul(2);
    let Some(expected_len) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| {
            pixels.checked_mul(if mode == Some("I") {
                4
            } else {
                native_filter_byte_layout(img, mode).unwrap_or(0)
            })
        })
    else {
        return false;
    };
    valid_layout && no_interior && expected_len != 0 && img.as_bytes().len() == expected_len
}

pub(super) fn shape_native_filter_byte_channels(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<usize> {
    let channels = shape_channels(shape)?;
    let valid = match shape.layout {
        SimdLayout::Luma8 => matches!(mode, None | Some("1" | "L")),
        SimdLayout::LumaA8 => matches!(mode, None | Some("LA" | "PA")),
        SimdLayout::Rgb8 => matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")),
        SimdLayout::Rgba8 => {
            matches!(mode, None | Some("RGBA" | "RGBa" | "RGBX" | "CMYK"))
        }
        SimdLayout::Luma16 | SimdLayout::Unsupported => false,
    };
    valid.then_some(channels)
}

pub(super) fn shape_native_i32_convolution_path(
    shape: SimdImageShape,
    mode: Option<&str>,
    border: usize,
) -> bool {
    shape_native_typed_filter_channels(shape, mode).is_some()
        && mode == Some("I")
        && shape.width as usize >= border.saturating_mul(2).saturating_add(1)
        && shape.height as usize > border.saturating_mul(2)
}

pub(super) fn shape_native_filter_identity_supported(
    shape: SimdImageShape,
    mode: Option<&str>,
    border: usize,
) -> bool {
    let valid_layout = shape_native_filter_byte_channels(shape, mode).is_some()
        || (mode == Some("I") && shape_native_typed_filter_channels(shape, mode).is_some());
    let no_interior = shape.width as usize <= border.saturating_mul(2)
        || shape.height as usize <= border.saturating_mul(2);
    valid_layout && no_interior && shape.width != 0 && shape.height != 0
}

/// Return whether a small constant byte image is unchanged by a convolution.
///
/// Filter.c leaves the border bytes untouched and evaluates every interior
/// byte with the same kernel contraction order as the scalar reference. A
/// constant image therefore needs only one exact interior evaluation per
/// channel to prove that the complete output is an identity. Keep the
/// bounded uniform scan shared with rank filters so this fast path cannot
/// turn a large non-uniform frame into an extra full-image pass.
fn native_small_uniform_convolution_identity(
    img: &DynamicImage,
    channels: usize,
    kernel: &[f32],
    row_width: usize,
    rounding_bias: f32,
) -> bool {
    let max_pixels = if row_width == 5 {
        SIMD_UNIFORM_CONVOLUTION_MAX_PIXELS
    } else {
        SIMD_UNIFORM_NEIGHBORHOOD_MAX_PIXELS
    };
    if row_width == 0
        || kernel.len() != row_width.saturating_mul(row_width)
        || !native_uniform_byte_image_up_to(img, channels, max_pixels)
    {
        return false;
    }
    let Some(first_pixel) = img.as_bytes().chunks_exact(channels).next() else {
        return false;
    };
    if (img.width() as usize) < row_width || (img.height() as usize) < row_width {
        return false;
    }
    let radius = row_width / 2;
    for channel in 0..channels {
        let sample = f32::from(first_pixel[channel]);
        let mut total = rounding_bias;
        for row in 0..row_width {
            let row_start = row * row_width;
            let middle = sample * kernel[row_start + radius];
            let mut row_sum = middle;
            for column in 0..radius {
                row_sum = sample.mul_add(kernel[row_start + column], row_sum);
            }
            for column in radius + 1..row_width {
                row_sum = sample.mul_add(kernel[row_start + column], row_sum);
            }
            total += row_sum;
        }
        let output = if total <= 0.0 {
            0
        } else if total >= 255.0 {
            255
        } else {
            total as u8
        };
        if output != first_pixel[channel] {
            return false;
        }
    }
    true
}

/// Return the filtered pixel tuple for a constant image, if the image is
/// small enough to prove uniformity cheaply. A 3x3 convolution then needs one
/// exact evaluation per channel; borders still copy the source tuple.
fn native_small_uniform_3x3_convolution(
    img: &DynamicImage,
    channels: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) -> Option<[u8; 4]> {
    if !(1..=4).contains(&channels)
        || !native_small_uniform_byte_image(img, channels)
        || img.width() < 3
        || img.height() < 3
    {
        return None;
    }
    let first_pixel = img.as_bytes().get(..channels)?;
    let mut filtered = [0u8; 4];
    for channel in 0..channels {
        let sample = f32::from(first_pixel[channel]);
        let mut total = rounding_bias;
        for row in 0..3 {
            let kernel_start = row * 3;
            let mut row_sum = sample * kernel[kernel_start + 1];
            row_sum = sample.mul_add(kernel[kernel_start], row_sum);
            row_sum = sample.mul_add(kernel[kernel_start + 2], row_sum);
            total += row_sum;
        }
        filtered[channel] = if total <= 0.0 {
            0
        } else if total >= 255.0 {
            255
        } else {
            total as u8
        };
    }
    Some(filtered)
}

fn simd_filter_3x3_i32(
    img: &DynamicImage,
    kernel: &[f32; 9],
    scale: f32,
    offset: f32,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if !use_native_i32_convolution_path(img, mode, 1) {
        return Err(simd_unsupported("Filter3x3"));
    }
    let normalized_kernel = std::array::from_fn(|index| kernel[index] / scale);
    let mut output = img.as_bytes().to_vec();
    let (vector_blocks, scalar_tail) = native_filter_3x3_i32_rows(
        img.as_bytes(),
        &mut output,
        img.width() as usize,
        img.height() as usize,
        &normalized_kernel,
        offset + 0.5,
    );
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    let result = crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, 4)?;
    Ok(preserve_mode(img, result))
}

fn simd_filter_identity(
    img: &DynamicImage,
    mode: Option<&str>,
    operation: &str,
) -> Result<DynamicImage, PilError> {
    let channels = if mode == Some("I") {
        4
    } else {
        native_filter_byte_layout(img, mode).ok_or_else(|| simd_unsupported(operation))?
    };
    let mut output = vec![0u8; img.as_bytes().len()];
    let (vector_blocks, scalar_tail) = copy_native_bytes(img.as_bytes(), &mut output)
        .ok_or_else(|| PilError::InternalError("SIMD filter identity buffer mismatch".into()))?;
    crate::compute::record_pipeline_operation_path("native-copy");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    let result =
        crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)?;
    Ok(preserve_mode(img, result))
}

pub(crate) fn simd_filter_3x3(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Filter3x3 {
        kernel,
        scale,
        offset,
    } = op
    else {
        return Err(PilError::ValueError("expected Filter3x3 op".into()));
    };
    if native_filter_identity_supported_for_image(img, mode, 1) {
        return simd_filter_identity(img, mode, "Filter3x3");
    }
    if mode == Some("I") {
        return simd_filter_3x3_i32(img, kernel, *scale, *offset, mode);
    }
    if !use_native_byte_convolution_path(img, mode, 1) {
        return Err(simd_unsupported("Filter3x3"));
    }
    let channels =
        native_filter_byte_layout(img, mode).ok_or_else(|| simd_unsupported("Filter3x3"))?;
    if mode == Some("LA")
        && matches!(img, DynamicImage::ImageLumaA8(_))
        && crate::compute::pool_cpu::ops::filter::binomial3x3_parameters_match(
            kernel, *scale, *offset,
        )
    {
        let mut output = img.as_bytes().to_vec();
        if let Some((vector_blocks, scalar_tail)) = native_filter_3x3_la_binomial_rows(
            img.as_bytes(),
            &mut output,
            img.width() as usize,
            img.height() as usize,
        ) {
            crate::compute::record_pipeline_operation_path("vector");
            crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
            crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
            let result = crate::image_utils::raw_bytes_to_image(
                img.width(),
                img.height(),
                output,
                channels,
            )?;
            return Ok(preserve_mode(img, result));
        }
    }
    let normalized_kernel = std::array::from_fn(|index| kernel[index] / *scale);
    if let Some(filtered) =
        native_small_uniform_3x3_convolution(img, channels, &normalized_kernel, *offset + 0.5)
    {
        let raw = img.as_bytes();
        if raw[..channels] == filtered[..channels] {
            return simd_filter_identity(img, mode, "Filter3x3");
        }
        let mut output = raw.to_vec();
        let width = img.width() as usize;
        let height = img.height() as usize;
        let row_stride = width * channels;
        for y in 1..height - 1 {
            for x in 1..width - 1 {
                let start = y * row_stride + x * channels;
                output[start..start + channels].copy_from_slice(&filtered[..channels]);
            }
        }
        crate::compute::record_pipeline_operation_path("scalar-control");
        let result =
            crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)?;
        return Ok(preserve_mode(img, result));
    }
    let mut output = img.as_bytes().to_vec();
    let rgb_byte_stream_blocks = if channels == 3 {
        native_filter_3x3_rgb_byte_stream(
            img.as_bytes(),
            &mut output,
            img.width() as usize,
            img.height() as usize,
            &normalized_kernel,
            *offset + 0.5,
        )
    } else {
        None
    };
    if let Some(vector_blocks) = rgb_byte_stream_blocks {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(0);
    } else {
        crate::compute::record_pipeline_operation_path("vector");
        native_filter_3x3_rows(
            img.as_bytes(),
            &mut output,
            img.width() as usize,
            img.height() as usize,
            channels,
            &normalized_kernel,
            *offset + 0.5,
        );
    }
    let result =
        crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)?;
    Ok(preserve_mode(img, result))
}

pub(super) use self::kernels::{
    native_filter_3x3_channel_block_integer, native_filter_3x3_luma_block_integer,
    native_filter_3x3_pixel_block_integer, native_filter_3x3_rgb_byte_block_integer,
    native_filter_3x3_vector,
};
pub(crate) use self::kernels::{
    native_filter_3x3_la_binomial_rows, uniform_i32_filter3x3_is_identity_simd,
};

use crate::compute::pool_cpu::ops::filter::uniform5x5_parameters_match;
use crate::compute::{
    record_pipeline_operation_path, record_pipeline_operation_scalar_tail,
    record_pipeline_operation_vector_blocks,
};
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;

#[inline(always)]
fn finish_native_filter5x5(
    img: &DynamicImage,
    output: Vec<u8>,
    channels: usize,
    vector_blocks: u64,
    scalar_tail: u64,
) -> Result<DynamicImage, PilError> {
    record_pipeline_operation_path("vector");
    if vector_blocks != 0 {
        record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if scalar_tail != 0 {
        record_pipeline_operation_scalar_tail(scalar_tail);
    }
    let result =
        crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)?;
    Ok(preserve_mode(img, result))
}

/// Return the unchanged RGB image when the exact normalized 5x5 box filter
/// receives a large constant image. The source's ordered float evaluation
/// preserves every constant byte for this parameter tuple; the vector scan
/// proves that the whole native RGB buffer has one pixel value.
pub(super) fn uniform_rgb_5x5_identity(
    image: &DynamicImage,
    kernel: &[f32; 25],
    scale: f32,
    offset: f32,
    mode: Option<&str>,
) -> Option<DynamicImage> {
    if !matches!(mode, None | Some("RGB"))
        || !matches!(image, DynamicImage::ImageRgb8(_))
        || !uniform5x5_parameters_match(kernel, scale, offset)
    {
        return None;
    }

    let width = image.width() as usize;
    let height = image.height() as usize;
    let pixel_count = width.checked_mul(height)?;
    if pixel_count <= kernels::SMALL_IMAGE_LIMIT_PIXELS || width < 5 || height < 5 {
        return None;
    }

    let evidence = kernels::scan_uniform_rgb(image.as_bytes(), width, height)?;
    if evidence.vector_blocks != 0 {
        record_pipeline_operation_path("vector");
        record_pipeline_operation_vector_blocks(evidence.vector_blocks);
    }
    if evidence.scalar_tail_pixels != 0 {
        record_pipeline_operation_scalar_tail(evidence.scalar_tail_pixels);
    }
    evidence.is_uniform.then(|| image.clone())
}

/// Execute Pillow's RGB 5x5 BLUR ring with vectorized vertical sums and
/// channel-interleaved horizontal taps. Only the exact native kernel enters;
/// smaller images keep the existing lower-overhead route.
#[cfg(not(feature = "parallel"))]
pub(super) fn rgb_blur_ring_5x5(
    image: &DynamicImage,
    kernel: &[f32; 25],
    scale: f32,
    offset: f32,
    mode: Option<&str>,
) -> Option<DynamicImage> {
    if !matches!(mode, None | Some("RGB"))
        || !matches!(image, DynamicImage::ImageRgb8(_))
        || !crate::compute::pool_cpu::ops::filter::rgb_blur_ring5x5_parameters_match(
            kernel, scale, offset,
        )
    {
        return None;
    }

    let width = image.width() as usize;
    let height = image.height() as usize;
    if width.checked_mul(height)? <= kernels::SMALL_IMAGE_LIMIT_PIXELS || width < 5 || height < 5 {
        return None;
    }

    let (output, vector_blocks, scalar_tail) =
        kernels::apply_rgb_blur_ring(image.as_bytes(), width, height)?;
    if vector_blocks != 0 {
        record_pipeline_operation_path("vector");
        record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if scalar_tail != 0 {
        record_pipeline_operation_scalar_tail(scalar_tail);
    }
    let result =
        crate::image_utils::raw_bytes_to_image(image.width(), image.height(), output, 3).ok()?;
    Some(crate::image::preserve_mode(image, result))
}

fn simd_filter_5x5_i32(
    img: &DynamicImage,
    kernel: &[f32; 25],
    scale: f32,
    offset: f32,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if !use_native_i32_convolution_path(img, mode, 2) {
        return Err(super::simd_unsupported("Filter5x5"));
    }
    let normalized_kernel = std::array::from_fn(|index| kernel[index] / scale);
    if crate::compute::pool_cpu::ops::filter::uniform_i32_filter5x5_is_identity(
        img.as_bytes(),
        img.width() as usize,
        img.height() as usize,
        &normalized_kernel,
        offset + 0.5,
    ) {
        crate::compute::record_pipeline_operation_path("scalar-control");
        let output = img.as_bytes().to_vec();
        let result = crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, 4)?;
        return Ok(preserve_mode(img, result));
    }
    let mut output = img.as_bytes().to_vec();
    let (vector_blocks, scalar_tail) = kernels::native_filter_5x5_i32_rows(
        img.as_bytes(),
        &mut output,
        img.width() as usize,
        img.height() as usize,
        &normalized_kernel,
        offset + 0.5,
    )
    .ok_or_else(|| PilError::InternalError("SIMD Filter5x5 I buffer mismatch".into()))?;
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    let result = crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, 4)?;
    Ok(preserve_mode(img, result))
}

fn try_specialized_filter5x5(
    img: &DynamicImage,
    kernel: &[f32; 25],
    scale: f32,
    offset: f32,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    if matches!(mode, None | Some("LA"))
        && matches!(img, DynamicImage::ImageLumaA8(_))
        && crate::compute::pool_cpu::ops::filter::la_single_outlier5x5_parameters_match(
            kernel, scale, offset,
        )
    {
        let mut output = img.as_bytes().to_vec();
        if let Some((vector_blocks, scalar_tail)) =
            kernels::native_filter_5x5_single_outlier_la_rows(
                img.as_bytes(),
                &mut output,
                img.width() as usize,
                img.height() as usize,
            )
        {
            return finish_native_filter5x5(img, output, 2, vector_blocks, scalar_tail).map(Some);
        }
    }
    if matches!(mode, None | Some("L"))
        && matches!(img, DynamicImage::ImageLuma8(_))
        && crate::compute::pool_cpu::ops::filter::uniform5x5_parameters_match(kernel, scale, offset)
    {
        let mut output = img.as_bytes().to_vec();
        if let Some((vector_blocks, scalar_tail)) = kernels::native_filter_5x5_uniform_luma_rows(
            img.as_bytes(),
            &mut output,
            img.width() as usize,
            img.height() as usize,
        ) {
            return finish_native_filter5x5(img, output, 1, vector_blocks, scalar_tail).map(Some);
        }
    }
    if matches!(mode, None | Some("L"))
        && matches!(img, DynamicImage::ImageLuma8(_))
        && crate::compute::pool_cpu::ops::filter::binomial5x5_parameters_match(
            kernel, scale, offset,
        )
    {
        let mut output = img.as_bytes().to_vec();
        if let Some((vector_blocks, scalar_tail)) = kernels::native_filter_5x5_binomial_luma_rows(
            img.as_bytes(),
            &mut output,
            img.width() as usize,
            img.height() as usize,
        ) {
            return finish_native_filter5x5(img, output, 1, vector_blocks, scalar_tail).map(Some);
        }
    }
    if matches!(mode, None | Some("LA"))
        && matches!(img, DynamicImage::ImageLumaA8(_))
        && crate::compute::pool_cpu::ops::filter::binomial5x5_parameters_match(
            kernel, scale, offset,
        )
    {
        let mut output = img.as_bytes().to_vec();
        if let Some((vector_blocks, scalar_tail)) = kernels::native_filter_5x5_binomial_la_rows(
            img.as_bytes(),
            &mut output,
            img.width() as usize,
            img.height() as usize,
        ) {
            return finish_native_filter5x5(img, output, 2, vector_blocks, scalar_tail).map(Some);
        }
    }
    Ok(None)
}

pub(crate) fn simd_filter_5x5(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Filter5x5 {
        kernel,
        scale,
        offset,
    } = op
    else {
        return Err(PilError::ValueError("expected Filter5x5 op".into()));
    };
    if native_filter_identity_supported_for_image(img, mode, 2) {
        return simd_filter_identity(img, mode, "Filter5x5");
    }
    if mode == Some("I") {
        return simd_filter_5x5_i32(img, kernel, *scale, *offset, mode);
    }
    if !use_native_byte_convolution_path(img, mode, 2) {
        return Err(super::simd_unsupported("Filter5x5"));
    }
    let channels = super::native_filter_byte_layout(img, mode)
        .ok_or_else(|| super::simd_unsupported("Filter5x5"))?;
    let normalized_kernel = std::array::from_fn(|index| kernel[index] / *scale);
    if native_small_uniform_convolution_identity(
        img,
        channels,
        &normalized_kernel,
        5,
        *offset + 0.5,
    ) {
        return simd_filter_identity(img, mode, "Filter5x5");
    }
    if let Some(result) = uniform_rgb_5x5_identity(img, kernel, *scale, *offset, mode) {
        return Ok(result);
    }
    #[cfg(not(feature = "parallel"))]
    if let Some(result) = rgb_blur_ring_5x5(img, kernel, *scale, *offset, mode) {
        return Ok(result);
    }
    if let Some(result) = try_specialized_filter5x5(img, kernel, *scale, *offset, mode)? {
        return Ok(result);
    }
    let mut output = img.as_bytes().to_vec();
    crate::compute::record_pipeline_operation_path("vector");
    kernels::native_filter_5x5_rows(
        img.as_bytes(),
        &mut output,
        img.width() as usize,
        img.height() as usize,
        channels,
        &normalized_kernel,
        *offset + 0.5,
    );
    let result =
        crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)?;
    Ok(preserve_mode(img, result))
}

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "parallel"))]
    use super::rgb_blur_ring_5x5;
    use super::uniform_rgb_5x5_identity;
    use crate::compute::pool_cpu::ops::filter::execute_filter5x5;
    use crate::raster::{DynamicImage, RgbImage};

    #[cfg(not(feature = "parallel"))]
    const RGB_BLUR_RING: [f32; 25] = [
        1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0,
        0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
    ];

    #[test]
    fn uniform_rgb_box_fast_path_matches_cpu_reference_for_byte_edges() {
        const WIDTH: usize = 257;
        const HEIGHT: usize = 256;
        let kernel = [1.0f32; 25];

        for value in [0u8, 1, 2, 17, 127, 128, 253, 254, 255] {
            let pixel = [value, 255 - value, value.wrapping_mul(31)];
            let raw = pixel.repeat(WIDTH * HEIGHT);
            let image = DynamicImage::ImageRgb8(
                RgbImage::from_raw(WIDTH as u32, HEIGHT as u32, raw)
                    .expect("valid uniform RGB image"),
            );
            let expected = execute_filter5x5(&image, &kernel, 25.0, 0.0, Some("RGB"))
                .expect("CPU Filter5x5 reference");
            let actual = uniform_rgb_5x5_identity(&image, &kernel, 25.0, 0.0, Some("RGB"))
                .expect("uniform RGB fast path admission");
            assert_eq!(actual.as_bytes(), expected.as_bytes(), "pixel={pixel:?}");
        }
    }

    #[test]
    fn uniform_rgb_box_fast_path_rejects_other_modes_and_parameters() {
        const WIDTH: usize = 257;
        const HEIGHT: usize = 256;
        let raw = [17u8, 83, 149].repeat(WIDTH * HEIGHT);
        let image = DynamicImage::ImageRgb8(
            RgbImage::from_raw(WIDTH as u32, HEIGHT as u32, raw).expect("valid uniform RGB image"),
        );
        let kernel = [1.0f32; 25];
        assert!(uniform_rgb_5x5_identity(&image, &kernel, 25.0, 0.0, Some("L")).is_none());
        assert!(uniform_rgb_5x5_identity(&image, &kernel, 24.0, 0.0, Some("RGB")).is_none());
        assert!(uniform_rgb_5x5_identity(&image, &kernel, 25.0, 1.0, Some("RGB")).is_none());
    }

    #[test]
    #[cfg(not(feature = "parallel"))]
    fn rgb_blur_ring_simd_path_matches_cpu_for_unaligned_interleaved_tails() {
        const WIDTH: usize = 257;
        const HEIGHT: usize = 257;
        let raw = (0..WIDTH * HEIGHT * 3)
            .map(|index| ((index * 73 + index / 7 * 29 + 17) % 256) as u8)
            .collect::<Vec<_>>();
        let image = DynamicImage::ImageRgb8(
            RgbImage::from_raw(WIDTH as u32, HEIGHT as u32, raw).expect("valid RGB image"),
        );
        let expected = execute_filter5x5(&image, &RGB_BLUR_RING, 16.0, 0.0, Some("RGB"))
            .expect("CPU RGB BLUR ring reference");
        let actual = rgb_blur_ring_5x5(&image, &RGB_BLUR_RING, 16.0, 0.0, Some("RGB"))
            .expect("large RGB BLUR ring SIMD admission");
        assert_eq!(actual.as_bytes(), expected.as_bytes());
    }

    #[test]
    #[cfg(not(feature = "parallel"))]
    fn rgb_blur_ring_simd_path_declines_small_or_nonmatching_inputs() {
        let raw = [17u8, 83, 149].repeat(256 * 256);
        let image =
            DynamicImage::ImageRgb8(RgbImage::from_raw(256, 256, raw).expect("valid RGB image"));
        assert!(rgb_blur_ring_5x5(&image, &RGB_BLUR_RING, 16.0, 0.0, Some("RGB")).is_none());
        assert!(rgb_blur_ring_5x5(&image, &RGB_BLUR_RING, 15.0, 0.0, Some("RGB")).is_none());
        assert!(rgb_blur_ring_5x5(&image, &RGB_BLUR_RING, 16.0, 0.0, Some("L")).is_none());
    }
}
