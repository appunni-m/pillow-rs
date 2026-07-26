use std::{fs, path::Path};

use pillow_rs::{Draw, Font, FontTextOptions, FontVariationAxis, Image, PilError};
use serde_json::{Value, json};

pub fn operation(case: &Value) -> Result<&str, PilError> {
    case.get("operation")
        .and_then(Value::as_str)
        .ok_or_else(|| PilError::ValueError("case operation must be a string".into()))
        .map(|operation| operation.strip_prefix("font.").unwrap_or(operation))
}

pub fn run(case: &Value, fixture_root: &Path) -> Value {
    match try_run(case, fixture_root) {
        Ok(value) => json!({"status": "ok", "value": value}),
        Err(error) => json!({
            "status": "error",
            "error": {
                "kind": error_kind(&error),
                "message": error.to_string(),
            }
        }),
    }
}

fn try_run(case: &Value, fixture_root: &Path) -> Result<Value, PilError> {
    let operation = operation(case)?;
    let params = inputs(case)?
        .get("params")
        .ok_or_else(|| PilError::ValueError("case.inputs.params missing".into()))?;
    let mut font = load_font(case, fixture_root)?;

    match operation {
        "load_default" | "truetype" => font_descriptor(&font),
        "font_size" => Ok(json!({
            "type": "size",
            "value": font.font_size(),
        })),
        "text_bbox" => {
            let (width, height) = font.text_bbox(text(params)?)?;
            Ok(json!({
                "type": "size",
                "value": [width, height],
            }))
        }
        "getname" => {
            let (family, style) = font.getname_optional();
            Ok(json!({"type": "name", "value": [family, style]}))
        }
        "getmetrics" => {
            let (ascent, descent) = font.getmetrics();
            Ok(json!({"type": "metrics", "value": [ascent, descent]}))
        }
        "getlength" => Ok(json!({
            "type": "length",
            "value": if has_text_options(params) {
                pillow_rs::font_getlength_with_options(&font, text(params)?, &text_options(params)?)?
            } else {
                pillow_rs::font_getlength(&font, text(params)?)?
            },
        })),
        "has_variations" => Ok(json!({
            "type": "bool",
            "value": font.has_variations(),
        })),
        "get_variation_axes" => Ok(variation_axes_value(font.get_variation_axes()?)),
        "get_variation_names" => Ok(json!({
            "type": "variation_names",
            "value": font
                .get_variation_names()?
                .into_iter()
                .map(|name| hex(&name))
                .collect::<Vec<_>>(),
        })),
        "set_variation_by_name" => {
            let name = required(params, "name")?
                .as_str()
                .ok_or_else(|| PilError::TypeError("name must be a string".into()))?;
            font.set_variation_by_name(name.as_bytes())?;
            Ok(json!({
                "type": "font_after_variation",
                "name": font.getname(),
                "length": font.getlength(text(params)?)?,
            }))
        }
        "set_variation_by_axes" => {
            let axes = required(params, "axes")?
                .as_array()
                .ok_or_else(|| PilError::TypeError("axes must be a list".into()))?
                .iter()
                .map(|value| {
                    value
                        .as_f64()
                        .map(|value| value as f32)
                        .ok_or_else(|| PilError::TypeError("axis must be a number".into()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            font.set_variation_by_axes(&axes)?;
            Ok(json!({
                "type": "font_after_variation",
                "name": font.getname(),
                "length": font.getlength(text(params)?)?,
            }))
        }
        "font_variant" => {
            let size = params
                .get("variant_size")
                .map(|value| {
                    value
                        .as_f64()
                        .map(|value| value as f32)
                        .ok_or_else(|| PilError::TypeError("variant_size must be a number".into()))
                })
                .transpose()?;
            font_descriptor(&font.font_variant(size)?)
        }
        "getbbox" => {
            if has_text_options(params) {
                Ok(bbox_float_value(pillow_rs::font_getbbox_with_options(
                    &font,
                    text(params)?,
                    &text_options(params)?,
                )?))
            } else {
                Ok(bbox_value(font.getbbox(text(params)?)?))
            }
        }
        "getbbox_binary" => Ok(bbox_value(font.getbbox_binary(text(params)?)?)),
        "getmask" => {
            let (width, height, pixels) = font.getmask(text(params)?)?;
            Ok(image_value(width, height, "L", &pixels))
        }
        "getmask2" => {
            let (width, height, pixels, offset) = if has_text_options(params) {
                pillow_rs::font_getmask2_with_options(&font, text(params)?, &text_options(params)?)?
            } else {
                font.getmask2(text(params)?)?
            };
            Ok(mask_with_offset_value(width, height, "L", &pixels, offset))
        }
        "getmask2_with_start" => {
            let start = pair_f64(required(params, "start")?, "start")?;
            let (width, height, pixels, offset) = font.getmask2_with_start(text(params)?, start)?;
            Ok(mask_with_offset_value(width, height, "L", &pixels, offset))
        }
        "get_transposed_mask" => {
            let (width, height, pixels) =
                font.get_transposed_mask(text(params)?, orientation(params)?)?;
            Ok(image_value(width, height, "L", &pixels))
        }
        "transposed_bbox" => Ok(bbox_value(pillow_rs::transposed_bbox(
            font.getbbox(text(params)?)?,
            orientation(params)?,
        ))),
        "validate_transposed_length" => {
            pillow_rs::validate_transposed_length(orientation(params)?)?;
            Ok(json!({
                "type": "length",
                "value": font.getlength(text(params)?)?,
            }))
        }
        "draw_text" => draw_text(&font, params),
        "render_text_binary" => {
            let fill = fill(params)?;
            let spacing = required(params, "spacing")?
                .as_f64()
                .ok_or_else(|| PilError::ValueError("spacing must be a number".into()))?
                as f32;
            let (width, height, pixels) = font.render_text_binary(text(params)?, fill, spacing)?;
            Ok(image_value(width, height, "RGBA", &pixels))
        }
        other => Err(PilError::NotImplementedError(format!(
            "unsupported font operation: {other}"
        ))),
    }
}

fn inputs(case: &Value) -> Result<&Value, PilError> {
    case.get("inputs")
        .ok_or_else(|| PilError::ValueError("case.inputs missing".into()))
}

fn required<'a>(object: &'a Value, field: &str) -> Result<&'a Value, PilError> {
    object
        .get(field)
        .ok_or_else(|| PilError::ValueError(format!("missing field: {field}")))
}

fn text(params: &Value) -> Result<&str, PilError> {
    required(params, "text")?
        .as_str()
        .ok_or_else(|| PilError::TypeError("text must be a string".into()))
}

fn orientation(params: &Value) -> Result<Option<&str>, PilError> {
    let value = match params.get("orientation") {
        Some(value) => value,
        None => return Ok(None),
    };
    if value.is_null() {
        return Ok(None);
    }
    let Some(method) = value.as_str() else {
        return Err(PilError::TypeError(
            "'str' object cannot be interpreted as an integer".into(),
        ));
    };
    if matches!(
        method,
        "FLIP_LEFT_RIGHT"
            | "FLIP_TOP_BOTTOM"
            | "ROTATE_90"
            | "ROTATE_180"
            | "ROTATE_270"
            | "TRANSPOSE"
            | "TRANSVERSE"
    ) {
        Ok(Some(method))
    } else {
        Err(PilError::TypeError(
            "an integer is required (got type str)".into(),
        ))
    }
}

fn has_text_options(params: &Value) -> bool {
    [
        "mode",
        "direction",
        "features",
        "language",
        "stroke_width",
        "anchor",
        "start",
    ]
    .into_iter()
    .any(|field| params.get(field).is_some())
}

fn text_options(params: &Value) -> Result<FontTextOptions, PilError> {
    Ok(FontTextOptions {
        mode: optional_string(params, "mode")?,
        direction: optional_string(params, "direction")?,
        features: match params.get("features") {
            Some(Value::Null) | None => None,
            Some(value) => Some(
                value
                    .as_array()
                    .ok_or_else(|| PilError::TypeError("features must be a list".into()))?
                    .iter()
                    .map(|item| {
                        item.as_str()
                            .map(str::to_owned)
                            .ok_or_else(|| PilError::TypeError("feature must be a string".into()))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        },
        language: optional_string(params, "language")?,
        stroke_width: params
            .get("stroke_width")
            .map(|value| {
                value
                    .as_f64()
                    .map(|value| value as f32)
                    .ok_or_else(|| PilError::TypeError("stroke_width must be a number".into()))
            })
            .transpose()?
            .unwrap_or(0.0),
        anchor: optional_string(params, "anchor")?,
        start: params
            .get("start")
            .map(|value| pair_f64(value, "start"))
            .transpose()?,
    })
}

fn optional_string(params: &Value, field: &str) -> Result<Option<String>, PilError> {
    match params.get(field) {
        Some(Value::Null) | None => Ok(None),
        Some(value) => value
            .as_str()
            .map(|value| Some(value.to_owned()))
            .ok_or_else(|| PilError::TypeError(format!("{field} must be a string"))),
    }
}

fn pair_f64(value: &Value, name: &str) -> Result<(f64, f64), PilError> {
    let values = value
        .as_array()
        .filter(|values| values.len() == 2)
        .ok_or_else(|| PilError::ValueError(format!("{name} must contain two numbers")))?;
    let first = values[0]
        .as_f64()
        .ok_or_else(|| PilError::TypeError(format!("{name}[0] must be a number")))?;
    let second = values[1]
        .as_f64()
        .ok_or_else(|| PilError::TypeError(format!("{name}[1] must be a number")))?;
    Ok((first, second))
}

fn pair_i32(value: &Value, name: &str) -> Result<(i32, i32), PilError> {
    let values = value
        .as_array()
        .filter(|values| values.len() == 2)
        .ok_or_else(|| PilError::ValueError(format!("{name} must contain two integers")))?;
    let parse = |index: usize| {
        values[index]
            .as_i64()
            .and_then(|value| i32::try_from(value).ok())
            .ok_or_else(|| PilError::TypeError(format!("{name}[{index}] must be an integer")))
    };
    Ok((parse(0)?, parse(1)?))
}

fn u32_field(params: &Value, field: &str) -> Result<u32, PilError> {
    required(params, field)?
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| PilError::TypeError(format!("{field} must be an unsigned integer")))
}

fn fill(params: &Value) -> Result<(u8, u8, u8, u8), PilError> {
    let values = required(params, "fill")?
        .as_array()
        .filter(|values| matches!(values.len(), 3 | 4))
        .ok_or_else(|| PilError::ValueError("fill must contain three or four channels".into()))?;
    let channel = |index: usize| {
        values[index]
            .as_u64()
            .and_then(|value| u8::try_from(value).ok())
            .ok_or_else(|| PilError::TypeError(format!("fill[{index}] must be u8")))
    };
    Ok((
        channel(0)?,
        channel(1)?,
        channel(2)?,
        if values.len() == 4 { channel(3)? } else { 255 },
    ))
}

fn load_font(case: &Value, fixture_root: &Path) -> Result<Font, PilError> {
    let inputs = inputs(case)?;
    let params = required(inputs, "params")?;
    let size = params.get("size").map_or(Ok(10.0), |value| {
        value
            .as_f64()
            .map(|value| value as f32)
            .ok_or_else(|| PilError::TypeError("size must be a number".into()))
    })?;
    let font = required(required(inputs, "assets")?, "font")?;
    match required(font, "kind")?
        .as_str()
        .ok_or_else(|| PilError::TypeError("font kind must be a string".into()))?
    {
        "load_default" => Font::load_default(size),
        "ref" => {
            let id = required(font, "id")?
                .as_str()
                .ok_or_else(|| PilError::TypeError("font id must be a string".into()))?;
            let data = fs::read(fixture_root.join(id))
                .map_err(|_| PilError::OsError("cannot open resource".into()))?;
            Font::from_bytes(data, size)
        }
        kind => Err(PilError::ValueError(format!(
            "unsupported font fixture font kind: {kind}"
        ))),
    }
}

fn draw_text(font: &Font, params: &Value) -> Result<Value, PilError> {
    let width = u32_field(params, "canvas_width")?;
    let height = u32_field(params, "canvas_height")?;
    let (x, y) = pair_i32(required(params, "xy")?, "xy")?;
    let mode = required(params, "mode")?
        .as_str()
        .ok_or_else(|| PilError::TypeError("mode must be a string".into()))?;
    let mut draw = Draw::new(
        Image::new(width, height, mode, (0, 0, 0, 0))?,
        Some(mode.to_string()),
    );
    draw.text(x, y, text(params)?, font, fill(params)?)?;
    let pixels = draw.image_clone()?.tobytes()?;
    Ok(image_value(width, height, mode, &pixels))
}

fn font_descriptor(font: &Font) -> Result<Value, PilError> {
    let (family, style) = font.getname_optional();
    let (ascent, descent) = font.getmetrics();
    Ok(json!({
        "type": "font",
        "size": font.font_size(),
        "name": [family, style],
        "metrics": [ascent, descent],
        "has_variations": font.has_variations(),
    }))
}

fn variation_axes_value(axes: Vec<FontVariationAxis>) -> Value {
    json!({
        "type": "variation_axes",
        "value": axes
            .into_iter()
            .map(|axis| {
                json!({
                    "minimum": axis.minimum,
                    "default": axis.default,
                    "maximum": axis.maximum,
                    "name_hex": hex(&axis.name),
                })
            })
            .collect::<Vec<_>>(),
    })
}

fn bbox_value((left, top, right, bottom): (i32, i32, i32, i32)) -> Value {
    json!({"type": "bbox", "value": [left, top, right, bottom]})
}

fn bbox_float_value((left, top, right, bottom): (f32, f32, f32, f32)) -> Value {
    json!({
        "type": "bbox",
        "value": [
            json_number(left),
            json_number(top),
            json_number(right),
            json_number(bottom),
        ],
    })
}

fn json_number(value: f32) -> Value {
    if value.fract() == 0.0 {
        json!(value as i32)
    } else {
        json!(value)
    }
}

fn image_value(width: u32, height: u32, mode: &str, pixels: &[u8]) -> Value {
    json!({
        "type": "image",
        "size": [width, height],
        "mode": mode,
        "pixels_hex": hex(pixels),
    })
}

fn mask_with_offset_value(
    width: u32,
    height: u32,
    mode: &str,
    pixels: &[u8],
    offset: (i32, i32),
) -> Value {
    json!({
        "type": "image_with_offset",
        "size": [width, height],
        "mode": mode,
        "pixels_hex": hex(pixels),
        "offset": [offset.0, offset.1],
    })
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

fn error_kind(error: &PilError) -> &'static str {
    match error {
        PilError::IOError(_) | PilError::Io(_) => "OSError",
        PilError::OsError(_) => "OSError",
        PilError::AssertionError(_) => "AssertionError",
        PilError::IndexError(_) => "IndexError",
        PilError::KeyError(_) => "KeyError",
        PilError::UnidentifiedImageError(_) => "UnidentifiedImageError",
        PilError::ValueError(_) | PilError::DimensionError(_) => "ValueError",
        PilError::SyntaxError(_) => "SyntaxError",
        PilError::TypeError(_) => "TypeError",
        PilError::NotImplementedError(_) => "NotImplementedError",
        PilError::ImageError(_)
        | PilError::UnknownFormat(_)
        | PilError::PaletteError(_)
        | PilError::InternalError(_) => "ValueError",
    }
}
