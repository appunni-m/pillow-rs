use std::{collections::BTreeSet, fs, path::PathBuf};

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
        mode: &'static str,
        pixels: Vec<u8>,
    },
    MaskWithOffset {
        size: (u32, u32),
        mode: &'static str,
        pixels: Vec<u8>,
        offset: (i32, i32),
    },
    Unit,
}

fn load_font(case: &Value) -> Font {
    let inputs = &case["inputs"];
    let font = inputs["assets"]["font"]
        .as_object()
        .expect("font asset must be an object");
    let kind = font["kind"].as_str().expect("font kind must be a string");
    let size = inputs["params"]["size"].as_f64().unwrap_or(10.0) as f32;

    match kind {
        "load_default" => Font::load_default(size).expect("default font must load"),
        "ref" => {
            let id = font["id"].as_str().expect("font ref id must be a string");
            let data = fs::read(crate_fixture_dir().join(id)).expect("font bytes must read");
            Font::from_bytes(data, size).expect("font bytes must parse")
        }
        other => panic!("unsupported imagingft fixture font kind: {other}"),
    }
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

fn parse_start(value: &Value) -> (f64, f64) {
    let coords = value
        .as_array()
        .expect("start must be an array of two floats");
    assert_eq!(coords.len(), 2, "start must contain exactly two values");
    let x = coords[0].as_f64().expect("start[0] must be a number");
    let y = coords[1].as_f64().expect("start[1] must be a number");
    (x, y)
}

fn pil_error_kind(err: &PilError) -> &'static str {
    match err {
        PilError::IOError(_) => "IOError",
        PilError::OsError(_) => "OsError",
        PilError::AssertionError(_) => "AssertionError",
        PilError::IndexError(_) => "IndexError",
        PilError::UnidentifiedImageError(_) => "UnidentifiedImageError",
        PilError::ValueError(_) => "ValueError",
        PilError::SyntaxError(_) => "SyntaxError",
        PilError::TypeError(_) => "TypeError",
        PilError::ImageError(_) => "ImageError",
        PilError::NotImplementedError(_) => "NotImplementedError",
        PilError::UnknownFormat(_) => "UnknownFormat",
        PilError::Io(_) => "IOError",
        PilError::PaletteError(_) => "PaletteError",
        PilError::InternalError(_) => "InternalError",
        PilError::DimensionError(_) => "DimensionError",
    }
}

fn run_case(operation: &str, font: &Font, params: &Value) -> Result<ApiValue, PilError> {
    match operation {
        "getname" => {
            let (family, style) = imagingft::getname(font);
            Ok(ApiValue::Name((family.to_string(), style.to_string())))
        }
        "getmetrics" => Ok(ApiValue::Metrics(imagingft::getmetrics(font))),
        "getlength" => {
            let text = params["text"].as_str().expect("text must be string");
            Ok(ApiValue::Length(imagingft::getlength(font, text)))
        }
        "has_variations" => Ok(ApiValue::Bool(imagingft::has_variations(font))),
        "getbbox" => {
            let text = params["text"].as_str().expect("text must be string");
            Ok(ApiValue::BBox(imagingft::getbbox(font, text)))
        }
        "getbbox_binary" => {
            let text = params["text"].as_str().expect("text must be string");
            Ok(ApiValue::BBox(imagingft::getbbox_binary(font, text)))
        }
        "getmask" => {
            let text = params["text"].as_str().expect("text must be string");
            let (width, height, pixels) = imagingft::getmask(font, text);
            Ok(ApiValue::Mask {
                size: (width, height),
                mode: "L",
                pixels,
            })
        }
        "getmask2" => {
            let text = params["text"].as_str().expect("text must be string");
            let (width, height, pixels, offset) = imagingft::getmask2(font, text);
            Ok(ApiValue::MaskWithOffset {
                size: (width, height),
                mode: "L",
                pixels,
                offset,
            })
        }
        "getmask2_with_start" => {
            let text = params["text"].as_str().expect("text must be string");
            let start = parse_start(&params["start"]);
            let (width, height, pixels, offset) = imagingft::getmask2_with_start(font, text, start);
            Ok(ApiValue::MaskWithOffset {
                size: (width, height),
                mode: "L",
                pixels,
                offset,
            })
        }
        "get_transposed_mask" => {
            let text = params["text"].as_str().expect("text must be string");
            let orientation = parse_orientation(&params["orientation"]);
            let (width, height, pixels) = imagingft::get_transposed_mask(font, text, orientation)?;
            Ok(ApiValue::Mask {
                size: (width, height),
                mode: "L",
                pixels,
            })
        }
        "transposed_bbox" => {
            let text = params["text"].as_str().expect("text must be string");
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
            let text = params["text"].as_str().expect("text must be string");
            let expected_width = 96u32;
            let expected_height = 64u32;
            let mut image = Image::new(expected_width, expected_height, "RGBA", (0, 0, 0, 0))
                .expect("draw_text canvas");
            let mut draw = Draw::new(image.clone(), Some("RGBA".to_string()));
            draw.text(10, 18, text, font, (20, 40, 200, 255))?;
            image = draw.image_clone();
            let pixels = image.tobytes()?;
            Ok(ApiValue::Mask {
                size: (expected_width, expected_height),
                mode: "RGBA",
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
    let expected_type = expected_error["type"]
        .as_str()
        .expect("expected error type");
    let expected_message = expected_error["message"]
        .as_str()
        .expect("expected error message");
    assert_eq!(expected_type, pil_error_kind(error), "{case_id}");
    assert_eq!(expected_message, &error.to_string(), "{case_id}");
}

#[test]
fn imagingft_public_api_parity_matches_fixture_oracles() {
    let fixture_dir = crate_fixture_dir().join("inputs/public-api");
    let mut any_cases = 0usize;
    let mut seen_operations = BTreeSet::new();
    let expected_operations: BTreeSet<&str> = [
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
        "draw_text",
        "transposed_bbox",
        "validate_transposed_length",
    ]
    .into_iter()
    .collect();

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
        seen_operations.insert(operation.to_string());

        if let Some(cases) = manifest["cases"].as_array() {
            for case in cases {
                any_cases += 1;
                let case_id = case["case_id"].as_str().unwrap_or("<missing case_id>");
                let expect_error = case["expect_error"].as_bool().unwrap_or(false);
                let params = &case["inputs"]["params"];
                let font = load_font(case);
                let expected_status = case["expectation"]["status"].as_str().unwrap_or("ok");
                let expected = &case["expectation"]["expected"];
                let actual = run_case(operation, &font, params);

                match (actual, expect_error) {
                    (Ok(value), false) => {
                        assert_ne!(expected_status, "error", "{case_id}");
                        compare_output(operation, value, expected, case_id);
                    }
                    (Err(error), true) => {
                        assert_error_matches(case_id, &error, expected);
                    }
                    (Ok(_), true) => panic!("{case_id}: expected error but got success"),
                    (Err(error), false) => panic!("{case_id}: unexpected error {error}"),
                }
            }
        }
    }

    for op in expected_operations {
        assert!(
            seen_operations.contains(op),
            "public-api fixture corpus missing required operation: {op}"
        );
    }

    assert!(
        any_cases > 0,
        "imagingft public-api fixture corpus must contain cases"
    );
}
