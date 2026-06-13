use image::{ColorType, DynamicImage};

/// PIL-compatible grayscale conversion using ITU-R BT.601 coefficients.
/// R: 0.299, G: 0.587, B: 0.114. PIL truncates (no rounding).
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
/// PIL-identical 16-bit fixed-point BT.601 luma.
/// PIL uses: Y = (19595*R + 38470*G + 7471*B + 32768) >> 16
/// This matches PIL pixel-for-pixel. Decimal approximation (299/587/114)
/// differs for ~3/10000 pixels due to rounding in different directions.
pub fn rgb_to_luma_u8(r: u8, g: u8, b: u8) -> u8 {
    (((19595u32 * r as u32 + 38470u32 * g as u32 + 7471u32 * b as u32 + 32768) >> 16) & 0xFF) as u8
}

/// PIL-identical BT.601 grayscale: Y = round(0.299*R + 0.587*G + 0.114*B)
/// Uses precomputed lookup tables — no per-pixel multiplication or division.
/// Tight single loop, no rayon overhead, no bounds checks.
pub fn pil_grayscale(img: &DynamicImage) -> image::GrayImage {
    pil_grayscale_inner(img, true)
}

/// Non-rounded BT.601 grayscale for mode "1" conversion.
/// PIL convert("1") uses truncation (no +32768), while convert("L") uses rounding.
pub fn pil_grayscale_truncate(img: &DynamicImage) -> image::GrayImage {
    pil_grayscale_inner(img, false)
}

fn pil_grayscale_inner(img: &DynamicImage, round: bool) -> image::GrayImage {
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let n = (w as usize) * (h as usize);
    let rgb_data = rgb.as_raw().as_slice();

    // PIL-identical 16-bit fixed-point BT.601:
    // Y = (19595*R + 38470*G + 7471*B + [32768]) >> 16
    let rounding = if round { 32768u32 } else { 0u32 };
    let mut gray = vec![0u8; n];
    let len = rgb_data.len().min(n * 3);
    let mut i = 0;
    while i + 2 < len {
        let r = rgb_data[i] as u32;
        let g = rgb_data[i + 1] as u32;
        let b = rgb_data[i + 2] as u32;
        let y = (19595u32 * r + 38470u32 * g + 7471u32 * b + rounding) >> 16;
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

/// Resolve a color value for a given mode (like PIL's ImageColor.getcolor).
/// Takes RGB tuple and mode string, returns mode-appropriate value.
pub fn getcolor(r: u8, g: u8, b: u8, mode: &str) -> Result<(u8, u8, u8, u8), crate::error::PilError> {
    match mode {
        "L" | "1" => {
            let luma = rgb_to_luma_u8(r, g, b);
            if mode == "1" {
                Ok((if luma >= 128 { 255 } else { 0 }, 0, 0, 255))
            } else {
                Ok((luma, luma, luma, 255))
            }
        }
        "RGB" => Ok((r, g, b, 255)),
        "RGBA" => Ok((r, g, b, 255)),
        "LA" => {
            let luma = rgb_to_luma_u8(r, g, b);
            Ok((luma, luma, luma, 255))
        }
        _ => Ok((r, g, b, 255)),
    }
}

/// Search a flat palette [r,g,b, r,g,b, ...] for a color, return index.
pub fn palette_getcolor(palette: &[u8], r: u8, g: u8, b: u8) -> Option<usize> {
    for i in (0..palette.len()).step_by(3) {
        if i + 2 < palette.len() && palette[i] == r && palette[i+1] == g && palette[i+2] == b {
            return Some(i / 3);
        }
    }
    None
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
