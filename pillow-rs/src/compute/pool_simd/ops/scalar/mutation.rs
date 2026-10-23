//! Scalar fallback for scalar pixel, data, alpha, and composite mutations.

/// Set a single pixel at (x,y) to color_rgba (packed 0xAABBGGRR).
///
/// Bounds check: no-op if out of bounds (matching PIL's silent ignore).
/// Mode-aware: for L mode, only R byte of color matters (G=B=R, A=0xFF).
/// For RGB mode, alpha byte is forced to 0xFF.
/// For LA mode, R and A bytes used, G=B=R.
/// For RGBA mode, full pixel written as-is.
///
/// Operates in-place on pixels slice.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn put_pixel(pixels: &mut [u32], w: u32, mode: u32, x: u32, y: u32, color_rgba: u32) {
    if x >= w {
        return;
    }
    let idx = (y * w + x) as usize;
    if idx >= pixels.len() {
        return;
    }

    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);

    let r = color_rgba & 0xFF;
    let g = (color_rgba >> 8) & 0xFF;
    let b = (color_rgba >> 16) & 0xFF;
    let a = color_rgba & 0xFF00_0000;

    let out_g = if has_gb { g } else { r };
    let out_b = if has_gb { b } else { r };
    let out_a = if has_a { a } else { 0xFF00_0000 };

    pixels[idx] = r | (out_g << 8) | (out_b << 16) | out_a;
}

/// Replace pixel data with logical Pillow-mode sample bytes.
///
/// Operates in-place on pixels slice.
/// Mode codes are defined by `pipeline::PixelMode`.
#[inline]
pub fn put_data(pixels: &mut [u32], mode: u32, data: &[u8]) {
    let channels = match mode {
        0 | 4 | 7 => 1,
        1 | 5 => 2,
        2 | 8 | 9 => 3,
        _ => 4,
    };
    let n_copy = data.len().min(pixels.len() * channels);

    for (sample_index, &sample) in data[..n_copy].iter().enumerate() {
        let pixel_index = sample_index / channels;
        let channel = sample_index % channels;
        let pixel = &mut pixels[pixel_index];
        let value = sample as u32;
        match mode {
            // L, P, and 1 use the R byte as their packed scalar sample.
            0 | 4 | 7 => {
                *pixel = value | (value << 8) | (value << 16) | 0xFF00_0000;
            }
            // Packed LA/PA stores luma in R and alpha in A.
            1 | 5 => {
                if channel == 0 {
                    *pixel = (*pixel & 0xFF00_0000) | value | (value << 8) | (value << 16);
                } else {
                    *pixel = (*pixel & 0x00FF_FFFF) | (value << 24);
                }
            }
            // RGB-family raw samples occupy the low three packed bytes.
            2 | 8 | 9 => {
                let shift = (channel * 8) as u32;
                *pixel = (*pixel & !(0xFF << shift)) | (value << shift);
                *pixel |= 0xFF00_0000;
            }
            // RGBA, CMYK, I, and F are all four raw bytes in order.
            _ => {
                let shift = (channel * 8) as u32;
                *pixel = (*pixel & !(0xFF << shift)) | (value << shift);
            }
        }
    }
}

/// Set alpha channel of all pixels to `alpha` (0-255).
///
/// Operates in-place on pixels slice.
/// Mode codes are defined by `pipeline::PixelMode`.
#[inline]
pub fn put_alpha(pixels: &mut [u32], mode: u32, alpha: u8) {
    let a_packed = (alpha as u32) << 24;
    for p in pixels.iter_mut() {
        if mode == 6 {
            // Pillow Convert.c:cmyk2rgb uses MULDIV255:
            // t = channel * (255-K) + 128; ((t >> 8) + t) >> 8.
            let c = *p & 0xFF;
            let m = (*p >> 8) & 0xFF;
            let y = (*p >> 16) & 0xFF;
            let k = (*p >> 24) & 0xFF;
            let nk = 255 - k;
            let convert = |channel: u32| {
                let t = channel * nk + 128;
                nk.saturating_sub(((t >> 8) + t) >> 8)
            };
            let r = convert(c);
            let g = convert(m);
            let b = convert(y);
            *p = r | (g << 8) | (b << 16) | a_packed;
        } else {
            *p = (*p & 0x00FF_FFFF) | a_packed;
        }
    }
}

/// Composite two images using a single mask band.
///
/// Formula: `out = (pixels * mask + other * (255 - mask)) / 255`
/// Applied per byte (all 4 channels independently).
///
/// Module-function variant (`Image.composite`): `1`/`L` masks use the low
/// luma byte while `LA`/`RGBA`/`RGBa` masks use the alpha byte.
///
/// Mode-aware: for L/LA modes (mode < 2), G and B mirror R.
/// Alpha preserved for LA/RGBA, forced to 0xFF for L/RGB.
///
/// The result starts as image2 and image1 is blended into its top-left overlap,
/// matching Pillow's `image2.copy(); image2.paste(image1, mask)` implementation.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
#[allow(clippy::too_many_arguments)]
pub fn composite_module(
    pixels: &[u32],
    width: u32,
    height: u32,
    mode: u32,
    other: &[u32],
    other_width: u32,
    other_height: u32,
    mask: &[u32],
    mask_width: u32,
    mask_height: u32,
    mask_alpha: bool,
) -> Vec<u32> {
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
    let mut out = other.to_vec();
    let overlap_width = width.min(other_width).min(mask_width);
    let overlap_height = height.min(other_height).min(mask_height);

    for y in 0..overlap_height {
        for x in 0..overlap_width {
            let source = pixels[(y * width + x) as usize];
            let destination = other[(y * other_width + x) as usize];
            let mask_pixel = mask[(y * mask_width + x) as usize];
            let amount = if mask_alpha {
                (mask_pixel >> 24) & 0xFF
            } else {
                mask_pixel & 0xFF
            };
            let inverse = 255 - amount;
            let blend = |source: u32, destination: u32| {
                (source * amount + destination * inverse + 127) / 255
            };

            let out_r = blend(source & 0xFF, destination & 0xFF);
            let out_g_raw = blend((source >> 8) & 0xFF, (destination >> 8) & 0xFF);
            let out_b_raw = blend((source >> 16) & 0xFF, (destination >> 16) & 0xFF);
            let out_a_raw = blend((source >> 24) & 0xFF, (destination >> 24) & 0xFF);
            let out_g = if has_gb { out_g_raw } else { out_r };
            let out_b = if has_gb { out_b_raw } else { out_r };
            let out_a = if has_a { out_a_raw << 24 } else { 0xFF00_0000 };

            out[(y * other_width + x) as usize] = out_r | (out_g << 8) | (out_b << 16) | out_a;
        }
    }
    out
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Effects + Lookup operations
//  (paste, alpha_composite, eval, point_op)
// ═══════════════════════════════════════════════════════════════════════════════

/// Paste: copy source image pixels onto destination at (paste_x, paste_y).
///
/// With mask: `out = (src * mask + dst * (255 - mask) + 127) / 255` (rounded division).
/// Without mask: opaque paste — src replaces dst in the paste rect.
/// Source may extend beyond dst bounds — clamped.
///
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn paste(
    pixels: &mut [u32],
    w: u32,
    h: u32,
    mode: u32,
    source: &[u32],
    src_w: u32,
    src_h: u32,
    paste_x: i32,
    paste_y: i32,
    mask: Option<&[u32]>,
    mask_alpha: bool,
) {
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);

    // Compute paste region clamped to destination bounds.
    // x_start/y_start = first source pixel visible in dest (0 if paste_x >= 0).
    // x_end/y_end     = last source pixel visible in dest (src_w if paste fits).
    let x_start = if paste_x < 0 {
        paste_x.unsigned_abs()
    } else {
        0
    };
    let y_start = if paste_y < 0 {
        paste_y.unsigned_abs()
    } else {
        0
    };
    // libImaging/Paste.c clips the destination rectangle and advances the
    // source origin by the clipped leading edge. For a negative placement the
    // visible source end therefore extends by that same source offset.
    let x_end = if paste_x < 0 {
        src_w.min(w.saturating_add(x_start))
    } else {
        src_w.min(w.saturating_sub(paste_x as u32))
    };
    let y_end = if paste_y < 0 {
        src_h.min(h.saturating_add(y_start))
    } else {
        src_h.min(h.saturating_sub(paste_y as u32))
    };

    if x_start >= x_end || y_start >= y_end {
        return;
    }

    let dst_ox = paste_x.max(0) as usize;
    let dst_oy = paste_y.max(0) as usize;
    let w_u = w as usize;
    let src_w_u = src_w as usize;

    if let Some(mask_pixels) = mask {
        // ── With mask: weighted blend per active channel ──
        for sy in y_start..y_end {
            for sx in x_start..x_end {
                let src_idx = (sy as usize) * src_w_u + (sx as usize);
                let dst_idx =
                    (dst_oy + (sy - y_start) as usize) * w_u + (dst_ox + (sx - x_start) as usize);

                let sp = source[src_idx];
                let dp = pixels[dst_idx];
                let mv = if mask_alpha {
                    (mask_pixels[src_idx] >> 24) & 0xFF
                } else {
                    mask_pixels[src_idx] & 0xFF
                };

                if mv == 0 {
                    continue;
                }
                if mv == 255 {
                    // Opaque mask: paste source pixel, mode-aware
                    let sr = sp & 0xFF;
                    let sg = (sp >> 8) & 0xFF;
                    let sb = (sp >> 16) & 0xFF;
                    let sa = sp & 0xFF00_0000;

                    let out_r = sr;
                    let out_g = if has_gb { sg } else { sr };
                    let out_b = if has_gb { sb } else { sr };
                    let out_a = if has_a { sa } else { 0xFF00_0000 };
                    pixels[dst_idx] = out_r | (out_g << 8) | (out_b << 16) | out_a;
                } else {
                    // Partial mask: (src * mask + dst * (255 - mask) + 127) / 255
                    let mv_u32 = mv;
                    let inv_mv = 255u32 - mv_u32;

                    let sr = sp & 0xFF;
                    let sg = (sp >> 8) & 0xFF;
                    let sb = (sp >> 16) & 0xFF;
                    let dr = dp & 0xFF;
                    let dg = (dp >> 8) & 0xFF;
                    let db = (dp >> 16) & 0xFF;

                    let out_r = (sr * mv_u32 + dr * inv_mv + 127) / 255;
                    let out_g_raw = (sg * mv_u32 + dg * inv_mv + 127) / 255;
                    let out_b_raw = (sb * mv_u32 + db * inv_mv + 127) / 255;

                    let out_g = if has_gb { out_g_raw } else { out_r };
                    let out_b = if has_gb { out_b_raw } else { out_r };
                    let out_a = if has_a {
                        let sa_val = (sp >> 24) & 0xFF;
                        let da_val = (dp >> 24) & 0xFF;
                        ((sa_val * mv_u32 + da_val * inv_mv + 127) / 255) << 24
                    } else {
                        0xFF00_0000
                    };

                    pixels[dst_idx] = out_r | (out_g << 8) | (out_b << 16) | out_a;
                }
            }
        }
    } else {
        // ── No mask: opaque paste — source replaces dest, mode-aware ──
        for sy in y_start..y_end {
            for sx in x_start..x_end {
                let src_idx = (sy as usize) * src_w_u + (sx as usize);
                let dst_idx =
                    (dst_oy + (sy - y_start) as usize) * w_u + (dst_ox + (sx - x_start) as usize);

                let sp = source[src_idx];
                let sr = sp & 0xFF;
                let sg = (sp >> 8) & 0xFF;
                let sb = (sp >> 16) & 0xFF;
                let sa = sp & 0xFF00_0000;

                let out_r = sr;
                let out_g = if has_gb { sg } else { sr };
                let out_b = if has_gb { sb } else { sr };
                let out_a = if has_a { sa } else { 0xFF00_0000 };

                pixels[dst_idx] = out_r | (out_g << 8) | (out_b << 16) | out_a;
            }
        }
    }
}

/// Alpha composite source onto destination at specified rectangles.
///
/// Standard Porter-Duff "over":
///   out_A = src_A + dst_A * (1 - src_A/255)
///   out_RGB = (src_RGB * src_A + dst_RGB * dst_A * (1 - src_A/255)) / out_A
///
/// Operates on the intersection of source and destination rectangles.
/// For L/RGB modes (no alpha channel): implicit alpha = 255 for both images,
/// so the compositing reduces to source-replaces-dest (correct per PIL).
///
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn alpha_composite(
    pixels: &mut [u32],
    w: u32,
    h: u32,
    mode: u32,
    source: &[u32],
    src_w: u32,
    src_h: u32,
    dest_x: i32,
    dest_y: i32,
    src_x: i32,
    src_y: i32,
) {
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);

    // Clamp rectangle starts to valid range
    let d_x = dest_x.max(0) as u32;
    let d_y = dest_y.max(0) as u32;
    let s_x = src_x.max(0) as u32;
    let s_y = src_y.max(0) as u32;

    // Work dimensions = intersection of what fits in both images
    let work_w = src_w.saturating_sub(s_x).min(w.saturating_sub(d_x));
    let work_h = src_h.saturating_sub(s_y).min(h.saturating_sub(d_y));

    if work_w == 0 || work_h == 0 {
        return;
    }

    let w_u = w as usize;
    let src_w_u = src_w as usize;

    // For L/RGB modes (no alpha channel), pixel alpha is implicitly 255.
    // Compositing two fully opaque images gives out = source (Porter-Duff over
    // with sa=da=1 -> out_a=1, out_RGB=src_RGB).
    let implicit_alpha = !has_a;

    for dy in 0..work_h {
        for dx in 0..work_w {
            let src_idx = ((s_y + dy) as usize) * src_w_u + (s_x + dx) as usize;
            let dst_idx = ((d_y + dy) as usize) * w_u + (d_x + dx) as usize;

            let sp = source[src_idx];
            let dp = pixels[dst_idx];

            // ── Alpha values (or implicit 1.0 for non-alpha modes) ──
            let sa = if implicit_alpha {
                1.0
            } else {
                ((sp >> 24) & 0xFF) as f64 / 255.0
            };
            let da = if implicit_alpha {
                1.0
            } else {
                ((dp >> 24) & 0xFF) as f64 / 255.0
            };
            let out_a_f = sa + da * (1.0 - sa);
            if out_a_f <= 0.0 {
                continue;
            }

            let sr = (sp & 0xFF) as f64;
            let sg = ((sp >> 8) & 0xFF) as f64;
            let sb = ((sp >> 16) & 0xFF) as f64;
            let dr = (dp & 0xFF) as f64;
            let dg = ((dp >> 8) & 0xFF) as f64;
            let db = ((dp >> 16) & 0xFF) as f64;

            // Porter-Duff over: out_ch = (src_ch * sa + dst_ch * da * (1 - sa)) / out_a
            let inv_sa = 1.0 - sa;
            let out_r = ((sr * sa + dr * da * inv_sa) / out_a_f)
                .round()
                .clamp(0.0, 255.0) as u32;
            let out_g_raw = ((sg * sa + dg * da * inv_sa) / out_a_f)
                .round()
                .clamp(0.0, 255.0) as u32;
            let out_b_raw = ((sb * sa + db * da * inv_sa) / out_a_f)
                .round()
                .clamp(0.0, 255.0) as u32;

            let out_g = if has_gb { out_g_raw } else { out_r };
            let out_b = if has_gb { out_b_raw } else { out_r };
            let out_a_byte = if has_a {
                (out_a_f * 255.0).round().clamp(0.0, 255.0) as u32
            } else {
                255u32
            };

            pixels[dst_idx] = out_r | (out_g << 8) | (out_b << 16) | (out_a_byte << 24);
        }
    }
}
