#![cfg(feature = "image-codecs-all")]

use std::fs;
use std::path::{Path, PathBuf};

use pillow_rs::Image;
use pillow_rs::compute::{self, Backend};
use pillow_rs::draw::Draw;
use pillow_rs::error::PilError;
use pillow_rs::image::PaletteTransparency;
use pillow_rs::ops::paste::PasteSource;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct Manifest {
    oracle: Oracle,
    paste_cases: Vec<PasteCase>,
    paste_error_cases: Vec<PasteErrorCase>,
    draw_cases: Vec<DrawCase>,
    apply_transparency_cases: Vec<ApplyTransparencyCase>,
}

#[derive(Deserialize)]
struct Oracle {
    implementation: String,
    version: String,
}

#[derive(Clone, Deserialize)]
struct ImageSpec {
    mode: String,
    size: [u32; 2],
    pixels_hex: String,
}

#[derive(Deserialize)]
struct PasteSourceSpec {
    kind: String,
    image: Option<ImageSpec>,
    value: Option<Value>,
}

#[derive(Deserialize)]
struct PasteCase {
    id: String,
    destination: ImageSpec,
    source: PasteSourceSpec,
    #[serde(rename = "box")]
    box_coords: Vec<i32>,
    mask: Option<ImageSpec>,
    expected: ImageSpec,
    backends: Vec<String>,
}

#[derive(Deserialize)]
struct PasteErrorCase {
    id: String,
    destination: ImageSpec,
    source: PasteSourceSpec,
    #[serde(rename = "box")]
    box_coords: Option<Vec<i32>>,
    mask: Option<ImageSpec>,
    expected_error: ExpectedError,
}

#[derive(Deserialize)]
struct ExpectedError {
    #[serde(rename = "type")]
    kind: String,
    message: String,
}

#[derive(Deserialize)]
struct DrawCase {
    id: String,
    source: ImageSpec,
    operation: String,
    parameters: Value,
    expected: ImageSpec,
    backends: Vec<String>,
    unsupported_backends: Vec<String>,
}

#[derive(Deserialize)]
struct ApplyTransparencyCase {
    id: String,
    input: String,
    expected: ApplyTransparencyExpected,
}

#[derive(Deserialize)]
struct ApplyTransparencyExpected {
    mode: String,
    size: [u32; 2],
    pixels_hex: String,
    palette_rgba_hex: String,
    before_info: Value,
    before_palette_mode: String,
    before_has_transparency_data: bool,
    info: Value,
    palette_mode: String,
    has_transparency_data: bool,
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/image_backend")
}

fn manifest() -> Manifest {
    let bytes = fs::read(fixture_root().join("backend_parity.json"))
        .expect("backend parity manifest must be readable");
    serde_json::from_slice(&bytes).expect("backend parity manifest must be valid JSON")
}

fn decode_hex(value: &str) -> Vec<u8> {
    assert!(
        value.len().is_multiple_of(2),
        "hex input must have byte pairs"
    );
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).expect("hex must be ASCII");
            u8::from_str_radix(text, 16).expect("fixture must contain valid hex")
        })
        .collect()
}

fn transparency_info(image: &Image) -> Value {
    match image.pending_palette_transparency() {
        Some(PaletteTransparency::Index(index)) => {
            serde_json::json!({"transparency": {"kind": "index", "value": index}})
        }
        Some(PaletteTransparency::Table(alpha)) => {
            let value_hex = alpha
                .iter()
                .map(|value| format!("{value:02x}"))
                .collect::<String>();
            serde_json::json!({
                "transparency": {"kind": "table", "value_hex": value_hex}
            })
        }
        None => Value::Object(Default::default()),
    }
}

fn image_from_spec(spec: &ImageSpec) -> Image {
    Image::frombytes(
        &spec.mode,
        (spec.size[0], spec.size[1]),
        &decode_hex(&spec.pixels_hex),
    )
    .unwrap_or_else(|error| panic!("{} fixture image: {error}", spec.mode))
}

fn backend(name: &str) -> Backend {
    Backend::parse(name).unwrap_or_else(|| panic!("unknown fixture backend {name}"))
}

fn paste_source(spec: &PasteSourceSpec) -> PasteSource {
    match spec.kind.as_str() {
        "image" => PasteSource::Image(image_from_spec(
            spec.image
                .as_ref()
                .expect("image source must have image data"),
        )),
        "scalar" => PasteSource::Scalar(
            spec.value
                .as_ref()
                .and_then(Value::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .expect("scalar paste value must fit u8"),
        ),
        "tuple" => {
            let values: Vec<u8> = spec
                .value
                .as_ref()
                .and_then(Value::as_array)
                .expect("tuple paste value must be an array")
                .iter()
                .map(|value| {
                    value
                        .as_u64()
                        .and_then(|value| u8::try_from(value).ok())
                        .expect("tuple paste channel must fit u8")
                })
                .collect();
            match values.as_slice() {
                [value] => PasteSource::Scalar(*value),
                [luma, alpha] => PasteSource::LumaAlpha(*luma, *alpha),
                [red, green, blue] => PasteSource::Rgb(*red, *green, *blue),
                [red, green, blue, alpha] => PasteSource::Rgba(*red, *green, *blue, *alpha),
                _ => panic!("unsupported fixture paste tuple length"),
            }
        }
        other => panic!("unsupported fixture paste source {other}"),
    }
}

fn assert_image(case_id: &str, actual: &Image, expected: &ImageSpec) {
    assert_eq!(
        actual.mode().expect("actual mode"),
        expected.mode,
        "{case_id}: mode"
    );
    assert_eq!(
        actual.size().expect("actual size"),
        (expected.size[0], expected.size[1]),
        "{case_id}: size"
    );
    assert_eq!(
        actual.tobytes().expect("actual pixels"),
        decode_hex(&expected.pixels_hex),
        "{case_id}: exact pixels"
    );
}

fn error_kind(error: &PilError) -> &'static str {
    match error {
        PilError::ValueError(_) => "ValueError",
        PilError::TypeError(_) => "TypeError",
        other => panic!("unexpected paste error category: {other:?}"),
    }
}

fn byte(value: &Value) -> u8 {
    value
        .as_u64()
        .and_then(|value| u8::try_from(value).ok())
        .expect("draw channel must fit u8")
}

fn color(value: &Value, mode: &str) -> (u8, u8, u8, u8) {
    if let Some(value) = value.as_u64().and_then(|value| u8::try_from(value).ok()) {
        return match mode {
            "RGB" => (value, 0, 0, 255),
            "RGBA" | "CMYK" => (value, 0, 0, 0),
            "LA" => (value, value, value, 0),
            _ => (value, value, value, 255),
        };
    }
    let values = value
        .as_array()
        .expect("draw color must be scalar or array");
    match values.as_slice() {
        [luma, alpha] => {
            let luma = byte(luma);
            (luma, luma, luma, byte(alpha))
        }
        [red, green, blue] => (byte(red), byte(green), byte(blue), 255),
        [red, green, blue, alpha] => (byte(red), byte(green), byte(blue), byte(alpha)),
        _ => panic!("unsupported draw fixture color"),
    }
}

fn flat_xy(parameters: &Value) -> [i32; 4] {
    let xy = parameters["xy"]
        .as_array()
        .expect("draw fixture xy must be an array");
    [
        xy[0].as_i64().expect("x0") as i32,
        xy[1].as_i64().expect("y0") as i32,
        xy[2].as_i64().expect("x1") as i32,
        xy[3].as_i64().expect("y1") as i32,
    ]
}

fn draw_case(case: &DrawCase, backend: Backend) -> Image {
    let source = image_from_spec(&case.source);
    let mode = source.mode().expect("draw source mode");
    let mut draw = Draw::new(source, Some(mode.clone()));
    let parameters = &case.parameters;
    let width = parameters.get("width").and_then(Value::as_u64).unwrap_or(1) as u32;
    let fill = parameters.get("fill").map(|value| color(value, &mode));
    let outline = parameters.get("outline").map(|value| color(value, &mode));

    match case.operation.as_str() {
        "line" => {
            let xy = flat_xy(parameters);
            draw.line(xy[0], xy[1], xy[2], xy[3], fill.expect("line fill"), width)
                .expect("draw line");
        }
        "rectangle" => {
            let xy = flat_xy(parameters);
            draw.rectangle(xy[0], xy[1], xy[2], xy[3], fill, outline, width)
                .expect("draw rectangle");
        }
        "ellipse" => {
            let xy = flat_xy(parameters);
            draw.ellipse(xy[0], xy[1], xy[2], xy[3], fill, outline, width)
                .expect("draw ellipse");
        }
        "polygon" => {
            let points: Vec<(i32, i32)> = parameters["xy"]
                .as_array()
                .expect("polygon points")
                .iter()
                .map(|point| {
                    let point = point.as_array().expect("polygon point");
                    (
                        point[0].as_i64().expect("polygon x") as i32,
                        point[1].as_i64().expect("polygon y") as i32,
                    )
                })
                .collect();
            draw.polygon(&points, fill, outline, width)
                .expect("draw polygon");
        }
        "point" => {
            let points: Vec<(i32, i32)> = parameters["xy"]
                .as_array()
                .expect("point coordinates")
                .iter()
                .map(|point| {
                    let point = point.as_array().expect("point coordinate");
                    (
                        point[0].as_i64().expect("point x") as i32,
                        point[1].as_i64().expect("point y") as i32,
                    )
                })
                .collect();
            draw.point(&points, fill.expect("point fill"))
                .expect("draw point");
        }
        "arc" => {
            let xy = flat_xy(parameters);
            draw.arc(
                xy[0],
                xy[1],
                xy[2],
                xy[3],
                parameters["start"].as_f64().expect("arc start"),
                parameters["end"].as_f64().expect("arc end"),
                fill.expect("arc fill"),
                width,
            )
            .expect("draw arc");
        }
        "chord" => {
            let xy = flat_xy(parameters);
            draw.chord(
                xy[0],
                xy[1],
                xy[2],
                xy[3],
                parameters["start"].as_f64().expect("chord start"),
                parameters["end"].as_f64().expect("chord end"),
                fill,
                outline,
                width,
            )
            .expect("draw chord");
        }
        "pieslice" => {
            let xy = flat_xy(parameters);
            draw.pieslice(
                xy[0],
                xy[1],
                xy[2],
                xy[3],
                parameters["start"].as_f64().expect("pieslice start"),
                parameters["end"].as_f64().expect("pieslice end"),
                fill,
                outline,
                width,
            )
            .expect("draw pieslice");
        }
        "circle" => {
            let xy = parameters["xy"].as_array().expect("circle center");
            draw.circle(
                xy[0].as_i64().expect("circle x") as i32,
                xy[1].as_i64().expect("circle y") as i32,
                parameters["radius"].as_f64().expect("circle radius"),
                fill,
                outline,
                width,
            )
            .expect("draw circle");
        }
        "rounded_rectangle" => {
            let xy = flat_xy(parameters);
            draw.rounded_rectangle(
                xy[0],
                xy[1],
                xy[2],
                xy[3],
                parameters["radius"]
                    .as_f64()
                    .expect("rounded rectangle radius"),
                fill,
                outline,
                width,
            )
            .expect("draw rounded rectangle");
        }
        operation => panic!("unsupported draw fixture operation {operation}"),
    }
    draw.into_image().use_backend(backend)
}

fn draw_registry_key(operation: &str) -> &'static str {
    match operation {
        "line" => "DrawLine",
        "rectangle" => "DrawRectangle",
        "rounded_rectangle" => "DrawRoundedRect",
        "ellipse" => "DrawEllipse",
        "circle" => "DrawCircle",
        "polygon" => "DrawPolygon",
        "arc" => "DrawArc",
        "chord" => "DrawChord",
        "pieslice" => "DrawPieslice",
        "point" => "DrawPoint",
        other => panic!("unsupported draw registry operation {other}"),
    }
}

#[test]
fn pillow_oracle_is_version_pinned() {
    let manifest = manifest();
    assert_eq!(manifest.oracle.implementation, "Pillow");
    assert_eq!(manifest.oracle.version, "12.2.0");
}

#[test]
fn paste_is_exact_on_every_declared_native_backend() {
    let available = compute::available_backends();
    for required in [Backend::Cpu, Backend::Simd, Backend::Gpu] {
        assert!(
            available.contains(&required),
            "forced-backend parity requires {required:?}; compiled backends: {available:?}"
        );
    }

    let entry = compute::registry::registry()
        .get("Paste")
        .expect("Paste must be registered");
    assert!(
        entry.cpu_fn.is_some(),
        "Paste must have a CPU implementation"
    );
    assert!(
        entry.simd_fn.is_some(),
        "Paste must have a dedicated SIMD-pool implementation, not a CPU-pool fallback"
    );
    assert_eq!(
        entry.gpu_shader,
        Some("paste.wgsl"),
        "Paste must have its own GPU shader"
    );
    assert!(
        entry.gpu_source.is_some(),
        "Paste must embed its GPU implementation"
    );

    for case in manifest().paste_cases {
        for backend_name in &case.backends {
            let selected = backend(backend_name);
            let mut destination = image_from_spec(&case.destination);
            let destination_palette = destination.palette();
            let destination_palette_alpha = destination.palette_alpha();
            let mask = case.mask.as_ref().map(image_from_spec);
            let source = paste_source(&case.source);
            match case.box_coords.as_slice() {
                [x, y] => destination
                    .paste_at(source, Some((*x, *y)), mask.as_ref())
                    .unwrap_or_else(|error| panic!("{} {backend_name}: {error}", case.id)),
                [left, top, right, bottom] => destination
                    .paste(source, Some((*left, *top, *right, *bottom)), mask.as_ref())
                    .unwrap_or_else(|error| panic!("{} {backend_name}: {error}", case.id)),
                _ => panic!("{} has invalid paste box", case.id),
            }
            let destination = destination.use_backend(selected);
            assert_eq!(
                destination.backend(),
                Some(selected),
                "{} must remain locked to {backend_name}",
                case.id
            );
            assert_image(
                &format!("{} [{backend_name}]", case.id),
                &destination,
                &case.expected,
            );
            if case.destination.mode == "P" {
                assert_eq!(
                    destination.palette(),
                    destination_palette,
                    "{} [{backend_name}]: destination palette",
                    case.id
                );
                assert_eq!(
                    destination.palette_alpha(),
                    destination_palette_alpha,
                    "{} [{backend_name}]: destination palette alpha",
                    case.id
                );
            }
        }
    }
}

#[test]
fn paste_validation_matches_exact_pillow_errors() {
    for case in manifest().paste_error_cases {
        let mut destination = image_from_spec(&case.destination);
        let source = paste_source(&case.source);
        let mask = case.mask.as_ref().map(image_from_spec);
        let result = match case.box_coords.as_deref() {
            None => destination.paste_at(source, None, mask.as_ref()),
            Some([x, y]) => destination.paste_at(source, Some((*x, *y)), mask.as_ref()),
            Some([left, top, right, bottom]) => {
                destination.paste(source, Some((*left, *top, *right, *bottom)), mask.as_ref())
            }
            Some(_) => panic!("{} has invalid error-fixture box", case.id),
        };
        let error = result.unwrap_err();
        assert_eq!(error_kind(&error), case.expected_error.kind, "{}", case.id);
        assert_eq!(
            error.to_string(),
            case.expected_error.message,
            "{}",
            case.id
        );
    }
}

#[test]
fn drawing_is_exact_and_backend_capabilities_are_truthful() {
    let registry = compute::registry::registry();
    for case in manifest().draw_cases {
        let key = draw_registry_key(&case.operation);
        let entry = registry
            .get(key)
            .unwrap_or_else(|| panic!("{} missing registry entry {key}", case.id));
        assert!(entry.cpu_fn.is_some(), "{} must have CPU drawing", case.id);
        for backend_name in &case.unsupported_backends {
            match backend(backend_name) {
                Backend::Gpu => assert!(
                    entry.gpu_shader.is_none(),
                    "{} must not claim a GPU shader",
                    case.id
                ),
                Backend::Simd => assert!(
                    entry.simd_fn.is_none(),
                    "{} must not claim a SIMD function",
                    case.id
                ),
                Backend::Cpu => panic!("CPU cannot be listed as unsupported"),
            }
        }
        for backend_name in &case.backends {
            let selected = backend(backend_name);
            let source = image_from_spec(&case.source);
            let source_palette = source.palette();
            let source_palette_alpha = source.palette_alpha();
            let actual = draw_case(&case, selected);
            assert_eq!(
                actual.backend(),
                Some(selected),
                "{} must remain locked to {backend_name}",
                case.id
            );
            assert_image(
                &format!("{} [{backend_name}]", case.id),
                &actual,
                &case.expected,
            );
            if case.source.mode == "P" {
                assert_eq!(
                    actual.palette(),
                    source_palette,
                    "{} [{backend_name}]: palette",
                    case.id
                );
                assert_eq!(
                    actual.palette_alpha(),
                    source_palette_alpha,
                    "{} [{backend_name}]: palette alpha",
                    case.id
                );
            }
        }
    }
}

#[test]
fn apply_transparency_preserves_indices_and_commits_palette_alpha() {
    for case in manifest().apply_transparency_cases {
        let input = fs::read(fixture_root().join(&case.input))
            .unwrap_or_else(|error| panic!("{} input: {error}", case.id));
        let mut image =
            Image::open_bytes(input).unwrap_or_else(|error| panic!("{} open: {error}", case.id));
        assert_eq!(
            transparency_info(&image),
            case.expected.before_info,
            "{} exact info before apply_transparency",
            case.id
        );
        assert_eq!(
            image.palette_mode(),
            Some(case.expected.before_palette_mode.as_str()),
            "{} palette mode before apply_transparency",
            case.id
        );
        assert_eq!(
            image.has_transparency_data(),
            case.expected.before_has_transparency_data,
            "{} transparency flag before apply_transparency",
            case.id
        );
        image
            .apply_transparency()
            .unwrap_or_else(|error| panic!("{} apply_transparency: {error}", case.id));
        assert_eq!(
            transparency_info(&image),
            case.expected.info,
            "{} exact info after apply_transparency",
            case.id
        );
        assert_eq!(
            image.palette_mode(),
            Some(case.expected.palette_mode.as_str()),
            "{} palette mode after apply_transparency",
            case.id
        );
        assert_eq!(
            image.has_transparency_data(),
            case.expected.has_transparency_data,
            "{} transparency flag after apply_transparency",
            case.id
        );
        assert_eq!(
            image.mode().expect("mode"),
            case.expected.mode,
            "{}",
            case.id
        );
        assert_eq!(
            image.size().expect("size"),
            (case.expected.size[0], case.expected.size[1]),
            "{}",
            case.id
        );
        assert_eq!(
            image.tobytes().expect("pixels"),
            decode_hex(&case.expected.pixels_hex),
            "{} exact indices",
            case.id
        );

        let palette = image.getpalette_trimmed().expect("indexed image palette");
        let alpha = image.palette_alpha().expect("indexed image alpha");
        let mut rgba = Vec::with_capacity(palette.len() / 3 * 4);
        for (index, rgb) in palette.chunks_exact(3).enumerate() {
            rgba.extend_from_slice(rgb);
            rgba.push(alpha.get(index).copied().unwrap_or(255));
        }
        assert_eq!(
            rgba,
            decode_hex(&case.expected.palette_rgba_hex),
            "{} exact RGBA palette",
            case.id
        );
    }
}
