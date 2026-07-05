//! PIL 12.2.0 `_imagingft.c` connector fixture tests.
//!
//! Every fixture row is an exact parity gate. Historical `incomplete` rows stay
//! in the fixture so older debt is visible, but the expected failure count is
//! now zero.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_in_result)]
#![allow(clippy::arithmetic_side_effects)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(unused_crate_dependencies)]

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use pillow_rs::font::{Font, imagingft};
use pillow_rs::{Draw, Image};

#[derive(Debug, Deserialize)]
struct Matrix {
    fixture_family: String,
    generator: String,
    pillow_version: String,
    oracle: String,
    pixel_matrix_min_passed: usize,
    rows: Vec<Row>,
}

#[derive(Debug, Deserialize)]
struct Row {
    id: String,
    operation: String,
    font: String,
    size: f32,
    text: String,
    status: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    expected: serde_json::Value,
    #[serde(default)]
    expected_size: Vec<u32>,
    #[serde(default)]
    expected_offset: Vec<i32>,
    #[serde(default)]
    expected_sha256: String,
    #[serde(default)]
    expected_raw: String,
}

#[derive(Debug)]
struct PixelFailure {
    id: String,
    reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct FontKey {
    path: String,
    size_bits: u32,
}

impl FontKey {
    fn from_row(row: &Row) -> Self {
        Self {
            path: row.font.clone(),
            size_bits: row.size.to_bits(),
        }
    }
}

#[derive(Default)]
struct FontCache {
    fonts: HashMap<FontKey, Font>,
}

impl FontCache {
    fn font_for(&mut self, row: &Row) -> &Font {
        let key = FontKey::from_row(row);
        if !self.fonts.contains_key(&key) {
            let font = load_font(row);
            self.fonts.insert(key.clone(), font);
        }
        self.fonts
            .get(&key)
            .expect("font was inserted or already present")
    }
}

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixture_dir() -> PathBuf {
    manifest_dir().join("tests").join("fixtures")
}

fn read_matrix() -> Matrix {
    let path = fixture_dir().join("imagingft_matrix.json");
    let data = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read fixture matrix {path:?}: {err}"));
    serde_json::from_str(&data).expect("imagingft fixture matrix is valid JSON")
}

fn load_font(row: &Row) -> Font {
    let path = manifest_dir().join(&row.font);
    let data =
        fs::read(&path).unwrap_or_else(|err| panic!("failed to read font for {}: {err}", row.id));
    Font::from_bytes(data, row.size)
        .unwrap_or_else(|err| panic!("failed to load font for {}: {err}", row.id))
}

fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn read_expected_raw(row: &Row) -> Vec<u8> {
    let path = fixture_dir().join(&row.expected_raw);
    fs::read(&path).unwrap_or_else(|err| panic!("failed to read raw oracle for {}: {err}", row.id))
}

fn assert_matrix_provenance(matrix: &Matrix) {
    assert_eq!(matrix.fixture_family, "pillow-rs-imagingft");
    assert_eq!(
        matrix.generator,
        "pillow-rs/scripts/build_imagingft_fixtures.py"
    );
    assert_eq!(matrix.pillow_version, "12.2.0");
    assert!(matrix.pixel_matrix_min_passed >= 7000);
    assert!(
        matrix.oracle.contains("PIL.ImageFont.FreeTypeFont"),
        "oracle description must name the PIL connector source"
    );
    assert!(!matrix.rows.is_empty(), "matrix must contain fixture rows");
}

fn assert_raw_file_matches_hash(row: &Row) {
    if row.expected_raw.is_empty() {
        return;
    }
    let raw = read_expected_raw(row);
    assert_eq!(
        sha256_hex(&raw),
        row.expected_sha256,
        "{} raw oracle hash mismatch",
        row.id
    );
}

fn compare_scalar(font: &Font, row: &Row) {
    if let Err(failure) = compare_scalar_result(font, row) {
        panic!("{}: {}", failure.id, failure.reason);
    }
}

fn compare_scalar_result(font: &Font, row: &Row) -> Result<(), PixelFailure> {
    match row.operation.as_str() {
        "getbbox" => {
            let actual = imagingft::getbbox(font, &row.text);
            let expected: Vec<i32> =
                serde_json::from_value(row.expected.clone()).expect("getbbox expected tuple");
            let expected = (expected[0], expected[1], expected[2], expected[3]);
            if actual != expected {
                return Err(PixelFailure {
                    id: row.id.clone(),
                    reason: format!("getbbox actual={actual:?} expected={expected:?}"),
                });
            }
        }
        "getlength" => {
            let actual = imagingft::getlength(font, &row.text);
            let expected: f32 =
                serde_json::from_value(row.expected.clone()).expect("getlength expected float");
            if actual != expected {
                return Err(PixelFailure {
                    id: row.id.clone(),
                    reason: format!("getlength actual={actual:?} expected={expected:?}"),
                });
            }
        }
        "getmetrics" => {
            let actual = imagingft::getmetrics(font);
            let expected: Vec<u32> =
                serde_json::from_value(row.expected.clone()).expect("getmetrics expected tuple");
            let expected = (expected[0], expected[1]);
            if actual != expected {
                return Err(PixelFailure {
                    id: row.id.clone(),
                    reason: format!("getmetrics actual={actual:?} expected={expected:?}"),
                });
            }
        }
        "getname" => {
            let actual = imagingft::getname(font);
            let expected: Vec<String> =
                serde_json::from_value(row.expected.clone()).expect("getname expected tuple");
            let expected = (expected[0].as_str(), expected[1].as_str());
            if actual != expected {
                return Err(PixelFailure {
                    id: row.id.clone(),
                    reason: format!("getname actual={actual:?} expected={expected:?}"),
                });
            }
        }
        other => panic!("{} is not a scalar operation: {other}", row.id),
    }
    Ok(())
}

fn compare_pixel(font: &Font, row: &Row) -> Result<(), PixelFailure> {
    let expected_raw = read_expected_raw(row);
    let (actual_size, actual_offset, actual_raw) = match row.operation.as_str() {
        "getmask" => {
            let (width, height, pixels) = imagingft::getmask(font, &row.text);
            (vec![width, height], Vec::new(), pixels)
        }
        "getmask2" => {
            let (width, height, pixels) = imagingft::getmask(font, &row.text);
            let bbox = imagingft::getbbox(font, &row.text);
            (vec![width, height], vec![bbox.0, bbox.1], pixels)
        }
        "draw_text" => {
            let image = Image::new(96, 64, row.mode.as_str(), (0, 0, 0, 0)).map_err(|err| {
                PixelFailure {
                    id: row.id.clone(),
                    reason: format!("image allocation failed: {err}"),
                }
            })?;
            let mut draw = Draw::new(image, Some(row.mode.clone()));
            draw.text(10, 18, &row.text, font, (20, 40, 200, 255))
                .map_err(|err| PixelFailure {
                    id: row.id.clone(),
                    reason: format!("draw failed: {err}"),
                })?;
            let pixels = draw.image_clone().tobytes().map_err(|err| PixelFailure {
                id: row.id.clone(),
                reason: format!("tobytes failed: {err}"),
            })?;
            (row.expected_size.clone(), Vec::new(), pixels)
        }
        other => {
            return Err(PixelFailure {
                id: row.id.clone(),
                reason: format!("unsupported pixel operation {other}"),
            });
        }
    };

    if actual_size != row.expected_size {
        return Err(PixelFailure {
            id: row.id.clone(),
            reason: format!(
                "size actual={actual_size:?} expected={:?}",
                row.expected_size
            ),
        });
    }
    if actual_offset != row.expected_offset {
        return Err(PixelFailure {
            id: row.id.clone(),
            reason: format!(
                "offset actual={actual_offset:?} expected={:?}",
                row.expected_offset
            ),
        });
    }
    if actual_raw != expected_raw {
        return Err(PixelFailure {
            id: row.id.clone(),
            reason: format!(
                "pixels actual_sha256={} expected_sha256={} actual_len={} expected_len={}",
                sha256_hex(&actual_raw),
                row.expected_sha256,
                actual_raw.len(),
                expected_raw.len()
            ),
        });
    }

    Ok(())
}

fn compare_row(font: &Font, row: &Row) -> Result<(), PixelFailure> {
    match row.operation.as_str() {
        "getbbox" | "getlength" | "getmetrics" | "getname" => compare_scalar_result(font, row),
        _ => compare_pixel(font, row),
    }
}

fn assert_required_rows_exist(matrix: &Matrix) {
    const REQUIRED_IDS: &[&str] = &[
        "dejavusans20_hello_getbbox",
        "dejavusans20_hello_getlength",
        "dejavusans20_hello_getmask_l",
        "dejavusans20_hello_getmask2_l",
        "dejavusans20_hello_draw_text_rgba",
        "dejavusans20_av_getbbox",
        "dejavusans20_av_getlength",
        "dejavusans20_av_getmask_l",
        "dejavusans20_av_getmask2_l",
        "dejavusans20_av_draw_text_rgba",
        "dejavusans20_jq_getbbox",
        "dejavusans20_jq_getlength",
        "dejavusans20_jq_getmask_l",
        "dejavusans20_jq_getmask2_l",
        "dejavusans20_jq_draw_text_rgba",
    ];
    for id in REQUIRED_IDS {
        assert!(
            matrix.rows.iter().any(|row| row.id == *id),
            "imagingft regression fixture row missing: {id}"
        );
    }
}

#[test]
fn imagingft_fixture_provenance_is_reproducible() {
    let matrix = read_matrix();
    assert_matrix_provenance(&matrix);
    assert_required_rows_exist(&matrix);
    for row in &matrix.rows {
        assert_raw_file_matches_hash(row);
    }
}

#[test]
fn imagingft_scalar_rows_match_pil_12_2_0() {
    let matrix = read_matrix();
    let mut fonts = FontCache::default();
    for row in matrix.rows.iter().filter(|row| {
        matches!(
            row.operation.as_str(),
            "getbbox" | "getlength" | "getmetrics" | "getname"
        )
    }) {
        let font = fonts.font_for(row);
        compare_scalar(font, row);
    }
}

#[test]
fn imagingft_historical_incomplete_rows_have_zero_failures() {
    let matrix = read_matrix();
    let rows: Vec<&Row> = matrix
        .rows
        .iter()
        .filter(|row| row.status == "incomplete")
        .collect();
    assert!(!rows.is_empty(), "incomplete parity rows must stay visible");

    let mut fonts = FontCache::default();
    let failures: Vec<PixelFailure> = rows
        .iter()
        .filter_map(|row| {
            let font = fonts.font_for(row);
            compare_row(font, row).err()
        })
        .collect();

    let expected_failures = 0;
    assert_eq!(
        failures.len(),
        expected_failures,
        "PIL 12.2.0 imagingft pixel incomplete baseline changed: {failures:#?}"
    );
    for failure in failures {
        eprintln!("imagingft incomplete: {}: {}", failure.id, failure.reason);
    }
}

#[test]
fn imagingft_large_pixel_matrix_has_all_pil_12_2_0_matches() {
    let matrix = read_matrix();
    let rows: Vec<&Row> = matrix
        .rows
        .iter()
        .filter(|row| row.status == "pixel_matrix")
        .collect();
    assert!(
        rows.len() >= matrix.pixel_matrix_min_passed,
        "pixel matrix has too few rows: {}",
        rows.len()
    );

    let mut fonts = FontCache::default();
    let mut passed = 0usize;
    let mut failures = Vec::new();
    for row in &rows {
        let font = fonts.font_for(row);
        match compare_pixel(font, row) {
            Ok(()) => passed += 1,
            Err(failure) => failures.push(failure),
        }
    }

    assert!(
        failures.is_empty(),
        "PIL 12.2.0 imagingft pixel matrix has failures: passed={passed} total={} failures={failures:#?}",
        passed + failures.len()
    );
    assert_eq!(passed, rows.len(), "every pixel matrix row must pass");
    eprintln!(
        "imagingft large pixel matrix: {passed} passed, {} failed",
        failures.len()
    );
}

#[test]
fn imagingft_all_fixture_rows_match_pil_12_2_0() {
    let matrix = read_matrix();
    let mut fonts = FontCache::default();
    let mut failures = Vec::new();
    for row in &matrix.rows {
        let font = fonts.font_for(row);
        if let Err(failure) = compare_row(font, row) {
            failures.push(failure);
        }
    }

    assert!(
        failures.is_empty(),
        "all imagingft fixture rows must match PIL 12.2.0 exactly: {failures:#?}"
    );
}
