use crate::error::PilError;

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
    palette.chunks_exact(3).position(|entry| entry == [r, g, b])
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
    let color = if mode == "RGBA" {
        vec![r, g, b, a]
    } else {
        vec![r, g, b]
    };
    palette_getcolor_validate_impl(palette, &color, mode).map_err(palette_error_message)
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
/// Validates and finds or appends a palette color.
///
/// RGB palettes reject non-opaque RGBA colors. RGBA palettes use alpha from the
/// color input when present and default to `255` otherwise.
///
/// # Errors
///
/// Returns an error string when the color is invalid for the palette mode or
/// cannot be appended because the palette is full. An empty color returns the
/// next slot index without changing the palette, matching Pillow.
fn palette_getcolor_validate_impl(
    palette: &mut Vec<u8>,
    color: &[u8],
    mode: &str,
) -> Result<usize, PilError> {
    if mode == "RGB" && color.len() >= 4 && color[3] != 255 {
        return Err(PilError::ValueError(
            "cannot add non-opaque RGBA color to RGB palette".into(),
        ));
    }
    // Pillow's Python `getcolor` appends the tuple bytes up to the mode band
    // count without padding (RGBA pads a 3-tuple to opaque), and matches
    // existing entries by the exact tuple.
    let mode_len = if mode == "RGBA" { 4 } else { 3 };
    // Pillow's ImagePalette.getcolor(()) still allocates the next slot index;
    // bytes(()) is empty, so the palette remains unchanged. Preserve that
    // public behavior instead of rejecting the empty tuple in core validation.
    if color.is_empty() {
        return Ok(palette.len() / mode_len);
    }
    let mut stored = color.to_vec();
    if mode == "RGBA" && stored.len() == 3 {
        stored.push(255);
    }
    if stored.len() > mode_len {
        stored.truncate(mode_len);
    }
    if mode == "RGB" && stored.len() == 3 {
        if let Some(index) = palette_getcolor(palette, stored[0], stored[1], stored[2]) {
            return Ok(index);
        }
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
        return Err(PilError::ValueError(
            "cannot allocate more than 256 colors".into(),
        ));
    }
    palette.extend_from_slice(&stored);
    Ok(idx)
}

/// Validates and stores a palette color from a host-neutral component slice.
pub fn palette_getcolor_validate(
    palette: &mut Vec<u8>,
    color: &[u8],
    mode: &str,
) -> Result<usize, String> {
    palette_getcolor_validate_impl(palette, color, mode).map_err(palette_error_message)
}

fn palette_error_message(error: PilError) -> String {
    match error {
        PilError::ValueError(message) => message,
        other => other.to_string(),
    }
}

/// Host-neutral input for `ImagePalette.getcolor`.
#[derive(Debug, Clone)]
pub enum PaletteColorInput {
    /// A tuple/list whose components were extracted by a binding.
    Components(Vec<u8>),
    /// A value that is not a tuple/list color specification. The string is
    /// only a host-provided representation used in Pillow's error message.
    Invalid(String),
}

/// Validates and stores a palette color from a host-neutral input.
pub fn palette_getcolor_validate_input(
    palette: &mut Vec<u8>,
    color: PaletteColorInput,
    mode: &str,
) -> Result<usize, String> {
    let result = match color {
        PaletteColorInput::Components(color)
            if (mode == "RGB" && color.len() == 3)
                || (mode == "RGBA" && matches!(color.len(), 3 | 4)) =>
        {
            palette_getcolor_validate_impl(palette, &color, mode)
        }
        PaletteColorInput::Components(color) => {
            palette_getcolor_validate_impl(palette, &color, mode)
        }
        PaletteColorInput::Invalid(repr) => Err(PilError::ValueError(format!(
            "unknown color specifier: {repr}"
        ))),
    };
    result.map_err(palette_error_message)
}
