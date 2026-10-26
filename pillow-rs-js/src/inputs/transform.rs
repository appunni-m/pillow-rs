use super::JsValue;
use crate::{
    err, has_python_float_marker, js_float_array, js_integer, js_integer_array, js_value_type_name,
};
use wasm_bindgen::JsCast;
fn transform_palette_fill_type_error() -> JsValue {
    err(pillow_rs::PilError::TypeError(
        "color must be int or single-element tuple".to_owned(),
    ))
}

fn transform_palette_integer(value: &JsValue) -> Result<i64, JsValue> {
    if let Some(boolean) = value.as_bool() {
        return Ok(i64::from(boolean));
    }
    if value.is_bigint() {
        let bigint = value.clone().unchecked_into::<js_sys::BigInt>();
        return bigint
            .to_string(10)
            .ok()
            .and_then(|value| value.as_string())
            .and_then(|value| value.parse::<i64>().ok())
            .ok_or_else(|| {
                err(pillow_rs::PilError::OverflowError(
                    "int too big to convert".to_owned(),
                ))
            });
    }
    if has_python_float_marker(value) {
        return Err(transform_palette_fill_type_error());
    }
    if let Some(number) = value.as_f64() {
        let positive_i64_limit = -(i64::MIN as f64);
        if !number.is_finite()
            || number.fract() != 0.0
            || number < i64::MIN as f64
            || number >= positive_i64_limit
        {
            return Err(transform_palette_fill_type_error());
        }
        return Ok(number as i64);
    }
    Err(transform_palette_fill_type_error())
}

fn transform_palette_fill(value: &JsValue) -> Result<pillow_rs::TransformFill, JsValue> {
    if value.as_bool().is_some() || value.is_bigint() {
        return transform_palette_integer(value).map(pillow_rs::TransformFill::Scalar);
    }
    if value.as_f64().is_some() || has_python_float_marker(value) {
        return transform_palette_integer(value).map(pillow_rs::TransformFill::Scalar);
    }
    if let Some(value) = value.as_string() {
        return Ok(pillow_rs::TransformFill::Name(value));
    }
    if value.is_array() {
        let values = js_sys::Array::from(value).iter().collect::<Vec<_>>();
        let kind = js_sys::Reflect::get(value, &JsValue::from_str("__pillow_rs_python_type__"))
            .ok()
            .and_then(|value| value.as_string())
            .unwrap_or_else(|| "tuple".to_owned());
        if values.len() == 1 && kind != "list" {
            return transform_palette_integer(&values[0]).map(pillow_rs::TransformFill::Scalar);
        }
        if !matches!(values.len(), 3 | 4) {
            return Err(transform_palette_fill_type_error());
        }
        if values.iter().any(|value| {
            value.as_bool().is_none()
                && !value.is_bigint()
                && !value
                    .as_f64()
                    .is_some_and(|number| number.is_finite() && number.fract() == 0.0)
        }) {
            return Err(transform_palette_fill_type_error());
        }
        if values.len() == 4 {
            let alpha = transform_palette_integer(&values[3]).ok();
            if alpha != Some(i64::from(u8::MAX)) {
                return Err(err(pillow_rs::PilError::ValueError(
                    "cannot add non-opaque RGBA color to RGB palette".to_owned(),
                )));
            }
        }
        let mut components = Vec::with_capacity(values.len());
        for value in values.iter().take(3) {
            components.push(transform_palette_integer(value).map_err(|_| {
                err(pillow_rs::PilError::ValueError(
                    "bytes must be in range(0, 256)".to_owned(),
                ))
            })?);
        }
        if values.len() == 4 {
            components.push(i64::from(u8::MAX));
        }
        return Ok(pillow_rs::TransformFill::Components(components));
    }
    Err(transform_palette_fill_type_error())
}

fn transform_float_array(value: &JsValue) -> Option<Vec<f64>> {
    if !value.is_array() {
        return None;
    }
    js_sys::Array::from(value)
        .iter()
        .map(|item| {
            if has_python_float_marker(&item) {
                js_sys::Reflect::get(&item, &JsValue::from_str("__pillow_rs_python_float__"))
                    .ok()?
                    .as_f64()
            } else {
                item.as_f64()
            }
        })
        .collect()
}

pub(crate) fn js_transform_fill(
    value: &JsValue,
    mode: &str,
) -> Result<Option<pillow_rs::TransformFill>, JsValue> {
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }
    if mode == "P" {
        return transform_palette_fill(value).map(Some);
    }
    if let Some(boolean) = value.as_bool() {
        return Ok(Some(pillow_rs::TransformFill::Scalar(i64::from(boolean))));
    }
    if value.is_bigint() {
        let bigint = value.clone().unchecked_into::<js_sys::BigInt>();
        if let Some(integer) = bigint
            .to_string(10)
            .ok()
            .and_then(|value| value.as_string())
            .and_then(|value| value.parse::<i64>().ok())
        {
            return Ok(Some(pillow_rs::TransformFill::Scalar(integer)));
        }
        return Ok(Some(pillow_rs::TransformFill::Invalid));
    }
    if let Some(value) = js_integer(value) {
        return Ok(Some(pillow_rs::TransformFill::Scalar(value)));
    }
    if let Some(value) = value.as_string() {
        return Ok(Some(pillow_rs::TransformFill::Name(value)));
    }
    if has_python_float_marker(value) {
        let number = js_sys::Reflect::get(value, &JsValue::from_str("__pillow_rs_python_float__"))?
            .as_f64()
            .ok_or_else(|| {
                err(pillow_rs::PilError::TypeError(
                    "color must be int or tuple".into(),
                ))
            })?;
        return Ok(Some(pillow_rs::TransformFill::FloatingScalar(number)));
    }
    if let Some(value) = value.as_f64() {
        return Ok(Some(pillow_rs::TransformFill::FloatingScalar(value)));
    }
    if let Some(values) = js_integer_array(value) {
        return Ok(Some(pillow_rs::TransformFill::Components(values)));
    }
    if let Some(values) = transform_float_array(value) {
        return Ok(Some(pillow_rs::TransformFill::FloatingComponents(values)));
    }
    if value.is_instance_of::<js_sys::Uint8Array>() {
        let values = js_sys::Uint8Array::new(value)
            .to_vec()
            .into_iter()
            .map(i64::from)
            .collect();
        return Ok(Some(pillow_rs::TransformFill::Components(values)));
    }
    Ok(Some(pillow_rs::TransformFill::Invalid))
}

pub(crate) fn js_transform_data(
    value: &JsValue,
) -> Result<Option<pillow_rs::TransformData>, JsValue> {
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }
    if let Some(values) = js_float_array(value) {
        return Ok(Some(pillow_rs::TransformData::Affine(values)));
    }
    if value.is_array() {
        let outer = js_sys::Array::from(value);
        let mut mesh = Vec::with_capacity(outer.length() as usize);
        let mut all_pairs = true;
        for item in outer.iter() {
            if !item.is_array() {
                all_pairs = false;
                break;
            }
            let pair = js_sys::Array::from(&item);
            if pair.length() != 2 {
                all_pairs = false;
                break;
            }
            let Some(bbox) = js_float_array(&pair.get(0)) else {
                all_pairs = false;
                break;
            };
            let Some(quad) = js_float_array(&pair.get(1)) else {
                all_pairs = false;
                break;
            };
            mesh.push((bbox, quad));
        }
        if all_pairs {
            return Ok(Some(pillow_rs::TransformData::Mesh(mesh)));
        }
        return Ok(Some(pillow_rs::TransformData::RawMesh(
            outer
                .iter()
                .map(|item| {
                    if !item.is_array() {
                        return Vec::new();
                    }
                    js_sys::Array::from(&item)
                        .iter()
                        .map(|part| js_float_array(&part).unwrap_or_default())
                        .collect()
                })
                .collect(),
        )));
    }
    if let Some(value) = value.as_string() {
        return Ok(Some(pillow_rs::TransformData::Text(value)));
    }
    if value.is_object() {
        return Ok(Some(pillow_rs::TransformData::Mapping));
    }
    Ok(Some(pillow_rs::TransformData::Invalid(js_value_type_name(
        value,
    ))))
}
