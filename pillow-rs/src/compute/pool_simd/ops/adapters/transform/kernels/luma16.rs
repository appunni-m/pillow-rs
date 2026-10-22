//! Transform Luma16 sampling kernel.

use super::{
    DynamicImage, ImageBuffer, Luma, PilError, ResampleFilter, SIMD_F64_LANES, TransformMethod,
    f64x8, native_affine_luma16_transform_supported_for_image, rotate_fused_mul_add,
    simd_affine_bilinear_transform_bytes, u16x8,
};

pub(in super::super) fn simd_affine_luma16_transform_bytes(
    img: &DynamicImage,
    width: u32,
    height: u32,
    data: &[f64],
    fill: Option<(u8, u8, u8, u8)>,
    filter: ResampleFilter,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    if !native_affine_luma16_transform_supported_for_image(
        img,
        width,
        height,
        &TransformMethod::Affine,
        data,
        filter,
        mode,
    ) {
        return Ok(None);
    }
    let DynamicImage::ImageLuma16(source) = img else {
        return Ok(None);
    };
    if matches!(filter, ResampleFilter::Bilinear) {
        // SPECIAL filtered transforms sample the first width bytes of each
        // native row. Reuse the exact byte SIMD kernel, then restore the
        // untouched second fill byte instead of filtering numeric u16 values.
        let big_endian = mode == Some("I;16B")
            || (cfg!(target_endian = "big") && matches!(mode, Some("I;16" | "I;16N")));
        let encode = |value: u16| {
            if big_endian {
                value.to_be_bytes()
            } else {
                value.to_le_bytes()
            }
        };
        let fill_value = fill.map_or(0, |color| u16::from_le_bytes([color.0, color.1]));
        let fill_bytes = encode(fill_value);
        let mut bytes = Vec::with_capacity(source.width() as usize * source.height() as usize);
        for row in source.as_raw().chunks_exact(source.width() as usize) {
            bytes.extend(
                row.iter()
                    .flat_map(|&value| encode(value))
                    .take(source.width() as usize),
            );
        }
        let plane = DynamicImage::ImageLuma8(
            ImageBuffer::from_raw(source.width(), source.height(), bytes)
                .ok_or_else(|| PilError::InternalError("SIMD I;16 byte plane shape".into()))?,
        );
        let Some(sampled) = simd_affine_bilinear_transform_bytes(
            &plane,
            width,
            height,
            data,
            Some((fill_bytes[0], 0, 0, 0)),
            Some("L"),
        )?
        else {
            return Ok(None);
        };
        let values = sampled
            .as_bytes()
            .iter()
            .map(|&byte| {
                let bytes = [byte, fill_bytes[1]];
                if big_endian {
                    u16::from_be_bytes(bytes)
                } else {
                    u16::from_le_bytes(bytes)
                }
            })
            .collect();
        return ImageBuffer::from_raw(width, height, values)
            .map(DynamicImage::ImageLuma16)
            .map(Some)
            .ok_or_else(|| PilError::InternalError("SIMD I;16 transform output shape".into()));
    }
    let source_width = source.width() as usize;
    let source_height = source.height() as usize;
    let destination_width = width as usize;
    let destination_height = height as usize;
    let [a, b, c, d, e, f] = data else {
        return Ok(None);
    };
    let fill = fill.map_or(0, |color| u16::from_le_bytes([color.0, color.1]));
    let mut output = Vec::with_capacity(destination_width * destination_height);
    let mut vector_blocks = 0u64;

    for destination_y in 0..destination_height {
        let mut destination_x = 0usize;
        while destination_x < destination_width {
            let count = (destination_width - destination_x).min(SIMD_F64_LANES);
            let x = f64x8::new(std::array::from_fn(|lane| {
                if lane < count {
                    (destination_x + lane) as f64 + 0.5
                } else {
                    0.0
                }
            }));
            let y = f64x8::splat(destination_y as f64 + 0.5);
            let source_x = (rotate_fused_mul_add(f64x8::splat(*a), x, f64x8::splat(*b) * y)
                + f64x8::splat(*c))
            .to_array();
            let source_y = (rotate_fused_mul_add(f64x8::splat(*d), x, f64x8::splat(*e) * y)
                + f64x8::splat(*f))
            .to_array();
            let mut values = [fill; SIMD_F64_LANES];
            for lane in 0..count {
                let input_x = if source_x[lane].is_finite() {
                    source_x[lane].floor() as i64
                } else {
                    -1
                };
                let input_y = if source_y[lane].is_finite() {
                    source_y[lane].floor() as i64
                } else {
                    -1
                };
                if input_x >= 0
                    && input_x < source_width as i64
                    && input_y >= 0
                    && input_y < source_height as i64
                {
                    values[lane] = source.get_pixel(input_x as u32, input_y as u32)[0];
                }
            }
            output.extend_from_slice(&u16x8::new(values).to_array()[..count]);
            vector_blocks = vector_blocks.saturating_add(1);
            destination_x += count;
        }
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    ImageBuffer::<Luma<u16>, Vec<u16>>::from_raw(width, height, output)
        .map(DynamicImage::ImageLuma16)
        .map(Some)
        .ok_or_else(|| PilError::InternalError("SIMD transform I;16 buffer shape mismatch".into()))
}
