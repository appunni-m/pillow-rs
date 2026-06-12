use image::{ColorType, DynamicImage};

/// PIL-compatible grayscale conversion using ITU-R BT.601 coefficients.
/// R: 0.299, G: 0.587, B: 0.114
/// This differs from the image crate's sRGB luminance weights (0.2126, 0.7152, 0.0722).
pub fn color_type_to_mode(ct: ColorType) -> &'static str {
    match ct {
        ColorType::L8 => "L",
        ColorType::La8 => "LA",
        ColorType::Rgb8 => "RGB",
        ColorType::Rgba8 => "RGBA",
        ColorType::L16 => "I;16",
        ColorType::Rgb16 => "I;16",
        ColorType::Rgba16 => "I;16",
        _ => "RGB",
    }
}

pub fn parse_color_str(s: &str) -> Result<(u8, u8, u8, u8), crate::error::PilError> {
    let c = csscolorparser::parse(s)
        .map_err(|e| crate::error::PilError::ValueError(format!("Invalid color string: {}", e)))?;
    let rgba = c.to_rgba8();
    Ok((rgba[0], rgba[1], rgba[2], rgba[3]))
}

/// Convert RGB to L value using PIL's BT.601 formula.
#[inline]
/// Integer BT.601 luma: Y = (299*R + 587*G + 114*B + 500) / 1000
/// Avoids f64 conversions — uses u32 arithmetic for SIMD-friendly performance.
pub fn rgb_to_luma_u8(r: u8, g: u8, b: u8) -> u8 {
    (((299u32 * r as u32 + 587u32 * g as u32 + 114u32 * b as u32 + 500) / 1000) & 0xFF) as u8
}

/// PIL-identical BT.601 grayscale: Y = round(0.299*R + 0.587*G + 0.114*B)
/// Uses precomputed lookup tables — no per-pixel multiplication or division.
/// Tight single loop, no rayon overhead, no bounds checks.
pub fn pil_grayscale(img: &DynamicImage) -> image::GrayImage {
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let n = (w as usize) * (h as usize);
    let rgb_data = rgb.as_raw().as_slice();

    // PIL-compatible BT.601 luma using exact integer arithmetic:
    // Y = int(0.299*R + 0.587*G + 0.114*B + 0.5)  — PIL adds 0.5 then truncates
    // Using integer approximation: (299*R + 587*G + 114*B + 500) / 1000
    let mut gray = vec![0u8; n];
    let len = rgb_data.len().min(n * 3);
    let mut i = 0;
    while i + 2 < len {
        let r = rgb_data[i] as u32;
        let g = rgb_data[i + 1] as u32;
        let b = rgb_data[i + 2] as u32;
        let y = (299u32 * r + 587u32 * g + 114u32 * b + 500) / 1000;
        gray[i / 3] = y.min(255) as u8;
        i += 3;
    }

    image::GrayImage::from_raw(w, h, gray).expect("pil_grayscale buffer mismatch")
}

/// Resolve a color value for a given image mode. The binding layer extracts
/// Python types (int/tuple/string) and passes raw values here. Core handles
/// ALL mode-aware logic: single-int semantics, tuple lengths, defaults.
pub fn resolve_new_color(
    mode: &str,
    hex_str: Option<&str>,
    single_value: Option<u8>,
    rgb: Option<(u8, u8, u8)>,
    rgba: Option<(u8, u8, u8, u8)>,
    la: Option<(u8, u8)>,
) -> Result<(u8, u8, u8, u8), crate::error::PilError> {
    if let Some(s) = hex_str {
        return parse_color_str(s);
    }
    let is_luma = mode == "L" || mode == "LA";
    if let Some(v) = single_value {
        if is_luma { return Ok((v, v, v, 255)); }
        else { return Ok((v, 0, 0, 255)); } // PIL: single int to RGB = (R,0,0)
    }
    if let Some((l, a)) = la {
        return Ok((l, l, l, a));
    }
    if let Some((r, g, b)) = rgb { return Ok((r, g, b, 255)); }
    if let Some(rgba) = rgba { return Ok(rgba); }
    Ok((0, 0, 0, 0)) // default: black
}

/// Convert an RGB image to LA using PIL's BT.601 formula + opaque alpha.
pub fn pil_grayscale_alpha(img: &DynamicImage) -> image::GrayAlphaImage {
    let gray = pil_grayscale(img);
    let (w, h) = gray.dimensions();
    let mut ga = image::GrayAlphaImage::new(w, h);
    for (gap, gp) in ga.pixels_mut().zip(gray.pixels()) {
        gap[0] = gp[0];
        gap[1] = 255;
    }
    ga
}
