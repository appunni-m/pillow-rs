use std::collections::BTreeSet;
use std::{fs, path::PathBuf};

use pillow_rs::{
    draw::Draw,
    error::PilError,
    font::{Font, imagingft},
    image::Image,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

fn crate_fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/imagingft")
}

enum ApiValue {
    Name((String, String)),
    Metrics((u32, u32)),
    Length(f32),
    Bool(bool),
    BBox((i32, i32, i32, i32)),
    Mask {
        size: (u32, u32),
        mode: String,
        pixels: Vec<u8>,
    },
    MaskWithOffset {
        size: (u32, u32),
        mode: String,
        pixels: Vec<u8>,
        offset: (i32, i32),
    },
    Unit,
}

fn artifact_path(raw_path: &str) -> PathBuf {
    let candidate = PathBuf::from(raw_path);
    if candidate.is_relative() {
        crate_fixture_dir().join(candidate)
    } else {
        candidate
    }
}

fn sha256_hex(data: &[u8]) -> String {
    let digest: [u8; 32] = Sha256::digest(data).into();
    digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

fn to_hex(data: &[u8]) -> String {
    data.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn parse_orientation(value: &Value) -> Option<&str> {
    value.as_str().filter(|value| !value.is_empty())
}

fn parse_start(value: &Value) -> Result<(f64, f64), PilError> {
    let coords = value.as_array().ok_or(PilError::ValueError(
        "start must be an array of two numbers".into(),
    ))?;
    if coords.len() != 2 {
        return Err(PilError::ValueError(
            "start must be an array of exactly two numbers".into(),
        ));
    }
    let x = coords[0]
        .as_f64()
        .ok_or(PilError::ValueError("start[0] must be a number".into()))?;
    let y = coords[1]
        .as_f64()
        .ok_or(PilError::ValueError("start[1] must be a number".into()))?;
    Ok((x, y))
}

fn parse_text(value: &Value) -> Result<&str, PilError> {
    value
        .as_str()
        .ok_or_else(|| PilError::ValueError("text must be a string".into()))
}

fn parse_size_u32(value: &Value) -> Result<u32, PilError> {
    let value = value.as_u64().ok_or(PilError::ValueError(
        "size value must be an unsigned integer".into(),
    ))?;
    u32::try_from(value).map_err(|_| PilError::ValueError("size must fit u32".into()))
}

fn parse_u8_value(value: &Value, index: usize) -> Result<u8, PilError> {
    value
        .as_u64()
        .and_then(|v| u8::try_from(v).ok())
        .ok_or_else(|| PilError::ValueError(format!("fill[{index}] must be u8")))
}

fn parse_fill(value: &Value) -> Result<(u8, u8, u8, u8), PilError> {
    let fill = value
        .as_array()
        .ok_or(PilError::ValueError("fill must be an array".into()))?;
    if fill.len() != 3 && fill.len() != 4 {
        return Err(PilError::ValueError(
            "fill must be [r, g, b] or [r, g, b, a]".into(),
        ));
    }
    let r = parse_u8_value(&fill[0], 0)?;
    let g = parse_u8_value(&fill[1], 1)?;
    let b = parse_u8_value(&fill[2], 2)?;
    let a = if fill.len() == 4 {
        parse_u8_value(&fill[3], 3)?
    } else {
        255
    };
    Ok((r, g, b, a))
}

fn expected_status(case: &Value) -> String {
    let expectation = case.get("expectation").unwrap_or(&Value::Null);

    if let Some(status) = expectation
        .get("expected")
        .and_then(|expected| expected.get("status"))
        .and_then(Value::as_str)
    {
        return status.to_string();
    }

    if let Some(status) = expectation.get("status").and_then(Value::as_str) {
        return status.to_string();
    }

    if let Some(status) = case.get("status").and_then(Value::as_str) {
        return status.to_string();
    }

    if case
        .get("expect_error")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        "error".to_string()
    } else {
        "ok".to_string()
    }
}

const REQUIRED_PUBLIC_OPS: [&str; 13] = [
    "getname",
    "getmetrics",
    "getlength",
    "has_variations",
    "getbbox",
    "getbbox_binary",
    "getmask",
    "getmask2",
    "getmask2_with_start",
    "get_transposed_mask",
    "transposed_bbox",
    "validate_transposed_length",
    "draw_text",
];

fn parse_xy(value: &Value) -> Result<(i32, i32), PilError> {
    let coords = value.as_array().ok_or(PilError::ValueError(
        "draw_text xy must be an array of two integers".into(),
    ))?;
    if coords.len() != 2 {
        return Err(PilError::ValueError(
            "draw_text xy must be an array of two integers".into(),
        ));
    }
    let x = coords[0]
        .as_i64()
        .ok_or(PilError::ValueError("xy[0] must be integer".into()))?;
    let y = coords[1]
        .as_i64()
        .ok_or(PilError::ValueError("xy[1] must be integer".into()))?;
    Ok((
        i32::try_from(x).map_err(|_| PilError::ValueError("xy[0] out of i32 range".into()))?,
        i32::try_from(y).map_err(|_| PilError::ValueError("xy[1] out of i32 range".into()))?,
    ))
}

fn load_font(case: &Value) -> Result<Font, PilError> {
    let inputs = case
        .get("inputs")
        .ok_or(PilError::ValueError("case inputs missing".into()))?;
    let font = inputs
        .get("assets")
        .and_then(|v| v.get("font"))
        .and_then(Value::as_object)
        .ok_or(PilError::ValueError("font asset must be an object".into()))?;
    let kind = font
        .get("kind")
        .and_then(Value::as_str)
        .ok_or(PilError::ValueError("font kind must be a string".into()))?;
    let size = inputs
        .get("params")
        .and_then(|p| p.get("size"))
        .and_then(Value::as_f64)
        .unwrap_or(10.0) as f32;

    match kind {
        "load_default" => Font::load_default(size)
            .map_err(|e| PilError::ValueError(format!("load_default failed: {e}"))),
        "ref" => {
            let id = font
                .get("id")
                .and_then(Value::as_str)
                .ok_or(PilError::ValueError(
                    "font ref requires an id string".into(),
                ))?;
            let data = fs::read(crate_fixture_dir().join(id))
                .map_err(|e| PilError::ValueError(format!("font bytes read failed ({id}): {e}")))?;
            Font::from_bytes(data, size)
                .map_err(|e| PilError::ValueError(format!("font parse failed: {e}")))
        }
        other => Err(PilError::ValueError(format!(
            "unsupported imagingft fixture font kind: {other}"
        ))),
    }
}

fn parse_spacing(value: &Value) -> Result<f32, PilError> {
    value
        .as_f64()
        .map(|v| v as f32)
        .ok_or_else(|| PilError::ValueError("spacing must be a number".into()))
}

fn run_case(operation: &str, font: &Font, params: &Value) -> Result<ApiValue, PilError> {
    match operation {
        "getname" => {
            let (family, style) = imagingft::getname(font);
            Ok(ApiValue::Name((family.to_string(), style.to_string())))
        }
        "getmetrics" => Ok(ApiValue::Metrics(imagingft::getmetrics(font))),
        "getlength" => {
            let text = parse_text(&params["text"])?;
            Ok(ApiValue::Length(imagingft::getlength(font, text)))
        }
        "has_variations" => Ok(ApiValue::Bool(imagingft::has_variations(font))),
        "getbbox" => {
            let text = parse_text(&params["text"])?;
            Ok(ApiValue::BBox(imagingft::getbbox(font, text)))
        }
        "getbbox_binary" => {
            let text = parse_text(&params["text"])?;
            Ok(ApiValue::BBox(imagingft::getbbox_binary(font, text)))
        }
        "getmask" => {
            let text = parse_text(&params["text"])?;
            let (width, height, pixels) = imagingft::getmask(font, text);
            Ok(ApiValue::Mask {
                size: (width, height),
                mode: "L".to_string(),
                pixels,
            })
        }
        "getmask2" => {
            let text = parse_text(&params["text"])?;
            let (width, height, pixels, offset) = imagingft::getmask2(font, text);
            Ok(ApiValue::MaskWithOffset {
                size: (width, height),
                mode: "L".to_string(),
                pixels,
                offset,
            })
        }
        "getmask2_with_start" => {
            let text = parse_text(&params["text"])?;
            let start = parse_start(&params["start"])?;
            let (width, height, pixels, offset) = imagingft::getmask2_with_start(font, text, start);
            Ok(ApiValue::MaskWithOffset {
                size: (width, height),
                mode: "L".to_string(),
                pixels,
                offset,
            })
        }
        "get_transposed_mask" => {
            let text = parse_text(&params["text"])?;
            let orientation = parse_orientation(&params["orientation"]);
            let (width, height, pixels) = imagingft::get_transposed_mask(font, text, orientation)?;
            Ok(ApiValue::Mask {
                size: (width, height),
                mode: "L".to_string(),
                pixels,
            })
        }
        "transposed_bbox" => {
            let text = parse_text(&params["text"])?;
            let orientation = parse_orientation(&params["orientation"]);
            Ok(ApiValue::BBox(imagingft::transposed_bbox(
                imagingft::getbbox(font, text),
                orientation,
            )))
        }
        "validate_transposed_length" => {
            let orientation = parse_orientation(&params["orientation"]);
            imagingft::validate_transposed_length(orientation)?;
            Ok(ApiValue::Unit)
        }
        "draw_text" => {
            let text = parse_text(&params["text"])?;
            let expected_width = parse_size_u32(&params["canvas_width"])?;
            let expected_height = parse_size_u32(&params["canvas_height"])?;
            let (x, y) = parse_xy(&params["xy"])?;
            let fill = parse_fill(&params["fill"])?;
            let mode = params
                .get("mode")
                .and_then(Value::as_str)
                .ok_or(PilError::ValueError("mode must be a string".into()))?;
            let mut image = Image::new(expected_width, expected_height, mode, (0, 0, 0, 0))
                .map_err(|error| {
                    PilError::ValueError(format!("draw_text canvas allocation failed: {error}"))
                })?;
            let mut draw = Draw::new(image.clone(), Some(mode.to_string()));
            draw.text(x, y, text, font, fill)?;
            image = draw.image_clone();
            let pixels = image.tobytes()?;
            Ok(ApiValue::Mask {
                size: (expected_width, expected_height),
                mode: mode.to_string(),
                pixels,
            })
        }
        "render_text_binary" => {
            let text = parse_text(&params["text"])?;
            let fill = parse_fill(&params["fill"])?;
            let spacing = parse_spacing(&params["spacing"])?;
            let (width, height, pixels) = imagingft::render_text_binary(font, text, fill, spacing);
            Ok(ApiValue::Mask {
                size: (width, height),
                mode: "RGBA".to_string(),
                pixels,
            })
        }
        other => Err(PilError::NotImplementedError(format!(
            "unsupported imagingft operation: {other}"
        ))),
    }
}

fn compare_image_payload(actual_pixels: &[u8], expected: &Value, case_id: &str) {
    if let Some(path) = expected.get("raw_path").and_then(Value::as_str) {
        let raw = fs::read(artifact_path(path)).expect("raw oracle must be readable");
        assert_eq!(raw, actual_pixels, "{case_id}");
        return;
    }

    if let Some(hex) = expected.get("pixels_hex").and_then(Value::as_str) {
        assert_eq!(to_hex(actual_pixels), hex, "{case_id}");
    }
}

fn compare_output(operation: &str, actual: ApiValue, expected: &Value, case_id: &str) {
    match actual {
        ApiValue::Name(actual) => {
            let expected_name = &expected["name"];
            assert_eq!(actual.0, expected_name[0].as_str().expect("expected name"));
            assert_eq!(actual.1, expected_name[1].as_str().expect("expected name"));
        }
        ApiValue::Metrics(actual) => {
            let expected_metrics = &expected["metrics"];
            assert_eq!(
                actual,
                (
                    expected_metrics[0].as_u64().expect("expected ascender") as u32,
                    expected_metrics[1].as_u64().expect("expected descender") as u32,
                )
            );
        }
        ApiValue::Length(actual) => {
            let expected_length = expected["length"].as_f64().expect("expected length") as f32;
            assert_eq!(actual, expected_length, "{case_id}");
        }
        ApiValue::Bool(actual) => {
            let expected_bool = expected["has_variations"].as_bool().expect("expected bool");
            assert_eq!(actual, expected_bool, "{case_id}");
        }
        ApiValue::BBox(actual) => {
            let expected_bbox = (
                expected["bbox"][0].as_i64().unwrap() as i32,
                expected["bbox"][1].as_i64().unwrap() as i32,
                expected["bbox"][2].as_i64().unwrap() as i32,
                expected["bbox"][3].as_i64().unwrap() as i32,
            );
            assert_eq!(actual, expected_bbox, "{case_id}");
        }
        ApiValue::Mask { size, mode, pixels } => {
            assert_eq!(
                size.0,
                expected["size"][0].as_u64().unwrap() as u32,
                "{case_id}"
            );
            assert_eq!(
                size.1,
                expected["size"][1].as_u64().unwrap() as u32,
                "{case_id}"
            );
            assert_eq!(
                mode,
                expected["mode"].as_str().expect("expected mode"),
                "{case_id}"
            );
            if let Some(sha) = expected.get("sha256").and_then(Value::as_str) {
                assert_eq!(sha256_hex(&pixels), sha, "{case_id}");
            }
            compare_image_payload(&pixels, expected, case_id);
            if operation == "get_transposed_mask" {
                if let Some(pixels_hex) = expected.get("pixels_hex").and_then(Value::as_str) {
                    assert_eq!(to_hex(&pixels), pixels_hex, "{case_id}");
                }
            }
        }
        ApiValue::MaskWithOffset {
            size,
            mode,
            pixels,
            offset,
        } => {
            assert_eq!(
                size.0,
                expected["size"][0].as_u64().unwrap() as u32,
                "{case_id}"
            );
            assert_eq!(
                size.1,
                expected["size"][1].as_u64().unwrap() as u32,
                "{case_id}"
            );
            assert_eq!(
                mode,
                expected["mode"].as_str().expect("expected mode"),
                "{case_id}"
            );
            assert_eq!(
                offset,
                (
                    expected["offset"][0].as_i64().unwrap() as i32,
                    expected["offset"][1].as_i64().unwrap() as i32
                )
            );
            if let Some(sha) = expected.get("sha256").and_then(Value::as_str) {
                assert_eq!(sha256_hex(&pixels), sha, "{case_id}");
            }
            compare_image_payload(&pixels, expected, case_id);
        }
        ApiValue::Unit => {}
    }
}

fn assert_error_matches(case_id: &str, error: &PilError, expected: &Value) {
    let expected_error = &expected["error"];
    if let Some(expected_type) = expected_error.get("type").and_then(Value::as_str) {
        let debug = format!("{error:?}");
        assert!(
            debug.contains(expected_type),
            "{case_id}: expected error type '{expected_type}', got '{debug}'"
        );
    }

    let actual_message = error.to_string();
    if let Some(pattern) = expected_error
        .get("message_pattern")
        .and_then(Value::as_str)
    {
        assert!(
            actual_message.contains(pattern),
            "{case_id}: expected message to contain '{pattern}', got '{actual_message}'"
        );
        return;
    }

    let expected_message = expected_error["message"]
        .as_str()
        .expect("expected error message");
    assert_eq!(expected_message, actual_message, "{case_id}");
}

#[test]
fn imagingft_public_api_parity_matches_fixture_oracles() {
    let fixture_dir = crate_fixture_dir().join("inputs/public-api");
    let mut any_cases = 0usize;
    let mut implemented_ops = BTreeSet::new();

    for entry in
        fs::read_dir(&fixture_dir).expect("public-api imagingft directory must be readable")
    {
        let entry = entry.expect("fixture entry must be readable");
        let path = entry.path();
        let Some(stem) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if !stem.starts_with("imagingft.") || !stem.ends_with(".json") {
            continue;
        }

        let manifest = serde_json::from_str::<Value>(
            &fs::read_to_string(&path).expect("imagingft fixture JSON must be readable"),
        )
        .expect("imagingft fixture JSON must parse");
        let manifest_operation = manifest["operation"]
            .as_str()
            .expect("fixture must define operation");
        let operation = manifest_operation
            .strip_prefix("imagingft.")
            .unwrap_or(manifest_operation);
        if matches!(
            operation,
            "getname"
                | "getmetrics"
                | "getlength"
                | "has_variations"
                | "getbbox"
                | "getbbox_binary"
                | "getmask"
                | "getmask2"
                | "getmask2_with_start"
                | "get_transposed_mask"
                | "transposed_bbox"
                | "validate_transposed_length"
                | "draw_text"
        ) {
            implemented_ops.insert(operation.to_string());
        }

        if let Some(cases) = manifest["cases"].as_array() {
            for case in cases {
                any_cases += 1;
                let case_id = case["case_id"].as_str().unwrap_or("<missing case_id>");
                let expect_error = case["expect_error"].as_bool().unwrap_or(false);
                let params = &case["inputs"]["params"];
                let font = load_font(case);
                if font.is_err() {
                    assert!(
                        expect_error,
                        "{case_id}: expected success but failed to load font"
                    );
                    let error = font.expect_err("case font must fail");
                    let expected_status = expected_status(case);
                    assert_eq!(expected_status, "error", "{case_id}");
                    let expected = &case["expectation"]["expected"];
                    assert_error_matches(case_id, &error, expected);
                    continue;
                }
                let font = font.expect("case font must load");
                let expected_status = expected_status(case);
                let expected = &case["expectation"]["expected"];
                let actual = run_case(operation, &font, params);

                match (actual, expect_error) {
                    (Ok(value), false) => {
                        assert_ne!(expected_status, "error", "{case_id}");
                        assert_eq!(expected_status, "ok", "{case_id}");
                        compare_output(operation, value, expected, case_id);
                    }
                    (Err(error), true) => {
                        assert_eq!(expected_status, "error", "{case_id}");
                        assert_error_matches(case_id, &error, expected);
                    }
                    (Ok(_), true) => {
                        assert!(false, "{case_id}: expected error but got success")
                    }
                    (Err(error), false) => {
                        assert!(false, "{case_id}: unexpected error {error}")
                    }
                }
            }
        }
    }

    assert!(
        any_cases > 0,
        "imagingft public-api fixture corpus must contain cases"
    );

    for op in REQUIRED_PUBLIC_OPS {
        assert!(
            implemented_ops.contains(op),
            "required imagingft public surface '{op}' not represented in fixture inputs"
        );
    }
}
