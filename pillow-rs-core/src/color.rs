use image::ColorType;

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
