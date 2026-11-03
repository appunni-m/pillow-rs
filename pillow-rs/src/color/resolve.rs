use super::parse_color_str;
use crate::raster::Mode;

/// Pillow `ImageMode` descriptors accepted by `ImageColor.getcolor`.
///
/// This is a color-result descriptor, not a raster identity. In particular,
/// Pillow accepts descriptors such as `I;32` and `I;16S` that have no
/// corresponding `raster::Mode` or `DynamicImage` variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorOutputMode {
    /// Pillow's bilevel `1` descriptor.
    One,
    /// Pillow's luma `L` descriptor.
    Luma,
    /// Pillow's luma-alpha `LA` descriptor.
    LumaAlpha,
    /// Pillow's premultiplied luma-alpha `La` descriptor.
    LumaAlphaPremultiplied,
    /// Pillow's palette-index `P` descriptor.
    Palette,
    /// Pillow's palette-alpha `PA` descriptor.
    PaletteAlpha,
    /// Pillow's RGB descriptor.
    Rgb,
    /// Pillow's RGBX descriptor.
    Rgbx,
    /// Pillow's RGBA descriptor.
    Rgba,
    /// Pillow's premultiplied RGBA `RGBa` descriptor.
    RgbPremultiplied,
    /// Pillow's CMYK descriptor.
    Cmyk,
    /// Pillow's YCbCr descriptor.
    Ycbcr,
    /// Pillow's LAB descriptor.
    Lab,
    /// Pillow's HSV descriptor.
    Hsv,
    /// Pillow's signed integer `I` descriptor.
    Integer,
    /// Pillow's float `F` descriptor.
    Float,
    /// Pillow's native unsigned 16-bit `I;16` descriptor.
    Integer16,
    /// Pillow's signed 16-bit `I;16S` descriptor.
    Integer16Signed,
    /// Pillow's little-endian unsigned 16-bit `I;16L` descriptor.
    Integer16LittleEndian,
    /// Pillow's little-endian signed 16-bit `I;16LS` descriptor.
    Integer16LittleEndianSigned,
    /// Pillow's big-endian unsigned 16-bit `I;16B` descriptor.
    Integer16BigEndian,
    /// Pillow's big-endian signed 16-bit `I;16BS` descriptor.
    Integer16BigEndianSigned,
    /// Pillow's native-order unsigned 16-bit `I;16N` descriptor.
    Integer16Native,
    /// Pillow's native-order signed 16-bit `I;16NS` descriptor.
    Integer16NativeSigned,
    /// Pillow's signed 32-bit `I;32` descriptor.
    Integer32,
    /// Pillow's big-endian signed 32-bit `I;32B` descriptor.
    Integer32BigEndian,
    /// Pillow's little-endian signed 32-bit `I;32L` descriptor.
    Integer32LittleEndian,
    /// Pillow's signed 32-bit `I;32S` descriptor.
    Integer32Signed,
    /// Pillow's big-endian signed 32-bit `I;32BS` descriptor.
    Integer32BigEndianSigned,
    /// Pillow's little-endian signed 32-bit `I;32LS` descriptor.
    Integer32LittleEndianSigned,
}

impl ColorOutputMode {
    /// Parses a Pillow `ImageMode` descriptor at a host/API boundary.
    #[must_use]
    pub fn from_pillow_name(name: &str) -> Option<Self> {
        match name {
            "1" => Some(Self::One),
            "L" => Some(Self::Luma),
            "LA" => Some(Self::LumaAlpha),
            "La" => Some(Self::LumaAlphaPremultiplied),
            "P" => Some(Self::Palette),
            "PA" => Some(Self::PaletteAlpha),
            "RGB" => Some(Self::Rgb),
            "RGBX" => Some(Self::Rgbx),
            "RGBA" => Some(Self::Rgba),
            "RGBa" => Some(Self::RgbPremultiplied),
            "CMYK" => Some(Self::Cmyk),
            "YCbCr" => Some(Self::Ycbcr),
            "LAB" => Some(Self::Lab),
            "HSV" => Some(Self::Hsv),
            "I" => Some(Self::Integer),
            "F" => Some(Self::Float),
            "I;16" => Some(Self::Integer16),
            "I;16S" => Some(Self::Integer16Signed),
            "I;16L" => Some(Self::Integer16LittleEndian),
            "I;16LS" => Some(Self::Integer16LittleEndianSigned),
            "I;16B" => Some(Self::Integer16BigEndian),
            "I;16BS" => Some(Self::Integer16BigEndianSigned),
            "I;16N" => Some(Self::Integer16Native),
            "I;16NS" => Some(Self::Integer16NativeSigned),
            "I;32" => Some(Self::Integer32),
            "I;32B" => Some(Self::Integer32BigEndian),
            "I;32L" => Some(Self::Integer32LittleEndian),
            "I;32S" => Some(Self::Integer32Signed),
            "I;32BS" => Some(Self::Integer32BigEndianSigned),
            "I;32LS" => Some(Self::Integer32LittleEndianSigned),
            _ => None,
        }
    }

    /// Parses a host Pillow descriptor while preserving Pillow's unknown-name error.
    pub fn parse_pillow_name(name: &str) -> Result<Self, crate::error::PilError> {
        Self::from_pillow_name(name)
            .ok_or_else(|| crate::error::PilError::KeyError(name.to_owned()))
    }
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
    mode: Mode,
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
    if mode == Mode::ImageI32
        && let Some(v) = int32_val
    {
        let bytes = v.to_le_bytes();
        return Ok((bytes[0], bytes[1], bytes[2], bytes[3]));
    }
    // For F mode with f64 value, pack as 4-byte LE f32
    if mode == Mode::ImageF32
        && let Some(v) = float_val
    {
        let f = v as f32;
        let bytes = f.to_le_bytes();
        return Ok((bytes[0], bytes[1], bytes[2], bytes[3]));
    }
    if matches!(
        mode,
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N
    )
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
        if mode == Mode::ImagePalette8 {
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
        if mode == Mode::ImageF32 {
            return Err(crate::error::PilError::TypeError(
                "must be real number, not tuple".to_owned(),
            ));
        }
        if matches!(
            mode,
            Mode::ImageI32
                | Mode::ImageI16
                | Mode::ImageI16L
                | Mode::ImageI16B
                | Mode::ImageI16N
        ) {
            return Err(crate::error::PilError::TypeError(
                "color must be int or single-element tuple".to_owned(),
            ));
        }
        if matches!(mode, Mode::ImageLuma8 | Mode::One | Mode::ImagePalette8) {
            if mode != Mode::ImagePalette8 {
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

/// Resolve parsed RGBA components to a typed Pillow color-output descriptor.
///
/// Components retain their raw integer values; this function does not clamp
/// them before luma or HSV calculations. Parse host strings with
/// [`ColorOutputMode::parse_pillow_name`] at the binding boundary.
pub fn getcolor(
    r: i32,
    g: i32,
    b: i32,
    a: i32,
    mode: ColorOutputMode,
) -> ColorValue {
    // Pillow's getcolor computes from the raw (unclamped) parsed components:
    // `(r * 19595 + g * 38470 + b * 7471 + 0x8000) >> 16` and colorsys HSV on
    // r/255, so out-of-range values like rgb(300,0,0) produce 90 / (0,255,300).
    let luma = (r * 19595 + g * 38470 + b * 7471 + 0x8000) >> 16;
    if mode == ColorOutputMode::Hsv {
        let (h, s, v) = rgb_to_hsv_i32(r, g, b);
        return ColorValue::Hsv(h, s, v);
    }
    if matches!(
        mode,
        ColorOutputMode::Luma
            | ColorOutputMode::LumaAlpha
            | ColorOutputMode::LumaAlphaPremultiplied
            | ColorOutputMode::One
            | ColorOutputMode::Integer
            | ColorOutputMode::Float
            | ColorOutputMode::Integer16
            | ColorOutputMode::Integer16Signed
            | ColorOutputMode::Integer16LittleEndian
            | ColorOutputMode::Integer16LittleEndianSigned
            | ColorOutputMode::Integer16BigEndian
            | ColorOutputMode::Integer16BigEndianSigned
            | ColorOutputMode::Integer16Native
            | ColorOutputMode::Integer16NativeSigned
            | ColorOutputMode::Integer32
            | ColorOutputMode::Integer32BigEndian
            | ColorOutputMode::Integer32LittleEndian
            | ColorOutputMode::Integer32Signed
            | ColorOutputMode::Integer32BigEndianSigned
            | ColorOutputMode::Integer32LittleEndianSigned
    ) {
        return match mode {
            ColorOutputMode::LumaAlpha => ColorValue::GrayAlpha(luma, a),
            ColorOutputMode::LumaAlphaPremultiplied => {
                ColorValue::GrayAlpha(premultiply_color_component(luma, a), a)
            }
            _ => ColorValue::Gray(luma),
        };
    }
    if mode == ColorOutputMode::RgbPremultiplied {
        return ColorValue::Rgba(
            premultiply_color_component(r, a),
            premultiply_color_component(g, a),
            premultiply_color_component(b, a),
            a,
        );
    }
    if matches!(mode, ColorOutputMode::PaletteAlpha | ColorOutputMode::Rgba) {
        return ColorValue::Rgba(r, g, b, a);
    }
    ColorValue::Rgb(r, g, b)
}

/// Matches Pillow's nearest-integer scaling for a premultiplied color channel.
fn premultiply_color_component(component: i32, alpha: i32) -> i32 {
    let product = i64::from(component) * i64::from(alpha);
    let rounded = if product < 0 {
        -((-product + 127) / 255)
    } else {
        (product + 127) / 255
    };
    rounded as i32
}

/// Resolve parsed RGB components using an exact raster mode identity.
///
/// Unlike [`getcolor`], this operation path does not round-trip through a
/// Pillow mode spelling. Internal raster identities use their documented
/// carrier's color result; operation support still decides whether they can
/// execute a requested transform.
pub(crate) fn getcolor_for_mode(
    r: i32,
    g: i32,
    b: i32,
    a: i32,
    mode: Mode,
) -> ColorValue {
    let luma = (r * 19595 + g * 38470 + b * 7471 + 0x8000) >> 16;
    match mode {
        Mode::ImageHsv8 => {
            let (h, s, v) = rgb_to_hsv_i32(r, g, b);
            ColorValue::Hsv(h, s, v)
        }
        Mode::ImageLumaA8 => ColorValue::GrayAlpha(luma, a),
        Mode::ImageLaPremultiplied8 => {
            ColorValue::GrayAlpha(premultiply_color_component(luma, a), a)
        }
        // NO_PILLOW_SUPPORT: internal high-depth LA uses its native channels.
        Mode::ImageLumaA16 => ColorValue::GrayAlpha(luma, a),
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N => ColorValue::Gray(luma),
        Mode::ImagePaletteAlpha8 | Mode::ImageRgba8 => ColorValue::Rgba(r, g, b, a),
        Mode::ImageRgbPremultiplied8 => ColorValue::Rgba(
            premultiply_color_component(r, a),
            premultiply_color_component(g, a),
            premultiply_color_component(b, a),
            a,
        ),
        // NO_PILLOW_SUPPORT: internal high-depth RGBA uses its native channels.
        Mode::ImageRgba16 | Mode::ImageRgba32F => ColorValue::Rgba(r, g, b, a),
        Mode::ImagePalette8
        | Mode::ImageRgb8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8 => ColorValue::Rgb(r, g, b),
        // NO_PILLOW_SUPPORT: internal high-depth RGB uses its native channels.
        Mode::ImageRgb16 | Mode::ImageRgb32F => ColorValue::Rgb(r, g, b),
    }
}
