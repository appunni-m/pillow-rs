use super::super::super::{simd_div255, simd_pack_u16x16};
use super::super::{has_visible_draw_point, has_visible_draw_rectangle, native_draw_layout};
use super::SimdDrawSpanWriter;
use crate::error::PilError;
use crate::raster::{DynamicImage, GenericImageView};
use wide::{u8x16, u16x16};

mod line;
pub(in crate::compute::pool_simd::ops::adapters::draw) use line::simd_draw_line_native;

/// Return the inclusive pixel span of a horizontal line after clipping it to
/// the destination.  Geometry is deliberately kept scalar: the SIMD kernel
/// below only owns the contiguous native-byte span once this control-plane
/// check has succeeded.
fn clipped_horizontal_span(
    width: u32,
    height: u32,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
) -> Option<(usize, usize, usize)> {
    if width == 0 || height == 0 || y0 != y1 || y0 < 0 || y0 >= height as i32 {
        return None;
    }
    let last_x = i64::from(width) - 1;
    let start = i64::from(x0).min(i64::from(x1)).max(0);
    let end = i64::from(x0).max(i64::from(x1)).min(last_x);
    (start <= end).then_some((start as usize, end as usize, y0 as usize))
}

fn draw_line_source_byte(fill: (u8, u8, u8, u8), channels: usize, byte: usize) -> u8 {
    match channels {
        1 => fill.0,
        2 => match byte % 2 {
            0 => fill.0,
            _ => fill.3,
        },
        3 => match byte % 3 {
            0 => fill.0,
            1 => fill.1,
            _ => fill.2,
        },
        4 => match byte % 4 {
            0 => fill.0,
            1 => fill.1,
            2 => fill.2,
            _ => fill.3,
        },
        _ => 0,
    }
}

#[inline]
fn draw_line_blend_byte(source: u8, destination: u8, alpha: u8) -> u8 {
    let weighted =
        u16::from(source) * u16::from(alpha) + u16::from(destination) * u16::from(u8::MAX - alpha);
    ((u32::from(weighted) + 127) / 255) as u8
}

/// Apply one masked native-byte vector block for a horizontal line.
///
/// The mask makes the short-span case vectorized too: a 13-pixel L line in a
/// 16-byte row still executes one vector load/store while preserving the
/// neighbouring pixels.  RGB alpha blending uses the same exact rounded
/// formula as `NativeDrawCanvas`; the 16 lanes are byte channels, not packed
/// pixels, so a three-byte layout needs no RGBA widening.
#[inline]
pub(in crate::compute::pool_simd::ops::adapters) fn draw_line_vector_block(
    row: &mut [u8],
    block_start: usize,
    target_start: usize,
    target_end: usize,
    channels: usize,
    fill: (u8, u8, u8, u8),
    alpha_blend_rgb: bool,
) {
    let input = <[u8; 16]>::try_from(&row[block_start..block_start + 16])
        .expect("draw line vector block has 16 bytes");
    let source = u8x16::new(std::array::from_fn(|lane| {
        draw_line_source_byte(fill, channels, block_start + lane)
    }));
    let transformed = if channels == 3 && alpha_blend_rgb {
        let destination = u16x16::from(u8x16::new(input));
        let source = u16x16::from(source);
        let alpha = u16x16::splat(u16::from(fill.3));
        let inverse_alpha = u16x16::splat(u16::from(u8::MAX - fill.3));
        let weighted = source * alpha + destination * inverse_alpha;
        simd_pack_u16x16(simd_div255(weighted + u16x16::splat(127)))
    } else {
        source
    };
    let mask = u8x16::new(std::array::from_fn(|lane| {
        let position = block_start + lane;
        if (target_start..target_end).contains(&position) {
            u8::MAX
        } else {
            0
        }
    }));
    let input = u8x16::new(input);
    let output = (transformed & mask) | (input & (u8x16::splat(u8::MAX) ^ mask));
    row[block_start..block_start + 16].copy_from_slice(&output.to_array());
}

/// Fill one clipped horizontal span in a native drawing buffer.
///
/// The span bounds are scalar geometry output. Every complete sixteen-byte
/// block, including a masked block for a short span, is written by the same
/// native-byte vector kernel used by the line fast path. Only a trailing
/// partial block after a completed vector block is scalar.
pub(super) fn simd_draw_span(
    output: &mut [u8],
    image_width: u32,
    image_height: u32,
    x0: i64,
    x1: i64,
    y: i64,
    channels: usize,
    fill: (u8, u8, u8, u8),
    alpha_blend_rgb: bool,
) -> Result<(u64, u64), PilError> {
    if y < 0
        || y >= i64::from(image_height)
        || image_width == 0
        || x1 < 0
        || x0 >= i64::from(image_width)
    {
        return Ok((0, 0));
    }
    let start_pixel = x0.max(0) as usize;
    let end_pixel = x1.min(i64::from(image_width) - 1) as usize;
    if start_pixel > end_pixel {
        return Ok((0, 0));
    }
    let row_bytes = (image_width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::InternalError("SIMD draw row stride overflow".into()))?;
    let row_start = (y as usize)
        .checked_mul(row_bytes)
        .ok_or_else(|| PilError::InternalError("SIMD draw row offset overflow".into()))?;
    let row_end = row_start
        .checked_add(row_bytes)
        .ok_or_else(|| PilError::InternalError("SIMD draw row end overflow".into()))?;
    let row = output
        .get_mut(row_start..row_end)
        .ok_or_else(|| PilError::InternalError("SIMD draw row buffer shape mismatch".into()))?;
    let target_start = start_pixel
        .checked_mul(channels)
        .ok_or_else(|| PilError::InternalError("SIMD draw span start overflow".into()))?;
    let target_end = end_pixel
        .checked_add(1)
        .and_then(|pixel| pixel.checked_mul(channels))
        .ok_or_else(|| PilError::InternalError("SIMD draw span end overflow".into()))?;

    let mut next = target_start;
    let mut vector_blocks = 0u64;
    while next
        .checked_add(16)
        .is_some_and(|block_end| block_end <= target_end)
    {
        draw_line_vector_block(
            row,
            next,
            target_start,
            target_end,
            channels,
            fill,
            alpha_blend_rgb,
        );
        vector_blocks = vector_blocks.saturating_add(1);
        next += 16;
    }

    let mut scalar_tail = 0u64;
    if next < target_end {
        if vector_blocks == 0 {
            let block_start = if target_start
                .checked_add(16)
                .is_some_and(|block_end| block_end <= row.len())
            {
                target_start
            } else {
                row.len().checked_sub(16).ok_or_else(|| {
                    PilError::InternalError("SIMD draw row has no vector block".into())
                })?
            };
            draw_line_vector_block(
                row,
                block_start,
                target_start,
                target_end,
                channels,
                fill,
                alpha_blend_rgb,
            );
            vector_blocks = 1;
        } else {
            for byte in next..target_end {
                let source = draw_line_source_byte(fill, channels, byte);
                row[byte] = if channels == 3 && alpha_blend_rgb {
                    draw_line_blend_byte(source, row[byte], fill.3)
                } else {
                    source
                };
                scalar_tail = scalar_tail.saturating_add(1);
            }
        }
    }
    Ok((vector_blocks, scalar_tail))
}

fn flush_simd_draw_span(
    span: &mut Option<(i64, i64, i64, i64)>,
    output: &mut [u8],
    image_width: u32,
    image_height: u32,
    channels: usize,
    fill: (u8, u8, u8, u8),
    alpha_blend_rgb: bool,
    vector_blocks: &mut u64,
    scalar_tail: &mut u64,
) -> Result<(), PilError> {
    let Some((x0, x1, _last_x, y)) = span.take() else {
        return Ok(());
    };
    let (vectors, tail) = simd_draw_span(
        output,
        image_width,
        image_height,
        x0,
        x1,
        y,
        channels,
        fill,
        alpha_blend_rgb,
    )?;
    *vector_blocks = vector_blocks.saturating_add(vectors);
    *scalar_tail = scalar_tail.saturating_add(tail);
    Ok(())
}

fn push_simd_draw_point(
    span: &mut Option<(i64, i64, i64, i64)>,
    x: i64,
    y: i64,
    output: &mut [u8],
    image_width: u32,
    image_height: u32,
    channels: usize,
    fill: (u8, u8, u8, u8),
    alpha_blend_rgb: bool,
    vector_blocks: &mut u64,
    scalar_tail: &mut u64,
) -> Result<(), PilError> {
    let extends_run = span
        .as_ref()
        .is_some_and(|(_x0, _x1, last_x, run_y)| *run_y == y && (x - *last_x).abs() == 1);
    if extends_run {
        if let Some((run_x0, run_x1, last_x, _run_y)) = span.as_mut() {
            *run_x0 = (*run_x0).min(x);
            *run_x1 = (*run_x1).max(x);
            *last_x = x;
        }
        return Ok(());
    }
    flush_simd_draw_span(
        span,
        output,
        image_width,
        image_height,
        channels,
        fill,
        alpha_blend_rgb,
        vector_blocks,
        scalar_tail,
    )?;
    *span = Some((x, x, x, y));
    Ok(())
}

/// Draw a width-one Bresenham line with scalar geometry and native-byte SIMD
/// spans.  Consecutive pixels on the same raster row are coalesced into one
/// contiguous span; steep lines remain a sequence of one-pixel masked vector
/// stores rather than entering the scalar CPU draw adapter.

/// Draw points through the native-byte masked vector kernel.  Adjacent points
/// on one row are coalesced so a point cloud that forms a span still gets wide
/// vector stores; isolated points use a masked vector block and never call the
/// scalar CPU drawing adapter.
pub(in crate::compute::pool_simd::ops::adapters::draw) fn simd_draw_points_native(
    img: &DynamicImage,
    points: &[(i32, i32)],
    fill: (u8, u8, u8, u8),
    alpha_blend_rgb: bool,
    channels: usize,
) -> Result<Option<DynamicImage>, PilError> {
    let (image_width, image_height) = img.dimensions();
    let row_bytes = (image_width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::InternalError("SIMD draw row stride overflow".into()))?;
    if has_visible_draw_point(image_width, image_height, points) && row_bytes < 16 {
        return Ok(None);
    }
    let mut output = img.as_bytes().to_vec();
    let mut span = None;
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for &(x, y) in points {
        push_simd_draw_point(
            &mut span,
            i64::from(x),
            i64::from(y),
            &mut output,
            image_width,
            image_height,
            channels,
            fill,
            alpha_blend_rgb,
            &mut vector_blocks,
            &mut scalar_tail,
        )?;
    }
    flush_simd_draw_span(
        &mut span,
        &mut output,
        image_width,
        image_height,
        channels,
        fill,
        alpha_blend_rgb,
        &mut vector_blocks,
        &mut scalar_tail,
    )?;

    if vector_blocks == 0 {
        crate::compute::record_pipeline_operation_path("scalar-control");
        return Ok(Some(img.clone()));
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(image_width, image_height, output, channels).map(Some)
}

fn simd_draw_vertical_segment(
    output: &mut [u8],
    image_width: u32,
    image_height: u32,
    x: i64,
    y0: i64,
    y1: i64,
    channels: usize,
    fill: (u8, u8, u8, u8),
    alpha_blend_rgb: bool,
) -> Result<(u64, u64), PilError> {
    if x < 0 || x >= i64::from(image_width) || image_height == 0 {
        return Ok((0, 0));
    }
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    if y0 < y1 {
        let start = y0.max(0);
        let end = y1.min(i64::from(image_height));
        for y in start..end {
            let (vectors, tail) = simd_draw_span(
                output,
                image_width,
                image_height,
                x,
                x,
                y,
                channels,
                fill,
                alpha_blend_rgb,
            )?;
            vector_blocks = vector_blocks.saturating_add(vectors);
            scalar_tail = scalar_tail.saturating_add(tail);
        }
    } else if y0 > y1 {
        let mut y = y0.min(i64::from(image_height) - 1);
        let stop = y1.max(-1);
        while y > stop {
            let (vectors, tail) = simd_draw_span(
                output,
                image_width,
                image_height,
                x,
                x,
                y,
                channels,
                fill,
                alpha_blend_rgb,
            )?;
            vector_blocks = vector_blocks.saturating_add(vectors);
            scalar_tail = scalar_tail.saturating_add(tail);
            y -= 1;
        }
    }
    Ok((vector_blocks, scalar_tail))
}

pub(in crate::compute::pool_simd::ops::adapters::draw) fn simd_draw_rectangle_native(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    width: u32,
    alpha_blend_rgb: bool,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_draw_layout(img, mode) else {
        return Ok(None);
    };
    let (image_width, image_height) = img.dimensions();
    if image_width == 0
        || image_height == 0
        || x1 < x0
        || y1 < y0
        || !has_visible_draw_rectangle(
            image_width,
            image_height,
            x0,
            y0,
            x1,
            y1,
            fill,
            outline,
            width,
        )
    {
        return Ok(None);
    }
    let row_bytes = (image_width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::InternalError("SIMD draw row stride overflow".into()))?;
    if row_bytes < 16 {
        return Ok(None);
    }
    let mut output = img.as_bytes().to_vec();
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;

    if let Some(fill) = fill {
        let start = i64::from(y0).max(0);
        let end = i64::from(y1).min(i64::from(image_height) - 1);
        for y in start..=end {
            let (vectors, tail) = simd_draw_span(
                &mut output,
                image_width,
                image_height,
                i64::from(x0),
                i64::from(x1),
                y,
                channels,
                fill,
                alpha_blend_rgb,
            )?;
            vector_blocks = vector_blocks.saturating_add(vectors);
            scalar_tail = scalar_tail.saturating_add(tail);
        }
    }

    if let Some(outline) = outline.filter(|_| width != 0) {
        let width = i64::from(width);
        for i in 0..width {
            for y in [
                i64::from(y0).saturating_add(i),
                i64::from(y1).saturating_sub(i),
            ] {
                let (vectors, tail) = simd_draw_span(
                    &mut output,
                    image_width,
                    image_height,
                    i64::from(x0),
                    i64::from(x1),
                    y,
                    channels,
                    outline,
                    alpha_blend_rgb,
                )?;
                vector_blocks = vector_blocks.saturating_add(vectors);
                scalar_tail = scalar_tail.saturating_add(tail);
            }
            for x in [
                i64::from(x1).saturating_sub(i),
                i64::from(x0).saturating_add(i),
            ] {
                let (vectors, tail) = simd_draw_vertical_segment(
                    &mut output,
                    image_width,
                    image_height,
                    x,
                    i64::from(y0).saturating_add(width),
                    i64::from(y1).saturating_sub(width).saturating_add(1),
                    channels,
                    outline,
                    alpha_blend_rgb,
                )?;
                vector_blocks = vector_blocks.saturating_add(vectors);
                scalar_tail = scalar_tail.saturating_add(tail);
            }
        }
    }

    if vector_blocks == 0 {
        return Ok(None);
    }
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(image_width, image_height, output, channels).map(Some)
}
