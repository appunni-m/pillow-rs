use super::JsValue;
use crate::{err, js_float_array};
pub(crate) fn js_color_triplet(value: &JsValue) -> Result<(u8, u8, u8), JsValue> {
    let values = if let Some(text) = value.as_string() {
        let (red, green, blue, _) = pillow_rs::parse_color_str_unclamped(&text).map_err(err)?;
        vec![red, green, blue]
    } else if let Some(values) = js_float_array(value) {
        values
            .into_iter()
            .map(|value| value as i32)
            .collect::<Vec<_>>()
    } else {
        return Err(err(pillow_rs::PilError::TypeError(
            "color must be a string or sequence".to_owned(),
        )));
    };
    if values.len() != 3 && values.len() != 4 {
        return Err(err(pillow_rs::PilError::TypeError(
            "color must be a 3- or 4-item sequence".to_owned(),
        )));
    }
    Ok((
        values[0].clamp(0, 255) as u8,
        values[1].clamp(0, 255) as u8,
        values[2].clamp(0, 255) as u8,
    ))
}

pub(crate) fn js_putpixel_value(value: &JsValue) -> pillow_rs::PutPixelValue {
    if let Ok(components) = js_sys::Reflect::get(
        value,
        &JsValue::from_str("__pillow_rs_putpixel_float_components__"),
    ) && components.is_array()
    {
        let values = js_sys::Array::from(&components)
            .iter()
            .map(|item| item.as_f64())
            .collect::<Option<Vec<_>>>();
        if let Some(values) = values {
            return pillow_rs::PutPixelValue::FloatComponents(values);
        }
    }
    if let Some(number) = value.as_f64() {
        if number.is_finite() && number.fract() == 0.0 {
            return pillow_rs::PutPixelValue::Integer(number as i64);
        }
        return pillow_rs::PutPixelValue::Float(number);
    }
    if value.is_array() {
        let values = js_sys::Array::from(value)
            .iter()
            .map(|item| item.as_f64())
            .collect::<Option<Vec<_>>>();
        if let Some(values) = values {
            if values
                .iter()
                .all(|item| item.is_finite() && item.fract() == 0.0)
            {
                return pillow_rs::PutPixelValue::Components(
                    values.into_iter().map(|item| item as i64).collect(),
                );
            }
            return pillow_rs::PutPixelValue::FloatComponents(values);
        }
    }
    pillow_rs::PutPixelValue::Invalid
}
