//! Rounded-rectangle SIMD span preparation and drawing.

use super::{
    SimdDrawSpanWriter, native_draw_layout, simd_draw_ellipse_native, simd_draw_rectangle_native,
    simd_write_clipped_ellipse_spans, simd_write_rect_spans,
};
use crate::compute::pool_cpu::ops::draw::{arc_clip_state, pie_clip_state};
use crate::error::PilError;
use crate::raster::{DynamicImage, GenericImageView};

type RoundedRectCorner = (i32, i32, i32, i32, f32, f32);

struct RoundedRectSpanGeometry {
    corners: Vec<RoundedRectCorner>,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    radius: i32,
    full_x: bool,
    full_y: bool,
}

enum RoundedRectGeometry {
    Ellipse,
    Rectangle,
    Spans(RoundedRectSpanGeometry),
}

fn write_rounded_rect_fill(
    writer: &mut SimdDrawSpanWriter<'_>,
    geometry: &RoundedRectSpanGeometry,
    fill: (u8, u8, u8, u8),
) -> Result<(), PilError> {
    for &(left, top, right, bottom, start, end) in &geometry.corners {
        let a = right.saturating_sub(left);
        let b = bottom.saturating_sub(top);
        let state = pie_clip_state(a, b, a.saturating_add(b), start, end);
        simd_write_clipped_ellipse_spans(writer, left, top, a, b, state, fill)?;
    }
    if geometry.full_x {
        simd_write_rect_spans(
            writer,
            geometry.x0,
            geometry.y0.saturating_add(geometry.radius + 1),
            geometry.x1,
            geometry.y1.saturating_sub(geometry.radius + 1),
            fill,
        )?;
    } else if geometry.x1 - geometry.radius - 1 > geometry.x0 + geometry.radius {
        simd_write_rect_spans(
            writer,
            geometry.x0.saturating_add(geometry.radius + 1),
            geometry.y0,
            geometry.x1.saturating_sub(geometry.radius + 1),
            geometry.y1,
            fill,
        )?;
    }
    if !geometry.full_x && !geometry.full_y {
        simd_write_rect_spans(
            writer,
            geometry.x0,
            geometry.y0.saturating_add(geometry.radius + 1),
            geometry.x0.saturating_add(geometry.radius),
            geometry.y1.saturating_sub(geometry.radius + 1),
            fill,
        )?;
        simd_write_rect_spans(
            writer,
            geometry.x1.saturating_sub(geometry.radius),
            geometry.y0.saturating_add(geometry.radius + 1),
            geometry.x1,
            geometry.y1.saturating_sub(geometry.radius + 1),
            fill,
        )?;
    }
    Ok(())
}

fn write_rounded_rect_outline(
    writer: &mut SimdDrawSpanWriter<'_>,
    geometry: &RoundedRectSpanGeometry,
    width: i32,
    outline: (u8, u8, u8, u8),
) -> Result<(), PilError> {
    for &(left, top, right, bottom, start, end) in &geometry.corners {
        let a = right.saturating_sub(left);
        let b = bottom.saturating_sub(top);
        let state = arc_clip_state(a, b, width, start, end);
        simd_write_clipped_ellipse_spans(writer, left, top, a, b, state, outline)?;
    }
    if !geometry.full_x {
        simd_write_rect_spans(
            writer,
            geometry.x0.saturating_add(geometry.radius + 1),
            geometry.y0,
            geometry.x1.saturating_sub(geometry.radius + 1),
            geometry.y0.saturating_add(width - 1),
            outline,
        )?;
        simd_write_rect_spans(
            writer,
            geometry.x0.saturating_add(geometry.radius + 1),
            geometry.y1.saturating_sub(width - 1),
            geometry.x1.saturating_sub(geometry.radius + 1),
            geometry.y1,
            outline,
        )?;
    }
    if !geometry.full_y {
        simd_write_rect_spans(
            writer,
            geometry.x0,
            geometry.y0.saturating_add(geometry.radius + 1),
            geometry.x0.saturating_add(width - 1),
            geometry.y1.saturating_sub(geometry.radius + 1),
            outline,
        )?;
        simd_write_rect_spans(
            writer,
            geometry.x1.saturating_sub(width - 1),
            geometry.y0.saturating_add(geometry.radius + 1),
            geometry.x1,
            geometry.y1.saturating_sub(geometry.radius + 1),
            outline,
        )?;
    }
    Ok(())
}

fn rounded_rect_geometry(x0: i32, y0: i32, x1: i32, y1: i32, radius: f64) -> RoundedRectGeometry {
    let mut diameter = radius * 2.0;
    let full_x = diameter >= f64::from(x1 - x0 - 1);
    if full_x {
        diameter = f64::from(x1 - x0);
    }
    let full_y = full_x && diameter >= f64::from(y1 - y0 - 1);
    if full_y {
        diameter = f64::from(y1 - y0);
    }
    if full_x && full_y {
        return RoundedRectGeometry::Ellipse;
    }
    if diameter == 0.0 {
        return RoundedRectGeometry::Rectangle;
    }

    let diameter = diameter as i32;
    let radius = diameter / 2;
    let corners = if full_x {
        vec![
            (x0, y0, x0 + diameter, y0 + diameter, 180.0_f32, 360.0_f32),
            (x0, y1 - diameter, x0 + diameter, y1, 0.0, 180.0),
        ]
    } else if full_y {
        vec![
            (x0, y0, x0 + diameter, y0 + diameter, 90.0_f32, 270.0_f32),
            (x1 - diameter, y0, x1, y0 + diameter, 270.0, 90.0),
        ]
    } else {
        vec![
            (x0, y0, x0 + diameter, y0 + diameter, 180.0_f32, 270.0_f32),
            (x1 - diameter, y0, x1, y0 + diameter, 270.0, 360.0),
            (x1 - diameter, y1 - diameter, x1, y1, 0.0, 90.0),
            (x0, y1 - diameter, x0 + diameter, y1, 90.0, 180.0),
        ]
    };
    RoundedRectGeometry::Spans(RoundedRectSpanGeometry {
        corners,
        x0,
        y0,
        x1,
        y1,
        radius,
        full_x,
        full_y,
    })
}

fn draw_rounded_rect_spans(
    img: &DynamicImage,
    channels: usize,
    geometry: &RoundedRectSpanGeometry,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    width: u32,
    alpha_blend_rgb: bool,
) -> Result<Option<DynamicImage>, PilError> {
    let (image_width, image_height) = img.dimensions();
    let mut output = img.as_bytes().to_vec();
    let (vector_blocks, scalar_tail) = {
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
            write_rounded_rect_fill(&mut writer, geometry, fill)?;
        }
        if let Some(outline) = outline.filter(|color| Some(*color) != fill && width != 0) {
            let width = i32::try_from(width).unwrap_or(i32::MAX);
            write_rounded_rect_outline(&mut writer, geometry, width, outline)?;
        }

        (writer.vector_blocks, writer.scalar_tail)
    };
    if vector_blocks == 0 {
        crate::compute::record_pipeline_operation_path("scalar-control");
        return Ok(Some(img.clone()));
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(image_width, image_height, output, channels).map(Some)
}

pub(in crate::compute::pool_simd::ops::adapters::draw) fn simd_draw_rounded_rect_native(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    radius: f64,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    width: u32,
    alpha_blend_rgb: bool,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_draw_layout(img, mode) else {
        return Ok(None);
    };
    let image_width = img.dimensions().0;
    let row_bytes = (image_width as usize)
        .checked_mul(channels)
        .ok_or_else(|| {
            PilError::InternalError("SIMD rounded rectangle row stride overflow".into())
        })?;
    if row_bytes < 16 || x1 < x0 || y1 < y0 || !radius.is_finite() || radius < 0.0 {
        return Ok(None);
    }

    match rounded_rect_geometry(x0, y0, x1, y1, radius) {
        RoundedRectGeometry::Ellipse => simd_draw_ellipse_native(
            img,
            x0,
            y0,
            x1,
            y1,
            fill,
            outline,
            width,
            alpha_blend_rgb,
            mode,
        ),
        RoundedRectGeometry::Rectangle => simd_draw_rectangle_native(
            img,
            x0,
            y0,
            x1,
            y1,
            fill,
            outline,
            width,
            alpha_blend_rgb,
            mode,
        ),
        RoundedRectGeometry::Spans(geometry) => draw_rounded_rect_spans(
            img,
            channels,
            &geometry,
            fill,
            outline,
            width,
            alpha_blend_rgb,
        ),
    }
}
