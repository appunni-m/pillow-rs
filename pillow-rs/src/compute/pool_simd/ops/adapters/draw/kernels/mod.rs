mod shapes;
mod spans;

use crate::error::PilError;

pub(super) use self::shapes::{
    simd_draw_arc_native, simd_draw_chord_native, simd_draw_ellipse_native,
    simd_draw_pieslice_native, simd_draw_polygon_native, simd_draw_rounded_rect_native,
};
pub(in crate::compute::pool_simd::ops::adapters) use self::spans::draw_line_vector_block;
pub(super) use self::spans::{
    simd_draw_line_native, simd_draw_points_native, simd_draw_rectangle_native,
};

struct SimdDrawSpanWriter<'a> {
    output: &'a mut [u8],
    image_width: u32,
    image_height: u32,
    channels: usize,
    alpha_blend_rgb: bool,
    vector_blocks: u64,
    scalar_tail: u64,
}

impl SimdDrawSpanWriter<'_> {
    fn write(&mut self, x0: i32, y: i32, x1: i32, color: (u8, u8, u8, u8)) -> Result<(), PilError> {
        let (vectors, tail) = spans::simd_draw_span(
            self.output,
            self.image_width,
            self.image_height,
            i64::from(x0),
            i64::from(x1),
            i64::from(y),
            self.channels,
            color,
            self.alpha_blend_rgb,
        )?;
        self.vector_blocks = self.vector_blocks.saturating_add(vectors);
        self.scalar_tail = self.scalar_tail.saturating_add(tail);
        Ok(())
    }
}
