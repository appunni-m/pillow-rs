use super::JsValue;
use crate::{err, js_float_array, js_integer, js_integer_array, js_value_type_name};
use wasm_bindgen::JsCast;
pub(crate) fn js_color3dlut_table(value: &JsValue) -> Result<pillow_rs::Color3DLutTable, JsValue> {
    if !value.is_array() {
        return Err(err(pillow_rs::PilError::TypeError(
            "Table must be a sequence of floats or a sequence of tuples of floats.".to_owned(),
        )));
    }
    let items = js_sys::Array::from(value).iter().collect::<Vec<_>>();
    if items.iter().all(JsValue::is_array) {
        let nested = items
            .into_iter()
            .map(|item| {
                js_float_array(&item).ok_or_else(|| {
                    err(pillow_rs::PilError::TypeError(
                        "Table must be a sequence of floats or a sequence of tuples of floats."
                            .to_owned(),
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(pillow_rs::Color3DLutTable::Nested(nested))
    } else {
        let flat = js_float_array(value).ok_or_else(|| {
            err(pillow_rs::PilError::TypeError(
                "Table must be a sequence of floats or a sequence of tuples of floats.".to_owned(),
            ))
        })?;
        Ok(pillow_rs::Color3DLutTable::Flat(flat))
    }
}

pub(crate) fn js_optional_resample(
    value: &JsValue,
) -> Result<Option<pillow_rs::ResampleInput>, JsValue> {
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }
    if let Some(code) = js_integer(value) {
        return Ok(Some(pillow_rs::ResampleInput::Code(code)));
    }
    if let Some(name) = value.as_string() {
        return Ok(Some(pillow_rs::ResampleInput::Name(name)));
    }
    Err(err(pillow_rs::PilError::TypeError(format!(
        "resample must be an integer or string, not {}",
        js_value_type_name(value)
    ))))
}

pub(crate) fn js_resize_box(value: &JsValue) -> Result<Option<(f64, f64, f64, f64)>, JsValue> {
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }
    let Some(values) = js_float_array(value) else {
        return Err(err(pillow_rs::PilError::TypeError(
            "box must be a 4-item sequence".to_owned(),
        )));
    };
    if values.len() != 4 {
        return Err(err(pillow_rs::PilError::TypeError(format!(
            "box must be a 4-item sequence, not {}",
            values.len()
        ))));
    }
    Ok(Some((values[0], values[1], values[2], values[3])))
}

pub(crate) fn js_optional_i32(value: &JsValue) -> Result<Option<i32>, JsValue> {
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }
    let Some(value) = js_integer(value) else {
        return Err(err(pillow_rs::PilError::TypeError(format!(
            "expected integer, not {}",
            js_value_type_name(value)
        ))));
    };
    Ok(Some(i32::try_from(value).map_err(|_| {
        err(pillow_rs::PilError::OverflowError(
            "integer argument is outside the supported range".to_owned(),
        ))
    })?))
}

pub(crate) fn js_chops_offset(value: &JsValue) -> Result<i32, JsValue> {
    if value.is_undefined() {
        return Ok(0);
    }
    if value.is_null() {
        return Err(chops_offset_type_error("NoneType"));
    }

    let integer = if let Some(boolean) = value.as_bool() {
        if boolean { 1 } else { 0 }
    } else if value.is_bigint() {
        let bigint = value.clone().unchecked_into::<js_sys::BigInt>();

        bigint
            .to_string(10)
            .ok()
            .and_then(|value| value.as_string())
            .and_then(|value| value.parse::<i64>().ok())
            .ok_or_else(chops_offset_c_long_overflow)?
    } else if has_python_float_marker(value) {
        return Err(chops_offset_type_error("float"));
    } else if let Some(number) = value.as_f64() {
        if !number.is_finite() || number.fract() != 0.0 {
            return Err(chops_offset_type_error("float"));
        }
        let positive_i64_limit = -(i64::MIN as f64);
        if number < i64::MIN as f64 || number >= positive_i64_limit {
            return Err(chops_offset_c_long_overflow());
        }
        number as i64
    } else {
        return Err(chops_offset_type_error(&js_value_type_name(value)));
    };

    i32::try_from(integer).map_err(|_| {
        let message = if integer < i64::from(i32::MIN) {
            "signed integer is less than minimum"
        } else {
            "signed integer is greater than maximum"
        };
        err(pillow_rs::PilError::OverflowError(message.to_owned()))
    })
}

pub(crate) fn has_python_float_marker(value: &JsValue) -> bool {
    js_sys::Reflect::has(value, &JsValue::from_str("__pillow_rs_python_float__")).unwrap_or(false)
}

fn chops_offset_type_error(type_name: &str) -> JsValue {
    err(pillow_rs::PilError::TypeError(format!(
        "'{type_name}' object cannot be interpreted as an integer"
    )))
}

fn chops_offset_c_long_overflow() -> JsValue {
    err(pillow_rs::PilError::OverflowError(
        "Python int too large to convert to C long".to_owned(),
    ))
}

pub(crate) fn js_dither_input(value: &JsValue) -> pillow_rs::DitherInput {
    if value.is_null() || value.is_undefined() {
        return pillow_rs::DitherInput::None;
    }
    // Map JavaScript arrays to Pillow's tuple error wording so the shared
    // validator preserves the same conversion contract as the Python binding.
    if value.is_array() {
        return pillow_rs::DitherInput::Invalid("tuple".to_owned());
    }
    if let Some(value) = js_integer(value) {
        return u32::try_from(value)
            .map(pillow_rs::DitherInput::Integer)
            .unwrap_or_else(|_| pillow_rs::DitherInput::Invalid("int".to_owned()));
    }
    if let Some(value) = value.as_string() {
        return pillow_rs::DitherInput::Name(value);
    }
    pillow_rs::DitherInput::Invalid(js_value_type_name(value))
}

pub(crate) fn js_convert_mode(value: &JsValue) -> pillow_rs::ConvertModeInput {
    if value.is_null() || value.is_undefined() {
        return pillow_rs::ConvertModeInput::None;
    }
    value
        .as_string()
        .map(pillow_rs::ConvertModeInput::Name)
        .unwrap_or_else(|| pillow_rs::ConvertModeInput::Invalid(js_value_type_name(value)))
}

pub(crate) fn js_convert_palette(value: &JsValue) -> pillow_rs::ConvertPaletteInput {
    if value.is_null() || value.is_undefined() {
        return pillow_rs::ConvertPaletteInput::None;
    }
    value
        .as_string()
        .map(pillow_rs::ConvertPaletteInput::Name)
        .unwrap_or_else(|| pillow_rs::ConvertPaletteInput::Invalid(js_value_type_name(value)))
}

pub(crate) fn js_optional_matrix(value: &JsValue) -> Result<Option<Vec<f64>>, JsValue> {
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }
    js_float_array(value).map(Some).ok_or_else(|| {
        err(pillow_rs::PilError::TypeError(
            "matrix must be a sequence of numbers".to_owned(),
        ))
    })
}

pub(crate) fn js_centering(value: &JsValue) -> pillow_rs::CenteringInput {
    if value.is_null() || value.is_undefined() {
        return pillow_rs::CenteringInput::Default;
    }
    if let Some(value) = value.as_f64() {
        return pillow_rs::CenteringInput::Scalar(value);
    }
    if let Some(values) = js_float_array(value) {
        return pillow_rs::CenteringInput::Values(values);
    }
    pillow_rs::CenteringInput::Invalid
}

pub(crate) fn js_imageops_color(value: &JsValue) -> pillow_rs::ImageOpsColor {
    if value.is_null() || value.is_undefined() {
        return pillow_rs::ImageOpsColor::None;
    }
    if let Some(value) = value.as_string() {
        return pillow_rs::ImageOpsColor::Name(value);
    }
    if let Some(value) = js_integer(value) {
        return pillow_rs::ImageOpsColor::Scalar(value);
    }
    if let Some(values) = js_integer_array(value) {
        return pillow_rs::ImageOpsColor::Components(values);
    }
    pillow_rs::ImageOpsColor::Invalid
}
