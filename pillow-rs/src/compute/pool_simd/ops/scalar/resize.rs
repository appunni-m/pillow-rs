//! Scalar fallback for scalar resize and thumbnail operations.

/// Resize image using nearest-neighbor or bilinear sampling.
///
/// PIL pixel-centered coordinate mapping:
///   `src_coord = (dst_coord + 0.5) * src_size / dst_size`
///
/// Nearest (filter=0): `floor(src_coord)`, clamped to bounds, mode-aware channel copy.
/// Bilinear (filter>=1): fractional source position, 4-neighbor weighted blend with f64
/// precision, mode-aware channel output.
///
/// For L/LA modes (mode < 2): only R channel carries luma; G and B are set to R.
/// For L/RGB modes (mode 0 or 2): alpha forced to 0xFF.
/// For LA/RGBA modes (mode 1 or 3): alpha preserved and interpolated.
///
/// Returns `(new_pixels, new_width, new_height)` since output dimensions differ from input.
///
/// filter: 0=Nearest, 1=Bilinear (other values fall back to Bilinear)
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn resize(
    pixels: &[u32],
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
    mode: u32,
    filter: u32,
) -> (Vec<u32>, u32, u32) {
    resize_box(
        pixels,
        src_w,
        src_h,
        dst_w,
        dst_h,
        mode,
        filter,
        0.0,
        0.0,
        src_w as f64,
        src_h as f64,
    )
}

/// Geometry shared by the nearest and bilinear boxed-resize passes.
#[derive(Clone, Copy)]
struct ResizeBoxGeometry {
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
    mode: u32,
    box_left: f64,
    box_top: f64,
    box_right: f64,
    box_bottom: f64,
}

/// Resize a floating-point source box to the destination dimensions.
///
/// This is the same packed-pixel kernel as [`resize`], with the source box
/// kept fractional for ImageOps.fit's `box=` contract.
#[inline]
fn resize_box(
    pixels: &[u32],
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
    mode: u32,
    filter: u32,
    box_left: f64,
    box_top: f64,
    box_right: f64,
    box_bottom: f64,
) -> (Vec<u32>, u32, u32) {
    // Guard against empty dimensions (avoids div-by-zero and u32 underflow)
    if src_w == 0 || src_h == 0 || dst_w == 0 || dst_h == 0 {
        return (vec![0u32; (dst_w * dst_h) as usize], dst_w, dst_h);
    }

    let count = (dst_w * dst_h) as usize;
    let mut output = vec![0u32; count];
    let geometry = ResizeBoxGeometry {
        src_w,
        src_h,
        dst_w,
        dst_h,
        mode,
        box_left,
        box_top,
        box_right,
        box_bottom,
    };

    if filter == 0 {
        resize_box_nearest(pixels, &mut output, geometry);
    } else {
        resize_box_bilinear(pixels, &mut output, geometry);
    }

    (output, dst_w, dst_h)
}

#[inline]
fn resize_box_nearest(pixels: &[u32], output: &mut [u32], geometry: ResizeBoxGeometry) {
    let ResizeBoxGeometry {
        src_w,
        src_h,
        dst_w,
        dst_h,
        mode,
        box_left,
        box_top,
        box_right,
        box_bottom,
    } = geometry;
    let has_gb = super::mode_has_green_blue(mode); // RGB or RGBA
    let has_a = super::mode_has_alpha(mode); // LA or RGBA
    let sw_f = box_right - box_left;
    let sh_f = box_bottom - box_top;
    let dw_f = dst_w as f64;
    let dh_f = dst_h as f64;
    let src_w_max = src_w - 1;
    let src_h_max = src_h - 1;
    // ── Nearest-neighbor ──
    // Pillow's ImagingScaleAffine builds integer coordinate tables with
    // a cumulative floating-point position (x0 + scale / 2, then one
    // scale increment per output pixel). Recomputing `(x + 0.5) * scale`
    // changes exact-boundary samples, notably for 14 -> 21 scaling where
    // Pillow repeats a source index across two output columns.
    let scale_x = sw_f / dw_f;
    let scale_y = sh_f / dh_f;
    let full_source_box = box_left == 0.0
        && box_top == 0.0
        && box_right == src_w as f64
        && box_bottom == src_h as f64;
    let mut x_positions = Vec::with_capacity(dst_w as usize);
    if full_source_box {
        let mut x_position = box_left + scale_x * 0.5;
        for _ in 0..dst_w {
            x_positions.push(x_position as u32);
            x_position += scale_x;
        }
    } else {
        for dx in 0..dst_w {
            x_positions.push((box_left + (dx as f64 + 0.5) * scale_x) as u32);
        }
    }
    let mut y_position = box_top + scale_y * 0.5;
    for dy in 0..dst_h {
        let sy = if full_source_box {
            y_position as u32
        } else {
            (box_top + (dy as f64 + 0.5) * scale_y) as u32
        };
        let sy = sy.min(src_h_max);
        for dx in 0..dst_w {
            let sx = x_positions[dx as usize];
            let sx = sx.min(src_w_max);

            let sp = pixels[(sy * src_w + sx) as usize];
            let r = sp & 0xFF;
            let g = (sp >> 8) & 0xFF;
            let b = (sp >> 16) & 0xFF;
            let a = sp & 0xFF00_0000;

            let out_r = r;
            let out_g = if has_gb { g } else { r };
            let out_b = if has_gb { b } else { r };
            let out_a = if has_a { a } else { 0xFF00_0000 };

            output[(dy * dst_w + dx) as usize] = out_r | (out_g << 8) | (out_b << 16) | out_a;
        }
        if full_source_box {
            y_position += scale_y;
        }
    }
}

#[inline]
fn resize_box_bilinear(pixels: &[u32], output: &mut [u32], geometry: ResizeBoxGeometry) {
    let ResizeBoxGeometry {
        src_w,
        src_h,
        dst_w,
        dst_h,
        mode,
        box_left,
        box_top,
        box_right,
        box_bottom,
    } = geometry;
    let has_gb = super::mode_has_green_blue(mode); // RGB or RGBA
    let has_a = super::mode_has_alpha(mode); // LA or RGBA
    let sw_f = box_right - box_left;
    let sh_f = box_bottom - box_top;
    let dw_f = dst_w as f64;
    let dh_f = dst_h as f64;
    let src_w_max = src_w - 1;
    let src_h_max = src_h - 1;
    // ── Bilinear (and fallback for Bicubic/Lanczos/Box/Hamming) ──
    for dy in 0..dst_h {
        let cy = box_top + (dy as f64 + 0.5) * sh_f / dh_f;
        let sy_floor = cy.floor();
        let fy = cy - sy_floor;
        let y0 = (sy_floor as u32).min(src_h_max);
        let y1 = (y0 + 1).min(src_h_max);

        for dx in 0..dst_w {
            let cx = box_left + (dx as f64 + 0.5) * sw_f / dw_f;
            let sx_floor = cx.floor();
            let fx = cx - sx_floor;
            let x0 = (sx_floor as u32).min(src_w_max);
            let x1 = (x0 + 1).min(src_w_max);

            let p00 = pixels[(y0 * src_w + x0) as usize];
            let p10 = pixels[(y0 * src_w + x1) as usize];
            let p01 = pixels[(y1 * src_w + x0) as usize];
            let p11 = pixels[(y1 * src_w + x1) as usize];

            // Extract all 4 channels from each of the 4 neighbors
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

            // Horizontal interpolation (top row and bottom row)
            let inv_fx = 1.0 - fx;
            let top_r = inv_fx * r00 + fx * r10;
            let top_g = inv_fx * g00 + fx * g10;
            let top_b = inv_fx * b00 + fx * b10;
            let top_a = inv_fx * a00 + fx * a10;

            let bot_r = inv_fx * r01 + fx * r11;
            let bot_g = inv_fx * g01 + fx * g11;
            let bot_b = inv_fx * b01 + fx * b11;
            let bot_a = inv_fx * a01 + fx * a11;

            // Vertical interpolation
            let inv_fy = 1.0 - fy;
            let out_r_f = inv_fy * top_r + fy * bot_r;
            let out_g_f = inv_fy * top_g + fy * bot_g;
            let out_b_f = inv_fy * top_b + fy * bot_b;
            let out_a_f = inv_fy * top_a + fy * bot_a;

            // Round to nearest integer (PIL: truncate after +0.5), clamp to [0, 255]
            let out_r = ((out_r_f + 0.5) as u32).min(255);
            let out_g_raw = ((out_g_f + 0.5) as u32).min(255);
            let out_b_raw = ((out_b_f + 0.5) as u32).min(255);
            let out_a_raw = ((out_a_f + 0.5) as u32).min(255);

            // Mode-aware channel output
            let out_g = if has_gb { out_g_raw } else { out_r };
            let out_b = if has_gb { out_b_raw } else { out_r };
            // Alpha: non-alpha modes force 0xFF in high byte; alpha modes use interpolated value
            let out_a = if has_a { out_a_raw << 24 } else { 0xFF00_0000 };

            output[(dy * dst_w + dx) as usize] = out_r | (out_g << 8) | (out_b << 16) | out_a;
        }
    }
}

// ── Image sizing operations (contain, cover, fit, quantize) ────────────────────

/// Contain: scale image to fit WITHIN dst_w × dst_h, preserving aspect ratio.
/// Scale = min(dst_w/w, dst_h/h). Delegates to bilinear resize.
/// Returns (resized_pixels, new_w, new_h).
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn contain(
    pixels: &[u32],
    w: u32,
    h: u32,
    mode: u32,
    dst_w: u32,
    dst_h: u32,
    filter: u32,
) -> (Vec<u32>, u32, u32) {
    if w == 0 || h == 0 || dst_w == 0 || dst_h == 0 {
        return (pixels.to_vec(), w, h);
    }
    let image_ratio = w as f64 / h as f64;
    let destination_ratio = dst_w as f64 / dst_h as f64;
    let (new_w, new_h) = if image_ratio != destination_ratio {
        if image_ratio > destination_ratio {
            let new_h = round_positive_ties_even(h as f64 / w as f64 * dst_w as f64);
            (dst_w, new_h.max(1))
        } else {
            let new_w = round_positive_ties_even(w as f64 / h as f64 * dst_h as f64);
            (new_w.max(1), dst_h)
        }
    } else {
        (dst_w, dst_h)
    };
    resize(pixels, w, h, new_w, new_h, mode, filter)
}

/// Scale: resize by a floating-point factor using the requested filter.
/// factor: scale multiplier (0.5 = half size, 2.0 = double).
/// Delegates to [`resize`] with the normalized filter code.
/// Returns (pixels, dst_w, dst_h).
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn scale(
    pixels: &[u32],
    w: u32,
    h: u32,
    mode: u32,
    factor: f64,
    filter: u32,
) -> (Vec<u32>, u32, u32) {
    // ImageOps.scale follows Python's round(width * factor), including
    // ties-to-even at half-pixel products (13 * 1.5 -> 20, 11 * 1.5 -> 16).
    let dst_w = round_positive_ties_even(w as f64 * factor).max(1);
    let dst_h = round_positive_ties_even(h as f64 * factor).max(1);
    resize(pixels, w, h, dst_w, dst_h, mode, filter)
}

/// Cover: scale image to COVER dst_w × dst_h, preserving aspect ratio.
/// Scale = max(dst_w/w, dst_h/h). Resize then center-crop to dst_w × dst_h.
/// Returns (cropped_pixels, actual_w, actual_h).
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn cover(
    pixels: &[u32],
    w: u32,
    h: u32,
    mode: u32,
    dst_w: u32,
    dst_h: u32,
    filter: u32,
) -> (Vec<u32>, u32, u32) {
    if w == 0 || h == 0 || dst_w == 0 || dst_h == 0 {
        return (pixels.to_vec(), w, h);
    }
    let image_ratio = w as f64 / h as f64;
    let destination_ratio = dst_w as f64 / dst_h as f64;
    let (new_w, new_h) = if image_ratio != destination_ratio {
        if image_ratio < destination_ratio {
            let new_h = round_positive_ties_even(h as f64 / w as f64 * dst_w as f64);
            (dst_w, new_h.max(1))
        } else {
            let new_w = round_positive_ties_even(w as f64 / h as f64 * dst_h as f64);
            (new_w.max(1), dst_h)
        }
    } else {
        (dst_w, dst_h)
    };
    // ImageOps.cover returns the resized image. It deliberately does not
    // crop the overflow back to the requested box; the old SIMD path did so
    // and first diverged on non-square targets.
    resize(pixels, w, h, new_w, new_h, mode, filter)
}

/// Fit: like contain but with bleed (zoom) and centering offset.
/// Scale = min(dst_w/w, dst_h/h) * (1.0 + bleed). Resize then crop with centering.
/// Returns (cropped_pixels, actual_w, actual_h).
/// bleed: zoom factor (0.0 = no bleed, 0.5 maximum typical).
/// centering: (cx, cy) in [0,1] where (0,0)=top-left, (1,1)=bottom-right.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn fit(
    pixels: &[u32],
    w: u32,
    h: u32,
    mode: u32,
    dst_w: u32,
    dst_h: u32,
    bleed: f32,
    centering: (f32, f32),
    filter: u32,
) -> (Vec<u32>, u32, u32) {
    // Pillow's ImageOps.fit can materialize a zero-width, non-empty source:
    // the destination is still allocated and remains zero-filled. Other
    // empty-dimension cases retain the historical no-sample result here;
    // callers that need Pillow's division error reject those inputs before
    // reaching this kernel.
    if w == 0 && h > 0 && dst_w > 0 && dst_h > 0 {
        return (vec![0u32; (dst_w * dst_h) as usize], dst_w, dst_h);
    }
    if w == 0 || h == 0 || dst_w == 0 || dst_h == 0 {
        return (pixels.to_vec(), w, h);
    }
    let b = if (0.0..0.5).contains(&bleed) {
        bleed as f64
    } else {
        0.0
    };
    let (cx, cy) = centering;
    let cx = cx.clamp(0.0, 1.0) as f64;
    let cy = cy.clamp(0.0, 1.0) as f64;

    // Match ImageOps.fit's floating-point crop box. Resampling this box
    // directly is important: resizing first and cropping integer pixels loses
    // the fractional centering contract and can even return the wrong size.
    let bleed_w = b * w as f64;
    let bleed_h = b * h as f64;
    let live_w = w as f64 - bleed_w * 2.0;
    let live_h = h as f64 - bleed_h * 2.0;
    let live_ratio = live_w / live_h;
    let output_ratio = dst_w as f64 / dst_h as f64;
    let (crop_w, crop_h) = if live_ratio == output_ratio {
        (live_w, live_h)
    } else if live_ratio >= output_ratio {
        (output_ratio * live_h, live_h)
    } else {
        (live_w, live_w / output_ratio)
    };
    let crop_left = bleed_w + (live_w - crop_w) * cx;
    let crop_top = bleed_h + (live_h - crop_h) * cy;
    let (cropped, _, _) = resize_box(
        pixels,
        w,
        h,
        dst_w,
        dst_h,
        mode,
        filter,
        crop_left,
        crop_top,
        crop_left + crop_w,
        crop_top + crop_h,
    );
    (cropped, dst_w, dst_h)
}

// ── Spatial operations (pad, expand, crop_border) ──────────────────────────────

/// Pad/crop image to dst_w×dst_h, centering the source.
///
/// If destination is larger than source, fills empty areas with fill_rgba.
/// If destination is smaller than source, crops from center.
/// Returns (pixels, dst_w, dst_h).
///
/// Mode-aware: alpha byte clamped for non-alpha modes (L, RGB).
/// For L/LA modes: G = B = R (luma carried in R channel).
/// For RGB/RGBA modes: G and B are independent.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
pub(super) fn round_positive_ties_even(value: f64) -> u32 {
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }
    let lower = value.floor();
    let fraction = value - lower;
    let rounded = if fraction < 0.5 {
        lower
    } else if fraction > 0.5 || (lower as u64) % 2 == 1 {
        lower + 1.0
    } else {
        lower
    };
    rounded.min(u32::MAX as f64) as u32
}

/// Thumbnail: resize to fit within `dst_w x dst_h` while preserving aspect ratio.
///
/// Computes `scale = min(dst_w/w, dst_h/h)`, then `new_w = (w * scale).max(1)`,
/// `new_h = (h * scale).max(1)`, and resizes using bilinear interpolation.
/// Returns `(Vec<u32>, new_w, new_h)`.
/// If no shrink is needed (dst_w >= w && dst_h >= h), returns a copy with alpha clamping.
///
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn thumbnail(
    pixels: &[u32],
    w: u32,
    h: u32,
    mode: u32,
    dst_w: u32,
    dst_h: u32,
    filter: u32,
) -> (Vec<u32>, u32, u32) {
    if dst_w == 0 || dst_h == 0 {
        return (Vec::new(), 0, 0);
    }

    // Match Pillow's round_aspect rule used by the CPU thumbnail executor.
    // Truncating `w * scale` is not equivalent: for example, a 17x11 image
    // bounded by 7x7 must become 7x5, not 7x4.  The SIMD nearest path used to
    // take the truncating route and returned a different image shape from the
    // CPU/Pillow path.
    let aspect = w as f64 / h as f64;
    let (new_w, new_h) = if dst_w as f64 / dst_h as f64 >= aspect {
        let adjusted = thumbnail_round_aspect(dst_h as f64 * aspect, |candidate| {
            (aspect - candidate / dst_h as f64).abs()
        });
        (adjusted, dst_h)
    } else {
        let adjusted = thumbnail_round_aspect(dst_w as f64 / aspect, |candidate| {
            if candidate == 0.0 {
                0.0
            } else {
                (aspect - dst_w as f64 / candidate).abs()
            }
        });
        (dst_w, adjusted)
    };

    if new_w >= w && new_h >= h {
        // No shrink needed — return a copy with alpha clamping
        let has_a = super::mode_has_alpha(mode);
        let out: Vec<u32> = pixels
            .iter()
            .map(|&p| {
                let a = p & 0xFF00_0000;
                let out_a = if has_a { a } else { 0xFF00_0000 };
                (p & 0x00FF_FFFF) | out_a
            })
            .collect();
        return (out, w, h);
    }

    resize(pixels, w, h, new_w, new_h, mode, filter)
}

#[inline]
fn thumbnail_round_aspect(number: f64, key: impl Fn(f64) -> f64) -> u32 {
    let floor = number.trunc();
    if number == floor {
        return floor as u32;
    }
    let ceil = floor + 1.0;
    let best = if key(floor) <= key(ceil) { floor } else { ceil };
    (best as u32).max(1)
}

/// Reduce: downsample by `factor` using block averaging.
///
/// Each output pixel is the average of a `factor x factor` input block.
/// For L/LA modes: only R channel is averaged (G = B = R in output).
/// For RGB/RGBA modes: R, G, B are averaged independently.
/// Alpha is averaged for LA/RGBA, forced to 0xFF for L/RGB.
/// Uses round-to-nearest: `(sum + area/2) / area`.
/// Returns `(Vec<u32>, new_w, new_h)` where `new_w = w/factor, new_h = h/factor`.
///
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn reduce(
    pixels: &[u32],
    w: u32,
    h: u32,
    mode: u32,
    x_factor: u32,
    y_factor: u32,
) -> (Vec<u32>, u32, u32) {
    if x_factor < 2 && y_factor < 2 {
        // Cannot reduce — return copy with alpha clamping
        let has_a = super::mode_has_alpha(mode);
        let out: Vec<u32> = pixels
            .iter()
            .map(|&p| {
                let a = p & 0xFF00_0000;
                let out_a = if has_a { a } else { 0xFF00_0000 };
                (p & 0x00FF_FFFF) | out_a
            })
            .collect();
        return (out, w, h);
    }

    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
    let fx = x_factor;
    let fy = y_factor;
    // Pillow Reduce.c computes ceil(w/xscale) x ceil(h/yscale).
    let new_w = w.div_ceil(fx);
    let new_h = h.div_ceil(fy);
    let mut out = vec![0u32; (new_w * new_h) as usize];

    for y in 0..new_h {
        for x in 0..new_w {
            let mut sum_r = 0u64;
            let mut sum_g = 0u64;
            let mut sum_b = 0u64;
            let mut sum_a = 0u64;
            let mut count = 0u64;

            for dy in 0..fy {
                let sy = y * fy + dy;
                if sy >= h {
                    break;
                }
                let row = sy * w;
                for dx in 0..fx {
                    let sx = x * fx + dx;
                    if sx >= w {
                        break;
                    }
                    let p = pixels[(row + sx) as usize];
                    let alpha = (p >> 24) & 0xFF;
                    let premultiply = |value: u32| (value * alpha + 127) / 255;
                    sum_r += u64::from(if has_a {
                        premultiply(p & 0xFF)
                    } else {
                        p & 0xFF
                    });
                    sum_g += u64::from(if has_a {
                        premultiply((p >> 8) & 0xFF)
                    } else {
                        (p >> 8) & 0xFF
                    });
                    sum_b += u64::from(if has_a {
                        premultiply((p >> 16) & 0xFF)
                    } else {
                        (p >> 16) & 0xFF
                    });
                    sum_a += (p >> 24) as u64;
                    count += 1;
                }
            }

            // Pillow's Reduce.c uses a truncated fixed-point reciprocal for
            // each (possibly partial) block. Integer division rounds some
            // half-way partial blocks one value too high (for example, a
            // single 255 sample in a six-pixel block is 42, not 43).
            let block_average = |sum: u64| -> u32 {
                let multiplier = (1u128 << 32) / (u128::from(count) * 256);
                (((u128::from(sum) + u128::from(count / 2)) * multiplier) >> 24) as u32
            };
            let out_a_raw = block_average(sum_a);
            let unpremultiply = |value: u32| {
                if has_a && out_a_raw != 0 {
                    value.saturating_mul(255) / out_a_raw
                } else {
                    value
                }
            };
            let out_r = unpremultiply(block_average(sum_r));
            let out_g_raw = unpremultiply(block_average(sum_g));
            let out_b_raw = unpremultiply(block_average(sum_b));

            let out_g = if has_gb { out_g_raw } else { out_r };
            let out_b = if has_gb { out_b_raw } else { out_r };
            let out_a_val = if has_a { out_a_raw } else { 255u32 };

            let dst_idx = (y * new_w + x) as usize;
            out[dst_idx] = out_r | (out_g << 8) | (out_b << 16) | (out_a_val << 24);
        }
    }

    (out, new_w, new_h)
}
