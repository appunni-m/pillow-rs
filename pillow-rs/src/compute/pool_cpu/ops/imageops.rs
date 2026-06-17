//! ImageOps CPU operations extracted from image.rs execute_op().
//! These implement PIL-compatible image operations: autocontrast, equalize,
//! invert, flip, mirror, posterize, solarize, grayscale, colorize,
//! contain, cover, fit, pad, scale, expand, and crop border.

use pillow_rs_image::{DynamicImage, GenericImage};

use crate::color::pil_grayscale;
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::ops::pil_resize::pil_resize;
use crate::pipeline::ResampleFilter;

/// Autocontrast: stretch image contrast based on histogram cutoff.
/// PIL: compute histogram, sort pixel values, find lo/hi at cutoff percentiles,
/// then linearly map [lo, hi] to [0, 255].
pub fn op_autocontrast(img: &DynamicImage, cutoff: f64) -> Result<DynamicImage, PilError> {
    let gray = img.to_luma8();
    let total = gray.len() as f64;
    let low_thresh = (total * cutoff / 100.0) as usize;
    let high_thresh = (total * (100.0 - cutoff) / 100.0) as usize;
    let mut sorted: Vec<u8> = gray.iter().copied().collect();
    sorted.sort_unstable();
    let lo = *sorted.get(low_thresh).unwrap_or(&0);
    let hi = *sorted
        .get(high_thresh.min(sorted.len() - 1))
        .unwrap_or(&255);
    if hi <= lo {
        return Ok(img.clone());
    }
    let mut rgb = img.to_rgb8();
    let scale = 255.0 / (hi - lo) as f64;
    let lo_f = lo as f64;
    for p in rgb.pixels_mut() {
        for c in 0..3 {
            p[c] = ((p[c] as f64 - lo_f) * scale).clamp(0.0, 255.0) as u8;
        }
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(rgb)))
}

/// Equalize: histogram equalization matching PIL's algorithm.
/// Build LUT from non-zero histogram bins, using PIL's step formula.
pub fn op_equalize(img: &DynamicImage) -> Result<DynamicImage, PilError> {
    // PIL 12 equalize: build LUT from non-zero histogram bins
    // step = (sum(non_zero_bins) - last_bin_count) / 255
    // lut[i] = floor(accumulator / step) where accumulator tracks step/2 + cumulative hist
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let mut out = pillow_rs_image::RgbImage::new(w, h);
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
        1 => DynamicImage::ImageLuma8(
            pillow_rs_image::GrayImage::from_raw(w, h, out).expect("invert L buffer"),
        ),
        2 => DynamicImage::ImageLumaA8(
            pillow_rs_image::GrayAlphaImage::from_raw(w, h, out).expect("invert LA buffer"),
        ),
        3 => DynamicImage::ImageRgb8(
            pillow_rs_image::RgbImage::from_raw(w, h, out).expect("invert RGB buffer"),
        ),
        _ => DynamicImage::ImageRgba8(
            pillow_rs_image::RgbaImage::from_raw(w, h, out).expect("invert RGBA buffer"),
        ),
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
    Ok(DynamicImage::ImageLuma8(pil_grayscale(img)))
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
    let mut out = pillow_rs_image::RgbImage::new(w, h);
    let &(br, bg, bb) = black;
    let &(wr, wg, wb) = white;
    for y in 0..h {
        for x in 0..w {
            let g = gray.get_pixel(x, y)[0] as f64 / 255.0;
            let r = (br as f64 + g * (wr as f64 - br as f64)) as u8;
            let gv = (bg as f64 + g * (wg as f64 - bg as f64)) as u8;
            let b = (bb as f64 + g * (wb as f64 - bb as f64)) as u8;
            out.put_pixel(x, y, pillow_rs_image::Rgb([r, gv, b]));
        }
    }
    // Colorize always outputs RGB (PIL behavior)
    Ok(DynamicImage::ImageRgb8(out))
}

/// Contain: resize to fit within (w, h) preserving aspect ratio.
pub fn op_contain(
    img: &DynamicImage,
    w: u32,
    h: u32,
    filter: ResampleFilter,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (iw, ih) = (img.width(), img.height());
    let ratio = (w as f64 / iw as f64).min(h as f64 / ih as f64);
    let nw = (iw as f64 * ratio) as u32;
    let nh = (ih as f64 * ratio) as u32;
    let result = pil_resize(img, nw.max(1), nh.max(1), filter, explicit_mode);
    Ok(preserve_mode(img, result))
}

/// Cover: resize to cover (w, h) preserving aspect ratio, crop excess.
pub fn op_cover(
    img: &DynamicImage,
    w: u32,
    h: u32,
    filter: ResampleFilter,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (iw, ih) = (img.width(), img.height());
    let ratio = (w as f64 / iw as f64).max(h as f64 / ih as f64);
    let nw = (iw as f64 * ratio) as u32;
    let nh = (ih as f64 * ratio) as u32;
    let resized = pil_resize(img, nw.max(1), nh.max(1), filter, explicit_mode);
    let x = (nw.saturating_sub(w)) / 2;
    let y = (nh.saturating_sub(h)) / 2;
    Ok(preserve_mode(img, resized.crop_imm(x, y, w, h)))
}

/// Fit: resize to fit within (w, h) with bleed and centering, then crop.
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
    // PIL's fit algorithm: apply bleed, compute ratio, resize, crop with centering
    let eff_w = w as f64 / (1.0 + 2.0 * bleed);
    let eff_h = h as f64 / (1.0 + 2.0 * bleed);
    let ratio = (eff_w / iw as f64).min(eff_h / ih as f64);
    let nw = (iw as f64 * ratio) as u32;
    let nh = (ih as f64 * ratio) as u32;
    let resized = pil_resize(img, nw.max(1), nh.max(1), filter, explicit_mode);
    let crop_x = ((nw as f64 - w as f64) * centering.0) as u32;
    let crop_y = ((nh as f64 - h as f64) * centering.1) as u32;
    Ok(preserve_mode(
        img,
        resized.crop_imm(
            crop_x.min(nw.saturating_sub(1)),
            crop_y.min(nh.saturating_sub(1)),
            w.min(nw),
            h.min(nh),
        ),
    ))
}

/// Pad: resize to fit within (w, h), then pad with fill color.
pub fn op_pad(
    img: &DynamicImage,
    w: u32,
    h: u32,
    filter: ResampleFilter,
    color: Option<(u8, u8, u8, u8)>,
    centering: (f64, f64),
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let fill = color.unwrap_or((0, 0, 0, 255));
    let (iw, ih) = (img.width(), img.height());
    // Step 1: contain (resize to fit within target, preserving aspect ratio)
    let ratio = (w as f64 / iw as f64).min(h as f64 / ih as f64);
    let nw = (iw as f64 * ratio) as u32;
    let nh = (ih as f64 * ratio) as u32;
    let resized = pil_resize(img, nw.max(1), nh.max(1), filter, explicit_mode);
    // Step 2: pad to target size
    let mut padded = DynamicImage::new_rgba8(w, h);
    for py in 0..h {
        for px in 0..w {
            padded.put_pixel(px, py, pillow_rs_image::Rgba([fill.0, fill.1, fill.2, fill.3]));
        }
    }
    let ox = ((w as f64 - nw as f64) * centering.0) as i64;
    let oy = ((h as f64 - nh as f64) * centering.1) as i64;
    let src_rgba = resized.to_rgba8();
    let (sw, sh) = (src_rgba.width(), src_rgba.height());
    for py in 0..sh.min(padded.height()) {
        for px in 0..sw.min(padded.width()) {
            let dx = (ox + px as i64) as u32;
            let dy = (oy + py as i64) as u32;
            if dx < padded.width() && dy < padded.height() {
                padded.put_pixel(dx, dy, *src_rgba.get_pixel(px, py));
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
            expanded.put_pixel(px, py, pillow_rs_image::Rgba([fill.0, fill.1, fill.2, fill.3]));
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
            let mut gray = pillow_rs_image::GrayImage::new(w, h);
            for y in 0..h {
                let val = y as u8;
                for x in 0..w {
                    gray.put_pixel(x, y, pillow_rs_image::Luma([val]));
                }
            }
            Ok(DynamicImage::ImageLuma8(gray))
        }
        _ => {
            // Default to RGB
            let mut rgb = pillow_rs_image::RgbImage::new(w, h);
            for y in 0..h {
                let val = y as u8;
                for x in 0..w {
                    rgb.put_pixel(x, y, pillow_rs_image::Rgb([val, val, val]));
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
            let mut gray = pillow_rs_image::GrayImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let dx = x as f64 - cx;
                    let dy = y as f64 - cy;
                    let dist = (dx * dx + dy * dy).sqrt();
                    let val = ((dist / max_dist * 255.0 + 0.5).min(255.0)) as u8;
                    gray.put_pixel(x, y, pillow_rs_image::Luma([val]));
                }
            }
            Ok(DynamicImage::ImageLuma8(gray))
        }
        _ => {
            let mut rgb = pillow_rs_image::RgbImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let dx = x as f64 - cx;
                    let dy = y as f64 - cy;
                    let dist = (dx * dx + dy * dy).sqrt();
                    let val = ((dist / max_dist * 255.0 + 0.5).min(255.0)) as u8;
                    rgb.put_pixel(x, y, pillow_rs_image::Rgb([val, val, val]));
                }
            }
            Ok(DynamicImage::ImageRgb8(rgb))
        }
    }
}
