//! Line geometry with native-byte SIMD span writes.

use super::{
    SimdDrawSpanWriter, clipped_horizontal_span, draw_line_vector_block, flush_simd_draw_span,
    native_draw_layout, push_simd_draw_point,
};
use crate::draw::{for_each_polygon_fill_span, wide_line_polygon_points};
use crate::error::PilError;
use crate::raster::{DynamicImage, GenericImageView};

pub(in crate::compute::pool_simd::ops::adapters::draw) fn simd_draw_line_native(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    fill: (u8, u8, u8, u8),
    width: u32,
    alpha_blend_rgb: bool,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_draw_layout(img, mode) else {
        return Ok(None);
    };
    let Some(row_bytes) = (img.width() as usize).checked_mul(channels) else {
        return Ok(None);
    };
    // A real SIMD block must be available in the native row. If the row is
    // narrower, automatic routing selects CPU and strict SIMD rejects it.
    if row_bytes < 16 {
        return Ok(None);
    }
    if width > 1 {
        return draw_wide_line(img, x0, y0, x1, y1, channels, fill, width, alpha_blend_rgb);
    }
    if y0 != y1 {
        return simd_draw_bresenham_native(img, x0, y0, x1, y1, channels, fill, alpha_blend_rgb);
    }
    draw_horizontal_line(
        img,
        x0,
        y0,
        x1,
        y1,
        channels,
        row_bytes,
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
    channels: usize,
    fill: (u8, u8, u8, u8),
    width: u32,
    alpha_blend_rgb: bool,
) -> Result<Option<DynamicImage>, PilError> {
    let (image_width, image_height) = img.dimensions();
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

fn draw_horizontal_line(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    channels: usize,
    row_bytes: usize,
    fill: (u8, u8, u8, u8),
    alpha_blend_rgb: bool,
) -> Result<Option<DynamicImage>, PilError> {
    let Some((start_pixel, end_pixel, row_index)) =
        clipped_horizontal_span(img.width(), img.height(), x0, y0, x1, y1)
    else {
        return Ok(None);
    };
    let Some(target_start) = start_pixel.checked_mul(channels) else {
        return Ok(None);
    };
    let Some(target_end) = end_pixel
        .checked_add(1)
        .and_then(|pixel| pixel.checked_mul(channels))
    else {
        return Ok(None);
    };
    let Some(row_start) = row_index.checked_mul(row_bytes) else {
        return Ok(None);
    };
    let Some(row_end) = row_start.checked_add(row_bytes) else {
        return Ok(None);
    };
    let mut output = img.as_bytes().to_vec();
    let Some(row) = output.get_mut(row_start..row_end) else {
        return Ok(None);
    };

    let mut next = target_start;
    let mut vector_blocks = 0u64;
    while next.checked_add(16).is_some_and(|end| end <= target_end) {
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
            // A short span uses a masked vector block wholly inside the row,
            // including when the span touches its right edge.
            let block_start = if target_start
                .checked_add(16)
                .is_some_and(|end| end <= row.len())
            {
                target_start
            } else {
                row.len() - 16
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
                let source = super::draw_line_source_byte(fill, channels, byte);
                row[byte] = if channels == 3 && alpha_blend_rgb {
                    super::draw_line_blend_byte(source, row[byte], fill.3)
                } else {
                    source
                };
                scalar_tail = scalar_tail.saturating_add(1);
            }
        }
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels).map(Some)
}

struct BresenhamSpans {
    output: Vec<u8>,
    span: Option<(i64, i64, i64, i64)>,
    image_width: u32,
    image_height: u32,
    channels: usize,
    fill: (u8, u8, u8, u8),
    alpha_blend_rgb: bool,
    vector_blocks: u64,
    scalar_tail: u64,
}

impl BresenhamSpans {
    #[inline(always)]
    fn push(&mut self, x: i64, y: i64) -> Result<(), PilError> {
        push_simd_draw_point(
            &mut self.span,
            x,
            y,
            &mut self.output,
            self.image_width,
            self.image_height,
            self.channels,
            self.fill,
            self.alpha_blend_rgb,
            &mut self.vector_blocks,
            &mut self.scalar_tail,
        )
    }

    fn finish(
        mut self,
        img: &DynamicImage,
        target_x: i64,
        target_y: i64,
    ) -> Result<Option<DynamicImage>, PilError> {
        self.push(target_x, target_y)?;
        flush_simd_draw_span(
            &mut self.span,
            &mut self.output,
            self.image_width,
            self.image_height,
            self.channels,
            self.fill,
            self.alpha_blend_rgb,
            &mut self.vector_blocks,
            &mut self.scalar_tail,
        )?;
        if self.vector_blocks == 0 {
            crate::compute::record_pipeline_operation_path("scalar-control");
            return Ok(Some(img.clone()));
        }
        crate::compute::record_pipeline_operation_path("vector");
        crate::compute::record_pipeline_operation_vector_blocks(self.vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(self.scalar_tail);
        crate::image_utils::raw_bytes_to_image(
            self.image_width,
            self.image_height,
            self.output,
            self.channels,
        )
        .map(Some)
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
    channels: usize,
    fill: (u8, u8, u8, u8),
    alpha_blend_rgb: bool,
) -> Result<Option<DynamicImage>, PilError> {
    let (image_width, image_height) = img.dimensions();
    let row_bytes = (image_width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::InternalError("SIMD draw row stride overflow".into()))?;
    if row_bytes < 16 {
        return Ok(None);
    }
    let geometry = BresenhamGeometry::new(x0, y0, x1, y1);
    let mut spans = BresenhamSpans {
        output: img.as_bytes().to_vec(),
        span: None,
        image_width,
        image_height,
        channels,
        fill,
        alpha_blend_rgb,
        vector_blocks: 0,
        scalar_tail: 0,
    };
    geometry.walk(&mut spans)?;
    spans.finish(img, geometry.target_x, geometry.target_y)
}
