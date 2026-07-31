#![cfg(feature = "image-codecs-all")]

use std::fs;
use std::path::{Path, PathBuf};

use image_slash_star::{ImageError, ImageFormat};
use pillow_rs::Backend;

use pillow_rs::PipelineOp;
use pillow_rs::ResampleFilter;
use pillow_rs::{Image, PilError};
use serde::Deserialize;

#[derive(Deserialize)]
struct Manifest {
    oracle: Oracle,
    decode: Vec<DecodeCase>,
    errors: Vec<ErrorCase>,
}

#[derive(Deserialize)]
struct Oracle {
    implementation: String,
    version: String,
    source: String,
}

#[derive(Deserialize)]
struct DecodeCase {
    id: String,
    feature: String,
    input: String,
    pixels: String,
    format: String,
    mode: String,
    width: u32,
    height: u32,
    palette_hex: Option<String>,
    palette_alpha_hex: Option<String>,
}

#[derive(Deserialize)]
struct ErrorCase {
    id: String,
    feature: String,
    input: String,
    stage: String,
    kind: String,
}

#[derive(Deserialize)]
struct OperationManifest {
    oracle: OperationOracle,
    operations: Vec<OperationCase>,
    errors: Vec<OperationErrorCase>,
}

#[derive(Deserialize)]
struct OperationOracle {
    implementation: String,
    version: String,
}

#[derive(Deserialize)]
struct OperationCase {
    id: String,
    input: String,
    pixels: String,
    encoded: String,
    operation: String,
    parameters: OperationParameters,
    mode: String,
    width: u32,
    height: u32,
    palette_hex: String,
    palette_alpha_hex: String,
}

#[derive(Deserialize)]
struct OperationErrorCase {
    id: String,
    input: String,
    operation: String,
    parameters: OperationParameters,
    kind: String,
    message: String,
}

#[derive(Default, Deserialize)]
struct OperationParameters {
    #[serde(rename = "box")]
    box_coords: Option<[u32; 4]>,
    size: Option<[u32; 2]>,
    method: Option<String>,
    angle: Option<f64>,
    expand: Option<bool>,
    border: Option<u32>,
    offset: Option<[i32; 2]>,
    matrix: Option<Vec<f64>>,
    point: Option<[u32; 2]>,
    value: Option<u8>,
    color: Option<[u8; 4]>,
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/image_backend")
}

fn manifest() -> Manifest {
    let path = fixture_root().join("manifest.json");
    let bytes = fs::read(path).expect("migration fixture manifest must be readable");
    serde_json::from_slice(&bytes).expect("migration fixture manifest must be valid JSON")
}

fn operation_manifest() -> OperationManifest {
    let path = fixture_root().join("operations.json");
    let bytes = fs::read(path).expect("operation fixture manifest must be readable");
    serde_json::from_slice(&bytes).expect("operation fixture manifest must be valid JSON")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn expected_format(name: &str) -> ImageFormat {
    ImageFormat::from_name(name)
        .unwrap_or_else(|error| panic!("unsupported manifest format {name}: {error}"))
}

fn required<T: Copy>(value: Option<T>, row: &OperationCase, name: &str) -> T {
    value.unwrap_or_else(|| panic!("{} missing {name}", row.id))
}

fn apply_operation(source: &Image, row: &OperationCase) -> Result<Image, PilError> {
    let parameters = &row.parameters;
    match row.operation.as_str() {
        "crop" => {
            let [left, top, right, bottom] = required(parameters.box_coords, row, "box");
            source.crop_box(left, top, right, bottom)
        }
        "resize_nearest" => {
            let [width, height] = required(parameters.size, row, "size");
            source.resize((width, height), Some("NEAREST"))
        }
        "thumbnail_nearest" => {
            let [width, height] = required(parameters.size, row, "size");
            let mut result = source.copy();
            result.thumbnail((width, height), Some(ResampleFilter::Nearest))?;
            Ok(result)
        }
        "rotate_27_expand" => source.rotate(
            required(parameters.angle, row, "angle"),
            required(parameters.expand, row, "expand"),
            None,
        ),
        operation if operation.starts_with("transpose_") => source.transpose(
            parameters
                .method
                .as_deref()
                .unwrap_or_else(|| panic!("{} missing method", row.id)),
        ),
        "imageops_flip" => pillow_rs::imageops_flip(source),
        "imageops_mirror" => pillow_rs::imageops_mirror(source),
        "imageops_crop" => {
            pillow_rs::imageops_crop(source, required(parameters.border, row, "border"))
        }
        "imagechops_offset" => {
            let [x, y] = required(parameters.offset, row, "offset");
            pillow_rs::chops_offset(source, x, y)
        }
        "imagechops_duplicate" => Ok(source.copy()),
        "putpixel_index" => {
            let [x, y] = required(parameters.point, row, "point");
            let mut result = source.copy();
            result.putpixel_mode(x, y, required(parameters.value, row, "value"), "P")?;
            Ok(result)
        }
        "putpixel_rgb" => {
            let [x, y] = required(parameters.point, row, "point");
            let [r, g, b, a] = required(parameters.color, row, "color");
            let mut result = source.copy();
            result.putpixel(x, y, r, g, b, a)?;
            Ok(result)
        }
        "crop_then_putpixel_index" => {
            let [left, top, right, bottom] = required(parameters.box_coords, row, "box");
            let [x, y] = required(parameters.point, row, "point");
            let mut result = source.crop_box(left, top, right, bottom)?;
            result.putpixel_mode(x, y, required(parameters.value, row, "value"), "P")?;
            Ok(result)
        }
        "transform_affine_nearest" => {
            let [width, height] = required(parameters.size, row, "size");
            let matrix = parameters
                .matrix
                .as_deref()
                .unwrap_or_else(|| panic!("{} missing matrix", row.id));
            source.transform_affine((width, height), matrix, (0, 0, 0, 0))
        }
        _ => panic!("{} has unsupported operation {}", row.id, row.operation),
    }
}

fn apply_error_operation(source: &Image, row: &OperationErrorCase) -> Result<Image, PilError> {
    match row.operation.as_str() {
        "putpixel_rgba" => {
            let [x, y] = row
                .parameters
                .point
                .unwrap_or_else(|| panic!("{} missing point", row.id));
            let [r, g, b, a] = row
                .parameters
                .color
                .unwrap_or_else(|| panic!("{} missing color", row.id));
            let mut result = source.copy();
            result.putpixel(x, y, r, g, b, a)?;
            Ok(result)
        }
        _ => panic!("{} has unsupported operation {}", row.id, row.operation),
    }
}

#[test]
fn pipeline_backend_lock_survives_every_append_path() {
    let source =
        Image::new(8, 8, "RGB", (12, 34, 56, 255)).expect("backend-lock source must construct");
    let locked = source
        .crop_box(0, 0, 7, 7)
        .expect("first pipeline op")
        .use_backend(Backend::Cpu);
    assert_eq!(locked.backend(), Some(Backend::Cpu));

    let extended = locked
        .transpose("FLIP_LEFT_RIGHT")
        .expect("second pipeline op");
    assert_eq!(extended.backend(), Some(Backend::Cpu));
    let extended_again = extended
        .resize((4, 4), Some("NEAREST"))
        .expect("third pipeline op");
    assert_eq!(extended_again.backend(), Some(Backend::Cpu));

    let unlocked = source
        .crop_box(0, 0, 7, 7)
        .expect("unlocked pipeline")
        .transpose("FLIP_TOP_BOTTOM")
        .expect("extend unlocked pipeline");
    assert_eq!(unlocked.backend(), None);

    let row = manifest()
        .decode
        .into_iter()
        .find(|row| row.mode == "P")
        .expect("manifest must contain a paletted fixture");
    let input = fs::read(fixture_root().join(row.input)).expect("paletted fixture");
    let paletted = Image::open_bytes(input).expect("paletted source must open");
    let palette_safe = paletted
        .crop_box(0, 0, row.width, row.height)
        .expect("palette-safe pipeline")
        .use_backend(Backend::Cpu)
        .transpose("FLIP_LEFT_RIGHT")
        .expect("extend palette-safe pipeline");
    assert_eq!(palette_safe.backend(), Some(Backend::Cpu));
}

#[test]
fn pipeline_verify_never_publishes_success_or_failure() {
    let row = manifest()
        .decode
        .into_iter()
        .find(|row| row.feature == "image-png")
        .expect("manifest must contain a PNG fixture");
    let input = fs::read(fixture_root().join(row.input)).expect("pipeline verification fixture");
    let source = Image::open_bytes(input).expect("pipeline verification source");
    let pipeline = source
        .crop_box(0, 0, row.width, row.height)
        .expect("successful verification pipeline");
    let peer = pipeline.clone();

    assert!(!source.is_materialized());
    assert!(!pipeline.is_materialized());
    pipeline
        .verify()
        .expect("pipeline verification must succeed");
    assert!(!source.is_materialized());
    assert!(!pipeline.is_materialized());
    assert!(!peer.is_materialized());

    let failing = Image::push_op(&source, PipelineOp::CropBorder { border: row.width });
    let failing_peer = failing.clone();
    failing
        .verify()
        .expect_err("invalid pipeline verification must fail");
    assert!(!source.is_materialized());
    assert!(!failing.is_materialized());
    assert!(!failing_peer.is_materialized());
}

#[test]
fn loaded_paletted_reads_share_one_index_view() {
    let row = manifest()
        .decode
        .into_iter()
        .find(|row| row.mode == "P")
        .expect("manifest must contain a paletted fixture");
    let input = fs::read(fixture_root().join(row.input)).expect("paletted fixture");
    let mut image = Image::open_bytes(input).expect("paletted fixture must open");
    image.load().expect("paletted fixture must load");
    let peer = image.clone();

    image.getpixel(0, 0).expect("first paletted read");
    peer.getpixel(0, 0).expect("repeated paletted read");

    let Image::Paletted(image_data) = &image else {
        panic!("loaded indexed fixture must remain paletted");
    };
    let Image::Paletted(peer_data) = &peer else {
        panic!("loaded indexed peer must remain paletted");
    };
    let image_view = image_data
        .materialized
        .get()
        .and_then(|result| result.as_ref().ok())
        .expect("first read must cache an index view");
    let peer_view = peer_data
        .materialized
        .get()
        .and_then(|result| result.as_ref().ok())
        .expect("peer must observe the index view");
    assert!(std::sync::Arc::ptr_eq(image_view, peer_view));
}

#[test]
fn manifest_open_bytes_auto_detects_and_preserves_state_across_load() {
    let manifest = manifest();
    assert_eq!(manifest.oracle.implementation, "Pillow");
    assert_eq!(manifest.oracle.version, "12.2.0");
    assert_eq!(manifest.oracle.source, "pillow-rs-py/pyproject.toml");
    let oracle_source = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("pillow-rs crate must live in the workspace")
            .join("pillow-rs-py/pyproject.toml"),
    )
    .expect("backend oracle source must exist");
    assert!(
        oracle_source.contains("pillow==12.2.0"),
        "backend oracle source must pin Pillow 12.2.0"
    );

    for row in manifest.decode {
        if row.feature == "image-avif" && !cfg!(feature = "image-avif") {
            continue;
        }
        let input = fs::read(fixture_root().join(&row.input))
            .unwrap_or_else(|error| panic!("{} input fixture: {error}", row.id));
        let expected_pixels = fs::read(fixture_root().join(&row.pixels))
            .unwrap_or_else(|error| panic!("{} pixel fixture: {error}", row.id));
        let expected_format = expected_format(&row.format);
        let mut image = Image::open_bytes(input)
            .unwrap_or_else(|error| panic!("{} open failed: {error}", row.id));

        assert!(!image.is_materialized(), "{} opened eagerly", row.id);
        let info_before = image
            .image_info()
            .unwrap_or_else(|| panic!("{} missing cached ImageInfo", row.id));
        assert_eq!(
            info_before.format, expected_format,
            "{} auto-detected format",
            row.id
        );
        assert_eq!(
            image.size().expect("fixture metadata size"),
            (row.width, row.height),
            "{} size",
            row.id
        );
        assert_eq!(
            image.mode().expect("fixture metadata mode"),
            row.mode,
            "{} mode",
            row.id
        );
        assert_eq!(image.format_name().as_deref(), Some(row.format.as_str()));
        assert_eq!(
            (info_before.width, info_before.height),
            (row.width, row.height)
        );
        assert!(
            !image.is_materialized(),
            "{} metadata decoded pixels",
            row.id
        );

        image
            .verify()
            .unwrap_or_else(|error| panic!("{} verify failed: {error}", row.id));
        assert!(!image.is_materialized(), "{} verify changed state", row.id);

        let peer = image.clone();
        let concurrent_pixels = std::thread::scope(|scope| {
            (0..4)
                .map(|_| {
                    let image = image.clone();
                    scope.spawn(move || image.tobytes())
                })
                .collect::<Vec<_>>()
                .into_iter()
                .map(|handle| handle.join().expect("fixture decode thread must not panic"))
                .collect::<Result<Vec<_>, _>>()
        })
        .unwrap_or_else(|error| panic!("{} concurrent load failed: {error}", row.id));
        assert!(
            image.is_materialized(),
            "{} implicit load was not cached",
            row.id
        );
        assert!(
            peer.is_materialized(),
            "{} clone did not share cache",
            row.id
        );
        assert!(
            concurrent_pixels
                .iter()
                .all(|pixels| pixels == &expected_pixels),
            "{} concurrent exact Pillow pixels",
            row.id
        );
        assert_eq!(
            peer.tobytes().expect("shared clone pixels"),
            expected_pixels,
            "{} repeated exact Pillow pixels",
            row.id
        );
        image
            .load()
            .unwrap_or_else(|error| panic!("{} load failed: {error}", row.id));

        assert!(
            image.is_materialized(),
            "{} load was not persistent",
            row.id
        );
        assert_eq!(
            image.size().expect("loaded fixture size"),
            (row.width, row.height),
            "{} loaded size",
            row.id
        );
        assert_eq!(
            image.mode().expect("loaded fixture mode"),
            row.mode,
            "{} loaded mode",
            row.id
        );
        assert_eq!(image.format_name().as_deref(), Some(row.format.as_str()));
        assert_eq!(image.image_info(), Some(info_before));
        assert_eq!(
            image
                .tobytes()
                .unwrap_or_else(|error| panic!("{} pixels failed: {error}", row.id)),
            expected_pixels,
            "{} exact Pillow pixels",
            row.id
        );

        if let Some(expected) = row.palette_hex {
            let palette = image
                .getpalette_trimmed()
                .unwrap_or_else(|| panic!("{} missing palette", row.id));
            assert_eq!(hex(&palette), expected, "{} exact Pillow palette", row.id);
        }
        if let Some(expected) = row.palette_alpha_hex {
            let alpha = image
                .palette_alpha()
                .unwrap_or_else(|| panic!("{} missing palette alpha", row.id));
            assert_eq!(hex(&alpha), expected, "{} exact palette alpha", row.id);
        }
    }
}

#[test]
fn manifest_byte_open_owns_one_stable_snapshot_and_validates_hints() {
    let manifest = manifest();
    let row = manifest
        .decode
        .iter()
        .find(|row| row.feature == "image-png")
        .expect("manifest must contain a PNG success fixture");
    let input = fs::read(fixture_root().join(&row.input)).expect("byte input fixture");
    let expected_pixels = fs::read(fixture_root().join(&row.pixels)).expect("byte pixel fixture");

    let image =
        Image::open_bytes_with_format(input.clone(), Some("png")).expect("open byte fixture");
    assert!(!image.is_materialized());
    image
        .verify()
        .expect("verification must use owned byte snapshot");
    assert_eq!(
        image.tobytes().expect("decode original byte snapshot"),
        expected_pixels
    );

    let hint_error =
        Image::open_bytes_with_format(input, Some("jpeg")).expect_err("hint must be validated");
    assert!(matches!(hint_error, PilError::ValueError(_)));
}

#[test]
fn manifest_error_rows_preserve_structured_failures() {
    for row in manifest().errors {
        assert_eq!(row.feature, "image-png", "{} feature", row.id);
        let input = fs::read(fixture_root().join(&row.input))
            .unwrap_or_else(|error| panic!("{} input fixture: {error}", row.id));
        match (row.stage.as_str(), row.kind.as_str()) {
            ("open", "unidentified") => {
                let error = Image::open_bytes(input).expect_err("fixture must fail during open");
                assert!(
                    matches!(error, PilError::UnidentifiedImageError(_)),
                    "{} returned {error:?}",
                    row.id
                );
            }
            ("verify", "malformed_png") => {
                let image = Image::open_bytes(input)
                    .unwrap_or_else(|error| panic!("{} open failed: {error}", row.id));
                let peer = image.clone();
                let error = image
                    .verify()
                    .expect_err("fixture must fail during verification");
                assert!(
                    matches!(
                        error,
                        PilError::ImageError(ImageError::Malformed {
                            format: ImageFormat::Png,
                            ..
                        })
                    ),
                    "{} returned {error:?}",
                    row.id
                );
                assert!(!image.is_materialized(), "{} verify changed state", row.id);
                // Pillow 12.2.0's PNG `verify()` checks IDAT CRCs, but the
                // ordinary lazy load path can still decode the same image.
                // Keep those paths separate: verification failure must not
                // poison the materialization cache shared by clones.
                let first = image.tobytes().expect("verify failure must not poison load");
                let second = peer.tobytes().expect("clone must still load pixels");
                assert_eq!(first, second, "{} stable decoded pixels", row.id);
                assert!(image.is_materialized(), "{} load did not cache pixels", row.id);
            }
            _ => panic!("{} has an unsupported error expectation", row.id),
        }
    }
}

#[test]
fn pillow_oracle_rows_prove_palette_safe_operations_exactly() {
    let manifest = operation_manifest();
    assert_eq!(manifest.oracle.implementation, "Pillow");
    assert_eq!(manifest.oracle.version, "12.2.0");
    let expected_source_pixels = fs::read(fixture_root().join("outputs/png_indexed_alpha.bin"))
        .expect("indexed source pixel fixture");

    for row in manifest.operations {
        let input = fs::read(fixture_root().join(&row.input))
            .unwrap_or_else(|error| panic!("{} input fixture: {error}", row.id));
        let expected_pixels = fs::read(fixture_root().join(&row.pixels))
            .unwrap_or_else(|error| panic!("{} pixel fixture: {error}", row.id));
        let expected_encoded = fs::read(fixture_root().join(&row.encoded))
            .unwrap_or_else(|error| panic!("{} encoded fixture: {error}", row.id));
        let source = Image::open_bytes(input)
            .unwrap_or_else(|error| panic!("{} open failed: {error}", row.id));
        let mut result = apply_operation(&source, &row)
            .unwrap_or_else(|error| panic!("{} operation failed: {error}", row.id));
        let result_peer = result.clone();

        assert_eq!(
            result.mode().expect("operation mode"),
            row.mode,
            "{} mode",
            row.id
        );
        assert_eq!(
            result.size().expect("operation size"),
            (row.width, row.height),
            "{} size",
            row.id
        );
        assert_eq!(
            hex(&result.getpalette_trimmed().expect("operation palette")),
            row.palette_hex,
            "{} exact Pillow palette",
            row.id
        );
        assert_eq!(
            hex(&result.palette_alpha().expect("operation palette alpha")),
            row.palette_alpha_hex,
            "{} exact Pillow palette alpha",
            row.id
        );
        assert_eq!(
            result.tobytes().expect("operation pixels"),
            expected_pixels,
            "{} exact Pillow indices",
            row.id
        );
        assert!(
            result.is_materialized(),
            "{} pipeline output not cached",
            row.id
        );
        assert!(
            result_peer.is_materialized(),
            "{} pipeline clone did not share output cache",
            row.id
        );
        assert_eq!(
            result_peer.tobytes().expect("shared pipeline pixels"),
            expected_pixels,
            "{} repeated pipeline indices",
            row.id
        );
        assert_eq!(
            source.tobytes().expect("unchanged operation source"),
            expected_source_pixels,
            "{} copy-on-write source isolation",
            row.id
        );
        if row.operation == "putpixel_index" {
            let mut loaded_source = source.copy();
            loaded_source.load().expect("materialize mutation source");
            let loaded_peer = loaded_source.clone();
            let loaded_result = apply_operation(&loaded_source, &row)
                .expect("mutate a clone of materialized storage");
            assert_eq!(
                loaded_result.tobytes().expect("loaded mutation pixels"),
                expected_pixels,
                "{} loaded copy-on-write output",
                row.id
            );
            assert_eq!(
                loaded_source.tobytes().expect("unchanged loaded source"),
                expected_source_pixels,
                "{} loaded source isolation",
                row.id
            );
            assert_eq!(
                loaded_peer.tobytes().expect("unchanged loaded peer"),
                expected_source_pixels,
                "{} loaded peer isolation",
                row.id
            );
        }
        assert_eq!(
            result.to_png_bytes().expect("operation PNG encoding"),
            expected_encoded,
            "{} exact Pillow PNG",
            row.id
        );

        result
            .load()
            .unwrap_or_else(|error| panic!("{} persistent load failed: {error}", row.id));
        assert!(result.is_materialized(), "{} did not persist load", row.id);
        assert_eq!(result.mode().expect("loaded operation mode"), "P");
        assert_eq!(
            result.tobytes().expect("loaded operation pixels"),
            expected_pixels,
            "{} exact loaded indices",
            row.id
        );
        assert_eq!(
            result
                .to_png_bytes()
                .expect("loaded operation PNG encoding"),
            expected_encoded,
            "{} exact loaded Pillow PNG",
            row.id
        );
    }
}

#[test]
fn pillow_oracle_rows_prove_palette_operation_errors_exactly() {
    let manifest = operation_manifest();
    assert_eq!(manifest.oracle.implementation, "Pillow");
    assert_eq!(manifest.oracle.version, "12.2.0");

    for row in manifest.errors {
        let input = fs::read(fixture_root().join(&row.input))
            .unwrap_or_else(|error| panic!("{} input fixture: {error}", row.id));
        let source = Image::open_bytes(input)
            .unwrap_or_else(|error| panic!("{} open failed: {error}", row.id));
        let error = apply_error_operation(&source, &row)
            .err()
            .unwrap_or_else(|| panic!("{} unexpectedly succeeded", row.id));
        match row.kind.as_str() {
            "ValueError" => assert!(
                matches!(&error, PilError::ValueError(message) if message == &row.message),
                "{} returned {error:?}",
                row.id
            ),
            _ => panic!("{} has unsupported error kind {}", row.id, row.kind),
        }
        assert!(!source.is_materialized(), "{} error decoded source", row.id);
    }
}
