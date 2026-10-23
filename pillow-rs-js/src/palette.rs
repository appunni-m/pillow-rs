use crate::values::*;
use wasm_bindgen::prelude::{JsValue, wasm_bindgen};

fn image_palette_bytes(value: &JsValue) -> Vec<u8> {
    if !value.is_array() {
        return vec![];
    }
    js_sys::Array::from(value)
        .iter()
        .map(|item| {
            item.as_f64()
                .and_then(|value| u8::try_from(value as i64).ok())
        })
        .collect::<Option<Vec<_>>>()
        .unwrap_or_default()
}

fn image_palette_color_input(value: &JsValue) -> pillow_rs::PaletteColorInput {
    let repr = if let Some(number) = value.as_f64() {
        if number.is_finite() && number.fract() == 0.0 {
            (number as i64).to_string()
        } else {
            number.to_string()
        }
    } else if value.is_array() {
        "[]".to_string()
    } else {
        "object".to_string()
    };
    let components = value
        .is_array()
        .then(|| {
            js_sys::Array::from(value)
                .iter()
                .map(|item| {
                    let value = item.as_f64()?;
                    if value.is_finite() && value.fract() == 0.0 {
                        u8::try_from(value as i64).ok()
                    } else {
                        None
                    }
                })
                .collect::<Option<Vec<_>>>()
        })
        .flatten();
    components.map_or_else(
        || pillow_rs::PaletteColorInput::Invalid(repr),
        pillow_rs::PaletteColorInput::Components,
    )
}

#[wasm_bindgen]
pub struct ImagePalette {
    mode: String,
    data: Vec<u8>,
}
#[wasm_bindgen]
impl ImagePalette {
    #[wasm_bindgen(constructor)]
    pub fn new(mode: &str) -> ImagePalette {
        ImagePalette {
            mode: mode.to_string(),
            data: vec![],
        }
    }
    /// Construct a palette from the optional host-side constructor inputs.
    #[wasm_bindgen(js_name = "newWithInput")]
    pub fn new_with_input(mode: Option<String>, palette: JsValue) -> ImagePalette {
        ImagePalette {
            mode: mode.unwrap_or_else(|| "RGB".to_string()),
            data: image_palette_bytes(&palette),
        }
    }

    #[wasm_bindgen(getter)]
    pub fn mode(&self) -> String {
        self.mode.clone()
    }

    #[wasm_bindgen(js_name = "copy")]
    pub fn copy(&self) -> ImagePalette {
        ImagePalette {
            mode: self.mode.clone(),
            data: self.data.clone(),
        }
    }
    #[wasm_bindgen(js_name = "tobytes")]
    pub fn tobytes(&self) -> Vec<u8> {
        self.data.clone()
    }
    #[wasm_bindgen(js_name = "getdata")]
    pub fn getdata(&self) -> Vec<u8> {
        self.data.clone()
    }
    #[wasm_bindgen(js_name = "save")]
    pub fn save(&self, _fp: JsValue) {}

    #[wasm_bindgen(js_name = "getcolor")]
    pub fn getcolor(&mut self, color: JsValue, _image: JsValue) -> Result<usize, JsValue> {
        let input = image_palette_color_input(&color);
        pillow_rs::palette_getcolor_validate_input(&mut self.data, input, &self.mode)
            .map_err(|message| err(pillow_rs::PilError::ValueError(message)))
    }
}

// ── ImageStat ────────────────────────────────────────────────────
#[wasm_bindgen(js_name = "getColor")]
pub fn getcolor(color: &str, mode: &str) -> Result<JsValue, JsValue> {
    let (r, g, b, a) = pillow_rs::parse_color_str_unclamped(color).map_err(err)?;
    let value = pillow_rs::getcolor(r, g, b, a, mode).map_err(err)?;
    match value {
        pillow_rs::ColorValue::Gray(value) => Ok(JsValue::from(value)),
        pillow_rs::ColorValue::GrayAlpha(gray, alpha) => {
            let arr = js_sys::Array::new();
            arr.push(&JsValue::from(gray));
            arr.push(&JsValue::from(alpha));
            Ok(arr.into())
        }
        pillow_rs::ColorValue::Rgb(red, green, blue) => {
            let arr = js_sys::Array::new();
            arr.push(&JsValue::from(red));
            arr.push(&JsValue::from(green));
            arr.push(&JsValue::from(blue));
            Ok(arr.into())
        }
        pillow_rs::ColorValue::Rgba(red, green, blue, alpha) => {
            let arr = js_sys::Array::new();
            arr.push(&JsValue::from(red));
            arr.push(&JsValue::from(green));
            arr.push(&JsValue::from(blue));
            arr.push(&JsValue::from(alpha));
            Ok(arr.into())
        }
        pillow_rs::ColorValue::Hsv(hue, saturation, value) => {
            let arr = js_sys::Array::new();
            arr.push(&JsValue::from(hue));
            arr.push(&JsValue::from(saturation));
            arr.push(&JsValue::from(value));
            Ok(arr.into())
        }
    }
}

#[wasm_bindgen(js_name = "getRgb")]
pub fn getrgb(color: &str) -> Result<JsValue, JsValue> {
    let (red, green, blue, alpha) = pillow_rs::parse_color_str_unclamped(color).map_err(err)?;
    let arr = js_sys::Array::new();
    arr.push(&JsValue::from(red));
    arr.push(&JsValue::from(green));
    arr.push(&JsValue::from(blue));
    if pillow_rs::color_has_explicit_alpha(color) {
        arr.push(&JsValue::from(alpha));
    }
    Ok(arr.into())
}

// ══════════════════════════════════════════════════════════════════════════════
// ImagePalette — palette operations
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = "paletteGetColor")]
pub fn palette_getcolor(palette: Vec<u8>, r: u8, g: u8, b: u8) -> Option<usize> {
    pillow_rs::palette_getcolor(&palette, r, g, b)
}

#[wasm_bindgen(js_name = "paletteGetColorAppend")]
pub fn palette_getcolor_append(
    palette: Vec<u8>,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
    mode: &str,
) -> Result<usize, JsValue> {
    let mut pal = palette;
    pillow_rs::palette_getcolor_append(&mut pal, r, g, b, a, mode)
        .map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen(js_name = "paletteGetColorValidate")]
pub fn palette_getcolor_validate(
    palette: Vec<u8>,
    color: Vec<u8>,
    mode: &str,
) -> Result<usize, JsValue> {
    pillow_rs::palette_getcolor_validate(&mut palette.clone(), &color, mode)
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

#[wasm_bindgen(js_name = "paletteToText")]
pub fn palette_to_text(palette: Vec<u8>, mode: &str) -> String {
    pillow_rs::palette_to_text(&palette, mode)
}

// ══════════════════════════════════════════════════════════════════════════════
// ImageStat — statistics
// ══════════════════════════════════════════════════════════════════════════════
