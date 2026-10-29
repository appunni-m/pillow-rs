//! Native byte nearest-neighbor Rotate kernel.

use super::super::super as rotate;
use super::super::super::super::SIMD_BYTE_VECTOR_LANES;
use super::super::super::{native_rotate_layout, simd_rotate_geometry};
use crate::error::PilError;
use crate::raster::DynamicImage;

#[inline]
fn nearest_rotate_fixed_coordinates(affine: [f64; 6]) -> [i64; 6] {
    let fixed = |value: f64| (value.mul_add(65_536.0, 0.5).floor()) as i64;
    let [a, b, c, d, e, f] = affine;
    [
        fixed(a),
        fixed(b),
        fixed(c + a * 0.5 + b * 0.5),
        fixed(d),
        fixed(e),
        fixed(f + d * 0.5 + e * 0.5),
    ]
}

/// Execute the exact nearest-neighbor affine contract for ordinary byte
/// layouts. Coordinate progression follows Pillow's signed 16.16 affine
/// sampler; each complete group of pixels is gathered into native `u8x16`
/// blocks and written without an RGBA expansion.
pub(in crate::compute::pool_simd::ops::adapters::rotate) fn simd_nearest_rotate_native(
    img: &DynamicImage,
    angle: f64,
    expand: bool,
    fill: Option<(u8, u8, u8, u8)>,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_rotate_layout(img, mode) else {
        return Ok(None);
    };
    let Some(geometry) =
        simd_rotate_geometry(img.width(), img.height(), angle, expand, center, translate)
    else {
        return Ok(None);
    };
    let width = img.width() as usize;
    let height = img.height() as usize;
    let destination_width = geometry.width as usize;
    let destination_height = geometry.height as usize;
    let source_len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| PilError::ValueError("SIMD rotate source dimensions overflow".into()))?;
    let destination_len = destination_width
        .checked_mul(destination_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| PilError::ValueError("SIMD rotate output dimensions overflow".into()))?;
    if destination_width == 0 || destination_height == 0 {
        return Ok(None);
    }
    let source = img.as_bytes();
    if source.len() != source_len {
        return Err(PilError::InternalError(
            "SIMD rotate source buffer shape mismatch".into(),
        ));
    }
    let mut output = vec![0u8; destination_len];
    let fill = fill.unwrap_or((0, 0, 0, 0));
    let [step_x_x, step_y_x, origin_x, step_x_y, step_y_y, origin_y] =
        nearest_rotate_fixed_coordinates(geometry.affine);
    let pixels_per_vector = (SIMD_BYTE_VECTOR_LANES / channels).max(1);
    let block_bytes = pixels_per_vector * channels;
    // Small positive angles keep nearest samples in one source row long
    // enough to copy native RGB spans. Other affine slopes retain the gather
    // kernel below, so this run detector does not tax arbitrary-angle calls.
    #[cfg(not(feature = "parallel"))]
    if channels == 3
        && step_x_x >= (65_536_i64 * 9 / 10)
        && step_x_x <= (65_536_i64 * 11 / 10)
        && (0_i64..=65_536_i64 / 8).contains(&step_x_y)
        && let Some((vector_blocks, scalar_tail)) = rotate::copy_contiguous_rgb_runs(
            source,
            &mut output,
            width,
            height,
            destination_width,
            destination_height,
            origin_x,
            origin_y,
            step_x_x,
            step_y_x,
            step_x_y,
            step_y_y,
            [fill.0, fill.1, fill.2],
        )
    {
        return rotate::finish_native_rotation(
            img,
            geometry.width,
            geometry.height,
            output,
            channels,
            vector_blocks,
            scalar_tail,
            "native-copy",
        );
    }

    let (vector_blocks, scalar_tail) = rotate::copy_nearest_affine_pixels(
        source,
        &mut output,
        width,
        height,
        destination_width,
        destination_height,
        channels,
        pixels_per_vector,
        block_bytes,
        origin_x,
        origin_y,
        step_x_x,
        step_y_x,
        step_x_y,
        step_y_y,
        fill,
    );

    rotate::finish_native_rotation(
        img,
        geometry.width,
        geometry.height,
        output,
        channels,
        vector_blocks,
        scalar_tail,
        "vector",
    )
}
