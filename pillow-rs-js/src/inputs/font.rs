use super::JsValue;
use crate::{err, js_float_array, js_integer, js_value_type_name};
use wasm_bindgen::JsCast;
pub(crate) fn js_font_text_input(value: &JsValue) -> pillow_rs::ImageFontTextInput {
    if value.is_instance_of::<js_sys::Uint8Array>() {
        return pillow_rs::ImageFontTextInput::Bytes(js_sys::Uint8Array::new(value).to_vec());
    }
    if let Some(text) = value.as_string() {
        return pillow_rs::ImageFontTextInput::Text(text);
    }
    pillow_rs::ImageFontTextInput::Text(value.as_string().unwrap_or_default())
}

pub(crate) fn js_pilfont_text_input(value: &JsValue) -> pillow_rs::PilFontTextInput {
    if value.is_instance_of::<js_sys::Uint8Array>() {
        return pillow_rs::PilFontTextInput::Bytes(js_sys::Uint8Array::new(value).to_vec());
    }
    if let Some(text) = value.as_string() {
        return pillow_rs::PilFontTextInput::Text(text);
    }
    pillow_rs::PilFontTextInput::Text(value.as_string().unwrap_or_default())
}

pub(crate) fn js_font_variation_name_input(
    value: &JsValue,
) -> pillow_rs::ImageFontVariationNameInput {
    if value.is_instance_of::<js_sys::Uint8Array>() {
        return pillow_rs::ImageFontVariationNameInput::Bytes(
            js_sys::Uint8Array::new(value).to_vec(),
        );
    }
    if let Some(name) = value.as_string() {
        return pillow_rs::ImageFontVariationNameInput::Text(name);
    }
    pillow_rs::ImageFontVariationNameInput::InvalidType(js_value_type_name(value))
}

pub(crate) fn js_font_variation_axes_input(
    value: &JsValue,
) -> pillow_rs::ImageFontVariationAxesInput {
    let Some(values) = js_float_array(value) else {
        return pillow_rs::ImageFontVariationAxesInput::Invalid;
    };
    pillow_rs::ImageFontVariationAxesInput::Values(
        values.into_iter().map(|value| value as f32).collect(),
    )
}

pub(crate) fn js_font_ink(value: &JsValue) -> Result<Option<i64>, JsValue> {
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }
    js_integer(value).map(Some).ok_or_else(|| {
        err(pillow_rs::PilError::TypeError(
            "ink must be an integer".to_owned(),
        ))
    })
}

pub(crate) fn js_font_features(value: &JsValue) -> (Option<Vec<String>>, bool) {
    if value.is_null() || value.is_undefined() {
        return (None, false);
    }
    if !value.is_array() {
        return (None, true);
    }
    let values = js_sys::Array::from(value)
        .iter()
        .map(|item| item.as_string())
        .collect::<Option<Vec<_>>>();
    values.map_or((None, true), |values| (Some(values), false))
}

pub(crate) fn js_font_start(value: &JsValue) -> (Option<(f64, f64)>, bool) {
    if value.is_null() || value.is_undefined() {
        return (None, false);
    }
    let Some(values) = js_float_array(value) else {
        return (None, true);
    };
    if values.len() != 2 {
        return (None, true);
    }
    (Some((values[0], values[1])), false)
}

#[derive(Default)]
pub(crate) struct JsFontOptionsInput<'a> {
    pub(crate) mode: Option<String>,
    pub(crate) direction: Option<String>,
    pub(crate) features: Option<&'a JsValue>,
    pub(crate) language: Option<String>,
    pub(crate) stroke_width: f64,
    pub(crate) anchor: Option<String>,
    pub(crate) ink: Option<i64>,
    pub(crate) start: Option<&'a JsValue>,
    pub(crate) embedded_color: bool,
    pub(crate) stroke_filled: bool,
    pub(crate) has_args: bool,
    pub(crate) has_kwargs: bool,
    pub(crate) anchor_invalid_length_error: bool,
}

pub(crate) fn js_font_options(input: JsFontOptionsInput<'_>) -> pillow_rs::ImageFontTextOptions {
    let default = JsValue::NULL;
    let (features, features_invalid) = js_font_features(input.features.unwrap_or(&default));
    let (start, start_invalid) = js_font_start(input.start.unwrap_or(&default));
    pillow_rs::ImageFontTextOptions {
        mode: input.mode,
        embedded_color: input.embedded_color,
        direction: input.direction,
        features,
        features_invalid,
        language: input.language,
        stroke_width: input.stroke_width as f32,
        stroke_fill: None,
        text_fill_is_integer: false,
        stroke_fill_is_integer: false,
        align: None,
        stroke_filled: input.stroke_filled,
        anchor: input.anchor,
        anchor_invalid_length_error: input.anchor_invalid_length_error,
        start,
        start_invalid,
        ink: input.ink,
        has_args: input.has_args,
        has_kwargs: input.has_kwargs,
    }
}
