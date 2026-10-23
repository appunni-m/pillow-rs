//! Scalar fallback for scalar affine transform and rotation operations.

// ── Geometry and spatial operations (rotate, remap_palette, equalize) ────────

/// Geometry.c stores byte bilinear samples by truncating the interpolated
/// value, unlike the resize helper above which rounds its result.
#[inline(always)]
fn bilinear_interp_truncated(c00: u32, c10: u32, c01: u32, c11: u32, fx: f64, fy: f64) -> u32 {
    // Blend horizontally before the vertical blend. Apart from matching the
    // source kernel's operation order, this keeps a constant edge sample
    // exactly constant instead of turning 15 into 14 on an f64 roundoff.
    let v = (1.0 - fy) * ((1.0 - fx) * c00 as f64 + fx * c10 as f64)
        + fy * ((1.0 - fx) * c01 as f64 + fx * c11 as f64);
    v as u32
}

#[inline(always)]
fn premultiply_rotate_channel(value: u32, alpha: u32) -> u32 {
    (value as f64 * alpha as f64 / 255.0 + 0.5) as u32
}

#[inline(always)]
fn unpremultiply_rotate_channel(value: u32, alpha: u32) -> u32 {
    if alpha == 0 {
        0
    } else {
        (value as f64 * 255.0 / alpha as f64) as u32
    }
}

/// Rotate image by angle_deg degrees using bilinear interpolation.
///
/// `expand`: if true, output canvas expands to fit rotated image. If false, output
/// is same size as input (corners may be clipped).
/// `fill_rgba`: packed u32 fill color (0xAABBGGRR) for empty areas behind the
/// rotated image.
///
/// Rotation is counter-clockwise (matching PIL). Uses inverse mapping: for each
/// output pixel, rotate the coordinate back to source space and bilinear-interpolate.
///
/// Mode-aware: for L/LA modes (mode < 2), G = B = R (luma carried in R). Alpha
/// preserved for LA/RGBA, forced to 0xFF for L/RGB.
///
/// Returns (new_pixels, new_w, new_h).
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn rotate(
    pixels: &[u32],
    w: u32,
    h: u32,
    mode: u32,
    angle_deg: f64,
    expand: bool,
    fill_rgba: u32,
) -> (Vec<u32>, u32, u32) {
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);

    let geometry = rotate_geometry(w, h, angle_deg, expand);
    let plan = RotatePlan {
        geometry,
        has_green_blue: has_gb,
        has_alpha: has_a,
        fill_red: fill_rgba & 0xFF,
        fill_green: (fill_rgba >> 8) & 0xFF,
        fill_blue: (fill_rgba >> 16) & 0xFF,
        fill_alpha: fill_rgba & 0xFF00_0000,
    };

    if w == 0 || h == 0 {
        // Pillow still computes the expanded canvas for an empty source and
        // fills it; returning the original dimensions loses the 90-degree
        // width/height swap and diverges before any pixel sampling occurs.
        let out_g = if has_gb {
            plan.fill_green
        } else {
            plan.fill_red
        };
        let out_b = if has_gb {
            plan.fill_blue
        } else {
            plan.fill_red
        };
        let out_a = if has_a { plan.fill_alpha } else { 0xFF00_0000 };
        let fill_pixel = plan.fill_red | (out_g << 8) | (out_b << 16) | out_a;
        return (
            vec![fill_pixel; (geometry.destination_width * geometry.destination_height) as usize],
            geometry.destination_width,
            geometry.destination_height,
        );
    }

    let mut out = vec![0u32; (geometry.destination_width * geometry.destination_height) as usize];
    rotate_pixels(pixels, &mut out, &plan);
    (out, geometry.destination_width, geometry.destination_height)
}

#[derive(Clone, Copy)]
struct RotateGeometry {
    source_width: u32,
    source_height: u32,
    source_width_f: f64,
    source_height_f: f64,
    destination_width: u32,
    destination_height: u32,
    affine: [f64; 6],
}

#[derive(Clone, Copy)]
struct RotatePlan {
    geometry: RotateGeometry,
    has_green_blue: bool,
    has_alpha: bool,
    fill_red: u32,
    fill_green: u32,
    fill_blue: u32,
    fill_alpha: u32,
}

fn rotate_geometry(w: u32, h: u32, angle_deg: f64, expand: bool) -> RotateGeometry {
    let sw = w as f64;
    let sh = h as f64;
    // Keep this transform aligned with pool_cpu::ops::geometry.  Pillow rounds
    // the trigonometric coefficients to 15 decimal places and rounds the two
    // expanded outer edges independently; taking ceil(max-min) loses a pixel
    // for odd dimensions and fractional rotations.
    let rad = -angle_deg.to_radians();
    let aff_a = crate::ops::rotate::round_rotate_coefficient(rad.cos());
    let aff_b = crate::ops::rotate::round_rotate_coefficient(rad.sin());
    let aff_d = crate::ops::rotate::round_rotate_coefficient(-rad.sin());
    let aff_e = aff_a;
    let center_x = sw / 2.0;
    let center_y = sh / 2.0;
    let mut aff_c = aff_a * -center_x + aff_b * -center_y + center_x;
    let mut aff_f = aff_d * -center_x + aff_e * -center_y + center_y;
    let transform =
        |x: f64, y: f64, c: f64, f: f64| (aff_a * x + aff_b * y + c, aff_d * x + aff_e * y + f);

    let corners = [(0.0, 0.0), (sw, 0.0), (sw, sh), (0.0, sh)];
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(cx, cy) in &corners {
        let (rx, ry) = transform(cx, cy, aff_c, aff_f);
        min_x = min_x.min(rx);
        max_x = max_x.max(rx);
        min_y = min_y.min(ry);
        max_y = max_y.max(ry);
    }

    let (dw, dh) = if expand {
        (
            (max_x.ceil() - min_x.floor()) as u32,
            (max_y.ceil() - min_y.floor()) as u32,
        )
    } else {
        (w, h)
    };

    if expand {
        let shift_x = -(dw as f64 - sw) / 2.0;
        let shift_y = -(dh as f64 - sh) / 2.0;
        (aff_c, aff_f) = transform(shift_x, shift_y, aff_c, aff_f);
    }
    RotateGeometry {
        source_width: w,
        source_height: h,
        source_width_f: sw,
        source_height_f: sh,
        destination_width: dw,
        destination_height: dh,
        affine: [aff_a, aff_b, aff_c, aff_d, aff_e, aff_f],
    }
}

#[inline]
fn rotate_pixels(pixels: &[u32], out: &mut [u32], plan: &RotatePlan) {
    for dy in 0..plan.geometry.destination_height {
        for dx in 0..plan.geometry.destination_width {
            let out_idx = (dy * plan.geometry.destination_width + dx) as usize;
            out[out_idx] = rotate_sample(pixels, dx, dy, plan);
        }
    }
}

#[inline]
fn rotate_sample(pixels: &[u32], dx: u32, dy: u32, plan: &RotatePlan) -> u32 {
    let RotatePlan {
        geometry:
            RotateGeometry {
                source_width_f: sw,
                source_height_f: sh,
                affine: [aff_a, aff_b, aff_c, aff_d, aff_e, aff_f],
                ..
            },
        has_green_blue: has_gb,
        has_alpha: has_a,
        fill_red: fill_r,
        fill_green: fill_g,
        fill_blue: fill_b,
        fill_alpha: fill_a,
    } = *plan;
    let transform =
        |x: f64, y: f64, c: f64, f: f64| (aff_a * x + aff_b * y + c, aff_d * x + aff_e * y + f);
    // Map the destination pixel center through the reverse affine
    // transform, then move from center space to Pillow's filter-side
    // source corner coordinates.
    let (src_x, src_y) = transform(dx as f64 + 0.5, dy as f64 + 0.5, aff_c, aff_f);
    let src_x = src_x - 0.5;
    let src_y = src_y - 0.5;

    // Pillow's bilinear filter has a half-pixel footprint. Coordinates
    // in [-0.5, width - 0.5) remain samples at the image edge, where
    // the source coordinate is clamped; only points outside that
    // support use the fill color.
    if src_x >= -0.5 && src_x < sw - 0.5 && src_y >= -0.5 && src_y < sh - 0.5 {
        return rotate_bilinear_sample(pixels, src_x, src_y, plan);
    }

    // Out of source bounds: fill with fill_rgba (mode-aware).
    let out_a = if has_a { fill_a } else { 0xFF00_0000 };
    let out_g = if has_gb { fill_g } else { fill_r };
    let out_b = if has_gb { fill_b } else { fill_r };
    fill_r | (out_g << 8) | (out_b << 16) | out_a
}

#[inline]
fn rotate_bilinear_sample(pixels: &[u32], src_x: f64, src_y: f64, plan: &RotatePlan) -> u32 {
    let RotateGeometry {
        source_width: w,
        source_height: h,
        source_width_f: sw,
        source_height_f: sh,
        ..
    } = plan.geometry;
    let src_x = src_x.clamp(0.0, sw - 1.0);
    let src_y = src_y.clamp(0.0, sh - 1.0);
    let sx = src_x.floor() as u32;
    let sy = src_y.floor() as u32;
    let fx = src_x - sx as f64;
    let fy = src_y - sy as f64;
    let sx1 = (sx + 1).min(w - 1);
    let sy1 = (sy + 1).min(h - 1);
    let p00 = pixels[(sy * w + sx) as usize];
    let p10 = pixels[(sy * w + sx1) as usize];
    let p01 = pixels[(sy1 * w + sx) as usize];
    let p11 = pixels[(sy1 * w + sx1) as usize];
    // Pillow's RGBA/LA rotate path samples premultiplied channels, then
    // truncates unpremultiplication on return (Geometry.c / Image.rotate).
    let [r00, g00, b00, a00] = rotate_sample_channels(p00, plan.has_alpha);
    let [r10, g10, b10, a10] = rotate_sample_channels(p10, plan.has_alpha);
    let [r01, g01, b01, a01] = rotate_sample_channels(p01, plan.has_alpha);
    let [r11, g11, b11, a11] = rotate_sample_channels(p11, plan.has_alpha);

    let out_r_sample = bilinear_interp_truncated(r00, r10, r01, r11, fx, fy);
    let out_g_sample = bilinear_interp_truncated(g00, g10, g01, g11, fx, fy);
    let out_b_sample = bilinear_interp_truncated(b00, b10, b01, b11, fx, fy);
    let out_a_raw = bilinear_interp_truncated(a00, a10, a01, a11, fx, fy);
    let out_r = if plan.has_alpha {
        unpremultiply_rotate_channel(out_r_sample, out_a_raw)
    } else {
        out_r_sample
    };
    let out_g_raw = if plan.has_alpha {
        unpremultiply_rotate_channel(out_g_sample, out_a_raw)
    } else {
        out_g_sample
    };
    let out_b_raw = if plan.has_alpha {
        unpremultiply_rotate_channel(out_b_sample, out_a_raw)
    } else {
        out_b_sample
    };
    let out_g = if plan.has_green_blue {
        out_g_raw
    } else {
        out_r
    };
    let out_b = if plan.has_green_blue {
        out_b_raw
    } else {
        out_r
    };
    let out_a = if plan.has_alpha {
        out_a_raw << 24
    } else {
        0xFF00_0000
    };
    out_r | (out_g << 8) | (out_b << 16) | out_a
}

#[inline]
fn rotate_sample_channels(pixel: u32, has_alpha: bool) -> [u32; 4] {
    let alpha = (pixel >> 24) & 0xFF;
    let red = pixel & 0xFF;
    let green = (pixel >> 8) & 0xFF;
    let blue = (pixel >> 16) & 0xFF;
    let premultiply = |channel| {
        if has_alpha {
            premultiply_rotate_channel(channel, alpha)
        } else {
            channel
        }
    };
    [
        premultiply(red),
        premultiply(green),
        premultiply(blue),
        alpha,
    ]
}

// ── Effects + Module operations (transform, put_pixel, put_data, put_alpha, composite_module) ──

/// Affine transform with configurable filter (0=Nearest, 1=Bilinear+).
///
/// Maps each destination pixel center (dx+0.5,dy+0.5) back to source
/// (sx,sy) via the matrix [a,b,c,d,e,f] where:
///   sx = a*(dx+0.5) + b*(dy+0.5) + c
///   sy = d*(dx+0.5) + e*(dy+0.5) + f
///
/// filter=0 (Nearest): truncate non-negative source coordinates, matching
/// Pillow's `COORD` macro, and reject negative/out-of-bounds coordinates.
/// filter>=1 (Bilinear): fractional source position, 4-neighbor weighted blend with
/// f64 precision, mode-aware channel output.
///
/// Empty areas (source coordinates out of bounds) are filled with fill_rgba
/// (packed 0xAABBGGRR).
///
/// Returns (output_pixels, dst_w, dst_h).
/// mode: 0=L, 1=LA, 2=RGB-like, 3=RGBA, 4=CMYK-like four-byte storage
#[inline]
pub fn transform(
    pixels: &[u32],
    w: u32,
    h: u32,
    mode: u32,
    dst_w: u32,
    dst_h: u32,
    matrix: &[f64; 8],
    filter: u32,
    fill_rgba: u32,
) -> (Vec<u32>, u32, u32) {
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
    let has_fourth = mode == 1 || mode == 3 || mode == 4;
    let nearest = filter == 0;

    let aff_a = matrix[0];
    let aff_b = matrix[1];
    let aff_c = matrix[2];
    let aff_d = matrix[3];
    let aff_e = matrix[4];
    let aff_f = matrix[5];

    // The affine byte kernel receives fill samples in the same packed layout
    // as its input. For alpha modes Pillow writes that fill into the temporary
    // premultiplied buffer without premultiplying it first, then expands it
    // when converting back to straight-alpha output.
    let fill_r = fill_rgba & 0xFF;
    let fill_g = (fill_rgba >> 8) & 0xFF;
    let fill_b = (fill_rgba >> 16) & 0xFF;
    let fill_a = (fill_rgba >> 24) & 0xFF;
    let unpremultiply = |value: u32, alpha: u32| {
        if alpha == 0 {
            0
        } else {
            (value.saturating_mul(255) / alpha).min(255)
        }
    };
    let fill_out_r = if has_a && !nearest {
        unpremultiply(fill_r, fill_a)
    } else {
        fill_r
    };
    let fill_out_g = if has_gb {
        if has_a && !nearest {
            unpremultiply(fill_g, fill_a)
        } else {
            fill_g
        }
    } else {
        fill_out_r
    };
    let fill_out_b = if has_gb {
        if has_a && !nearest {
            unpremultiply(fill_b, fill_a)
        } else {
            fill_b
        }
    } else {
        fill_out_r
    };
    let fill_out_a = if has_fourth { fill_a } else { 255 };
    let fill_pixel = fill_out_r | (fill_out_g << 8) | (fill_out_b << 16) | (fill_out_a << 24);

    // Guard against empty source dimensions (avoids underflow on w-1 / h-1)
    if w == 0 || h == 0 {
        return (vec![fill_pixel; (dst_w * dst_h) as usize], dst_w, dst_h);
    }

    let plan = TransformPlan {
        source_width: w,
        source_height: h,
        destination_width: dst_w,
        destination_height: dst_h,
        affine: [aff_a, aff_b, aff_c, aff_d, aff_e, aff_f],
        has_green_blue: has_gb,
        has_alpha: has_a,
        has_fourth,
        fill_pixel,
    };
    let mut out = vec![0u32; (dst_w * dst_h) as usize];
    if nearest {
        transform_nearest(pixels, &mut out, plan);
    } else {
        transform_bilinear(pixels, &mut out, plan);
    }

    (out, dst_w, dst_h)
}

#[derive(Clone, Copy)]
struct TransformPlan {
    source_width: u32,
    source_height: u32,
    destination_width: u32,
    destination_height: u32,
    affine: [f64; 6],
    has_green_blue: bool,
    has_alpha: bool,
    has_fourth: bool,
    fill_pixel: u32,
}

#[inline]
fn transform_nearest(pixels: &[u32], out: &mut [u32], plan: TransformPlan) {
    let TransformPlan {
        source_width: w,
        source_height: h,
        destination_width: dst_w,
        destination_height: dst_h,
        affine: [aff_a, aff_b, aff_c, aff_d, aff_e, aff_f],
        has_green_blue: has_gb,
        has_fourth,
        fill_pixel,
        ..
    } = plan;
    for dy in 0..dst_h {
        for dx in 0..dst_w {
            // Geometry.c's affine_transform() evaluates destination
            // pixel centers. Its nearest filter then applies COORD(),
            // which rejects negative values and truncates non-negative
            // values toward zero.
            let sx = aff_a * (dx as f64 + 0.5) + aff_b * (dy as f64 + 0.5) + aff_c;
            let sy = aff_d * (dx as f64 + 0.5) + aff_e * (dy as f64 + 0.5) + aff_f;
            let out_idx = (dy * dst_w + dx) as usize;

            let ix = if sx < 0.0 { -1 } else { sx as i64 };
            let iy = if sy < 0.0 { -1 } else { sy as i64 };
            if ix >= 0 && ix < w as i64 && iy >= 0 && iy < h as i64 {
                let sp = pixels[(iy as u32 * w + ix as u32) as usize];
                let r = sp & 0xFF;
                let g = (sp >> 8) & 0xFF;
                let b_val = (sp >> 16) & 0xFF;
                let a_val = sp & 0xFF00_0000;
                let og = if has_gb { g } else { r };
                let ob = if has_gb { b_val } else { r };
                let oa = if has_fourth { a_val } else { 0xFF00_0000 };
                out[out_idx] = r | (og << 8) | (ob << 16) | oa;
            } else {
                out[out_idx] = fill_pixel;
            }
        }
    }
}

#[inline]
fn transform_bilinear(pixels: &[u32], out: &mut [u32], plan: TransformPlan) {
    let dst_w = plan.destination_width;
    let dst_h = plan.destination_height;
    for dy in 0..dst_h {
        for dx in 0..dst_w {
            let out_idx = (dy * dst_w + dx) as usize;
            out[out_idx] = transform_bilinear_pixel(pixels, dx, dy, plan);
        }
    }
}

#[inline]
fn transform_bilinear_pixel(pixels: &[u32], dx: u32, dy: u32, plan: TransformPlan) -> u32 {
    let TransformPlan {
        source_width: w,
        source_height: h,
        affine: [aff_a, aff_b, aff_c, aff_d, aff_e, aff_f],
        fill_pixel,
        ..
    } = plan;
    let w_f = w as f64;
    let h_f = h as f64;
    let w_max = w - 1;
    let h_max = h - 1;
    // Geometry.c evaluates destination pixel centers and the bilinear filter
    // applies its half-pixel shift after checking center-space bounds.
    let sx = aff_a * (dx as f64 + 0.5) + aff_b * (dy as f64 + 0.5) + aff_c;
    let sy = aff_d * (dx as f64 + 0.5) + aff_e * (dy as f64 + 0.5) + aff_f;
    if !(sx >= 0.0 && sx < w_f && sy >= 0.0 && sy < h_f) {
        return fill_pixel;
    }
    let sample_x = sx - 0.5;
    let sample_y = sy - 0.5;
    let x_floor = sample_x.floor() as i64;
    let y_floor = sample_y.floor() as i64;
    let x0 = x_floor.clamp(0, w_max as i64) as u32;
    let y0 = y_floor.clamp(0, h_max as i64) as u32;
    let x1 = (x_floor + 1).clamp(0, w_max as i64) as u32;
    let y1 = (y_floor + 1).clamp(0, h_max as i64) as u32;
    let fx = sample_x - x_floor as f64;
    let fy = sample_y - y_floor as f64;
    let neighbors = [
        pixels[(y0 * w + x0) as usize],
        pixels[(y0 * w + x1) as usize],
        pixels[(y1 * w + x0) as usize],
        pixels[(y1 * w + x1) as usize],
    ];
    transform_bilinear_channels(neighbors, fx, fy, plan)
}

#[inline]
fn transform_bilinear_channels(pixels: [u32; 4], fx: f64, fy: f64, plan: TransformPlan) -> u32 {
    let [p00, p10, p01, p11] = pixels;
    // Extract all four stored channels from each of the four neighbors.
    let r00 = (p00 & 0xFF) as f64;
    let g00 = ((p00 >> 8) & 0xFF) as f64;
    let b00 = ((p00 >> 16) & 0xFF) as f64;
    let a00 = ((p00 >> 24) & 0xFF) as f64;
    let r10 = (p10 & 0xFF) as f64;
    let g10 = ((p10 >> 8) & 0xFF) as f64;
    let b10 = ((p10 >> 16) & 0xFF) as f64;
    let a10 = ((p10 >> 24) & 0xFF) as f64;
    let r01 = (p01 & 0xFF) as f64;
    let g01 = ((p01 >> 8) & 0xFF) as f64;
    let b01 = ((p01 >> 16) & 0xFF) as f64;
    let a01 = ((p01 >> 24) & 0xFF) as f64;
    let r11 = (p11 & 0xFF) as f64;
    let g11 = ((p11 >> 8) & 0xFF) as f64;
    let b11 = ((p11 >> 16) & 0xFF) as f64;
    let a11 = ((p11 >> 24) & 0xFF) as f64;
    let inv_fx = 1.0 - fx;
    let inv_fy = 1.0 - fy;
    // Keep the CPU byte path term order: weighted samples truncate toward zero.
    let out_r_f = inv_fx * inv_fy * r00 + fx * inv_fy * r10 + inv_fx * fy * r01 + fx * fy * r11;
    let out_g_f = inv_fx * inv_fy * g00 + fx * inv_fy * g10 + inv_fx * fy * g01 + fx * fy * g11;
    let out_b_f = inv_fx * inv_fy * b00 + fx * inv_fy * b10 + inv_fx * fy * b01 + fx * fy * b11;
    let out_a_raw =
        bilinear_interp_truncated(a00 as u32, a10 as u32, a01 as u32, a11 as u32, fx, fy);
    let (out_r, out_g_raw, out_b_raw) = if plan.has_alpha {
        let premultiply =
            |value: u32, alpha: u32| ((value.saturating_mul(alpha) + 127) / 255).min(255);
        let interpolate =
            |c00, c10, c01, c11| bilinear_interp_truncated(c00, c10, c01, c11, fx, fy);
        (
            unpremultiply_transform_channel(
                interpolate(
                    premultiply(r00 as u32, a00 as u32),
                    premultiply(r10 as u32, a10 as u32),
                    premultiply(r01 as u32, a01 as u32),
                    premultiply(r11 as u32, a11 as u32),
                ),
                out_a_raw,
            ),
            unpremultiply_transform_channel(
                interpolate(
                    premultiply(g00 as u32, a00 as u32),
                    premultiply(g10 as u32, a10 as u32),
                    premultiply(g01 as u32, a01 as u32),
                    premultiply(g11 as u32, a11 as u32),
                ),
                out_a_raw,
            ),
            unpremultiply_transform_channel(
                interpolate(
                    premultiply(b00 as u32, a00 as u32),
                    premultiply(b10 as u32, a10 as u32),
                    premultiply(b01 as u32, a01 as u32),
                    premultiply(b11 as u32, a11 as u32),
                ),
                out_a_raw,
            ),
        )
    } else {
        (
            out_r_f.clamp(0.0, 255.0) as u32,
            out_g_f.clamp(0.0, 255.0) as u32,
            out_b_f.clamp(0.0, 255.0) as u32,
        )
    };
    let out_g = if plan.has_green_blue {
        out_g_raw
    } else {
        out_r
    };
    let out_b = if plan.has_green_blue {
        out_b_raw
    } else {
        out_r
    };
    let out_a = if plan.has_fourth {
        out_a_raw << 24
    } else {
        0xFF00_0000
    };
    out_r | (out_g << 8) | (out_b << 16) | out_a
}

#[inline(always)]
fn unpremultiply_transform_channel(value: u32, alpha: u32) -> u32 {
    if alpha == 0 {
        0
    } else {
        (value.saturating_mul(255) / alpha).min(255)
    }
}
