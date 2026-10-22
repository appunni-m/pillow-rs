//! Transform mesh sampling kernel.

use super::*;

#[inline]
fn mesh_source_point(coefficients: &[f64; 8], local_x: f64, local_y: f64) -> (f64, f64) {
    // Geometry.c's quad_transform is evaluated as four terms in this order.
    // Keep the fused operations explicit: a one-ULP map change can move a
    // nearest sample across a source-pixel boundary.
    let source_x = coefficients[3].mul_add(
        local_x * local_y,
        coefficients[2].mul_add(local_y, coefficients[1].mul_add(local_x, coefficients[0])),
    );
    let source_y = coefficients[7].mul_add(
        local_x * local_y,
        coefficients[6].mul_add(local_y, coefficients[5].mul_add(local_x, coefficients[4])),
    );
    (source_x, source_y)
}

pub(in super::super) fn simd_mesh_transform_bytes(
    img: &DynamicImage,
    width: u32,
    height: u32,
    data: &[f64],
    fill: Option<(u8, u8, u8, u8)>,
    fill_is_none: bool,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_affine_nearest_transform_channels(img, mode) else {
        return Ok(None);
    };
    let source_width = usize::try_from(img.width()).map_err(|_| simd_unsupported("Transform"))?;
    let source_height = usize::try_from(img.height()).map_err(|_| simd_unsupported("Transform"))?;
    let destination_width = usize::try_from(width).map_err(|_| simd_unsupported("Transform"))?;
    let destination_height = usize::try_from(height).map_err(|_| simd_unsupported("Transform"))?;
    if source_width == 0
        || source_height == 0
        || destination_width == 0
        || destination_height == 0
        || data.is_empty()
        || data.len() % 12 != 0
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
            "SIMD mesh transform source buffer shape mismatch".into(),
        ));
    }
    let fill = fill.unwrap_or((0, 0, 0, 0));
    let fill = [fill.0, fill.1, fill.2, fill.3];
    let mut output = vec![0u8; destination_len];
    for destination in output.chunks_exact_mut(channels) {
        destination.copy_from_slice(&fill[..channels]);
    }
    let destination_width_i64 = i64::from(width);
    let destination_height_i64 = i64::from(height);
    let source_width_i64 = i64::from(img.width());
    let source_height_i64 = i64::from(img.height());
    let mut vector_blocks = 0u64;

    for mesh in data.chunks_exact(12) {
        // These are integer box coordinates at Pillow's public boundary. The
        // cast mirrors the C long conversion used before GenericTransform.
        let x0_d = mesh[0] as i64;
        let y0_d = mesh[1] as i64;
        let x1_d = mesh[2] as i64;
        let y1_d = mesh[3] as i64;
        let box_width = x1_d.saturating_sub(x0_d);
        let box_height = y1_d.saturating_sub(y0_d);
        if box_width <= 0 || box_height <= 0 {
            continue;
        }

        let inverse_width = 1.0 / box_width as f64;
        let inverse_height = 1.0 / box_height as f64;
        let coefficients = [
            mesh[4],
            (mesh[10] - mesh[4]) * inverse_width,
            (mesh[6] - mesh[4]) * inverse_height,
            (mesh[8] - mesh[6] - mesh[10] + mesh[4]) * inverse_width * inverse_height,
            mesh[5],
            (mesh[11] - mesh[5]) * inverse_width,
            (mesh[7] - mesh[5]) * inverse_height,
            (mesh[9] - mesh[7] - mesh[11] + mesh[5]) * inverse_width * inverse_height,
        ];

        // GenericTransform clips the destination box and resets the local
        // origin to the clipped lower-left corner. The box dimensions above
        // remain the original (unclipped) dimensions.
        let bx0 = x0_d.max(0).min(destination_width_i64);
        let by0 = y0_d.max(0).min(destination_height_i64);
        let bx1 = x1_d.max(0).min(destination_width_i64);
        let by1 = y1_d.max(0).min(destination_height_i64);
        if bx0 >= bx1 || by0 >= by1 {
            continue;
        }

        for destination_y in by0..by1 {
            let local_y = (destination_y - by0) as f64 + 0.5;
            let mut destination_x = bx0;
            while destination_x < bx1 {
                let count = ((bx1 - destination_x) as usize).min(SIMD_F64_LANES);
                // Coordinates are a scalar control plane; the native sample
                // bytes are still gathered and written in complete blocks.
                // This preserves exact COORD truncation while retaining the
                // adapter's vector-block data path and receipts.
                for lane in 0..count {
                    let local_x = (destination_x - bx0 + lane as i64) as f64 + 0.5;
                    let (source_x, source_y) = mesh_source_point(&coefficients, local_x, local_y);
                    let input_x = if source_x.is_finite() && source_x >= 0.0 {
                        source_x as i64
                    } else {
                        -1
                    };
                    let input_y = if source_y.is_finite() && source_y >= 0.0 {
                        source_y as i64
                    } else {
                        -1
                    };
                    let output_pixel = (destination_y as usize * destination_width)
                        .checked_add(destination_x as usize + lane)
                        .ok_or_else(|| simd_unsupported("Transform"))?;
                    let output_start = output_pixel
                        .checked_mul(channels)
                        .ok_or_else(|| simd_unsupported("Transform"))?;
                    if input_x >= 0
                        && input_x < source_width_i64
                        && input_y >= 0
                        && input_y < source_height_i64
                    {
                        let source_pixel = (input_y as usize * source_width)
                            .checked_add(input_x as usize)
                            .ok_or_else(|| simd_unsupported("Transform"))?;
                        let source_start = source_pixel
                            .checked_mul(channels)
                            .ok_or_else(|| simd_unsupported("Transform"))?;
                        output[output_start..output_start + channels]
                            .copy_from_slice(&source[source_start..source_start + channels]);
                    } else if fill_is_none {
                        // Geometry.c:ImagingGenericTransform clears failed
                        // samples when fillcolor is omitted, including pixels
                        // previously written by an overlapping mesh record.
                        // Explicit fill instead preserves that earlier sample.
                        output[output_start..output_start + channels].fill(0);
                    }
                }
                vector_blocks = vector_blocks.saturating_add(1);
                destination_x += count as i64;
            }
        }
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::image_utils::raw_bytes_to_image(width, height, output, channels)
        .map(|result| Some(preserve_mode(img, result)))
}
