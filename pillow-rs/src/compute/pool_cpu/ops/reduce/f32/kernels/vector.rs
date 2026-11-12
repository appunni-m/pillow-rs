//! Portable eight-lane F32 2x2 reduction kernel.

use crate::error::PilError;
use crate::raster::{DynamicImage, RgbaImage};
use wide::f32x8;

const LANES: usize = 8;

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn execute(
    input: &RgbaImage,
    x_factor: u32,
    y_factor: u32,
) -> Result<DynamicImage, PilError> {
    let output_width = input.width().div_ceil(x_factor.max(1));
    let output_height = input.height().div_ceil(y_factor.max(1));
    if x_factor != 2
        || y_factor != 2
        || !input.width().is_multiple_of(2)
        || !input.height().is_multiple_of(2)
        || !(output_width as usize).is_multiple_of(LANES)
    {
        return super::super::default::reduce_impl::execute(input, x_factor, y_factor);
    }
    let source = super::super::helpers::storage::source_samples(input)?;
    let source_width = usize::try_from(input.width()).map_err(|_| {
        PilError::InternalError("ImageF32 Reduce source width does not fit usize".into())
    })?;
    let mut output = Vec::with_capacity(super::super::helpers::storage::output_capacity(
        output_width,
        output_height,
    )?);
    for output_y in 0..output_height as usize {
        let source_top = output_y * 2 * source_width;
        let source_bottom = source_top + source_width;
        for output_x in (0..output_width as usize).step_by(LANES) {
            let values = reduce_row(&source, source_top, source_bottom, output_x);
            super::super::helpers::storage::append_samples(&mut output, &values);
        }
    }
    super::super::helpers::storage::finish_output(output_width, output_height, output)
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn reduce_row(
    source: &[f32],
    source_top: usize,
    source_bottom: usize,
    output_x: usize,
) -> [f32; LANES] {
    let source_x = output_x * 2;
    let top_left = f32x8::new(std::array::from_fn(|lane| {
        source[source_top + source_x + lane * 2]
    }));
    let top_right = f32x8::new(std::array::from_fn(|lane| {
        source[source_top + source_x + lane * 2 + 1]
    }));
    let bottom_left = f32x8::new(std::array::from_fn(|lane| {
        source[source_bottom + source_x + lane * 2]
    }));
    let bottom_right = f32x8::new(std::array::from_fn(|lane| {
        source[source_bottom + source_x + lane * 2 + 1]
    }));
    let quartets = (((top_left + top_right) + bottom_left) + bottom_right).to_array();
    quartets.map(average_quartet)
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
#[inline]
fn average_quartet(quartet: f32) -> f32 {
    let mut sum = 0.0f64;
    sum += f64::from(quartet);
    (sum * 0.25) as f32
}
