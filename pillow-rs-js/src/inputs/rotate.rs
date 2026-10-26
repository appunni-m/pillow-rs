use super::JsValue;
use crate::{js_float_array, js_integer, js_value_type_name};
pub(crate) fn js_rotate_resample(value: &JsValue) -> pillow_rs::RotateResampleInput {
    if value.is_null() || value.is_undefined() {
        return pillow_rs::RotateResampleInput::None;
    }
    if let Some(code) = js_integer(value) {
        return pillow_rs::RotateResampleInput::Code(code);
    }
    if let Some(name) = value.as_string() {
        return pillow_rs::RotateResampleInput::Name(name);
    }
    pillow_rs::RotateResampleInput::Name(js_value_type_name(value))
}

pub(crate) fn js_rotate_expand(value: &JsValue) -> pillow_rs::RotateExpandInput {
    if value.is_null() || value.is_undefined() {
        return pillow_rs::RotateExpandInput::Boolean(false);
    }
    if let Some(value) = value.as_bool() {
        return pillow_rs::RotateExpandInput::Boolean(value);
    }
    if let Some(value) = js_integer(value) {
        return pillow_rs::RotateExpandInput::Integer(value);
    }
    // The parity corpus uses JSON values, so this is the JavaScript truth
    // value for a non-boolean/non-integer expand argument. Empty arrays and
    // objects are truthy in JavaScript, just as their decoded host values are
    // intentionally kept as non-scalar inputs here.
    pillow_rs::RotateExpandInput::Boolean(true)
}

pub(crate) fn js_rotate_point(value: &JsValue) -> pillow_rs::RotatePointInput {
    if value.is_null() || value.is_undefined() {
        return pillow_rs::RotatePointInput::Default;
    }
    if let Some(values) = js_float_array(value) {
        return pillow_rs::RotatePointInput::Values(values);
    }
    pillow_rs::RotatePointInput::Invalid {
        type_name: js_value_type_name(value),
        truthy: true,
    }
}
