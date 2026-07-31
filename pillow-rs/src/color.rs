//! Pillow-compatible color parsing and color-space conversion.
//!
//! This module accepts Rust primitives and Pillow-owned raster buffers, then
//! applies the same mode-aware color behavior that Pillow exposes through
//! `ImageColor`, `Image.new`, and `Image.convert`.
//!
//! # Input Conventions
//!
//! - Mode values are Pillow mode strings such as `"L"`, `"RGB"`, `"RGBA"`,
//!   `"CMYK"`, `"HSV"`, `"YCbCr"`, `"I"`, and `"F"`.
//! - Multi-byte scalar modes are represented in little-endian byte order inside
//!   RGBA-like storage when this crate needs to carry them through
//!   Pillow raster buffers.
//! - CMYK is stored in an RGBA buffer as `C`, `M`, `Y`, `K` channels.
//! - HSV and YCbCr are stored in RGB buffers using their channel names in order.
//!
//! # Output Conventions
//!
//! Conversion helpers return concrete Pillow image buffers or
//! [`crate::raster::DynamicImage`] values with tightly packed rows. Color
//! resolver helpers return `(r, g, b, a)` tuples even when the destination mode
//! will later use only one or two channels.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{ColorType, DynamicImage, RgbImage};

/// Maps a codec color type to the nearest Pillow mode string.
///
/// # Returns
///
/// A static mode identifier such as `"L"`, `"LA"`, `"RGB"`, `"RGBA"`, or
/// `"I;16"`. Unsupported color types currently fall back to `"RGB"` because
/// the core image APIs operate on RGB-compatible buffers for unknown modes.
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

/// Parses a CSS/Pillow-style color string into an RGBA tuple.
///
/// # Inputs
///
/// - `s`: a color string accepted by `csscolorparser`, such as `"#ff0000"`,
///   `"red"`, or `"rgba(255, 0, 0, 0.5)"`.
///
/// # Returns
///
/// A tuple in `(red, green, blue, alpha)` byte order.
///
/// # Errors
///
/// Returns [`crate::PilError::ValueError`] when the string cannot be parsed.
pub fn parse_color_str(s: &str) -> Result<(u8, u8, u8, u8), crate::error::PilError> {
    if s.len() > 100 {
        return Err(crate::error::PilError::ValueError(
            "color specifier is too long".to_owned(),
        ));
    }
    let lowered = s.to_ascii_lowercase();
    // Pillow's regexes and colormap lookup operate on the lowercased string
    // without trimming, so leading/trailing whitespace is always rejected.
    if lowered.starts_with(char::is_whitespace) || lowered.ends_with(char::is_whitespace) {
        return Err(crate::error::PilError::ValueError(format!(
            "unknown color specifier: '{lowered}'"
        )));
    }
    if let Some(rgba) = parse_legacy_rgba_function(s) {
        return Ok(rgba);
    }
    // Pillow 12.2.0 `ImageColor.getcolor` accepts only its anchored integer
    // `rgba(...)` form; float-alpha and wrong-arity rgba strings are rejected
    // before the css fallback (which is more permissive).
    if lowered.starts_with("rgba(") {
        return Err(crate::error::PilError::ValueError(format!(
            "unknown color specifier: '{lowered}'"
        )));
    }
    // `rgb(...)` must have exactly three integer or percent components.
    if lowered.starts_with("rgb(") && !legacy_rgb_components_valid(&lowered) {
        return Err(crate::error::PilError::ValueError(format!(
            "unknown color specifier: '{lowered}'"
        )));
    }
    // CSS extra names Pillow's colormap does not define.
    if lowered == "transparent" || lowered == "currentcolor" {
        return Err(crate::error::PilError::ValueError(format!(
            "unknown color specifier: '{lowered}'"
        )));
    }
    let c = csscolorparser::parse(s).map_err(|_| {
        crate::error::PilError::ValueError(format!("unknown color specifier: '{lowered}'"))
    })?;
    let rgba = c.to_rgba8();
    Ok((rgba[0], rgba[1], rgba[2], rgba[3]))
}

/// Parse Pillow's ``ImageColor.getrgb`` legacy forms without clamping.
///
/// Pillow 12.2.0 `ImageColor.getrgb` returns raw integer components for
/// ``rgb(r, g, b)`` and ``rgba(r, g, b, a)`` even when they exceed 255, and
/// rounds percent components with ``int(value * 255 / 100.0 + 0.5)``.  The
/// css fallback below clamps to 8-bit, so the legacy forms must be parsed
/// here before delegating.
pub fn parse_color_str_unclamped(s: &str) -> Result<(i32, i32, i32, i32), crate::error::PilError> {
    let lowered = s.to_ascii_lowercase();
    let bad =
        || crate::error::PilError::ValueError(format!("unknown color specifier: '{lowered}'"));
    // Pillow anchors `rgb(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*)` (integer form)
    // and a percent form `rgb(\s*(\d+)%\s*,\s*(\d+)%\s*,\s*(\d+)%\s*)`.
    if lowered.starts_with("rgb(") {
        let Some(rest) = lowered
            .strip_prefix("rgb(")
            .and_then(|r| r.strip_suffix(')'))
        else {
            return Err(bad());
        };
        let parts: Vec<&str> = rest.split(',').map(str::trim).collect();
        if parts.len() != 3 {
            return Err(bad());
        }
        let mut values = [0i32; 3];
        let mut percent = [false; 3];
        for (index, part) in parts.iter().enumerate() {
            let (digits, is_percent) = match (*part).strip_suffix('%') {
                Some(digits) => (digits, true),
                None => (*part, false),
            };
            if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return Err(bad());
            }
            percent[index] = is_percent;
            let raw: i32 = digits.parse().map_err(|_| bad())?;
            values[index] = if is_percent {
                (raw as f64 * 255.0 / 100.0 + 0.5) as i32
            } else {
                raw
            };
        }
        // Mixed integer/percent forms fall through to the css parser (which
        // clamps); Pillow matches either the all-integer or all-percent regex.
        if percent == [true, true, true] || percent == [false, false, false] {
            return Ok((values[0], values[1], values[2], 255));
        }
        return Err(bad());
    }
    // Pillow's rgba regex is anchored integer-only; float-alpha and other
    // arities are rejected before the css fallback.
    if lowered.starts_with("rgba(") {
        let Some(rest) = lowered
            .strip_prefix("rgba(")
            .and_then(|r| r.strip_suffix(')'))
        else {
            return Err(bad());
        };
        let parts: Vec<&str> = rest.split(',').map(str::trim).collect();
        if parts.len() != 4
            || parts
                .iter()
                .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()))
        {
            return Err(bad());
        }
        let values: Vec<i32> = parts
            .iter()
            .map(|p| p.parse().map_err(|_| bad()))
            .collect::<Result<_, _>>()?;
        return Ok((values[0], values[1], values[2], values[3]));
    }
    // Pillow has no hsla form.
    if lowered.starts_with("hsla(") {
        return Err(bad());
    }
    let clamped = parse_color_str(s)?;
    Ok((
        clamped.0 as i32,
        clamped.1 as i32,
        clamped.2 as i32,
        clamped.3 as i32,
    ))
}

/// Validates Pillow's anchored integer/percent `rgb(r, g, b)` forms.
fn legacy_rgb_components_valid(lowered: &str) -> bool {
    let Some(rest) = lowered
        .strip_prefix("rgb(")
        .and_then(|r| r.strip_suffix(')'))
    else {
        return false;
    };
    let parts: Vec<&str> = rest.split(',').map(str::trim).collect();
    if parts.len() != 3 {
        return false;
    }
    parts.iter().all(|part| {
        if let Some(percent) = part.strip_suffix('%') {
            !percent.is_empty() && percent.bytes().all(|b| b.is_ascii_digit())
        } else {
            !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())
        }
    })
}

/// Parse Pillow's legacy ``rgba(r, g, b, a)`` integer form.
///
/// Pillow's ``getrgb`` matches ``rgba\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)``
/// and uses the alpha integer directly (0..255).  The css parser treats the
/// fourth component as a 0..1/percentage CSS alpha and clamps values such as
/// 128, so the legacy integer form must be handled before delegating.
fn parse_legacy_rgba_function(s: &str) -> Option<(u8, u8, u8, u8)> {
    // Pillow's regex is anchored on the lowercased string without trimming,
    // so leading/trailing whitespace is rejected.
    let lower = s.to_ascii_lowercase();
    let rest = lower.strip_prefix("rgba(")?.strip_suffix(')')?;
    let parts: Vec<&str> = rest.split(',').collect();
    if parts.len() != 4 {
        return None;
    }
    let mut values = [0u8; 4];
    for (index, part) in parts.iter().enumerate() {
        let text = part.trim();
        if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        values[index] = text.parse().ok()?;
    }
    Some((values[0], values[1], values[2], values[3]))
}

/// Whether Pillow's ``ImageColor.getrgb`` returns a four-tuple for this
/// color string.  Pillow emits alpha only for 4/8-digit hex (``#rgba``,
/// ``#rrggbbaa``) and for ``rgba(...)`` syntax; every other accepted form
/// returns a three-tuple even when the parsed value happens to be opaque.
pub fn color_has_explicit_alpha(s: &str) -> bool {
    let lower = s.trim().to_ascii_lowercase();
    if lower.starts_with("rgba(") {
        return true;
    }
    if let Some(hex) = lower.strip_prefix('#') {
        let digits = hex.chars().filter(|c| c.is_ascii_hexdigit()).count();
        return digits == 4 || digits == 8;
    }
    false
}

#[inline]
/// Converts one RGB pixel to a Pillow-compatible `L` value.
///
/// PIL uses: Y = (19595*R + 38470*G + 7471*B + 32768) >> 16
/// This matches PIL pixel-for-pixel. Decimal approximation (299/587/114)
/// differs for ~3/10000 pixels due to rounding in different directions.
pub fn rgb_to_luma_u8(r: u8, g: u8, b: u8) -> u8 {
    (((19595u32 * r as u32 + 38470u32 * g as u32 + 7471u32 * b as u32 + 32768) >> 16) & 0xFF) as u8
}

/// Converts an image to Pillow-compatible `L` grayscale.
///
/// The conversion uses Pillow's rounded BT.601 fixed-point formula, not the
/// `image` crate's sRGB luminance weights. The returned image has the same
/// dimensions as `img` and one byte per pixel.
pub fn pil_grayscale(img: &DynamicImage) -> Result<crate::raster::GrayImage, PilError> {
    pil_grayscale_inner(img, true)
}

/// Converts an image to grayscale with Pillow's mode `"1"` truncation rule.
///
/// `Image.convert("1")` uses the BT.601 coefficients without the rounding bias
/// used by `"L"` conversion. The returned buffer is still an 8-bit grayscale
/// image; callers perform the final binary thresholding step.
pub fn pil_grayscale_truncate(img: &DynamicImage) -> Result<crate::raster::GrayImage, PilError> {
    pil_grayscale_inner(img, false)
}

fn pil_grayscale_inner(
    img: &DynamicImage,
    round: bool,
) -> Result<crate::raster::GrayImage, PilError> {
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let dims = CheckedDims::new(w, h, 1)?;
    let rgb_data = rgb.as_raw().as_slice();

    // PIL-identical 16-bit fixed-point BT.601:
    // Y = (19595*R + 38470*G + 7471*B + [32768]) >> 16
    let rounding = if round { 32768u32 } else { 0u32 };
    let mut gray = dims.alloc_buffer();
    let len = rgb_data.len().min(dims.total_pixels() * 3);
    let mut i = 0;
    while i + 2 < len {
        let r = rgb_data[i] as u32;
        let g = rgb_data[i + 1] as u32;
        let b = rgb_data[i + 2] as u32;
        let y = (19595u32 * r + 38470u32 * g + 7471u32 * b + rounding) >> 16;
        gray[i / 3] = y.min(255) as u8;
        i += 3;
    }

    crate::raster::GrayImage::from_raw(w, h, gray)
        .ok_or_else(|| PilError::InternalError("pil_grayscale buffer mismatch".to_string()))
}

/// Converts a CMYK image to Pillow-compatible grayscale.
///
/// The input is stored as RGBA where channels mean `C`, `M`, `Y`, and `K`.
/// Output dimensions match the input and each output pixel is an `L` byte.
pub fn cmyk_to_grayscale(img: &DynamicImage) -> Result<crate::raster::GrayImage, PilError> {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let dims = CheckedDims::new(w, h, 1)?;
    let mut gray = dims.alloc_buffer();
    for (i, p) in rgba.pixels().enumerate() {
        let c = p[0] as u32;
        let m = p[1] as u32;
        let y_ = p[2] as u32;
        let k = p[3] as u32;
        let nk = 255u32.saturating_sub(k);
        let r = (nk as i32 - muldiv255(c, nk) as i32).clamp(0, 255) as u8;
        let g = (nk as i32 - muldiv255(m, nk) as i32).clamp(0, 255) as u8;
        let b = (nk as i32 - muldiv255(y_, nk) as i32).clamp(0, 255) as u8;
        gray[i] = rgb_to_luma_u8(r, g, b);
    }
    crate::raster::GrayImage::from_raw(w, h, gray)
        .ok_or_else(|| PilError::InternalError("cmyk_to_grayscale buffer mismatch".to_string()))
}

/// Resolves `Image.new` color input into core RGBA storage.
///
/// Binding crates normalize host-language values into the optional Rust
/// arguments here. This function owns the mode-aware Pillow rules: string
/// colors, scalar `L`/`I`/`F` values, tuple expansion, default black, and
/// single-int semantics for multi-band images.
///
/// # Returns
///
/// A tuple in core `(r, g, b, a)` byte order. For modes with fewer channels,
/// later image construction code chooses which fields are stored.
///
/// # Errors
///
/// Returns [`crate::PilError::ValueError`] when `hex_str` is present but cannot
/// be parsed as a color.
pub fn resolve_new_color(
    mode: &str,
    hex_str: Option<&str>,
    single_value: Option<u8>,
    rgb: Option<(u8, u8, u8)>,
    rgba: Option<(u8, u8, u8, u8)>,
    la: Option<(u8, u8)>,
    int32_val: Option<i32>,
    float_val: Option<f64>,
) -> Result<(u8, u8, u8, u8), crate::error::PilError> {
    if let Some(s) = hex_str {
        return parse_color_str(s);
    }
    // For I mode with i32 value, pack as 4-byte LE
    if mode == "I" {
        if let Some(v) = int32_val {
            let bytes = v.to_le_bytes();
            return Ok((bytes[0], bytes[1], bytes[2], bytes[3]));
        }
    }
    // For F mode with f64 value, pack as 4-byte LE f32
    if mode == "F" {
        if let Some(v) = float_val {
            let f = v as f32;
            let bytes = f.to_le_bytes();
            return Ok((bytes[0], bytes[1], bytes[2], bytes[3]));
        }
    }
    if let Some(v) = single_value {
        // PIL: single int for multi-band images = first band = value, rest = 0
        return Ok((v, 0, 0, 0));
    }
    if let Some((l, a)) = la {
        return Ok((l, l, l, a));
    }
    if let Some((r, g, b)) = rgb {
        // PIL rejects color tuples for single-channel modes (L, 1, P)
        // The Python Image.new wrapper uses this path for P-mode by converting
        // "P" to "L" internally. Match PIL's P-mode behavior: index 0 with
        // the color stored in the palette (palette managed on Python side).
        if mode == "L" || mode == "1" || mode == "P" {
            return Ok((0, 0, 0, 0));
        }
        return Ok((r, g, b, 255));
    }
    if let Some(rgba) = rgba {
        return Ok(rgba);
    }
    Ok((0, 0, 0, 0)) // default: black
}

/// Resolves an RGB color for a Pillow `ImageColor.getcolor` destination mode.
///
/// # Inputs
///
/// - `r`, `g`, `b`: source color channels.
/// - `mode`: destination Pillow mode.
///
/// # Returns
///
/// A tuple in `(r, g, b, a)` order. For `"L"` and `"LA"`, the luma value is
/// duplicated into the RGB fields so callers can store the channel appropriate
/// for their target buffer. For `"1"`, the first field is thresholded to
/// either `0` or `255`.
/// A Pillow-compatible ``ImageColor.getcolor`` result.
///
/// Pillow returns a scalar graylevel for base-L modes, ``(graylevel, alpha)``
/// for ``LA``, ``(r, g, b[, a])`` for color modes, and a 0..255 HSV triple
/// for ``HSV``.  The shape is part of the public behavior, so the core keeps
/// it explicit instead of folding it into a fixed four-tuple.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorValue {
    Gray(i32),
    GrayAlpha(i32, i32),
    Rgb(i32, i32, i32),
    Rgba(i32, i32, i32, i32),
    Hsv(i32, i32, i32),
}

/// Convert one RGB pixel to Pillow's ``colorsys.rgb_to_hsv`` 0..255 triple.
///
/// Pillow calls ``colorsys.rgb_to_hsv(r/255, g/255, b/255)`` and truncates
/// each 0..1 float to an integer with ``int(h*255)``; the float arithmetic
/// (including the ``% 1.0`` hue wrap) is replicated exactly here.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "int() truncation of the 0..1 colorsys floats is the public contract"
)]
fn rgb_to_hsv_i32(r: i32, g: i32, b: i32) -> (i32, i32, i32) {
    let rf = r as f64 / 255.0;
    let gf = g as f64 / 255.0;
    let bf = b as f64 / 255.0;
    let maxc = rf.max(gf).max(bf);
    let minc = rf.min(gf).min(bf);
    let v = maxc;
    if minc == maxc {
        return (0, 0, (v * 255.0) as i32);
    }
    let s = (maxc - minc) / maxc;
    // CPython colorsys.rgb_to_hsv divides the channel deltas by the range
    // itself (`rangec = maxc - minc`), not by 6 * range.
    let range = maxc - minc;
    let rc = (maxc - rf) / range;
    let gc = (maxc - gf) / range;
    let bc = (maxc - bf) / range;
    let h = if rf == maxc {
        bc - gc
    } else if gf == maxc {
        2.0 + rc - bc
    } else {
        4.0 + gc - rc
    };
    let h = (h / 6.0).rem_euclid(1.0);
    ((h * 255.0) as i32, (s * 255.0) as i32, (v * 255.0) as i32)
}

pub fn getcolor(
    r: i32,
    g: i32,
    b: i32,
    a: i32,
    mode: &str,
) -> Result<ColorValue, crate::error::PilError> {
    // Pillow's getcolor computes from the raw (unclamped) parsed components:
    // `(r * 19595 + g * 38470 + b * 7471 + 0x8000) >> 16` and colorsys HSV on
    // r/255, so out-of-range values like rgb(300,0,0) produce 90 / (0,255,300).
    let luma = (r * 19595 + g * 38470 + b * 7471 + 0x8000) >> 16;
    if mode == "HSV" {
        let (h, s, v) = rgb_to_hsv_i32(r, g, b);
        return Ok(ColorValue::Hsv(h, s, v));
    }
    if matches!(mode, "L" | "LA" | "1" | "I" | "F" | "I;16" | "I;16B") {
        return if mode == "LA" {
            Ok(ColorValue::GrayAlpha(luma, a))
        } else {
            Ok(ColorValue::Gray(luma))
        };
    }
    if mode.ends_with('A') {
        return Ok(ColorValue::Rgba(r, g, b, a));
    }
    Ok(ColorValue::Rgb(r, g, b))
}

/// Searches a flat RGB palette for an exact color match.
///
/// # Inputs
///
/// `palette` is laid out as `[r, g, b, r, g, b, ...]`.
///
/// # Returns
///
/// The palette index when found, or `None` when the RGB triple is absent.
pub fn palette_getcolor(palette: &[u8], r: u8, g: u8, b: u8) -> Option<usize> {
    for i in (0..palette.len()).step_by(3) {
        if i + 2 < palette.len() && palette[i] == r && palette[i + 1] == g && palette[i + 2] == b {
            return Some(i / 3);
        }
    }
    None
}

/// Finds or appends a color in a Pillow-compatible palette.
///
/// # Inputs
///
/// - `palette`: mutable flat palette storage.
/// - `r`, `g`, `b`, `a`: color to locate.
/// - `mode`: `"RGB"` for 3-byte entries or `"RGBA"` for 4-byte entries.
///
/// # Returns
///
/// The palette index for an existing or newly appended entry.
///
/// # Errors
///
/// Returns an error string when the palette already has 256 entries and the
/// color is not present.
pub fn palette_getcolor_append(
    palette: &mut Vec<u8>,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
    mode: &str,
) -> Result<usize, String> {
    let step = if mode == "RGBA" { 4 } else { 3 };

    // Search for existing color
    for i in (0..palette.len()).step_by(step) {
        if palette[i] == r
            && palette[i + 1] == g
            && palette[i + 2] == b
            && (step == 3 || palette[i + 3] == a)
        {
            return Ok(i / step);
        }
    }

    // Not found — append
    let idx = palette.len() / step;
    if idx >= 256 {
        return Err("cannot allocate more than 256 colors".into());
    }
    palette.push(r);
    palette.push(g);
    palette.push(b);
    if step == 4 {
        palette.push(a);
    }
    Ok(idx)
}

/// Formats a palette as Pillow-compatible text.
///
/// The output contains a header and 256 indexed rows. Missing palette entries
/// are emitted as zeroes, matching Pillow's fixed-size text palette format.
pub fn palette_to_text(palette: &[u8], mode: &str) -> String {
    let step = match mode {
        "L" | "P" => 1,
        "RGBA" => 4,
        _ => 3,
    };
    let _palette_len = palette.len();
    let mut out = String::with_capacity(4096);
    out.push_str("# Palette\n");
    out.push_str("# Mode: ");
    out.push_str(mode);
    out.push('\n');
    for i in 0..256 {
        use std::fmt::Write;
        let _ = write!(out, "{i}");
        for j in 0..step {
            let val = palette.get(i * step + j).copied().unwrap_or(0);
            let _ = write!(out, " {val}");
        }
        out.push('\n');
    }
    out
}

/// Converts an image to Pillow-compatible `LA`.
///
/// The luma channel uses [`pil_grayscale`]. The alpha channel is set to fully
/// opaque (`255`) for every pixel.
pub fn pil_grayscale_alpha(img: &DynamicImage) -> Result<crate::raster::GrayAlphaImage, PilError> {
    let gray = pil_grayscale(img)?;
    let (w, h) = gray.dimensions();
    let mut ga = crate::raster::GrayAlphaImage::new(w, h);
    for (gap, gp) in ga.pixels_mut().zip(gray.pixels()) {
        gap[0] = gp[0];
        gap[1] = 255;
    }
    Ok(ga)
}

// ── Non-standard mode conversions ──

/// Applies Pillow's `MULDIV255` integer scaling helper.
///
/// This is equivalent to `(a * b + 128) / 255` with Pillow-compatible rounding.
#[inline]
pub fn muldiv255(a: u32, b: u32) -> u32 {
    let t = a * b + 128;
    ((t >> 8) + t) >> 8
}

/// Converts CMYK storage to RGB using Pillow-compatible integer arithmetic.
///
/// The input is an RGBA buffer interpreted as `C`, `M`, `Y`, `K`. The returned
/// image is `RGB8` with the same dimensions.
pub fn cmyk_to_rgb(img: &DynamicImage) -> DynamicImage {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut out = RgbImage::new(w, h);
    for (op, ip) in out.pixels_mut().zip(rgba.pixels()) {
        let c = ip[0] as u32;
        let m = ip[1] as u32;
        let y = ip[2] as u32;
        let k = ip[3] as u32;
        let nk = 255u32 - k;
        let r = (nk as i32 - muldiv255(c, nk) as i32).clamp(0, 255) as u8;
        let g = (nk as i32 - muldiv255(m, nk) as i32).clamp(0, 255) as u8;
        let b = (nk as i32 - muldiv255(y, nk) as i32).clamp(0, 255) as u8;
        *op = crate::raster::Rgb([r, g, b]);
    }
    DynamicImage::ImageRgb8(out)
}

/// Converts HSV storage to RGB using Pillow's integer-domain algorithm.
///
/// HSV is stored as an RGB buffer interpreted as `H`, `S`, `V`, all in the
/// `0..=255` byte range. The returned image is `RGB8` with the same dimensions.
///
/// PIL's formula (from C source):
/// ```text
///   fs = s / 255.0    (normalized saturation)
///   h = h * 6.0 / 255.0  (0-6 sector mapping)
///   j = floor(h); f = h - j
///   p = v * (1.0 - fs)     ← v is 0-255, so p is 0-255
///   q = v * (1.0 - fs * f)
///   t = v * (1.0 - fs * (1.0 - f))
///   Then round all to nearest integer, CLIP8
/// ```
pub fn hsv_to_rgb(img: &DynamicImage) -> DynamicImage {
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let mut out = RgbImage::new(w, h);
    for (op, ip) in out.pixels_mut().zip(rgb.pixels()) {
        let h_in = ip[0] as f32;
        let s_in = ip[1] as f32;
        let v = ip[2] as f32;

        if s_in == 0.0 {
            let g = v.round().clamp(0.0, 255.0) as u8;
            *op = crate::raster::Rgb([g, g, g]);
        } else {
            let fs = s_in / 255.0; // normalized saturation
            let h = h_in * 6.0 / 255.0; // 0-6 sector mapping
            let j = h.floor() as i32;
            let f = h - h.floor();
            // p, q, t are in 0-255 range (v is 0-255)
            let p = v * (1.0 - fs);
            let q = v * (1.0 - fs * f);
            let t = v * (1.0 - fs * (1.0 - f));
            // PIL rounds all values, then CLIP8
            let up = p.round().clamp(0.0, 255.0) as u8;
            let uq = q.round().clamp(0.0, 255.0) as u8;
            let ut = t.round().clamp(0.0, 255.0) as u8;
            let uv = v.round().clamp(0.0, 255.0) as u8;
            let (r, g, b) = match j % 6 {
                0 => (uv, ut, up),
                1 => (uq, uv, up),
                2 => (up, uv, ut),
                3 => (up, uq, uv),
                4 => (ut, up, uv),
                _ => (uv, up, uq),
            };
            *op = crate::raster::Rgb([r, g, b]);
        }
    }
    DynamicImage::ImageRgb8(out)
}

/// Converts Pillow `"I"` storage to `L`.
///
/// The input is RGBA storage interpreted as little-endian `i32` pixels.
/// Output uses Pillow's scaling formula: `L = (I + 32768) / 256`, clamped to a
/// byte.
#[allow(dead_code)]
pub(crate) fn i32_to_l(img: &DynamicImage) -> crate::raster::GrayImage {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut gray = crate::raster::GrayImage::new(w, h);
    for (gp, rp) in gray.pixels_mut().zip(rgba.pixels()) {
        let i = i32::from_le_bytes([rp[0], rp[1], rp[2], rp[3]]);
        let l = ((i as i64 + 32768) / 256).clamp(0, 255) as u8;
        gp[0] = l;
    }
    gray
}

/// Converts Pillow `"F"` storage to `L`.
///
/// The input is RGBA storage interpreted as little-endian `f32` pixels. Output
/// clamps each value to `0..=255` and truncates to a byte.
#[allow(dead_code)]
pub(crate) fn f32_to_l(img: &DynamicImage) -> crate::raster::GrayImage {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut gray = crate::raster::GrayImage::new(w, h);
    for (gp, rp) in gray.pixels_mut().zip(rgba.pixels()) {
        let f = f32::from_le_bytes([rp[0], rp[1], rp[2], rp[3]]);
        let l = (f.clamp(0.0, 255.0)) as u8;
        gp[0] = l;
    }
    gray
}

/// Converts RGB to HSV using Pillow's `rgb2hsv` precision behavior.
///
/// Input and output are `RGB8` buffers. Output channels are `H`, `S`, `V`, each
/// in `0..=255`.
///
/// PIL uses float (f32) for intermediate values (rc, gc, bc, cr, h), only
/// promoting to double when combining with `2.0`/`4.0`/`6.0`/`255.0` literals.
/// The result is stored back to f32, and the final `(int)(h * 255.0)` truncates.
/// This function matches PIL pixel-for-pixel by mimicking the exact precision
/// flow: f32 divisions → f64 promotions for constant arithmetic → f32 storage.
pub fn rgb_to_hsv(img: &DynamicImage) -> DynamicImage {
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let mut out = crate::raster::RgbImage::new(w, h);
    for (op, ip) in out.pixels_mut().zip(rgb.pixels()) {
        let r = ip[0];
        let g = ip[1];
        let b = ip[2];
        let maxc = r.max(g.max(b));
        let minc = r.min(g.min(b));
        let v = maxc;
        let (uh, us) = if minc == maxc {
            (0u8, 0u8)
        } else {
            // All intermediate computations in f32, matching PIL's `float` type
            let cr = (maxc - minc) as f32;
            let rc = (maxc as f32 - r as f32) / cr;
            let gc = (maxc as f32 - g as f32) / cr;
            let bc = (maxc as f32 - b as f32) / cr;

            // Compute h_val in f64 (PIL promotes to double due to 2.0/4.0 literals),
            // then immediately cast back to f32 for storage in `h`.
            let h_float: f32 = if r == maxc {
                (bc as f64 - gc as f64) as f32
            } else if g == maxc {
                (2.0_f64 + rc as f64 - bc as f64) as f32
            } else {
                (4.0_f64 + gc as f64 - rc as f64) as f32
            };

            // PIL: h = fmod((h / 6.0 + 1.0), 1.0)
            // h is float (f32), 6.0/1.0 are double literals → promotion to double.
            // The fmod result is stored back to float (f32).
            let h_double = h_float as f64;
            let h_fmod = ((h_double / 6.0) + 1.0) % 1.0;
            let h_stored = h_fmod as f32; // PIL stores h back to float here

            // PIL: uh = (int)(h * 255.0) — float promoted to double for multiplication
            // with 255.0 (double literal), then truncated
            let s_float = cr / maxc as f32; // s = cr / maxc in f32
            let us = ((s_float as f64) * 255.0) as u8;
            let uh = ((h_stored as f64) * 255.0) as u8;

            (uh, us)
        };
        *op = crate::raster::Rgb([uh, us, v]);
    }
    DynamicImage::ImageRgb8(out)
}

/// Converts RGB to YCbCr using Pillow's lookup-table BT.601 conversion.
///
/// The returned image is `RGB8` storage interpreted as `Y`, `Cb`, `Cr`.
/// Uses precomputed tables matching PIL's ConvertYCbCr.c with SCALE=6.
/// Table formula: `table[i] = (int)(i * coeff * 64 + 0.5)`.
pub fn rgb_to_ycbcr(img: &DynamicImage) -> DynamicImage {
    use std::sync::OnceLock;

    fn make_table(coeff: f64) -> [i32; 256] {
        let mut t = [0i32; 256];
        for i in 0..256 {
            t[i] = (i as f64 * coeff * 64.0 + 0.5) as i32;
        }
        t
    }

    static Y_R: OnceLock<[i32; 256]> = OnceLock::new();
    static Y_G: OnceLock<[i32; 256]> = OnceLock::new();
    static Y_B: OnceLock<[i32; 256]> = OnceLock::new();
    static CB_R: OnceLock<[i32; 256]> = OnceLock::new();
    static CB_G: OnceLock<[i32; 256]> = OnceLock::new();
    static CB_B: OnceLock<[i32; 256]> = OnceLock::new();
    static CR_G: OnceLock<[i32; 256]> = OnceLock::new();
    static CR_B: OnceLock<[i32; 256]> = OnceLock::new();

    let y_r = Y_R.get_or_init(|| make_table(0.299));
    let y_g = Y_G.get_or_init(|| make_table(0.587));
    let y_b = Y_B.get_or_init(|| make_table(0.114));
    let cb_r = CB_R.get_or_init(|| make_table(-0.16874));
    let cb_g = CB_G.get_or_init(|| make_table(-0.33126));
    let cb_b = CB_B.get_or_init(|| make_table(0.5));
    let cr_g = CR_G.get_or_init(|| make_table(-0.41869));
    let cr_b = CR_B.get_or_init(|| make_table(-0.08131));

    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let mut out = crate::raster::RgbImage::new(w, h);
    for (op, ip) in out.pixels_mut().zip(rgb.pixels()) {
        let r = ip[0] as usize;
        let g = ip[1] as usize;
        let b = ip[2] as usize;

        let y = ((y_r[r] + y_g[g] + y_b[b]) >> 6) as u8;
        let cb = (((cb_r[r] + cb_g[g] + cb_b[b]) >> 6) + 128) as u8;
        let cr = (((cb_b[r] + cr_g[g] + cr_b[b]) >> 6) + 128) as u8; // Cr_R = Cb_B

        *op = crate::raster::Rgb([y, cb, cr]);
    }
    DynamicImage::ImageRgb8(out)
}

/// Converts YCbCr storage to RGB using Pillow's lookup-table BT.601 conversion.
///
/// The input is `RGB8` storage interpreted as `Y`, `Cb`, `Cr`; the returned
/// image is `RGB8`.
/// PIL uses precomputed tables with 6-bit fixed-point (SCALE=6, multiply by 64):
/// ```text
///   R = Y + (R_Cr[Cr] >> 6)
///   G = Y + ((G_Cb[Cb] + G_Cr[Cr]) >> 6)
///   B = Y + (B_Cb[Cb] >> 6)
/// ```
/// Tables extracted directly from PIL's _imaging C extension.
pub fn ycbcr_to_rgb(img: &DynamicImage) -> DynamicImage {
    const R_CR: [i32; 256] = [
        -11484, -11394, -11305, -11215, -11125, -11036, -10946, -10856, -10766, -10677, -10587,
        -10497, -10407, -10318, -10228, -10138, -10049, -9959, -9869, -9779, -9690, -9600, -9510,
        -9420, -9331, -9241, -9151, -9062, -8972, -8882, -8792, -8703, -8613, -8523, -8433, -8344,
        -8254, -8164, -8075, -7985, -7895, -7805, -7716, -7626, -7536, -7446, -7357, -7267, -7177,
        -7088, -6998, -6908, -6818, -6729, -6639, -6549, -6459, -6370, -6280, -6190, -6101, -6011,
        -5921, -5831, -5742, -5652, -5562, -5472, -5383, -5293, -5203, -5113, -5024, -4934, -4844,
        -4755, -4665, -4575, -4485, -4396, -4306, -4216, -4126, -4037, -3947, -3857, -3768, -3678,
        -3588, -3498, -3409, -3319, -3229, -3139, -3050, -2960, -2870, -2781, -2691, -2601, -2511,
        -2422, -2332, -2242, -2152, -2063, -1973, -1883, -1794, -1704, -1614, -1524, -1435, -1345,
        -1255, -1165, -1076, -986, -896, -807, -717, -627, -537, -448, -358, -268, -178, -89, 0,
        90, 179, 269, 359, 449, 538, 628, 718, 808, 897, 987, 1077, 1166, 1256, 1346, 1436, 1525,
        1615, 1705, 1795, 1884, 1974, 2064, 2153, 2243, 2333, 2423, 2512, 2602, 2692, 2782, 2871,
        2961, 3051, 3140, 3230, 3320, 3410, 3499, 3589, 3679, 3769, 3858, 3948, 4038, 4127, 4217,
        4307, 4397, 4486, 4576, 4666, 4756, 4845, 4935, 5025, 5114, 5204, 5294, 5384, 5473, 5563,
        5653, 5743, 5832, 5922, 6012, 6102, 6191, 6281, 6371, 6460, 6550, 6640, 6730, 6819, 6909,
        6999, 7089, 7178, 7268, 7358, 7447, 7537, 7627, 7717, 7806, 7896, 7986, 8076, 8165, 8255,
        8345, 8434, 8524, 8614, 8704, 8793, 8883, 8973, 9063, 9152, 9242, 9332, 9421, 9511, 9601,
        9691, 9780, 9870, 9960, 10050, 10139, 10229, 10319, 10408, 10498, 10588, 10678, 10767,
        10857, 10947, 11037, 11126, 11216, 11306, 11395,
    ];
    const G_CB: [i32; 256] = [
        2819, 2797, 2775, 2753, 2731, 2709, 2687, 2665, 2643, 2621, 2599, 2577, 2555, 2533, 2511,
        2489, 2467, 2445, 2423, 2401, 2379, 2357, 2335, 2313, 2291, 2269, 2247, 2225, 2202, 2180,
        2158, 2136, 2114, 2092, 2070, 2048, 2026, 2004, 1982, 1960, 1938, 1916, 1894, 1872, 1850,
        1828, 1806, 1784, 1762, 1740, 1718, 1696, 1674, 1652, 1630, 1608, 1586, 1564, 1542, 1520,
        1498, 1476, 1454, 1432, 1410, 1388, 1366, 1344, 1321, 1299, 1277, 1255, 1233, 1211, 1189,
        1167, 1145, 1123, 1101, 1079, 1057, 1035, 1013, 991, 969, 947, 925, 903, 881, 859, 837,
        815, 793, 771, 749, 727, 705, 683, 661, 639, 617, 595, 573, 551, 529, 507, 485, 463, 440,
        418, 396, 374, 352, 330, 308, 286, 264, 242, 220, 198, 176, 154, 132, 110, 88, 66, 44, 22,
        0, -21, -43, -65, -87, -109, -131, -153, -175, -197, -219, -241, -263, -285, -307, -329,
        -351, -373, -395, -417, -439, -462, -484, -506, -528, -550, -572, -594, -616, -638, -660,
        -682, -704, -726, -748, -770, -792, -814, -836, -858, -880, -902, -924, -946, -968, -990,
        -1012, -1034, -1056, -1078, -1100, -1122, -1144, -1166, -1188, -1210, -1232, -1254, -1276,
        -1298, -1320, -1343, -1365, -1387, -1409, -1431, -1453, -1475, -1497, -1519, -1541, -1563,
        -1585, -1607, -1629, -1651, -1673, -1695, -1717, -1739, -1761, -1783, -1805, -1827, -1849,
        -1871, -1893, -1915, -1937, -1959, -1981, -2003, -2025, -2047, -2069, -2091, -2113, -2135,
        -2157, -2179, -2201, -2224, -2246, -2268, -2290, -2312, -2334, -2356, -2378, -2400, -2422,
        -2444, -2466, -2488, -2510, -2532, -2554, -2576, -2598, -2620, -2642, -2664, -2686, -2708,
        -2730, -2752, -2774, -2796,
    ];
    const G_CR: [i32; 256] = [
        5850, 5805, 5759, 5713, 5667, 5622, 5576, 5530, 5485, 5439, 5393, 5347, 5302, 5256, 5210,
        5165, 5119, 5073, 5028, 4982, 4936, 4890, 4845, 4799, 4753, 4708, 4662, 4616, 4570, 4525,
        4479, 4433, 4388, 4342, 4296, 4251, 4205, 4159, 4113, 4068, 4022, 3976, 3931, 3885, 3839,
        3794, 3748, 3702, 3656, 3611, 3565, 3519, 3474, 3428, 3382, 3336, 3291, 3245, 3199, 3154,
        3108, 3062, 3017, 2971, 2925, 2879, 2834, 2788, 2742, 2697, 2651, 2605, 2559, 2514, 2468,
        2422, 2377, 2331, 2285, 2240, 2194, 2148, 2102, 2057, 2011, 1965, 1920, 1874, 1828, 1782,
        1737, 1691, 1645, 1600, 1554, 1508, 1463, 1417, 1371, 1325, 1280, 1234, 1188, 1143, 1097,
        1051, 1006, 960, 914, 868, 823, 777, 731, 686, 640, 594, 548, 503, 457, 411, 366, 320, 274,
        229, 183, 137, 91, 46, 0, -45, -90, -136, -182, -228, -273, -319, -365, -410, -456, -502,
        -547, -593, -639, -685, -730, -776, -822, -867, -913, -959, -1005, -1050, -1096, -1142,
        -1187, -1233, -1279, -1324, -1370, -1416, -1462, -1507, -1553, -1599, -1644, -1690, -1736,
        -1781, -1827, -1873, -1919, -1964, -2010, -2056, -2101, -2147, -2193, -2239, -2284, -2330,
        -2376, -2421, -2467, -2513, -2558, -2604, -2650, -2696, -2741, -2787, -2833, -2878, -2924,
        -2970, -3016, -3061, -3107, -3153, -3198, -3244, -3290, -3335, -3381, -3427, -3473, -3518,
        -3564, -3610, -3655, -3701, -3747, -3793, -3838, -3884, -3930, -3975, -4021, -4067, -4112,
        -4158, -4204, -4250, -4295, -4341, -4387, -4432, -4478, -4524, -4569, -4615, -4661, -4707,
        -4752, -4798, -4844, -4889, -4935, -4981, -5027, -5072, -5118, -5164, -5209, -5255, -5301,
        -5346, -5392, -5438, -5484, -5529, -5575, -5621, -5666, -5712, -5758, -5804,
    ];
    const B_CB: [i32; 256] = [
        -14515, -14402, -14288, -14175, -14062, -13948, -13835, -13721, -13608, -13495, -13381,
        -13268, -13154, -13041, -12928, -12814, -12701, -12587, -12474, -12360, -12247, -12134,
        -12020, -11907, -11793, -11680, -11567, -11453, -11340, -11226, -11113, -11000, -10886,
        -10773, -10659, -10546, -10433, -10319, -10206, -10092, -9979, -9865, -9752, -9639, -9525,
        -9412, -9298, -9185, -9072, -8958, -8845, -8731, -8618, -8505, -8391, -8278, -8164, -8051,
        -7938, -7824, -7711, -7597, -7484, -7371, -7257, -7144, -7030, -6917, -6803, -6690, -6577,
        -6463, -6350, -6236, -6123, -6010, -5896, -5783, -5669, -5556, -5443, -5329, -5216, -5102,
        -4989, -4876, -4762, -4649, -4535, -4422, -4309, -4195, -4082, -3968, -3855, -3741, -3628,
        -3515, -3401, -3288, -3174, -3061, -2948, -2834, -2721, -2607, -2494, -2381, -2267, -2154,
        -2040, -1927, -1814, -1700, -1587, -1473, -1360, -1246, -1133, -1020, -906, -793, -679,
        -566, -453, -339, -226, -112, 0, 113, 227, 340, 454, 567, 680, 794, 907, 1021, 1134, 1247,
        1361, 1474, 1588, 1701, 1815, 1928, 2041, 2155, 2268, 2382, 2495, 2608, 2722, 2835, 2949,
        3062, 3175, 3289, 3402, 3516, 3629, 3742, 3856, 3969, 4083, 4196, 4310, 4423, 4536, 4650,
        4763, 4877, 4990, 5103, 5217, 5330, 5444, 5557, 5670, 5784, 5897, 6011, 6124, 6237, 6351,
        6464, 6578, 6691, 6804, 6918, 7031, 7145, 7258, 7372, 7485, 7598, 7712, 7825, 7939, 8052,
        8165, 8279, 8392, 8506, 8619, 8732, 8846, 8959, 9073, 9186, 9299, 9413, 9526, 9640, 9753,
        9866, 9980, 10093, 10207, 10320, 10434, 10547, 10660, 10774, 10887, 11001, 11114, 11227,
        11341, 11454, 11568, 11681, 11794, 11908, 12021, 12135, 12248, 12361, 12475, 12588, 12702,
        12815, 12929, 13042, 13155, 13269, 13382, 13496, 13609, 13722, 13836, 13949, 14063, 14176,
        14289, 14403,
    ];

    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let mut out = RgbImage::new(w, h);
    for (op, ip) in out.pixels_mut().zip(rgb.pixels()) {
        let y_ = ip[0] as i32;
        let cb = ip[1] as i32;
        let cr = ip[2] as i32;

        // PIL: right-shift is arithmetic (sign-extending) for negative values
        let r = y_ + (R_CR[cr as usize] >> 6);
        let g = y_ + ((G_CB[cb as usize] + G_CR[cr as usize]) >> 6);
        let b = y_ + (B_CB[cb as usize] >> 6);

        *op = crate::raster::Rgb([
            r.clamp(0, 255) as u8,
            g.clamp(0, 255) as u8,
            b.clamp(0, 255) as u8,
        ]);
    }
    DynamicImage::ImageRgb8(out)
}

/// Converts Pillow `"I"` storage to RGB.
///
/// The input is RGBA storage interpreted as little-endian `i32` pixels. Each
/// value is clamped to `0..=255` and broadcast to `R=G=B`.
pub fn i_to_rgb(img: &DynamicImage) -> DynamicImage {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut out = RgbImage::new(w, h);
    for (op, ip) in out.pixels_mut().zip(rgba.pixels()) {
        // I mode packs int32 as RGBA bytes (little-endian)
        let val = i32::from_le_bytes([ip[0], ip[1], ip[2], ip[3]]);
        let clamped = val.clamp(0, 255) as u8;
        *op = crate::raster::Rgb([clamped, clamped, clamped]);
    }
    DynamicImage::ImageRgb8(out)
}

/// Converts Pillow `"F"` storage to RGB.
///
/// The input is RGBA storage interpreted as little-endian `f32` pixels. Each
/// value is clamped to `0..=255`, truncated to a byte, and broadcast to `R=G=B`.
pub fn f_to_rgb(img: &DynamicImage) -> DynamicImage {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut out = RgbImage::new(w, h);
    for (op, ip) in out.pixels_mut().zip(rgba.pixels()) {
        let val = f32::from_le_bytes([ip[0], ip[1], ip[2], ip[3]]);
        // PIL: F→X casts float to int via truncation
        let clamped = val.clamp(0.0, 255.0) as u8;
        *op = crate::raster::Rgb([clamped, clamped, clamped]);
    }
    DynamicImage::ImageRgb8(out)
}

/// Converts `P` palette-index storage to RGB.
///
/// `img` is interpreted as one index byte per pixel. When `palette` is present,
/// it must be a flat sequence of RGB triples; missing entries map to black.
/// Without a palette, each index maps to the grayscale ramp `RGB(i, i, i)`.
pub fn p_to_rgb(img: &DynamicImage, palette: Option<&[u8]>) -> DynamicImage {
    let luma = img.to_luma8();
    let (w, h) = luma.dimensions();
    let mut out = crate::raster::RgbImage::new(w, h);
    for (op, ip) in out.pixels_mut().zip(luma.pixels()) {
        let idx = ip[0] as usize;
        if let Some(pal) = palette {
            if idx * 3 + 2 < pal.len() {
                op[0] = pal[idx * 3];
                op[1] = pal[idx * 3 + 1];
                op[2] = pal[idx * 3 + 2];
            } else {
                op[0] = 0;
                op[1] = 0;
                op[2] = 0;
            }
        } else {
            // PIL default palette: grayscale ramp
            op[0] = ip[0];
            op[1] = ip[0];
            op[2] = ip[0];
        }
    }
    DynamicImage::ImageRgb8(out)
}

/// Converts a non-standard Pillow mode into a standard RGB-family image.
///
/// `palette` supplies `P` mode RGB triples when `src_mode == "P"`.
///
/// # Returns
///
/// `Some(image)` for modes that need reinterpretation (`CMYK`, `HSV`, `YCbCr`,
/// `I`, `F`, `P`) and `None` for modes that are already standard.
pub fn convert_from_nonstandard(
    src_mode: &str,
    img: &DynamicImage,
    palette: Option<&[u8]>,
) -> Option<DynamicImage> {
    match src_mode {
        "CMYK" => Some(cmyk_to_rgb(img)),
        "HSV" => Some(hsv_to_rgb(img)),
        "YCbCr" => Some(ycbcr_to_rgb(img)),
        "I" => Some(i_to_rgb(img)),
        "F" => Some(f_to_rgb(img)),
        "P" => Some(p_to_rgb(img, palette)),
        _ => None,
    }
}

/// Validates and finds or appends a palette color.
///
/// RGB palettes reject non-opaque RGBA colors. RGBA palettes use alpha from the
/// color input when present and default to `255` otherwise.
///
/// # Errors
///
/// Returns an error string when the color is too short, invalid for the palette
/// mode, or cannot be appended because the palette is full.
pub fn palette_getcolor_validate(
    palette: &mut Vec<u8>,
    color: &[u8],
    mode: &str,
) -> Result<usize, String> {
    if color.is_empty() {
        return Err("color must have at least 1 element".into());
    }
    if mode == "RGB" && color.len() >= 4 && color[3] != 255 {
        return Err("cannot add non-opaque RGBA color to RGB palette".into());
    }
    // Pillow's Python `getcolor` appends the tuple bytes up to the mode band
    // count without padding (RGBA pads a 3-tuple to opaque), and matches
    // existing entries by the exact tuple.
    let mode_len = if mode == "RGBA" { 4 } else { 3 };
    let mut stored = color.to_vec();
    if mode == "RGBA" && stored.len() == 3 {
        stored.push(255);
    }
    if stored.len() > mode_len {
        stored.truncate(mode_len);
    }
    let entries = palette.len() / mode_len;
    for i in 0..entries {
        let start = i * mode_len;
        if palette.get(start..start + stored.len()) == Some(&stored[..]) {
            return Ok(i);
        }
    }
    let idx = entries;
    if idx >= 256 {
        return Err("cannot allocate more than 256 colors".into());
    }
    palette.extend_from_slice(&stored);
    Ok(idx)
}
