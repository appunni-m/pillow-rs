use image::{ColorType, DynamicImage};

/// PIL-compatible grayscale conversion using ITU-R BT.601 coefficients.
/// R: 0.299, G: 0.587, B: 0.114
/// This differs from the image crate's sRGB luminance weights (0.2126, 0.7152, 0.0722).
const BT601_R: f64 = 0.299;
const BT601_G: f64 = 0.587;
const BT601_B: f64 = 0.114;

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
pub fn rgb_to_luma_u8(r: u8, g: u8, b: u8) -> u8 {
    ((BT601_R * r as f64 + BT601_G * g as f64 + BT601_B * b as f64) + 0.5) as u8
}

/// Convert an RGB image to grayscale using PIL's BT.601 formula.
/// Produces identical output to PIL's Image.convert("L").
pub fn pil_grayscale(img: &DynamicImage) -> image::GrayImage {
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let mut gray = image::GrayImage::new(w, h);
    for (gp, rp) in gray.pixels_mut().zip(rgb.pixels()) {
        gp[0] = rgb_to_luma_u8(rp[0], rp[1], rp[2]);
    }
    gray
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
) -> Result<(u8, u8, u8, u8), crate::error::PilError> {
    if let Some(s) = hex_str {
        return parse_color_str(s);
    }
    let is_luma = mode == "L" || mode == "LA";
    if let Some(v) = single_value {
        if is_luma { return Ok((v, v, v, 255)); }
        else { return Ok((v, 0, 0, 255)); } // PIL: single int to RGB = (R,0,0)
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
