use wasm_bindgen::prelude::JsValue;

pub(crate) fn err(e: pillow_rs::PilError) -> JsValue {
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
        pillow_rs::PilError::MemoryError(_) => "MemoryError",
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

pub(crate) fn js_optional_string(value: &JsValue, name: &str) -> Result<Option<String>, JsValue> {
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }
    value.as_string().map(Some).ok_or_else(|| {
        err(pillow_rs::PilError::TypeError(format!(
            "{name} must be a string or None"
        )))
    })
}

pub(crate) fn formatted_pixel_to_js(value: pillow_rs::FormattedPixelValue) -> JsValue {
    match value {
        pillow_rs::FormattedPixelValue::Scalar(value) => JsValue::from(value),
        pillow_rs::FormattedPixelValue::Integer(value) => JsValue::from(value),
        pillow_rs::FormattedPixelValue::Float(value) => JsValue::from_f64(value),
        pillow_rs::FormattedPixelValue::Components(values) => {
            js_sys::Array::from_iter(values.into_iter().map(JsValue::from)).into()
        }
        pillow_rs::FormattedPixelValue::InlineComponents { bytes, bands } => {
            js_sys::Array::from_iter(
                bytes
                    .into_iter()
                    .take(usize::from(bands))
                    .map(JsValue::from),
            )
            .into()
        }
    }
}

pub(crate) fn formatted_image_data_to_js(value: pillow_rs::FormattedImageData) -> JsValue {
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

pub(crate) fn formatted_extrema_to_js(value: pillow_rs::FormattedExtrema) -> JsValue {
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

pub(crate) fn image_info_value_to_js(value: pillow_rs::ImageInfoValue) -> JsValue {
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

pub(crate) fn image_info_to_js(fields: Vec<(String, pillow_rs::ImageInfoValue)>) -> JsValue {
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

pub(crate) fn js_value_type_name(value: &JsValue) -> String {
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

pub(crate) fn js_value_display(value: &JsValue) -> String {
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

pub(crate) fn js_integer(value: &JsValue) -> Option<i64> {
    let number = value.as_f64()?;
    if number.is_finite() && number.fract() == 0.0 {
        Some(number as i64)
    } else {
        None
    }
}

pub(crate) fn js_integer_array(value: &JsValue) -> Option<Vec<i64>> {
    if !value.is_array() {
        return None;
    }
    js_sys::Array::from(value)
        .iter()
        .map(|item| js_integer(&item))
        .collect()
}

pub(crate) fn js_float_array(value: &JsValue) -> Option<Vec<f64>> {
    if !value.is_array() {
        return None;
    }
    js_sys::Array::from(value)
        .iter()
        .map(|item| item.as_f64())
        .collect()
}

pub(crate) fn call_float_callback(
    callback: &js_sys::Function,
    values: &[f64],
) -> Result<Vec<f64>, JsValue> {
    let arguments = js_sys::Array::new();
    for value in values {
        arguments.push(&JsValue::from_f64(*value));
    }
    let result = callback.apply(&JsValue::NULL, &arguments)?;
    if let Some(value) = result.as_f64() {
        return Ok(vec![value]);
    }
    js_float_array(&result).ok_or_else(|| {
        err(pillow_rs::PilError::TypeError(
            "Color3DLUT callback must return a number sequence".to_owned(),
        ))
    })
}
