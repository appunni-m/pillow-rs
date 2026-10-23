use super::super::native_draw_layout;
use super::SimdDrawSpanWriter;
use super::spans::simd_draw_rectangle_native;
use crate::compute::pool_cpu::ops::draw::{
    arc_clip_state, chord_clip_state, chord_line_clip_state, for_each_clipped_ellipse_span,
    for_each_ellipse_span, normalize_angles, pie_clip_state, pie_side_clip_state,
};
use crate::draw::{for_each_bresenham_point, for_each_polygon_fill_span, wide_line_polygon_points};
use crate::error::PilError;
use crate::raster::{DynamicImage, GenericImageView};

mod rounded_rect;
pub(in crate::compute::pool_simd::ops::adapters::draw) use rounded_rect::simd_draw_rounded_rect_native;

fn simd_write_ellipse_spans(
    writer: &mut SimdDrawSpanWriter<'_>,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    geometry_width: i32,
    color: (u8, u8, u8, u8),
) -> Result<(), PilError> {
    let mut first_error = None;
    for_each_ellipse_span(
        x0,
        y0,
        x1,
        y1,
        geometry_width,
        |span_x0, span_y, span_x1| {
            if first_error.is_none()
                && let Err(error) = writer.write(span_x0, span_y, span_x1, color)
            {
                first_error = Some(error);
            }
        },
    );
    first_error.map_or(Ok(()), Err)
}

fn simd_write_clipped_ellipse_spans(
    writer: &mut SimdDrawSpanWriter<'_>,
    x0: i32,
    y0: i32,
    a: i32,
    b: i32,
    state: crate::compute::pool_cpu::ops::draw::ClipEllipseState,
    color: (u8, u8, u8, u8),
) -> Result<(), PilError> {
    let mut first_error = None;
    for_each_clipped_ellipse_span(x0, y0, a, b, state, |span_x0, span_y, span_x1| {
        if first_error.is_none()
            && let Err(error) = writer.write(span_x0, span_y, span_x1, color)
        {
            first_error = Some(error);
        }
    });
    first_error.map_or(Ok(()), Err)
}

/// Draw an ellipse using scalar Pillow-compatible scan conversion and native
/// byte SIMD span writes.  The geometry helper emits the same ordered spans
/// as the CPU canvas; this adapter owns the destination buffer and therefore
/// preserves repeated-span blending without entering a CPU pixel loop.
pub(in crate::compute::pool_simd::ops::adapters::draw) fn simd_draw_ellipse_native(
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
    let row_bytes = (image_width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::InternalError("SIMD ellipse row stride overflow".into()))?;
    if row_bytes < 16 {
        return Ok(None);
    }

    let mut output = img.as_bytes().to_vec();
    let mut writer = SimdDrawSpanWriter {
        output: &mut output,
        image_width,
        image_height,
        channels,
        alpha_blend_rgb,
        vector_blocks: 0,
        scalar_tail: 0,
    };

    if let Some(fill) = fill {
        let geometry_width = x1
            .checked_sub(x0)
            .and_then(|a| y1.checked_sub(y0).map(|b| a.saturating_add(b)))
            .unwrap_or(i32::MAX);
        simd_write_ellipse_spans(&mut writer, x0, y0, x1, y1, geometry_width, fill)?;
    }
    if let Some(outline) = outline.filter(|color| Some(*color) != fill && width != 0) {
        simd_write_ellipse_spans(
            &mut writer,
            x0,
            y0,
            x1,
            y1,
            i32::try_from(width).unwrap_or(i32::MAX),
            outline,
        )?;
    }
    // Keep the small scalar callback above out of the generated data path if
    // no valid span was produced: this is a SIMD control-plane no-op, not an
    // implicit CPU fallback.
    let vector_blocks = writer.vector_blocks;
    let scalar_tail = writer.scalar_tail;
    drop(writer);
    if vector_blocks == 0 {
        crate::compute::record_pipeline_operation_path("scalar-control");
        return Ok(Some(img.clone()));
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(image_width, image_height, output, channels).map(Some)
}

pub(in crate::compute::pool_simd::ops::adapters::draw) fn simd_draw_arc_native(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    start: f64,
    end: f64,
    fill: Option<(u8, u8, u8, u8)>,
    width: u32,
    alpha_blend_rgb: bool,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_draw_layout(img, mode) else {
        return Ok(None);
    };
    let (image_width, image_height) = img.dimensions();
    let row_bytes = (image_width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::InternalError("SIMD arc row stride overflow".into()))?;
    if row_bytes < 16 {
        return Ok(None);
    }
    let mut output = img.as_bytes().to_vec();
    let mut writer = SimdDrawSpanWriter {
        output: &mut output,
        image_width,
        image_height,
        channels,
        alpha_blend_rgb,
        vector_blocks: 0,
        scalar_tail: 0,
    };
    let (start, end) = normalize_angles(start as f32, end as f32);
    if start + 360.0 == end {
        if let Some(fill) = fill {
            simd_write_ellipse_spans(
                &mut writer,
                x0,
                y0,
                x1,
                y1,
                i32::try_from(width).unwrap_or(i32::MAX),
                fill,
            )?;
        }
    } else if start != end
        && let (Some(a), Some(b)) = (x1.checked_sub(x0), y1.checked_sub(y0))
        && a >= 0
        && b >= 0
    {
        let state = arc_clip_state(a, b, i32::try_from(width).unwrap_or(i32::MAX), start, end);
        if let Some(fill) = fill {
            simd_write_clipped_ellipse_spans(&mut writer, x0, y0, a, b, state, fill)?;
        }
    }

    let vector_blocks = writer.vector_blocks;
    let scalar_tail = writer.scalar_tail;
    drop(writer);
    if vector_blocks == 0 {
        crate::compute::record_pipeline_operation_path("scalar-control");
        return Ok(Some(img.clone()));
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(image_width, image_height, output, channels).map(Some)
}

pub(in crate::compute::pool_simd::ops::adapters::draw) fn simd_draw_chord_native(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    start: f64,
    end: f64,
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
    let row_bytes = (image_width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::InternalError("SIMD chord row stride overflow".into()))?;
    if row_bytes < 16 {
        return Ok(None);
    }
    let mut output = img.as_bytes().to_vec();
    let mut writer = SimdDrawSpanWriter {
        output: &mut output,
        image_width,
        image_height,
        channels,
        alpha_blend_rgb,
        vector_blocks: 0,
        scalar_tail: 0,
    };
    let (start, end) = normalize_angles(start as f32, end as f32);
    let dimensions = x1
        .checked_sub(x0)
        .zip(y1.checked_sub(y0))
        .filter(|(a, b)| *a >= 0 && *b >= 0);
    if start + 360.0 == end {
        if let Some(fill) = fill {
            let geometry_width = x1
                .checked_sub(x0)
                .and_then(|a| y1.checked_sub(y0).map(|b| a.saturating_add(b)))
                .unwrap_or(i32::MAX);
            simd_write_ellipse_spans(&mut writer, x0, y0, x1, y1, geometry_width, fill)?;
        }
        if let Some(outline) = outline.filter(|color| Some(*color) != fill && width != 0) {
            simd_write_ellipse_spans(
                &mut writer,
                x0,
                y0,
                x1,
                y1,
                i32::try_from(width).unwrap_or(i32::MAX),
                outline,
            )?;
        }
    } else if start != end
        && let Some((a, b)) = dimensions
    {
        if let Some(fill) = fill {
            let state = chord_clip_state(a, b, a.saturating_add(b).saturating_add(1), start, end);
            simd_write_clipped_ellipse_spans(&mut writer, x0, y0, a, b, state, fill)?;
        }
        if let Some(outline) = outline.filter(|color| Some(*color) != fill && width != 0) {
            let width = i32::try_from(width).unwrap_or(i32::MAX);
            let line_state = chord_line_clip_state(a, b, width, start, end);
            simd_write_clipped_ellipse_spans(&mut writer, x0, y0, a, b, line_state, outline)?;
            let arc_state = chord_clip_state(a, b, width, start, end);
            simd_write_clipped_ellipse_spans(&mut writer, x0, y0, a, b, arc_state, outline)?;
        }
    }

    let vector_blocks = writer.vector_blocks;
    let scalar_tail = writer.scalar_tail;
    drop(writer);
    if vector_blocks == 0 {
        crate::compute::record_pipeline_operation_path("scalar-control");
        return Ok(Some(img.clone()));
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(image_width, image_height, output, channels).map(Some)
}

pub(in crate::compute::pool_simd::ops::adapters::draw) fn simd_draw_pieslice_native(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    start: f64,
    end: f64,
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
    let row_bytes = (image_width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::InternalError("SIMD pieslice row stride overflow".into()))?;
    if row_bytes < 16 {
        return Ok(None);
    }
    let mut output = img.as_bytes().to_vec();
    let mut writer = SimdDrawSpanWriter {
        output: &mut output,
        image_width,
        image_height,
        channels,
        alpha_blend_rgb,
        vector_blocks: 0,
        scalar_tail: 0,
    };
    let (start, end) = normalize_angles(start as f32, end as f32);
    let dimensions = x1
        .checked_sub(x0)
        .zip(y1.checked_sub(y0))
        .filter(|(a, b)| *a >= 0 && *b >= 0);
    if start + 360.0 == end {
        if let Some(fill) = fill {
            let geometry_width = x1
                .checked_sub(x0)
                .and_then(|a| y1.checked_sub(y0).map(|b| a.saturating_add(b)))
                .unwrap_or(i32::MAX);
            simd_write_ellipse_spans(&mut writer, x0, y0, x1, y1, geometry_width, fill)?;
        }
        if let Some(outline) = outline.filter(|color| Some(*color) != fill && width != 0) {
            simd_write_ellipse_spans(
                &mut writer,
                x0,
                y0,
                x1,
                y1,
                i32::try_from(width).unwrap_or(i32::MAX),
                outline,
            )?;
        }
    } else if start != end
        && let Some((a, b)) = dimensions
    {
        if let Some(fill) = fill {
            let state = pie_clip_state(a, b, a.saturating_add(b), start, end);
            simd_write_clipped_ellipse_spans(&mut writer, x0, y0, a, b, state, fill)?;
        }
        if let Some(outline) = outline.filter(|color| Some(*color) != fill && width != 0) {
            let width = i32::try_from(width).unwrap_or(i32::MAX);
            for angle in [start, end] {
                let state = pie_side_clip_state(a, b, width, angle);
                simd_write_clipped_ellipse_spans(&mut writer, x0, y0, a, b, state, outline)?;
            }
            let center_x =
                ((f64::from(x0) + f64::from(x1) - f64::from(width)) / 2.0).round() as i32;
            let center_y =
                ((f64::from(y0) + f64::from(y1) - f64::from(width)) / 2.0).round() as i32;
            simd_write_ellipse_spans(
                &mut writer,
                center_x,
                center_y,
                center_x.saturating_add(width - 1),
                center_y.saturating_add(width - 1),
                width.saturating_mul(2).saturating_sub(2),
                outline,
            )?;
            let state = pie_clip_state(a, b, width, start, end);
            simd_write_clipped_ellipse_spans(&mut writer, x0, y0, a, b, state, outline)?;
        }
    }

    let vector_blocks = writer.vector_blocks;
    let scalar_tail = writer.scalar_tail;
    drop(writer);
    if vector_blocks == 0 {
        crate::compute::record_pipeline_operation_path("scalar-control");
        return Ok(Some(img.clone()));
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(image_width, image_height, output, channels).map(Some)
}

fn simd_write_rect_spans(
    writer: &mut SimdDrawSpanWriter<'_>,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: (u8, u8, u8, u8),
) -> Result<(), PilError> {
    if y0 > y1 {
        return Ok(());
    }
    for y in i64::from(y0)..=i64::from(y1) {
        let y = y.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
        writer.write(x0, y, x1, color)?;
    }
    Ok(())
}

fn polygon_fill_intervals(
    points: &[(i32, i32)],
    image_width: u32,
    image_height: u32,
) -> std::collections::BTreeMap<i32, Vec<(i32, i32)>> {
    let mut rows = std::collections::BTreeMap::new();
    for_each_polygon_fill_span(points, image_width, image_height, |x0, x1, y| {
        rows.entry(y).or_insert_with(Vec::new).push((x0, x1));
    });
    for intervals in rows.values_mut() {
        intervals.sort_unstable();
        let mut merged: Vec<(i32, i32)> = Vec::with_capacity(intervals.len());
        for (x0, x1) in std::mem::take(intervals) {
            if let Some(last) = merged.last_mut()
                && x0 <= last.1.saturating_add(1)
            {
                last.1 = last.1.max(x1);
            } else {
                merged.push((x0, x1));
            }
        }
        *intervals = merged;
    }
    rows
}

pub(in crate::compute::pool_simd::ops::adapters::draw) fn simd_draw_polygon_native(
    img: &DynamicImage,
    points: &[(i32, i32)],
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
    let row_bytes = (image_width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::InternalError("SIMD polygon row stride overflow".into()))?;
    if row_bytes < 16 {
        return Ok(None);
    }

    let mut output = img.as_bytes().to_vec();
    let mut writer = SimdDrawSpanWriter {
        output: &mut output,
        image_width,
        image_height,
        channels,
        alpha_blend_rgb,
        vector_blocks: 0,
        scalar_tail: 0,
    };
    let mut first_error = None;
    if let Some(fill) = fill {
        for_each_polygon_fill_span(points, image_width, image_height, |x0, x1, y| {
            if first_error.is_none()
                && let Err(error) = writer.write(x0, y, x1, fill)
            {
                first_error = Some(error);
            }
        });
    }

    // Pillow masks a wide polygon outline against the filled polygon. Keep
    // that mask as compact row intervals rather than materializing a second
    // full-frame image. Stroke spans are still emitted in edge order, so
    // repeated writes and RGB alpha blending retain CPU ordering.
    if let Some(outline) = outline.filter(|color| Some(*color) != fill && width != 0) {
        let mask = polygon_fill_intervals(points, image_width, image_height);
        for index in 0..points.len() {
            let (x0, y0) = points[index];
            let (x1, y1) = points[(index + 1) % points.len()];
            if width == 1 {
                for_each_bresenham_point(x0, y0, x1, y1, |x, y| {
                    if first_error.is_none()
                        && let Err(error) = writer.write(x, y, x, outline)
                    {
                        first_error = Some(error);
                    }
                });
                continue;
            }

            let stroke_width = width.saturating_mul(2).saturating_sub(1);
            if let Some(stroke_points) = wide_line_polygon_points(x0, y0, x1, y1, stroke_width) {
                for_each_polygon_fill_span(
                    &stroke_points,
                    image_width,
                    image_height,
                    |span_x0, span_x1, y| {
                        if first_error.is_some() {
                            return;
                        }
                        if let Some(row) = mask.get(&y) {
                            for &(mask_x0, mask_x1) in row {
                                let clipped_x0 = span_x0.max(mask_x0);
                                let clipped_x1 = span_x1.min(mask_x1);
                                if clipped_x0 <= clipped_x1
                                    && let Err(error) =
                                        writer.write(clipped_x0, y, clipped_x1, outline)
                                {
                                    first_error = Some(error);
                                    break;
                                }
                            }
                        }
                    },
                );
            } else if first_error.is_none()
                && mask
                    .get(&y0)
                    .is_some_and(|row| row.iter().any(|&(left, right)| x0 >= left && x0 <= right))
                && let Err(error) = writer.write(x0, y0, x0, outline)
            {
                first_error = Some(error);
            }
        }
    }
    if let Some(error) = first_error {
        return Err(error);
    }

    let vector_blocks = writer.vector_blocks;
    let scalar_tail = writer.scalar_tail;
    drop(writer);
    if vector_blocks == 0 {
        crate::compute::record_pipeline_operation_path("scalar-control");
        return Ok(Some(img.clone()));
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(image_width, image_height, output, channels).map(Some)
}
