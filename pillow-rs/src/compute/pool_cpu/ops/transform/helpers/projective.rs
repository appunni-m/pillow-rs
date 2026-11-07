use crate::error::PilError;
use crate::pipeline::ResampleFilter;

/// Apply a projective or quadrilateral transform to native 8-bit samples.
///
/// Pillow represents perspective transforms as an inverse homography and
/// quadrilateral transforms as a bilinear map from output coordinates to the
/// four source corners. QUAD coefficients are prepared once at the public
/// boundary and remain in that form in the lazy pipeline.
#[inline(always)]
fn projective_source_at(
    destination: (u32, u32),
    data: &[f64],
    quad: bool,
    dx: f64,
    dy: f64,
) -> Option<(f64, f64)> {
    let (dst_w, dst_h) = destination;
    if quad {
        if dst_w == 0 || dst_h == 0 {
            return None;
        }
        // QUAD data are already the (x0, x1, x2, x3, y0, y1, y2, y3)
        // coefficients prepared from the public NW/SW/SE/NE corners.
        let dx = dx + 0.5;
        let dy = dy + 0.5;
        let x = data[0] + data[1] * dx + data[2] * dy + data[3] * dx * dy;
        let y = data[4] + data[5] * dx + data[6] * dy + data[7] * dx * dy;
        return Some((x, y));
    }

    // Pillow 12.2.0's libImaging/Geometry.c `perspective_transform` evaluates
    // the inverse map at each destination pixel center.
    let dx = dx + 0.5;
    let dy = dy + 0.5;
    // Keep Geometry.c's fused grouping: the first product is fused with the
    // second product, then the constant term is added. Reassociation can move
    // an edge coordinate by one ULP after an LA/RGBA premultiplied round trip.
    let denominator = data[6].mul_add(dx, data[7] * dy) + 1.0;
    if denominator == 0.0 || !denominator.is_finite() {
        return None;
    }
    let x = (data[0].mul_add(dx, data[1] * dy) + data[2]) / denominator;
    let y = (data[3].mul_add(dx, data[4] * dy) + data[5]) / denominator;
    (x.is_finite() && y.is_finite()).then_some((x, y))
}

#[inline(always)]
fn projective_fill_sample(destination: &mut [u8], fill_color: (u8, u8, u8, u8)) {
    for (channel, value) in destination.iter_mut().enumerate() {
        *value = match channel {
            0 => fill_color.0,
            1 => fill_color.1,
            2 => fill_color.2,
            _ => fill_color.3,
        };
    }
}

#[inline(always)]
fn projective_nearest_coordinate(value: f64) -> i64 {
    if value < 0.0 { -1 } else { value as i64 }
}

#[inline(always)]
fn transform_projective_sample(
    raw: &[u8],
    destination: &mut [u8],
    source_dimensions: (u32, u32),
    channels: usize,
    fill_color: (u8, u8, u8, u8),
    nearest: bool,
    filter: &ResampleFilter,
    sx: f64,
    sy: f64,
) {
    let (src_w, src_h) = source_dimensions;
    if nearest {
        // Geometry.c defines COORD() as -1 for negative values and a C cast
        // otherwise. Maps were evaluated at destination pixel centers above.
        let ix = projective_nearest_coordinate(sx);
        let iy = projective_nearest_coordinate(sy);
        if ix >= 0 && ix < i64::from(src_w) && iy >= 0 && iy < i64::from(src_h) {
            let in_idx = (iy as usize * src_w as usize + ix as usize) * channels;
            destination.copy_from_slice(&raw[in_idx..in_idx + channels]);
        } else {
            projective_fill_sample(destination, fill_color);
        }
        return;
    }

    if src_w == 0
        || src_h == 0
        || sx < 0.0
        || sx >= f64::from(src_w)
        || sy < 0.0
        || sy >= f64::from(src_h)
    {
        projective_fill_sample(destination, fill_color);
        return;
    }
    // Geometry.c checks the unshifted coordinate, subtracts 0.5, then clips
    // the sample window. Horizontal interpolation precedes vertical, and the
    // final UINT8 cast truncates toward zero.
    let sample_x = sx - 0.5;
    let sample_y = sy - 0.5;
    if matches!(filter, ResampleFilter::Bicubic) {
        // Pillow's BICUBIC callback uses four-tap Horner form with the same
        // fused operations as `cubic_sample`.
        let floor_x = sample_x.floor() as i64 - 1;
        let floor_y = sample_y.floor() as i64 - 1;
        let fx = sample_x - (floor_x + 1) as f64;
        let fy = sample_y - (floor_y + 1) as f64;
        for channel in 0..channels {
            let mut rows = [0.0; 4];
            for (row, output) in rows.iter_mut().enumerate() {
                let y = (floor_y + row as i64).clamp(0, i64::from(src_h - 1)) as usize;
                let samples = [
                    raw[(y * src_w as usize + floor_x.clamp(0, i64::from(src_w - 1)) as usize)
                        * channels
                        + channel] as f64,
                    raw[(y * src_w as usize
                        + (floor_x + 1).clamp(0, i64::from(src_w - 1)) as usize)
                        * channels
                        + channel] as f64,
                    raw[(y * src_w as usize
                        + (floor_x + 2).clamp(0, i64::from(src_w - 1)) as usize)
                        * channels
                        + channel] as f64,
                    raw[(y * src_w as usize
                        + (floor_x + 3).clamp(0, i64::from(src_w - 1)) as usize)
                        * channels
                        + channel] as f64,
                ];
                *output = cubic_sample(samples, fx);
            }
            destination[channel] = cubic_sample(rows, fy).clamp(0.0, 255.0) as u8;
        }
    } else {
        let floor_x = sample_x.floor() as i64;
        let floor_y = sample_y.floor() as i64;
        let x0 = floor_x.clamp(0, i64::from(src_w - 1)) as usize;
        let x1 = (floor_x + 1).clamp(0, i64::from(src_w - 1)) as usize;
        let y0 = floor_y.clamp(0, i64::from(src_h - 1)) as usize;
        let y1 = (floor_y + 1).clamp(0, i64::from(src_h - 1)) as usize;
        let fx = sample_x - floor_x as f64;
        let fy = sample_y - floor_y as f64;
        for channel in 0..channels {
            let p00 = raw[(y0 * src_w as usize + x0) * channels + channel] as f64;
            let p10 = raw[(y0 * src_w as usize + x1) * channels + channel] as f64;
            let p01 = raw[(y1 * src_w as usize + x0) * channels + channel] as f64;
            let p11 = raw[(y1 * src_w as usize + x1) * channels + channel] as f64;
            let horizontal_top = (p10 - p00).mul_add(fx, p00);
            let horizontal_bottom = (p11 - p01).mul_add(fx, p01);
            destination[channel] = (horizontal_bottom - horizontal_top)
                .mul_add(fy, horizontal_top)
                .clamp(0.0, 255.0) as u8;
        }
    }
}

pub(super) fn transform_projective_generic(
    raw: &[u8],
    source_dimensions: (u32, u32),
    dst_w: u32,
    dst_h: u32,
    data: &[f64],
    filter: &ResampleFilter,
    fill: Option<(u8, u8, u8, u8)>,
    channels: usize,
    quad: bool,
) -> Result<Vec<u8>, PilError> {
    let pixel_count = usize::try_from(dst_w)
        .ok()
        .and_then(|width| {
            usize::try_from(dst_h)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| PilError::ValueError("image dimensions are too large".into()))?;
    let byte_count = pixel_count
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("image dimensions are too large".into()))?;
    let mut out = vec![0u8; byte_count];
    transform_projective_generic_into(
        raw,
        source_dimensions,
        dst_w,
        dst_h,
        data,
        filter,
        fill,
        channels,
        quad,
        &mut out,
    )?;
    Ok(out)
}

/// Apply the generic projective sampler into a caller-owned native output.
pub(super) fn transform_projective_generic_into(
    raw: &[u8],
    source_dimensions: (u32, u32),
    dst_w: u32,
    dst_h: u32,
    data: &[f64],
    filter: &ResampleFilter,
    fill: Option<(u8, u8, u8, u8)>,
    channels: usize,
    quad: bool,
    out: &mut [u8],
) -> Result<(), PilError> {
    let (src_w, src_h) = source_dimensions;
    let pixel_count = usize::try_from(dst_w)
        .ok()
        .and_then(|width| {
            usize::try_from(dst_h)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| PilError::ValueError("image dimensions are too large".into()))?;
    let byte_count = pixel_count
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("image dimensions are too large".into()))?;
    if out.len() != byte_count {
        return Err(PilError::InternalError(
            "Transform projective destination buffer shape mismatch".into(),
        ));
    }
    let fill_color = fill.unwrap_or((0, 0, 0, 255));
    let nearest = matches!(filter, ResampleFilter::Nearest);

    for dy in 0..dst_h {
        for dx in 0..dst_w {
            let out_idx = (dy as usize * dst_w as usize + dx as usize) * channels;
            let destination = &mut out[out_idx..out_idx + channels];
            let Some((sx, sy)) =
                projective_source_at((dst_w, dst_h), data, quad, f64::from(dx), f64::from(dy))
            else {
                projective_fill_sample(destination, fill_color);
                continue;
            };
            transform_projective_sample(
                raw,
                destination,
                (src_w, src_h),
                channels,
                fill_color,
                nearest,
                filter,
                sx,
                sy,
            );
        }
    }

    Ok(())
}

#[inline(always)]
pub(super) fn transform_mesh_source_coordinate(
    coefficients: [f64; 8],
    local_x: f64,
    local_y: f64,
) -> (f64, f64) {
    let sx_linear =
        coefficients[2].mul_add(local_y, coefficients[1].mul_add(local_x, coefficients[0]));
    let sx = (coefficients[3] * local_x).mul_add(local_y, sx_linear);
    let sy_linear =
        coefficients[6].mul_add(local_y, coefficients[5].mul_add(local_x, coefficients[4]));
    let sy = (coefficients[7] * local_x).mul_add(local_y, sy_linear);
    (sx, sy)
}

#[inline(always)]
fn transform_mesh_sample_pixel(
    raw: &[u8],
    destination: &mut [u8],
    source_dimensions: (u32, u32),
    channels: usize,
    fill_is_none: bool,
    filter: ResampleFilter,
    sx: f64,
    sy: f64,
) {
    let (sw, sh) = source_dimensions;
    if matches!(filter, ResampleFilter::Nearest) {
        // Geometry.c `COORD`: negative/non-finite values are outside and
        // nonnegative values are truncated toward zero.
        let ix = if sx.is_finite() && sx >= 0.0 {
            sx as i64
        } else {
            -1
        };
        let iy = if sy.is_finite() && sy >= 0.0 {
            sy as i64
        } else {
            -1
        };
        if ix >= 0 && ix < i64::from(sw) && iy >= 0 && iy < i64::from(sh) {
            let source_idx = ((iy as usize * sw as usize) + ix as usize) * channels;
            destination.copy_from_slice(&raw[source_idx..source_idx + channels]);
        } else if fill_is_none {
            destination.fill(0);
        }
        return;
    }

    if !sx.is_finite()
        || !sy.is_finite()
        || sx < 0.0
        || sx >= f64::from(sw)
        || sy < 0.0
        || sy >= f64::from(sh)
    {
        if fill_is_none {
            destination.fill(0);
        }
        return;
    }

    let (x, y, dx, dy) = match filter {
        ResampleFilter::Bilinear => {
            let x = (sx - 0.5).floor() as i64;
            let y = (sy - 0.5).floor() as i64;
            (x, y, sx - 0.5 - x as f64, sy - 0.5 - y as f64)
        }
        ResampleFilter::Bicubic => {
            let x = (sx - 0.5).floor() as i64 - 1;
            let y = (sy - 0.5).floor() as i64 - 1;
            (x, y, sx - 0.5 - (x + 1) as f64, sy - 0.5 - (y + 1) as f64)
        }
        _ => return,
    };

    for (channel, output) in destination.iter_mut().enumerate() {
        let value = match filter {
            ResampleFilter::Bilinear => {
                let x0 = x.clamp(0, i64::from(sw - 1)) as usize;
                let x1 = (x + 1).clamp(0, i64::from(sw - 1)) as usize;
                let y0 = y.clamp(0, i64::from(sh - 1)) as usize;
                let y1 = (y + 1).clamp(0, i64::from(sh - 1)) as usize;
                let p00 = raw[(y0 * sw as usize + x0) * channels + channel] as f64;
                let p10 = raw[(y0 * sw as usize + x1) * channels + channel] as f64;
                let p01 = raw[(y1 * sw as usize + x0) * channels + channel] as f64;
                let p11 = raw[(y1 * sw as usize + x1) * channels + channel] as f64;
                let top = (p10 - p00).mul_add(dx, p00);
                let bottom = (p11 - p01).mul_add(dx, p01);
                (bottom - top).mul_add(dy, top)
            }
            ResampleFilter::Bicubic => {
                let mut rows = [0.0; 4];
                for (row, output) in rows.iter_mut().enumerate() {
                    let yy = y + row as i64;
                    let cy = yy.clamp(0, i64::from(sh - 1)) as usize;
                    let samples = [
                        raw[(cy * sw as usize + x.clamp(0, i64::from(sw - 1)) as usize) * channels
                            + channel] as f64,
                        raw[(cy * sw as usize + (x + 1).clamp(0, i64::from(sw - 1)) as usize)
                            * channels
                            + channel] as f64,
                        raw[(cy * sw as usize + (x + 2).clamp(0, i64::from(sw - 1)) as usize)
                            * channels
                            + channel] as f64,
                        raw[(cy * sw as usize + (x + 3).clamp(0, i64::from(sw - 1)) as usize)
                            * channels
                            + channel] as f64,
                    ];
                    *output = cubic_sample(samples, dx);
                }
                cubic_sample(rows, dy)
            }
            _ => unreachable!(),
        };
        *output = match filter {
            ResampleFilter::Bicubic => value.clamp(0.0, 255.0) as u8,
            _ => value as u8,
        };
    }
}

/// Apply Pillow's piecewise quadrilateral mesh transform using the exact
/// destination-box and source-filter contracts of `Geometry.c`.
///
/// `Image.transform(..., Transform.MESH, ...)` lowers each `(box, quad)`
/// record to a local QUAD transform. The C implementation clips the output
/// box, evaluates each local pixel at its center, uses `COORD` (truncate
/// toward zero, with negative values outside) for nearest sampling, and uses
/// horizontal-first filtered interpolation after subtracting `0.5` from the
/// source coordinate. Keep those details here instead of treating a mesh as
/// a global normalized bilinear map: clipping changes the local origin while
/// the coefficients still use the original box dimensions.
pub(super) fn transform_mesh_with_filter(
    raw: &[u8],
    source_dimensions: (u32, u32),
    dst_w: u32,
    dst_h: u32,
    mesh_data: &[f64],
    fill: Option<(u8, u8, u8, u8)>,
    channels: usize,
    filter: ResampleFilter,
    fill_is_none: bool,
) -> Result<Vec<u8>, PilError> {
    let output_len = mesh_output_len(dst_w, dst_h, channels)?;
    let mut out = vec![0u8; output_len];
    transform_mesh_with_filter_into(
        raw,
        source_dimensions,
        dst_w,
        dst_h,
        mesh_data,
        fill,
        channels,
        filter,
        fill_is_none,
        &mut out,
    )?;
    Ok(out)
}

/// Apply a complete MESH request into a caller-owned native output buffer.
pub(super) fn transform_mesh_with_filter_into(
    raw: &[u8],
    source_dimensions: (u32, u32),
    dst_w: u32,
    dst_h: u32,
    mesh_data: &[f64],
    fill: Option<(u8, u8, u8, u8)>,
    channels: usize,
    filter: ResampleFilter,
    fill_is_none: bool,
    out: &mut [u8],
) -> Result<(), PilError> {
    let fill_color = fill.unwrap_or((0, 0, 0, 255));
    if out.len() != mesh_output_len(dst_w, dst_h, channels)? {
        return Err(PilError::InternalError(
            "Transform MESH destination buffer shape mismatch".into(),
        ));
    }
    let fill_bytes = [fill_color.0, fill_color.1, fill_color.2, fill_color.3];
    for destination in out.chunks_exact_mut(channels) {
        destination.copy_from_slice(&fill_bytes[..channels]);
    }
    for mesh in mesh_data.chunks_exact(12) {
        transform_mesh_record_with_filter(
            raw,
            source_dimensions,
            (dst_w, dst_h),
            mesh,
            out,
            channels,
            filter,
            fill_is_none,
        )?;
    }
    Ok(())
}

/// Applies one MESH record directly to an existing output buffer.
///
/// The caller initializes the output once and can then stream records into it
/// while retaining the original source raster between host callbacks.
pub(in crate::compute::pool_cpu::ops::transform) fn transform_mesh_record_with_filter(
    raw: &[u8],
    source_dimensions: (u32, u32),
    destination_dimensions: (u32, u32),
    mesh: &[f64],
    output: &mut [u8],
    channels: usize,
    filter: ResampleFilter,
    fill_is_none: bool,
) -> Result<(), PilError> {
    let (dst_w, dst_h) = destination_dimensions;
    if mesh.len() != 12 || output.len() != mesh_output_len(dst_w, dst_h, channels)? {
        return Err(PilError::InternalError(
            "Transform MESH record or output shape mismatch".into(),
        ));
    }
    let mesh: &[f64; 12] = mesh
        .try_into()
        .map_err(|_| PilError::InternalError("Transform MESH record shape mismatch".into()))?;
    apply_mesh_record(
        raw,
        source_dimensions,
        destination_dimensions,
        mesh,
        output,
        channels,
        filter,
        fill_is_none,
    );
    Ok(())
}

/// Applies a host-shaped MESH record with QUAD coefficients already resolved.
pub(in crate::compute::pool_cpu::ops::transform) fn transform_mesh_record_with_coefficients(
    raw: &[u8],
    source_dimensions: (u32, u32),
    destination_dimensions: (u32, u32),
    box_coordinates: (i64, i64, i64, i64),
    coefficients: [f64; 8],
    output: &mut [u8],
    channels: usize,
    filter: ResampleFilter,
    fill_is_none: bool,
) -> Result<(), PilError> {
    let (dst_w, dst_h) = destination_dimensions;
    if output.len() != mesh_output_len(dst_w, dst_h, channels)? {
        return Err(PilError::InternalError(
            "Transform MESH output shape mismatch".into(),
        ));
    }
    apply_mesh_record_geometry(
        raw,
        source_dimensions,
        destination_dimensions,
        box_coordinates,
        coefficients,
        output,
        channels,
        filter,
        fill_is_none,
    );
    Ok(())
}

fn apply_mesh_record(
    raw: &[u8],
    source_dimensions: (u32, u32),
    destination_dimensions: (u32, u32),
    mesh: &[f64; 12],
    output: &mut [u8],
    channels: usize,
    filter: ResampleFilter,
    fill_is_none: bool,
) {
    let Some((box_coordinates, coefficients)) = mesh_coefficients(mesh) else {
        return;
    };
    apply_mesh_record_geometry(
        raw,
        source_dimensions,
        destination_dimensions,
        box_coordinates,
        coefficients,
        output,
        channels,
        filter,
        fill_is_none,
    );
}

fn apply_mesh_record_geometry(
    raw: &[u8],
    source_dimensions: (u32, u32),
    destination_dimensions: (u32, u32),
    box_coordinates: (i64, i64, i64, i64),
    coefficients: [f64; 8],
    output: &mut [u8],
    channels: usize,
    filter: ResampleFilter,
    fill_is_none: bool,
) {
    let (dst_w, _) = destination_dimensions;
    let (bx0, by0, bx1, by1) = clipped_mesh_box(box_coordinates, destination_dimensions);
    if bx0 >= bx1 || by0 >= by1 {
        return;
    }
    for destination_y in by0..by1 {
        let local_y = (destination_y - by0) as f64 + 0.5;
        for destination_x in bx0..bx1 {
            let local_x = (destination_x - bx0) as f64 + 0.5;
            // Preserve Geometry.c's fused arithmetic order. Folding the
            // cross-term into local_x * local_y changes boundary truncation.
            let (sx, sy) = transform_mesh_source_coordinate(coefficients, local_x, local_y);
            let out_idx =
                ((destination_y as usize * dst_w as usize) + destination_x as usize) * channels;
            let destination = &mut output[out_idx..out_idx + channels];
            if fill_is_none {
                // Pillow streams mesh records into the same output. A failed
                // sample clears the pixel only when fillcolor is omitted;
                // with explicit fill, preserve any earlier record's pixel.
                destination.fill(0);
            }
            transform_mesh_sample_pixel(
                raw,
                destination,
                source_dimensions,
                channels,
                fill_is_none,
                filter,
                sx,
                sy,
            );
        }
    }
}

fn mesh_output_len(width: u32, height: u32, channels: usize) -> Result<usize, PilError> {
    if channels == 0 {
        return Err(PilError::InternalError(
            "Transform MESH requires at least one channel".into(),
        ));
    }
    usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| PilError::ValueError("image dimensions are too large".into()))
}

fn mesh_coefficients(mesh: &[f64; 12]) -> Option<((i64, i64, i64, i64), [f64; 8])> {
    let [x0, y0, x1, y1]: [f64; 4] = mesh[..4].try_into().ok()?;
    let (x0_d, y0_d, x1_d, y1_d) = (x0 as i64, y0 as i64, x1 as i64, y1 as i64);
    let width = x1_d.saturating_sub(x0_d);
    let height = y1_d.saturating_sub(y0_d);
    // Empty/inverted boxes contribute no pixels and cannot create an
    // unbounded loop.
    if width <= 0 || height <= 0 {
        return None;
    }
    let [x0, y0, x1, y1, x2, y2, x3, y3]: [f64; 8] = mesh[4..12].try_into().ok()?;
    let inverse_width = 1.0 / width as f64;
    let inverse_height = 1.0 / height as f64;
    Some((
        (x0_d, y0_d, x1_d, y1_d),
        [
            x0,
            (x3 - x0) * inverse_width,
            (x1 - x0) * inverse_height,
            (x2 - x1 - x3 + x0) * inverse_width * inverse_height,
            y0,
            (y3 - y0) * inverse_width,
            (y1 - y0) * inverse_height,
            (y2 - y1 - y3 + y0) * inverse_width * inverse_height,
        ],
    ))
}

fn clipped_mesh_box(
    (x0, y0, x1, y1): (i64, i64, i64, i64),
    (dst_w, dst_h): (u32, u32),
) -> (i64, i64, i64, i64) {
    (
        x0.max(0).min(i64::from(dst_w)),
        y0.max(0).min(i64::from(dst_h)),
        x1.max(0).min(i64::from(dst_w)),
        y1.max(0).min(i64::from(dst_h)),
    )
}

#[inline]
pub(super) fn cubic_sample(samples: [f64; 4], distance: f64) -> f64 {
    let p1 = samples[1];
    let p2 = -samples[0] + samples[2];
    let p3 = (samples[0] - samples[1]).mul_add(2.0, samples[2]) - samples[3];
    let p4 = -samples[0] + samples[1] - samples[2] + samples[3];
    let inner = distance.mul_add(p4, p3);
    let middle = distance.mul_add(inner, p2);
    distance.mul_add(middle, p1)
}

#[cfg(test)]
mod tests {
    use super::cubic_sample;

    #[test]
    fn cubic_sample_preserves_native_horner_fma_order() {
        // Pillow's compiled bicubic callback uses fused Horner steps and
        // returns 37 here; separate multiply/add operations return 38.
        let value = cubic_sample([174.0, 36.0, 38.0, 84.0], 0.9999999999999996);
        assert_eq!(value as u8, 37);
    }
}
