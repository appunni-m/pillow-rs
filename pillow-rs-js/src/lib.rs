// AS PER DESIGN — DO NOT REMOVE: Deferred lint cleanup. See CODEBASE_AUDIT.md Fix 2.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_in_result)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::redundant_clone)]
// WASM binding conventions differ from standard Rust naming
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unused_variables)]

//! pillow-rs WASM — full Pillow API for the browser. Thin delegation to pillow-rs.
use pillow_rs::Image as RsImage;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

fn err(e: pillow_rs::PilError) -> JsValue {
    let name = match &e {
        pillow_rs::PilError::IOError(_)
        | pillow_rs::PilError::OsError(_)
        | pillow_rs::PilError::Io(_) => "OSError",
        pillow_rs::PilError::AssertionError(_) => "AssertionError",
        pillow_rs::PilError::IndexError(_) => "IndexError",
        pillow_rs::PilError::AttributeError(_) => "AttributeError",
        pillow_rs::PilError::EOFError(_) => "EOFError",
        pillow_rs::PilError::KeyError(_)
        | pillow_rs::PilError::KeyErrorInt(_)
        | pillow_rs::PilError::UnsupportedLibraqm => "KeyError",
        pillow_rs::PilError::ValueError(_) => "ValueError",
        pillow_rs::PilError::OverflowError(_) => "OverflowError",
        pillow_rs::PilError::DecompressionBombError(_) => "DecompressionBombError",
        pillow_rs::PilError::UnicodeEncodeError { .. } => "UnicodeEncodeError",
        pillow_rs::PilError::ZeroDivisionError(_) => "ZeroDivisionError",
        pillow_rs::PilError::TypeError(_) => "TypeError",
        pillow_rs::PilError::SystemError(_) => "SystemError",
        pillow_rs::PilError::SyntaxError(_) => "SyntaxError",
        pillow_rs::PilError::NotImplementedError(_) => "NotImplementedError",
        pillow_rs::PilError::UnidentifiedImageError(_) => "UnidentifiedImageError",
        _ => "Error",
    };
    let message = match &e {
        // Python's ``str(KeyError("name"))`` includes the repr of a string
        // key.  JavaScript Error does not add that formatting for us.
        pillow_rs::PilError::KeyError(message) => format!("'{message}'"),
        _ => e.to_string(),
    };
    let error = js_sys::Error::new(&message);
    error.set_name(name);
    error.into()
}

fn js_optional_string(value: &JsValue, name: &str) -> Result<Option<String>, JsValue> {
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }
    value.as_string().map(Some).ok_or_else(|| {
        err(pillow_rs::PilError::TypeError(format!(
            "{name} must be a string or None"
        )))
    })
}

fn formatted_pixel_to_js(value: pillow_rs::FormattedPixelValue) -> JsValue {
    match value {
        pillow_rs::FormattedPixelValue::Scalar(value) => JsValue::from(value),
        pillow_rs::FormattedPixelValue::Integer(value) => JsValue::from(value),
        pillow_rs::FormattedPixelValue::Float(value) => JsValue::from_f64(value),
        pillow_rs::FormattedPixelValue::Components(values) => {
            js_sys::Array::from_iter(values.into_iter().map(JsValue::from)).into()
        }
    }
}

fn formatted_image_data_to_js(value: pillow_rs::FormattedImageData) -> JsValue {
    let array = js_sys::Array::new();
    match value {
        pillow_rs::FormattedImageData::Scalars(values) => {
            for value in values {
                array.push(&JsValue::from(value));
            }
        }
        pillow_rs::FormattedImageData::IntegerScalars(values) => {
            for value in values {
                array.push(&JsValue::from(value));
            }
        }
        pillow_rs::FormattedImageData::FloatScalars(values) => {
            for value in values {
                array.push(&JsValue::from_f64(value));
            }
        }
        pillow_rs::FormattedImageData::Components(values) => {
            for components in values {
                let pixel = js_sys::Array::new();
                for value in components {
                    pixel.push(&JsValue::from(value));
                }
                array.push(&pixel);
            }
        }
    }
    array.into()
}

fn formatted_extrema_to_js(value: pillow_rs::FormattedExtrema) -> JsValue {
    match value {
        pillow_rs::FormattedExtrema::Empty => JsValue::NULL,
        pillow_rs::FormattedExtrema::EmptyMultiple(count) => {
            let array = js_sys::Array::new();
            for _ in 0..count {
                array.push(&JsValue::NULL);
            }
            array.into()
        }
        pillow_rs::FormattedExtrema::Single((minimum, maximum)) => {
            let pair = js_sys::Array::new();
            pair.push(&JsValue::from(minimum));
            pair.push(&JsValue::from(maximum));
            pair.into()
        }
        pillow_rs::FormattedExtrema::Multiple(values) => {
            let array = js_sys::Array::new();
            for (minimum, maximum) in values {
                let pair = js_sys::Array::new();
                pair.push(&JsValue::from(minimum));
                pair.push(&JsValue::from(maximum));
                array.push(&pair);
            }
            array.into()
        }
        pillow_rs::FormattedExtrema::Integer((minimum, maximum)) => {
            let pair = js_sys::Array::new();
            pair.push(&JsValue::from(minimum));
            pair.push(&JsValue::from(maximum));
            pair.into()
        }
        pillow_rs::FormattedExtrema::Float((minimum, maximum)) => {
            let pair = js_sys::Array::new();
            pair.push(&JsValue::from_f64(minimum));
            pair.push(&JsValue::from_f64(maximum));
            pair.into()
        }
    }
}

fn image_info_value_to_js(value: pillow_rs::ImageInfoValue) -> JsValue {
    match value {
        // `JsValue::from(i64)` is not a JavaScript number conversion in
        // wasm-bindgen; it produces an unusable value for this JSON-facing
        // path. Pillow's metadata integers are within the safe JS range, so
        // make the numeric conversion explicit.
        pillow_rs::ImageInfoValue::Integer(value) => JsValue::from_f64(value as f64),
        pillow_rs::ImageInfoValue::Float(value) => JsValue::from_f64(value),
        pillow_rs::ImageInfoValue::String(value) => JsValue::from_str(&value),
        pillow_rs::ImageInfoValue::Bytes(value) => {
            let object = js_sys::Object::new();
            let _ = js_sys::Reflect::set(
                &object,
                &JsValue::from_str("kind"),
                &JsValue::from_str("bytes"),
            );
            let _ = js_sys::Reflect::set(
                &object,
                &JsValue::from_str("encoding"),
                &JsValue::from_str("base64"),
            );
            let _ = js_sys::Reflect::set(
                &object,
                &JsValue::from_str("data"),
                &js_sys::Uint8Array::from(value.as_slice()),
            );
            object.into()
        }
        pillow_rs::ImageInfoValue::IntegerList(values) => js_sys::Array::from_iter(
            values
                .into_iter()
                .map(|value| JsValue::from_f64(value as f64)),
        )
        .into(),
        pillow_rs::ImageInfoValue::FloatList(values) => {
            js_sys::Array::from_iter(values.into_iter().map(JsValue::from_f64)).into()
        }
        pillow_rs::ImageInfoValue::IntegerTuple(values) => js_sys::Array::from_iter(
            values
                .into_iter()
                .map(|value| JsValue::from_f64(value as f64)),
        )
        .into(),
        pillow_rs::ImageInfoValue::Object(fields) => image_info_to_js(fields),
    }
}

fn image_info_to_js(fields: Vec<(String, pillow_rs::ImageInfoValue)>) -> JsValue {
    let object = js_sys::Object::new();
    for (key, value) in fields {
        let _ = js_sys::Reflect::set(
            &object,
            &JsValue::from_str(&key),
            &image_info_value_to_js(value),
        );
    }
    object.into()
}

fn js_value_type_name(value: &JsValue) -> String {
    if value.is_null() || value.is_undefined() {
        return "NoneType".to_string();
    }
    if value.is_array() {
        return "list".to_string();
    }
    if value.as_string().is_some() {
        return "str".to_string();
    }
    if value.as_f64().is_some() {
        return "int".to_string();
    }
    "object".to_string()
}

fn js_value_display(value: &JsValue) -> String {
    if let Some(number) = value.as_f64() {
        if number.fract() == 0.0 {
            return format!("{}", number as i64);
        }
        return format!("{number}");
    }
    if let Some(boolean) = value.as_bool() {
        return boolean.to_string();
    }
    if let Some(string) = value.as_string() {
        return string;
    }
    js_value_type_name(value)
}

fn js_integer(value: &JsValue) -> Option<i64> {
    let number = value.as_f64()?;
    if number.is_finite() && number.fract() == 0.0 {
        Some(number as i64)
    } else {
        None
    }
}

fn js_integer_array(value: &JsValue) -> Option<Vec<i64>> {
    if !value.is_array() {
        return None;
    }
    js_sys::Array::from(value)
        .iter()
        .map(|item| js_integer(&item))
        .collect()
}

fn js_float_array(value: &JsValue) -> Option<Vec<f64>> {
    if !value.is_array() {
        return None;
    }
    js_sys::Array::from(value)
        .iter()
        .map(|item| item.as_f64())
        .collect()
}

fn js_color3dlut_table(value: &JsValue) -> Result<pillow_rs::Color3DLutTable, JsValue> {
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

fn js_optional_resample(value: &JsValue) -> Result<Option<pillow_rs::ResampleInput>, JsValue> {
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

fn js_resize_box(value: &JsValue) -> Result<Option<(i32, i32, i32, i32)>, JsValue> {
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }
    let Some(values) = js_integer_array(value) else {
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
    let values = values
        .into_iter()
        .map(|value| {
            i32::try_from(value).map_err(|_| {
                err(pillow_rs::PilError::OverflowError(
                    "signed integer is outside the image coordinate range".to_owned(),
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some((values[0], values[1], values[2], values[3])))
}

fn js_optional_i32(value: &JsValue) -> Result<Option<i32>, JsValue> {
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

fn js_python_dither(value: &JsValue) -> pillow_rs::PythonDitherInput {
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

fn js_convert_mode(value: &JsValue) -> pillow_rs::PythonConvertModeInput {
    if value.is_null() || value.is_undefined() {
        return pillow_rs::PythonConvertModeInput::None;
    }
    value
        .as_string()
        .map(pillow_rs::PythonConvertModeInput::Name)
        .unwrap_or_else(|| pillow_rs::PythonConvertModeInput::Invalid(js_value_type_name(value)))
}

fn js_convert_palette(value: &JsValue) -> pillow_rs::PythonConvertPaletteInput {
    if value.is_null() || value.is_undefined() {
        return pillow_rs::PythonConvertPaletteInput::None;
    }
    value
        .as_string()
        .map(pillow_rs::PythonConvertPaletteInput::Name)
        .unwrap_or_else(|| pillow_rs::PythonConvertPaletteInput::Invalid(js_value_type_name(value)))
}

fn js_optional_matrix(value: &JsValue) -> Result<Option<Vec<f64>>, JsValue> {
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }
    js_float_array(value).map(Some).ok_or_else(|| {
        err(pillow_rs::PilError::TypeError(
            "matrix must be a sequence of numbers".to_owned(),
        ))
    })
}

fn js_centering(value: &JsValue) -> pillow_rs::CenteringInput {
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

fn js_imageops_color(value: &JsValue) -> pillow_rs::ImageOpsColor {
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

fn js_draw_color_input(value: &JsValue) -> pillow_rs::DrawColorInput {
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

fn js_draw_points_input(value: &JsValue) -> pillow_rs::DrawPointsInput {
    if !value.is_array() {
        return pillow_rs::DrawPointsInput::Invalid;
    }
    let values = js_sys::Array::from(value).iter().collect::<Vec<_>>();
    if let Some(flat) = values
        .iter()
        .map(|value| js_integer(value))
        .collect::<Option<Vec<_>>>()
    {
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

fn js_draw_box_input(value: &JsValue) -> pillow_rs::DrawBoxInput {
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

fn js_draw_circle_center_input(value: &JsValue) -> pillow_rs::DrawCircleCenterInput {
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

fn js_draw_optional_color(
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

fn js_draw_box_coordinates(value: &JsValue) -> Result<(i32, i32, i32, i32), JsValue> {
    pillow_rs::normalize_draw_box(js_draw_box_input(value)).map_err(err)
}

fn js_font_text_input(value: &JsValue) -> pillow_rs::ImageFontTextInput {
    if value.is_instance_of::<js_sys::Uint8Array>() {
        return pillow_rs::ImageFontTextInput::Bytes(js_sys::Uint8Array::new(value).to_vec());
    }
    if let Some(text) = value.as_string() {
        return pillow_rs::ImageFontTextInput::Text(text);
    }
    pillow_rs::ImageFontTextInput::Text(value.as_string().unwrap_or_default())
}

fn js_pilfont_text_input(value: &JsValue) -> pillow_rs::PilFontTextInput {
    if value.is_instance_of::<js_sys::Uint8Array>() {
        return pillow_rs::PilFontTextInput::Bytes(js_sys::Uint8Array::new(value).to_vec());
    }
    if let Some(text) = value.as_string() {
        return pillow_rs::PilFontTextInput::Text(text);
    }
    pillow_rs::PilFontTextInput::Text(value.as_string().unwrap_or_default())
}

fn js_font_variation_name_input(value: &JsValue) -> pillow_rs::ImageFontVariationNameInput {
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

fn js_font_variation_axes_input(value: &JsValue) -> pillow_rs::ImageFontVariationAxesInput {
    let Some(values) = js_float_array(value) else {
        return pillow_rs::ImageFontVariationAxesInput::Invalid;
    };
    pillow_rs::ImageFontVariationAxesInput::Values(
        values.into_iter().map(|value| value as f32).collect(),
    )
}

fn js_font_ink(value: &JsValue) -> Result<Option<i64>, JsValue> {
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }
    js_integer(value).map(Some).ok_or_else(|| {
        err(pillow_rs::PilError::TypeError(
            "ink must be an integer".to_owned(),
        ))
    })
}

fn js_font_features(value: &JsValue) -> (Option<Vec<String>>, bool) {
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

fn js_font_start(value: &JsValue) -> (Option<(f64, f64)>, bool) {
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

fn js_font_options(
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

fn js_reduce_factor(value: &JsValue) -> pillow_rs::ReduceFactor {
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

fn js_reduce_box(value: &JsValue) -> pillow_rs::ReduceBox {
    if let Some(values) = js_integer_array(value) {
        return pillow_rs::ReduceBox::Sequence(values);
    }
    pillow_rs::ReduceBox::InvalidType(js_value_type_name(value))
}

fn js_transform_fill(value: &JsValue) -> Option<pillow_rs::TransformFill> {
    if value.is_null() || value.is_undefined() {
        return None;
    }
    if let Some(value) = js_integer(value) {
        return Some(pillow_rs::TransformFill::Scalar(value));
    }
    if let Some(value) = value.as_string() {
        return Some(pillow_rs::TransformFill::Name(value));
    }
    if let Some(values) = js_integer_array(value) {
        return Some(pillow_rs::TransformFill::Components(values));
    }
    if let Some(values) = js_float_array(value) {
        return Some(pillow_rs::TransformFill::FloatingComponents(values));
    }
    Some(pillow_rs::TransformFill::Invalid)
}

fn js_transform_data(value: &JsValue) -> Result<Option<pillow_rs::TransformData>, JsValue> {
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

fn js_rotate_resample(value: &JsValue) -> pillow_rs::RotateResampleInput {
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

fn js_rotate_expand(value: &JsValue) -> pillow_rs::RotateExpandInput {
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

fn js_rotate_point(value: &JsValue) -> pillow_rs::RotatePointInput {
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

fn js_color_triplet(value: &JsValue) -> Result<(u8, u8, u8), JsValue> {
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

fn js_putpixel_value(value: &JsValue) -> pillow_rs::PutPixelValue {
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

fn js_paste_source(value: &JsValue) -> pillow_rs::PythonPasteSource {
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

fn js_paste_box(value: Option<&JsValue>) -> pillow_rs::PythonPasteBox {
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

fn js_paste_mask(value: Option<&JsValue>) -> pillow_rs::PythonPasteMask {
    let Some(value) = value else {
        return pillow_rs::PythonPasteMask::None;
    };
    pillow_rs::PythonPasteMask::Invalid(js_value_type_name(value))
}

fn js_new_color_input(
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
        let integer = if number.fract() == 0.0
            && number >= f64::from(i32::MIN)
            && number <= f64::from(i32::MAX)
        {
            Some(number as i32)
        } else {
            None
        };
        let single = if number.fract() == 0.0 && (0.0..=255.0).contains(&number) {
            Some(number as u8)
        } else {
            None
        };
        return Ok(pillow_rs::PythonNewColorInput::from_parts(
            None,
            single,
            None,
            None,
            None,
            integer,
            Some(number),
            true,
        ));
    }
    if let Some(values) = js_float_array(value) {
        let integers = values
            .iter()
            .all(|number| number.is_finite() && number.fract() == 0.0);
        if !integers {
            return Err(err(pillow_rs::PilError::TypeError(
                "color sequence must contain integers".to_owned(),
            )));
        }
        let values = values
            .into_iter()
            .map(|number| {
                if !(0.0..=255.0).contains(&number) {
                    return None;
                }
                Some(number as u8)
            })
            .collect::<Option<Vec<_>>>();
        let Some(values) = values else {
            return Err(err(pillow_rs::PilError::ValueError(
                "color components must be in range 0..255".to_owned(),
            )));
        };
        let input = match values.as_slice() {
            [r, g, b] => pillow_rs::PythonNewColorInput::from_parts(
                None,
                None,
                Some((*r, *g, *b)),
                None,
                None,
                None,
                None,
                true,
            ),
            [r, g, b, a] => pillow_rs::PythonNewColorInput::from_parts(
                None,
                None,
                None,
                Some((*r, *g, *b, *a)),
                None,
                None,
                None,
                true,
            ),
            [luma, alpha] => pillow_rs::PythonNewColorInput::from_parts(
                None,
                None,
                None,
                None,
                Some((*luma, *alpha)),
                None,
                None,
                true,
            ),
            _ => {
                return Err(err(pillow_rs::PilError::TypeError(
                    "color must be a sequence of length 2, 3, or 4".to_owned(),
                )));
            }
        };
        return Ok(input);
    }
    Err(err(pillow_rs::PilError::TypeError(
        "color must be a number or sequence".to_owned(),
    )))
}

#[wasm_bindgen]
pub struct Image {
    inner: RsImage,
}

#[wasm_bindgen]
pub struct ArrayDescriptorLayout {
    mode: String,
    raw_mode: String,
    width: usize,
    height: usize,
    dimensions: usize,
    mode_reinterprets_dtype: bool,
}

#[wasm_bindgen]
impl ArrayDescriptorLayout {
    #[wasm_bindgen(getter)]
    pub fn mode(&self) -> String {
        self.mode.clone()
    }

    #[wasm_bindgen(getter, js_name = "rawMode")]
    pub fn raw_mode(&self) -> String {
        self.raw_mode.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn width(&self) -> usize {
        self.width
    }

    #[wasm_bindgen(getter)]
    pub fn height(&self) -> usize {
        self.height
    }

    #[wasm_bindgen(getter)]
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    #[wasm_bindgen(getter, js_name = "modeReinterpretsDtype")]
    pub fn mode_reinterprets_dtype(&self) -> bool {
        self.mode_reinterprets_dtype
    }
}

#[wasm_bindgen(js_name = "resolveArrayLayout")]
pub fn resolve_array_layout(
    shape: Vec<usize>,
    typestr: &str,
    mode: Option<String>,
) -> Result<ArrayDescriptorLayout, JsValue> {
    let layout = pillow_rs::resolve_array_layout(&shape, typestr, mode.as_deref()).map_err(err)?;
    Ok(ArrayDescriptorLayout {
        mode: layout.mode,
        raw_mode: layout.raw_mode,
        width: layout.width,
        height: layout.height,
        dimensions: layout.dimensions,
        mode_reinterprets_dtype: layout.mode_reinterprets_dtype,
    })
}

#[wasm_bindgen]
impl Image {
    #[wasm_bindgen(constructor)]
    pub fn new(mode: &str, w: u32, h: u32, r: u8, g: u8, b: u8, a: u8) -> Result<Image, JsValue> {
        #[cfg(feature = "debug-hooks")]
        console_error_panic_hook::set_once();
        // Initialize console_log with a conservative default (Warn).
        // Users can change the level at runtime via setLogLevel().
        #[cfg(feature = "debug-hooks")]
        console_log::init_with_level(log::Level::Warn).ok();
        RsImage::new(w, h, mode, (r, g, b, a))
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "open")]
    pub fn open(data: Vec<u8>) -> Result<Image, JsValue> {
        RsImage::open_bytes(data)
            .map(|i| Image { inner: i })
            .map_err(err)
    }

    // Properties
    #[wasm_bindgen(getter)]
    pub fn width(&mut self) -> Result<u32, JsValue> {
        self.inner.size().map(|(w, _)| w).map_err(err)
    }
    #[wasm_bindgen(getter)]
    pub fn height(&mut self) -> Result<u32, JsValue> {
        self.inner.size().map(|(_, h)| h).map_err(err)
    }
    #[wasm_bindgen(getter)]
    pub fn mode(&mut self) -> Result<String, JsValue> {
        self.inner.mode().map_err(err)
    }
    #[wasm_bindgen(getter)]
    pub fn format(&self) -> Option<String> {
        self.inner.format_name()
    }
    #[wasm_bindgen(js_name = "compatibilityInfo")]
    pub fn compatibility_info(&self) -> JsValue {
        image_info_to_js(self.inner.compatibility_info())
    }
    #[wasm_bindgen(js_name = "convertedCompatibilityInfo")]
    pub fn converted_compatibility_info(&self, target_mode: &str) -> JsValue {
        image_info_to_js(self.inner.converted_compatibility_info(target_mode))
    }
    #[wasm_bindgen(js_name = "replaceFrom")]
    pub fn replace_from(&mut self, other: &Image) {
        self.inner = other.inner.clone();
    }
    pub fn size(&mut self) -> Result<Vec<u32>, JsValue> {
        self.inner.size().map(|(w, h)| vec![w, h]).map_err(err)
    }

    // Transforms
    #[wasm_bindgen(js_name = "resize")]
    pub fn resize(&self, w: u32, h: u32, f: Option<String>) -> Result<Image, JsValue> {
        let filter = f.map(pillow_rs::ResampleInput::Name);
        self.inner
            .resize((i64::from(w), i64::from(h)), filter, None)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "resizeWithInput")]
    pub fn resize_with_input(
        &self,
        size: JsValue,
        resample: JsValue,
        box_coords: JsValue,
    ) -> Result<Image, JsValue> {
        let values = js_integer_array(&size).ok_or_else(|| {
            err(pillow_rs::PilError::TypeError(
                "size must be a two-item sequence".to_owned(),
            ))
        })?;
        if values.len() != 2 {
            return Err(err(pillow_rs::PilError::TypeError(format!(
                "size must be a two-item sequence, not {}",
                values.len()
            ))));
        }
        let filter = js_optional_resample(&resample)?;
        let box_coords = js_resize_box(&box_coords)?;
        self.inner
            .resize((values[0], values[1]), filter, box_coords)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "crop")]
    pub fn crop(&self, l: u32, t: u32, r: u32, b: u32) -> Result<Image, JsValue> {
        self.inner
            .crop_unsigned((l, t, r, b))
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "cropWithInput")]
    pub fn crop_with_input(&self, box_value: JsValue) -> Result<Image, JsValue> {
        let box_coords = if box_value.is_null() || box_value.is_undefined() {
            None
        } else {
            let values = js_float_array(&box_value).ok_or_else(|| {
                err(pillow_rs::PilError::TypeError(
                    "box must be a 4-item sequence".to_owned(),
                ))
            })?;
            if values.len() != 4 {
                return Err(err(pillow_rs::PilError::TypeError(format!(
                    "box must be a 4-item sequence, not {}",
                    values.len()
                ))));
            }
            Some((values[0], values[1], values[2], values[3]))
        };
        self.inner
            .crop_float(box_coords)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "rotate")]
    pub fn rotate(&self, a: f64) -> Result<Image, JsValue> {
        self.inner
            .rotate(a, false, None)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "rotateWithInput")]
    pub fn rotate_with_input(
        &self,
        angle: f64,
        resample: JsValue,
        expand: JsValue,
        center: JsValue,
        translate: JsValue,
        fillcolor: JsValue,
    ) -> Result<Image, JsValue> {
        self.inner
            .rotate_with_input(
                angle,
                js_rotate_resample(&resample),
                js_rotate_expand(&expand),
                js_rotate_point(&center),
                js_rotate_point(&translate),
                js_imageops_color(&fillcolor),
            )
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "transpose")]
    pub fn transpose(&self, m: &str) -> Result<Image, JsValue> {
        self.inner
            .transpose(m)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "convert")]
    pub fn convert(&self, m: &str, dither: Option<String>) -> Result<Image, JsValue> {
        self.inner
            .convert(m, None, dither.as_deref(), None, None)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "convertWithInput")]
    pub fn convert_with_input(
        &self,
        mode: JsValue,
        matrix: JsValue,
        dither: JsValue,
        palette: JsValue,
        colors: JsValue,
    ) -> Result<Image, JsValue> {
        let matrix = js_optional_matrix(&matrix)?;
        let colors = js_optional_i32(&colors)?.map(|value| {
            u32::try_from(value).map_err(|_| {
                err(pillow_rs::PilError::OverflowError(
                    "colors is outside the supported range".to_owned(),
                ))
            })
        });
        let colors = colors.transpose()?;
        self.inner
            .convert_with_input(
                js_convert_mode(&mode),
                matrix,
                js_python_dither(&dither),
                js_convert_palette(&palette),
                colors,
            )
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "filter")]
    pub fn filter(&self, n: &str) -> Result<Image, JsValue> {
        self.inner
            .filter(n)
            .map(|i| Image { inner: i })
            .map_err(err)
    }

    // Paste
    #[wasm_bindgen(js_name = "pasteImage")]
    pub fn paste_image(&mut self, src: &Image, x: i32, y: i32) -> Result<(), JsValue> {
        use pillow_rs::PasteSource;
        self.inner
            .paste_at(PasteSource::Image(src.inner.clone()), Some((x, y)), None)
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "pasteImageMasked")]
    pub fn paste_image_masked(
        &mut self,
        src: &Image,
        x: i32,
        y: i32,
        mask: &Image,
    ) -> Result<(), JsValue> {
        use pillow_rs::PasteSource;
        self.inner
            .paste_at(
                PasteSource::Image(src.inner.clone()),
                Some((x, y)),
                Some(&mask.inner),
            )
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "pasteImageRegion")]
    pub fn paste_image_region(
        &mut self,
        src: &Image,
        l: i32,
        t: i32,
        r: i32,
        b: i32,
    ) -> Result<(), JsValue> {
        use pillow_rs::PasteSource;
        self.inner
            .paste(
                PasteSource::Image(src.inner.clone()),
                Some((l, t, r, b)),
                None,
            )
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "pasteImageRegionMasked")]
    pub fn paste_image_region_masked(
        &mut self,
        src: &Image,
        l: i32,
        t: i32,
        r: i32,
        b: i32,
        mask: &Image,
    ) -> Result<(), JsValue> {
        use pillow_rs::PasteSource;
        self.inner
            .paste(
                PasteSource::Image(src.inner.clone()),
                Some((l, t, r, b)),
                Some(&mask.inner),
            )
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "pasteColor")]
    pub fn paste_color(
        &mut self,
        r: u8,
        g: u8,
        b: u8,
        a: u8,
        l: i32,
        t: i32,
        rt: i32,
        bt: i32,
    ) -> Result<(), JsValue> {
        use pillow_rs::PasteSource;
        self.inner
            .paste(PasteSource::Rgba(r, g, b, a), Some((l, t, rt, bt)), None)
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "pasteScalarRegion")]
    pub fn paste_scalar_region(
        &mut self,
        value: u8,
        l: i32,
        t: i32,
        r: i32,
        b: i32,
    ) -> Result<(), JsValue> {
        use pillow_rs::PasteSource;
        self.inner
            .paste(PasteSource::Scalar(value), Some((l, t, r, b)), None)
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "pasteScalarAt")]
    pub fn paste_scalar_at(&mut self, value: u8, x: i32, y: i32) -> Result<(), JsValue> {
        use pillow_rs::PasteSource;
        self.inner
            .paste_at(PasteSource::Scalar(value), Some((x, y)), None)
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "pasteLumaAlphaRegion")]
    pub fn paste_luma_alpha_region(
        &mut self,
        luma: u8,
        alpha: u8,
        l: i32,
        t: i32,
        r: i32,
        b: i32,
    ) -> Result<(), JsValue> {
        use pillow_rs::PasteSource;
        self.inner
            .paste(
                PasteSource::LumaAlpha(luma, alpha),
                Some((l, t, r, b)),
                None,
            )
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "pasteRgbAt")]
    pub fn paste_rgb_at(
        &mut self,
        r: u8,
        g: u8,
        b: u8,
        x: i32,
        y: i32,
        mask: &Image,
    ) -> Result<(), JsValue> {
        use pillow_rs::PasteSource;
        self.inner
            .paste_at(PasteSource::Rgb(r, g, b), Some((x, y)), Some(&mask.inner))
            .map_err(err)
    }

    // Pixels
    #[wasm_bindgen(js_name = "getpixel")]
    pub fn getpixel(&mut self, x: u32, y: u32) -> Result<Vec<u8>, JsValue> {
        self.inner
            .getpixel(x, y)
            .map(|(r, g, b, a)| vec![r, g, b, a])
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "getpixelFormatted")]
    pub fn getpixel_formatted(&self, x: u32, y: u32) -> Result<JsValue, JsValue> {
        self.inner
            .getpixel_formatted(x, y)
            .map(formatted_pixel_to_js)
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "putpixel")]
    pub fn putpixel(&mut self, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) -> Result<(), JsValue> {
        self.inner.putpixel(x, y, r, g, b, a).map_err(err)
    }
    #[wasm_bindgen(js_name = "putpixelValue")]
    pub fn putpixel_value(&mut self, x: u32, y: u32, value: JsValue) -> Result<(), JsValue> {
        self.inner
            .putpixel_value(x, y, js_putpixel_value(&value))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "point")]
    pub fn point(&self, lut: Vec<u8>) -> Result<Image, JsValue> {
        pillow_rs::image_eval_validated(&self.inner, &lut)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    /// Applies the floating-output form of Pillow's `Image.point` operation.
    ///
    /// The ordinary `point` export remains byte-oriented for compatibility.
    /// Keep the output mode explicit here because a `Float64Array` is needed
    /// to preserve fractional LUT values across the WASM boundary.
    #[wasm_bindgen(js_name = "pointWithMode")]
    pub fn point_with_mode(&self, lut: Vec<f64>, mode: &str) -> Result<Image, JsValue> {
        if mode != "F" {
            return Err(JsValue::from_str(
                "only Image.point output mode 'F' is supported by this export",
            ));
        }
        pillow_rs::image_eval_float(&self.inner, &lut)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "pointWithTransform")]
    pub fn point_with_transform(&self, scale: f64, offset: f64) -> Result<Image, JsValue> {
        pillow_rs::image_eval_point_transform(&self.inner, scale, offset)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "putalpha")]
    pub fn putalpha(&mut self, a: u8) -> Result<(), JsValue> {
        self.inner.putalpha(a).map_err(err)
    }
    #[wasm_bindgen(js_name = "putalphaInput")]
    pub fn putalpha_input(&mut self, alpha: JsValue) -> Result<(), JsValue> {
        let input = if let Some(value) = alpha.as_f64() {
            if !value.is_finite() || value.fract() != 0.0 {
                pillow_rs::PutAlphaInput::Invalid("float".to_owned())
            } else {
                pillow_rs::PutAlphaInput::Integer(value as i64)
            }
        } else {
            pillow_rs::PutAlphaInput::Invalid(js_value_type_name(&alpha))
        };
        self.inner.putalpha_with_input(input).map_err(err)
    }
    #[wasm_bindgen(js_name = "putalphaImageInput")]
    pub fn putalpha_image_input(&mut self, alpha: &Image) -> Result<(), JsValue> {
        self.inner
            .putalpha_with_input(pillow_rs::PutAlphaInput::Image(alpha.inner.clone()))
            .map_err(err)
    }

    // Bands
    #[wasm_bindgen(js_name = "split")]
    pub fn split(&self) -> Result<Vec<Image>, JsValue> {
        self.inner
            .split()
            .map(|v| v.into_iter().map(|i| Image { inner: i }).collect())
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "getbands")]
    pub fn getbands(&self) -> Result<Vec<String>, JsValue> {
        self.inner.getbands().map_err(err)
    }
    #[wasm_bindgen(js_name = "getchannel")]
    pub fn getchannel(&mut self, ch: i32) -> Result<Image, JsValue> {
        self.inner
            .getchannel(ch)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "alphaComposite")]
    pub fn alpha_composite(
        &mut self,
        src: &Image,
        dest: Option<JsValue>,
        source: Option<JsValue>,
    ) -> Result<(), JsValue> {
        let normalize_box = |value: Option<&JsValue>, default: Vec<i64>| match value {
            None => pillow_rs::AlphaCompositeBox::Values(default),
            Some(value) if value.is_null() || value.is_undefined() => {
                pillow_rs::AlphaCompositeBox::Values(default)
            }
            Some(value) => js_integer_array(value).map_or(
                pillow_rs::AlphaCompositeBox::Invalid,
                pillow_rs::AlphaCompositeBox::Values,
            ),
        };
        self.inner
            .alpha_composite_public(
                &src.inner,
                normalize_box(dest.as_ref(), vec![0, 0]),
                normalize_box(source.as_ref(), vec![0, 0]),
            )
            .map_err(err)
    }

    // Analysis
    #[wasm_bindgen(js_name = "getbbox")]
    pub fn getbbox(&self, a: Option<bool>) -> Result<JsValue, JsValue> {
        self.inner
            .getbbox(a.unwrap_or(true))
            .map(|r| match r {
                Some((left, top, right, bottom)) => {
                    let value = js_sys::Array::new();
                    value.push(&JsValue::from(left));
                    value.push(&JsValue::from(top));
                    value.push(&JsValue::from(right));
                    value.push(&JsValue::from(bottom));
                    value.into()
                }
                None => JsValue::NULL,
            })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "getextrema")]
    pub fn getextrema(&self) -> Result<js_sys::Array, JsValue> {
        let extrema = self.inner.getextrema().map_err(err)?;
        let arr = js_sys::Array::new();
        for (a, b) in &extrema {
            let pair = js_sys::Array::new();
            pair.push(&JsValue::from(*a));
            pair.push(&JsValue::from(*b));
            arr.push(&pair);
        }
        Ok(arr)
    }
    #[wasm_bindgen(js_name = "getextremaFormatted")]
    pub fn getextrema_formatted(&self) -> Result<JsValue, JsValue> {
        self.inner
            .getextrema_formatted()
            .map(formatted_extrema_to_js)
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "histogram")]
    pub fn histogram(&self) -> Result<Vec<u32>, JsValue> {
        self.inner.histogram().map_err(err)
    }
    #[wasm_bindgen(js_name = "histogramWithInput")]
    pub fn histogram_with_input(&self, mask: &Image) -> Result<Vec<u32>, JsValue> {
        self.inner
            .histogram_with_input(pillow_rs::ImageAnalysisMask::Image(mask.inner.clone()))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "histogramInvalidInput")]
    pub fn histogram_invalid_input(&self, type_name: &str) -> Result<Vec<u32>, JsValue> {
        self.inner
            .histogram_with_input(pillow_rs::ImageAnalysisMask::Invalid(type_name.to_owned()))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "entropy")]
    pub fn entropy(&mut self) -> Result<f64, JsValue> {
        self.inner.entropy().map_err(err)
    }
    #[wasm_bindgen(js_name = "entropyWithInput")]
    pub fn entropy_with_input(&mut self, mask: &Image) -> Result<f64, JsValue> {
        self.inner.entropy_with_mask(Some(&mask.inner)).map_err(err)
    }
    #[wasm_bindgen(js_name = "getcolors")]
    pub fn getcolors(&mut self, m: u32) -> Result<JsValue, JsValue> {
        match self.inner.getcolors(m).map_err(err)? {
            Some(colors) => {
                let arr = js_sys::Array::new();
                for (count, color) in &colors {
                    let entry = js_sys::Array::new();
                    entry.push(&JsValue::from(*count));
                    match color {
                        pillow_rs::FormattedPixelValue::Scalar(value) => {
                            entry.push(&JsValue::from(*value));
                        }
                        pillow_rs::FormattedPixelValue::Integer(value) => {
                            entry.push(&JsValue::from(*value));
                        }
                        pillow_rs::FormattedPixelValue::Float(value) => {
                            entry.push(&JsValue::from(*value));
                        }
                        pillow_rs::FormattedPixelValue::Components(values) => {
                            let color_arr = js_sys::Array::new();
                            for value in values {
                                color_arr.push(&JsValue::from(*value));
                            }
                            entry.push(&color_arr);
                        }
                    }
                    arr.push(&entry);
                }
                Ok(arr.into())
            }
            None => Ok(JsValue::null()),
        }
    }
    #[wasm_bindgen(js_name = "getdata")]
    pub fn getdata(&mut self, b: Option<i32>) -> Result<Vec<u8>, JsValue> {
        self.inner.getdata(b).map_err(err)
    }
    #[wasm_bindgen(js_name = "getdataFormatted")]
    pub fn getdata_formatted(&self, b: Option<i32>) -> Result<JsValue, JsValue> {
        self.inner
            .getdata_formatted(b)
            .map(formatted_image_data_to_js)
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "getprojection")]
    pub fn getprojection(&mut self) -> Result<js_sys::Array, JsValue> {
        let (h_proj, v_proj) = self.inner.getprojection().map_err(err)?;
        let h_arr = js_sys::Array::new();
        for val in &h_proj {
            h_arr.push(&JsValue::from(*val));
        }
        let v_arr = js_sys::Array::new();
        for val in &v_proj {
            v_arr.push(&JsValue::from(*val));
        }
        let result = js_sys::Array::new();
        result.push(&h_arr);
        result.push(&v_arr);
        Ok(result)
    }

    // Enhancement
    #[wasm_bindgen(js_name = "enhanceBrightness")]
    pub fn bright(&self, f: f64) -> Result<Image, JsValue> {
        self.inner
            .enhance_brightness(f)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "enhanceContrast")]
    pub fn contrast(&self, f: f64) -> Result<Image, JsValue> {
        self.inner
            .enhance_contrast(f)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "enhanceColor")]
    pub fn color(&self, f: f64) -> Result<Image, JsValue> {
        self.inner
            .enhance_color(f)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "enhanceSharpness")]
    pub fn sharp(&self, f: f64) -> Result<Image, JsValue> {
        self.inner
            .enhance_sharpness(f)
            .map(|i| Image { inner: i })
            .map_err(err)
    }

    // Filters
    #[wasm_bindgen(js_name = "gaussianBlur")]
    pub fn gaussian(&self, r: f32) -> Result<Image, JsValue> {
        self.inner
            .gaussian_blur(r)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "gaussianBlurXY")]
    pub fn gaussian_xy(&self, rx: f32, ry: f32) -> Result<Image, JsValue> {
        self.inner
            .gaussian_blur_xy(rx, ry)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "boxBlur")]
    pub fn boxb(&self, r: f32) -> Result<Image, JsValue> {
        self.inner
            .box_blur(r)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "boxBlurXY")]
    pub fn box_xy(&self, rx: f32, ry: f32) -> Result<Image, JsValue> {
        self.inner
            .box_blur_xy(rx, ry)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "unsharpMask")]
    pub fn unsharp(&self, r: f32, p: i32, t: u8) -> Result<Image, JsValue> {
        self.inner
            .unsharp_mask(r, p, t)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "maxFilter")]
    pub fn maxf(&self, s: u32) -> Result<Image, JsValue> {
        self.inner
            .max_filter(s)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "minFilter")]
    pub fn minf(&self, s: u32) -> Result<Image, JsValue> {
        self.inner
            .min_filter(s)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "medianFilter")]
    pub fn medianf(&self, s: u32) -> Result<Image, JsValue> {
        self.inner
            .median_filter(s)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "modeFilter")]
    pub fn modef(&self, s: u32) -> Result<Image, JsValue> {
        self.inner
            .mode_filter(s)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "rankFilter")]
    pub fn rankf(&self, s: u32, r: u32) -> Result<Image, JsValue> {
        self.inner
            .rank_filter(s, r)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "kernelFilter")]
    pub fn kernelf(
        &self,
        kernel: Vec<f32>,
        scale: f32,
        offset: i32,
        size: u32,
    ) -> Result<Image, JsValue> {
        let kernel = Some(kernel.into_iter().map(f64::from).collect());
        self.inner
            .kernel_filter(
                kernel,
                Some(f64::from(scale)),
                f64::from(offset),
                (size, size),
            )
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "color3DLUT")]
    pub fn color3dlut(
        &self,
        size_x: u32,
        size_y: u32,
        size_z: u32,
        table: Vec<f64>,
        channels: u32,
        target_mode: Option<String>,
    ) -> Result<Image, JsValue> {
        let input = pillow_rs::prepare_color3dlut(table, (size_x, size_y, size_z), channels)
            .map_err(err)?;
        self.inner
            .color3dlut(input, target_mode.as_deref())
            .map(|i| Image { inner: i })
            .map_err(err)
    }

    /// Transform a Color3DLUT table using a named parity-corpus callback.
    /// Table traversal, callback-result slice semantics, and final length
    /// validation remain in the shared Rust core; this binding only decodes
    /// the callback asset name used by the JS host.
    #[wasm_bindgen(js_name = "color3DLUTTransform")]
    pub fn color3dlut_transform(
        table: Vec<f64>,
        size_x: u32,
        size_y: u32,
        size_z: u32,
        channels_in: u32,
        channels_out: JsValue,
        with_normals: bool,
        callback: String,
    ) -> Result<Vec<f64>, JsValue> {
        let size = (size_x, size_y, size_z);
        let input = pillow_rs::prepare_color3dlut(table, size, channels_in).map_err(err)?;
        let channels_out = js_optional_i32(&channels_out)?
            .map(|value| {
                u32::try_from(value).map_err(|_| {
                    err(pillow_rs::PilError::ValueError(
                        "Only 3 or 4 output channels are supported".to_owned(),
                    ))
                })
            })
            .transpose()?;
        let output_channels = channels_out.unwrap_or(channels_in);
        let output = pillow_rs::color3dlut_transform_table(
            &input,
            channels_out,
            with_normals,
            |values| match callback.as_str() {
                "color3dlut-transform-identity" => {
                    Ok(values[values.len().saturating_sub(3)..].to_vec())
                }
                "color3dlut-transform-rgba" => {
                    let mut result = values[values.len().saturating_sub(3)..].to_vec();
                    result.push(1.0);
                    Ok(result)
                }
                "color3dlut-short-result" => Ok(values.iter().copied().take(2).collect()),
                _ => Err(err(pillow_rs::PilError::NotImplementedError(format!(
                    "unsupported Color3DLUT callback: {callback}"
                )))),
            },
            err,
        )?;
        pillow_rs::color3dlut_prepare_table(
            pillow_rs::Color3DLutTable::Flat(output.0),
            size,
            output_channels,
        )
        .map_err(err)
    }

    // Quantize/Reduce
    #[wasm_bindgen(js_name = "quantize")]
    pub fn quantize(&self, c: u32) -> Result<Image, JsValue> {
        self.inner
            .quantize(c, 0, None, true, 0)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "quantizeWithInput")]
    pub fn quantize_with_input(
        &self,
        colors: JsValue,
        method: JsValue,
        kmeans: JsValue,
        dither: JsValue,
        palette: JsValue,
    ) -> Result<Image, JsValue> {
        let colors = js_optional_i32(&colors)?;
        let method = js_optional_i32(&method)?;
        let kmeans = js_optional_i32(&kmeans)?;
        let dither = if dither.is_null() || dither.is_undefined() {
            None
        } else {
            Some(dither.as_bool().ok_or_else(|| {
                err(pillow_rs::PilError::TypeError(
                    "dither must be a boolean".to_owned(),
                ))
            })?)
        };
        let palette = if palette.is_null() || palette.is_undefined() {
            pillow_rs::QuantizePalette::None
        } else {
            pillow_rs::QuantizePalette::Other
        };
        self.inner
            .quantize_with_input(colors, method, kmeans, palette, dither)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "quantizeWithPaletteInput")]
    pub fn quantize_with_palette_input(
        &self,
        colors: JsValue,
        method: JsValue,
        kmeans: JsValue,
        dither: JsValue,
        palette: &Image,
    ) -> Result<Image, JsValue> {
        let colors = js_optional_i32(&colors)?;
        let method = js_optional_i32(&method)?;
        let kmeans = js_optional_i32(&kmeans)?;
        let dither = if dither.is_null() || dither.is_undefined() {
            None
        } else {
            Some(dither.as_bool().ok_or_else(|| {
                err(pillow_rs::PilError::TypeError(
                    "dither must be a boolean".to_owned(),
                ))
            })?)
        };
        self.inner
            .quantize_with_input(
                colors,
                method,
                kmeans,
                pillow_rs::QuantizePalette::Image(palette.inner.clone()),
                dither,
            )
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "reduce")]
    pub fn reduce(&self, f: u32) -> Result<Image, JsValue> {
        self.inner
            .reduce(f, f)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "reduceWithInput")]
    pub fn reduce_with_input(
        &self,
        factor: JsValue,
        box_coords: JsValue,
    ) -> Result<Image, JsValue> {
        let box_coords = if box_coords.is_null() || box_coords.is_undefined() {
            None
        } else {
            Some(js_reduce_box(&box_coords))
        };
        self.inner
            .reduce_public(js_reduce_factor(&factor), box_coords)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "remapPalette")]
    pub fn remap(&mut self, m: Vec<u8>) -> Result<Image, JsValue> {
        self.inner
            .remap_palette(&m)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "effectSpread")]
    pub fn spread(&self, d: u32) -> Result<Image, JsValue> {
        pillow_rs::image_effect_spread(&self.inner, d)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "effectNoise")]
    pub fn noise(&self, sigma: f64) -> Result<Image, JsValue> {
        pillow_rs::image_effect_noise(&self.inner, sigma)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "eval")]
    pub fn eval(&self, lut: Vec<u8>) -> Result<Image, JsValue> {
        pillow_rs::image_eval_replicated_for_image(&self.inner, &lut)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "thumbnail")]
    pub fn thumb(&mut self, w: u32, h: u32) -> Result<(), JsValue> {
        self.inner
            .thumbnail((i64::from(w), i64::from(h)), None)
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "thumbnailWithInput")]
    pub fn thumb_with_input(&mut self, size: JsValue, resample: JsValue) -> Result<(), JsValue> {
        let values = js_integer_array(&size).ok_or_else(|| {
            err(pillow_rs::PilError::TypeError(
                "size must be a two-item sequence".to_owned(),
            ))
        })?;
        if values.len() != 2 {
            return Err(err(pillow_rs::PilError::TypeError(format!(
                "size must be a two-item sequence, not {}",
                values.len()
            ))));
        }
        self.inner
            .thumbnail((values[0], values[1]), js_optional_resample(&resample)?)
            .map_err(err)
    }

    // Bookkeeping
    #[wasm_bindgen(js_name = "seek")]
    pub fn seek(&mut self, f: u32) -> Result<(), JsValue> {
        self.inner.seek(f).map_err(err)
    }
    #[wasm_bindgen(js_name = "tell")]
    pub fn tell_js(&self) -> u32 {
        self.inner.tell()
    }
    #[wasm_bindgen(js_name = "load")]
    pub fn load(&mut self) -> Result<(), JsValue> {
        self.inner.load().map_err(err)
    }
    #[wasm_bindgen(js_name = "verify")]
    pub fn verify(&self) -> Result<(), JsValue> {
        self.inner.verify().map_err(err)
    }
    #[wasm_bindgen(js_name = "fromBytes")]
    pub fn frombytes(&self, m: &str, w: u32, h: u32, d: Vec<u8>) -> Result<Image, JsValue> {
        RsImage::frombytes(m, (w, h), &d)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "fromBytesInPlace")]
    pub fn frombytes_in_place(
        &mut self,
        m: &str,
        w: u32,
        h: u32,
        d: Vec<u8>,
    ) -> Result<(), JsValue> {
        self.inner = RsImage::frombytes(m, (w, h), &d).map_err(err)?;
        Ok(())
    }
    #[wasm_bindgen(js_name = "pasteValue")]
    pub fn paste_value(
        &mut self,
        source: JsValue,
        box_value: Option<JsValue>,
        mask: Option<JsValue>,
    ) -> Result<(), JsValue> {
        self.inner
            .paste_with_input(
                js_paste_source(&source),
                js_paste_box(box_value.as_ref()),
                js_paste_mask(mask.as_ref()),
            )
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "pasteValueMasked")]
    pub fn paste_value_masked(
        &mut self,
        source: JsValue,
        box_value: Option<JsValue>,
        mask: &Image,
    ) -> Result<(), JsValue> {
        self.inner
            .paste_with_input(
                js_paste_source(&source),
                js_paste_box(box_value.as_ref()),
                pillow_rs::PythonPasteMask::Image(mask.inner.clone()),
            )
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "putdataValues")]
    pub fn putdata_values(
        &mut self,
        data: JsValue,
        scale: Option<f64>,
        offset: Option<f64>,
    ) -> Result<(), JsValue> {
        if data.is_instance_of::<js_sys::Uint8Array>() {
            let bytes = js_sys::Uint8Array::new(&data).to_vec();
            let scale = scale.unwrap_or(1.0);
            let offset = offset.unwrap_or(0.0);
            if self
                .inner
                .putdata_bytes_fast_path(&bytes, bytes.len(), scale, offset)
                .map_err(err)?
            {
                return Ok(());
            }
            let kind = self.inner.putdata_value_kind().map_err(err)?;
            let values = bytes
                .into_iter()
                .map(|value| match kind {
                    pillow_rs::PutDataValueKind::Numeric => {
                        pillow_rs::PutDataValue::Number(f64::from(value))
                    }
                    pillow_rs::PutDataValueKind::Components { .. } => {
                        pillow_rs::PutDataValue::Packed(i64::from(value))
                    }
                })
                .collect::<Vec<_>>();
            return self
                .inner
                .putdata_values(&values, scale, offset)
                .map_err(err);
        }
        if !data.is_array() {
            return Err(err(pillow_rs::PilError::TypeError(
                "argument must be a sequence".to_owned(),
            )));
        }
        let kind = self.inner.putdata_value_kind().map_err(err)?;
        let values = js_sys::Array::from(&data)
            .iter()
            .map(|value| {
                if let Some(number) = value.as_f64() {
                    return match kind {
                        pillow_rs::PutDataValueKind::Numeric => {
                            Ok(pillow_rs::PutDataValue::Number(number))
                        }
                        pillow_rs::PutDataValueKind::Components { .. }
                            if number.is_finite() && number.fract() == 0.0 =>
                        {
                            Ok(pillow_rs::PutDataValue::Packed(number as i64))
                        }
                        pillow_rs::PutDataValueKind::Components { .. } => {
                            Ok(pillow_rs::PutDataValue::Number(number))
                        }
                    };
                }
                if let Some(values) = js_integer_array(&value) {
                    if values.len() == 1 {
                        return Ok(pillow_rs::PutDataValue::Packed(i64::from(values[0])));
                    }
                    return Ok(pillow_rs::PutDataValue::Components(
                        values.into_iter().map(i128::from).collect(),
                    ));
                }
                if value.is_object() && !value.is_array() {
                    if matches!(
                        kind,
                        pillow_rs::PutDataValueKind::Components { channels: 2 }
                    ) && js_sys::Reflect::has(
                        &value,
                        &JsValue::from_str("__pillow_rs_putdata_custom_index__"),
                    )
                    .unwrap_or(false)
                    {
                        return Err(pillow_rs::PilError::TypeError(
                            "color must be int or tuple".to_owned(),
                        ));
                    }
                }
                if value.is_array() && js_sys::Array::from(&value).length() == 1 {
                    if matches!(
                        kind,
                        pillow_rs::PutDataValueKind::Components { channels: 2 }
                    ) {
                        return Err(pillow_rs::PilError::SystemError(
                            "new style getargs format but argument is not a tuple".to_owned(),
                        ));
                    }
                    return Err(pillow_rs::PilError::TypeError(
                        "color must be int, or tuple of one, three or four elements".to_owned(),
                    ));
                }
                Err(pillow_rs::PilError::TypeError(
                    "color must be int or tuple".to_owned(),
                ))
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        self.inner
            .putdata_values(&values, scale.unwrap_or(1.0), offset.unwrap_or(0.0))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "putdata")]
    pub fn putdata(&mut self, d: Vec<u8>) -> Result<(), JsValue> {
        self.inner.putdata(&d).map_err(err)
    }
    #[wasm_bindgen(js_name = "transform")]
    pub fn transform(&self, sz: Vec<u32>, d: Vec<f64>) -> Result<Image, JsValue> {
        self.inner
            .transform_affine((sz[0], sz[1]), &d, (0, 0, 0, 255))
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "transformWithInput")]
    pub fn transform_with_input(
        &self,
        size: JsValue,
        method: i32,
        data: JsValue,
        resample: i32,
        fill: i32,
        fillcolor: JsValue,
    ) -> Result<Image, JsValue> {
        let size = js_integer_array(&size).ok_or_else(|| {
            err(pillow_rs::PilError::TypeError(
                "size must be a two-item sequence".to_owned(),
            ))
        })?;
        if size.len() != 2 {
            return Err(err(pillow_rs::PilError::TypeError(format!(
                "size must be a two-item sequence, not {}",
                size.len()
            ))));
        }
        let size = (
            u32::try_from(size[0]).map_err(|_| {
                err(pillow_rs::PilError::ValueError(
                    "width and height must be >= 0".to_owned(),
                ))
            })?,
            u32::try_from(size[1]).map_err(|_| {
                err(pillow_rs::PilError::ValueError(
                    "width and height must be >= 0".to_owned(),
                ))
            })?,
        );
        self.inner
            .transform_public(
                size,
                method,
                js_transform_data(&data)?,
                resample,
                fill,
                js_transform_fill(&fillcolor),
            )
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "toBytes")]
    pub fn tobytes(&self) -> Result<Vec<u8>, JsValue> {
        self.inner.tobytes().map_err(err)
    }
    #[wasm_bindgen(js_name = "toBytesEncoded")]
    pub fn tobytes_encoded(
        &self,
        encoder_name: &str,
        args: Vec<String>,
    ) -> Result<Vec<u8>, JsValue> {
        let mode = self.inner.mode().map_err(err)?;
        self.inner
            .tobytes_encoded(&mode, encoder_name, &args)
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "copy")]
    pub fn copy(&self) -> Image {
        Image {
            inner: self.inner.copy(),
        }
    }
    #[wasm_bindgen(js_name = "tobitmap")]
    pub fn tobitmap(&mut self) -> Result<Vec<u8>, JsValue> {
        self.inner.tobitmap().map_err(err)
    }
    // More methods
    #[wasm_bindgen(js_name = "getpalette")]
    pub fn getpalette(&self, rawmode: Option<String>) -> Result<Option<Vec<u8>>, JsValue> {
        self.inner
            .getpalette_with_input(rawmode.as_deref())
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "putpalette")]
    pub fn putpalette(&mut self, data: Vec<u8>, rawmode: Option<String>) -> Result<(), JsValue> {
        self.inner
            .putpalette(&data, rawmode.as_deref().unwrap_or("RGB"))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "getexif")]
    pub fn getexif(&self) -> Vec<u8> {
        self.inner.getexif()
    }
    #[wasm_bindgen(js_name = "getxmp")]
    pub fn getxmp(&self) -> JsValue {
        JsValue::from_str("{}")
    }
    #[wasm_bindgen(js_name = "getChildImages")]
    pub fn get_child_images(&self) -> Vec<Image> {
        vec![]
    }
    #[wasm_bindgen(js_name = "getFlattenedData")]
    pub fn get_flattened(&self) -> Result<Vec<u8>, JsValue> {
        self.inner.tobytes().map_err(err)
    }
    #[wasm_bindgen(js_name = "applyTransparency")]
    pub fn apply_transparency(&mut self) -> Result<(), JsValue> {
        self.inner.apply_transparency().map_err(err)
    }
    #[wasm_bindgen(js_name = "paletteMode")]
    pub fn palette_mode(&self) -> Option<String> {
        self.inner.palette_mode().map(str::to_owned)
    }
    #[wasm_bindgen(js_name = "paletteRgba")]
    pub fn palette_rgba(&self) -> Option<Vec<u8>> {
        self.inner.getpalette_rgba()
    }
    #[wasm_bindgen(js_name = "pendingTransparencyIndex")]
    pub fn pending_transparency_index(&self) -> Option<u8> {
        match self.inner.pending_palette_transparency() {
            Some(pillow_rs::PaletteTransparency::Index(index)) => Some(index),
            _ => None,
        }
    }
    #[wasm_bindgen(js_name = "pendingTransparencyTable")]
    pub fn pending_transparency_table(&self) -> Option<Vec<u8>> {
        match self.inner.pending_palette_transparency() {
            Some(pillow_rs::PaletteTransparency::Table(alpha)) => Some(alpha),
            _ => None,
        }
    }
    #[wasm_bindgen(js_name = "hasTransparencyData")]
    pub fn has_transparency_data(&self) -> bool {
        self.inner.has_transparency_data()
    }
    #[wasm_bindgen(js_name = "draft")]
    pub fn draft(&self) -> Image {
        Image {
            inner: self.inner.clone(),
        }
    }
    #[wasm_bindgen(js_name = "putpixelRaw")]
    pub fn putpixel_raw(
        &mut self,
        x: u32,
        y: u32,
        r: u8,
        g: u8,
        b: u8,
        a: u8,
    ) -> Result<(), JsValue> {
        self.inner.putpixel(x, y, r, g, b, a).map_err(err)
    }

    pub fn repr(&mut self) -> Result<String, JsValue> {
        let (w, h) = self.inner.size().map_err(err)?;
        let m = self.inner.mode().map_err(err)?;
        Ok(format!("<Image {}x{} {}>", w, h, m))
    }
}

// ── ImageDraw ────────────────────────────────────────────────────
use pillow_rs::Draw;

#[wasm_bindgen]
pub struct ImageDraw {
    draw: Draw,
}

#[wasm_bindgen]
impl ImageDraw {
    #[wasm_bindgen(constructor)]
    pub fn new(img: &Image, mode: Option<String>) -> Result<ImageDraw, JsValue> {
        let draw = Draw::new(img.inner.clone(), mode);
        draw.validate_mode().map_err(err)?;
        Ok(ImageDraw { draw })
    }

    #[wasm_bindgen(js_name = "line")]
    pub fn line(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        r: u8,
        g: u8,
        b: u8,
        a: u8,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        self.draw
            .line(x0, y0, x1, y1, (r, g, b, a), width.unwrap_or(1))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "lineWithInput")]
    pub fn line_with_input(
        &mut self,
        points: JsValue,
        r: u8,
        g: u8,
        b: u8,
        a: u8,
        width: Option<u32>,
        joint: Option<String>,
    ) -> Result<(), JsValue> {
        let input = if !points.is_array() {
            pillow_rs::DrawPointsInput::Invalid
        } else {
            let values = js_sys::Array::from(&points).iter().collect::<Vec<_>>();
            if let Some(flat) = values
                .iter()
                .map(|value| value.as_f64())
                .collect::<Option<Vec<_>>>()
            {
                pillow_rs::DrawPointsInput::Flat(
                    flat.into_iter().map(|value| value as i32).collect(),
                )
            } else {
                let nested = values
                    .iter()
                    .map(|value| {
                        if !value.is_array() {
                            return None;
                        }
                        js_sys::Array::from(value)
                            .iter()
                            .map(|item| item.as_f64().map(|number| number as i32))
                            .collect::<Option<Vec<_>>>()
                    })
                    .collect::<Option<Vec<_>>>();
                nested.map_or(
                    pillow_rs::DrawPointsInput::InvalidSequence,
                    pillow_rs::DrawPointsInput::Nested,
                )
            }
        };
        self.draw
            .polyline_with_input_joint(input, (r, g, b, a), width.unwrap_or(1), joint.as_deref())
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "lineWithColorInput")]
    pub fn line_with_color_input(
        &mut self,
        points: JsValue,
        fill: JsValue,
        width: Option<u32>,
        joint: Option<String>,
    ) -> Result<(), JsValue> {
        let color = self
            .draw
            .color_with_input(js_draw_color_input(&fill))
            .map_err(err)?;
        self.draw
            .polyline_with_input_joint(
                js_draw_points_input(&points),
                color,
                width.unwrap_or(1),
                joint.as_deref(),
            )
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "rectangleWithInput")]
    pub fn rectangle_with_input(
        &mut self,
        xy: JsValue,
        fill: JsValue,
        outline: JsValue,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let (x0, y0, x1, y1) = js_draw_box_coordinates(&xy)?;
        let fill = js_draw_optional_color(&self.draw, &fill)?;
        let outline = js_draw_optional_color(&self.draw, &outline)?;
        self.draw
            .rectangle(x0, y0, x1, y1, fill, outline, width.unwrap_or(1))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "ellipseWithInput")]
    pub fn ellipse_with_input(
        &mut self,
        xy: JsValue,
        fill: JsValue,
        outline: JsValue,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let (x0, y0, x1, y1) = js_draw_box_coordinates(&xy)?;
        let fill = js_draw_optional_color(&self.draw, &fill)?;
        let outline = js_draw_optional_color(&self.draw, &outline)?;
        self.draw
            .ellipse(x0, y0, x1, y1, fill, outline, width.unwrap_or(1))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "arcWithInput")]
    pub fn arc_with_input(
        &mut self,
        xy: JsValue,
        start: f64,
        end: f64,
        fill: JsValue,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let (x0, y0, x1, y1) = js_draw_box_coordinates(&xy)?;
        let fill = self
            .draw
            .color_with_input(js_draw_color_input(&fill))
            .map_err(err)?;
        self.draw
            .arc(x0, y0, x1, y1, start, end, fill, width.unwrap_or(1))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "chordWithInput")]
    pub fn chord_with_input(
        &mut self,
        xy: JsValue,
        start: f64,
        end: f64,
        fill: JsValue,
        outline: JsValue,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let (x0, y0, x1, y1) = js_draw_box_coordinates(&xy)?;
        let fill = js_draw_optional_color(&self.draw, &fill)?;
        let outline = js_draw_optional_color(&self.draw, &outline)?;
        self.draw
            .chord(
                x0,
                y0,
                x1,
                y1,
                start,
                end,
                fill,
                outline,
                width.unwrap_or(1),
            )
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "piesliceWithInput")]
    pub fn pieslice_with_input(
        &mut self,
        xy: JsValue,
        start: f64,
        end: f64,
        fill: JsValue,
        outline: JsValue,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let (x0, y0, x1, y1) = js_draw_box_coordinates(&xy)?;
        let fill = js_draw_optional_color(&self.draw, &fill)?;
        let outline = js_draw_optional_color(&self.draw, &outline)?;
        self.draw
            .pieslice(
                x0,
                y0,
                x1,
                y1,
                start,
                end,
                fill,
                outline,
                width.unwrap_or(1),
            )
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "circleWithInput")]
    pub fn circle_with_input(
        &mut self,
        xy: JsValue,
        radius: f64,
        fill: JsValue,
        outline: JsValue,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let center = js_draw_circle_center_input(&xy);
        let fill = js_draw_optional_color(&self.draw, &fill)?;
        let outline = js_draw_optional_color(&self.draw, &outline)?;
        self.draw
            .circle_with_input(center, radius, fill, outline, width.unwrap_or(1))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "roundedRectangleWithInput")]
    pub fn rounded_rectangle_with_input(
        &mut self,
        xy: JsValue,
        radius: f64,
        fill: JsValue,
        outline: JsValue,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let input = js_draw_box_input(&xy);
        let fill = js_draw_optional_color(&self.draw, &fill)?;
        let outline = js_draw_optional_color(&self.draw, &outline)?;
        self.draw
            .rounded_rectangle_with_input(input, radius, fill, outline, width.unwrap_or(1))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "polygonWithInput")]
    pub fn polygon_with_input(
        &mut self,
        points: JsValue,
        fill: JsValue,
        outline: JsValue,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let fill = js_draw_optional_color(&self.draw, &fill)?;
        let outline = js_draw_optional_color(&self.draw, &outline)?;
        self.draw
            .polygon_with_input(
                js_draw_points_input(&points),
                fill,
                outline,
                width.unwrap_or(1),
            )
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "pointWithInput")]
    pub fn point_with_input(&mut self, points: JsValue, fill: JsValue) -> Result<(), JsValue> {
        let fill = self
            .draw
            .color_with_input(js_draw_color_input(&fill))
            .map_err(err)?;
        self.draw
            .point_with_input(js_draw_points_input(&points), fill)
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "shapeWithInput")]
    pub fn shape_with_input(
        &mut self,
        points: JsValue,
        fill: JsValue,
        outline: JsValue,
    ) -> Result<(), JsValue> {
        let fill = js_draw_optional_color(&self.draw, &fill)?;
        let outline = js_draw_optional_color(&self.draw, &outline)?;
        self.draw
            .shape_with_input(js_draw_points_input(&points), fill, outline)
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "regularPolygonWithInput")]
    pub fn regular_polygon_with_input(
        &mut self,
        bounding_circle: JsValue,
        n_sides: JsValue,
        rotation: f64,
        fill: JsValue,
        outline: JsValue,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let circle = if let Some(values) = js_float_array(&bounding_circle) {
            if values.len() == 3 {
                pillow_rs::RegularPolygonCircle::Flat(values[0], values[1], values[2])
            } else {
                pillow_rs::RegularPolygonCircle::Invalid
            }
        } else if bounding_circle.is_array() {
            let values = js_sys::Array::from(&bounding_circle);
            if values.length() == 2 {
                let center = js_float_array(&values.get(0));
                let radius = values.get(1).as_f64();
                match (center, radius) {
                    (Some(center), Some(radius)) if center.len() == 2 => {
                        pillow_rs::RegularPolygonCircle::Nested(center[0], center[1], radius)
                    }
                    _ => pillow_rs::RegularPolygonCircle::Invalid,
                }
            } else {
                pillow_rs::RegularPolygonCircle::Invalid
            }
        } else {
            pillow_rs::RegularPolygonCircle::Invalid
        };
        let sides = js_integer(&n_sides).map_or(
            pillow_rs::RegularPolygonSides::Invalid,
            pillow_rs::RegularPolygonSides::Value,
        );
        let fill = js_draw_optional_color(&self.draw, &fill)?;
        let outline = js_draw_optional_color(&self.draw, &outline)?;
        self.draw
            .regular_polygon(circle, sides, rotation, fill, outline, width.unwrap_or(1))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "rectangle")]
    pub fn rect(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        fr: Option<u8>,
        fg: Option<u8>,
        fb: Option<u8>,
        fa: Option<u8>,
        or: Option<u8>,
        og: Option<u8>,
        ob: Option<u8>,
        oa: Option<u8>,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        let out = or.map(|r| (r, og.unwrap_or(0), ob.unwrap_or(0), oa.unwrap_or(255)));
        self.draw
            .rectangle(x0, y0, x1, y1, fill, out, width.unwrap_or(1))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "ellipse")]
    pub fn ellipse(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        fr: Option<u8>,
        fg: Option<u8>,
        fb: Option<u8>,
        fa: Option<u8>,
        or: Option<u8>,
        og: Option<u8>,
        ob: Option<u8>,
        oa: Option<u8>,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        let out = or.map(|r| (r, og.unwrap_or(0), ob.unwrap_or(0), oa.unwrap_or(255)));
        self.draw
            .ellipse(x0, y0, x1, y1, fill, out, width.unwrap_or(1))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "polygon")]
    pub fn polygon(
        &mut self,
        points: Vec<i32>,
        fr: Option<u8>,
        fg: Option<u8>,
        fb: Option<u8>,
        fa: Option<u8>,
        or: Option<u8>,
        og: Option<u8>,
        ob: Option<u8>,
        oa: Option<u8>,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        let out = or.map(|r| (r, og.unwrap_or(0), ob.unwrap_or(0), oa.unwrap_or(255)));
        self.draw
            .polygon_with_input(
                pillow_rs::DrawPointsInput::Flat(points),
                fill,
                out,
                width.unwrap_or(1),
            )
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "point")]
    pub fn point(&mut self, pts: Vec<i32>, r: u8, g: u8, b: u8, a: u8) -> Result<(), JsValue> {
        self.draw
            .point_with_input(pillow_rs::DrawPointsInput::Flat(pts), (r, g, b, a))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "arc")]
    pub fn arc(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        start: f64,
        end: f64,
        r: u8,
        g: u8,
        b: u8,
        a: u8,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        self.draw
            .arc(x0, y0, x1, y1, start, end, (r, g, b, a), width.unwrap_or(1))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "chord")]
    pub fn chord(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        start: f64,
        end: f64,
        fr: Option<u8>,
        fg: Option<u8>,
        fb: Option<u8>,
        fa: Option<u8>,
        or: Option<u8>,
        og: Option<u8>,
        ob: Option<u8>,
        oa: Option<u8>,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        let out = or.map(|r| (r, og.unwrap_or(0), ob.unwrap_or(0), oa.unwrap_or(255)));
        self.draw
            .chord(x0, y0, x1, y1, start, end, fill, out, width.unwrap_or(1))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "pieslice")]
    pub fn pieslice(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        start: f64,
        end: f64,
        fr: Option<u8>,
        fg: Option<u8>,
        fb: Option<u8>,
        fa: Option<u8>,
        or: Option<u8>,
        og: Option<u8>,
        ob: Option<u8>,
        oa: Option<u8>,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        let out = or.map(|r| (r, og.unwrap_or(0), ob.unwrap_or(0), oa.unwrap_or(255)));
        self.draw
            .pieslice(x0, y0, x1, y1, start, end, fill, out, width.unwrap_or(1))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "circle")]
    pub fn circle(
        &mut self,
        cx: f64,
        cy: f64,
        radius: f64,
        fr: Option<u8>,
        fg: Option<u8>,
        fb: Option<u8>,
        fa: Option<u8>,
        or: Option<u8>,
        og: Option<u8>,
        ob: Option<u8>,
        oa: Option<u8>,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        let out = or.map(|r| (r, og.unwrap_or(0), ob.unwrap_or(0), oa.unwrap_or(255)));
        self.draw
            .circle(cx as i32, cy as i32, radius, fill, out, width.unwrap_or(1))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "roundedRectangle")]
    pub fn rounded_rect(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        radius: f64,
        fr: Option<u8>,
        fg: Option<u8>,
        fb: Option<u8>,
        fa: Option<u8>,
        or: Option<u8>,
        og: Option<u8>,
        ob: Option<u8>,
        oa: Option<u8>,
        width: Option<u32>,
    ) -> Result<(), JsValue> {
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        let out = or.map(|r| (r, og.unwrap_or(0), ob.unwrap_or(0), oa.unwrap_or(255)));
        self.draw
            .rounded_rectangle(x0, y0, x1, y1, radius, fill, out, width.unwrap_or(1))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "text")]
    pub fn text(
        &mut self,
        x: f64,
        y: f64,
        text: &str,
        font: &ImageFont,
        r: u8,
        g: u8,
        b: u8,
        a: u8,
    ) -> Result<(), JsValue> {
        self.draw
            .text(x as i32, y as i32, text, &font.font, (r, g, b, a))
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "textWithInput")]
    #[allow(clippy::too_many_arguments)]
    pub fn text_with_input(
        &mut self,
        x: f64,
        y: f64,
        text: JsValue,
        font: Option<ImageFont>,
        fill: JsValue,
        direction: Option<String>,
        features: JsValue,
        language: Option<String>,
        stroke_width: f64,
        anchor: Option<String>,
        embedded_color: bool,
        font_size: Option<f64>,
    ) -> Result<(), JsValue> {
        let options = js_font_options(
            None,
            direction,
            &features,
            language,
            stroke_width,
            anchor,
            None,
            &JsValue::NULL,
            embedded_color,
            false,
            false,
            false,
            true,
        );
        let color = self
            .draw
            .text_color_with_input(js_draw_color_input(&fill))
            .map_err(err)?;
        self.draw
            .text_with_optional_font_input(
                x as i32,
                y as i32,
                js_font_text_input(&text),
                font.as_ref().map(|font| &font.font),
                color,
                font_size.map(|size| size as f32),
                &options,
            )
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "multilineTextWithInput")]
    #[allow(clippy::too_many_arguments)]
    pub fn multiline_text_with_input(
        &mut self,
        x: f64,
        y: f64,
        text: JsValue,
        font: Option<ImageFont>,
        fill: JsValue,
        spacing: f64,
        direction: Option<String>,
        features: JsValue,
        language: Option<String>,
        stroke_width: f64,
        anchor: Option<String>,
        embedded_color: bool,
        font_size: Option<f64>,
    ) -> Result<(), JsValue> {
        let options = js_font_options(
            None,
            direction,
            &features,
            language,
            stroke_width,
            anchor,
            None,
            &JsValue::NULL,
            embedded_color,
            false,
            false,
            false,
            true,
        );
        let color = self
            .draw
            .text_color_with_input(js_draw_color_input(&fill))
            .map_err(err)?;
        self.draw
            .multiline_text_with_optional_font_input(
                x,
                y,
                js_font_text_input(&text),
                font.as_ref().map(|font| &font.font),
                color,
                spacing,
                font_size.map(|size| size as f32),
                &options,
            )
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "textbboxWithInput")]
    #[allow(clippy::too_many_arguments)]
    pub fn textbbox_with_input(
        &mut self,
        x: i32,
        y: i32,
        text: JsValue,
        font: Option<ImageFont>,
        direction: Option<String>,
        features: JsValue,
        language: Option<String>,
        stroke_width: f64,
        anchor: Option<String>,
        embedded_color: bool,
        font_size: Option<f64>,
    ) -> Result<Vec<i32>, JsValue> {
        let options = js_font_options(
            None,
            direction,
            &features,
            language,
            stroke_width,
            anchor,
            None,
            &JsValue::NULL,
            embedded_color,
            false,
            false,
            false,
            true,
        );
        self.draw.validate_text_options(&options).map_err(err)?;
        let bbox = pillow_rs::imagefont_textbbox_at_with_optional_font_input(
            font.as_ref().map(|font| &font.font),
            font_size.map(|size| size as f32),
            (x, y),
            js_font_text_input(&text),
            &options,
        )
        .map_err(err)?;
        Ok(vec![bbox.0, bbox.1, bbox.2, bbox.3])
    }
    #[wasm_bindgen(js_name = "textlengthWithInput")]
    #[allow(clippy::too_many_arguments)]
    pub fn textlength_with_input(
        &mut self,
        text: JsValue,
        font: Option<ImageFont>,
        direction: Option<String>,
        features: JsValue,
        language: Option<String>,
        embedded_color: bool,
        font_size: Option<f64>,
    ) -> Result<f64, JsValue> {
        let options = js_font_options(
            None,
            direction,
            &features,
            language,
            0.0,
            None,
            None,
            &JsValue::NULL,
            embedded_color,
            false,
            false,
            false,
            false,
        );
        self.draw.validate_text_options(&options).map_err(err)?;
        pillow_rs::imagefont_getlength_with_optional_font_input(
            font.as_ref().map(|font| &font.font),
            font_size.map(|size| size as f32),
            js_font_text_input(&text),
            &options,
        )
        .map(|value| value as f64)
        .map_err(err)
    }
    #[wasm_bindgen(js_name = "multilineTextbboxWithInput")]
    #[allow(clippy::too_many_arguments)]
    pub fn multiline_textbbox_with_input(
        &mut self,
        x: i32,
        y: i32,
        text: JsValue,
        font: Option<ImageFont>,
        spacing: i32,
        align: String,
        direction: Option<String>,
        features: JsValue,
        language: Option<String>,
        stroke_width: f64,
        anchor: Option<String>,
        embedded_color: bool,
        font_size: Option<f64>,
    ) -> Result<Vec<i32>, JsValue> {
        let options = js_font_options(
            None,
            direction,
            &features,
            language,
            stroke_width,
            anchor,
            None,
            &JsValue::NULL,
            embedded_color,
            false,
            false,
            false,
            true,
        );
        self.draw.validate_text_options(&options).map_err(err)?;
        let bbox = pillow_rs::imagefont_multiline_textbbox_with_optional_font_input(
            font.as_ref().map(|font| &font.font),
            font_size.map(|size| size as f32),
            (x, y),
            js_font_text_input(&text),
            spacing,
            &align,
            &options,
        )
        .map_err(err)?;
        Ok(vec![bbox.0, bbox.1, bbox.2, bbox.3])
    }
    #[wasm_bindgen(js_name = "bitmap")]
    pub fn bitmap(
        &mut self,
        x: i32,
        y: i32,
        bitmap: &Image,
        fr: Option<u8>,
        fg: Option<u8>,
        fb: Option<u8>,
        fa: Option<u8>,
    ) -> Result<(), JsValue> {
        let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
        self.draw.bitmap(x, y, &bitmap.inner, fill).map_err(err)
    }
    #[wasm_bindgen(js_name = "bitmapWithInput")]
    pub fn bitmap_with_input(
        &mut self,
        x: i32,
        y: i32,
        bitmap: &Image,
        fill: JsValue,
    ) -> Result<(), JsValue> {
        self.draw
            .bitmap_with_input(x, y, &bitmap.inner, js_draw_color_input(&fill))
            .map_err(err)
    }
    #[wasm_bindgen(getter)]
    pub fn image(&self) -> Result<Image, JsValue> {
        // Core image_clone() already handles mode preservation
        // (RGB→RGB, RGBA→RGBA, L→L, etc.)
        Ok(Image {
            inner: self.draw.image_clone().map_err(err)?,
        })
    }
}

// ── ImageFont ────────────────────────────────────────────────────
use pillow_rs::FreeTypeFont as RsImageFont;

#[wasm_bindgen]
pub struct ImageFont {
    font: RsImageFont,
}

#[wasm_bindgen]
pub struct ImageFontMask {
    width: u32,
    height: u32,
    mode: String,
    offset_x: i32,
    offset_y: i32,
    pixels: Vec<u8>,
}

#[wasm_bindgen]
impl ImageFontMask {
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.height
    }

    #[wasm_bindgen(getter)]
    pub fn mode(&self) -> String {
        self.mode.clone()
    }

    #[wasm_bindgen(getter, js_name = "offsetX")]
    pub fn offset_x(&self) -> i32 {
        self.offset_x
    }

    #[wasm_bindgen(getter, js_name = "offsetY")]
    pub fn offset_y(&self) -> i32 {
        self.offset_y
    }

    #[wasm_bindgen(getter)]
    pub fn pixels(&self) -> Vec<u8> {
        self.pixels.clone()
    }
}

#[wasm_bindgen]
impl ImageFont {
    #[wasm_bindgen(constructor)]
    pub fn new(data: Vec<u8>, size: f32) -> Result<ImageFont, JsValue> {
        pillow_rs::imagefont_from_bytes(data, size)
            .map(|f| ImageFont { font: f })
            .map_err(err)
    }

    #[wasm_bindgen(js_name = "fromBytes")]
    pub fn from_bytes(
        data: Vec<u8>,
        size: f32,
        index: Option<u32>,
        encoding: Option<String>,
        layout_engine: Option<String>,
    ) -> Result<ImageFont, JsValue> {
        let options = pillow_rs::ImageFontLoadOptions {
            index: index.map(|value| value as usize),
            encoding: encoding.filter(|value| !value.is_empty()),
            layout_engine,
        };
        pillow_rs::imagefont_from_bytes_with_options(data, size, &options)
            .map(|font| ImageFont { font })
            .map_err(err)
    }

    #[wasm_bindgen(js_name = "getmask2")]
    pub fn getmask2(
        &self,
        text: &str,
        start_x: Option<f64>,
        start_y: Option<f64>,
    ) -> Result<ImageFontMask, JsValue> {
        let (width, height, pixels, offset) = pillow_rs::imagefont_getmask2_with_start(
            &self.font,
            text,
            (start_x.unwrap_or(0.0), start_y.unwrap_or(0.0)),
        )
        .map_err(err)?;
        Ok(ImageFontMask {
            width,
            height,
            mode: "L".to_owned(),
            offset_x: offset.0,
            offset_y: offset.1,
            pixels,
        })
    }

    #[wasm_bindgen(js_name = "getbboxWithOptions")]
    #[allow(clippy::too_many_arguments)]
    pub fn getbbox_with_options(
        &self,
        text: JsValue,
        mode: Option<String>,
        direction: Option<String>,
        features: JsValue,
        language: Option<String>,
        stroke_width: f64,
        anchor: Option<String>,
    ) -> Result<Vec<f64>, JsValue> {
        let options = js_font_options(
            mode,
            direction,
            &features,
            language,
            stroke_width,
            anchor,
            None,
            &JsValue::NULL,
            false,
            false,
            false,
            false,
            false,
        );
        let bbox = pillow_rs::imagefont_getbbox_input_with_options(
            &self.font,
            js_font_text_input(&text),
            &options,
        )
        .map_err(err)?;
        Ok(vec![
            bbox.0 as f64,
            bbox.1 as f64,
            bbox.2 as f64,
            bbox.3 as f64,
        ])
    }

    #[wasm_bindgen(js_name = "getlengthWithOptions")]
    pub fn getlength_with_options(
        &self,
        text: JsValue,
        mode: Option<String>,
        direction: Option<String>,
        features: JsValue,
        language: Option<String>,
    ) -> Result<f64, JsValue> {
        let options = js_font_options(
            mode,
            direction,
            &features,
            language,
            0.0,
            None,
            None,
            &JsValue::NULL,
            false,
            false,
            false,
            false,
            false,
        );
        pillow_rs::imagefont_getlength_input_with_options(
            &self.font,
            js_font_text_input(&text),
            &options,
        )
        .map(|value| value as f64)
        .map_err(err)
    }

    #[wasm_bindgen(js_name = "getMetrics")]
    pub fn get_metrics(&self) -> Vec<u32> {
        let (ascent, descent) = pillow_rs::imagefont_getmetrics(&self.font);
        vec![ascent, descent]
    }

    #[wasm_bindgen(js_name = "getmaskWithOptions")]
    #[allow(clippy::too_many_arguments)]
    pub fn getmask_with_options(
        &self,
        text: JsValue,
        mode: Option<String>,
        direction: Option<String>,
        features: JsValue,
        language: Option<String>,
        stroke_width: f64,
        anchor: Option<String>,
        ink: JsValue,
        start: JsValue,
    ) -> Result<ImageFontMask, JsValue> {
        let ink = js_font_ink(&ink)?;
        let options = js_font_options(
            mode,
            direction,
            &features,
            language,
            stroke_width,
            anchor,
            ink,
            &start,
            false,
            false,
            false,
            false,
            false,
        );
        let (width, height, pixels) = pillow_rs::imagefont_getmask_input_with_options(
            &self.font,
            js_font_text_input(&text),
            &options,
        )
        .map_err(err)?;
        Ok(ImageFontMask {
            width,
            height,
            mode: if options.uses_color_mask() {
                "RGBA".to_owned()
            } else {
                "L".to_owned()
            },
            offset_x: 0,
            offset_y: 0,
            pixels,
        })
    }

    #[wasm_bindgen(js_name = "getmask2WithOptions")]
    #[allow(clippy::too_many_arguments)]
    pub fn getmask2_with_options(
        &self,
        text: JsValue,
        mode: Option<String>,
        direction: Option<String>,
        features: JsValue,
        language: Option<String>,
        stroke_width: f64,
        anchor: Option<String>,
        ink: JsValue,
        start: JsValue,
        stroke_filled: bool,
        has_args: bool,
        has_kwargs: bool,
    ) -> Result<ImageFontMask, JsValue> {
        let ink = js_font_ink(&ink)?;
        let options = js_font_options(
            mode,
            direction,
            &features,
            language,
            stroke_width,
            anchor,
            ink,
            &start,
            false,
            stroke_filled,
            has_args,
            has_kwargs,
            false,
        );
        let (width, height, pixels, offset) = pillow_rs::imagefont_getmask2_input_with_options(
            &self.font,
            js_font_text_input(&text),
            &options,
        )
        .map_err(err)?;
        Ok(ImageFontMask {
            width,
            height,
            mode: if options.uses_color_mask() {
                "RGBA".to_owned()
            } else {
                "L".to_owned()
            },
            offset_x: offset.0,
            offset_y: offset.1,
            pixels,
        })
    }

    #[wasm_bindgen(js_name = "getname")]
    pub fn getname(&self) -> js_sys::Array {
        let (family, style) = pillow_rs::imagefont_getname(&self.font);
        let result = js_sys::Array::new();
        result.push(&family.map_or(JsValue::NULL, JsValue::from_str));
        result.push(&style.map_or(JsValue::NULL, JsValue::from_str));
        result
    }

    #[wasm_bindgen(js_name = "getVariationAxes")]
    pub fn get_variation_axes(&self) -> Result<js_sys::Array, JsValue> {
        let axes = pillow_rs::imagefont_get_variation_axes(&self.font).map_err(err)?;
        let result = js_sys::Array::new();
        for axis in axes {
            let object = js_sys::Object::new();
            js_sys::Reflect::set(
                &object,
                &JsValue::from_str("minimum"),
                &JsValue::from_f64(axis.minimum as f64),
            )?;
            js_sys::Reflect::set(
                &object,
                &JsValue::from_str("default"),
                &JsValue::from_f64(axis.default as f64),
            )?;
            js_sys::Reflect::set(
                &object,
                &JsValue::from_str("maximum"),
                &JsValue::from_f64(axis.maximum as f64),
            )?;
            js_sys::Reflect::set(
                &object,
                &JsValue::from_str("name"),
                &js_sys::Uint8Array::from(axis.name.as_slice()).into(),
            )?;
            result.push(&object);
        }
        Ok(result)
    }

    #[wasm_bindgen(js_name = "getVariationNames")]
    pub fn get_variation_names(&self) -> Result<js_sys::Array, JsValue> {
        let names = pillow_rs::imagefont_get_variation_names(&self.font).map_err(err)?;
        let result = js_sys::Array::new();
        for name in names {
            result.push(&js_sys::Uint8Array::from(name.as_slice()));
        }
        Ok(result)
    }

    #[wasm_bindgen(js_name = "setVariationByName")]
    pub fn set_variation_by_name(&mut self, name: Vec<u8>) -> Result<(), JsValue> {
        pillow_rs::imagefont_set_variation_by_name(&mut self.font, &name).map_err(err)
    }

    #[wasm_bindgen(js_name = "setVariationByNameWithInput")]
    pub fn set_variation_by_name_with_input(&mut self, name: JsValue) -> Result<(), JsValue> {
        pillow_rs::imagefont_set_variation_by_name_input(
            &mut self.font,
            js_font_variation_name_input(&name),
        )
        .map_err(err)
    }

    #[wasm_bindgen(js_name = "setVariationByAxes")]
    pub fn set_variation_by_axes(&mut self, axes: Vec<f32>) -> Result<(), JsValue> {
        pillow_rs::imagefont_set_variation_by_axes(&mut self.font, &axes).map_err(err)
    }

    #[wasm_bindgen(js_name = "setVariationByAxesWithInput")]
    pub fn set_variation_by_axes_with_input(&mut self, axes: JsValue) -> Result<(), JsValue> {
        pillow_rs::imagefont_set_variation_by_axes_input(
            &mut self.font,
            js_font_variation_axes_input(&axes),
        )
        .map_err(err)
    }

    #[wasm_bindgen(js_name = "fontVariant")]
    pub fn font_variant(&self, size: Option<f32>) -> Result<ImageFont, JsValue> {
        pillow_rs::imagefont_variant(&self.font, size)
            .map(|font| ImageFont { font })
            .map_err(err)
    }

    #[wasm_bindgen(js_name = "fontVariantWithOptions")]
    pub fn font_variant_with_options(
        &self,
        font: JsValue,
        size: Option<f64>,
        index: Option<u32>,
        encoding: Option<String>,
        layout_engine: Option<String>,
    ) -> Result<ImageFont, JsValue> {
        let font_bytes = if font.is_null() || font.is_undefined() {
            None
        } else if font.is_instance_of::<js_sys::Uint8Array>() {
            Some(js_sys::Uint8Array::new(&font).to_vec())
        } else {
            return Err(err(pillow_rs::PilError::TypeError(
                "font must be a file path or file-like object".to_owned(),
            )));
        };
        let options = pillow_rs::ImageFontVariantOptions {
            font_bytes,
            size: size.map(|value| value as f32),
            index: index.map(|value| value as usize),
            encoding,
            layout_engine,
        };
        pillow_rs::imagefont_variant_with_options(&self.font, &options)
            .map(|font| ImageFont { font })
            .map_err(err)
    }

    #[wasm_bindgen(js_name = "getTransposedMask")]
    pub fn get_transposed_mask(
        &self,
        text: &str,
        orientation: Option<String>,
    ) -> Result<ImageFontMask, JsValue> {
        let (width, height, pixels) =
            pillow_rs::imagefont_get_transposed_mask(&self.font, text, orientation.as_deref())
                .map_err(err)?;
        Ok(ImageFontMask {
            width,
            height,
            mode: "L".to_owned(),
            offset_x: 0,
            offset_y: 0,
            pixels,
        })
    }

    #[wasm_bindgen(js_name = "getTransposedBbox")]
    pub fn get_transposed_bbox(
        &self,
        text: &str,
        orientation: Option<String>,
    ) -> Result<Vec<i32>, JsValue> {
        let bbox = pillow_rs::transposed_bbox(
            pillow_rs::imagefont_getbbox(&self.font, text).map_err(err)?,
            orientation.as_deref(),
        );
        Ok(vec![bbox.0, bbox.1, bbox.2, bbox.3])
    }

    #[wasm_bindgen(js_name = "getTransposedLength")]
    pub fn get_transposed_length(
        &self,
        text: &str,
        orientation: Option<String>,
    ) -> Result<f32, JsValue> {
        pillow_rs::validate_transposed_length(orientation.as_deref()).map_err(err)?;
        pillow_rs::imagefont_getlength(&self.font, text).map_err(err)
    }

    #[wasm_bindgen(js_name = "getbbox")]
    pub fn getbbox(&self, text: &str) -> Result<Vec<u32>, JsValue> {
        let (w, h) = pillow_rs::imagefont_text_bbox(&self.font, text).map_err(err)?;
        Ok(vec![w, h])
    }
    #[wasm_bindgen(js_name = "getmask")]
    pub fn getmask(&self, text: &str) -> Result<Vec<u8>, JsValue> {
        let (w, h, data) = pillow_rs::imagefont_getmask(&self.font, text).map_err(err)?;
        let mut result = vec![
            w as u8,
            (w >> 8) as u8,
            (w >> 16) as u8,
            (w >> 24) as u8,
            h as u8,
            (h >> 8) as u8,
            (h >> 16) as u8,
            (h >> 24) as u8,
        ];
        result.extend(data);
        Ok(result)
    }
}

// ── ImagePalette ─────────────────────────────────────────────────
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
#[wasm_bindgen]
pub struct ImageStat {
    inner: pillow_rs::StatResult,
}
#[wasm_bindgen]
impl ImageStat {
    #[wasm_bindgen(constructor)]
    pub fn new(img: &Image, mask: Option<Image>) -> Result<ImageStat, JsValue> {
        let s = img
            .inner
            .stat_formatted_with_mask(match mask.as_ref() {
                Some(mask) => pillow_rs::ImageOpsMask::Image(mask.inner.clone()),
                None => pillow_rs::ImageOpsMask::None,
            })
            .map_err(err)?;
        Ok(ImageStat { inner: s })
    }
    fn val_to_js(&self, v: &pillow_rs::StatValue) -> JsValue {
        use pillow_rs::StatValue;
        match v {
            StatValue::Int(i) => JsValue::from_f64(*i as f64),
            StatValue::Float(f) => JsValue::from_f64(*f),
            StatValue::IntList(l) => {
                let arr = js_sys::Array::new();
                for &x in l {
                    arr.push(&JsValue::from_f64(x as f64));
                }
                arr.into()
            }
            StatValue::FloatList(l) => {
                let arr = js_sys::Array::new();
                for &x in l {
                    arr.push(&JsValue::from_f64(x));
                }
                arr.into()
            }
            StatValue::ExtremaSingle((min, max)) => {
                let arr = js_sys::Array::new();
                arr.push(&JsValue::from_f64(*min as f64));
                arr.push(&JsValue::from_f64(*max as f64));
                arr.into()
            }
            StatValue::ExtremaList(l) => {
                let arr = js_sys::Array::new();
                for &(min, max) in l {
                    let pair = js_sys::Array::new();
                    pair.push(&JsValue::from_f64(min as f64));
                    pair.push(&JsValue::from_f64(max as f64));
                    arr.push(&pair);
                }
                arr.into()
            }
        }
    }
    pub fn toObject(&self) -> js_sys::Object {
        let obj = js_sys::Object::new();
        js_sys::Reflect::set(&obj, &"count".into(), &self.val_to_js(&self.inner.count)).ok();
        js_sys::Reflect::set(&obj, &"sum".into(), &self.val_to_js(&self.inner.sum)).ok();
        js_sys::Reflect::set(&obj, &"sum2".into(), &self.val_to_js(&self.inner.sum2)).ok();
        js_sys::Reflect::set(&obj, &"mean".into(), &self.val_to_js(&self.inner.mean)).ok();
        js_sys::Reflect::set(&obj, &"median".into(), &self.val_to_js(&self.inner.median)).ok();
        js_sys::Reflect::set(&obj, &"rms".into(), &self.val_to_js(&self.inner.rms)).ok();
        js_sys::Reflect::set(&obj, &"var".into(), &self.val_to_js(&self.inner.var)).ok();
        js_sys::Reflect::set(&obj, &"stddev".into(), &self.val_to_js(&self.inner.stddev)).ok();
        js_sys::Reflect::set(
            &obj,
            &"extrema".into(),
            &self.val_to_js(&self.inner.extrema),
        )
        .ok();
        obj
    }
}

// ── ImageSequence ────────────────────────────────────────────────
#[wasm_bindgen]
pub struct ImageSequence {
    image: Image,
    yielded: bool,
}
#[wasm_bindgen]
impl ImageSequence {
    #[wasm_bindgen(constructor)]
    pub fn new(img: &Image) -> ImageSequence {
        ImageSequence {
            image: Image {
                inner: img.inner.clone(),
            },
            yielded: false,
        }
    }
    #[wasm_bindgen(js_name = "next")]
    pub fn next(&mut self) -> Option<Image> {
        if self.yielded {
            return None;
        }
        self.yielded = true;
        Some(Image {
            inner: self.image.inner.clone(),
        })
    }
}

// ── Remaining stubs (WASM equivalents for file-I/O functions) ────
#[wasm_bindgen]
impl Image {
    #[wasm_bindgen(js_name = "save")]
    pub fn save(&mut self) -> Result<Vec<u8>, JsValue> {
        // Returns PNG-encoded bytes for download (browser) or fs.writeFile (server).
        // Uses the image crate's PNG encoder built into pillow-rs.
        self.inner.to_png_bytes().map_err(err)
    }

    #[wasm_bindgen(js_name = "saveWithInput")]
    pub fn save_with_input(
        &mut self,
        format: JsValue,
        extension: JsValue,
    ) -> Result<Vec<u8>, JsValue> {
        let format = js_optional_string(&format, "format")?;
        let extension = js_optional_string(&extension, "extension")?;
        let resolved =
            RsImage::resolve_save_format(format.as_deref(), extension.as_deref()).map_err(err)?;
        self.inner.encode(&resolved).map_err(err)
    }

    /// Encode DynamicImage to PNG bytes
    fn encode_png(img: &mut RsImage) -> Result<Vec<u8>, JsValue> {
        img.to_png_bytes()
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
    #[wasm_bindgen(js_name = "show")]
    pub fn show(&self) -> JsValue {
        JsValue::from_str("show: use toBytes() for display")
    }
    #[wasm_bindgen(js_name = "close")]
    pub fn close(&self) {}
    #[wasm_bindgen(js_name = "draftFn")]
    pub fn draft_fn(&self, _m: &str, _w: u32, _h: u32) -> Image {
        Image {
            inner: self.inner.clone(),
        }
    }
    #[wasm_bindgen(js_name = "toqimage")]
    pub fn toqimage(&self) -> JsValue {
        JsValue::from_str("Qt not available in WASM")
    }
    #[wasm_bindgen(js_name = "toqpixmap")]
    pub fn toqpixmap(&self) -> JsValue {
        JsValue::from_str("Qt not available in WASM")
    }
    #[wasm_bindgen(js_name = "getim")]
    pub fn getim(&self) -> JsValue {
        JsValue::null()
    }
}
#[wasm_bindgen]
impl ImageFont {
    #[wasm_bindgen(js_name = "load")]
    pub fn load(_path: &str, _size: f32) -> Result<ImageFont, JsValue> {
        Err(JsValue::from_str(
            "Use new ImageFont(data, size) with font bytes",
        ))
    }
    #[wasm_bindgen(js_name = "loadPath")]
    pub fn load_path(_path: &str, _size: f32) -> Result<ImageFont, JsValue> {
        Err(JsValue::from_str(
            "Use new ImageFont(data, size) with font bytes",
        ))
    }
    #[wasm_bindgen(js_name = "loadDefault")]
    pub fn load_default() -> Result<ImageFont, JsValue> {
        pillow_rs::imagefont_load_default(10.0)
            .map(|font| ImageFont { font })
            .map_err(err)
    }
}

// ── Legacy PILfont ──────────────────────────────────────────────
#[wasm_bindgen]
pub struct PilFont {
    font: pillow_rs::PilFont,
}

#[wasm_bindgen]
impl PilFont {
    #[wasm_bindgen(js_name = "fromBytes")]
    pub fn from_bytes(metrics: Vec<u8>, glyph_image: Vec<u8>) -> Result<PilFont, JsValue> {
        let glyph_image = pillow_rs::PilFont::open_pilfont_glyph_image(glyph_image).map_err(err)?;
        pillow_rs::PilFont::from_pilfont_glyph_data(&metrics, glyph_image)
            .map(|font| PilFont { font })
            .map_err(err)
    }

    #[wasm_bindgen(js_name = "loadDefault")]
    pub fn load_default() -> Result<PilFont, JsValue> {
        pillow_rs::PilFont::load_default()
            .map(|font| PilFont { font })
            .map_err(err)
    }

    #[wasm_bindgen(js_name = "getbboxWithInput")]
    pub fn getbbox_with_input(&self, text: JsValue) -> Result<Vec<i32>, JsValue> {
        let bbox = self
            .font
            .getbbox_input(js_pilfont_text_input(&text))
            .map_err(err)?;
        Ok(vec![bbox.0, bbox.1, bbox.2, bbox.3])
    }

    #[wasm_bindgen(js_name = "getlengthWithInput")]
    pub fn getlength_with_input(&self, text: JsValue) -> Result<i32, JsValue> {
        self.font
            .getlength_input(js_pilfont_text_input(&text))
            .map_err(err)
    }

    #[wasm_bindgen(js_name = "getmaskWithInput")]
    pub fn getmask_with_input(&self, text: JsValue) -> Result<ImageFontMask, JsValue> {
        let mask = self
            .font
            .getmask_input(js_pilfont_text_input(&text))
            .map_err(err)?;
        Ok(ImageFontMask {
            width: mask.width,
            height: mask.height,
            mode: mask.mode.as_str().to_owned(),
            offset_x: 0,
            offset_y: 0,
            pixels: mask.pixels,
        })
    }
}
#[wasm_bindgen(js_name = "imageOpen")]
pub fn image_open_path(_path: &str) -> Result<Image, JsValue> {
    Err(JsValue::from_str(
        "Use Image.open(bytes) instead of file path in WASM",
    ))
}
#[wasm_bindgen(js_name = "imageNew")]
pub fn image_new(mode: &str, w: u32, h: u32, r: u8, g: u8, b: u8, a: u8) -> Result<Image, JsValue> {
    RsImage::new(w, h, mode, (r, g, b, a))
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "imageNewWithInput")]
pub fn image_new_with_input(mode: &str, w: u32, h: u32, color: JsValue) -> Result<Image, JsValue> {
    let input = js_new_color_input(
        mode,
        (!color.is_null() && !color.is_undefined()).then_some(&color),
    )?;
    RsImage::new_with_input(w, h, mode, input)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "fromBytesFn")]
pub fn from_bytes_fn(
    mode: &str,
    w: u32,
    h: u32,
    data: Vec<u8>,
    decoder_name: Option<String>,
) -> Result<Image, JsValue> {
    pillow_rs::image_frombytes(
        mode,
        (w, h),
        &data,
        decoder_name.as_deref().unwrap_or("raw"),
    )
    .map(|i| Image { inner: i })
    .map_err(err)
}

#[wasm_bindgen(js_name = "fromArrayFn")]
pub fn from_array_fn(
    shape: Vec<f64>,
    typestr: &str,
    mode: Option<String>,
    data: Vec<u8>,
) -> Result<Image, JsValue> {
    let shape = shape
        .into_iter()
        .map(|value| {
            if !value.is_finite() || value < 0.0 || value.fract() != 0.0 {
                return Err(err(pillow_rs::PilError::OverflowError(
                    "signed integer is greater than maximum".into(),
                )));
            }
            usize::try_from(value as u128).map_err(|_| {
                err(pillow_rs::PilError::OverflowError(
                    "signed integer is greater than maximum".into(),
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let layout = pillow_rs::resolve_array_layout(&shape, typestr, mode.as_deref()).map_err(err)?;
    pillow_rs::from_resolved_array_interface(&layout, &data)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "openFn")]
pub fn open_fn(data: Vec<u8>, mode: JsValue, formats: JsValue) -> Result<Image, JsValue> {
    let mode = if mode.is_null() || mode.is_undefined() {
        pillow_rs::PythonOpenModeInput::None
    } else if let Some(name) = mode.as_string() {
        pillow_rs::PythonOpenModeInput::Name(name)
    } else {
        pillow_rs::PythonOpenModeInput::Invalid(js_value_display(&mode))
    };
    let formats = if formats.is_null() || formats.is_undefined() {
        pillow_rs::PythonOpenFormatsInput::None
    } else if formats.is_array() {
        let values = js_sys::Array::from(&formats)
            .iter()
            .map(|value| value.as_string())
            .collect::<Option<Vec<_>>>();
        match values {
            Some(values) => pillow_rs::PythonOpenFormatsInput::Names(values),
            None => pillow_rs::PythonOpenFormatsInput::Invalid(js_value_type_name(&formats)),
        }
    } else {
        pillow_rs::PythonOpenFormatsInput::Invalid(js_value_type_name(&formats))
    };
    let format_names = pillow_rs::validate_python_open_inputs(mode, formats).map_err(err)?;
    let format_refs = format_names
        .as_ref()
        .map(|values| values.iter().map(String::as_str).collect::<Vec<_>>());
    RsImage::open_bytes_with_formats(data, format_refs.as_deref())
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "validateOpenInputs")]
pub fn validate_open_inputs(mode: JsValue, formats: JsValue) -> Result<(), JsValue> {
    let mode = if mode.is_null() || mode.is_undefined() {
        pillow_rs::PythonOpenModeInput::None
    } else if let Some(name) = mode.as_string() {
        pillow_rs::PythonOpenModeInput::Name(name)
    } else {
        pillow_rs::PythonOpenModeInput::Invalid(js_value_display(&mode))
    };
    let formats = if formats.is_null() || formats.is_undefined() {
        pillow_rs::PythonOpenFormatsInput::None
    } else if formats.is_array() {
        let values = js_sys::Array::from(&formats)
            .iter()
            .map(|value| value.as_string())
            .collect::<Option<Vec<_>>>();
        match values {
            Some(values) => pillow_rs::PythonOpenFormatsInput::Names(values),
            None => pillow_rs::PythonOpenFormatsInput::Invalid(js_value_type_name(&formats)),
        }
    } else {
        pillow_rs::PythonOpenFormatsInput::Invalid(js_value_type_name(&formats))
    };
    pillow_rs::validate_python_open_inputs(mode, formats)
        .map(|_| ())
        .map_err(err)
}

#[wasm_bindgen(js_name = "validateOpenSource")]
pub fn validate_open_source(data: Vec<u8>) -> Result<(), JsValue> {
    pillow_rs::validate_python_open_source_bytes(&data)
        .map(|_| ())
        .map_err(err)
}

#[wasm_bindgen(js_name = "imageNewPaletteIndex")]
pub fn image_new_palette_index(w: u32, h: u32, index: u8) -> Image {
    Image {
        inner: RsImage::new_palette_index(w, h, index),
    }
}

// ── ImageChops ───────────────────────────────────────────────────
#[wasm_bindgen]
pub struct ImageChops {}
#[wasm_bindgen]
impl ImageChops {
    #[wasm_bindgen(js_name = "add")]
    pub fn add(
        a: &Image,
        b: &Image,
        scale: Option<f64>,
        offset: Option<f64>,
    ) -> Result<Image, JsValue> {
        pillow_rs::chops_add(
            &a.inner,
            &b.inner,
            scale.unwrap_or(1.0),
            offset.unwrap_or(0.0),
        )
        .map(|i| Image { inner: i })
        .map_err(err)
    }
    #[wasm_bindgen(js_name = "subtract")]
    pub fn sub(
        a: &Image,
        b: &Image,
        scale: Option<f64>,
        offset: Option<f64>,
    ) -> Result<Image, JsValue> {
        pillow_rs::chops_subtract(
            &a.inner,
            &b.inner,
            scale.unwrap_or(1.0),
            offset.unwrap_or(0.0),
        )
        .map(|i| Image { inner: i })
        .map_err(err)
    }
    #[wasm_bindgen(js_name = "multiply")]
    pub fn mul(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_multiply(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "screen")]
    pub fn scr(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_screen(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "darker")]
    pub fn dark(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_darker(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "lighter")]
    pub fn light(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_lighter(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "difference")]
    pub fn diff(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_difference(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "invert")]
    pub fn inv(img: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_invert(&img.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "hardLight")]
    pub fn hard(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_hard_light(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "softLight")]
    pub fn soft(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_soft_light(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "overlay")]
    pub fn over(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_overlay(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "offset")]
    pub fn off(img: &Image, x: i32, y: Option<i32>) -> Result<Image, JsValue> {
        pillow_rs::chops_offset_with_default(&img.inner, x, y)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "addModulo")]
    pub fn addm(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_add_modulo(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "subtractModulo")]
    pub fn subm(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_subtract_modulo(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "blend")]
    pub fn blnd(a: &Image, b: &Image, alpha: f64) -> Result<Image, JsValue> {
        pillow_rs::image_blend(&a.inner, &b.inner, alpha)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "composite")]
    pub fn comp(a: &Image, b: &Image, m: &Image) -> Result<Image, JsValue> {
        pillow_rs::image_composite(&a.inner, &b.inner, &m.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "constant")]
    pub fn cnst(img: &Image, v: u8) -> Result<Image, JsValue> {
        pillow_rs::chops_constant(&img.inner, v)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "duplicate")]
    pub fn dup(img: &Image) -> Image {
        Image {
            inner: img.inner.clone(),
        }
    }
    #[wasm_bindgen(js_name = "logicalAnd")]
    pub fn land(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_logical_and(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "logicalOr")]
    pub fn lor(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_logical_or(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "logicalXor")]
    pub fn lxor(a: &Image, b: &Image) -> Result<Image, JsValue> {
        pillow_rs::chops_logical_xor(&a.inner, &b.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
}

// ── ImageOps ─────────────────────────────────────────────────────
#[wasm_bindgen]
pub struct ImageOps {}
#[wasm_bindgen]
impl ImageOps {
    #[wasm_bindgen(js_name = "invert")]
    pub fn inv(img: &Image) -> Result<Image, JsValue> {
        pillow_rs::imageops_invert(&img.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "flip")]
    pub fn flip(img: &Image) -> Result<Image, JsValue> {
        pillow_rs::imageops_flip(&img.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "mirror")]
    pub fn mirror(img: &Image) -> Result<Image, JsValue> {
        pillow_rs::imageops_mirror(&img.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "grayscale")]
    pub fn gray(img: &Image) -> Result<Image, JsValue> {
        pillow_rs::imageops_grayscale(&img.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "posterize")]
    pub fn post(img: &Image, b: u8) -> Result<Image, JsValue> {
        pillow_rs::imageops_posterize(&img.inner, b)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "solarize")]
    pub fn sol(img: &Image, t: u8) -> Result<Image, JsValue> {
        pillow_rs::imageops_solarize(&img.inner, t)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "equalize")]
    pub fn eq(img: &Image) -> Result<Image, JsValue> {
        pillow_rs::imageops_equalize(&img.inner)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "equalizeWithInput")]
    pub fn eq_with_input(img: &Image, mask: &Image) -> Result<Image, JsValue> {
        pillow_rs::imageops_equalize_with_mask(
            &img.inner,
            pillow_rs::ImageOpsMask::Image(mask.inner.clone()),
        )
        .map(|i| Image { inner: i })
        .map_err(err)
    }
    #[wasm_bindgen(js_name = "autocontrast")]
    pub fn auto(img: &Image, c: f64) -> Result<Image, JsValue> {
        pillow_rs::imageops_autocontrast(&img.inner, c)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "autocontrastWithMask")]
    pub fn auto_with_mask(img: &Image, c: f64, mask: &Image) -> Result<Image, JsValue> {
        pillow_rs::imageops_autocontrast_with_mask(
            &img.inner,
            c,
            pillow_rs::ImageOpsMask::Image(mask.inner.clone()),
        )
        .map(|i| Image { inner: i })
        .map_err(err)
    }
    #[wasm_bindgen(js_name = "autocontrastInvalidMask")]
    pub fn auto_with_invalid_mask(img: &Image, c: f64, type_name: &str) -> Result<Image, JsValue> {
        let mask = pillow_rs::ImageOpsMask::Invalid(type_name.to_owned());
        pillow_rs::imageops_autocontrast_with_mask(&img.inner, c, mask)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "expand")]
    pub fn expand(img: &Image, border: u32, r: u8, g: u8, b: u8, a: u8) -> Result<Image, JsValue> {
        pillow_rs::imageops_expand(&img.inner, border, (r, g, b, a))
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "contain")]
    pub fn contain(img: &Image, w: u32, h: u32) -> Result<Image, JsValue> {
        pillow_rs::imageops_contain(&img.inner, w, h, None)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "containWithInput")]
    pub fn contain_with_input(
        img: &Image,
        w: u32,
        h: u32,
        method: JsValue,
    ) -> Result<Image, JsValue> {
        pillow_rs::imageops_contain_with_input(&img.inner, w, h, js_optional_resample(&method)?)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "cover")]
    pub fn cover(img: &Image, w: u32, h: u32) -> Result<Image, JsValue> {
        pillow_rs::imageops_cover(&img.inner, w, h, None)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "coverWithInput")]
    pub fn cover_with_input(
        img: &Image,
        w: u32,
        h: u32,
        method: JsValue,
    ) -> Result<Image, JsValue> {
        pillow_rs::imageops_cover_with_input(&img.inner, w, h, js_optional_resample(&method)?)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "fit")]
    pub fn fit(img: &Image, w: u32, h: u32) -> Result<Image, JsValue> {
        pillow_rs::imageops_fit(&img.inner, w, h, None, 0.0, (0.5, 0.5))
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "fitWithInput")]
    pub fn fit_with_input(
        img: &Image,
        w: u32,
        h: u32,
        method: JsValue,
        bleed: f64,
        centering: JsValue,
    ) -> Result<Image, JsValue> {
        pillow_rs::imageops_fit_with_input(
            &img.inner,
            w,
            h,
            js_optional_resample(&method)?,
            bleed,
            js_centering(&centering),
        )
        .map(|i| Image { inner: i })
        .map_err(err)
    }
    #[wasm_bindgen(js_name = "pad")]
    pub fn pad(
        img: &Image,
        w: u32,
        h: u32,
        r: Option<u8>,
        g: Option<u8>,
        b: Option<u8>,
        a: Option<u8>,
    ) -> Result<Image, JsValue> {
        let color = r.map(|cr| (cr, g.unwrap_or(0), b.unwrap_or(0), a.unwrap_or(255)));
        pillow_rs::imageops_pad(&img.inner, w, h, None, color, (0.5, 0.5))
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "padWithInput")]
    pub fn pad_with_input(
        img: &Image,
        w: u32,
        h: u32,
        method: JsValue,
        color: JsValue,
        centering: JsValue,
    ) -> Result<Image, JsValue> {
        pillow_rs::imageops_pad_with_input(
            &img.inner,
            w,
            h,
            js_optional_resample(&method)?,
            js_imageops_color(&color),
            js_centering(&centering),
        )
        .map(|i| Image { inner: i })
        .map_err(err)
    }
    #[wasm_bindgen(js_name = "scale")]
    pub fn scale(img: &Image, factor: f64) -> Result<Image, JsValue> {
        pillow_rs::imageops_scale_with_input(&img.inner, factor, None)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "scaleWithInput")]
    pub fn scale_with_input(img: &Image, factor: f64, method: JsValue) -> Result<Image, JsValue> {
        pillow_rs::imageops_scale_with_input(&img.inner, factor, js_optional_resample(&method)?)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "crop")]
    pub fn crop(img: &Image, border: u32) -> Result<Image, JsValue> {
        pillow_rs::imageops_crop(&img.inner, border)
            .map(|i| Image { inner: i })
            .map_err(err)
    }
    #[wasm_bindgen(js_name = "colorize")]
    pub fn colorize(
        img: &Image,
        black_r: u8,
        black_g: u8,
        black_b: u8,
        white_r: u8,
        white_g: u8,
        white_b: u8,
    ) -> Result<Image, JsValue> {
        pillow_rs::imageops_colorize(
            &img.inner,
            (black_r, black_g, black_b),
            (white_r, white_g, white_b),
            None,
            0,
            127,
            255,
        )
        .map(|i| Image { inner: i })
        .map_err(err)
    }
}

#[wasm_bindgen(js_name = "colorizeFn")]
pub fn colorize_fn(
    img: &Image,
    black: JsValue,
    white: JsValue,
    mid: JsValue,
    blackpoint: i32,
    midpoint: i32,
    whitepoint: i32,
) -> Result<Image, JsValue> {
    let black = js_color_triplet(&black)?;
    let white = js_color_triplet(&white)?;
    let mid = if mid.is_null() || mid.is_undefined() {
        None
    } else {
        Some(js_color_triplet(&mid)?)
    };
    if !(0..=255).contains(&blackpoint)
        || !(0..=255).contains(&midpoint)
        || !(0..=255).contains(&whitepoint)
    {
        return Err(err(pillow_rs::PilError::AssertionError(
            "colorize points out of range".to_owned(),
        )));
    }
    pillow_rs::imageops_colorize(
        &img.inner,
        black,
        white,
        mid,
        blackpoint as u8,
        midpoint as u8,
        whitepoint as u8,
    )
    .map(|i| Image { inner: i })
    .map_err(err)
}

// ── Module functions ─────────────────────────────────────────────
#[wasm_bindgen(js_name = "color3DLUTCheckSize")]
pub fn color3dlut_check_size(size: Vec<f64>) -> Result<Vec<u32>, JsValue> {
    pillow_rs::color3dlut_check_size(&size)
        .map(|(x, y, z)| vec![x, y, z])
        .map_err(err)
}

#[wasm_bindgen(js_name = "color3DLUTNew")]
pub fn color3dlut_new(
    table: JsValue,
    size_x: u32,
    size_y: u32,
    size_z: u32,
    channels: u32,
) -> Result<Vec<f64>, JsValue> {
    pillow_rs::color3dlut_prepare_table(
        js_color3dlut_table(&table)?,
        (size_x, size_y, size_z),
        channels,
    )
    .map_err(err)
}

#[wasm_bindgen(js_name = "color3DLUTGenerate")]
pub fn color3dlut_generate(
    size_x: u32,
    size_y: u32,
    size_z: u32,
    channels: u32,
    callback: String,
) -> Result<Vec<f64>, JsValue> {
    pillow_rs::color3dlut_generate_table(
        (size_x, size_y, size_z),
        channels,
        |values| match callback.as_str() {
            "color3dlut-generate-identity" => Ok(values.to_vec()),
            "color3dlut-short-result" => Ok(values.iter().copied().take(2).collect()),
            _ => Err(err(pillow_rs::PilError::NotImplementedError(format!(
                "unsupported Color3DLUT callback: {callback}"
            )))),
        },
        err,
    )
}

#[wasm_bindgen(js_name = "color3DLUTRepr")]
pub fn color3dlut_repr(
    table_type: String,
    size_x: u32,
    size_y: u32,
    size_z: u32,
    channels: u32,
    target_mode: Option<String>,
) -> String {
    pillow_rs::color3dlut_repr(
        &table_type,
        (size_x, size_y, size_z),
        channels,
        target_mode.as_deref(),
    )
}

#[wasm_bindgen(js_name = "merge")]
pub fn merge(mode: &str, bands: Vec<Image>) -> Result<Image, JsValue> {
    let imgs: Vec<RsImage> = bands.iter().map(|b| b.inner.clone()).collect();
    pillow_rs::image_merge(mode, &imgs)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "kernelPrepare")]
pub fn kernel_prepare(
    kernel: Vec<f64>,
    scale: Option<f64>,
    offset: f64,
    size: Vec<u32>,
) -> Result<(), JsValue> {
    let size = match size.as_slice() {
        [width, height] => (*width, *height),
        _ => {
            return Err(err(pillow_rs::PilError::ValueError(
                "bad kernel size".to_owned(),
            )));
        }
    };
    pillow_rs::prepare_kernel(Some(kernel), scale, offset, size)
        .map(|_| ())
        .map_err(err)
}

#[wasm_bindgen(js_name = "mergeWithInput")]
pub fn merge_with_input(
    mode: &str,
    bands: Vec<Image>,
    band_count: u32,
    invalid_type: Option<String>,
) -> Result<Image, JsValue> {
    // Rust-exported wasm-bindgen structs do not implement JsCast. The
    // workflow therefore supplies image handles as a typed Vec and preserves
    // the first non-image's public type separately; validation/order remains
    // in the shared core contract.
    let mut inputs = bands
        .into_iter()
        .map(|image| pillow_rs::MergeInput::Image(image.inner))
        .collect::<Vec<_>>();
    if let Some(type_name) = invalid_type {
        inputs.push(pillow_rs::MergeInput::Invalid(type_name));
    }
    while inputs.len() < band_count as usize {
        inputs.push(pillow_rs::MergeInput::Invalid("object".to_owned()));
    }
    inputs.truncate(band_count as usize);
    pillow_rs::image_merge_inputs(mode, &inputs)
        .map(|i| Image { inner: i })
        .map_err(err)
}
#[wasm_bindgen(js_name = "blend")]
pub fn blend(a: &Image, b: &Image, alpha: f64) -> Result<Image, JsValue> {
    pillow_rs::image_blend(&a.inner, &b.inner, alpha)
        .map(|i| Image { inner: i })
        .map_err(err)
}
#[wasm_bindgen(js_name = "composite")]
pub fn composite(a: &Image, b: &Image, m: &Image) -> Result<Image, JsValue> {
    pillow_rs::image_composite(&a.inner, &b.inner, &m.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

/// Activate a compute backend. Returns true if the backend exists.
#[wasm_bindgen]
pub fn enable_backend(name: &str) -> Result<bool, JsValue> {
    let backend = pillow_rs::Backend::parse(name).ok_or_else(|| {
        err(pillow_rs::PilError::ValueError(format!(
            "unknown backend: {name}"
        )))
    })?;
    pillow_rs::enable_backend(backend).map_err(err)
}

/// Deactivate a compute backend. Returns true if it was active.
#[wasm_bindgen]
pub fn disable_backend(name: &str) -> Result<bool, JsValue> {
    let backend = pillow_rs::Backend::parse(name).ok_or_else(|| {
        err(pillow_rs::PilError::ValueError(format!(
            "unknown backend: {name}"
        )))
    })?;
    pillow_rs::disable_backend(backend).map_err(err)
}

/// List backends that exist on this machine.
#[wasm_bindgen]
pub fn available_backends() -> Vec<String> {
    pillow_rs::available_backends()
        .iter()
        .map(|b| format!("{:?}", b).to_lowercase())
        .collect()
}

/// List currently active backends (priority order).
#[wasm_bindgen]
pub fn active_backends() -> Result<Vec<String>, JsValue> {
    Ok(pillow_rs::active_backends()
        .map_err(err)?
        .into_iter()
        .map(|b| format!("{:?}", b).to_lowercase())
        .collect())
}

/// Check if a specific backend is active.
#[wasm_bindgen]
pub fn backend_enabled(name: &str) -> Result<bool, JsValue> {
    let backend = pillow_rs::Backend::parse(name).ok_or_else(|| {
        err(pillow_rs::PilError::ValueError(format!(
            "unknown backend: {name}"
        )))
    })?;
    pillow_rs::backend_enabled(backend).map_err(err)
}

fn backend_name(backend: pillow_rs::Backend) -> String {
    format!("{backend:?}").to_lowercase()
}

fn set_pipeline_telemetry_field(object: &js_sys::Object, name: &str, value: &JsValue) {
    let _ = js_sys::Reflect::set(object, &JsValue::from_str(name), value);
}

fn pipeline_resource_telemetry_to_js(resource: pillow_rs::PipelineResourceTelemetry) -> JsValue {
    let object = js_sys::Object::new();
    set_pipeline_telemetry_field(
        &object,
        "upload_bytes",
        &JsValue::from_f64(resource.upload_bytes as f64),
    );
    set_pipeline_telemetry_field(
        &object,
        "readback_bytes",
        &JsValue::from_f64(resource.readback_bytes as f64),
    );
    set_pipeline_telemetry_field(
        &object,
        "auxiliary_bytes",
        &JsValue::from_f64(resource.auxiliary_bytes as f64),
    );
    set_pipeline_telemetry_field(
        &object,
        "parameter_bytes",
        &JsValue::from_f64(resource.parameter_bytes as f64),
    );
    set_pipeline_telemetry_field(
        &object,
        "retained_cache_bytes",
        &JsValue::from_f64(resource.retained_cache_bytes as f64),
    );
    set_pipeline_telemetry_field(
        &object,
        "full_frame_copy_count",
        &JsValue::from_f64(resource.full_frame_copy_count as f64),
    );
    set_pipeline_telemetry_field(
        &object,
        "mode_conversion_count",
        &JsValue::from_f64(resource.mode_conversion_count as f64),
    );
    set_pipeline_telemetry_field(
        &object,
        "host_buffer_count",
        &JsValue::from_f64(resource.host_buffer_count as f64),
    );
    set_pipeline_telemetry_field(
        &object,
        "host_buffer_bytes",
        &JsValue::from_f64(resource.host_buffer_bytes as f64),
    );
    set_pipeline_telemetry_field(
        &object,
        "peak_live_host_bytes",
        &JsValue::from_f64(resource.peak_live_host_bytes as f64),
    );
    set_pipeline_telemetry_field(
        &object,
        "fused_operation_count",
        &JsValue::from_f64(resource.fused_operation_count as f64),
    );
    set_pipeline_telemetry_field(
        &object,
        "host_allocation_count",
        &JsValue::from_f64(resource.host_allocation_count as f64),
    );
    set_pipeline_telemetry_field(
        &object,
        "host_allocated_bytes",
        &JsValue::from_f64(resource.host_allocated_bytes as f64),
    );
    object.into()
}

fn pipeline_operation_telemetry_to_js(
    samples: Vec<pillow_rs::PipelineOperationTelemetry>,
) -> JsValue {
    let values = js_sys::Array::new();
    for sample in samples {
        let value = js_sys::Object::new();
        set_pipeline_telemetry_field(&value, "operation", &JsValue::from_str(sample.operation));
        set_pipeline_telemetry_field(&value, "path", &JsValue::from_str(sample.path));
        set_pipeline_telemetry_field(
            &value,
            "vector_block_count",
            &JsValue::from_f64(sample.vector_block_count as f64),
        );
        set_pipeline_telemetry_field(
            &value,
            "scalar_tail_count",
            &JsValue::from_f64(sample.scalar_tail_count as f64),
        );
        set_pipeline_telemetry_field(
            &value,
            "mode_conversion_count",
            &JsValue::from_f64(sample.mode_conversion_count as f64),
        );
        set_pipeline_telemetry_field(
            &value,
            "handoff_count",
            &JsValue::from_f64(sample.handoff_count as f64),
        );
        values.push(&value);
    }
    values.into()
}

/// Enable bounded WASM image-pipeline execution telemetry for parity evidence.
#[wasm_bindgen(js_name = "setPipelineTelemetry")]
pub fn set_pipeline_telemetry(enabled: bool) -> bool {
    pillow_rs::Backend::set_pipeline_telemetry_enabled(enabled)
}

/// Take the most recent completed WASM image-pipeline receipt, or `null`.
#[wasm_bindgen(js_name = "takePipelineTelemetry")]
pub fn take_pipeline_telemetry() -> JsValue {
    let operation_telemetry = pillow_rs::Backend::take_pipeline_operation_telemetry();
    let Some((
        requested_backend,
        actual_backend,
        operation_count,
        route_ns,
        validation_ns,
        backend_ns,
        dispatch_count,
        fallback_reason,
        resource,
        resize_coeff_cache_hits,
        resize_coeff_cache_misses,
    )) = pillow_rs::Backend::take_pipeline_telemetry()
    else {
        if operation_telemetry.is_empty() {
            return JsValue::NULL;
        }
        let object = js_sys::Object::new();
        let operation_telemetry = pipeline_operation_telemetry_to_js(operation_telemetry);
        set_pipeline_telemetry_field(&object, "operation_telemetry", &operation_telemetry);
        return object.into();
    };

    let object = js_sys::Object::new();
    let requested = requested_backend
        .map(backend_name)
        .map_or(JsValue::NULL, |value| JsValue::from_str(&value));
    set_pipeline_telemetry_field(&object, "requested_backend", &requested);
    set_pipeline_telemetry_field(
        &object,
        "actual_backend",
        &JsValue::from_str(&backend_name(actual_backend)),
    );
    set_pipeline_telemetry_field(
        &object,
        "operation_count",
        &JsValue::from_f64(operation_count as f64),
    );
    set_pipeline_telemetry_field(&object, "route_ns", &JsValue::from_f64(route_ns as f64));
    set_pipeline_telemetry_field(
        &object,
        "validation_ns",
        &JsValue::from_f64(validation_ns as f64),
    );
    set_pipeline_telemetry_field(&object, "backend_ns", &JsValue::from_f64(backend_ns as f64));
    let dispatch = dispatch_count.map_or(JsValue::NULL, |value| JsValue::from_f64(value as f64));
    set_pipeline_telemetry_field(&object, "dispatch_count", &dispatch);
    let fallback = fallback_reason.map_or(JsValue::NULL, |value| JsValue::from_str(&value));
    set_pipeline_telemetry_field(&object, "fallback_reason", &fallback);
    let resource = resource.map_or(JsValue::NULL, pipeline_resource_telemetry_to_js);
    set_pipeline_telemetry_field(&object, "resource", &resource);
    set_pipeline_telemetry_field(
        &object,
        "resize_coeff_cache_hits",
        &JsValue::from_f64(resize_coeff_cache_hits as f64),
    );
    set_pipeline_telemetry_field(
        &object,
        "resize_coeff_cache_misses",
        &JsValue::from_f64(resize_coeff_cache_misses as f64),
    );
    let operation_telemetry = pipeline_operation_telemetry_to_js(operation_telemetry);
    set_pipeline_telemetry_field(&object, "operation_telemetry", &operation_telemetry);
    object.into()
}

/// Set the maximum log level shown in the browser console.
/// Levels (ascending): 0=off, 1=error, 2=warn, 3=info, 4=debug, 5=trace.
#[wasm_bindgen(js_name = "setLogLevel")]
pub fn set_log_level(level: u8) {
    #[cfg(feature = "debug-hooks")]
    {
        let lvl = match level {
            0 => log::LevelFilter::Off,
            1 => log::LevelFilter::Error,
            2 => log::LevelFilter::Warn,
            3 => log::LevelFilter::Info,
            4 => log::LevelFilter::Debug,
            5 => log::LevelFilter::Trace,
            _ => log::LevelFilter::Warn,
        };
        log::set_max_level(lvl);
    }

    #[cfg(not(feature = "debug-hooks"))]
    let _ = level;
}

// ══════════════════════════════════════════════════════════════════════════════
// ImageChops — per-pixel channel operations (thin wrappers)
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = "addModulo")]
pub fn add_modulo(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_add_modulo(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "constant")]
pub fn constant(img: &Image, value: u8) -> Result<Image, JsValue> {
    pillow_rs::chops_constant(&img.inner, value)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "darker")]
pub fn darker(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_darker(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "hardLight")]
pub fn hard_light(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_hard_light(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "lighter")]
pub fn lighter(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_lighter(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "logicalAnd")]
pub fn logical_and(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_logical_and(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "logicalOr")]
pub fn logical_or(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_logical_or(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "logicalXor")]
pub fn logical_xor(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_logical_xor(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "multiply")]
pub fn multiply(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_multiply(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "offset")]
pub fn offset(img: &Image, xoffset: i32, yoffset: i32) -> Result<Image, JsValue> {
    pillow_rs::chops_offset(&img.inner, xoffset, yoffset)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "overlay")]
pub fn overlay(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_overlay(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "screenFn")]
pub fn screen(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_screen(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "softLight")]
pub fn soft_light(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_soft_light(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "subtractModulo")]
pub fn subtract_modulo(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::chops_subtract_modulo(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

// ══════════════════════════════════════════════════════════════════════════════
// ImageOps — high-level image operations
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = "autocontrastFn")]
pub fn autocontrast(img: &Image, cutoff: f64) -> Result<Image, JsValue> {
    pillow_rs::imageops_autocontrast(&img.inner, cutoff)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "equalizeFn")]
pub fn equalize(img: &Image) -> Result<Image, JsValue> {
    pillow_rs::imageops_equalize(&img.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "flipFn")]
pub fn flip(img: &Image) -> Result<Image, JsValue> {
    pillow_rs::imageops_flip(&img.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "mirrorFn")]
pub fn mirror(img: &Image) -> Result<Image, JsValue> {
    pillow_rs::imageops_mirror(&img.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "posterizeFn")]
pub fn posterize(img: &Image, bits: u8) -> Result<Image, JsValue> {
    pillow_rs::imageops_posterize(&img.inner, bits)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "solarizeFn")]
pub fn solarize(img: &Image, threshold: u8) -> Result<Image, JsValue> {
    pillow_rs::imageops_solarize(&img.inner, threshold)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "grayscaleFn")]
pub fn grayscale(img: &Image) -> Result<Image, JsValue> {
    pillow_rs::imageops_grayscale(&img.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "expand")]
pub fn expand(
    img: &Image,
    border: u32,
    fill_r: u8,
    fill_g: u8,
    fill_b: u8,
    fill_a: u8,
) -> Result<Image, JsValue> {
    pillow_rs::imageops_expand(&img.inner, border, (fill_r, fill_g, fill_b, fill_a))
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "containFn")]
pub fn contain(img: &Image, w: u32, h: u32) -> Result<Image, JsValue> {
    pillow_rs::imageops_contain(&img.inner, w, h, None)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "coverFn")]
pub fn cover(img: &Image, w: u32, h: u32) -> Result<Image, JsValue> {
    pillow_rs::imageops_cover(&img.inner, w, h, None)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "fitFn")]
pub fn fit(img: &Image, w: u32, h: u32) -> Result<Image, JsValue> {
    pillow_rs::imageops_fit(&img.inner, w, h, None, 0.0, (0.5, 0.5))
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "padFn")]
pub fn pad(img: &Image, w: u32, h: u32, color: Vec<u8>) -> Result<Image, JsValue> {
    let c = match color.len() {
        3 => Some((color[0], color[1], color[2], 255)),
        4 => Some((color[0], color[1], color[2], color[3])),
        _ => None,
    };
    pillow_rs::imageops_pad(&img.inner, w, h, None, c, (0.5, 0.5))
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "cropFn")]
pub fn crop_border(img: &Image, border: u32) -> Result<Image, JsValue> {
    pillow_rs::imageops_crop(&img.inner, border)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "scaleFn")]
pub fn scale(img: &Image, factor: f64) -> Result<Image, JsValue> {
    pillow_rs::imageops_scale_with_input(&img.inner, factor, None)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "alphaCompositeFn")]
pub fn alpha_composite_fn(a: &Image, b: &Image) -> Result<Image, JsValue> {
    pillow_rs::image_alpha_composite(&a.inner, &b.inner)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "exifTransposeFn")]
pub fn exif_transpose_fn(img: &mut Image, in_place: bool) -> Result<JsValue, JsValue> {
    let result = pillow_rs::imageops_exif_transpose(&img.inner, in_place).map_err(err)?;
    if in_place {
        if let Some(transposed) = result {
            img.inner = transposed;
        }
        return Ok(JsValue::NULL);
    }
    Ok(result
        .map(|inner| Image { inner })
        .map_or(JsValue::NULL, Into::into))
}

#[wasm_bindgen(js_name = "exifOrientation")]
pub fn exif_orientation(raw: Vec<u8>) -> Option<u32> {
    pillow_rs::exif_get_orientation(&raw)
}

#[wasm_bindgen(js_name = "exifRemoveOrientation")]
pub fn exif_remove_orientation(raw: Vec<u8>) -> Vec<u8> {
    pillow_rs::exif_remove_orientation(&raw)
}

// ══════════════════════════════════════════════════════════════════════════════
// ImageModule — module-level operations
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = "effectMandelbrot")]
pub fn effect_mandelbrot(
    w: u32,
    h: u32,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    quality: u32,
) -> Result<Image, JsValue> {
    let quality = quality.try_into().map_err(|_| {
        err(pillow_rs::PilError::ValueError(
            "quality exceeds supported Mandelbrot iteration range".to_string(),
        ))
    })?;
    pillow_rs::image_effect_mandelbrot((w, h), (x0, y0, x1, y1), quality)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "effectMandelbrotWithExtent")]
pub fn effect_mandelbrot_with_extent(
    size: JsValue,
    extent: JsValue,
    quality: i32,
) -> Result<Image, JsValue> {
    let size = js_integer_array(&size).ok_or_else(|| {
        err(pillow_rs::PilError::TypeError(
            "argument 1 must be 2-item sequence".to_owned(),
        ))
    })?;
    if size.len() != 2 || size.iter().any(|value| *value < 0) {
        return Err(err(pillow_rs::PilError::ValueError(
            "size must be a non-negative 2-item sequence".to_owned(),
        )));
    }
    let extent_values = js_float_array(&extent);
    let extent_type = js_value_type_name(&extent);
    pillow_rs::image_effect_mandelbrot_with_extent(
        (size[0] as u32, size[1] as u32),
        extent_values.as_deref(),
        &extent_type,
        quality,
    )
    .map(|i| Image { inner: i })
    .map_err(err)
}

#[wasm_bindgen(js_name = "effectNoiseFn")]
pub fn effect_noise(width: u32, height: u32, sigma: f64) -> Result<Image, JsValue> {
    pillow_rs::image_effect_noise_from_size((width, height), sigma)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "effectSpreadFn")]
pub fn effect_spread(img: &Image, distance: u32) -> Result<Image, JsValue> {
    pillow_rs::image_effect_spread(&img.inner, distance)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "evalFn")]
pub fn eval_fn(img: &Image, lut: Vec<u8>, _n_bands: usize) -> Result<Image, JsValue> {
    pillow_rs::image_eval_replicated_for_image(&img.inner, &lut)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "linearGradientFn")]
pub fn linear_gradient(mode: &str) -> Result<Image, JsValue> {
    pillow_rs::image_linear_gradient(mode)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "radialGradientFn")]
pub fn radial_gradient(mode: &str) -> Result<Image, JsValue> {
    pillow_rs::image_radial_gradient(mode)
        .map(|i| Image { inner: i })
        .map_err(err)
}

#[wasm_bindgen(js_name = "mergeFn")]
pub fn merge_fn(mode: &str, bands: Vec<Image>) -> Result<Image, JsValue> {
    let inner_bands: Vec<pillow_rs::Image> = bands.iter().map(|b| b.inner.clone()).collect();
    pillow_rs::image_merge(mode, &inner_bands)
        .map(|i| Image { inner: i })
        .map_err(err)
}

// ══════════════════════════════════════════════════════════════════════════════
// ImageColor — color resolution
// ══════════════════════════════════════════════════════════════════════════════

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

#[wasm_bindgen(js_name = "statFromList")]
pub fn stat_from_list(data: Vec<f64>) -> Result<JsValue, JsValue> {
    let (count, sum, mean, min, max) = pillow_rs::stat_from_list(&data);
    let obj = js_sys::Object::new();
    js_sys::Reflect::set(&obj, &JsValue::from_str("count"), &JsValue::from_f64(count))?;
    js_sys::Reflect::set(&obj, &JsValue::from_str("sum"), &JsValue::from_f64(sum))?;
    js_sys::Reflect::set(&obj, &JsValue::from_str("mean"), &JsValue::from_f64(mean))?;
    js_sys::Reflect::set(&obj, &JsValue::from_str("min"), &JsValue::from_f64(min))?;
    js_sys::Reflect::set(&obj, &JsValue::from_str("max"), &JsValue::from_f64(max))?;
    Ok(obj.into())
}

#[wasm_bindgen(js_name = "statFromHistogram")]
pub fn stat_from_histogram(data: Vec<f64>) -> ImageStat {
    ImageStat {
        inner: pillow_rs::stat_from_histogram(&data),
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Draw helpers
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = "outlineCurve")]
pub fn outline_curve(points: Vec<f64>, steps: i32) -> Result<Vec<i32>, JsValue> {
    let steps = u32::try_from(steps)
        .map_err(|_| JsValue::from_str("steps must be greater than or equal to 0"))?;
    let pts = pillow_rs::outline_curve_points(&points, steps);
    let mut flat = Vec::with_capacity(pts.len() * 2);
    for (x, y) in pts {
        flat.push(x);
        flat.push(y);
    }
    Ok(flat)
}

// ══════════════════════════════════════════════════════════════════════════════
// Color helpers
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = "resolveNewColor")]
pub fn resolve_new_color(
    mode: &str,
    hex: Option<String>,
    single: Option<u8>,
    rgb: Option<Vec<u8>>,
    rgba: Option<Vec<u8>>,
    la: Option<Vec<u8>>,
) -> Result<JsValue, JsValue> {
    let hex = hex.as_deref();
    let rgb = rgb.map(|v| {
        if v.len() == 3 {
            (v[0], v[1], v[2])
        } else {
            (0, 0, 0)
        }
    });
    let rgba = rgba.map(|v| {
        if v.len() == 4 {
            (v[0], v[1], v[2], v[3])
        } else {
            (0, 0, 0, 0)
        }
    });
    let la = la.map(|v| if v.len() == 2 { (v[0], v[1]) } else { (0, 0) });
    let c = pillow_rs::resolve_new_color(mode, hex, single, rgb, rgba, la, None, None)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let arr = js_sys::Array::new();
    arr.push(&JsValue::from(c.0));
    arr.push(&JsValue::from(c.1));
    arr.push(&JsValue::from(c.2));
    arr.push(&JsValue::from(c.3));
    Ok(arr.into())
}
