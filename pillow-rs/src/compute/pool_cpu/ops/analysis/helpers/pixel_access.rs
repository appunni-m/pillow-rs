use crate::compute::registry::support::{
    ModeSupport, OperationId, OperationValueResult, OutputModeRule, PixelValueShape,
    StrategySelection,
};
use crate::compute::{Architecture, Backend};
use crate::error::PilError;
use crate::image::FormattedPixelValue;
use crate::raster::{
    DynamicImage, GenericImageView, GrayAlphaImage, GrayImage, ImageBuffer, Luma, LumaA, Rgb,
    Rgb32FImage, RgbImage, Rgba, Rgba32FImage, RgbaImage,
};

/// Declares a Pillow-defined Default strategy for one exact GetPixel mode.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) const fn pillow_get_pixel_support(
    mode: crate::raster::Mode,
    shape: PixelValueShape,
) -> ModeSupport {
    ModeSupport::cpu(
        OperationId::GetPixel,
        mode,
        OutputModeRule::NoRasterResult,
        &[],
        &[],
    )
    .with_value_result(OperationValueResult::Pixel(shape))
}

/// Declares a native high-depth GetPixel extension for one exact mode.
// NO_PILLOW_SUPPORT: GetPixel returns the native sample shape of internal high-depth modes.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) const fn core_get_pixel_support(
    mode: crate::raster::Mode,
    shape: PixelValueShape,
) -> ModeSupport {
    ModeSupport::cpu_core_only(
        OperationId::GetPixel,
        mode,
        OutputModeRule::NoRasterResult,
        &[],
        &[],
        "native high-depth GetPixel values extend Pillow's mode contract",
    )
    .with_value_result(OperationValueResult::Pixel(shape))
}

/// Declares the Rust/JavaScript fixed-RGBA8 access as a core extension.
// NO_PILLOW_SUPPORT: this Rust/JavaScript accessor has no matching Pillow GetPixel result.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) const fn rgba_get_pixel_support(mode: crate::raster::Mode) -> ModeSupport {
    ModeSupport::cpu_core_only(
        OperationId::GetPixelRgba,
        mode,
        OutputModeRule::NoRasterResult,
        &[],
        &[],
        "the Rust/JavaScript accessor returns a fixed RGBA8 value",
    )
    .with_value_result(OperationValueResult::Pixel(PixelValueShape::Rgba8))
}

/// Checks that a mode owner received its declared Default pixel-read strategy.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn validate_default_selection(
    selection: StrategySelection,
    operation: OperationId,
    mode: crate::raster::Mode,
) -> Result<(), PilError> {
    if selection.implementation.operation != operation
        || selection.implementation.mode != mode
        || selection.implementation.backend != Backend::Cpu
        || selection.implementation.architecture != Architecture::Default
    {
        return Err(PilError::InternalError(
            "pixel read selection did not match its exact mode and Default implementation".into(),
        ));
    }
    Ok(())
}

/// Reads the sample used by Pillow's scalar byte modes.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn scalar_u8(image: &GrayImage, x: u32, y: u32) -> FormattedPixelValue {
    FormattedPixelValue::Scalar(image.get_pixel(x, y).0[0])
}

/// Reads the two byte components used by LA, La, and PA.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn components_u8_2(image: &GrayAlphaImage, x: u32, y: u32) -> FormattedPixelValue {
    FormattedPixelValue::Components(image.get_pixel(x, y).0.to_vec())
}

/// Reads a native three-byte pixel.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn components_u8_3(image: &RgbImage, x: u32, y: u32) -> FormattedPixelValue {
    FormattedPixelValue::Components(image.get_pixel(x, y).0.to_vec())
}

/// Reads LAB samples in Pillow's signed-offset component representation.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn components_lab8(image: &RgbImage, x: u32, y: u32) -> FormattedPixelValue {
    let [luma, a, b] = image.get_pixel(x, y).0;
    FormattedPixelValue::Components(vec![luma, a.wrapping_sub(128), b.wrapping_sub(128)])
}

/// Reads four native byte components, including physical RGBX and CMYK lanes.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn components_u8_4(image: &RgbaImage, x: u32, y: u32) -> FormattedPixelValue {
    FormattedPixelValue::Components(image.get_pixel(x, y).0.to_vec())
}

/// Reads the raw I sample from Pillow's four-byte scalar carrier.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn scalar_i32(
    image: &RgbaImage,
    x: u32,
    y: u32,
) -> Result<FormattedPixelValue, PilError> {
    let sample = scalar_bytes(image, x, y)?;
    Ok(FormattedPixelValue::Integer(i32::from_le_bytes(sample)))
}

/// Reads the raw F sample from Pillow's four-byte scalar carrier.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn scalar_f32(
    image: &RgbaImage,
    x: u32,
    y: u32,
) -> Result<FormattedPixelValue, PilError> {
    let sample = scalar_bytes(image, x, y)?;
    Ok(FormattedPixelValue::Float(f64::from(f32::from_le_bytes(
        sample,
    ))))
}

/// Reads one native unsigned 16-bit luma sample.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn scalar_u16(
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    x: u32,
    y: u32,
) -> Result<FormattedPixelValue, PilError> {
    let index = pixel_index(image.width(), x, y)?;
    let sample = image
        .as_raw()
        .get(index)
        .copied()
        .ok_or_else(|| PilError::InternalError("I;16 pixel index out of bounds".into()))?;
    Ok(FormattedPixelValue::Integer(i32::from(sample)))
}

/// Reads native unsigned 16-bit LA samples.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn components_u16_2(
    image: &ImageBuffer<LumaA<u16>, Vec<u16>>,
    x: u32,
    y: u32,
) -> FormattedPixelValue {
    FormattedPixelValue::U16Components(image.get_pixel(x, y).0.to_vec())
}

/// Reads native unsigned 16-bit RGB samples.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn components_u16_3(
    image: &ImageBuffer<Rgb<u16>, Vec<u16>>,
    x: u32,
    y: u32,
) -> FormattedPixelValue {
    FormattedPixelValue::U16Components(image.get_pixel(x, y).0.to_vec())
}

/// Reads native unsigned 16-bit RGBA samples.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn components_u16_4(
    image: &ImageBuffer<Rgba<u16>, Vec<u16>>,
    x: u32,
    y: u32,
) -> FormattedPixelValue {
    FormattedPixelValue::U16Components(image.get_pixel(x, y).0.to_vec())
}

/// Reads native 32-bit float RGB samples.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn components_f32_3(image: &Rgb32FImage, x: u32, y: u32) -> FormattedPixelValue {
    FormattedPixelValue::FloatComponents(image.get_pixel(x, y).0.to_vec())
}

/// Reads native 32-bit float RGBA samples.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn components_f32_4(image: &Rgba32FImage, x: u32, y: u32) -> FormattedPixelValue {
    FormattedPixelValue::FloatComponents(image.get_pixel(x, y).0.to_vec())
}

/// Reads one pixel using the Rust/JavaScript accessor's fixed RGBA8 shape.
// NO_PILLOW_SUPPORT: Pillow's `getpixel` preserves each mode's scalar/tuple shape.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn rgba_pixel(image: &DynamicImage, x: u32, y: u32) -> [u8; 4] {
    image.get_pixel(x, y).0
}

/// Reads a direct RGB tuple without allocating the formatted component vector.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn rgb_pixel(image: &RgbImage, x: u32, y: u32) -> [u8; 3] {
    image.get_pixel(x, y).0
}

/// Returns the number of host value bytes produced by one formatted pixel.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn formatted_pixel_bytes(value: &FormattedPixelValue) -> usize {
    match value {
        FormattedPixelValue::Scalar(_) => 1,
        FormattedPixelValue::Integer(_) => std::mem::size_of::<i32>(),
        FormattedPixelValue::Float(_) => std::mem::size_of::<f64>(),
        FormattedPixelValue::Components(values) => values.len(),
        FormattedPixelValue::U16Components(values) => {
            values.len().saturating_mul(std::mem::size_of::<u16>())
        }
        FormattedPixelValue::FloatComponents(values) => {
            values.len().saturating_mul(std::mem::size_of::<f32>())
        }
        FormattedPixelValue::InlineComponents { bands, .. } => usize::from(*bands),
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn pixel_index(width: u32, x: u32, y: u32) -> Result<usize, PilError> {
    (y as usize)
        .checked_mul(width as usize)
        .and_then(|row| row.checked_add(x as usize))
        .ok_or_else(|| PilError::InternalError("pixel index overflow".into()))
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn scalar_bytes(image: &RgbaImage, x: u32, y: u32) -> Result<[u8; 4], PilError> {
    let offset = pixel_index(image.width(), x, y)?
        .checked_mul(4)
        .ok_or_else(|| PilError::InternalError("scalar pixel offset overflow".into()))?;
    let end = offset
        .checked_add(4)
        .ok_or_else(|| PilError::InternalError("scalar pixel offset overflow".into()))?;
    let sample = image
        .as_raw()
        .get(offset..end)
        .ok_or_else(|| PilError::InternalError("scalar pixel index out of bounds".into()))?;
    Ok([sample[0], sample[1], sample[2], sample[3]])
}
