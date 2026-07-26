use std::{fs, path::Path};

use pillow_rs::{
    draw::Draw,
    error::PilError,
    font::{Font, imagingft},
    image::Image,
};
use serde_json::{Value, json};

pub fn operation(case: &Value) -> Result<&str, PilError> {
    case.get("operation")
        .and_then(Value::as_str)
        .ok_or_else(|| PilError::ValueError("case operation must be a string".into()))
        .map(|operation| operation.strip_prefix("imagingft.").unwrap_or(operation))
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
    let font = load_font(case, fixture_root)?;

    match operation {
        "getname" => {
            let (family, style) = imagingft::getname(&font);
            Ok(json!({"type": "name", "value": [family, style]}))
        }
        "getmetrics" => {
            let (ascent, descent) = imagingft::getmetrics(&font);
            Ok(json!({"type": "metrics", "value": [ascent, descent]}))
        }
        "getlength" => Ok(json!({
            "type": "length",
            "value": imagingft::getlength(&font, text(params)?),
        })),
        "has_variations" => Ok(json!({
            "type": "bool",
            "value": imagingft::has_variations(&font),
        })),
        "getbbox" => Ok(bbox_value(imagingft::getbbox_result(&font, text(params)?)?)),
        "getbbox_binary" => Ok(bbox_value(imagingft::getbbox_binary_result(
            &font,
            text(params)?,
        )?)),
        "getmask" => {
            let (width, height, pixels) = imagingft::getmask(&font, text(params)?);
            Ok(image_value(width, height, "L", &pixels))
        }
        "getmask2" => {
            let (width, height, pixels, offset) = imagingft::getmask2_result(&font, text(params)?)?;
            Ok(mask_with_offset_value(width, height, "L", &pixels, offset))
        }
        "getmask2_with_start" => {
            let start = pair_f64(required(params, "start")?, "start")?;
            let (width, height, pixels, offset) =
                imagingft::getmask2_with_start_result(&font, text(params)?, start)?;
            Ok(mask_with_offset_value(width, height, "L", &pixels, offset))
        }
        "get_transposed_mask" => {
            let (width, height, pixels) =
                imagingft::get_transposed_mask(&font, text(params)?, orientation(params)?)?;
            Ok(image_value(width, height, "L", &pixels))
        }
        "transposed_bbox" => Ok(bbox_value(imagingft::transposed_bbox(
            imagingft::getbbox(&font, text(params)?),
            orientation(params)?,
        ))),
        "validate_transposed_length" => {
            imagingft::validate_transposed_length(orientation(params)?)?;
            Ok(json!({
                "type": "length",
                "value": imagingft::getlength(&font, text(params)?),
            }))
        }
        "draw_text" => draw_text(&font, params),
        "render_text_binary" => {
            let fill = fill(params)?;
            let spacing = required(params, "spacing")?
                .as_f64()
                .ok_or_else(|| PilError::ValueError("spacing must be a number".into()))?
                as f32;
            let (width, height, pixels) =
                imagingft::render_text_binary(&font, text(params)?, fill, spacing);
            Ok(image_value(width, height, "RGBA", &pixels))
        }
        other => Err(PilError::NotImplementedError(format!(
            "unsupported imagingft operation: {other}"
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
            "unsupported imagingft fixture font kind: {kind}"
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
    let pixels = draw.image_clone().tobytes()?;
    Ok(image_value(width, height, mode, &pixels))
}

fn bbox_value((left, top, right, bottom): (i32, i32, i32, i32)) -> Value {
    json!({"type": "bbox", "value": [left, top, right, bottom]})
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
