//! Cumulative affine nearest sampling that copies complete F32 sample words.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::RgbaImage;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(
    input: &RgbaImage,
    width: u32,
    height: u32,
    bounds: super::super::super::helpers::typed_nearest::BoxBounds,
) -> Result<RgbaImage, PilError> {
    let (source_width, source_height) = input.dimensions();
    let dims = CheckedDims::new_allow_empty(width, height, 4)?;
    let mut output = vec![0; dims.total_bytes()];
    let (left, top, scale_x, scale_y) = match bounds {
        Some((left, top, right, bottom)) => {
            let left = left as f32;
            let top = top as f32;
            let right = right as f32;
            let bottom = bottom as f32;
            (
                f64::from(left),
                f64::from(top),
                f64::from(right - left) / f64::from(width),
                f64::from(bottom - top) / f64::from(height),
            )
        }
        None => (
            0.0,
            0.0,
            f64::from(source_width) / f64::from(width),
            f64::from(source_height) / f64::from(height),
        ),
    };
    let last_x = i64::from(source_width - 1);
    let last_y = i64::from(source_height - 1);
    let mut source_y_coordinate = top + scale_y * 0.5;
    for destination_y in 0..height as usize {
        let source_y = (source_y_coordinate.floor() as i64).clamp(0, last_y) as usize;
        let mut source_x_coordinate = left + scale_x * 0.5;
        for destination_x in 0..width as usize {
            let source_x = (source_x_coordinate.floor() as i64).clamp(0, last_x) as usize;
            let source_start = (source_y * source_width as usize + source_x) * 4;
            let destination_start = (destination_y * width as usize + destination_x) * 4;
            output[destination_start..destination_start + 4]
                .copy_from_slice(&input.as_raw()[source_start..source_start + 4]);
            source_x_coordinate += scale_x;
        }
        source_y_coordinate += scale_y;
    }
    RgbaImage::from_raw(width, height, output).ok_or_else(|| {
        PilError::ValueError("resize_f output shape must match its dimensions".into())
    })
}
