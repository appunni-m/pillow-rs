// ── Enhance operations extracted from image.rs execute_op() ──

use image::DynamicImage;
use crate::color::pil_grayscale;
use crate::error::PilError;
use crate::image::preserve_mode;

pub fn op_enhance_brightness(img: &DynamicImage, factor: f64) -> Result<DynamicImage, PilError> {
    let mut rgb = img.to_rgb8();
    let f = factor;
    for p in rgb.pixels_mut() {
        for c in 0..3 {
            p[c] = ((p[c] as f64 * f).clamp(0.0, 255.0)) as u8;
        }
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(rgb)))
}

pub fn op_enhance_contrast(img: &DynamicImage, factor: f64) -> Result<DynamicImage, PilError> {
    // PIL: convert to L, compute rounded mean, create uniform gray degenerate,
    // then blend: degenerate * (1-factor) + original * factor
    let gray = pil_grayscale(img);
    let pixels: Vec<u8> = gray.pixels().map(|p| p[0]).collect();
    let n = pixels.len() as u64;
    let mean = if n > 0 {
        let sum: u64 = pixels.iter().map(|&p| p as u64).sum();
        // int(mean + 0.5) matching PIL's ImageStat
        ((sum as f64 / n as f64) + 0.5) as u8
    } else {
        0
    };
    let m = mean as f64;
    let f = factor;
    let mut rgb = img.to_rgb8();
    for p in rgb.pixels_mut() {
        for c in 0..3 {
            p[c] = (m * (1.0 - f) + p[c] as f64 * f).clamp(0.0, 255.0) as u8;
        }
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(rgb)))
}

pub fn op_enhance_color_saturation(img: &DynamicImage, factor: f64) -> Result<DynamicImage, PilError> {
    // Use PIL's rounded grayscale conversion (to_luma8 truncates)
    let gray = pil_grayscale(img);
    let mut rgb = img.to_rgb8();
    let f = factor;
    for (px, gp) in rgb.pixels_mut().zip(gray.pixels()) {
        let g = gp[0] as f64;
        // blend formula: gray * (1-factor) + original * factor
        px[0] = ((g + f * (px[0] as f64 - g)).clamp(0.0, 255.0)) as u8;
        px[1] = ((g + f * (px[1] as f64 - g)).clamp(0.0, 255.0)) as u8;
        px[2] = ((g + f * (px[2] as f64 - g)).clamp(0.0, 255.0)) as u8;
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(rgb)))
}

pub fn op_enhance_sharpness(img: &DynamicImage, factor: f64) -> Result<DynamicImage, PilError> {
    // PIL: apply SMOOTH filter (3x3 kernel [1,1,1; 1,5,1; 1,1,1] / 13, offset 0),
    // then blend: smoothed * (1-factor) + original * factor
    let f = factor;
    let rgb = img.to_rgb8();
    let (w, h) = (rgb.width() as i32, rgb.height() as i32);
    // Pre-divided kernel values matching PIL's layout
    // kernel: [1,1,1, 1,5,1, 1,1,1], scale=13
    let inv_scale = 1.0f32 / 13.0f32;
    let k = inv_scale; // edges = 1/13
    let kc = 5.0f32 * inv_scale; // center = 5/13
    let rounding_bias = 0.5f32; // offset=0 => 0+0.5
    let mut blurred = rgb.clone();
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            // bottom row (y+1): kernel[0..2] = 1,1,1
            let bp = rgb.get_pixel((x - 1) as u32, (y + 1) as u32);
            let cp = rgb.get_pixel(x as u32, (y + 1) as u32);
            let ap = rgb.get_pixel((x + 1) as u32, (y + 1) as u32);
            let row_b_r = bp[0] as f32 * k + cp[0] as f32 * k + ap[0] as f32 * k;
            let row_b_g = bp[1] as f32 * k + cp[1] as f32 * k + ap[1] as f32 * k;
            let row_b_b = bp[2] as f32 * k + cp[2] as f32 * k + ap[2] as f32 * k;
            // center row (y): kernel[3..5] = 1,5,1
            let bp = rgb.get_pixel((x - 1) as u32, y as u32);
            let cp = rgb.get_pixel(x as u32, y as u32);
            let ap = rgb.get_pixel((x + 1) as u32, y as u32);
            let row_c_r = bp[0] as f32 * k + cp[0] as f32 * kc + ap[0] as f32 * k;
            let row_c_g = bp[1] as f32 * k + cp[1] as f32 * kc + ap[1] as f32 * k;
            let row_c_b = bp[2] as f32 * k + cp[2] as f32 * kc + ap[2] as f32 * k;
            // top row (y-1): kernel[6..8] = 1,1,1
            let bp = rgb.get_pixel((x - 1) as u32, (y - 1) as u32);
            let cp = rgb.get_pixel(x as u32, (y - 1) as u32);
            let ap = rgb.get_pixel((x + 1) as u32, (y - 1) as u32);
            let row_t_r = bp[0] as f32 * k + cp[0] as f32 * k + ap[0] as f32 * k;
            let row_t_g = bp[1] as f32 * k + cp[1] as f32 * k + ap[1] as f32 * k;
            let row_t_b = bp[2] as f32 * k + cp[2] as f32 * k + ap[2] as f32 * k;
            // Accumulate: start with rounding_bias, then add each row group
            let mut r = rounding_bias;
            let mut g = rounding_bias;
            let mut b = rounding_bias;
            r += row_b_r;
            g += row_b_g;
            b += row_b_b;
            r += row_c_r;
            g += row_c_g;
            b += row_c_b;
            r += row_t_r;
            g += row_t_g;
            b += row_t_b;
            blurred.put_pixel(
                x as u32,
                y as u32,
                image::Rgb([
                    r.clamp(0.0, 255.0) as u8,
                    g.clamp(0.0, 255.0) as u8,
                    b.clamp(0.0, 255.0) as u8,
                ]),
            );
        }
    }
    // blend: blurred * (1-f) + original * f   (matching PIL's Image.blend)
    for y in 0..h {
        for x in 0..w {
            let op = rgb.get_pixel(x as u32, y as u32);
            let bp = blurred.get_pixel(x as u32, y as u32);
            blurred.put_pixel(
                x as u32,
                y as u32,
                image::Rgb([
                    (bp[0] as f64 * (1.0 - f) + op[0] as f64 * f).clamp(0.0, 255.0) as u8,
                    (bp[1] as f64 * (1.0 - f) + op[1] as f64 * f).clamp(0.0, 255.0) as u8,
                    (bp[2] as f64 * (1.0 - f) + op[2] as f64 * f).clamp(0.0, 255.0) as u8,
                ]),
            );
        }
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(blurred)))
}
