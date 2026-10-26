use crate::error::PilError;
use crate::pipeline::{ResampleFilter, TransformMethod};
use crate::raster::{
    DynamicImage, GenericImageView, GrayAlphaImage, GrayImage, ImageBuffer, RgbImage, RgbaImage,
};

/// Apply a projective or quadrilateral transform to native 8-bit samples.
///
/// Pillow represents perspective transforms as an inverse homography and
/// quadrilateral transforms as a bilinear map from output coordinates to the
/// four source corners. Keeping both calculations here lets the public core
/// route methods 2 and 3 through the same lazy pipeline as affine transforms.
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
        // Pillow's Image.__transformer first converts QUAD's raw NW/SW/SE/NE
        // corners to bilinear coefficients, then evaluates pixel centers.
        let dx = dx + 0.5;
        let dy = dy + 0.5;
        let x0 = data[0];
        let y0 = data[1];
        let sw = f64::from(dst_w);
        let sh = f64::from(dst_h);
        let inverse_width = 1.0 / sw;
        let inverse_height = 1.0 / sh;
        let x1 = (data[6] - x0) * inverse_width;
        let x2 = (data[2] - x0) * inverse_height;
        let x3 = (data[4] - data[2] - data[6] + x0) * inverse_width * inverse_height;
        let y1 = (data[7] - y0) * inverse_width;
        let y2 = (data[3] - y0) * inverse_height;
        let y3 = (data[5] - data[3] - data[7] + y0) * inverse_width * inverse_height;
        let x = x0 + x1 * dx + x2 * dy + x3 * dx * dy;
        let y = y0 + y1 * dx + y2 * dy + y3 * dx * dy;
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
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    data: &[f64],
    filter: &ResampleFilter,
    fill: Option<(u8, u8, u8, u8)>,
    quad: bool,
) -> Result<DynamicImage, PilError> {
    let channels = img.color().channel_count() as usize;
    let raw = img.as_bytes();
    let (src_w, src_h) = img.dimensions();
    let fill_color = fill.unwrap_or((0, 0, 0, 255));
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

    Ok(match channels {
        1 => DynamicImage::ImageLuma8(GrayImage::from_raw(dst_w, dst_h, out).ok_or_else(|| {
            PilError::InternalError("transform projective L buffer shape mismatch".into())
        })?),
        2 => DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(dst_w, dst_h, out).ok_or_else(
            || PilError::InternalError("transform projective LA buffer shape mismatch".into()),
        )?),
        3 => DynamicImage::ImageRgb8(RgbImage::from_raw(dst_w, dst_h, out).ok_or_else(|| {
            PilError::InternalError("transform projective RGB buffer shape mismatch".into())
        })?),
        4 => DynamicImage::ImageRgba8(RgbaImage::from_raw(dst_w, dst_h, out).ok_or_else(|| {
            PilError::InternalError("transform projective RGBA buffer shape mismatch".into())
        })?),
        _ => unreachable!(),
    })
}

/// Preserve Pillow's native unsigned 16-bit transform sampling contract.
///
/// Nearest copies complete samples; filtered transforms sample the first
/// width bytes of each two-byte row and replace only the first logical output
/// byte. Preserve the other fill byte and the mode's byte order in `ImageLuma16`.
pub(super) fn transform_luma16(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    method: &TransformMethod,
    data: &[f64],
    filter: &ResampleFilter,
    fill: Option<(u8, u8, u8, u8)>,
    fill_is_none: bool,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageLuma16(source) = img else {
        unreachable!()
    };
    let (src_w, src_h) = source.dimensions();
    let fill_value = fill.map_or(0, |value| u16::from_le_bytes([value.0, value.1]));
    let nearest = matches!(filter, ResampleFilter::Nearest);
    let big_endian = mode == Some("I;16B")
        || (cfg!(target_endian = "big") && matches!(mode, Some("I;16" | "I;16N")));
    let encode = |value: u16| {
        if big_endian {
            value.to_be_bytes()
        } else {
            value.to_le_bytes()
        }
    };
    let fill_bytes = if nearest {
        fill_value.to_ne_bytes()
    } else {
        encode(fill_value)
    };
    let carrier = if nearest {
        // A two-band carrier relocates both native bytes without interpreting
        // them as alpha. SPECIAL nearest uses floating center coordinates even
        // for affine transforms, so bypass the ordinary 16.16 byte sampler.
        DynamicImage::ImageLumaA8(
            ImageBuffer::from_raw(src_w, src_h, img.as_bytes().to_vec())
                .ok_or_else(|| PilError::InternalError("I;16 transform source shape".into()))?,
        )
    } else {
        // Geometry.c selects filter8 for SPECIAL modes, reads only the first
        // width bytes of each 2*width-byte row, and writes only byte zero.
        let mut bytes = Vec::with_capacity(src_w as usize * src_h as usize);
        for row in source.as_raw().chunks(src_w.max(1) as usize) {
            bytes.extend(
                row.iter()
                    .flat_map(|&value| encode(value))
                    .take(src_w as usize),
            );
        }
        DynamicImage::ImageLuma8(
            ImageBuffer::from_raw(src_w, src_h, bytes)
                .ok_or_else(|| PilError::InternalError("I;16 transform byte plane shape".into()))?,
        )
    };
    let mut projective = Vec::new();
    let (method, data) = if nearest && matches!(method, TransformMethod::Affine) {
        projective.extend_from_slice(data);
        projective.extend([0.0, 0.0]);
        (&TransformMethod::Perspective, projective.as_slice())
    } else {
        (method, data)
    };
    let result = super::op_transform(
        &carrier,
        dst_w,
        dst_h,
        method,
        data,
        filter,
        Some((fill_bytes[0], fill_bytes[1], 0, 0)),
        fill_is_none,
        Some(if nearest { "PA" } else { "L" }),
    )?;
    let values = if nearest {
        result
            .as_bytes()
            .chunks_exact(2)
            .map(|bytes| u16::from_ne_bytes([bytes[0], bytes[1]]))
            .collect()
    } else {
        result
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
            .collect()
    };
    ImageBuffer::from_raw(dst_w, dst_h, values)
        .map(DynamicImage::ImageLuma16)
        .ok_or_else(|| PilError::InternalError("I;16 transform output shape".into()))
}

pub(super) fn transform_mesh(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    mesh_data: &[f64],
    fill: Option<(u8, u8, u8, u8)>,
    filter: ResampleFilter,
    fill_is_none: bool,
) -> Result<DynamicImage, PilError> {
    transform_mesh_with_filter(img, dst_w, dst_h, mesh_data, fill, filter, fill_is_none)
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
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    mesh_data: &[f64],
    fill: Option<(u8, u8, u8, u8)>,
    filter: ResampleFilter,
    fill_is_none: bool,
) -> Result<DynamicImage, PilError> {
    let channels = img.color().channel_count() as usize;
    let raw = img.as_bytes();
    let (sw, sh) = img.dimensions();
    let sw_f = sw as f64;
    let sh_f = sh as f64;
    let fill_color = fill.unwrap_or((0, 0, 0, 255));

    let output_len = usize::try_from(dst_w)
        .ok()
        .and_then(|width| {
            usize::try_from(dst_h)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| PilError::ValueError("image dimensions are too large".into()))?;
    let fill_bytes = [fill_color.0, fill_color.1, fill_color.2, fill_color.3];
    let mut out = vec![0u8; output_len];
    for destination in out.chunks_exact_mut(channels) {
        destination.copy_from_slice(&fill_bytes[..channels]);
    }

    // Process each mesh element
    for mesh in mesh_data.chunks_exact(12) {
        let x0_d = mesh[0] as i64;
        let y0_d = mesh[1] as i64;
        let x1_d = mesh[2] as i64;
        let y1_d = mesh[3] as i64;
        let width = x1_d.saturating_sub(x0_d);
        let height = y1_d.saturating_sub(y0_d);
        // Python computes these divisors before entering the C transform.
        // Valid mesh boxes are positive; an empty/inverted box contributes no
        // pixels here and, importantly, cannot create an unbounded loop.
        if width <= 0 || height <= 0 {
            continue;
        }

        let x0_s = mesh[4];
        let y0_s = mesh[5];
        let x1_s = mesh[6];
        let y1_s = mesh[7];
        let x2_s = mesh[8];
        let y2_s = mesh[9];
        let x3_s = mesh[10];
        let y3_s = mesh[11];
        let width_f = width as f64;
        let height_f = height as f64;
        let inverse_width = 1.0 / width_f;
        let inverse_height = 1.0 / height_f;
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

        // ImagingGenericTransform clips the box, then passes coordinates
        // relative to that clipped lower-left corner to the map function.
        let bx0 = x0_d.max(0).min(i64::from(dst_w));
        let by0 = y0_d.max(0).min(i64::from(dst_h));
        let bx1 = x1_d.max(0).min(i64::from(dst_w));
        let by1 = y1_d.max(0).min(i64::from(dst_h));
        if bx0 >= bx1 || by0 >= by1 {
            continue;
        }

        for destination_y in by0..by1 {
            let local_y = (destination_y - by0) as f64 + 0.5;
            for destination_x in bx0..bx1 {
                let local_x = (destination_x - bx0) as f64 + 0.5;
                // Pillow src/libImaging/Geometry.c quad_transform is compiled
                // into two linear
                // fused additions followed by a rounded x-product and a
                // final fused y-product on this target. Keep the cross-term
                // multiplication separate from the final `mul_add`; folding
                // it into `local_x * local_y` changes boundary truncation.
                let sx_linear = coefficients[2]
                    .mul_add(local_y, coefficients[1].mul_add(local_x, coefficients[0]));
                let sx = (coefficients[3] * local_x).mul_add(local_y, sx_linear);
                let sy_linear = coefficients[6]
                    .mul_add(local_y, coefficients[5].mul_add(local_x, coefficients[4]));
                let sy = (coefficients[7] * local_x).mul_add(local_y, sy_linear);
                let out_idx =
                    ((destination_y as usize * dst_w as usize) + destination_x as usize) * channels;

                if matches!(filter, ResampleFilter::Nearest) {
                    // Geometry.c `COORD`: negative values are outside and
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
                        out[out_idx..out_idx + channels]
                            .copy_from_slice(&raw[source_idx..source_idx + channels]);
                    } else if fill_is_none {
                        out[out_idx..out_idx + channels].fill(0);
                    }
                    continue;
                }

                if !sx.is_finite()
                    || !sy.is_finite()
                    || sx < 0.0
                    || sx >= sw_f
                    || sy < 0.0
                    || sy >= sh_f
                {
                    if fill_is_none {
                        out[out_idx..out_idx + channels].fill(0);
                    }
                    continue;
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
                    _ => continue,
                };

                for channel in 0..channels {
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
                                    raw[(cy * sw as usize
                                        + (x).clamp(0, i64::from(sw - 1)) as usize)
                                        * channels
                                        + channel] as f64,
                                    raw[(cy * sw as usize
                                        + (x + 1).clamp(0, i64::from(sw - 1)) as usize)
                                        * channels
                                        + channel] as f64,
                                    raw[(cy * sw as usize
                                        + (x + 2).clamp(0, i64::from(sw - 1)) as usize)
                                        * channels
                                        + channel] as f64,
                                    raw[(cy * sw as usize
                                        + (x + 3).clamp(0, i64::from(sw - 1)) as usize)
                                        * channels
                                        + channel] as f64,
                                ];
                                *output = cubic_sample(samples, dx);
                            }
                            cubic_sample(rows, dy)
                        }
                        _ => unreachable!(),
                    };
                    out[out_idx + channel] = match filter {
                        ResampleFilter::Bicubic => value.clamp(0.0, 255.0) as u8,
                        _ => value as u8,
                    };
                }
            }
        }
    }

    crate::image_utils::raw_bytes_to_image(dst_w, dst_h, out, channels)
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
