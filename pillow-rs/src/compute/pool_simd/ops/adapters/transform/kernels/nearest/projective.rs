//! Projective and quadrilateral nearest sampling for native byte layouts.

use super::super::{
    DynamicImage, PilError, SIMD_F64_LANES, f64x8, native_affine_nearest_transform_channels,
    preserve_mode, simd_unsupported,
};

#[inline(always)]
fn projective_nearest_source_points(
    coefficients: [f64; 8],
    quad: bool,
    destination_x: usize,
    destination_y: usize,
    count: usize,
    destination_width: f64,
    destination_height: f64,
) -> ([f64; SIMD_F64_LANES], [f64; SIMD_F64_LANES]) {
    let [a, b, c, d, e, f, g, h] = coefficients;
    let x_values = std::array::from_fn(|lane| {
        if lane < count {
            (destination_x + lane) as f64
        } else {
            0.0
        }
    });
    let y_values = [destination_y as f64; SIMD_F64_LANES];
    let x = f64x8::new(x_values);
    let y = f64x8::new(y_values);
    if quad {
        let x = x + f64x8::splat(0.5);
        let y = y + f64x8::splat(0.5);
        let inv_width = 1.0 / destination_width;
        let inv_height = 1.0 / destination_height;
        let x1 = (g - a) * inv_width;
        let x2 = (c - a) * inv_height;
        let x3 = (e - c - g + a) * inv_width * inv_height;
        let y1 = (h - b) * inv_width;
        let y2 = (d - b) * inv_height;
        let y3 = (f - d - h + b) * inv_width * inv_height;
        let sx = f64x8::splat(a)
            + f64x8::splat(x1) * x
            + f64x8::splat(x2) * y
            + f64x8::splat(x3) * x * y;
        let sy = f64x8::splat(b)
            + f64x8::splat(y1) * x
            + f64x8::splat(y2) * y
            + f64x8::splat(y3) * x * y;
        (sx.to_array(), sy.to_array())
    } else {
        let x = x + f64x8::splat(0.5);
        let y = y + f64x8::splat(0.5);
        let denominator = f64x8::splat(g) * x + f64x8::splat(h) * y + f64x8::splat(1.0);
        let sx = (f64x8::splat(a) * x + f64x8::splat(b) * y + f64x8::splat(c)) / denominator;
        let sy = (f64x8::splat(d) * x + f64x8::splat(e) * y + f64x8::splat(f)) / denominator;
        (sx.to_array(), sy.to_array())
    }
}

/// Vectorized nearest sampling for perspective and quadrilateral transforms.
/// The homography/quad coordinate equations run in eight `f64` lanes; source
/// address validation and the irregular gathers remain scalar control work.
/// No CPU transform is retried after this adapter has been admitted.
pub(in super::super::super) fn simd_projective_nearest_transform_bytes(
    img: &DynamicImage,
    width: u32,
    height: u32,
    data: &[f64],
    fill: Option<(u8, u8, u8, u8)>,
    mode: Option<&str>,
    quad: bool,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_affine_nearest_transform_channels(img, mode) else {
        return Ok(None);
    };
    let destination_width = usize::try_from(width).map_err(|_| simd_unsupported("Transform"))?;
    let destination_height = usize::try_from(height).map_err(|_| simd_unsupported("Transform"))?;
    let source_width = usize::try_from(img.width()).map_err(|_| simd_unsupported("Transform"))?;
    let source_height = usize::try_from(img.height()).map_err(|_| simd_unsupported("Transform"))?;
    if destination_width == 0
        || destination_height == 0
        || source_width == 0
        || source_height == 0
        || data.len() != 8
    {
        return Ok(None);
    }
    let source_len = source_width
        .checked_mul(source_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Transform"))?;
    let destination_len = destination_width
        .checked_mul(destination_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Transform"))?;
    let source = img.as_bytes();
    if source.len() != source_len {
        return Err(PilError::InternalError(
            "SIMD projective transform source buffer shape mismatch".into(),
        ));
    }
    let fill = fill.unwrap_or((0, 0, 0, 255));
    let fill = [fill.0, fill.1, fill.2, fill.3];
    let [a, b, c, d, e, f, g, h] = data else {
        return Ok(None);
    };
    let coefficients = [*a, *b, *c, *d, *e, *f, *g, *h];
    let mut output = vec![0u8; destination_len];
    let mut vector_blocks = 0u64;
    let destination_width_f = destination_width as f64;
    let destination_height_f = destination_height as f64;
    // Pillow 12.2.0's Geometry.c evaluates projective and quad maps at
    // destination pixel centers and its COORD macro truncates nonnegative
    // coordinates toward zero, returning -1 for any negative coordinate.
    let nearest_coordinate = |value: f64| {
        if !value.is_finite() {
            -1
        } else if value < 0.0 {
            -1
        } else {
            value as i64
        }
    };

    for destination_y in 0..destination_height {
        let mut destination_x = 0usize;
        while destination_x < destination_width {
            let count = (destination_width - destination_x).min(SIMD_F64_LANES);
            let (source_x, source_y) = projective_nearest_source_points(
                coefficients,
                quad,
                destination_x,
                destination_y,
                count,
                destination_width_f,
                destination_height_f,
            );

            for lane in 0..count {
                let output_start = (destination_y * destination_width + destination_x + lane)
                    .checked_mul(channels)
                    .ok_or_else(|| simd_unsupported("Transform"))?;
                let sx = source_x[lane];
                let sy = source_y[lane];
                let input_x = nearest_coordinate(sx);
                let input_y = nearest_coordinate(sy);
                if input_x >= 0
                    && input_x < source_width as i64
                    && input_y >= 0
                    && input_y < source_height as i64
                {
                    let source_start = (input_y as usize * source_width + input_x as usize)
                        .checked_mul(channels)
                        .ok_or_else(|| simd_unsupported("Transform"))?;
                    output[output_start..output_start + channels]
                        .copy_from_slice(&source[source_start..source_start + channels]);
                } else {
                    output[output_start..output_start + channels]
                        .copy_from_slice(&fill[..channels]);
                }
            }
            vector_blocks = vector_blocks.saturating_add(1);
            destination_x += count;
        }
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::image_utils::raw_bytes_to_image(width, height, output, channels)
        .map(|result| Some(preserve_mode(img, result)))
}
