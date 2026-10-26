use super::parse_color_str;

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
    if mode == "I"
        && let Some(v) = int32_val
    {
        let bytes = v.to_le_bytes();
        return Ok((bytes[0], bytes[1], bytes[2], bytes[3]));
    }
    // For F mode with f64 value, pack as 4-byte LE f32
    if mode == "F"
        && let Some(v) = float_val
    {
        let f = v as f32;
        let bytes = f.to_le_bytes();
        return Ok((bytes[0], bytes[1], bytes[2], bytes[3]));
    }
    if matches!(mode, "I;16" | "I;16L" | "I;16B" | "I;16N")
        && let Some(v) = int32_val
    {
        let bytes = (v as u16).to_le_bytes();
        return Ok((bytes[0], bytes[1], 0, 0));
    }
    if let Some(v) = single_value {
        // PIL: single int for multi-band images = first band = value, rest = 0
        return Ok((v, 0, 0, 0));
    }
    if let Some((l, a)) = la {
        if mode == "P" {
            // Pillow's single-band color validation rejects a two-component
            // tuple before palette allocation; it does not reinterpret LA as
            // an RGB palette entry.
            return Err(crate::error::PilError::TypeError(
                "color must be int or single-element tuple".to_owned(),
            ));
        }
        return Ok((l, l, l, a));
    }
    if let Some((r, g, b)) = rgb {
        // Pillow accepts only a scalar or one-element tuple for single-band
        // modes. The F path uses a distinct native conversion error, while
        // integer/luma modes share the standard color-type message.
        if mode == "F" {
            return Err(crate::error::PilError::TypeError(
                "must be real number, not tuple".to_owned(),
            ));
        }
        if matches!(mode, "I" | "I;16" | "I;16L" | "I;16B" | "I;16N") {
            return Err(crate::error::PilError::TypeError(
                "color must be int or single-element tuple".to_owned(),
            ));
        }
        if mode == "L" || mode == "1" || mode == "P" {
            if mode != "P" {
                return Err(crate::error::PilError::TypeError(
                    "color must be int or single-element tuple".to_owned(),
                ));
            }
            // The Python Image.new wrapper preserves P-mode tuple provenance
            // and allocates the tuple color into the palette after resolution.
            // Keep the resolver's zero-index sentinel for that binding path.
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
    /// One grayscale sample, including Pillow's raw integer color semantics.
    Gray(i32),
    /// Grayscale and alpha samples.
    GrayAlpha(i32, i32),
    /// Red, green, and blue samples.
    Rgb(i32, i32, i32),
    /// Red, green, blue, and alpha samples.
    Rgba(i32, i32, i32, i32),
    /// Hue, saturation, and value samples on Pillow's 0–255 scale.
    Hsv(i32, i32, i32),
}

/// Return whether `mode` is one of the descriptors accepted by Pillow's
/// `ImageMode.getmode`. `ImageColor.getcolor` asks that resolver for every
/// non-HSV mode, so an unknown name must raise `KeyError` instead of silently
/// taking the RGB fallback.
fn is_image_mode(mode: &str) -> bool {
    matches!(
        mode,
        "1" | "L"
            | "LA"
            | "La"
            | "P"
            | "PA"
            | "RGB"
            | "RGBX"
            | "RGBA"
            | "RGBa"
            | "CMYK"
            | "YCbCr"
            | "LAB"
            | "HSV"
            | "I"
            | "F"
            | "I;16"
            | "I;16S"
            | "I;16L"
            | "I;16LS"
            | "I;16B"
            | "I;16BS"
            | "I;16N"
            | "I;16NS"
            | "I;32"
            | "I;32B"
            | "I;32L"
            | "I;32S"
            | "I;32BS"
            | "I;32LS"
    )
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

/// Resolve parsed RGBA components to the requested Pillow color mode.
///
/// Components retain their raw integer values; this function does not clamp
/// them before luma or HSV calculations. Unknown modes return a key error.
pub fn getcolor(
    r: i32,
    g: i32,
    b: i32,
    a: i32,
    mode: &str,
) -> Result<ColorValue, crate::error::PilError> {
    if !is_image_mode(mode) {
        return Err(crate::error::PilError::KeyError(mode.to_owned()));
    }
    // Pillow's getcolor computes from the raw (unclamped) parsed components:
    // `(r * 19595 + g * 38470 + b * 7471 + 0x8000) >> 16` and colorsys HSV on
    // r/255, so out-of-range values like rgb(300,0,0) produce 90 / (0,255,300).
    let luma = (r * 19595 + g * 38470 + b * 7471 + 0x8000) >> 16;
    if mode == "HSV" {
        let (h, s, v) = rgb_to_hsv_i32(r, g, b);
        return Ok(ColorValue::Hsv(h, s, v));
    }
    if matches!(
        mode,
        "L" | "LA"
            | "La"
            | "1"
            | "I"
            | "F"
            | "I;16"
            | "I;16S"
            | "I;16L"
            | "I;16LS"
            | "I;16B"
            | "I;16BS"
            | "I;16N"
            | "I;16NS"
            | "I;32"
            | "I;32B"
            | "I;32L"
            | "I;32S"
            | "I;32BS"
            | "I;32LS"
    ) {
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
