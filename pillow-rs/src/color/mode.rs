use crate::raster::ColorType;

/// Maps a codec color type to the nearest Pillow mode string.
///
/// # Returns
///
/// A static mode identifier such as `"L"`, `"LA"`, `"RGB"`, `"RGBA"`, or
/// `"I;16"`. Unknown color types fall back to `"RGB"` because the core image
/// APIs operate on RGB-compatible buffers for forward-compatible codec
/// metadata.
pub fn color_type_to_mode(ct: ColorType) -> &'static str {
    match ct {
        ColorType::L8 => "L",
        ColorType::La8 => "LA",
        ColorType::Rgb8 => "RGB",
        ColorType::Rgba8 => "RGBA",
        ColorType::Cmyk8 => "CMYK",
        ColorType::L16 => "I;16",
        ColorType::La16 => "LA",
        ColorType::Rgb16 | ColorType::Rgb32F => "RGB",
        ColorType::Rgba16 | ColorType::Rgba32F => "RGBA",
        ColorType::L32F => "F",
        ColorType::L32I => "I",
        _ => "RGB",
    }
}
