//! ImageOps CPU operations extracted from image.rs execute_op().
//! These implement PIL-compatible image operations: autocontrast, equalize,
//! invert, flip, mirror, posterize, solarize, grayscale, colorize,
//! contain, cover, fit, pad, scale, expand, and crop border.

use image_slash_star::{DynamicImage, GenericImage, GenericImageView};

use crate::color::pil_grayscale;

/// Python 3's round() (banker's rounding): rounds half to even.
/// This matches PIL's behavior: round(12.5) -> 12, round(13.5) -> 14.
fn bankers_round(x: f64) -> f64 {
    let floor = x.floor();
    let frac = x - floor;
    if frac == 0.5 {
        if floor % 2.0 == 0.0 {
            floor
        } else {
            floor + 1.0
        }
    } else {
        (x + 0.5).floor()
    }
}
use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::ops::pil_resize::pil_resize;
use crate::ops::pil_resize::pil_resize_boxed;
use crate::pipeline::ResampleFilter;

/// Autocontrast: stretch image contrast based on histogram cutoff.
/// PIL: per-channel histogram, find lo/hi at cutoff percentiles for each channel,
/// then linearly map [lo, hi] to [0, 255] using truncation (int() cast).
pub fn op_autocontrast(img: &DynamicImage, cutoff: f64) -> Result<DynamicImage, PilError> {
    let channels = img.color().channel_count() as usize;
    let (w, h) = (img.width(), img.height());
    let dims = CheckedDims::new(w, h, 1)?;
    let total = dims.total_pixels() as f64;
    let raw = img.as_bytes();
    let mut out = raw.to_vec();
    let stride = w as usize * channels;
    for c in 0..channels {
        // Build sorted list of pixel values for this channel
        let mut sorted: Vec<u8> = Vec::with_capacity(dims.total_pixels());
        for y in 0..h as usize {
            for x in 0..w as usize {
                sorted.push(raw[y * stride + x * channels + c]);
            }
        }
        sorted.sort_unstable();
        let low_thresh = (total * cutoff / 100.0) as usize;
        let high_thresh = (total * (100.0 - cutoff) / 100.0) as usize;
        let lo = *sorted.get(low_thresh).unwrap_or(&0) as f64;
        let hi = *sorted
            .get(high_thresh.min(sorted.len() - 1))
            .unwrap_or(&255) as f64;
        if hi <= lo {
            continue; // No stretch for this channel
        }
        let scale = 255.0 / (hi - lo);
        let offset = -lo * scale;
        for y in 0..h as usize {
            for x in 0..w as usize {
                let idx = y * stride + x * channels + c;
                // PIL: int(ix * scale + offset) with clamping to [0,255]
                // Uses PIL's exact formula to match floating-point edge cases.
                // (ix - lo) * scale can produce different fp results than ix*scale + offset
                let val = out[idx] as f64 * scale + offset;
                out[idx] = if val < 0.0 {
                    0
                } else if val > 255.0 {
                    255
                } else {
                    val as u8
                };
            }
        }
    }
    let result = super::filter::raw_bytes_to_image(w, h, out, channels)?;
    Ok(preserve_mode(img, result))
}

/// Equalize: histogram equalization matching PIL's algorithm.
/// Build LUT from non-zero histogram bins, using PIL's step formula.
pub fn op_equalize(img: &DynamicImage) -> Result<DynamicImage, PilError> {
    // PIL 12 equalize: build LUT from non-zero histogram bins
    // step = (sum(non_zero_bins) - last_bin_count) / 255
    // lut[i] = floor(accumulator / step) where accumulator tracks step/2 + cumulative hist
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let mut out = image_slash_star::RgbImage::new(w, h);
    for ch in 0..3 {
        let mut hist = [0u32; 256];
        for px in rgb.pixels() {
            hist[px[ch] as usize] += 1;
        }
        // Collect non-zero bins
        let nonzero: Vec<u32> = hist.iter().filter(|&&c| c > 0).copied().collect();
        if nonzero.len() <= 1 {
            // Identity LUT
            continue; // out already has original pixels from the RgbImage
        }
        let total: u32 = nonzero.iter().sum();
        let step = (total - nonzero[nonzero.len() - 1]) / 255;
        if step == 0 {
            continue; // Identity LUT
        }
        let mut n = step / 2;
        let mut lut = [0u8; 256];
        for i in 0..256 {
            lut[i] = (n / step).min(255) as u8;
            n += hist[i];
        }
        for (opx, ipx) in out.pixels_mut().zip(rgb.pixels()) {
            opx[ch] = lut[ipx[ch] as usize];
        }
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(out)))
}

/// Invert: subtract each pixel value from 255 (all channels, matching PIL's point()).
pub fn op_invert(img: &DynamicImage) -> Result<DynamicImage, PilError> {
    let channels = img.color().channel_count() as usize;
    let (w, h) = (img.width(), img.height());
    let raw = img.as_bytes();
    let mut out = raw.to_vec();
    let stride = w as usize * channels;
    for y in 0..h as usize {
        for x in 0..w as usize {
            for c in 0..channels {
                let idx = y * stride + x * channels + c;
                out[idx] = 255 - out[idx];
            }
        }
    }
    let result = match channels {
        1 => image_slash_star::GrayImage::from_raw(w, h, out)
            .map(DynamicImage::ImageLuma8)
            .ok_or_else(|| PilError::InternalError("invert L buffer shape mismatch".to_string()))?,
        2 => image_slash_star::GrayAlphaImage::from_raw(w, h, out)
            .map(DynamicImage::ImageLumaA8)
            .ok_or_else(|| {
                PilError::InternalError("invert LA buffer shape mismatch".to_string())
            })?,
        3 => image_slash_star::RgbImage::from_raw(w, h, out)
            .map(DynamicImage::ImageRgb8)
            .ok_or_else(|| {
                PilError::InternalError("invert RGB buffer shape mismatch".to_string())
            })?,
        _ => image_slash_star::RgbaImage::from_raw(w, h, out)
            .map(DynamicImage::ImageRgba8)
            .ok_or_else(|| {
                PilError::InternalError("invert RGBA buffer shape mismatch".to_string())
            })?,
    };
    Ok(result)
}

/// Flip vertically.
pub fn op_flip(img: &DynamicImage) -> Result<DynamicImage, PilError> {
    Ok(img.flipv())
}

/// Mirror horizontally.
pub fn op_mirror(img: &DynamicImage) -> Result<DynamicImage, PilError> {
    Ok(img.fliph())
}

/// Posterize: reduce the number of bits per channel.
pub fn op_posterize(img: &DynamicImage, bits: u8) -> Result<DynamicImage, PilError> {
    let mask = !((1u8 << (8 - bits)) - 1);
    let mut rgb = img.to_rgb8();
    for p in rgb.pixels_mut() {
        for c in 0..3 {
            p[c] &= mask;
        }
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(rgb)))
}

/// Solarize: invert pixels where value >= threshold.
/// PIL uses >=, not >.
pub fn op_solarize(img: &DynamicImage, threshold: u8) -> Result<DynamicImage, PilError> {
    let t = threshold;
    let mut rgb = img.to_rgb8();
    for p in rgb.pixels_mut() {
        for c in 0..3 {
            if p[c] >= t {
                // PIL uses >=, not >
                p[c] = 255 - p[c];
            }
        }
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(rgb)))
}

/// Grayscale: convert to L-mode using PIL's BT.601 formula.
pub fn op_grayscale(img: &DynamicImage) -> Result<DynamicImage, PilError> {
    Ok(DynamicImage::ImageLuma8(pil_grayscale(img)?))
}

/// Colorize: map grayscale values to a two-color gradient.
/// Always outputs RGB (PIL behavior).
pub fn op_colorize(
    img: &DynamicImage,
    black: &(u8, u8, u8),
    white: &(u8, u8, u8),
) -> Result<DynamicImage, PilError> {
    let gray = img.to_luma8();
    let (w, h) = gray.dimensions();
    let mut out = image_slash_star::RgbImage::new(w, h);
    let &(br, bg, bb) = black;
    let &(wr, wg, wb) = white;
    for y in 0..h {
        for x in 0..w {
            let g = gray.get_pixel(x, y)[0] as f64 / 255.0;
            let r = (br as f64 + g * (wr as f64 - br as f64)) as u8;
            let gv = (bg as f64 + g * (wg as f64 - bg as f64)) as u8;
            let b = (bb as f64 + g * (wb as f64 - bb as f64)) as u8;
            out.put_pixel(x, y, image_slash_star::Rgb([r, gv, b]));
        }
    }
    // Colorize always outputs RGB (PIL behavior)
    Ok(DynamicImage::ImageRgb8(out))
}

/// Contain: resize to fit within (w, h) preserving aspect ratio.
/// PIL: adjusts one dimension using round(), does not truncate.
pub fn op_contain(
    img: &DynamicImage,
    w: u32,
    h: u32,
    filter: ResampleFilter,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (iw, ih) = (img.width(), img.height());
    let im_ratio = iw as f64 / ih as f64;
    let dest_ratio = w as f64 / h as f64;
    let (nw, nh) = if (im_ratio - dest_ratio).abs() < 1e-10 {
        (w, h)
    } else if im_ratio > dest_ratio {
        // Image is wider: adjust height
        let new_h = bankers_round(ih as f64 / iw as f64 * w as f64) as u32;
        (w, new_h)
    } else {
        // Image is taller: adjust width
        let new_w = bankers_round(iw as f64 / ih as f64 * h as f64) as u32;
        (new_w, h)
    };
    let result = pil_resize(img, nw.max(1), nh.max(1), filter, explicit_mode);
    Ok(preserve_mode(img, result))
}

/// Cover: resize to cover (w, h) preserving aspect ratio.
/// PIL: adjusts one dimension using round(), does NOT crop.
pub fn op_cover(
    img: &DynamicImage,
    w: u32,
    h: u32,
    filter: ResampleFilter,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (iw, ih) = (img.width(), img.height());
    let im_ratio = iw as f64 / ih as f64;
    let dest_ratio = w as f64 / h as f64;
    let (nw, nh) = if (im_ratio - dest_ratio).abs() < 1e-10 {
        (w, h)
    } else if im_ratio < dest_ratio {
        // Image is taller: adjust height to cover
        let new_h = bankers_round(ih as f64 / iw as f64 * w as f64) as u32;
        (w, new_h)
    } else {
        // Image is wider: adjust width to cover
        let new_w = bankers_round(iw as f64 / ih as f64 * h as f64) as u32;
        (new_w, h)
    };
    let result = pil_resize(img, nw.max(1), nh.max(1), filter, explicit_mode);
    Ok(preserve_mode(img, result))
}

/// Fit: resize to fit within (w, h) with bleed and centering, then crop.
/// PIL: applies bleed to source, computes crop box, resize with box parameter.
/// Uses PIL's exact box-based resize to match pixel-perfect output.
pub fn op_fit(
    img: &DynamicImage,
    w: u32,
    h: u32,
    filter: ResampleFilter,
    bleed: f64,
    centering: (f64, f64),
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (iw, ih) = (img.width(), img.height());
    // Bleed pixels (PIL: bleed * image.size)
    let bleed_w = bleed * iw as f64;
    let bleed_h = bleed * ih as f64;
    // Live size
    let live_w = (iw as f64 - 2.0 * bleed_w).max(1.0);
    let live_h = (ih as f64 - 2.0 * bleed_h).max(1.0);
    let live_ratio = live_w / live_h;
    let output_ratio = w as f64 / h as f64;
    // Compute crop dimensions (PIL: floats, no rounding)
    let (crop_w, crop_h) = if (live_ratio - output_ratio).abs() < 1e-10 {
        (live_w, live_h)
    } else if live_ratio >= output_ratio {
        // Live is wider: crop sides
        (output_ratio * live_h, live_h)
    } else {
        // Live is taller: crop top/bottom
        (live_w, live_w / output_ratio)
    };
    // Compute crop position with centering (PIL: floats, no rounding)
    let crop_left = bleed_w + (live_w - crop_w) * centering.0;
    let crop_top = bleed_h + (live_h - crop_h) * centering.1;
    // Use PIL's box-based resize (maps source box to target size)
    let result = pil_resize_boxed(
        img,
        w.max(1),
        h.max(1),
        crop_left,
        crop_top,
        crop_left + crop_w,
        crop_top + crop_h,
        filter,
        explicit_mode,
    );
    Ok(preserve_mode(img, result))
}

/// Pad: resize to fit within (w, h), then pad with fill color.
/// PIL: contain then paste with centering, using round() for paste offset.
pub fn op_pad(
    img: &DynamicImage,
    w: u32,
    h: u32,
    filter: ResampleFilter,
    color: Option<(u8, u8, u8, u8)>,
    centering: (f64, f64),
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // PIL: Image.new(image.mode, size, color) defaults to mode-appropriate fill
    // RGBA/LA modes: transparent fill (alpha=0). L/RGB: opaque black.
    let has_alpha = matches!(
        img.color(),
        image_slash_star::ColorType::Rgba8 | image_slash_star::ColorType::La8
    );
    let default_fill = if has_alpha {
        (0, 0, 0, 0)
    } else {
        (0, 0, 0, 255)
    };
    let fill = color.unwrap_or(default_fill);
    let (iw, ih) = (img.width(), img.height());
    // Step 1: contain (resize to fit within target)
    let im_ratio = iw as f64 / ih as f64;
    let dest_ratio = w as f64 / h as f64;
    let (nw, nh) = if (im_ratio - dest_ratio).abs() < 1e-10 {
        (w, h)
    } else if im_ratio > dest_ratio {
        let new_h = bankers_round(ih as f64 / iw as f64 * w as f64) as u32;
        (w, new_h)
    } else {
        let new_w = bankers_round(iw as f64 / ih as f64 * h as f64) as u32;
        (new_w, h)
    };
    let resized = pil_resize(img, nw.max(1), nh.max(1), filter, explicit_mode);
    if nw == w && nh == h {
        return Ok(preserve_mode(img, resized));
    }
    // Step 2: pad to target size
    let mut padded = DynamicImage::new_rgba8(w, h);
    for py in 0..h {
        for px in 0..w {
            padded.put_pixel(
                px,
                py,
                image_slash_star::Rgba([fill.0, fill.1, fill.2, fill.3]),
            );
        }
    }
    // PIL: x = round((size[0] - resized.width) * max(0, min(centering[0], 1)))
    let cx = centering.0.clamp(0.0, 1.0);
    let cy = centering.1.clamp(0.0, 1.0);
    let src_rgba = resized.to_rgba8();
    // For RGBA images, PIL's paste alpha-composites the source over the destination.
    // We match this by blending source alpha with destination background.
    if nw != w {
        let ox = bankers_round((w as f64 - nw as f64) * cx) as u32;
        for py in 0..nh.min(h) {
            for px in 0..nw.min(w) {
                let dx = ox + px;
                if dx < w {
                    let sp = *src_rgba.get_pixel(px, py);
                    let dp = padded.get_pixel(dx, py);
                    let sa = sp[3] as u32;
                    let da = dp[3] as u32;
                    let oa = sa + (da * (255u32 - sa)) / 255;
                    if oa > 0 {
                        let or = ((sp[0] as u32 * sa + dp[0] as u32 * da * (255u32 - sa) / 255)
                            / oa) as u8;
                        let og = ((sp[1] as u32 * sa + dp[1] as u32 * da * (255u32 - sa) / 255)
                            / oa) as u8;
                        let ob = ((sp[2] as u32 * sa + dp[2] as u32 * da * (255u32 - sa) / 255)
                            / oa) as u8;
                        padded.put_pixel(dx, py, image_slash_star::Rgba([or, og, ob, oa as u8]));
                    }
                }
            }
        }
    } else {
        let oy = bankers_round((h as f64 - nh as f64) * cy) as u32;
        for py in 0..nh.min(h) {
            for px in 0..nw.min(w) {
                let dy = oy + py;
                if dy < h {
                    let sp = *src_rgba.get_pixel(px, py);
                    let dp = padded.get_pixel(px, dy);
                    let sa = sp[3] as u32;
                    let da = dp[3] as u32;
                    let oa = sa + (da * (255u32 - sa)) / 255;
                    if oa > 0 {
                        let or = ((sp[0] as u32 * sa + dp[0] as u32 * da * (255u32 - sa) / 255)
                            / oa) as u8;
                        let og = ((sp[1] as u32 * sa + dp[1] as u32 * da * (255u32 - sa) / 255)
                            / oa) as u8;
                        let ob = ((sp[2] as u32 * sa + dp[2] as u32 * da * (255u32 - sa) / 255)
                            / oa) as u8;
                        padded.put_pixel(px, dy, image_slash_star::Rgba([or, og, ob, oa as u8]));
                    }
                }
            }
        }
    }
    Ok(preserve_mode(img, padded))
}

/// CropBorder: remove `border` pixels from all four sides.
pub fn op_crop_border(img: &DynamicImage, border: u32) -> Result<DynamicImage, PilError> {
    let b = border;
    let (w, h) = (img.width(), img.height());
    if 2 * b >= w || 2 * b >= h {
        return Err(PilError::ValueError(
            "crop border exceeds image dimensions".into(),
        ));
    }
    Ok(img.crop_imm(b, b, w - 2 * b, h - 2 * b))
}

/// Scale: resize by a floating-point factor.
pub fn op_scale(
    img: &DynamicImage,
    factor: f64,
    filter: ResampleFilter,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let new_w = (img.width() as f64 * factor).round() as u32;
    let new_h = (img.height() as f64 * factor).round() as u32;
    let result = pil_resize(img, new_w.max(1), new_h.max(1), filter, explicit_mode);
    Ok(preserve_mode(img, result))
}

/// Expand: add a border of `border` pixels with `fill` color around the image.
/// The fill is a 4-tuple (r,g,b,a). Mode-appropriate fill resolution is done
/// in the Python binding layer to match PIL's Image.new behavior.
pub fn op_expand(
    img: &DynamicImage,
    border: u32,
    fill: (u8, u8, u8, u8),
) -> Result<DynamicImage, PilError> {
    let (w, h) = (img.width(), img.height());
    let new_w = w + 2 * border;
    let new_h = h + 2 * border;
    let mut expanded = DynamicImage::new_rgba8(new_w, new_h);
    for py in 0..new_h {
        for px in 0..new_w {
            expanded.put_pixel(
                px,
                py,
                image_slash_star::Rgba([fill.0, fill.1, fill.2, fill.3]),
            );
        }
    }
    let src_rgba = img.to_rgba8();
    let (sw, sh) = (src_rgba.width(), src_rgba.height());
    for py in 0..sh.min(expanded.height()) {
        for px in 0..sw.min(expanded.width()) {
            let dx = (border as i64 + px as i64) as u32;
            let dy = (border as i64 + py as i64) as u32;
            if dx < expanded.width() && dy < expanded.height() {
                expanded.put_pixel(dx, dy, *src_rgba.get_pixel(px, py));
            }
        }
    }
    Ok(preserve_mode(img, expanded))
}

/// Generate a 256x256 linear gradient (top-to-bottom, black-to-white).
pub fn op_linear_gradient(mode: &crate::pipeline::ColorMode) -> Result<DynamicImage, PilError> {
    use crate::pipeline::ColorMode;
    let w = 256u32;
    let h = 256u32;
    match mode {
        ColorMode::L => {
            let mut gray = image_slash_star::GrayImage::new(w, h);
            for y in 0..h {
                let val = y as u8;
                for x in 0..w {
                    gray.put_pixel(x, y, image_slash_star::Luma([val]));
                }
            }
            Ok(DynamicImage::ImageLuma8(gray))
        }
        _ => {
            // Default to RGB
            let mut rgb = image_slash_star::RgbImage::new(w, h);
            for y in 0..h {
                let val = y as u8;
                for x in 0..w {
                    rgb.put_pixel(x, y, image_slash_star::Rgb([val, val, val]));
                }
            }
            Ok(DynamicImage::ImageRgb8(rgb))
        }
    }
}

/// Generate a 256x256 radial gradient (center-out, black-to-white).
pub fn op_radial_gradient(mode: &crate::pipeline::ColorMode) -> Result<DynamicImage, PilError> {
    use crate::pipeline::ColorMode;
    let w = 256u32;
    let h = 256u32;
    let cx = 128.0f64;
    let cy = 128.0f64;
    let max_dist = (cx * cx + cy * cy).sqrt();
    match mode {
        ColorMode::L => {
            let mut gray = image_slash_star::GrayImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let dx = x as f64 - cx;
                    let dy = y as f64 - cy;
                    let dist = (dx * dx + dy * dy).sqrt();
                    let val = ((dist / max_dist * 255.0 + 0.5).min(255.0)) as u8;
                    gray.put_pixel(x, y, image_slash_star::Luma([val]));
                }
            }
            Ok(DynamicImage::ImageLuma8(gray))
        }
        _ => {
            let mut rgb = image_slash_star::RgbImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let dx = x as f64 - cx;
                    let dy = y as f64 - cy;
                    let dist = (dx * dx + dy * dy).sqrt();
                    let val = ((dist / max_dist * 255.0 + 0.5).min(255.0)) as u8;
                    rgb.put_pixel(x, y, image_slash_star::Rgb([val, val, val]));
                }
            }
            Ok(DynamicImage::ImageRgb8(rgb))
        }
    }
}
