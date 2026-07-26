use std::{fs, path::Path};

use pillow_rs::{
    Draw, Image, ImageFont, ImageFontTextOptions, ImageFontVariantOptions, ImageFontVariationAxis,
    PilError, PilFont,
};
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

    match operation {
        "load" | "load_path" | "load_default_imagefont" => {
            let font = load_pilfont(case, fixture_root)?;
            return Ok(json!({"type": "pilfont", "mode": font.mode().as_str()}));
        }
        "ImageFont.getbbox" => {
            let font = load_pilfont(case, fixture_root)?;
            let (width, height) = match text_bytes(params)? {
                Some(bytes) => font.getsize(&bytes)?,
                None => font.getsize(text(params)?.as_bytes())?,
            };
            return Ok(json!({"type": "bbox", "value": [0, 0, width, height]}));
        }
        "ImageFont.getlength" => {
            let font = load_pilfont(case, fixture_root)?;
            let (width, _) = match text_bytes(params)? {
                Some(bytes) => font.getsize(&bytes)?,
                None => font.getsize(text(params)?.as_bytes())?,
            };
            return Ok(json!({"type": "length", "value": width}));
        }
        "ImageFont.getmask" => {
            let font = load_pilfont(case, fixture_root)?;
            let mask = match text_bytes(params)? {
                Some(bytes) => font.getmask(&bytes)?,
                None => font.getmask(text(params)?.as_bytes())?,
            };
            return Ok(image_value(
                mask.width,
                mask.height,
                mask.mode.as_str(),
                &mask.pixels,
            ));
        }
        "TransposedFont.getbbox" => {
            let font = load_pilfont(case, fixture_root)?;
            let (width, height) = match text_bytes(params)? {
                Some(bytes) => font.getsize(&bytes)?,
                None => font.getsize(text(params)?.as_bytes())?,
            };
            return Ok(bbox_value(pillow_rs::transposed_bbox(
                (0, 0, width, height),
                orientation(params)?,
            )));
        }
        "TransposedFont.getlength" => {
            let font = load_pilfont(case, fixture_root)?;
            pillow_rs::validate_transposed_length(orientation(params)?)?;
            let (width, _) = match text_bytes(params)? {
                Some(bytes) => font.getsize(&bytes)?,
                None => font.getsize(text(params)?.as_bytes())?,
            };
            return Ok(json!({"type": "length", "value": width}));
        }
        "TransposedFont.getmask" => {
            let font = load_pilfont(case, fixture_root)?;
            let mask = match text_bytes(params)? {
                Some(bytes) => font.getmask(&bytes)?,
                None => font.getmask(text(params)?.as_bytes())?,
            };
            let mut image = mask.to_image()?;
            if let Some(orientation) = orientation(params)? {
                image = image.transpose(orientation)?;
            }
            let (width, height) = image.size()?;
            return Ok(image_value(
                width,
                height,
                image.mode()?.as_str(),
                &image.tobytes_unpacked()?,
            ));
        }
        _ => {}
    }

    let mut font = load_font(case, fixture_root)?;

    match operation {
        "load_default" | "truetype" => font_descriptor(&font),
        "font_size" => Ok(json!({
            "type": "size",
            "value": pillow_rs::font_size(&font),
        })),
        "text_bbox" => {
            let (width, height) = match text_bytes(params)? {
                Some(bytes) => pillow_rs::font_text_bbox_bytes(&font, &bytes)?,
                None => pillow_rs::font_text_bbox(&font, text(params)?)?,
            };
            Ok(json!({
                "type": "size",
                "value": [width, height],
            }))
        }
        "getname" => {
            let (family, style) = pillow_rs::font_getname_optional(&font);
            Ok(json!({"type": "name", "value": [family, style]}))
        }
        "getmetrics" => {
            let (ascent, descent) = pillow_rs::font_getmetrics(&font);
            Ok(json!({"type": "metrics", "value": [ascent, descent]}))
        }
        "getlength" => Ok(json!({
            "type": "length",
            "value": getlength(&font, params)?,
        })),
        "has_variations" => Ok(json!({
            "type": "bool",
            "value": pillow_rs::font_has_variations(&font),
        })),
        "get_variation_axes" => Ok(variation_axes_value(pillow_rs::font_get_variation_axes(
            &font,
        )?)),
        "get_variation_names" => Ok(json!({
            "type": "variation_names",
            "value": pillow_rs::font_get_variation_names(&font)?
                .into_iter()
                .map(|name| hex(&name))
                .collect::<Vec<_>>(),
        })),
        "set_variation_by_name" => {
            let name = variation_name(params)?;
            for _ in 0..repeat_count(params)? {
                pillow_rs::font_set_variation_by_name(&mut font, &name)?;
            }
            Ok(json!({
                "type": "font_after_variation",
                "name": pillow_rs::font_getname(&font),
                "length": getlength(&font, params)?,
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
            pillow_rs::font_set_variation_by_axes(&mut font, &axes)?;
            Ok(json!({
                "type": "font_after_variation",
                "name": pillow_rs::font_getname(&font),
                "length": getlength(&font, params)?,
            }))
        }
        "font_variant" => {
            let options = font_variant_options(case, fixture_root, params)?;
            if uses_font_variant_options(params, case) {
                font_descriptor(&pillow_rs::font_variant_with_options(&font, &options)?)
            } else {
                font_descriptor(&pillow_rs::font_variant(&font, options.size)?)
            }
        }
        "getbbox" => Ok(getbbox(&font, params)?),
        "getbbox_binary" => Ok(bbox_value(match text_bytes(params)? {
            Some(bytes) => pillow_rs::font_getbbox_binary_bytes(&font, &bytes)?,
            None => pillow_rs::font_getbbox_binary(&font, text(params)?)?,
        })),
        "getmask" => {
            let (width, height, pixels) = getmask(&font, params)?;
            Ok(image_value(width, height, "L", &pixels))
        }
        "getmask2" => {
            let (width, height, pixels, offset) = getmask2(&font, params)?;
            Ok(mask_with_offset_value(width, height, "L", &pixels, offset))
        }
        "getmask2_with_start" => {
            let start = pair_f64(required(params, "start")?, "start")?;
            let (width, height, pixels, offset) = match text_bytes(params)? {
                Some(bytes) => pillow_rs::font_getmask2_bytes_with_start(&font, &bytes, start)?,
                None => pillow_rs::font_getmask2_with_start(&font, text(params)?, start)?,
            };
            Ok(mask_with_offset_value(width, height, "L", &pixels, offset))
        }
        "get_transposed_mask" => {
            let (width, height, pixels) =
                pillow_rs::font_get_transposed_mask(&font, text(params)?, orientation(params)?)?;
            Ok(image_value(width, height, "L", &pixels))
        }
        "transposed_bbox" => Ok(bbox_value(pillow_rs::transposed_bbox(
            pillow_rs::font_getbbox(&font, text(params)?)?,
            orientation(params)?,
        ))),
        "validate_transposed_length" => {
            pillow_rs::validate_transposed_length(orientation(params)?)?;
            Ok(json!({
                "type": "length",
                "value": getlength(&font, params)?,
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
                pillow_rs::font_render_text_binary(&font, text(params)?, fill, spacing)?;
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

fn text_bytes(params: &Value) -> Result<Option<Vec<u8>>, PilError> {
    params
        .get("text_bytes_hex")
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| PilError::TypeError("text_bytes_hex must be a string".into()))
                .and_then(hex_to_bytes)
        })
        .transpose()
}

fn variation_name(params: &Value) -> Result<Vec<u8>, PilError> {
    if let Some(value) = params.get("name_bytes_hex") {
        return value
            .as_str()
            .ok_or_else(|| PilError::TypeError("name_bytes_hex must be a string".into()))
            .and_then(hex_to_bytes);
    }
    required(params, "name")?
        .as_str()
        .map(|value| value.as_bytes().to_vec())
        .ok_or_else(|| PilError::TypeError("name must be a string".into()))
}

fn repeat_count(params: &Value) -> Result<usize, PilError> {
    params
        .get("repeat_count")
        .map(|value| {
            value
                .as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| PilError::TypeError("repeat_count must be an integer".into()))
        })
        .transpose()
        .map(|value| value.unwrap_or(1))
}

fn hex_to_bytes(value: &str) -> Result<Vec<u8>, PilError> {
    if value.len() % 2 != 0 {
        return Err(PilError::ValueError(
            "text_bytes_hex must contain an even number of hex digits".into(),
        ));
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_digit(pair[0])?;
            let low = hex_digit(pair[1])?;
            Ok((high << 4) | low)
        })
        .collect()
}

fn hex_digit(value: u8) -> Result<u8, PilError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(PilError::ValueError(
            "text_bytes_hex must contain only hex digits".into(),
        )),
    }
}

fn getlength(font: &ImageFont, params: &Value) -> Result<f32, PilError> {
    match (text_bytes(params)?, has_text_options(params)) {
        (Some(bytes), true) => {
            pillow_rs::font_getlength_bytes_with_options(font, &bytes, &text_options(params)?)
        }
        (Some(bytes), false) => pillow_rs::font_getlength_bytes(font, &bytes),
        (None, true) => {
            pillow_rs::font_getlength_with_options(font, text(params)?, &text_options(params)?)
        }
        (None, false) => pillow_rs::font_getlength(font, text(params)?),
    }
}

fn getbbox(font: &ImageFont, params: &Value) -> Result<Value, PilError> {
    match (text_bytes(params)?, has_text_options(params)) {
        (Some(bytes), true) => Ok(bbox_float_value(
            pillow_rs::font_getbbox_bytes_with_options(font, &bytes, &text_options(params)?)?,
        )),
        (Some(bytes), false) => Ok(bbox_value(pillow_rs::font_getbbox_bytes(font, &bytes)?)),
        (None, true) => Ok(bbox_float_value(pillow_rs::font_getbbox_with_options(
            font,
            text(params)?,
            &text_options(params)?,
        )?)),
        (None, false) => Ok(bbox_value(pillow_rs::font_getbbox(font, text(params)?)?)),
    }
}

fn getmask(font: &ImageFont, params: &Value) -> Result<(u32, u32, Vec<u8>), PilError> {
    match (text_bytes(params)?, has_text_options(params)) {
        (Some(bytes), true) => {
            pillow_rs::font_getmask_bytes_with_options(font, &bytes, &text_options(params)?)
        }
        (Some(bytes), false) => pillow_rs::font_getmask_bytes(font, &bytes),
        (None, true) => {
            pillow_rs::font_getmask_with_options(font, text(params)?, &text_options(params)?)
        }
        (None, false) => pillow_rs::font_getmask(font, text(params)?),
    }
}

fn getmask2(font: &ImageFont, params: &Value) -> Result<(u32, u32, Vec<u8>, (i32, i32)), PilError> {
    match (text_bytes(params)?, has_text_options(params)) {
        (Some(bytes), true) => {
            pillow_rs::font_getmask2_bytes_with_options(font, &bytes, &text_options(params)?)
        }
        (Some(bytes), false) => pillow_rs::font_getmask2_bytes(font, &bytes),
        (None, true) => {
            pillow_rs::font_getmask2_with_options(font, text(params)?, &text_options(params)?)
        }
        (None, false) => pillow_rs::font_getmask2(font, text(params)?),
    }
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
        "ink",
        "args",
        "kwargs",
    ]
    .into_iter()
    .any(|field| params.get(field).is_some())
}

fn text_options(params: &Value) -> Result<ImageFontTextOptions, PilError> {
    Ok(ImageFontTextOptions {
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
        ink: params
            .get("ink")
            .map(|value| {
                value.as_i64().ok_or_else(|| {
                    PilError::TypeError("an integer is required (got type list)".into())
                })
            })
            .transpose()?,
        has_args: params.get("args").is_some(),
        has_kwargs: params.get("kwargs").is_some(),
    })
}

fn uses_font_variant_options(params: &Value, case: &Value) -> bool {
    params.get("variant_index").is_some()
        || params.get("variant_encoding").is_some()
        || params.get("variant_layout_engine").is_some()
        || case
            .get("inputs")
            .and_then(|inputs| inputs.get("assets"))
            .and_then(|assets| assets.get("variant_font"))
            .is_some()
}

fn font_variant_options(
    case: &Value,
    fixture_root: &Path,
    params: &Value,
) -> Result<ImageFontVariantOptions, PilError> {
    let font_bytes = case
        .get("inputs")
        .and_then(|inputs| inputs.get("assets"))
        .and_then(|assets| assets.get("variant_font"))
        .map(|font| {
            let id = required(font, "id")?
                .as_str()
                .ok_or_else(|| PilError::TypeError("variant font id must be a string".into()))?;
            fs::read(fixture_root.join(id))
                .map_err(|_| PilError::OsError("cannot open resource".into()))
        })
        .transpose()?;
    let size = params
        .get("variant_size")
        .map(|value| {
            value
                .as_f64()
                .map(|value| value as f32)
                .ok_or_else(|| PilError::TypeError("variant_size must be a number".into()))
        })
        .transpose()?;
    let index = params
        .get("variant_index")
        .map(|value| {
            value
                .as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| PilError::TypeError("variant_index must be an integer".into()))
        })
        .transpose()?;
    Ok(ImageFontVariantOptions {
        font_bytes,
        size,
        index,
        encoding: optional_string(params, "variant_encoding")?,
        layout_engine: optional_string(params, "variant_layout_engine")?,
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

fn load_font(case: &Value, fixture_root: &Path) -> Result<ImageFont, PilError> {
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
        "load_default" => pillow_rs::font_load_default(size),
        "ref" => {
            let id = required(font, "id")?
                .as_str()
                .ok_or_else(|| PilError::TypeError("font id must be a string".into()))?;
            let data = fs::read(fixture_root.join(id))
                .map_err(|_| PilError::OsError("cannot open resource".into()))?;
            pillow_rs::font_from_bytes(data, size)
        }
        kind => Err(PilError::ValueError(format!(
            "unsupported font fixture font kind: {kind}"
        ))),
    }
}

fn load_pilfont(case: &Value, fixture_root: &Path) -> Result<PilFont, PilError> {
    let font = inputs(case)?
        .get("assets")
        .and_then(|assets| assets.get("font"))
        .ok_or_else(|| PilError::ValueError("case.inputs.assets.font missing".into()))?;
    let kind = required(font, "kind")?
        .as_str()
        .ok_or_else(|| PilError::TypeError("font asset kind must be a string".into()))?;
    match kind {
        "pilfont_default" => PilFont::load_default(),
        "pilfont_ref" => {
            let id = required(font, "id")?
                .as_str()
                .ok_or_else(|| PilError::TypeError("font asset id must be a string".into()))?;
            let metrics_path = fixture_root.join(id);
            let metrics = fs::read(&metrics_path).map_err(|error| {
                PilError::IOError(format!("failed to read PILfont metrics fixture: {error}"))
            })?;
            let image = load_pilfont_glyph_image(&metrics_path)?;
            PilFont::from_pilfont_data(&metrics, image).or_else(|error| match error {
                PilError::TypeError(message) if message == "invalid font image mode" => {
                    Err(cannot_find_glyph_data_error(&metrics_path))
                }
                other => Err(other),
            })
        }
        other => Err(PilError::ValueError(format!(
            "unsupported bitmap ImageFont asset kind: {other}"
        ))),
    }
}

fn load_pilfont_glyph_image(metrics_path: &Path) -> Result<Image, PilError> {
    let mut last_image: Option<Image> = None;
    for extension in ["png", "gif", "pbm"] {
        let bitmap_path = metrics_path.with_extension(extension);
        let Ok(bitmap) = fs::read(&bitmap_path) else {
            continue;
        };
        let image = match PilFont::open_glyph_image(bitmap.clone()) {
            Ok(image) => image,
            Err(error) if should_surface_pilfont_image_load_error(&error) => return Err(error),
            Err(_) => continue,
        };
        let mode = image.mode()?;
        if matches!(mode.as_str(), "1" | "L") {
            return Ok(image);
        }
        last_image = Some(image);
    }
    let _ = last_image;
    Err(cannot_find_glyph_data_error(metrics_path))
}

fn should_surface_pilfont_image_load_error(error: &PilError) -> bool {
    match error {
        PilError::ValueError(message) => message.starts_with("b'Invalid token for this mode: "),
        PilError::IOError(message) => message == "image file is truncated (0 bytes not processed)",
        _ => false,
    }
}

fn cannot_find_glyph_data_error(metrics_path: &Path) -> PilError {
    let root = metrics_path.with_extension("");
    PilError::IOError(format!(
        "cannot find glyph data file {}.{{gif|pbm|png}}",
        root.display()
    ))
}

fn draw_text(font: &ImageFont, params: &Value) -> Result<Value, PilError> {
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

fn font_descriptor(font: &ImageFont) -> Result<Value, PilError> {
    let (family, style) = pillow_rs::font_getname_optional(font);
    let (ascent, descent) = pillow_rs::font_getmetrics(font);
    Ok(json!({
        "type": "font",
        "size": pillow_rs::font_size(font),
        "name": [family, style],
        "metrics": [ascent, descent],
        "has_variations": pillow_rs::font_has_variations(font),
    }))
}

fn variation_axes_value(axes: Vec<ImageFontVariationAxis>) -> Value {
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
