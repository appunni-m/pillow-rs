use crate::values::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::JsValue;

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

fn has_python_float_marker(value: &JsValue) -> bool {
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

pub(crate) fn js_python_dither(value: &JsValue) -> pillow_rs::PythonDitherInput {
    if value.is_null() || value.is_undefined() {
        return pillow_rs::PythonDitherInput::None;
    }
    // The Python facade receives a tuple for sequence-valued dither inputs.
    // Preserve that public type name at the host boundary so the shared
    // validator reports Pillow's tuple/int error instead of JavaScript's list.
    if value.is_array() {
        return pillow_rs::PythonDitherInput::Invalid("tuple".to_owned());
    }
    if let Some(value) = js_integer(value) {
        return u32::try_from(value)
            .map(pillow_rs::PythonDitherInput::Integer)
            .unwrap_or_else(|_| pillow_rs::PythonDitherInput::Invalid("int".to_owned()));
    }
    if let Some(value) = value.as_string() {
        return pillow_rs::PythonDitherInput::Name(value);
    }
    pillow_rs::PythonDitherInput::Invalid(js_value_type_name(value))
}

pub(crate) fn js_convert_mode(value: &JsValue) -> pillow_rs::PythonConvertModeInput {
    if value.is_null() || value.is_undefined() {
        return pillow_rs::PythonConvertModeInput::None;
    }
    value
        .as_string()
        .map(pillow_rs::PythonConvertModeInput::Name)
        .unwrap_or_else(|| pillow_rs::PythonConvertModeInput::Invalid(js_value_type_name(value)))
}

pub(crate) fn js_convert_palette(value: &JsValue) -> pillow_rs::PythonConvertPaletteInput {
    if value.is_null() || value.is_undefined() {
        return pillow_rs::PythonConvertPaletteInput::None;
    }
    value
        .as_string()
        .map(pillow_rs::PythonConvertPaletteInput::Name)
        .unwrap_or_else(|| pillow_rs::PythonConvertPaletteInput::Invalid(js_value_type_name(value)))
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

pub(crate) fn js_font_options(
    mode: Option<String>,
    direction: Option<String>,
    features: &JsValue,
    language: Option<String>,
    stroke_width: f64,
    anchor: Option<String>,
    ink: Option<i64>,
    start: &JsValue,
    embedded_color: bool,
    stroke_filled: bool,
    has_args: bool,
    has_kwargs: bool,
    anchor_invalid_length_error: bool,
) -> pillow_rs::ImageFontTextOptions {
    let (features, features_invalid) = js_font_features(features);
    let (start, start_invalid) = js_font_start(start);
    pillow_rs::ImageFontTextOptions {
        mode,
        embedded_color,
        direction,
        features,
        features_invalid,
        language,
        stroke_width: stroke_width as f32,
        stroke_fill: None,
        text_fill_is_integer: false,
        stroke_fill_is_integer: false,
        align: None,
        stroke_filled,
        anchor,
        anchor_invalid_length_error,
        start,
        start_invalid,
        ink,
        has_args,
        has_kwargs,
    }
}

pub(crate) fn js_reduce_factor(value: &JsValue) -> pillow_rs::ReduceFactor {
    if let Some(value) = js_integer(value) {
        return pillow_rs::ReduceFactor::Scalar(value);
    }
    if let Some(values) = js_integer_array(value) {
        return pillow_rs::ReduceFactor::Sequence(values);
    }
    if let Some(values) = js_float_array(value) {
        return pillow_rs::ReduceFactor::FloatingSequence(values);
    }
    pillow_rs::ReduceFactor::Invalid(js_value_type_name(value))
}

pub(crate) fn js_reduce_box(value: &JsValue) -> pillow_rs::ReduceBox {
    if let Some(values) = js_integer_array(value) {
        return pillow_rs::ReduceBox::Sequence(values);
    }
    pillow_rs::ReduceBox::InvalidType(js_value_type_name(value))
}

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

pub(crate) fn js_new_color_input(
    mode: &str,
    value: Option<&JsValue>,
) -> Result<pillow_rs::PythonNewColorInput, JsValue> {
    let Some(value) = value else {
        return Ok(pillow_rs::PythonNewColorInput::from_parts(
            None, None, None, None, None, None, None, false,
        ));
    };
    if let Some(text) = value.as_string() {
        return Ok(pillow_rs::PythonNewColorInput::from_parts(
            Some(text),
            None,
            None,
            None,
            None,
            None,
            None,
            true,
        ));
    }
    if let Some(number) = value.as_f64() {
        return js_new_color_number(mode, number);
    }
    if let Some(values) = js_float_array(value) {
        return js_new_color_sequence(values);
    }
    Err(err(pillow_rs::PilError::TypeError(
        "color must be a number or sequence".to_owned(),
    )))
}

fn js_new_color_number(mode: &str, number: f64) -> Result<pillow_rs::PythonNewColorInput, JsValue> {
    if !number.is_finite() {
        if mode == "F" {
            return Ok(pillow_rs::PythonNewColorInput::from_parts(
                None,
                None,
                None,
                None,
                None,
                None,
                Some(number),
                true,
            ));
        }
        return Err(err(pillow_rs::PilError::TypeError(
            "color must be a number or sequence".to_owned(),
        )));
    }
    let integer =
        (number.fract() == 0.0 && number >= f64::from(i32::MIN) && number <= f64::from(i32::MAX))
            .then_some(number as i32);
    let single = (number.fract() == 0.0 && (0.0..=255.0).contains(&number)).then_some(number as u8);
    Ok(pillow_rs::PythonNewColorInput::from_parts(
        None,
        single,
        None,
        None,
        None,
        integer,
        Some(number),
        true,
    ))
}

fn js_new_color_sequence(values: Vec<f64>) -> Result<pillow_rs::PythonNewColorInput, JsValue> {
    if !values
        .iter()
        .all(|number| number.is_finite() && number.fract() == 0.0)
    {
        return Err(err(pillow_rs::PilError::TypeError(
            "color sequence must contain integers".to_owned(),
        )));
    }
    let values = values
        .into_iter()
        .map(|number| (0.0..=255.0).contains(&number).then_some(number as u8))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| {
            err(pillow_rs::PilError::ValueError(
                "color components must be in range 0..255".to_owned(),
            ))
        })?;
    match values.as_slice() {
        [r, g, b] => Ok(pillow_rs::PythonNewColorInput::from_parts(
            None,
            None,
            Some((*r, *g, *b)),
            None,
            None,
            None,
            None,
            true,
        )),
        [r, g, b, a] => Ok(pillow_rs::PythonNewColorInput::from_parts(
            None,
            None,
            None,
            Some((*r, *g, *b, *a)),
            None,
            None,
            None,
            true,
        )),
        [luma, alpha] => Ok(pillow_rs::PythonNewColorInput::from_parts(
            None,
            None,
            None,
            None,
            Some((*luma, *alpha)),
            None,
            None,
            true,
        )),
        _ => Err(err(pillow_rs::PilError::TypeError(
            "color must be a sequence of length 2, 3, or 4".to_owned(),
        ))),
    }
}
