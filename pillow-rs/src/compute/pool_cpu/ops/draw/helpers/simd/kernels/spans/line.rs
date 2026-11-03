//! Line geometry with native-byte SIMD span writes.

use super::{
    SimdDrawSpanWriter, clipped_horizontal_span, flush_simd_draw_span, push_simd_draw_point,
};
use super::super::super::DrawLayout;
use crate::draw::{for_each_polygon_fill_span, wide_line_polygon_points};
use crate::error::PilError;
use crate::raster::{DynamicImage, GenericImageView};

pub(in crate::compute::pool_cpu::ops::draw::helpers::simd) fn simd_draw_line_native(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    fill: (u8, u8, u8, u8),
    width: u32,
    alpha_blend_rgb: bool,
    layout: DrawLayout,
) -> Result<Option<DynamicImage>, PilError> {
    let channels = layout.channels;
    let Some(row_bytes) = (img.width() as usize).checked_mul(channels) else {
        return Ok(None);
    };
    // A real SIMD block must be available in the native row. If the row is
    // narrower, automatic routing selects CPU and strict SIMD rejects it.
    if row_bytes < 16 {
        return Ok(None);
    }
    if width > 1 {
        return draw_wide_line(
            img,
            x0,
            y0,
            x1,
            y1,
            fill,
            width,
            alpha_blend_rgb,
            layout,
        );
    }
    if y0 != y1 {
        return simd_draw_bresenham_native(
            img,
            x0,
            y0,
            x1,
            y1,
            fill,
            alpha_blend_rgb,
            layout,
        );
    }
    draw_horizontal_line(
        img,
        x0,
        y0,
        x1,
        y1,
        layout,
        fill,
        alpha_blend_rgb,
    )
}

fn draw_wide_line(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    fill: (u8, u8, u8, u8),
    width: u32,
    alpha_blend_rgb: bool,
    layout: DrawLayout,
) -> Result<Option<DynamicImage>, PilError> {
    let (image_width, image_height) = img.dimensions();
    let mut writer = SimdDrawSpanWriter::new(img, layout, alpha_blend_rgb);
    if let Some(points) = wide_line_polygon_points(x0, y0, x1, y1, width) {
        let mut first_error = None;
        for_each_polygon_fill_span(&points, image_width, image_height, |span_x0, span_x1, y| {
            if first_error.is_none()
                && let Err(error) = writer.write(span_x0, y, span_x1, fill)
            {
                first_error = Some(error);
            }
        });
        if let Some(error) = first_error {
            return Err(error);
        }
    } else {
        writer.write(x0, y0, x0, fill)?;
    }
    if writer.vector_blocks == 0 {
        crate::compute::record_pipeline_operation_path("scalar-control");
        return Ok(Some(img.clone()));
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(writer.vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(writer.scalar_tail);
    writer.into_image().map(Some)
}

fn draw_horizontal_line(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    layout: DrawLayout,
    fill: (u8, u8, u8, u8),
    alpha_blend_rgb: bool,
) -> Result<Option<DynamicImage>, PilError> {
    if clipped_horizontal_span(img.width(), img.height(), x0, y0, x1, y1).is_none() {
        return Ok(None);
    }
    let mut writer = SimdDrawSpanWriter::new(img, layout, alpha_blend_rgb);
    writer.write_span(
        i64::from(x0.min(x1)),
        i64::from(x0.max(x1)),
        i64::from(y0),
        fill,
    )?;
    if writer.vector_blocks == 0 {
        return Ok(None);
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(writer.vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(writer.scalar_tail);
    writer.into_image().map(Some)
}

struct BresenhamSpans {
    writer: SimdDrawSpanWriter,
    span: Option<(i64, i64, i64, i64)>,
    fill: (u8, u8, u8, u8),
}

impl BresenhamSpans {
    #[inline(always)]
    fn push(&mut self, x: i64, y: i64) -> Result<(), PilError> {
        push_simd_draw_point(
            &mut self.span,
            x,
            y,
            &mut self.writer,
            self.fill,
        )
    }

    fn finish(
        mut self,
        img: &DynamicImage,
        target_x: i64,
        target_y: i64,
    ) -> Result<Option<DynamicImage>, PilError> {
        self.push(target_x, target_y)?;
        flush_simd_draw_span(&mut self.span, &mut self.writer, self.fill)?;
        if self.writer.vector_blocks == 0 {
            crate::compute::record_pipeline_operation_path("scalar-control");
            return Ok(Some(img.clone()));
        }
        crate::compute::record_pipeline_operation_path("vector");
        crate::compute::record_pipeline_operation_vector_blocks(self.writer.vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(self.writer.scalar_tail);
        self.writer.into_image().map(Some)
    }
}

#[derive(Clone, Copy)]
struct BresenhamGeometry {
    x: i64,
    y: i64,
    target_x: i64,
    target_y: i64,
    dx: i64,
    dy: i64,
    step_x: i64,
    step_y: i64,
}

impl BresenhamGeometry {
    fn new(x0: i32, y0: i32, x1: i32, y1: i32) -> Self {
        let x = i64::from(x0);
        let y = i64::from(y0);
        let target_x = i64::from(x1);
        let target_y = i64::from(y1);
        let mut dx = target_x - x;
        let step_x = if dx < 0 {
            dx = -dx;
            -1
        } else {
            1
        };
        let mut dy = target_y - y;
        let step_y = if dy < 0 {
            dy = -dy;
            -1
        } else {
            1
        };
        Self {
            x,
            y,
            target_x,
            target_y,
            dx,
            dy,
            step_x,
            step_y,
        }
    }

    fn walk(self, spans: &mut BresenhamSpans) -> Result<(), PilError> {
        if self.dx == 0 {
            self.walk_vertical(spans)
        } else if self.dy == 0 {
            self.walk_horizontal(spans)
        } else if self.dx > self.dy {
            self.walk_shallow(spans)
        } else {
            self.walk_steep(spans)
        }
    }

    fn walk_vertical(self, spans: &mut BresenhamSpans) -> Result<(), PilError> {
        let mut y = self.y;
        for _ in 0..self.dy {
            spans.push(self.x, y)?;
            y += self.step_y;
        }
        Ok(())
    }

    fn walk_horizontal(self, spans: &mut BresenhamSpans) -> Result<(), PilError> {
        let mut x = self.x;
        for _ in 0..self.dx {
            spans.push(x, self.y)?;
            x += self.step_x;
        }
        Ok(())
    }

    fn walk_shallow(self, spans: &mut BresenhamSpans) -> Result<(), PilError> {
        let steps = self.dx;
        let dy = self.dy + self.dy;
        let mut error = dy - self.dx;
        let dx = self.dx + self.dx;
        let (mut x, mut y) = (self.x, self.y);
        for _ in 0..steps {
            spans.push(x, y)?;
            if error >= 0 {
                y += self.step_y;
                error -= dx;
            }
            error += dy;
            x += self.step_x;
        }
        Ok(())
    }

    fn walk_steep(self, spans: &mut BresenhamSpans) -> Result<(), PilError> {
        let steps = self.dy;
        let dx = self.dx + self.dx;
        let mut error = dx - self.dy;
        let dy = self.dy + self.dy;
        let (mut x, mut y) = (self.x, self.y);
        for _ in 0..steps {
            spans.push(x, y)?;
            if error >= 0 {
                x += self.step_x;
                error -= dy;
            }
            error += dx;
            y += self.step_y;
        }
        Ok(())
    }
}

/// Draw a width-one Bresenham line with scalar geometry and native-byte SIMD
/// spans. Consecutive pixels on one row are coalesced before the vector store.
fn simd_draw_bresenham_native(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    fill: (u8, u8, u8, u8),
    alpha_blend_rgb: bool,
    layout: DrawLayout,
) -> Result<Option<DynamicImage>, PilError> {
    let image_width = img.width();
    let row_bytes = (image_width as usize)
        .checked_mul(layout.channels)
        .ok_or_else(|| PilError::InternalError("SIMD draw row stride overflow".into()))?;
    if row_bytes < 16 {
        return Ok(None);
    }
    let geometry = BresenhamGeometry::new(x0, y0, x1, y1);
    let mut spans = BresenhamSpans {
        writer: SimdDrawSpanWriter::new(img, layout, alpha_blend_rgb),
        span: None,
        fill,
    };
    geometry.walk(&mut spans)?;
    spans.finish(img, geometry.target_x, geometry.target_y)
}
