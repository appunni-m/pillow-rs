//! Projective and quadrilateral bilinear sampling for native byte layouts.

use super::super::{
    DynamicImage, PilError, SIMD_F64_LANES, f64x8, native_affine_nearest_transform_channels,
    preserve_mode, simd_unsupported,
};

#[inline(always)]
fn projective_bilinear_source_points(
    coefficients: [f64; 8],
    quad: bool,
    destination_x: usize,
    destination_y: usize,
    count: usize,
    inverse_width: f64,
    inverse_height: f64,
) -> ([f64; SIMD_F64_LANES], [f64; SIMD_F64_LANES]) {
    let [a, b, c, d, e, f, g, h] = coefficients;
    let x = f64x8::new(std::array::from_fn(|lane| {
        if lane < count {
            (destination_x + lane) as f64
        } else {
            0.0
        }
    }));
    let y = f64x8::splat(destination_y as f64);
    if quad {
        let source_x = f64x8::splat(a)
            + f64x8::splat(g - a) * x * f64x8::splat(inverse_width)
            + f64x8::splat(c - a) * y * f64x8::splat(inverse_height)
            + f64x8::splat(e - c - g + a) * x * y * f64x8::splat(inverse_width * inverse_height);
        let source_y = f64x8::splat(b)
            + f64x8::splat(h - b) * x * f64x8::splat(inverse_width)
            + f64x8::splat(d - b) * y * f64x8::splat(inverse_height)
            + f64x8::splat(f - d - h + b) * x * y * f64x8::splat(inverse_width * inverse_height);
        (source_x.to_array(), source_y.to_array())
    } else {
        let denominator = f64x8::splat(g) * x + f64x8::splat(h) * y + f64x8::splat(1.0);
        let source_x = (f64x8::splat(a) * x + f64x8::splat(b) * y + f64x8::splat(c)) / denominator;
        let source_y = (f64x8::splat(d) * x + f64x8::splat(e) * y + f64x8::splat(f)) / denominator;
        (source_x.to_array(), source_y.to_array())
    }
}

#[inline(always)]
fn projective_bilinear_neighborhoods(
    source: &[u8],
    source_width: usize,
    source_height: usize,
    channels: usize,
    fill: [u8; 4],
    count: usize,
    source_x: [f64; SIMD_F64_LANES],
    source_y: [f64; SIMD_F64_LANES],
) -> (
    [f64; SIMD_F64_LANES],
    [f64; SIMD_F64_LANES],
    [[[f64; SIMD_F64_LANES]; 4]; 4],
) {
    let mut fx_values = [0.0; SIMD_F64_LANES];
    let mut fy_values = [0.0; SIMD_F64_LANES];
    let mut neighbors = [[[0.0; SIMD_F64_LANES]; 4]; 4];
    for channel in 0..channels {
        for neighbor in 0..4 {
            for lane in 0..count {
                neighbors[channel][neighbor][lane] = fill[channel] as f64;
            }
        }
    }
    for lane in 0..count {
        let sx = source_x[lane];
        let sy = source_y[lane];
        if !sx.is_finite()
            || !sy.is_finite()
            || sx < 0.0
            || sx >= source_width as f64
            || sy < 0.0
            || sy >= source_height as f64
        {
            continue;
        }
        let x0 = sx.floor() as usize;
        let y0 = sy.floor() as usize;
        let x1 = (x0 + 1).min(source_width - 1);
        let y1 = (y0 + 1).min(source_height - 1);
        fx_values[lane] = sx - x0 as f64;
        fy_values[lane] = sy - y0 as f64;
        let coordinates = [(x0, y0), (x1, y0), (x0, y1), (x1, y1)];
        for (neighbor, &(source_x, source_y)) in coordinates.iter().enumerate() {
            let source_start = (source_y * source_width + source_x) * channels;
            for channel in 0..channels {
                neighbors[channel][neighbor][lane] = source[source_start + channel] as f64;
            }
        }
    }
    (fx_values, fy_values, neighbors)
}

/// Vectorized bilinear sampling for perspective and quadrilateral transforms.
///
/// The inverse-map equations and four-neighbour interpolation run in
/// `f64x8` blocks. Source addresses are irregular gathers, so their bounds
/// checks and byte loads remain scalar control work; they are packed into the
/// vector lanes before interpolation. Invalid coordinates use Pillow's fill
/// sample and never trigger a CPU retry.
pub(in super::super::super) fn simd_projective_bilinear_transform_bytes(
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
            "SIMD projective bilinear source buffer shape mismatch".into(),
        ));
    }
    let fill = fill.unwrap_or((0, 0, 0, 255));
    let fill = [fill.0, fill.1, fill.2, fill.3];
    let [a, b, c, d, e, f, g, h] = data else {
        return Ok(None);
    };
    let coefficients = [*a, *b, *c, *d, *e, *f, *g, *h];
    let inverse_width = 1.0 / destination_width as f64;
    let inverse_height = 1.0 / destination_height as f64;
    let mut output = vec![0u8; destination_len];
    let mut vector_blocks = 0u64;
    let one = f64x8::splat(1.0);

    for destination_y in 0..destination_height {
        let mut destination_x = 0usize;
        while destination_x < destination_width {
            let count = (destination_width - destination_x).min(SIMD_F64_LANES);
            let (source_x, source_y) = projective_bilinear_source_points(
                coefficients,
                quad,
                destination_x,
                destination_y,
                count,
                inverse_width,
                inverse_height,
            );
            let (fx_values, fy_values, neighbors) = projective_bilinear_neighborhoods(
                source,
                source_width,
                source_height,
                channels,
                fill,
                count,
                source_x,
                source_y,
            );

            let fx = f64x8::new(fx_values);
            let fy = f64x8::new(fy_values);
            let mut block = [0u8; SIMD_F64_LANES * 4];
            for channel in 0..channels {
                let p00 = f64x8::new(neighbors[channel][0]);
                let p10 = f64x8::new(neighbors[channel][1]);
                let p01 = f64x8::new(neighbors[channel][2]);
                let p11 = f64x8::new(neighbors[channel][3]);
                let interpolated =
                    (one - fx) * ((one - fy) * p00 + fy * p01) + fx * ((one - fy) * p10 + fy * p11);
                let values = interpolated
                    .to_array()
                    .map(|value| value.clamp(0.0, 255.0).round() as u8);
                for lane in 0..count {
                    block[lane * channels + channel] = values[lane];
                }
            }
            let output_start = (destination_y * destination_width + destination_x) * channels;
            let output_bytes = count * channels;
            output[output_start..output_start + output_bytes]
                .copy_from_slice(&block[..output_bytes]);
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
