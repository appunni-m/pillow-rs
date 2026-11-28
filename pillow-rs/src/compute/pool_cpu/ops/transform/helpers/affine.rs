use super::projective::cubic_sample;
use super::scalar::affine_nearest_fixed;
use crate::error::PilError;

// VERIFY: source=reviewed@2026-11-27; parity=pass@migration-parity-403d8607d0ba40038b9ec2047fcf0561; benchmark=pending
pub(super) fn transform_affine_luma_nearest_into(
    raw: &[u8],
    source_dimensions: (u32, u32),
    dst_w: u32,
    dst_h: u32,
    affine: [f64; 6],
    fill: Option<(u8, u8, u8, u8)>,
    output: &mut [u8],
    validity: Option<&mut [u8]>,
) -> Result<bool, PilError> {
    let Some([step_x, step_y, origin_x, step_x_y, step_y_y, origin_y]) =
        affine_nearest_fixed(affine, dst_w, dst_h)
    else {
        return Ok(false);
    };
    let width = usize::try_from(dst_w)
        .map_err(|_| PilError::ValueError("transform output is too large".into()))?;
    let height = usize::try_from(dst_h)
        .map_err(|_| PilError::ValueError("transform output is too large".into()))?;
    if width == 0 || height == 0 {
        return Ok(false);
    }
    let output_len = width
        .checked_mul(height)
        .ok_or_else(|| PilError::ValueError("transform output is too large".into()))?;
    if output.len() != output_len {
        return Err(PilError::InternalError(
            "Transform L nearest destination buffer shape mismatch".into(),
        ));
    }
    if validity
        .as_ref()
        .is_some_and(|validity| validity.len() != output_len)
    {
        return Err(PilError::InternalError(
            "Transform L nearest validity buffer shape mismatch".into(),
        ));
    }
    let (source_width, source_height) = source_dimensions;
    let source_width = usize::try_from(source_width)
        .map_err(|_| PilError::ValueError("transform source is too large".into()))?;
    let source_height = i64::from(source_height);
    let fill = fill.map_or(0, |color| color.0);
    output.fill(fill);
    let mut validity = validity;

    for (y, row) in output.chunks_exact_mut(width).enumerate() {
        let mut sx = origin_x + y as i64 * step_y;
        let mut sy = origin_y + y as i64 * step_y_y;
        for (x, pixel) in row.iter_mut().enumerate() {
            let ix = sx >> 16;
            let iy = sy >> 16;
            let valid = ix >= 0 && ix < source_width as i64 && iy >= 0 && iy < source_height;
            if valid {
                *pixel = raw[iy as usize * source_width + ix as usize];
            }
            if let Some(validity) = validity.as_deref_mut() {
                validity[y * width + x] = u8::from(valid);
            }
            sx += step_x;
            sy += step_x_y;
        }
    }

    Ok(true)
}

// VERIFY: source=reviewed@2026-11-27; parity=pass@migration-parity-403d8607d0ba40038b9ec2047fcf0561; benchmark=pending
pub(super) fn transform_affine_luma_alpha_nearest_into(
    raw: &[u8],
    source_dimensions: (u32, u32),
    dst_w: u32,
    dst_h: u32,
    affine: [f64; 6],
    fill: Option<(u8, u8, u8, u8)>,
    output: &mut [u8],
    validity: Option<&mut [u8]>,
) -> Result<bool, PilError> {
    let Some([step_x, step_y, origin_x, step_x_y, step_y_y, origin_y]) =
        affine_nearest_fixed(affine, dst_w, dst_h)
    else {
        return Ok(false);
    };
    let width = usize::try_from(dst_w)
        .map_err(|_| PilError::ValueError("transform output is too large".into()))?;
    let height = usize::try_from(dst_h)
        .map_err(|_| PilError::ValueError("transform output is too large".into()))?;
    if width == 0 || height == 0 {
        return Ok(false);
    }
    let output_len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(2))
        .ok_or_else(|| PilError::ValueError("transform output is too large".into()))?;
    if output.len() != output_len {
        return Err(PilError::InternalError(
            "Transform LA nearest destination buffer shape mismatch".into(),
        ));
    }
    if validity
        .as_ref()
        .is_some_and(|validity| validity.len() != output_len / 2)
    {
        return Err(PilError::InternalError(
            "Transform LA nearest validity buffer shape mismatch".into(),
        ));
    }
    let (source_width, source_height) = source_dimensions;
    let source_width = usize::try_from(source_width)
        .map_err(|_| PilError::ValueError("transform source is too large".into()))?;
    let source_height = i64::from(source_height);
    let fill = fill.map_or([0, 0], |color| [color.0, color.1]);
    output.fill(0);
    let mut validity = validity;

    for (y, row) in output.chunks_exact_mut(width * 2).enumerate() {
        let mut sx = origin_x + y as i64 * step_y;
        let mut sy = origin_y + y as i64 * step_y_y;
        for (x, pixel) in row.chunks_exact_mut(2).enumerate() {
            let ix = sx >> 16;
            let iy = sy >> 16;
            let valid = ix >= 0 && ix < source_width as i64 && iy >= 0 && iy < source_height;
            if valid {
                let source_start = (iy as usize * source_width + ix as usize) * 2;
                pixel.copy_from_slice(&raw[source_start..source_start + 2]);
            } else {
                pixel.copy_from_slice(&fill);
            }
            if let Some(validity) = validity.as_deref_mut() {
                validity[y * width + x] = u8::from(valid);
            }
            sx += step_x;
            sy += step_x_y;
        }
    }

    Ok(true)
}

// VERIFY: source=reviewed@2026-11-27; parity=pass@migration-parity-403d8607d0ba40038b9ec2047fcf0561; benchmark=pending
pub(super) fn transform_affine_rgb_nearest_into(
    raw: &[u8],
    source_dimensions: (u32, u32),
    dst_w: u32,
    dst_h: u32,
    affine: [f64; 6],
    fill: Option<(u8, u8, u8, u8)>,
    output: &mut [u8],
    validity: Option<&mut [u8]>,
) -> Result<bool, PilError> {
    let Some([step_x, step_y, origin_x, step_x_y, step_y_y, origin_y]) =
        affine_nearest_fixed(affine, dst_w, dst_h)
    else {
        return Ok(false);
    };
    let width = usize::try_from(dst_w)
        .map_err(|_| PilError::ValueError("transform output is too large".into()))?;
    let height = usize::try_from(dst_h)
        .map_err(|_| PilError::ValueError("transform output is too large".into()))?;
    if width == 0 || height == 0 {
        return Ok(false);
    }
    let output_len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(3))
        .ok_or_else(|| PilError::ValueError("transform output is too large".into()))?;
    if output.len() != output_len {
        return Err(PilError::InternalError(
            "Transform RGB nearest destination buffer shape mismatch".into(),
        ));
    }
    if validity
        .as_ref()
        .is_some_and(|validity| validity.len() != output_len / 3)
    {
        return Err(PilError::InternalError(
            "Transform RGB nearest validity buffer shape mismatch".into(),
        ));
    }
    let (source_width, source_height) = source_dimensions;
    let source_width = usize::try_from(source_width)
        .map_err(|_| PilError::ValueError("transform source is too large".into()))?;
    let source_height = i64::from(source_height);
    let fill = fill.map_or([0, 0, 0], |color| [color.0, color.1, color.2]);
    output.fill(0);
    let mut validity = validity;

    for (y, row) in output.chunks_exact_mut(width * 3).enumerate() {
        let mut sx = origin_x + y as i64 * step_y;
        let mut sy = origin_y + y as i64 * step_y_y;
        for (x, pixel) in row.chunks_exact_mut(3).enumerate() {
            let ix = sx >> 16;
            let iy = sy >> 16;
            let valid = ix >= 0 && ix < source_width as i64 && iy >= 0 && iy < source_height;
            if valid {
                let source_start = (iy as usize * source_width + ix as usize) * 3;
                pixel.copy_from_slice(&raw[source_start..source_start + 3]);
            } else {
                pixel.copy_from_slice(&fill);
            }
            if let Some(validity) = validity.as_deref_mut() {
                validity[y * width + x] = u8::from(valid);
            }
            sx += step_x;
            sy += step_x_y;
        }
    }

    Ok(true)
}

// VERIFY: source=reviewed@2026-11-27; parity=pass@migration-parity-403d8607d0ba40038b9ec2047fcf0561; benchmark=pending
#[inline(always)]
fn transform_affine_fixed_nearest_pixels(
    raw: &[u8],
    output: &mut [u8],
    source_dimensions: (u32, u32),
    destination_dimensions: (u32, u32),
    channels: usize,
    steps: [i64; 6],
    fill_color: (u8, u8, u8, u8),
    validity: Option<&mut [u8]>,
) {
    let (sw, sh) = source_dimensions;
    let (dst_w, dst_h) = destination_dimensions;
    let [a, b, c, d, e, f] = steps;
    let fill = [fill_color.0, fill_color.1, fill_color.2, fill_color.3];
    let mut validity = validity;
    for y in 0..dst_h {
        let mut sx = c + i64::from(y) * b;
        let mut sy = f + i64::from(y) * e;
        for x in 0..dst_w {
            let ix = sx >> 16;
            let iy = sy >> 16;
            let index = (y as usize * dst_w as usize + x as usize) * channels;
            let pixel = &mut output[index..index + channels];
            let valid = ix >= 0 && ix < i64::from(sw) && iy >= 0 && iy < i64::from(sh);
            if valid {
                let source = (iy as usize * sw as usize + ix as usize) * channels;
                pixel.copy_from_slice(&raw[source..source + channels]);
            } else {
                pixel.copy_from_slice(&fill[..channels]);
            }
            if let Some(validity) = validity.as_deref_mut() {
                validity[y as usize * dst_w as usize + x as usize] = u8::from(valid);
            }
            sx += a;
            sy += d;
        }
    }
}

// VERIFY: source=reviewed@2026-11-27; parity=pass@migration-parity-403d8607d0ba40038b9ec2047fcf0561; benchmark=pending
#[inline(always)]
fn transform_affine_generic_pixels(
    raw: &[u8],
    output: &mut [u8],
    source_dimensions: (u32, u32),
    destination_dimensions: (u32, u32),
    affine: [f64; 6],
    channels: usize,
    fill_color: (u8, u8, u8, u8),
    nearest: bool,
    bicubic: bool,
    validity: Option<&mut [u8]>,
) {
    let (dst_w, dst_h) = destination_dimensions;
    let [aff_a, aff_b, aff_c, aff_d, aff_e, aff_f] = affine;
    let mut validity = validity;
    for dy in 0..dst_h {
        for dx in 0..dst_w {
            // Geometry.c's affine_transform() evaluates destination pixel
            // centers before applying the matrix. The nearest filter then
            // truncates non-negative coordinates, while interpolating filters
            // subtract 0.5 inside their own filter prologue.
            // Keep the same grouping as Geometry.c's affine_transform()
            // expression as compiled by clang: the first product is fused
            // with the second product, then the translation is added. A
            // reassociated plain expression can move an edge coordinate by
            // one ULP after an LA/RGBA premultiplied round trip.
            let sx = aff_a.mul_add(dx as f64 + 0.5, aff_b * (dy as f64 + 0.5)) + aff_c;
            let sy = aff_d.mul_add(dx as f64 + 0.5, aff_e * (dy as f64 + 0.5)) + aff_f;
            let out_idx = (dy * dst_w + dx) as usize * channels;
            let valid = transform_affine_generic_pixel(
                raw,
                &mut output[out_idx..out_idx + channels],
                source_dimensions,
                sx,
                sy,
                channels,
                fill_color,
                nearest,
                bicubic,
            );
            if let Some(validity) = validity.as_deref_mut() {
                validity[dy as usize * dst_w as usize + dx as usize] = u8::from(valid);
            }
        }
    }
}

// VERIFY: source=reviewed@2026-11-27; parity=pass@migration-parity-403d8607d0ba40038b9ec2047fcf0561; benchmark=pending
#[inline(always)]
fn transform_affine_generic_pixel(
    raw: &[u8],
    output_pixel: &mut [u8],
    source_dimensions: (u32, u32),
    sx: f64,
    sy: f64,
    channels: usize,
    fill_color: (u8, u8, u8, u8),
    nearest: bool,
    bicubic: bool,
) -> bool {
    let (sw, sh) = source_dimensions;
    if nearest {
        let ix = nearest_sample_coordinate(sx);
        let iy = nearest_sample_coordinate(sy);
        if ix >= 0 && ix < sw as i64 && iy >= 0 && iy < sh as i64 {
            let in_idx = (iy as u32 * sw + ix as u32) as usize * channels;
            output_pixel.copy_from_slice(&raw[in_idx..in_idx + channels]);
            true
        } else {
            write_affine_fill_pixel(output_pixel, channels, fill_color);
            false
        }
    } else if !sx.is_finite() || !sy.is_finite() {
        write_affine_fill_pixel(output_pixel, channels, fill_color);
        false
    } else if sw > 0 && sh > 0 && sx >= 0.0 && sx < sw as f64 && sy >= 0.0 && sy < sh as f64 {
        // Matches Geometry.c's BILINEAR_HEAD(): bounds are checked in center
        // space, then the filter moves to surrounding sample corners.
        let sample_x = sx - 0.5;
        let sample_y = sy - 0.5;
        if bicubic {
            // Pillow's BICUBIC_BODY uses four horizontally clipped taps and a
            // sequential vertical fallback: an out-of-range row repeats the
            // previous row rather than independently clamping every row.
            let x_floor = sample_x.floor() as i64;
            let y_floor = sample_y.floor() as i64;
            let x = x_floor - 1;
            let y = y_floor - 1;
            let fx = sample_x - x_floor as f64;
            let fy = sample_y - y_floor as f64;
            for ch in 0..channels {
                let mut rows = [0.0; 4];
                for row in 0..4 {
                    let yy = y + row as i64;
                    if row == 0 || (yy >= 0 && yy < sh as i64) {
                        let y_index = yy.clamp(0, sh as i64 - 1) as usize;
                        let samples = [
                            raw[(y_index * sw as usize + x.clamp(0, sw as i64 - 1) as usize)
                                * channels
                                + ch] as f64,
                            raw[(y_index * sw as usize + (x + 1).clamp(0, sw as i64 - 1) as usize)
                                * channels
                                + ch] as f64,
                            raw[(y_index * sw as usize + (x + 2).clamp(0, sw as i64 - 1) as usize)
                                * channels
                                + ch] as f64,
                            raw[(y_index * sw as usize + (x + 3).clamp(0, sw as i64 - 1) as usize)
                                * channels
                                + ch] as f64,
                        ];
                        rows[row] = cubic_sample(samples, fx);
                    } else {
                        rows[row] = rows[row - 1];
                    }
                }
                output_pixel[ch] = cubic_sample(rows, fy).clamp(0.0, 255.0) as u8;
            }
        } else {
            let x_floor = sample_x.floor() as i64;
            let y_floor = sample_y.floor() as i64;
            let x0 = x_floor.clamp(0, sw as i64 - 1) as u32;
            let y0 = y_floor.clamp(0, sh as i64 - 1) as u32;
            let x1 = (x_floor + 1).clamp(0, sw as i64 - 1) as u32;
            let y1 = (y_floor + 1).clamp(0, sh as i64 - 1) as u32;
            let fx = sample_x - x_floor as f64;
            let fy = sample_y - y_floor as f64;
            for ch in 0..channels {
                let p00 = raw[(y0 * sw + x0) as usize * channels + ch] as f64;
                let p10 = raw[(y0 * sw + x1) as usize * channels + ch] as f64;
                let p01 = raw[(y1 * sw + x0) as usize * channels + ch] as f64;
                let p11 = raw[(y1 * sw + x1) as usize * channels + ch] as f64;
                // Geometry.c interpolates each row and then interpolates
                // those results; a four-term sum can differ by one ULP.
                let top = (p10 - p00).mul_add(fx, p00);
                let bottom = (p11 - p01).mul_add(fx, p01);
                let value = (bottom - top).mul_add(fy, top);
                // Pillow's byte transform truncates toward zero. Rounding
                // makes exact halfway samples one value too high.
                output_pixel[ch] = value.clamp(0.0, 255.0) as u8;
            }
        }
        true
    } else {
        write_affine_fill_pixel(output_pixel, channels, fill_color);
        false
    }
}

// VERIFY: source=reviewed@2026-11-27; parity=pass@migration-parity-403d8607d0ba40038b9ec2047fcf0561; benchmark=pending
/// Convert a nearest affine coordinate using the result observed from Pillow
/// on the current target architecture.
#[inline(always)]
pub(super) fn nearest_sample_coordinate(value: f64) -> i64 {
    if value.is_nan() {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            return -1;
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        {
            return 0;
        }
    }
    if !value.is_finite() || value < 0.0 {
        -1
    } else {
        value as i64
    }
}

// VERIFY: source=reviewed@2026-11-27; parity=pass@migration-parity-403d8607d0ba40038b9ec2047fcf0561; benchmark=pending
#[inline(always)]
fn write_affine_fill_pixel(output_pixel: &mut [u8], channels: usize, fill_color: (u8, u8, u8, u8)) {
    for ch in 0..channels.min(4) {
        output_pixel[ch] = match ch {
            0 => fill_color.0,
            1 => fill_color.1,
            2 => fill_color.2,
            _ => fill_color.3,
        };
    }
}

/// Apply an affine transform working on the native number of channels.
/// When `nearest` is true, uses nearest-neighbor sampling.
pub(super) fn transform_affine_generic(
    raw: &[u8],
    source_dimensions: (u32, u32),
    dst_w: u32,
    dst_h: u32,
    aff_a: f64,
    aff_b: f64,
    aff_c: f64,
    aff_d: f64,
    aff_e: f64,
    aff_f: f64,
    fill: Option<(u8, u8, u8, u8)>,
    channels: usize,
    nearest: bool,
    bicubic: bool,
) -> Result<Vec<u8>, PilError> {
    let output_len = usize::try_from(dst_w)
        .ok()
        .and_then(|width| {
            usize::try_from(dst_h)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| PilError::ValueError("image dimensions are too large".into()))?;
    let mut output = vec![0u8; output_len];
    transform_affine_generic_into(
        raw,
        source_dimensions,
        dst_w,
        dst_h,
        aff_a,
        aff_b,
        aff_c,
        aff_d,
        aff_e,
        aff_f,
        fill,
        channels,
        nearest,
        bicubic,
        &mut output,
    )?;
    Ok(output)
}

/// Apply the generic affine sampler into a caller-owned native output buffer.
// VERIFY: source=reviewed@2026-11-27; parity=pass@migration-parity-403d8607d0ba40038b9ec2047fcf0561; benchmark=pending
pub(super) fn transform_affine_generic_into(
    raw: &[u8],
    source_dimensions: (u32, u32),
    dst_w: u32,
    dst_h: u32,
    aff_a: f64,
    aff_b: f64,
    aff_c: f64,
    aff_d: f64,
    aff_e: f64,
    aff_f: f64,
    fill: Option<(u8, u8, u8, u8)>,
    channels: usize,
    nearest: bool,
    bicubic: bool,
    out: &mut [u8],
) -> Result<(), PilError> {
    transform_affine_generic_into_with_validity(
        raw,
        source_dimensions,
        dst_w,
        dst_h,
        aff_a,
        aff_b,
        aff_c,
        aff_d,
        aff_e,
        aff_f,
        fill,
        channels,
        nearest,
        bicubic,
        out,
        None,
    )
}

// VERIFY: source=reviewed@2026-11-27; parity=pass@migration-parity-403d8607d0ba40038b9ec2047fcf0561; benchmark=pending
pub(super) fn transform_affine_generic_into_with_validity(
    raw: &[u8],
    source_dimensions: (u32, u32),
    dst_w: u32,
    dst_h: u32,
    aff_a: f64,
    aff_b: f64,
    aff_c: f64,
    aff_d: f64,
    aff_e: f64,
    aff_f: f64,
    fill: Option<(u8, u8, u8, u8)>,
    channels: usize,
    nearest: bool,
    bicubic: bool,
    out: &mut [u8],
    validity: Option<&mut [u8]>,
) -> Result<(), PilError> {
    let (sw, sh) = source_dimensions;
    let output_len = usize::try_from(dst_w)
        .ok()
        .and_then(|width| {
            usize::try_from(dst_h)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| PilError::ValueError("image dimensions are too large".into()))?;
    if out.len() != output_len {
        return Err(PilError::InternalError(
            "Transform affine destination buffer shape mismatch".into(),
        ));
    }
    if channels == 0
        || validity
            .as_ref()
            .is_some_and(|validity| validity.len() != output_len / channels)
    {
        return Err(PilError::InternalError(
            "Transform affine validity buffer shape mismatch".into(),
        ));
    }
    // Both legacy fill defaults have zero in every stored channel. The mode
    // owner supplies the exact channel count and output identity.
    let fill_color = fill.unwrap_or((0, 0, 0, 0));
    let affine = [aff_a, aff_b, aff_c, aff_d, aff_e, aff_f];

    if nearest && let Some(steps) = affine_nearest_fixed(affine, dst_w, dst_h) {
        transform_affine_fixed_nearest_pixels(
            raw,
            out,
            (sw, sh),
            (dst_w, dst_h),
            channels,
            steps,
            fill_color,
            validity,
        );
        return Ok(());
    }

    transform_affine_generic_pixels(
        raw,
        out,
        (sw, sh),
        (dst_w, dst_h),
        affine,
        channels,
        fill_color,
        nearest,
        bicubic,
        validity,
    );

    Ok(())
}
