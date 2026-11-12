//! Host conversion for supplied Quantize palette carriers.

use crate::{Bound, PyAny, PyResult, map_error};
use pillow_rs::{Image as RsImage, Mode, QuantizePalette};
use pyo3::exceptions::PyAttributeError;
use pyo3::types::PyAnyMethods;

/// Converts native and foreign Pillow-compatible palette carriers to core input.
// VERIFY: source=reviewed@2026-11-12; parity=pass@quantize-supplied-palette-20261112-cpu-default; benchmark=pending
pub(super) fn quantize_palette_from_python(value: &Bound<'_, PyAny>) -> PyResult<QuantizePalette> {
    if let Some(image) = crate::inputs::image_from_python(value) {
        return Ok(QuantizePalette::Image(Box::new(image)));
    }

    let mode_value = match value.getattr("mode") {
        Ok(mode) => mode,
        Err(error) if error.is_instance_of::<PyAttributeError>(value.py()) => {
            return Ok(QuantizePalette::Other);
        }
        Err(error) => return Err(error),
    };
    let Ok(image_mode) = mode_value.extract::<String>() else {
        return Ok(QuantizePalette::Other);
    };
    let Some(image_mode) = Mode::from_pillow_name(&image_mode) else {
        return Ok(QuantizePalette::Other);
    };
    if image_mode != Mode::ImagePalette8 {
        let image = RsImage::new(1, 1, image_mode, (0, 0, 0, 255)).map_err(map_error)?;
        return Ok(QuantizePalette::Image(Box::new(image)));
    }

    // A foreign ImagingCore exposes its attached palette through these native
    // methods. Copy only the small color table into the Rust-owned P carrier;
    // pixel data and quantization remain in the typed Rust core.
    // TODO(pillow-rs#3ptb): a foreign P ImagingCore with no palette pointer
    // triggers Pillow's source-mode-dependent grayscale/WEB default palette;
    // carry the exact source Mode into core before adapting that route.
    let palette_mode_method = match value.getattr("getpalettemode") {
        Ok(method) => method,
        Err(error) if error.is_instance_of::<PyAttributeError>(value.py()) => {
            return Ok(QuantizePalette::Other);
        }
        Err(error) => return Err(error),
    };
    let palette_mode = palette_mode_method.call0()?.extract::<String>()?;
    let palette_method = match value.getattr("getpalette") {
        Ok(method) => method,
        Err(error) if error.is_instance_of::<PyAttributeError>(value.py()) => {
            return Ok(QuantizePalette::Other);
        }
        Err(error) => return Err(error),
    };
    let palette_bytes = palette_method
        .call1((palette_mode.as_str(), palette_mode.as_str()))?
        .extract::<Vec<u8>>()?;
    let mut image = RsImage::new(1, 1, Mode::ImagePalette8, (0, 0, 0, 255)).map_err(map_error)?;
    image
        .putpalette(&palette_bytes, &palette_mode)
        .map_err(map_error)?;
    Ok(QuantizePalette::Image(Box::new(image)))
}
