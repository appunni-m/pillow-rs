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
    // `csscolorparser` accepts bare hexadecimal words such as `bad` and
    // `deadbeef`, while Pillow only accepts hexadecimal colors with a leading
    // `#`; unprefixed values are looked up as named colors instead.
    if !lowered.starts_with('#')
        && matches!(lowered.len(), 3 | 4 | 6 | 8)
        && lowered.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
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
