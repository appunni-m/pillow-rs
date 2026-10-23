//! SIMD Pad adapter and native-layout resize/fill orchestration.

use super::resize::{
    native_pad_channels_for_image, native_pad_channels_for_shape, native_pad_contained_dimensions,
};
use super::{
    SimdImageShape, copy_native_bytes, native_expand_fill_sample, native_fill_row,
    simd_resize_convolution, simd_resize_convolution_into, simd_resize_f, simd_resize_i32,
    simd_resize_nearest, simd_unsupported,
};
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::pipeline::{PipelineOp, ResampleFilter};
use crate::raster::DynamicImage;

fn native_pad_offsets(
    contained_width: u32,
    contained_height: u32,
    target_width: u32,
    target_height: u32,
    centering: (f64, f64),
) -> Option<(usize, usize)> {
    if !centering.0.is_finite() || !centering.1.is_finite() {
        return None;
    }
    let width_gap = target_width.checked_sub(contained_width)?;
    let height_gap = target_height.checked_sub(contained_height)?;
    if width_gap != 0 {
        Some((
            usize::try_from(super::resize::native_pad_round_dimension(
                f64::from(width_gap) * centering.0.clamp(0.0, 1.0),
            )?)
            .ok()?,
            0,
        ))
    } else {
        Some((
            0,
            usize::try_from(super::resize::native_pad_round_dimension(
                f64::from(height_gap) * centering.1.clamp(0.0, 1.0),
            )?)
            .ok()?,
        ))
    }
}

/// Pillow's indexed `P` pad path uses nearest-neighbour sampling regardless
/// of the requested filter. `PA` is different: its index and alpha bytes are
/// filtered independently, so it retains the requested kernel.
fn native_pad_filter(mode: Option<&str>, filter: ResampleFilter) -> ResampleFilter {
    if mode == Some("P") {
        ResampleFilter::Nearest
    } else {
        filter
    }
}

fn native_pad_supported_for_image(
    img: &DynamicImage,
    target_width: u32,
    target_height: u32,
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    let Some(channels) = native_pad_channels_for_image(img, mode) else {
        return false;
    };
    let Some(expected_bytes) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return false;
    };
    if img.as_bytes().len() != expected_bytes {
        return false;
    }
    let filter = native_pad_filter(mode, filter);
    // A zero-width/zero-height source still has a valid Pillow pad contract:
    // the output is a filled canvas. There is no aspect-ratio geometry to
    // compute, so admission is based on the vector fill data plane directly.
    if img.width() == 0 || img.height() == 0 {
        return target_width != 0
            && target_height != 0
            && super::resize::native_resize_supported_for_dimensions(
                img.width(),
                img.height(),
                target_width,
                target_height,
                filter,
                channels,
            );
    }
    let Some((contained_width, contained_height)) = super::resize::native_pad_contained_dimensions(
        img.width(),
        img.height(),
        target_width,
        target_height,
    ) else {
        return false;
    };
    if native_pad_offsets(
        contained_width,
        contained_height,
        target_width,
        target_height,
        (0.5, 0.5),
    )
    .is_none()
    {
        return false;
    }
    (img.width(), img.height()) == (contained_width, contained_height)
        || super::resize::native_resize_supported_for_dimensions(
            img.width(),
            img.height(),
            contained_width,
            contained_height,
            filter,
            channels,
        )
}

fn native_pad_supported_for_shape(
    shape: SimdImageShape,
    target_width: u32,
    target_height: u32,
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    let Some(channels) = native_pad_channels_for_shape(shape, mode) else {
        return false;
    };
    let filter = native_pad_filter(mode, filter);
    if shape.width == 0 || shape.height == 0 {
        return target_width != 0
            && target_height != 0
            && super::resize::native_resize_supported_for_dimensions(
                shape.width,
                shape.height,
                target_width,
                target_height,
                filter,
                channels,
            );
    }
    let Some((contained_width, contained_height)) = super::resize::native_pad_contained_dimensions(
        shape.width,
        shape.height,
        target_width,
        target_height,
    ) else {
        return false;
    };
    native_pad_offsets(
        contained_width,
        contained_height,
        target_width,
        target_height,
        (0.5, 0.5),
    )
    .is_some()
        && ((shape.width, shape.height) == (contained_width, contained_height)
            || super::resize::native_resize_supported_for_dimensions(
                shape.width,
                shape.height,
                contained_width,
                contained_height,
                filter,
                channels,
            ))
}

pub(super) fn supports_for_image(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    let PipelineOp::Pad { w, h, filter, .. } = op else {
        return false;
    };
    native_pad_supported_for_image(img, *w, *h, *filter, mode)
}

pub(super) fn supports_for_shape(
    shape: SimdImageShape,
    op: &PipelineOp,
    mode: Option<&str>,
) -> bool {
    let PipelineOp::Pad { w, h, filter, .. } = op else {
        return false;
    };
    native_pad_supported_for_shape(shape, *w, *h, *filter, mode)
}

#[derive(Clone, Copy)]
struct NativePadCanvas {
    contained_width: u32,
    contained_height: u32,
    offset_x: usize,
    offset_y: usize,
    source_width: usize,
    source_height: usize,
    target_width: u32,
    target_height: u32,
    target_width_usize: usize,
    target_height_usize: usize,
    channels: usize,
    source_stride: usize,
    target_stride: usize,
    output_len: usize,
}

fn native_pad_empty_source(
    img: &DynamicImage,
    target_width: u32,
    target_height: u32,
    channels: usize,
    color: Option<(u8, u8, u8, u8)>,
) -> Result<Option<(DynamicImage, u64, u64)>, PilError> {
    // Pillow returns a filled canvas when a source dimension is zero. Keep
    // this native byte path available for raw and color-space modes too.
    let target_width_usize = usize::try_from(target_width).map_err(|_| simd_unsupported("Pad"))?;
    let target_height_usize =
        usize::try_from(target_height).map_err(|_| simd_unsupported("Pad"))?;
    let target_stride = target_width_usize
        .checked_mul(channels)
        .ok_or_else(|| simd_unsupported("Pad"))?;
    let output_len = target_stride
        .checked_mul(target_height_usize)
        .ok_or_else(|| simd_unsupported("Pad"))?;
    let default_fill = if channels == 2 || channels == 4 {
        (0, 0, 0, 0)
    } else {
        (0, 0, 0, u8::MAX)
    };
    let fill = color.unwrap_or(default_fill);
    let mut output = vec![0u8; output_len];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for row in output.chunks_exact_mut(target_stride) {
        let (blocks, tail) = native_fill_row(row, fill, channels)
            .ok_or_else(|| PilError::InternalError("SIMD pad fill shape mismatch".into()))?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    let result =
        crate::image_utils::raw_bytes_to_image(target_width, target_height, output, channels)?;
    Ok(Some((
        preserve_mode(img, result),
        vector_blocks,
        scalar_tail,
    )))
}

fn native_pad_vertical_identity(
    img: &DynamicImage,
    canvas: NativePadCanvas,
    fill: (u8, u8, u8, u8),
    mode: Option<&str>,
) -> Result<Option<(DynamicImage, u64, u64)>, PilError> {
    // For a vertical-only pad with an unchanged contain size, reuse one fill
    // row and append the original bytes as a contiguous span.
    let fill_pixel = (0..canvas.channels)
        .map(|channel| native_expand_fill_sample(fill, canvas.channels, channel))
        .collect::<Vec<_>>();
    let source = img.as_bytes();
    let expected_source = canvas
        .source_stride
        .checked_mul(canvas.source_height)
        .ok_or_else(|| simd_unsupported("Pad"))?;
    if source.len() != expected_source {
        return Ok(None);
    }
    let source_end = canvas
        .offset_y
        .checked_add(canvas.source_height)
        .filter(|end| *end <= canvas.target_height_usize)
        .ok_or_else(|| simd_unsupported("Pad"))?;
    let needs_fill_rows = canvas.offset_y != 0 || source_end != canvas.target_height_usize;
    let (fill_row, vector_blocks, scalar_tail) = if mode == Some("RGBA") && needs_fill_rows {
        let mut row = vec![0u8; canvas.target_stride];
        let (vector_blocks, scalar_tail) = native_fill_row(&mut row, fill, canvas.channels)
            .ok_or_else(|| PilError::InternalError("SIMD RGBA Pad fill shape mismatch".into()))?;
        (row, vector_blocks, scalar_tail)
    } else {
        (fill_pixel.repeat(canvas.target_width_usize), 0, 0)
    };
    if fill_row.len() != canvas.target_stride {
        return Ok(None);
    }
    let append_vertical_rows = matches!(mode, Some("RGB" | "RGBA" | "CMYK" | "RGBX"))
        || (canvas.channels == 1 && matches!(mode, None | Some("L")));
    let mut output = if append_vertical_rows {
        Vec::with_capacity(canvas.output_len)
    } else {
        fill_row.repeat(canvas.target_height_usize)
    };
    if append_vertical_rows {
        for _ in 0..canvas.offset_y {
            output.extend_from_slice(&fill_row);
        }
        output.extend_from_slice(source);
        for _ in source_end..canvas.target_height_usize {
            output.extend_from_slice(&fill_row);
        }
        if output.len() != canvas.output_len {
            return Ok(None);
        }
    } else {
        let destination_start = canvas
            .offset_y
            .checked_mul(canvas.target_stride)
            .ok_or_else(|| simd_unsupported("Pad"))?;
        let destination_end = destination_start
            .checked_add(source.len())
            .filter(|end| *end <= output.len())
            .ok_or_else(|| simd_unsupported("Pad"))?;
        output[destination_start..destination_end].copy_from_slice(source);
    }
    let result = crate::image_utils::raw_bytes_to_image(
        canvas.target_width,
        canvas.target_height,
        output,
        canvas.channels,
    )?;
    Ok(Some((
        preserve_mode(img, result),
        vector_blocks,
        scalar_tail,
    )))
}

fn native_pad_full_width_convolution(
    img: &DynamicImage,
    canvas: NativePadCanvas,
    filter: ResampleFilter,
    fill: (u8, u8, u8, u8),
    luma_byte_i32: bool,
) -> Result<Option<(DynamicImage, u64, u64)>, PilError> {
    // Resize directly into the final canvas's full-width content window.
    let fill_pixel = (0..canvas.channels)
        .map(|channel| native_expand_fill_sample(fill, canvas.channels, channel))
        .collect::<Vec<_>>();
    let fill_row = fill_pixel.repeat(canvas.target_width_usize);
    if fill_row.len() != canvas.target_stride {
        return Ok(None);
    }
    let mut output = fill_row.repeat(canvas.target_height_usize);
    let content_len = canvas
        .source_stride
        .checked_mul(canvas.source_height)
        .ok_or_else(|| simd_unsupported("Pad"))?;
    let window_start = canvas
        .offset_y
        .checked_mul(canvas.target_stride)
        .ok_or_else(|| simd_unsupported("Pad"))?;
    let window_end = window_start
        .checked_add(content_len)
        .filter(|end| *end <= output.len())
        .ok_or_else(|| simd_unsupported("Pad"))?;
    simd_resize_convolution_into(
        img,
        canvas.contained_width,
        canvas.contained_height,
        filter,
        canvas.channels,
        false,
        luma_byte_i32,
        0,
        false,
        &mut output[window_start..window_end],
    )?;
    let result = crate::image_utils::raw_bytes_to_image(
        canvas.target_width,
        canvas.target_height,
        output,
        canvas.channels,
    )?;
    Ok(Some((preserve_mode(img, result), 0, 0)))
}

fn native_pad_resized_source(
    img: &DynamicImage,
    canvas: NativePadCanvas,
    filter: ResampleFilter,
    mode: Option<&str>,
    luma_byte_i32: bool,
) -> Result<Option<DynamicImage>, PilError> {
    if (img.width(), img.height()) == (canvas.contained_width, canvas.contained_height) {
        return Ok(None);
    }
    if mode == Some("F") {
        return Ok(Some(simd_resize_f(
            img,
            canvas.contained_width,
            canvas.contained_height,
            &filter,
        )?));
    }
    if mode == Some("I") {
        return Ok(Some(simd_resize_i32(
            img,
            canvas.contained_width,
            canvas.contained_height,
            &filter,
        )?));
    }
    let resized = match filter {
        ResampleFilter::Nearest => simd_resize_nearest(
            img,
            canvas.contained_width,
            canvas.contained_height,
            canvas.channels,
        )?,
        _ => simd_resize_convolution(
            img,
            canvas.contained_width,
            canvas.contained_height,
            filter,
            canvas.channels,
            match canvas.channels {
                2 => matches!(mode, None | Some("LA")),
                4 => matches!(mode, None | Some("RGBA")),
                _ => false,
            },
            luma_byte_i32,
        )?,
    };
    Ok(Some(resized))
}

fn native_pad_compose_canvas(
    img: &DynamicImage,
    source: &[u8],
    canvas: NativePadCanvas,
    fill: (u8, u8, u8, u8),
) -> Result<Option<(DynamicImage, u64, u64)>, PilError> {
    if source.len() != canvas.source_stride.saturating_mul(canvas.source_height)
        || canvas.offset_x.saturating_add(canvas.source_width) > canvas.target_width_usize
        || canvas.offset_y.saturating_add(canvas.source_height) > canvas.target_height_usize
    {
        return Ok(None);
    }
    let mut output = vec![0u8; canvas.output_len];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    let fill_is_zero = (0..canvas.channels)
        .all(|sample| native_expand_fill_sample(fill, canvas.channels, sample) == 0);
    let content_start = canvas
        .offset_x
        .checked_mul(canvas.channels)
        .ok_or_else(|| simd_unsupported("Pad"))?;
    let content_end = content_start
        .checked_add(canvas.source_stride)
        .ok_or_else(|| simd_unsupported("Pad"))?;
    for (output_y, row) in output.chunks_exact_mut(canvas.target_stride).enumerate() {
        if output_y < canvas.offset_y || output_y - canvas.offset_y >= canvas.source_height {
            if !fill_is_zero {
                let (blocks, tail) =
                    native_fill_row(row, fill, canvas.channels).ok_or_else(|| {
                        PilError::InternalError("SIMD pad fill shape mismatch".into())
                    })?;
                vector_blocks = vector_blocks.saturating_add(blocks);
                scalar_tail = scalar_tail.saturating_add(tail);
            }
            continue;
        }

        let source_y = output_y - canvas.offset_y;
        if !fill_is_zero {
            let (blocks, tail) = native_fill_row(&mut row[..content_start], fill, canvas.channels)
                .ok_or_else(|| PilError::InternalError("SIMD pad fill shape mismatch".into()))?;
            vector_blocks = vector_blocks.saturating_add(blocks);
            scalar_tail = scalar_tail.saturating_add(tail);
        }
        let source_start = source_y
            .checked_mul(canvas.source_stride)
            .ok_or_else(|| simd_unsupported("Pad"))?;
        let (blocks, tail) = copy_native_bytes(
            &source[source_start..source_start + canvas.source_stride],
            &mut row[content_start..content_end],
        )
        .ok_or_else(|| PilError::InternalError("SIMD pad copy shape mismatch".into()))?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
        if !fill_is_zero {
            let (blocks, tail) = native_fill_row(&mut row[content_end..], fill, canvas.channels)
                .ok_or_else(|| PilError::InternalError("SIMD pad fill shape mismatch".into()))?;
            vector_blocks = vector_blocks.saturating_add(blocks);
            scalar_tail = scalar_tail.saturating_add(tail);
        }
    }
    let result = crate::image_utils::raw_bytes_to_image(
        canvas.target_width,
        canvas.target_height,
        output,
        canvas.channels,
    )?;
    Ok(Some((
        preserve_mode(img, result),
        vector_blocks,
        scalar_tail,
    )))
}

fn native_pad_bytes(
    img: &DynamicImage,
    target_width: u32,
    target_height: u32,
    filter: ResampleFilter,
    color: Option<(u8, u8, u8, u8)>,
    centering: (f64, f64),
    mode: Option<&str>,
) -> Result<Option<(DynamicImage, u64, u64)>, PilError> {
    let Some(channels) = native_pad_channels_for_image(img, mode) else {
        return Ok(None);
    };
    let filter = native_pad_filter(mode, filter);
    if !native_pad_supported_for_image(img, target_width, target_height, filter, mode) {
        return Ok(None);
    }
    if img.width() == 0 || img.height() == 0 {
        return native_pad_empty_source(img, target_width, target_height, channels, color);
    }
    let Some((contained_width, contained_height)) =
        native_pad_contained_dimensions(img.width(), img.height(), target_width, target_height)
    else {
        return Ok(None);
    };
    let Some((offset_x, offset_y)) = native_pad_offsets(
        contained_width,
        contained_height,
        target_width,
        target_height,
        centering,
    ) else {
        return Ok(None);
    };
    let source_width = usize::try_from(contained_width).map_err(|_| simd_unsupported("Pad"))?;
    let source_height = usize::try_from(contained_height).map_err(|_| simd_unsupported("Pad"))?;
    let target_width_usize = usize::try_from(target_width).map_err(|_| simd_unsupported("Pad"))?;
    let target_height_usize =
        usize::try_from(target_height).map_err(|_| simd_unsupported("Pad"))?;
    let source_stride = source_width
        .checked_mul(channels)
        .ok_or_else(|| simd_unsupported("Pad"))?;
    let target_stride = target_width_usize
        .checked_mul(channels)
        .ok_or_else(|| simd_unsupported("Pad"))?;
    let output_len = target_stride
        .checked_mul(target_height_usize)
        .ok_or_else(|| simd_unsupported("Pad"))?;
    let canvas = NativePadCanvas {
        contained_width,
        contained_height,
        offset_x,
        offset_y,
        source_width,
        source_height,
        target_width,
        target_height,
        target_width_usize,
        target_height_usize,
        channels,
        source_stride,
        target_stride,
        output_len,
    };
    let default_fill = if channels == 2 || channels == 4 {
        (0, 0, 0, 0)
    } else {
        (0, 0, 0, u8::MAX)
    };
    let fill = color.unwrap_or(default_fill);

    if (img.width(), img.height()) == (contained_width, contained_height)
        && offset_x == 0
        && source_width == target_width_usize
    {
        return native_pad_vertical_identity(img, canvas, fill, mode);
    }

    let luma_byte_i32 = matches!(filter, ResampleFilter::Bicubic)
        && matches!(mode, None | Some("L"))
        && matches!(img, DynamicImage::ImageLuma8(_));
    if (mode == Some("RGBX")
        || matches!(mode, None | Some("L")) && matches!(img, DynamicImage::ImageLuma8(_)))
        && !matches!(filter, ResampleFilter::Nearest)
        && (img.width(), img.height()) != (contained_width, contained_height)
        && offset_x == 0
        && source_width == target_width_usize
    {
        return native_pad_full_width_convolution(img, canvas, filter, fill, luma_byte_i32);
    }

    // Identity contain copies the source directly; equal-sized convolution
    // resampling would change edge pixels from Pillow's behavior.
    let resized = native_pad_resized_source(img, canvas, filter, mode, luma_byte_i32)?;
    let source = resized
        .as_ref()
        .map_or_else(|| img.as_bytes(), DynamicImage::as_bytes);
    native_pad_compose_canvas(img, source, canvas, fill)
}

/// Execute `ImageOps.pad` with a scalar contain/centering control plane and
/// native-layout SIMD resize, fill, and row-copy data planes. Explicit SIMD
/// never retries the CPU pad implementation when this contract is not met.
pub(crate) fn simd_pad(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Pad {
        w,
        h,
        filter,
        color,
        centering,
    } = op
    else {
        return Err(PilError::ValueError("expected Pad op".into()));
    };
    let Some((result, vector_blocks, scalar_tail)) =
        native_pad_bytes(img, *w, *h, *filter, *color, *centering, mode)?
    else {
        return Err(simd_unsupported("Pad"));
    };
    if vector_blocks != 0 || scalar_tail != 0 {
        crate::compute::record_pipeline_operation_path("vector");
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
