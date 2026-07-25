#![cfg(feature = "image-codecs-all")]

use std::fs;
use std::path::{Path, PathBuf};

use pillow_rs::Image;
use pillow_rs::compute::{self, Backend};
use pillow_rs::draw::Draw;
use pillow_rs::error::PilError;
use pillow_rs::font::Font;
use pillow_rs::image::{PaletteTransparency, PutDataValue};
use pillow_rs::ops::{chops, imageops, module_fns, paste::PasteSource};
use pillow_rs::pipeline::PipelineOp;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct Manifest {
    oracle: Oracle,
    coverage: Vec<CoverageRow>,
    paste_cases: Vec<PasteCase>,
    paste_error_cases: Vec<PasteErrorCase>,
    draw_cases: Vec<DrawCase>,
    apply_transparency_cases: Vec<ApplyTransparencyCase>,
    indexed_immediate_draw_cases: Vec<IndexedImmediateDrawCase>,
}

#[derive(Deserialize)]
struct Oracle {
    implementation: String,
    version: String,
    freetype_version: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct ImageSpec {
    mode: String,
    size: [u32; 2],
    pixels_hex: String,
    palette_hex: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct CoverageRow {
    operation: String,
    mode: String,
    case_id: String,
    expected: ImageSpec,
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
    prepare_alpha: Option<u8>,
    backends: Vec<String>,
    expected: ApplyTransparencyExpected,
}

#[derive(Deserialize)]
struct ApplyTransparencyExpected {
    mode: String,
    size: [u32; 2],
    pixels_hex: String,
    palette_hex: Option<String>,
    palette_rgba_hex: String,
    before_info: Value,
    before_palette_mode: String,
    before_has_transparency_data: bool,
    info: Value,
    palette_mode: String,
    has_transparency_data: bool,
}

#[derive(Deserialize)]
struct IndexedImmediateDrawCase {
    id: String,
    input: String,
    operation: String,
    parameters: Value,
    expected: IndexedImmediateDrawExpected,
}

#[derive(Deserialize)]
struct IndexedImmediateDrawExpected {
    mode: String,
    size: [u32; 2],
    pixels_hex: String,
    palette_hex: Option<String>,
    format: String,
    info: Value,
    palette_mode: String,
    has_transparency_data: bool,
    palette_rgba_hex: String,
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
    let storage_mode = if spec.mode == "PA" { "LA" } else { &spec.mode };
    let mut image = Image::frombytes(
        storage_mode,
        (spec.size[0], spec.size[1]),
        &decode_hex(&spec.pixels_hex),
    )
    .unwrap_or_else(|error| panic!("{} fixture image: {error}", spec.mode));
    if let Some(palette_hex) = &spec.palette_hex {
        image
            .putpalette(&decode_hex(palette_hex), "RGB")
            .unwrap_or_else(|error| panic!("{} fixture palette: {error}", spec.mode));
    }
    image
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
    if let Some(palette_hex) = &expected.palette_hex {
        assert_eq!(
            actual.getpalette_trimmed(),
            Some(decode_hex(palette_hex)),
            "{case_id}: exact palette"
        );
    }
}

fn assert_exact_samples(case_id: &str, actual: &Image, mode: &str, expected: &[u8]) {
    assert_eq!(actual.mode().expect("actual mode"), mode, "{case_id}: mode");
    assert_eq!(
        actual.tobytes().expect("actual pixels"),
        expected,
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

fn assert_value_error(case_id: &str, error: PilError, expected: &str) {
    assert_eq!(error_kind(&error), "ValueError", "{case_id}: error type");
    assert_eq!(error.to_string(), expected, "{case_id}: exact error");
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
    assert_eq!(manifest.oracle.freetype_version, "2.14.3");
}

#[test]
fn backend_coverage_metadata_matches_every_exact_case() {
    let Manifest {
        coverage,
        paste_cases,
        draw_cases,
        apply_transparency_cases,
        indexed_immediate_draw_cases,
        ..
    } = manifest();
    let mut expected = Vec::new();
    expected.extend(paste_cases.into_iter().map(|case| CoverageRow {
        operation: "Image.paste".to_owned(),
        mode: case.expected.mode.clone(),
        case_id: case.id,
        expected: case.expected,
    }));
    expected.extend(draw_cases.into_iter().map(|case| CoverageRow {
        operation: format!("ImageDraw.{}", case.operation),
        mode: case.expected.mode.clone(),
        case_id: case.id,
        expected: case.expected,
    }));
    expected.extend(
        apply_transparency_cases
            .into_iter()
            .map(|case| CoverageRow {
                operation: "Image.apply_transparency".to_owned(),
                mode: case.expected.mode.clone(),
                case_id: case.id,
                expected: ImageSpec {
                    mode: case.expected.mode,
                    size: case.expected.size,
                    pixels_hex: case.expected.pixels_hex,
                    palette_hex: case.expected.palette_hex,
                },
            }),
    );
    expected.extend(
        indexed_immediate_draw_cases
            .into_iter()
            .map(|case| CoverageRow {
                operation: format!("ImageDraw.{}", case.operation),
                mode: case.expected.mode.clone(),
                case_id: case.id,
                expected: ImageSpec {
                    mode: case.expected.mode,
                    size: case.expected.size,
                    pixels_hex: case.expected.pixels_hex,
                    palette_hex: case.expected.palette_hex,
                },
            }),
    );
    assert_eq!(
        coverage, expected,
        "generated coverage rows must exactly mirror every Rust-consumed oracle image"
    );
}

#[test]
fn effect_spread_registry_exposes_only_the_sequential_cpu_lane() {
    let op = PipelineOp::EffectSpread { distance: 0 };
    let entry = compute::registry::registry()
        .get("EffectSpread")
        .expect("EffectSpread must be registered");

    assert_eq!(
        (
            entry.cpu_fn.is_some(),
            entry.simd_fn.is_some(),
            entry.gpu_shader,
            entry.gpu_source.is_some(),
            compute::registry::cpu_supports(&op),
            compute::registry::simd_supports(&op),
            compute::registry::gpu_supports(&op),
            compute::registry::map_op_to_gpu(&op).is_some(),
        ),
        (true, false, None, false, true, false, false, false)
    );
}

#[test]
fn forced_non_cpu_backends_reject_effect_spread_without_fallback() {
    for (backend, expected) in [
        (Backend::Simd, "SIMD: no native impl for EffectSpread"),
        (Backend::Gpu, "GPU: no native impl for EffectSpread"),
    ] {
        let image = Image::frombytes("L", (2, 1), &[17, 231]).expect("spread source");
        let spread = module_fns::effect_spread(&image, 0)
            .expect("spread must queue")
            .use_backend(backend);
        let error = spread
            .tobytes()
            .expect_err("unsupported forced backend must reject EffectSpread");

        assert_eq!(
            (error_kind(&error), error.to_string()),
            ("ValueError", expected.to_owned()),
            "{backend:?}"
        );
    }
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
            if matches!(case.destination.mode.as_str(), "P" | "PA") {
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
fn paste_is_exact_on_portable_cpu_backend() {
    for case in manifest().paste_cases {
        let mut destination = image_from_spec(&case.destination);
        let destination_palette = destination.palette();
        let destination_palette_alpha = destination.palette_alpha();
        let mask = case.mask.as_ref().map(image_from_spec);
        let source = paste_source(&case.source);
        match case.box_coords.as_slice() {
            [x, y] => destination
                .paste_at(source, Some((*x, *y)), mask.as_ref())
                .unwrap_or_else(|error| panic!("{} cpu: {error}", case.id)),
            [left, top, right, bottom] => destination
                .paste(source, Some((*left, *top, *right, *bottom)), mask.as_ref())
                .unwrap_or_else(|error| panic!("{} cpu: {error}", case.id)),
            _ => panic!("{} has invalid paste box", case.id),
        }
        assert_image(&format!("{} [cpu]", case.id), &destination, &case.expected);
        if matches!(case.destination.mode.as_str(), "P" | "PA") {
            assert_eq!(
                destination.palette(),
                destination_palette,
                "{} [cpu]: destination palette",
                case.id
            );
            assert_eq!(
                destination.palette_alpha(),
                destination_palette_alpha,
                "{} [cpu]: destination palette alpha",
                case.id
            );
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
fn gpu_paste_propagates_nested_auxiliary_errors_before_dispatch() {
    let mut deferred_source =
        Image::frombytes("P", (2, 1), &[1, 2]).expect("deferred paste source");
    let source_mode = deferred_source.mode().expect("source mode");
    let mut source_draw = Draw::new(deferred_source, Some(source_mode));
    source_draw
        .point(&[(0, 0)], (1, 2, 3, 255))
        .expect("queue CPU-only source draw");
    deferred_source = source_draw.into_image().use_backend(Backend::Gpu);

    let mut destination = Image::frombytes("P", (2, 1), &[0, 0]).expect("paste destination");
    destination
        .paste_at(PasteSource::Image(deferred_source), Some((0, 0)), None)
        .expect("queue paste with deferred source");
    let error = destination
        .use_backend(Backend::Gpu)
        .materialize()
        .expect_err("nested source backend error must escape GPU paste");
    assert_value_error(
        "GPU Paste nested source",
        error,
        "GPU: no native impl for DrawPoint",
    );

    let source = Image::new(2, 1, "RGBA", (10, 20, 30, 255)).expect("plain paste source");
    let mut deferred_mask = Image::frombytes("1", (2, 1), &[255, 255]).expect("deferred mask");
    let mask_mode = deferred_mask.mode().expect("mask mode");
    let mut mask_draw = Draw::new(deferred_mask, Some(mask_mode));
    mask_draw
        .point(&[(0, 0)], (0, 0, 0, 255))
        .expect("queue CPU-only mask draw");
    deferred_mask = mask_draw.into_image().use_backend(Backend::Gpu);

    let mut destination =
        Image::new(2, 1, "RGBA", (0, 0, 0, 255)).expect("masked paste destination");
    destination
        .paste_at(
            PasteSource::Image(source),
            Some((0, 0)),
            Some(&deferred_mask),
        )
        .expect("queue paste with deferred mask");
    let error = destination
        .use_backend(Backend::Gpu)
        .materialize()
        .expect_err("nested mask backend error must escape GPU paste");
    assert_value_error(
        "GPU Paste nested mask",
        error,
        "GPU: no native impl for DrawPoint",
    );
}

#[test]
fn forced_unsupported_backend_wins_before_source_decode() {
    let mut corrupt = fs::read(fixture_root().join("inputs/png_indexed_alpha_table.png"))
        .expect("indexed PNG fixture");
    let mut position = 8usize;
    loop {
        let length = u32::from_be_bytes(
            corrupt[position..position + 4]
                .try_into()
                .expect("PNG chunk length"),
        ) as usize;
        if &corrupt[position + 4..position + 8] == b"IDAT" {
            assert_ne!(length, 0, "fixture IDAT must contain compressed bytes");
            corrupt[position + 8] ^= 0xff;
            break;
        }
        position += length + 12;
    }
    Image::open_bytes(corrupt.clone())
        .expect("corrupt payload still has an inspectable PNG header")
        .materialize()
        .expect_err("corrupt IDAT must fail source decoding");

    for selected in [Backend::Simd, Backend::Gpu] {
        let source = Image::open_bytes(corrupt.clone()).expect("inspectable corrupt PNG");
        let mode = source.mode().expect("header mode");
        let mut draw = Draw::new(source, Some(mode));
        draw.point(&[(0, 0)], (3, 3, 3, 255))
            .expect("queue unsupported point without decoding");
        let error = draw
            .into_image()
            .use_backend(selected)
            .materialize()
            .expect_err("unsupported backend must win before corrupt source decode");
        let expected = match selected {
            Backend::Simd => "SIMD: no native impl for DrawPoint",
            Backend::Gpu => "GPU: no native impl for DrawPoint",
            Backend::Cpu => unreachable!(),
        };
        assert_value_error(
            &format!("pre-materialization DrawPoint [{selected:?}]"),
            error,
            expected,
        );
    }
}

#[test]
fn gpu_auxiliary_failures_follow_operation_then_slot_order() {
    let palette = (0..=255u8)
        .flat_map(|index| [index, index, index])
        .collect::<Vec<_>>();
    let mut plain_source = Image::frombytes("P", (2, 1), &[1, 2]).expect("first paste source");
    plain_source
        .putpalette(&palette, "RGB")
        .expect("first source palette");

    let deferred_mask = Image::frombytes("1", (2, 1), &[0]).expect("first paste mask");
    let mask_mode = deferred_mask.mode().expect("mask mode");
    let mut mask_draw = Draw::new(deferred_mask, Some(mask_mode));
    mask_draw
        .point(&[(0, 0)], (255, 255, 255, 255))
        .expect("queue first operation's failing mask");
    let deferred_mask = mask_draw.into_image().use_backend(Backend::Simd);

    let mut deferred_source = Image::frombytes("P", (2, 1), &[3, 4]).expect("second paste source");
    deferred_source
        .putpalette(&palette, "RGB")
        .expect("second source palette");
    let source_mode = deferred_source.mode().expect("source mode");
    let mut source_draw = Draw::new(deferred_source, Some(source_mode));
    source_draw
        .line(0, 0, 1, 0, (1, 1, 1, 255), 1)
        .expect("queue second operation's failing source");
    let deferred_source = source_draw.into_image().use_backend(Backend::Gpu);

    let mut destination = Image::frombytes("P", (2, 1), &[0, 0]).expect("paste destination");
    destination
        .putpalette(&palette, "RGB")
        .expect("destination palette");
    let mut destination = destination
        .crop((0, 0, 2, 1))
        .expect("queue supported outer operation")
        .use_backend(Backend::Gpu);
    destination
        .paste_at(
            PasteSource::Image(plain_source),
            Some((0, 0)),
            Some(&deferred_mask),
        )
        .expect("queue first paste");
    destination
        .paste_at(PasteSource::Image(deferred_source), Some((0, 0)), None)
        .expect("queue second paste");

    let error = destination
        .materialize()
        .expect_err("first operation's third slot must fail before second operation's source");
    assert_value_error(
        "GPU auxiliary operation/slot order",
        error,
        "SIMD: no native impl for DrawPoint",
    );
}

#[test]
fn putalpha_and_core_sample_replacement_match_on_each_native_backend() {
    let available = compute::available_backends();
    for required in [Backend::Cpu, Backend::Simd, Backend::Gpu] {
        assert!(
            available.contains(&required),
            "forced-backend parity requires {required:?}; compiled backends: {available:?}"
        );
    }

    let registry = compute::registry::registry();
    for (operation, shader) in [("PutAlpha", "put_alpha.wgsl"), ("PutData", "put_data.wgsl")] {
        let entry = registry
            .get(operation)
            .unwrap_or_else(|| panic!("{operation} must be registered"));
        assert!(
            entry.cpu_fn.is_some(),
            "{operation} must have a CPU implementation"
        );
        assert!(
            entry.simd_fn.is_some(),
            "{operation} must have a dedicated SIMD implementation"
        );
        assert_eq!(
            entry.gpu_shader,
            Some(shader),
            "{operation} must have its own GPU shader"
        );
        assert!(
            entry.gpu_source.is_some(),
            "{operation} must embed its GPU implementation"
        );
    }

    for (mode, pixels, message) in [
        ("1", vec![0x80], "conversion from 1 to LA not supported"),
        (
            "YCbCr",
            vec![1, 2, 3],
            "conversion from YCbCr to RGBA not supported",
        ),
        (
            "HSV",
            vec![1, 2, 3],
            "conversion from HSV to RGBA not supported",
        ),
        (
            "I",
            vec![1, 0, 0, 0],
            "conversion from I to LA not supported",
        ),
        (
            "F",
            1.0f32.to_le_bytes().to_vec(),
            "conversion from F to LA not supported",
        ),
    ] {
        let mut image =
            Image::frombytes(mode, (1, 1), &pixels).expect("unsupported putalpha input");
        let error = image
            .putalpha(128)
            .expect_err("unsupported putalpha mode must fail before dispatch");
        assert_eq!(error.to_string(), message, "putalpha {mode} exact error");
    }

    let palette = vec![1, 2, 3, 4, 5, 6, 7, 8, 9];
    for selected in [Backend::Cpu, Backend::Simd, Backend::Gpu] {
        let backend_name = format!("{selected:?}");

        let mut luma = Image::frombytes("L", (3, 1), &[7, 99, 250]).expect("L input");
        luma.putalpha(128).expect("L putalpha");
        let luma = luma.use_backend(selected);
        assert_exact_samples(
            &format!("putalpha L [{backend_name}]"),
            &luma,
            "LA",
            &[7, 128, 99, 128, 250, 128],
        );

        let mut luma_alpha =
            Image::frombytes("LA", (3, 1), &[7, 8, 99, 100, 250, 251]).expect("LA input");
        luma_alpha.putalpha(128).expect("LA putalpha");
        let luma_alpha = luma_alpha.use_backend(selected);
        assert_exact_samples(
            &format!("putalpha LA [{backend_name}]"),
            &luma_alpha,
            "LA",
            &[7, 128, 99, 128, 250, 128],
        );

        let mut rgb = Image::frombytes("RGB", (3, 1), &[1, 2, 3, 40, 50, 60, 250, 0, 128])
            .expect("RGB input");
        rgb.putalpha(128).expect("RGB putalpha");
        let rgb = rgb.use_backend(selected);
        assert_exact_samples(
            &format!("putalpha RGB [{backend_name}]"),
            &rgb,
            "RGBA",
            &[1, 2, 3, 128, 40, 50, 60, 128, 250, 0, 128, 128],
        );

        let mut rgba = Image::frombytes(
            "RGBA",
            (3, 1),
            &[1, 2, 3, 4, 40, 50, 60, 70, 250, 0, 128, 255],
        )
        .expect("RGBA input");
        rgba.putalpha(128).expect("RGBA putalpha");
        let rgba = rgba.use_backend(selected);
        assert_exact_samples(
            &format!("putalpha RGBA [{backend_name}]"),
            &rgba,
            "RGBA",
            &[1, 2, 3, 128, 40, 50, 60, 128, 250, 0, 128, 128],
        );

        let mut cmyk = Image::frombytes(
            "CMYK",
            (3, 1),
            &[50, 100, 150, 200, 0, 255, 17, 0, 255, 0, 255, 128],
        )
        .expect("CMYK input");
        cmyk.putalpha(128).expect("CMYK putalpha");
        let cmyk = cmyk.use_backend(selected);
        assert_exact_samples(
            &format!("putalpha CMYK [{backend_name}]"),
            &cmyk,
            "RGBA",
            &[44, 33, 23, 128, 255, 0, 238, 128, 0, 127, 0, 128],
        );

        let mut paletted = Image::frombytes("P", (3, 1), &[0, 2, 1]).expect("P putalpha input");
        paletted
            .putpalette(&palette, "RGB")
            .expect("P putalpha palette");
        let source_palette = paletted.palette();
        paletted.putalpha(128).expect("P putalpha");
        let paletted = paletted.use_backend(selected);
        assert_exact_samples(
            &format!("putalpha P [{backend_name}]"),
            &paletted,
            "PA",
            &[0, 128, 2, 128, 1, 128],
        );
        assert_eq!(
            paletted.getbands().expect("PA bands"),
            vec!["P".to_owned(), "A".to_owned()],
            "putalpha P [{backend_name}]: bands"
        );
        assert_eq!(
            paletted.palette(),
            source_palette,
            "putalpha P [{backend_name}]: palette"
        );

        let mut repeated_alpha =
            Image::frombytes("P", (3, 1), &[0, 2, 1]).expect("repeated P putalpha input");
        repeated_alpha
            .putpalette(&palette, "RGB")
            .expect("repeated P putalpha palette");
        let source_palette = repeated_alpha.palette();
        repeated_alpha
            .putalpha(128)
            .expect("first P to PA putalpha");
        repeated_alpha.putalpha(17).expect("second PA putalpha");
        let repeated_alpha = repeated_alpha.use_backend(selected);
        assert_exact_samples(
            &format!("putalpha PA [{backend_name}]"),
            &repeated_alpha,
            "PA",
            &[0, 17, 2, 17, 1, 17],
        );
        assert_eq!(
            repeated_alpha.palette(),
            source_palette,
            "putalpha PA [{backend_name}]: palette"
        );

        let mut pa_data = Image::frombytes("P", (3, 1), &[0, 2, 1]).expect("PA putdata input");
        pa_data
            .putpalette(&palette, "RGB")
            .expect("PA putdata palette");
        let source_palette = pa_data.palette();
        pa_data.putalpha(128).expect("P to PA before putdata");
        pa_data
            .putdata(&[9, 33])
            .expect("one complete PA replacement");
        let pa_data = pa_data.use_backend(selected);
        assert_exact_samples(
            &format!("putdata PA [{backend_name}]"),
            &pa_data,
            "PA",
            &[9, 33, 2, 128, 1, 128],
        );
        assert_eq!(
            pa_data.palette(),
            source_palette,
            "putdata PA [{backend_name}]: palette"
        );

        let mut pa_crop = Image::frombytes("P", (3, 1), &[0, 2, 1]).expect("PA crop input");
        pa_crop
            .putpalette(&palette, "RGB")
            .expect("PA crop palette");
        let source_palette = pa_crop.palette();
        pa_crop.putalpha(128).expect("P to PA before crop");
        let pa_crop = pa_crop
            .crop_box(1, 0, 3, 1)
            .expect("PA crop")
            .use_backend(selected);
        assert_eq!(
            pa_crop.size().expect("PA crop size"),
            (2, 1),
            "crop PA [{backend_name}]: size"
        );
        assert_exact_samples(
            &format!("crop PA [{backend_name}]"),
            &pa_crop,
            "PA",
            &[2, 128, 1, 128],
        );
        assert_eq!(
            pa_crop.palette(),
            source_palette,
            "crop PA [{backend_name}]: palette"
        );

        let mut paste_pa_destination =
            Image::frombytes("P", (3, 1), &[0, 1, 0]).expect("PA paste destination");
        paste_pa_destination
            .putpalette(&palette, "RGB")
            .expect("PA paste destination palette");
        paste_pa_destination
            .putalpha(128)
            .expect("PA paste destination alpha");
        let mut paste_pa_source =
            Image::frombytes("P", (1, 1), &[2]).expect("PA paste source indices");
        paste_pa_source
            .putpalette(&palette, "RGB")
            .expect("PA paste source palette");
        paste_pa_source.putalpha(33).expect("PA paste source alpha");
        paste_pa_destination
            .paste_at(PasteSource::Image(paste_pa_source), Some((1, 0)), None)
            .expect("paste PA source into PA destination");
        let paste_pa_destination = paste_pa_destination.use_backend(selected);
        assert_exact_samples(
            &format!("paste PA into PA [{backend_name}]"),
            &paste_pa_destination,
            "PA",
            &[0, 128, 2, 33, 0, 128],
        );

        let mut paste_p_destination =
            Image::frombytes("P", (3, 1), &[0, 1, 0]).expect("P-to-PA paste destination");
        paste_p_destination
            .putpalette(&palette, "RGB")
            .expect("P-to-PA paste destination palette");
        paste_p_destination
            .putalpha(128)
            .expect("P-to-PA paste destination alpha");
        let mut paste_p_source =
            Image::frombytes("P", (1, 1), &[2]).expect("P paste source indices");
        paste_p_source
            .putpalette(&palette, "RGB")
            .expect("P paste source palette");
        paste_p_destination
            .paste_at(PasteSource::Image(paste_p_source), Some((1, 0)), None)
            .expect("paste P source into PA destination");
        let paste_p_destination = paste_p_destination.use_backend(selected);
        assert_exact_samples(
            &format!("paste P into PA [{backend_name}]"),
            &paste_p_destination,
            "PA",
            &[0, 128, 2, 255, 0, 128],
        );

        for (source, expected, label) in [
            (PasteSource::Scalar(2), [0, 128, 2, 0, 0, 128], "scalar"),
            (
                PasteSource::LumaAlpha(2, 33),
                [0, 128, 2, 33, 0, 128],
                "two-band",
            ),
        ] {
            let mut solid_destination =
                Image::frombytes("P", (3, 1), &[0, 1, 0]).expect("PA solid paste destination");
            solid_destination
                .putpalette(&palette, "RGB")
                .expect("PA solid paste palette");
            solid_destination
                .putalpha(128)
                .expect("PA solid paste destination alpha");
            solid_destination
                .paste(source, Some((1, 0, 2, 1)), None)
                .expect("solid PA paste");
            let solid_destination = solid_destination.use_backend(selected);
            assert_exact_samples(
                &format!("paste PA {label} [{backend_name}]"),
                &solid_destination,
                "PA",
                &expected,
            );
        }

        let mut paste_before_alpha =
            Image::frombytes("P", (3, 1), &[0, 1, 0]).expect("P paste-before-alpha destination");
        paste_before_alpha
            .putpalette(&palette, "RGB")
            .expect("P paste-before-alpha destination palette");
        let mut paste_before_alpha_source =
            Image::frombytes("P", (1, 1), &[2]).expect("P paste-before-alpha source");
        paste_before_alpha_source
            .putpalette(&palette, "RGB")
            .expect("P paste-before-alpha source palette");
        paste_before_alpha
            .paste_at(
                PasteSource::Image(paste_before_alpha_source),
                Some((1, 0)),
                None,
            )
            .expect("paste P before putalpha");
        paste_before_alpha
            .putalpha(33)
            .expect("promote pasted P samples to PA");
        let paste_before_alpha = paste_before_alpha.use_backend(selected);
        assert_exact_samples(
            &format!("paste P then putalpha [{backend_name}]"),
            &paste_before_alpha,
            "PA",
            &[0, 33, 2, 33, 0, 33],
        );

        let mut data_before_alpha =
            Image::frombytes("P", (3, 1), &[0, 1, 0]).expect("P putdata-before-alpha input");
        data_before_alpha
            .putpalette(&palette, "RGB")
            .expect("P putdata-before-alpha palette");
        data_before_alpha
            .putdata(&[2])
            .expect("replace one P sample before putalpha");
        data_before_alpha
            .putalpha(33)
            .expect("promote replaced P samples to PA");
        let data_before_alpha = data_before_alpha.use_backend(selected);
        assert_exact_samples(
            &format!("putdata P then putalpha [{backend_name}]"),
            &data_before_alpha,
            "PA",
            &[2, 33, 1, 33, 0, 33],
        );

        let mut crop_before_alpha =
            Image::frombytes("P", (3, 1), &[0, 1, 2]).expect("P crop-before-alpha input");
        crop_before_alpha
            .putpalette(&palette, "RGB")
            .expect("P crop-before-alpha palette");
        let mut crop_before_alpha = crop_before_alpha
            .crop_box(1, 0, 3, 1)
            .expect("crop P before putalpha");
        crop_before_alpha
            .putalpha(33)
            .expect("promote cropped P samples to PA");
        let crop_before_alpha = crop_before_alpha.use_backend(selected);
        assert_exact_samples(
            &format!("crop P then putalpha [{backend_name}]"),
            &crop_before_alpha,
            "PA",
            &[1, 33, 2, 33],
        );

        let rgb_palette = vec![255, 0, 0, 0, 255, 0, 0, 0, 255];
        let mut alpha_then_convert =
            Image::frombytes("P", (3, 1), &[0, 2, 1]).expect("P conversion input");
        alpha_then_convert
            .putpalette(&rgb_palette, "RGB")
            .expect("P conversion palette");
        alpha_then_convert
            .putalpha(128)
            .expect("P to PA before conversion");
        let alpha_then_convert = alpha_then_convert
            .convert("RGBA", None, None, None, None)
            .expect("PA to RGBA conversion")
            .use_backend(selected);
        assert_exact_samples(
            &format!("putalpha P then convert RGBA [{backend_name}]"),
            &alpha_then_convert,
            "RGBA",
            &[255, 0, 0, 128, 0, 0, 255, 128, 0, 255, 0, 128],
        );

        let mut data_then_convert =
            Image::frombytes("P", (3, 1), &[0, 2, 1]).expect("PA data conversion input");
        data_then_convert
            .putpalette(&rgb_palette, "RGB")
            .expect("PA data conversion palette");
        data_then_convert
            .putalpha(128)
            .expect("P to PA before sample replacement");
        data_then_convert
            .putdata(&[2, 33, 1, 44, 0, 55])
            .expect("complete PA sample replacement");
        let data_then_convert = data_then_convert
            .convert("RGBA", None, None, None, None)
            .expect("mutated PA to RGBA conversion")
            .use_backend(selected);
        assert_exact_samples(
            &format!("putalpha and putdata PA then convert RGBA [{backend_name}]"),
            &data_then_convert,
            "RGBA",
            &[0, 0, 255, 33, 0, 255, 0, 44, 255, 0, 0, 55],
        );

        let rgba_palette = vec![
            255, 0, 0, 0, //
            0, 255, 0, 64, //
            0, 0, 255, 255,
        ];
        let mut palette_alpha_ignored =
            Image::frombytes("P", (3, 1), &[0, 1, 2]).expect("RGBA-palette P input");
        palette_alpha_ignored
            .putpalette(&rgba_palette, "RGBA")
            .expect("RGBA palette");
        palette_alpha_ignored
            .putalpha(128)
            .expect("P to PA with RGBA palette");
        let palette_alpha_ignored = palette_alpha_ignored
            .convert("RGBA", None, None, None, None)
            .expect("PA with RGBA palette to RGBA")
            .use_backend(selected);
        assert_exact_samples(
            &format!("PA sample alpha overrides palette alpha [{backend_name}]"),
            &palette_alpha_ignored,
            "RGBA",
            &[255, 0, 0, 128, 0, 255, 0, 128, 0, 0, 255, 128],
        );

        let mut paletted = Image::frombytes("P", (3, 1), &[0, 2, 1]).expect("P putdata input");
        paletted
            .putpalette(&palette, "RGB")
            .expect("P putdata palette");
        let source_palette = paletted.palette();
        paletted.putdata(&[128]).expect("P putdata");
        let paletted = paletted.use_backend(selected);
        assert_exact_samples(
            &format!("putdata P [{backend_name}]"),
            &paletted,
            "P",
            &[128, 2, 1],
        );
        assert_eq!(
            paletted.palette(),
            source_palette,
            "putdata P [{backend_name}]: palette"
        );

        let mut luma_alpha =
            Image::frombytes("LA", (3, 1), &[7, 8, 99, 100, 250, 251]).expect("LA putdata input");
        luma_alpha.putdata(&[128, 17]).expect("LA putdata");
        let luma_alpha = luma_alpha.use_backend(selected);
        assert_exact_samples(
            &format!("putdata LA [{backend_name}]"),
            &luma_alpha,
            "LA",
            &[128, 17, 99, 100, 250, 251],
        );

        let mut rgb = Image::frombytes("RGB", (3, 1), &[1, 2, 3, 40, 50, 60, 250, 0, 128])
            .expect("RGB putdata input");
        rgb.putdata(&[128, 0, 0]).expect("RGB putdata");
        let rgb = rgb.use_backend(selected);
        assert_exact_samples(
            &format!("putdata RGB [{backend_name}]"),
            &rgb,
            "RGB",
            &[128, 0, 0, 40, 50, 60, 250, 0, 128],
        );

        let mut rgba = Image::frombytes(
            "RGBA",
            (3, 1),
            &[1, 2, 3, 4, 40, 50, 60, 70, 250, 0, 128, 255],
        )
        .expect("RGBA putdata input");
        rgba.putdata(&[128, 0, 0, 17]).expect("RGBA putdata");
        let rgba = rgba.use_backend(selected);
        assert_exact_samples(
            &format!("putdata RGBA [{backend_name}]"),
            &rgba,
            "RGBA",
            &[128, 0, 0, 17, 40, 50, 60, 70, 250, 0, 128, 255],
        );

        let mut cmyk = Image::frombytes(
            "CMYK",
            (3, 1),
            &[50, 100, 150, 200, 0, 255, 17, 0, 255, 0, 255, 128],
        )
        .expect("CMYK putdata input");
        cmyk.putdata(&[128, 0, 0, 0]).expect("CMYK putdata");
        let cmyk = cmyk.use_backend(selected);
        assert_exact_samples(
            &format!("putdata CMYK [{backend_name}]"),
            &cmyk,
            "CMYK",
            &[128, 0, 0, 0, 0, 255, 17, 0, 255, 0, 255, 128],
        );

        let mut binary = Image::new(1, 1, "1", (0, 0, 0, 255)).expect("mode 1 input");
        binary.putdata(&[2]).expect("mode 1 putdata");
        let binary = binary.use_backend(selected);
        assert_eq!(
            binary.getdata(None).expect("mode 1 logical samples"),
            vec![2],
            "mode 1 must retain the assigned value on {selected:?}"
        );
        assert_eq!(
            binary.getpixel(0, 0).expect("mode 1 pixel").0,
            2,
            "mode 1 getpixel must expose the assigned value on {selected:?}"
        );
        assert_eq!(
            binary.tobytes().expect("packed mode 1 bytes"),
            vec![0x80],
            "mode 1 tobytes packs truthiness on {selected:?}"
        );

        let i_initial = [111i32, 222, 333]
            .into_iter()
            .flat_map(i32::to_le_bytes)
            .collect::<Vec<_>>();
        let mut integer = Image::frombytes("I", (3, 1), &i_initial).expect("I putdata input");
        integer.putdata(&(-7i32).to_le_bytes()).expect("I putdata");
        let integer = integer.use_backend(selected);
        let i_expected = [-7i32, 222, 333]
            .into_iter()
            .flat_map(i32::to_le_bytes)
            .collect::<Vec<_>>();
        assert_exact_samples(
            &format!("putdata I [{backend_name}]"),
            &integer,
            "I",
            &i_expected,
        );

        let f_initial = [1.25f32, 2.5, 3.75]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>();
        let mut float = Image::frombytes("F", (3, 1), &f_initial).expect("F putdata input");
        float.putdata(&(-1.5f32).to_le_bytes()).expect("F putdata");
        let float = float.use_backend(selected);
        let f_expected = [-1.5f32, 2.5, 3.75]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>();
        assert_exact_samples(
            &format!("putdata F [{backend_name}]"),
            &float,
            "F",
            &f_expected,
        );

        // These normalization assertions cover the bulk Rust-core API. Public
        // host sequences use immediate CPU writes so each Python coercion
        // callback can observe the prior pixel; manifest targets deliberately
        // do not infer public SIMD/GPU support from this core-only contract.
        let mut normalized_rgb =
            Image::frombytes("RGB", (2, 1), &[11, 12, 13, 40, 41, 42]).expect("RGB value input");
        normalized_rgb
            .putdata_values(&[PutDataValue::Packed(0x010203)], 17.0, 23.0)
            .expect("packed RGB putdata value");
        let normalized_rgb = normalized_rgb.use_backend(selected);
        assert_exact_samples(
            &format!("normalized packed RGB putdata [{backend_name}]"),
            &normalized_rgb,
            "RGB",
            &[3, 2, 1, 40, 41, 42],
        );

        let mut normalized_integer =
            Image::frombytes("I", (2, 1), &[111, 0, 0, 0, 222, 0, 0, 0]).expect("I value input");
        normalized_integer
            .putdata_values(&[PutDataValue::Number(-2.2)], 2.0, 3.0)
            .expect("scaled I putdata value");
        let normalized_integer = normalized_integer.use_backend(selected);
        let normalized_integer_expected = [-1i32, 222]
            .into_iter()
            .flat_map(i32::to_le_bytes)
            .collect::<Vec<_>>();
        assert_exact_samples(
            &format!("normalized scaled I putdata [{backend_name}]"),
            &normalized_integer,
            "I",
            &normalized_integer_expected,
        );
    }
}

#[test]
fn pa_draw_point_is_native_on_cpu_and_truthfully_rejected_elsewhere() {
    let entry = compute::registry::registry()
        .get("DrawPoint")
        .expect("DrawPoint registration");
    assert!(
        entry.cpu_fn.is_some(),
        "DrawPoint must have a CPU implementation"
    );
    assert!(
        entry.simd_fn.is_none(),
        "DrawPoint must not claim a SIMD implementation"
    );
    assert!(
        entry.gpu_shader.is_none(),
        "DrawPoint must not claim a GPU shader"
    );

    for selected in [Backend::Simd, Backend::Gpu] {
        let mut image = Image::frombytes("P", (3, 1), &[0, 1, 0]).expect("PA draw destination");
        image
            .putpalette(&[255, 0, 0, 0, 255, 0, 0, 0, 255], "RGB")
            .expect("PA draw palette");
        image.putalpha(128).expect("PA draw alpha");
        let mut draw = Draw::new(image, Some("PA".to_owned()));
        draw.point(&[(1, 0)], (2, 2, 2, 33))
            .expect("queue PA point");
        let image = draw.into_image().use_backend(selected);

        let expected = match selected {
            Backend::Simd => "SIMD: no native impl for DrawPoint",
            Backend::Gpu => "GPU: no native impl for DrawPoint",
            Backend::Cpu => unreachable!(),
        };
        let error = image
            .materialize()
            .expect_err("forced backend must reject unsupported PA DrawPoint");
        assert_value_error(&format!("PA DrawPoint [{selected:?}]"), error, expected);
    }
}

#[test]
fn indexed_bitmap_and_text_preserve_pillow_format_and_pending_transparency() {
    for case in manifest().indexed_immediate_draw_cases {
        let input = fs::read(fixture_root().join(&case.input))
            .unwrap_or_else(|error| panic!("{} input: {error}", case.id));
        let target =
            Image::open_bytes(input).unwrap_or_else(|error| panic!("{} open: {error}", case.id));
        let mut draw = Draw::new(target, Some("P".to_owned()));
        match case.operation.as_str() {
            "bitmap" => {
                let mask = Image::frombytes("L", (1, 1), &[255]).expect("bitmap mask");
                let xy = case.parameters["xy"].as_array().expect("bitmap xy");
                let fill = byte(&case.parameters["fill"]);
                draw.bitmap(
                    xy[0].as_i64().expect("bitmap x") as i32,
                    xy[1].as_i64().expect("bitmap y") as i32,
                    &mask,
                    Some((fill, fill, fill, 255)),
                )
                .expect("draw indexed bitmap");
            }
            "text" => {
                let xy = case.parameters["xy"].as_array().expect("text xy");
                let fill = byte(&case.parameters["fill"]);
                let font = Font::load_default(
                    case.parameters["font_size"]
                        .as_f64()
                        .expect("default font size") as f32,
                )
                .expect("Pillow default font");
                draw.text(
                    xy[0].as_i64().expect("text x") as i32,
                    xy[1].as_i64().expect("text y") as i32,
                    case.parameters["text"].as_str().expect("text"),
                    &font,
                    (fill, fill, fill, 255),
                )
                .expect("draw indexed text");
            }
            operation => panic!("unsupported indexed immediate draw operation {operation}"),
        }
        let actual = draw.into_image();
        assert_eq!(
            actual.mode().expect("mode"),
            case.expected.mode,
            "{}",
            case.id
        );
        assert_eq!(
            actual.size().expect("size"),
            (case.expected.size[0], case.expected.size[1]),
            "{}",
            case.id
        );
        assert_eq!(
            actual.tobytes().expect("pixels"),
            decode_hex(&case.expected.pixels_hex),
            "{} exact Pillow pixels",
            case.id
        );
        assert_eq!(
            actual.getpalette_trimmed(),
            case.expected.palette_hex.as_deref().map(decode_hex),
            "{} exact palette",
            case.id
        );
        assert_eq!(
            actual.format_name().as_deref(),
            Some(case.expected.format.as_str()),
            "{} source format",
            case.id
        );
        assert_eq!(
            transparency_info(&actual),
            case.expected.info,
            "{}",
            case.id
        );
        assert_eq!(
            actual.palette_mode(),
            Some(case.expected.palette_mode.as_str()),
            "{}",
            case.id
        );
        assert_eq!(
            actual.has_transparency_data(),
            case.expected.has_transparency_data,
            "{}",
            case.id
        );
        assert_eq!(
            actual.getpalette_rgba(),
            Some(decode_hex(&case.expected.palette_rgba_hex)),
            "{} exact RGBA palette",
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
            let selected = backend(backend_name);
            let expected = match selected {
                Backend::Gpu => {
                    assert!(
                        entry.gpu_shader.is_none(),
                        "{} must not claim a GPU shader",
                        case.id
                    );
                    format!("GPU: no native impl for {key}")
                }
                Backend::Simd => {
                    assert!(
                        entry.simd_fn.is_none(),
                        "{} must not claim a SIMD function",
                        case.id
                    );
                    format!("SIMD: no native impl for {key}")
                }
                Backend::Cpu => panic!("CPU cannot be listed as unsupported"),
            };
            let error = draw_case(&case, selected)
                .materialize()
                .expect_err("forced backend must reject unsupported drawing");
            assert_value_error(&format!("{} [{backend_name}]", case.id), error, &expected);
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
fn indexed_ops_are_exact_on_every_declared_native_backend() {
    let available = compute::available_backends();
    let registry = compute::registry::registry();
    for (key, shader) in [
        ("InvertChops", "invert_chops.wgsl"),
        ("RemapPalette", "remap_palette.wgsl"),
        ("Transform", "transform.wgsl"),
        ("CompositeModule", "composite_module.wgsl"),
    ] {
        let entry = registry
            .get(key)
            .unwrap_or_else(|| panic!("{key} must be registered"));
        assert!(
            entry.cpu_fn.is_some(),
            "{key} must have a CPU implementation"
        );
        assert!(
            entry.simd_fn.is_some(),
            "{key} must have a dedicated SIMD-pool implementation"
        );
        assert_eq!(entry.gpu_shader, Some(shader), "{key} GPU shader");
        assert!(
            entry.gpu_source.is_some(),
            "{key} must embed its GPU shader"
        );
    }

    let palette = [10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120];
    let mut source = Image::frombytes("P", (4, 1), &[0, 1, 2, 3]).expect("indexed source image");
    source
        .putpalette(&palette, "RGB")
        .expect("indexed source palette");

    let mut image2 = Image::frombytes("P", (4, 1), &[8, 7, 6, 5]).expect("indexed background");
    let image2_palette = [1, 2, 3, 4, 5, 6, 7, 8, 9];
    image2
        .putpalette(&image2_palette, "RGB")
        .expect("indexed background palette");
    let mask = Image::frombytes("L", (4, 1), &[0, 127, 128, 255]).expect("composite mask");

    for selected in [Backend::Cpu, Backend::Simd, Backend::Gpu] {
        assert!(
            available.contains(&selected),
            "forced-backend parity requires {selected:?}; compiled backends: {available:?}"
        );

        let inverted = chops::invert(&source)
            .expect("invert P")
            .use_backend(selected);
        assert_eq!(inverted.mode().expect("invert mode"), "P");
        assert_eq!(
            inverted.tobytes().expect("invert indices"),
            [255, 254, 253, 252]
        );
        assert_eq!(
            inverted.getpalette_trimmed(),
            Some(Vec::new()),
            "Pillow allocates ImageChops.invert(P) without a palette"
        );

        let remapped = source
            .remap_palette(&[2, 0, 3])
            .expect("remap P")
            .use_backend(selected);
        assert_eq!(remapped.mode().expect("remap mode"), "P");
        assert_eq!(remapped.tobytes().expect("remapped indices"), [1, 0, 0, 2]);
        assert_eq!(
            remapped.getpalette_trimmed(),
            Some(vec![70, 80, 90, 10, 20, 30, 100, 110, 120])
        );

        let transformed = source
            .transform_affine((4, 1), &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0], (9, 9, 9, 255))
            .expect("transform P")
            .use_backend(selected);
        assert_eq!(transformed.mode().expect("transform mode"), "P");
        assert_eq!(
            transformed.tobytes().expect("transformed indices"),
            [0, 1, 2, 3]
        );
        assert_eq!(transformed.getpalette_trimmed(), Some(palette.to_vec()));

        let composited = module_fns::composite(&source, &image2, &mask)
            .expect("composite P")
            .use_backend(selected);
        assert_eq!(composited.mode().expect("composite mode"), "P");
        assert_eq!(
            composited.tobytes().expect("composite indices"),
            [8, 4, 4, 3]
        );
        assert_eq!(
            composited.getpalette_trimmed(),
            Some(image2_palette.to_vec()),
            "Image.composite(P) owns image2's palette"
        );
    }
}

#[test]
fn indexed_affine_fill_and_raw_rgba_palette_match_pillow_12_2() {
    let available = compute::available_backends();
    let mut palette = vec![0; 30];
    palette[15..18].copy_from_slice(&[1, 2, 3]);
    palette[18..21].copy_from_slice(&[4, 5, 6]);
    palette[21..24].copy_from_slice(&[7, 8, 9]);
    palette[27..30].copy_from_slice(&[255, 0, 0]);
    let mut source = Image::frombytes("P", (2, 1), &[5, 6]).expect("indexed affine source");
    source
        .putpalette(&palette, "RGB")
        .expect("indexed affine palette");
    let matrix = [1.0, 0.0, -1.0, 0.0, 1.0, 0.0];

    let mut rgba_palette = vec![0; 1024];
    rgba_palette[..16].copy_from_slice(&[
        10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120, 130, 140, 150, 160,
    ]);
    let l_source = Image::frombytes("L", (4, 1), &[0, 1, 2, 3]).expect("L remap source");
    let mut p_source = Image::frombytes("P", (4, 1), &[0, 1, 2, 3]).expect("P remap source");
    p_source
        .putpalette(&[1, 2, 3, 4, 5, 6, 7, 8, 9], "RGB")
        .expect("P attached palette");

    for selected in [Backend::Cpu, Backend::Simd, Backend::Gpu] {
        assert!(
            available.contains(&selected),
            "forced-backend parity requires {selected:?}; compiled backends: {available:?}"
        );

        let tuple_fill = source
            .transform_affine((4, 1), &matrix, (255, 0, 0, 255))
            .expect("tuple fill transform")
            .use_backend(selected);
        assert_eq!(tuple_fill.size().expect("tuple fill size"), (4, 1));
        assert_eq!(
            tuple_fill.tobytes().expect("tuple fill indices"),
            [0, 5, 6, 0],
            "Pillow resolves tuple/string P fill colors to index zero"
        );
        assert_eq!(tuple_fill.getpalette_trimmed(), Some(palette.clone()));

        let scalar_fill = source
            .transform_affine_palette_index((4, 1), &matrix, 7)
            .expect("scalar fill transform")
            .use_backend(selected);
        assert_eq!(scalar_fill.size().expect("scalar fill size"), (4, 1));
        assert_eq!(
            scalar_fill.tobytes().expect("scalar fill indices"),
            [7, 5, 6, 7],
            "Pillow preserves a scalar P fill as its raw palette index"
        );
        assert_eq!(scalar_fill.getpalette_trimmed(), Some(palette.clone()));

        for remap_source in [&l_source, &p_source] {
            let remapped = remap_source
                .remap_palette_with_source(&[2, 0, 3], Some(&rgba_palette))
                .expect("RGBA source palette remap")
                .use_backend(selected);
            assert_eq!(remapped.mode().expect("remap mode"), "P");
            assert_eq!(remapped.tobytes().expect("remapped indices"), [1, 0, 0, 2]);
            assert_eq!(
                remapped.getpalette_trimmed(),
                Some(vec![90, 100, 110, 10, 20, 30, 130, 140, 150])
            );
            assert_eq!(remapped.palette_alpha(), Some(vec![120, 40, 160]));
            assert_eq!(remapped.palette_mode(), Some("RGBA"));
        }
    }
}

#[test]
fn rgba_fast_octree_retains_exact_pillow_12_2_palette_alpha() {
    let source = Image::frombytes(
        "RGBA",
        (4, 1),
        &[255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 64, 99, 88, 77, 0],
    )
    .expect("RGBA quantize source");

    let two = source
        .quantize(2, 0, None, false)
        .expect("FASTOCTREE colors=2");
    assert_eq!(two.tobytes().expect("colors=2 indices"), [0, 1, 0, 0]);
    assert_eq!(two.getpalette_trimmed(), Some(vec![99, 88, 77, 0, 255, 0]));
    assert_eq!(two.palette_alpha(), Some(vec![0, 128]));
    assert_eq!(two.palette_mode(), Some("RGBA"));

    let four = source
        .quantize(4, 0, None, false)
        .expect("FASTOCTREE colors=4");
    assert_eq!(four.tobytes().expect("colors=4 indices"), [3, 1, 2, 0]);
    assert_eq!(
        four.getpalette_trimmed(),
        Some(vec![99, 88, 77, 0, 255, 0, 0, 0, 255, 255, 0, 0])
    );
    assert_eq!(four.palette_alpha(), Some(vec![0, 128, 64, 255]));
    assert_eq!(four.palette_mode(), Some("RGBA"));
}

#[test]
fn quantize_color_bounds_and_single_color_match_pillow_12_2() {
    let rgb = Image::frombytes("RGB", (3, 1), &[10, 20, 30, 250, 240, 230, 20, 70, 130])
        .expect("RGB quantize source");
    let rgb_one = rgb
        .quantize(1, 0, None, false)
        .expect("RGB colors=1 is valid");
    assert_eq!(rgb_one.tobytes().expect("RGB colors=1 indices"), [0, 0, 0]);
    assert_eq!(
        rgb_one.getpalette_trimmed(),
        Some(vec![93, 110, 130]),
        "Pillow median cut retains its rounded single-box centroid"
    );

    let rgba = Image::frombytes(
        "RGBA",
        (3, 1),
        &[10, 20, 30, 40, 250, 240, 230, 220, 20, 70, 130, 190],
    )
    .expect("RGBA quantize source");
    let rgba_one = rgba
        .quantize(1, 0, None, false)
        .expect("RGBA colors=1 is valid");
    assert_eq!(
        rgba_one.tobytes().expect("RGBA colors=1 indices"),
        [0, 0, 0]
    );
    assert_eq!(
        rgba_one.getpalette_trimmed(),
        Some(vec![10, 20, 30]),
        "FASTOCTREE colors=1 keeps Pillow's most-populated coarse bucket"
    );
    assert_eq!(rgba_one.palette_alpha(), Some(vec![40]));

    for colors in [0, 257] {
        match rgb.quantize(colors, 0, None, false) {
            Err(PilError::ValueError(message)) => {
                assert_eq!(message, "bad number of colors");
            }
            other => panic!("colors={colors} should fail exactly, got {other:?}"),
        }
    }
}

#[test]
fn rgba_fast_octree_uses_pillow_float_bucket_average_at_4096_square() {
    const SIDE: u32 = 4096;
    let pixel_count = SIDE as usize * SIDE as usize;
    let mut pixels = vec![255u8; pixel_count * 4];
    pixels[0] = 254;
    let source = Image::frombytes("RGBA", (SIDE, SIDE), &pixels).expect("large FASTOCTREE source");
    drop(pixels);

    let quantized = source
        .quantize(2, 0, None, false)
        .expect("large FASTOCTREE colors=2");
    let palette = quantized
        .getpalette_trimmed()
        .expect("large FASTOCTREE palette");
    assert_eq!(
        &palette[..3],
        &[255, 255, 255],
        "Pillow converts the large bucket sum and count to C float before division"
    );
    assert_eq!(quantized.palette_alpha().as_deref(), Some(&[255, 0][..]));
}

#[test]
fn composite_uses_image2_mode_canvas_and_palette_on_every_backend() {
    let available = compute::available_backends();
    let mask = Image::frombytes("L", (2, 1), &[0, 255]).expect("composite mask");

    let mut p1 = Image::frombytes("P", (2, 1), &[1, 2]).expect("P image1");
    p1.putpalette(&[0, 0, 0, 10, 20, 30, 200, 100, 50], "RGB")
        .expect("P image1 palette");
    let mut p2 = Image::frombytes("P", (2, 1), &[3, 4]).expect("P image2");
    let p2_palette = vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 8, 9, 10];
    p2.putpalette(&p2_palette, "RGB").expect("P image2 palette");
    let l = Image::frombytes("L", (2, 1), &[40, 180]).expect("L source");
    let rgb = Image::frombytes("RGB", (2, 1), &[11, 22, 33, 210, 120, 30]).expect("RGB source");

    let mut large_p2 =
        Image::frombytes("P", (4, 2), &[3, 4, 5, 6, 7, 8, 9, 10]).expect("large image2");
    large_p2
        .putpalette(&p2_palette, "RGB")
        .expect("large image2 palette");

    for selected in [Backend::Cpu, Backend::Simd, Backend::Gpu] {
        assert!(
            available.contains(&selected),
            "forced-backend parity requires {selected:?}; compiled backends: {available:?}"
        );

        let p_over_l = module_fns::composite(&p1, &l, &mask)
            .expect("P over L")
            .use_backend(selected);
        assert_eq!(p_over_l.mode().expect("P over L mode"), "L");
        assert_eq!(p_over_l.tobytes().expect("P over L bytes"), [40, 124]);
        assert_eq!(p_over_l.getpalette_trimmed(), None);

        let l_over_p = module_fns::composite(&l, &p2, &mask)
            .expect("L over P")
            .use_backend(selected);
        assert_eq!(l_over_p.mode().expect("L over P mode"), "P");
        assert_eq!(l_over_p.tobytes().expect("L over P bytes"), [3, 180]);
        assert_eq!(l_over_p.getpalette_trimmed(), Some(p2_palette.clone()));

        let p_over_rgb = module_fns::composite(&p1, &rgb, &mask)
            .expect("P over RGB")
            .use_backend(selected);
        assert_eq!(p_over_rgb.mode().expect("P over RGB mode"), "RGB");
        assert_eq!(
            p_over_rgb.tobytes().expect("P over RGB bytes"),
            [11, 22, 33, 200, 100, 50]
        );
        assert_eq!(p_over_rgb.getpalette_trimmed(), None);

        let rgb_over_p = module_fns::composite(&rgb, &p2, &mask)
            .expect("RGB over P")
            .use_backend(selected);
        assert_eq!(rgb_over_p.mode().expect("RGB over P mode"), "P");
        assert_eq!(rgb_over_p.tobytes().expect("RGB over P bytes"), [3, 32]);
        assert_eq!(rgb_over_p.getpalette_trimmed(), Some(p2_palette.clone()));

        let large = module_fns::composite(&p1, &large_p2, &mask)
            .expect("larger image2 composite")
            .use_backend(selected);
        assert_eq!(large.mode().expect("larger image2 mode"), "P");
        assert_eq!(
            large.tobytes().expect("larger image2 indices"),
            [3, 2, 5, 6, 7, 8, 9, 10]
        );
        assert_eq!(large.getpalette_trimmed(), Some(p2_palette.clone()));
    }
}

#[test]
fn composite_mask_band_and_rgba_output_match_pillow_on_every_backend() {
    let available = compute::available_backends();
    let image1 = Image::frombytes(
        "RGBA",
        (3, 1),
        &[10, 20, 30, 40, 60, 80, 100, 120, 20, 70, 130, 190],
    )
    .expect("RGBA image1");
    let image2 = Image::frombytes(
        "RGBA",
        (3, 1),
        &[250, 240, 230, 220, 200, 180, 160, 140, 220, 160, 100, 40],
    )
    .expect("RGBA image2");
    let rgba_mask = Image::frombytes("RGBA", (3, 1), &[255, 0, 0, 0, 0, 0, 0, 255, 9, 8, 7, 128])
        .expect("adversarial RGBA mask");
    let la_mask =
        Image::frombytes("LA", (3, 1), &[255, 0, 0, 255, 77, 128]).expect("adversarial LA mask");
    let l_mask = Image::frombytes("L", (3, 1), &[0, 255, 128]).expect("L mask");
    let mode1_mask = Image::frombytes("1", (3, 1), &[0b0100_0000]).expect("1 mask");

    let alpha_mask_expected = [250, 240, 230, 220, 60, 80, 100, 120, 120, 115, 115, 115];
    let bit_mask_expected = [250, 240, 230, 220, 60, 80, 100, 120, 220, 160, 100, 40];

    for selected in [Backend::Cpu, Backend::Simd, Backend::Gpu] {
        assert!(
            available.contains(&selected),
            "forced-backend parity requires {selected:?}; compiled backends: {available:?}"
        );

        for (label, mask) in [
            ("RGBA alpha", &rgba_mask),
            ("LA alpha", &la_mask),
            ("L luma", &l_mask),
        ] {
            let composited = module_fns::composite(&image1, &image2, mask)
                .unwrap_or_else(|error| panic!("{label} composite setup failed: {error}"))
                .use_backend(selected);
            assert_eq!(composited.mode().expect("composite mode"), "RGBA");
            assert_eq!(
                composited.tobytes().expect("composite bytes"),
                alpha_mask_expected,
                "{label} mask differs on {selected:?}"
            );
        }

        let mode1 = module_fns::composite(&image1, &image2, &mode1_mask)
            .expect("1 mask composite")
            .use_backend(selected);
        assert_eq!(
            mode1.tobytes().expect("1 mask composite bytes"),
            bit_mask_expected,
            "1 mask must use unpacked 0/255 luma on {selected:?}"
        );
    }

    let invalid_mask = Image::frombytes("RGB", (3, 1), &[0; 9]).expect("invalid RGB mask");
    match module_fns::composite(&image1, &image2, &invalid_mask) {
        Err(PilError::ValueError(message)) => assert_eq!(message, "bad transparency mask"),
        other => panic!("RGB composite mask should fail exactly, got {other:?}"),
    }
}

#[test]
fn backend_lock_reaches_nested_indexed_to_color_pipeline() {
    let available = compute::available_backends();
    let mut source = Image::frombytes("P", (4, 1), &[0, 1, 2, 3]).expect("indexed source");
    source
        .putpalette(&[10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120], "RGB")
        .expect("indexed source palette");

    for selected in [Backend::Cpu, Backend::Simd, Backend::Gpu] {
        assert!(
            available.contains(&selected),
            "forced-backend parity requires {selected:?}; compiled backends: {available:?}"
        );
        let remapped = source.remap_palette(&[2, 0, 3]).expect("nested remap");
        let locked = imageops::grayscale(&remapped)
            .expect("nested P to grayscale")
            .use_backend(selected);
        assert_eq!(locked.backend(), Some(selected));
        let Image::Pipeline { source: nested, .. } = &locked else {
            panic!("P to color boundary must remain a nested pipeline");
        };
        assert_eq!(
            nested.backend(),
            Some(selected),
            "forced backend must reach the palette-preserving nested source"
        );
        assert_eq!(
            locked.tobytes().expect("nested grayscale bytes"),
            [18, 78, 78, 108],
            "{selected:?} nested indexed-to-color output"
        );
    }
}

#[test]
fn apply_transparency_preserves_indices_and_commits_palette_alpha() {
    for case in manifest().apply_transparency_cases {
        for backend_name in &case.backends {
            let selected = backend(backend_name);
            let input = fs::read(fixture_root().join(&case.input))
                .unwrap_or_else(|error| panic!("{} input: {error}", case.id));
            let mut image = Image::open_bytes(input)
                .unwrap_or_else(|error| panic!("{} open: {error}", case.id));
            if let Some(alpha) = case.prepare_alpha {
                image
                    .putalpha(alpha)
                    .unwrap_or_else(|error| panic!("{} prepare PA: {error}", case.id));
            }
            image = image.use_backend(selected);
            assert_eq!(
                transparency_info(&image),
                case.expected.before_info,
                "{} [{backend_name}] exact info before apply_transparency",
                case.id
            );
            assert_eq!(
                image.palette_mode(),
                Some(case.expected.before_palette_mode.as_str()),
                "{} [{backend_name}] palette mode before apply_transparency",
                case.id
            );
            assert_eq!(
                image.has_transparency_data(),
                case.expected.before_has_transparency_data,
                "{} [{backend_name}] transparency before apply_transparency",
                case.id
            );
            image
                .apply_transparency()
                .unwrap_or_else(|error| panic!("{} apply_transparency: {error}", case.id));
            assert_eq!(
                transparency_info(&image),
                case.expected.info,
                "{} [{backend_name}] exact info after apply_transparency",
                case.id
            );
            assert_eq!(
                image.palette_mode(),
                Some(case.expected.palette_mode.as_str()),
                "{} [{backend_name}] palette mode after apply_transparency",
                case.id
            );
            assert_eq!(
                image.has_transparency_data(),
                case.expected.has_transparency_data,
                "{} [{backend_name}] transparency after apply_transparency",
                case.id
            );
            assert_eq!(
                image.mode().expect("mode"),
                case.expected.mode,
                "{} [{backend_name}]",
                case.id
            );
            assert_eq!(
                image.size().expect("size"),
                (case.expected.size[0], case.expected.size[1]),
                "{} [{backend_name}]",
                case.id
            );
            assert_eq!(
                image.tobytes().expect("pixels"),
                decode_hex(&case.expected.pixels_hex),
                "{} [{backend_name}] exact samples",
                case.id
            );
            assert_eq!(
                image.getpalette_rgba(),
                Some(decode_hex(&case.expected.palette_rgba_hex)),
                "{} [{backend_name}] exact RGBA palette",
                case.id
            );
        }
    }
}
