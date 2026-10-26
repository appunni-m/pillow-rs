use super::JsValue;
use crate::{err, js_float_array};
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
