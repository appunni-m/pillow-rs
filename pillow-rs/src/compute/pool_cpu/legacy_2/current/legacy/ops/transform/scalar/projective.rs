use super::{DynamicImage, PilError, ResampleFilter, RgbaImage, sample_transform_scalar32};
pub(in crate::compute::pool_cpu::ops::transform) fn transform_projective_scalar32<
    const IS_FLOAT: bool,
>(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    data: &[f64],
    filter: &ResampleFilter,
    fill: Option<(u8, u8, u8, u8)>,
    _fill_is_none: bool,
    quad: bool,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageRgba8(rgba) = img else {
        return Err(PilError::InternalError(
            "Scalar32 projective transform requires four-byte storage".into(),
        ));
    };
    let (src_w, src_h) = rgba.dimensions();
    let source_len = usize::try_from(src_w)
        .ok()
        .and_then(|width| {
            usize::try_from(src_h)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| PilError::ValueError("image dimensions are too large".into()))?;
    let output_len = usize::try_from(dst_w)
        .ok()
        .and_then(|width| {
            usize::try_from(dst_h)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| PilError::ValueError("image dimensions are too large".into()))?;
    let source = rgba.as_raw();
    if source.len() / 4 != source_len {
        return Err(PilError::InternalError(
            "Scalar32 projective source buffer shape mismatch".into(),
        ));
    }
    let fill_word = fill
        .map(|color| [color.0, color.1, color.2, color.3])
        .unwrap_or([0; 4]);
    let _ = output_len
        .checked_mul(4)
        .ok_or_else(|| PilError::ValueError("image dimensions are too large".into()))?;
    let mut output = fill_word.repeat(output_len);

    let quad_coefficients = if quad {
        if dst_w == 0 || dst_h == 0 {
            None
        } else {
            let x0 = data[0];
            let y0 = data[1];
            let inverse_width = 1.0 / f64::from(dst_w);
            let inverse_height = 1.0 / f64::from(dst_h);
            Some([
                x0,
                (data[6] - x0) * inverse_width,
                (data[2] - x0) * inverse_height,
                (data[4] - data[2] - data[6] + x0) * inverse_width * inverse_height,
                y0,
                (data[7] - y0) * inverse_width,
                (data[3] - y0) * inverse_height,
                (data[5] - data[3] - data[7] + y0) * inverse_width * inverse_height,
            ])
        }
    } else {
        None
    };
    let source_at = |dx: f64, dy: f64| -> Option<(f64, f64)> {
        let xin = dx + 0.5;
        let yin = dy + 0.5;
        let (sx, sy) = if let Some(coefficients) = quad_coefficients {
            let sx_linear = coefficients[1].mul_add(xin, coefficients[0]);
            let sx_linear = coefficients[2].mul_add(yin, sx_linear);
            let sx = (coefficients[3] * xin).mul_add(yin, sx_linear);
            let sy_linear = coefficients[5].mul_add(xin, coefficients[4]);
            let sy_linear = coefficients[6].mul_add(yin, sy_linear);
            let sy = (coefficients[7] * xin).mul_add(yin, sy_linear);
            (sx, sy)
        } else {
            let denominator = data[6].mul_add(xin, data[7] * yin) + 1.0;
            let sx = (data[0].mul_add(xin, data[1] * yin) + data[2]) / denominator;
            let sy = (data[3].mul_add(xin, data[4] * yin) + data[5]) / denominator;
            (sx, sy)
        };
        (sx.is_finite() && sy.is_finite()).then_some((sx, sy))
    };

    for dy in 0..dst_h {
        for dx in 0..dst_w {
            let output_index = (dy as usize) * dst_w as usize + dx as usize;
            if let Some((sx, sy)) = source_at(f64::from(dx), f64::from(dy))
                && sx >= 0.0
                && sx < f64::from(src_w)
                && sy >= 0.0
                && sy < f64::from(src_h)
            {
                output[output_index * 4..output_index * 4 + 4].copy_from_slice(
                    &sample_transform_scalar32::<IS_FLOAT>(source, src_w, src_h, sx, sy, filter),
                );
            }
        }
    }

    let out = RgbaImage::from_raw(dst_w, dst_h, output).ok_or_else(|| {
        PilError::InternalError("Scalar32 projective output shape mismatch".into())
    })?;
    Ok(DynamicImage::ImageRgba8(out))
}

/// Apply Pillow's typed 32-bit filters to a piecewise quadrilateral mesh.
#[inline]
fn transform_mesh_scalar32_record<const IS_FLOAT: bool>(
    source: &[u8],
    source_width: u32,
    source_height: u32,
    source_width_f64: f64,
    source_height_f64: f64,
    destination_width: u32,
    destination_height: u32,
    mesh: &[f64],
    output: &mut [u8],
    fill_is_none: bool,
    filter: &ResampleFilter,
) {
    let x0_d = mesh[0] as i64;
    let y0_d = mesh[1] as i64;
    let x1_d = mesh[2] as i64;
    let y1_d = mesh[3] as i64;
    let width = x1_d.saturating_sub(x0_d);
    let height = y1_d.saturating_sub(y0_d);
    if width <= 0 || height <= 0 {
        return;
    }
    let inverse_width = 1.0 / width as f64;
    let inverse_height = 1.0 / height as f64;
    let x0_s = mesh[4];
    let y0_s = mesh[5];
    let x1_s = mesh[6];
    let y1_s = mesh[7];
    let x2_s = mesh[8];
    let y2_s = mesh[9];
    let x3_s = mesh[10];
    let y3_s = mesh[11];
    let coefficients = [
        x0_s,
        (x3_s - x0_s) * inverse_width,
        (x1_s - x0_s) * inverse_height,
        (x2_s - x1_s - x3_s + x0_s) * inverse_width * inverse_height,
        y0_s,
        (y3_s - y0_s) * inverse_width,
        (y1_s - y0_s) * inverse_height,
        (y2_s - y1_s - y3_s + y0_s) * inverse_width * inverse_height,
    ];
    let x0 = x0_d.max(0).min(i64::from(destination_width));
    let y0 = y0_d.max(0).min(i64::from(destination_height));
    let x1 = x1_d.max(0).min(i64::from(destination_width));
    let y1 = y1_d.max(0).min(i64::from(destination_height));
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    for destination_y in y0..y1 {
        let local_y = (destination_y - y0) as f64 + 0.5;
        for destination_x in x0..x1 {
            let local_x = (destination_x - x0) as f64 + 0.5;
            let sx_linear =
                coefficients[2].mul_add(local_y, coefficients[1].mul_add(local_x, coefficients[0]));
            let sx = (coefficients[3] * local_x).mul_add(local_y, sx_linear);
            let sy_linear =
                coefficients[6].mul_add(local_y, coefficients[5].mul_add(local_x, coefficients[4]));
            let sy = (coefficients[7] * local_x).mul_add(local_y, sy_linear);
            let output_index =
                destination_y as usize * destination_width as usize + destination_x as usize;
            if !sx.is_finite()
                || !sy.is_finite()
                || sx < 0.0
                || sx >= source_width_f64
                || sy < 0.0
                || sy >= source_height_f64
            {
                // ImagingGenericTransform clears a failed sample when
                // fillcolor is omitted; with an explicit fill the output was
                // initialized to that word and must retain a prior sample.
                if fill_is_none {
                    output[output_index * 4..output_index * 4 + 4].fill(0);
                }
                continue;
            }
            output[output_index * 4..output_index * 4 + 4].copy_from_slice(
                &sample_transform_scalar32::<IS_FLOAT>(
                    source,
                    source_width,
                    source_height,
                    sx,
                    sy,
                    filter,
                ),
            );
        }
    }
}

pub(in crate::compute::pool_cpu::ops::transform) fn transform_mesh_scalar32<
    const IS_FLOAT: bool,
>(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    mesh_data: &[f64],
    fill: Option<(u8, u8, u8, u8)>,
    fill_is_none: bool,
    filter: &ResampleFilter,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageRgba8(rgba) = img else {
        return Err(PilError::InternalError(
            "Scalar32 mesh transform requires four-byte storage".into(),
        ));
    };
    let (src_w, src_h) = rgba.dimensions();
    let source_len = usize::try_from(src_w)
        .ok()
        .and_then(|width| {
            usize::try_from(src_h)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| PilError::ValueError("image dimensions are too large".into()))?;
    let output_len = usize::try_from(dst_w)
        .ok()
        .and_then(|width| {
            usize::try_from(dst_h)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| PilError::ValueError("image dimensions are too large".into()))?;
    let source = rgba.as_raw();
    if source.len() / 4 != source_len {
        return Err(PilError::InternalError(
            "Scalar32 mesh source buffer shape mismatch".into(),
        ));
    }
    let fill_word = fill
        .map(|color| [color.0, color.1, color.2, color.3])
        .unwrap_or([0; 4]);
    let _ = output_len
        .checked_mul(4)
        .ok_or_else(|| PilError::ValueError("image dimensions are too large".into()))?;
    let mut output = fill_word.repeat(output_len);
    let sw_f = f64::from(src_w);
    let sh_f = f64::from(src_h);

    for mesh in mesh_data.chunks_exact(12) {
        transform_mesh_scalar32_record::<IS_FLOAT>(
            source,
            src_w,
            src_h,
            sw_f,
            sh_f,
            dst_w,
            dst_h,
            mesh,
            &mut output,
            fill_is_none,
            filter,
        );
    }

    let out = RgbaImage::from_raw(dst_w, dst_h, output)
        .ok_or_else(|| PilError::InternalError("Scalar32 mesh output shape mismatch".into()))?;
    Ok(DynamicImage::ImageRgba8(out))
}
