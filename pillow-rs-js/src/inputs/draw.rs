use super::JsValue;
use crate::{err, js_float_array, js_integer, js_integer_array};
pub(crate) fn js_draw_color_input(value: &JsValue) -> pillow_rs::DrawColorInput {
    if value.is_null() || value.is_undefined() {
        return pillow_rs::DrawColorInput::None;
    }
    if let Some(value) = value.as_string() {
        return pillow_rs::DrawColorInput::String(value);
    }
    if let Some(value) = js_integer(value) {
        return pillow_rs::DrawColorInput::Integer(value);
    }
    if let Some(value) = value.as_f64() {
        return pillow_rs::DrawColorInput::Float(value);
    }
    if let Some(values) = js_integer_array(value) {
        return pillow_rs::DrawColorInput::Components(values);
    }
    pillow_rs::DrawColorInput::Invalid
}

pub(crate) fn js_draw_points_input(value: &JsValue) -> pillow_rs::DrawPointsInput {
    if !value.is_array() {
        return pillow_rs::DrawPointsInput::Invalid;
    }
    let values = js_sys::Array::from(value).iter().collect::<Vec<_>>();
    if let Some(flat) = values.iter().map(js_integer).collect::<Option<Vec<_>>>() {
        return pillow_rs::DrawPointsInput::Flat(
            flat.into_iter().map(|value| value as i32).collect(),
        );
    }
    let nested = values
        .iter()
        .map(|value| {
            if !value.is_array() {
                return None;
            }
            js_sys::Array::from(value)
                .iter()
                .map(|value| js_integer(&value))
                .collect::<Option<Vec<_>>>()
                .map(|point| point.into_iter().map(|value| value as i32).collect())
        })
        .collect::<Option<Vec<Vec<i32>>>>();
    nested.map_or(
        pillow_rs::DrawPointsInput::InvalidSequence,
        pillow_rs::DrawPointsInput::Nested,
    )
}

pub(crate) fn js_draw_box_input(value: &JsValue) -> pillow_rs::DrawBoxInput {
    if !value.is_array() {
        return pillow_rs::DrawBoxInput::Invalid;
    }
    let values = js_sys::Array::from(value).iter().collect::<Vec<_>>();
    if values.iter().all(JsValue::is_array) {
        let nested = values
            .iter()
            .map(|value| {
                js_sys::Array::from(value)
                    .iter()
                    .map(|value| js_integer(&value))
                    .collect::<Option<Vec<_>>>()
                    .map(|point| point.into_iter().map(|value| value as i32).collect())
            })
            .collect::<Option<Vec<Vec<i32>>>>();
        return nested.map_or(
            pillow_rs::DrawBoxInput::Invalid,
            pillow_rs::DrawBoxInput::Nested,
        );
    }
    js_integer_array(value).map_or(pillow_rs::DrawBoxInput::Invalid, |values| {
        pillow_rs::DrawBoxInput::Flat(values.into_iter().map(|value| value as i32).collect())
    })
}

pub(crate) fn js_draw_circle_center_input(value: &JsValue) -> pillow_rs::DrawCircleCenterInput {
    if value.is_null() || value.is_undefined() {
        return pillow_rs::DrawCircleCenterInput::Invalid;
    }
    if let Some(value) = js_integer(value) {
        let _ = value;
        return pillow_rs::DrawCircleCenterInput::Integer;
    }
    if value.as_string().is_some() {
        return pillow_rs::DrawCircleCenterInput::Text;
    }
    if let Some(values) = js_float_array(value) {
        return pillow_rs::DrawCircleCenterInput::Values(values);
    }
    if value.is_object() {
        return pillow_rs::DrawCircleCenterInput::Mapping;
    }
    pillow_rs::DrawCircleCenterInput::Invalid
}

pub(crate) fn js_draw_optional_color(
    draw: &pillow_rs::Draw,
    value: &JsValue,
) -> Result<Option<(u8, u8, u8, u8)>, JsValue> {
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }
    draw.color_with_input(js_draw_color_input(value))
        .map(Some)
        .map_err(err)
}

pub(crate) fn js_draw_box_coordinates(value: &JsValue) -> Result<(i32, i32, i32, i32), JsValue> {
    pillow_rs::normalize_draw_box(js_draw_box_input(value)).map_err(err)
}
