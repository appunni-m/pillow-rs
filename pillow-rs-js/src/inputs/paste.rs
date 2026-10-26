use super::JsValue;
use crate::{js_integer_array, js_value_type_name};
pub(crate) fn js_paste_source(value: &JsValue) -> pillow_rs::PythonPasteSource {
    if let Some(number) = value.as_f64() {
        if number.is_finite() && number.fract() == 0.0 {
            return pillow_rs::PythonPasteSource::Scalar(number as i64);
        }
        return pillow_rs::PythonPasteSource::Float(number);
    }
    if let Some(value) = value.as_string() {
        return pillow_rs::PythonPasteSource::String(value);
    }
    if let Some(values) = js_integer_array(value) {
        return pillow_rs::PythonPasteSource::Components(values);
    }
    pillow_rs::PythonPasteSource::Invalid
}

pub(crate) fn js_paste_box(value: Option<&JsValue>) -> pillow_rs::PythonPasteBox {
    let Some(value) = value else {
        return pillow_rs::PythonPasteBox::None;
    };
    if let Some(values) = js_integer_array(value) {
        return pillow_rs::PythonPasteBox::Values(values);
    }
    pillow_rs::PythonPasteBox::Invalid {
        length: if value.is_array() {
            Some(js_sys::Array::from(value).length() as usize)
        } else {
            None
        },
        type_name: js_value_type_name(value),
    }
}

pub(crate) fn js_paste_mask(value: Option<&JsValue>) -> pillow_rs::PythonPasteMask {
    let Some(value) = value else {
        return pillow_rs::PythonPasteMask::None;
    };
    pillow_rs::PythonPasteMask::Invalid(js_value_type_name(value))
}
