//! Pillow-compatible scalar and tuple normalization for `Image.putpixel`.

use crate::error::PilError;
use crate::raster::Mode;

use super::super::Image;
use super::super::data_access::is_l16_identity;
use super::{PutPixelExecutionRoute, PutPixelValue};

impl Image {
    /// Applies Pillow's public `putpixel` scalar/tuple normalization.
    // TODO(pillow-rs#3ptb): verify typed input conversion, palette mutation,
    // partial writes, and strategy receipts over all modes in CPU parity.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    pub fn putpixel_value(&mut self, x: u32, y: u32, value: PutPixelValue) -> Result<(), PilError> {
        self.putpixel_value_with_route(x, y, value, None)
    }

    /// Applies Pillow's `putpixel` contract under a parent operation route.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn putpixel_value_with_context(
        &mut self,
        x: u32,
        y: u32,
        value: PutPixelValue,
        requested_backend: Option<crate::compute::Backend>,
        context: crate::compute::ExecutionContext,
    ) -> Result<(), PilError> {
        self.putpixel_value_with_route(x, y, value, Some((requested_backend, context)))
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn putpixel_value_with_route(
        &mut self,
        x: u32,
        y: u32,
        value: PutPixelValue,
        route: Option<PutPixelExecutionRoute>,
    ) -> Result<(), PilError> {
        let allocates_palette_color = self.has_palette_samples()
            && matches!(&value, PutPixelValue::Components(values) if matches!(values.len(), 3 | 4));
        let value = if allocates_palette_color {
            let reserved = self.palette_reserved_indices();
            self.prepare_putpixel_palette(value, &reserved)?
        } else {
            self.prepare_putpixel_palette(value, &[])?
        };
        let mode = self.mode_identity()?;
        let result = match value {
            PutPixelValue::Integer(value) => self.putpixel_integer_value(x, y, value, mode, route),
            PutPixelValue::Float(value) => self.putpixel_float_value(x, y, value, mode, route),
            // Invalid numeric sequences use Pillow's tuple-arity diagnostic.
            PutPixelValue::Invalid => Err(invalid_putpixel_value_error(mode)),
            PutPixelValue::Components(values) => {
                self.putpixel_components_value(x, y, values, mode, route)
            }
            PutPixelValue::FloatComponents(values) => {
                self.putpixel_float_components_value(x, y, values, mode, route)
            }
        };
        if result.is_ok()
            && let Some((_, context)) = route
        {
            *self = self.clone().with_execution_context(context);
        }
        result
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn putpixel_integer_value(
        &mut self,
        x: u32,
        y: u32,
        value: i64,
        mode: Mode,
        route: Option<PutPixelExecutionRoute>,
    ) -> Result<(), PilError> {
        if is_l16_identity(mode) {
            return self.putpixel_l16_with_route(x, y, value, route);
        }
        if mode == Mode::ImageI32 {
            // getink narrows the signed integer before storage. Going through
            // f64 first loses low bits and saturates on cast.
            return self.putpixel_mode_scalar_for_mode(x, y, f64::from(value as i32), mode, route);
        }
        if mode == Mode::ImageF32 {
            return self.putpixel_mode_scalar_for_mode(x, y, value as f64, mode, route);
        }
        if matches!(mode, Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8) {
            return self.putpixel_mode_for_mode(x, y, value.clamp(0, 255) as u8, mode);
        }
        // Multiband integer ink is ABGR, including alpha in bits 24..31 for
        // two-band images. A singleton tuple takes this path without clipping.
        let bytes = value.to_le_bytes();
        self.putpixel(x, y, bytes[0], bytes[1], bytes[2], bytes[3])
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn putpixel_float_value(
        &mut self,
        x: u32,
        y: u32,
        value: f64,
        mode: Mode,
        route: Option<PutPixelExecutionRoute>,
    ) -> Result<(), PilError> {
        if mode == Mode::ImageF32 {
            return self.putpixel_mode_scalar_for_mode(x, y, value, mode, route);
        }
        Err(if mode.is_single_character_pillow_mode() {
            PilError::TypeError("color must be int or single-element tuple".into())
        } else {
            PilError::TypeError("color must be int or tuple".into())
        })
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn putpixel_components_value(
        &mut self,
        x: u32,
        y: u32,
        values: Vec<i64>,
        mode: Mode,
        route: Option<PutPixelExecutionRoute>,
    ) -> Result<(), PilError> {
        if let [value] = values.as_slice() {
            return self.putpixel_integer_value(x, y, *value, mode, route);
        }
        let bands = mode.band_count();
        let palette_color = matches!(mode, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8)
            && matches!(values.len(), 3 | 4);
        if !palette_color
            && ((bands == 1)
                || (bands == 2 && values.len() != 2)
                || (bands >= 3 && !matches!(values.len(), 3 | 4)))
        {
            return Err(PilError::TypeError(
                match bands {
                    1 => "color must be int or single-element tuple",
                    2 => "color must be int, or tuple of one or two elements",
                    _ => "color must be int, or tuple of one, three or four elements",
                }
                .into(),
            ));
        }
        let bytes: Vec<u8> = values
            .iter()
            .map(|&value| value.clamp(0, 255) as u8)
            .collect();
        match bytes.as_slice() {
            [value, alpha] => self.putpixel(x, y, *value, *value, *value, *alpha),
            [r, g, b] => self.putpixel(x, y, *r, *g, *b, 255),
            [r, g, b, a] => self.putpixel(x, y, *r, *g, *b, *a),
            _ => Err(PilError::InternalError(
                "validated putpixel arity changed".into(),
            )),
        }
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn putpixel_float_components_value(
        &mut self,
        x: u32,
        y: u32,
        values: Vec<f64>,
        mode: Mode,
        route: Option<PutPixelExecutionRoute>,
    ) -> Result<(), PilError> {
        if let [value] = values.as_slice()
            && mode == Mode::ImageF32
        {
            return self.putpixel_mode_scalar_for_mode(x, y, *value, mode, route);
        }
        Err(invalid_putpixel_value_error(mode))
    }
}

// VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
fn invalid_putpixel_value_error(mode: Mode) -> PilError {
    if mode.is_single_character_pillow_mode() {
        return PilError::TypeError("color must be int or single-element tuple".into());
    }
    PilError::TypeError("color must be int, or tuple of one, three or four elements".into())
}
