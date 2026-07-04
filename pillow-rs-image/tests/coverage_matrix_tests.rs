// AS PER DESIGN — DO NOT REMOVE: Tests may unwrap/expect.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_in_result)]
#![allow(unused_crate_dependencies)]

//! Coverage matrix tests — driven by tests/fixtures/coverage_matrix.json
//! Each row in the matrix is one test assertion.
//! Decode: load asset → decode → compare pixel bytes with PIL reference bytes.
//! Encode: decode reference → encode with params → decode → compare pixel bytes.

// AS PER DESIGN — DO NOT REMOVE:
//   Tests may use unwrap/expect. The deny lints are for production code only.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use pillow_rs_image as img;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CoverageMatrix {
    formats: HashMap<String, FormatData>,
    summary: Summary,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct FormatData {
    decode: Vec<DecodeRow>,
    encode: Vec<EncodeRow>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct DecodeRow {
    id: String,
    #[serde(rename = "type")]
    row_type: String,
    format: String,
    category: String,
    status: String,
    asset: Option<String>,
    asset_path: Option<String>,
    expect_error: Option<bool>,
    ref_mode: Option<String>,
    ref_size: Option<Vec<u32>>,
    ref_path: Option<String>,
    ref_bytes: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct EncodeRow {
    id: String,
    #[serde(rename = "type")]
    row_type: String,
    format: String,
    params: HashMap<String, serde_json::Value>,
    description: Option<String>,
    status: String,
    #[serde(default)]
    source_format: Option<String>,
    #[serde(default)]
    source_asset: Option<String>,
    #[serde(default)]
    ref_bytes: Option<usize>,
    #[serde(default)]
    ref_mode: Option<String>,
    #[serde(default)]
    ref_size: Option<Vec<u32>>,
    #[serde(default)]
    ref_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Summary {
    total_rows: usize,
    decode_rows: usize,
    encode_rows: usize,
    formats: usize,
    assets_available: usize,
    decode_active: usize,
    decode_planned: usize,
    encode_not_wired: usize,
}

#[derive(Debug)]
struct PixelParityRef {
    id: String,
    bytes: Vec<u8>,
    width: Option<u32>,
    height: Option<u32>,
    mode: Option<String>,
}

#[derive(Debug)]
struct PixelMismatch {
    byte_index: usize,
    pixel_index: usize,
    x: u32,
    y: u32,
    channel: usize,
    expected: u8,
    actual: u8,
}

fn expected_raw_name(module: &str, format: &str, asset: &str) -> String {
    format!("{module}.{format}_{}.bin", asset.replace('.', "_"))
}

fn load_pixel_reference(
    manifest_dir: &Path,
    id: &str,
    ref_path: Option<&str>,
    module: &str,
    format: &str,
    asset: &str,
    ref_size: Option<&[u32]>,
    ref_mode: Option<&str>,
) -> Option<PixelParityRef> {
    let raw_path = ref_path.map_or_else(
        || {
            manifest_dir
                .join("tests")
                .join("fixtures")
                .join("outputs")
                .join("raws")
                .join(expected_raw_name(module, format, asset))
        },
        |path| manifest_dir.join(path),
    );

    let bytes = match fs::read(&raw_path) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("  SKIP [{id}]: reference pixels not readable at {raw_path:?}: {err}");
            return None;
        }
    };

    Some(PixelParityRef {
        id: id.to_owned(),
        bytes,
        width: ref_size.and_then(|s| s.first().copied()),
        height: ref_size.and_then(|s| s.get(1).copied()),
        mode: ref_mode.map(str::to_owned),
    })
}

fn mode_bytes_per_pixel(mode: Option<&str>) -> Option<usize> {
    match mode {
        Some("1") | Some("P") | Some("L") | Some("L8") => Some(1),
        Some("I;16") | Some("I;16B") | Some("I;16L") | Some("L16") | Some("La8") => Some(2),
        Some("RGB") | Some("Rgb8") => Some(3),
        Some("RGBA") | Some("Rgba8") | Some("La16") => Some(4),
        Some("Rgb16") => Some(6),
        Some("Rgba16") => Some(8),
        _ => None,
    }
}

fn first_pixel_mismatches(
    expected: &[u8],
    actual: &[u8],
    width: u32,
    bytes_per_pixel: usize,
) -> Vec<PixelMismatch> {
    expected
        .chunks(64)
        .zip(actual.chunks(64))
        .enumerate()
        .flat_map(|(chunk_index, (expected_chunk, actual_chunk))| {
            expected_chunk
                .iter()
                .zip(actual_chunk)
                .enumerate()
                .filter_map(move |(offset, (&expected, &actual))| {
                    if expected == actual {
                        return None;
                    }
                    let byte_index = chunk_index * 64 + offset;
                    let pixel_index = byte_index / bytes_per_pixel;
                    let x = (pixel_index as u32) % width;
                    let y = (pixel_index as u32) / width;
                    Some(PixelMismatch {
                        byte_index,
                        pixel_index,
                        x,
                        y,
                        channel: byte_index % bytes_per_pixel,
                        expected,
                        actual,
                    })
                })
        })
        .take(8)
        .collect()
}

fn count_mismatched_bytes(expected: &[u8], actual: &[u8]) -> usize {
    expected
        .chunks(1024)
        .zip(actual.chunks(1024))
        .map(|(expected_chunk, actual_chunk)| {
            expected_chunk
                .iter()
                .zip(actual_chunk)
                .filter(|(expected, actual)| expected != actual)
                .count()
        })
        .sum()
}

fn assert_pixel_parity(
    expected: &PixelParityRef,
    actual: &img::DecodedImage,
) -> Result<(), String> {
    if let Some(width) = expected.width {
        if actual.width != width {
            return Err(format!(
                "width mismatch: actual {}, expected {}",
                actual.width, width
            ));
        }
    }
    if let Some(height) = expected.height {
        if actual.height != height {
            return Err(format!(
                "height mismatch: actual {}, expected {}",
                actual.height, height
            ));
        }
    }

    let actual_bytes = actual.as_bytes();
    if actual_bytes.len() != expected.bytes.len() {
        return Err(format!(
            "byte length mismatch: actual {}, expected {}",
            actual_bytes.len(),
            expected.bytes.len()
        ));
    }

    if actual_bytes == expected.bytes.as_slice() {
        return Ok(());
    }

    let bytes_per_pixel = mode_bytes_per_pixel(expected.mode.as_deref())
        .unwrap_or_else(|| usize::from(actual.color.bytes_per_pixel()));
    let width = expected.width.unwrap_or(actual.width).max(1);
    let mismatch_count = count_mismatched_bytes(&expected.bytes, actual_bytes);
    let examples = first_pixel_mismatches(&expected.bytes, actual_bytes, width, bytes_per_pixel)
        .into_iter()
        .map(|m| {
            format!(
                "byte {} pixel {} ({}, {}) channel {} expected {:02x} actual {:02x}",
                m.byte_index, m.pixel_index, m.x, m.y, m.channel, m.expected, m.actual
            )
        })
        .collect::<Vec<_>>()
        .join("; ");

    Err(format!(
        "{} mismatched byte(s) out of {} for mode {}; first: {}",
        mismatch_count,
        actual_bytes.len(),
        expected.mode.as_deref().unwrap_or("?"),
        examples
    ))
}

// ── Decode Tests ─────────────────────────────────────────────────────────

#[test]
fn test_decode_matrix() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let matrix_path = manifest_dir
        .join("tests")
        .join("fixtures")
        .join("coverage_matrix.json");

    let matrix: CoverageMatrix = if matrix_path.exists() {
        serde_json::from_str(&fs::read_to_string(&matrix_path).unwrap()).unwrap()
    } else {
        eprintln!(
            "SKIP: coverage_matrix.json not found. Run: python scripts/generate_decode_refs.py"
        );
        return;
    };

    let assets_dir = manifest_dir
        .join("tests")
        .join("fixtures")
        .join("input")
        .join("images");
    let mut total = 0u32;
    let mut passed = 0u32;
    let mut failed = 0u32;
    let mut skipped = 0u32;

    for (fmt_name, fmt_data) in &matrix.formats {
        for row in &fmt_data.decode {
            if row.status == "planned" {
                skipped += 1;
                continue;
            }
            if row.expect_error.unwrap_or(false) {
                skipped += 1;
                continue;
            }

            let asset_name = match &row.asset {
                Some(a) => a,
                None => {
                    skipped += 1;
                    continue;
                }
            };
            let asset_path = assets_dir.join(fmt_name).join(asset_name);
            if !asset_path.exists() {
                skipped += 1;
                continue;
            }

            total += 1;
            let data = match fs::read(&asset_path) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("  FAIL [{}]: read error {}", row.id, e);
                    failed += 1;
                    continue;
                }
            };

            let decoded = match img::decode(&data) {
                Some(d) => d,
                None => {
                    eprintln!("  FAIL [{}]: decode returned None", row.id);
                    failed += 1;
                    continue;
                }
            };

            let Some(expected) = load_pixel_reference(
                manifest_dir,
                &row.id,
                row.ref_path.as_deref(),
                "Decode",
                fmt_name,
                asset_name,
                row.ref_size.as_deref(),
                row.ref_mode.as_deref(),
            ) else {
                skipped += 1;
                continue;
            };

            match assert_pixel_parity(&expected, &decoded) {
                Ok(()) => {
                    eprintln!(
                        "  OK   [{}] {} bytes pixel-parity (mode={})",
                        expected.id,
                        decoded.as_bytes().len(),
                        row.ref_mode.as_deref().unwrap_or("?")
                    );
                    passed += 1;
                }
                Err(message) => {
                    eprintln!("  FAIL [{}]: {message}", expected.id);
                    failed += 1;
                }
            }
        }
    }

    eprintln!("\ndecode matrix: {passed}/{total} passed, {failed} failed, {skipped} skipped");
    if failed > 0 {
        panic!("{failed} decode test(s) failed");
    }
}

// ── Encode Tests ─────────────────────────────────────────────────────────

#[test]
fn test_encode_matrix() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let matrix_path = manifest_dir
        .join("tests")
        .join("fixtures")
        .join("coverage_matrix.json");

    let matrix: CoverageMatrix = if matrix_path.exists() {
        serde_json::from_str(&fs::read_to_string(&matrix_path).unwrap()).unwrap()
    } else {
        eprintln!("SKIP: no matrix");
        return;
    };

    let mut total = 0u32;
    let mut passed = 0u32;
    let mut failed = 0u32;
    let mut skipped = 0u32;
    let assets_dir = manifest_dir
        .join("tests")
        .join("fixtures")
        .join("input")
        .join("images");

    for (fmt_name, fmt_data) in &matrix.formats {
        if fmt_data.encode.is_empty() {
            continue;
        }

        for row in &fmt_data.encode {
            if row.status == "planned" {
                skipped += 1;
                continue;
            }

            total += 1;

            // Determine source: use row's source_asset if present, otherwise fall back
            // to the first active decode row for this format.
            let asset_data = if let (Some(ref src_fmt), Some(ref src_asset)) =
                (&row.source_format, &row.source_asset)
            {
                let path = assets_dir.join(src_fmt).join(src_asset);
                if path.exists() {
                    fs::read(&path).unwrap()
                } else {
                    eprintln!("  FAIL [{}]: source asset not found: {:?}", row.id, path);
                    failed += 1;
                    continue;
                }
            } else {
                // Fallback: find a decode row in this format
                let source_row = fmt_data
                    .decode
                    .iter()
                    .find(|r| r.status == "active" && r.asset.is_some());
                match source_row {
                    Some(src) => {
                        let path = assets_dir.join(fmt_name).join(src.asset.as_ref().unwrap());
                        if path.exists() {
                            fs::read(&path).unwrap()
                        } else {
                            skipped += 1;
                            continue;
                        }
                    }
                    None => {
                        skipped += 1;
                        continue;
                    }
                }
            };

            let decoded = match img::decode(&asset_data) {
                Some(d) => d,
                None => {
                    eprintln!("  FAIL [{}]: source decode failed", row.id);
                    failed += 1;
                    continue;
                }
            };

            // Build encode options from row params
            let opts = img::encode_options::EncodeOptions {
                quality: row
                    .params
                    .get("quality")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u8),
                compression: row
                    .params
                    .get("compression")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u8),
                lossless: row.params.get("lossless").and_then(|v| v.as_bool()),
                progressive: row.params.get("progressive").and_then(|v| v.as_bool()),
                interlace: row
                    .params
                    .get("interlace")
                    .or_else(|| row.params.get("interlaced"))
                    .and_then(|v| v.as_bool()),
                ..Default::default()
            };

            let format = match fmt_name.as_str() {
                "jpeg" => img::ImageFormat::Jpeg,
                "png" => img::ImageFormat::Png,
                "gif" => img::ImageFormat::Gif,
                "bmp" => img::ImageFormat::Bmp,
                "tiff" => img::ImageFormat::Tiff,
                "webp" => img::ImageFormat::WebP,
                "ico" => img::ImageFormat::Ico,
                _ => {
                    skipped += 1;
                    continue;
                }
            };

            let encoded = match img::encode(&decoded, format, &opts) {
                Some(e) => e,
                None => {
                    eprintln!("  FAIL [{}]: encode returned None", row.id);
                    failed += 1;
                    continue;
                }
            };

            // Roundtrip: re-decode and compare pixels against the PIL reference.
            match img::decode(&encoded) {
                Some(redecoded) => {
                    if let Some(expected) = row.ref_path.as_deref().and_then(|ref_path| {
                        load_pixel_reference(
                            manifest_dir,
                            &row.id,
                            Some(ref_path),
                            "Encode",
                            fmt_name,
                            row.source_asset.as_deref().unwrap_or(""),
                            row.ref_size.as_deref(),
                            row.ref_mode.as_deref(),
                        )
                    }) {
                        match assert_pixel_parity(&expected, &redecoded) {
                            Ok(()) => {
                                eprintln!(
                                    "  OK   [{}] {}B, re-decoded {}x{} pixel-parity (mode={})",
                                    row.id,
                                    encoded.len(),
                                    redecoded.width,
                                    redecoded.height,
                                    row.ref_mode.as_deref().unwrap_or("?")
                                );
                                passed += 1;
                            }
                            Err(message) => {
                                eprintln!("  FAIL [{}]: {message}", row.id);
                                failed += 1;
                            }
                        }
                    } else {
                        // No ref — fallback: just check non-zero dimensions
                        if redecoded.width > 0 && redecoded.height > 0 {
                            eprintln!("  OK   [{}] encoded {}B (no ref)", row.id, encoded.len());
                            passed += 1;
                        } else {
                            eprintln!("  FAIL [{}]: zero dimensions after roundtrip", row.id);
                            failed += 1;
                        }
                    }
                }
                None => {
                    eprintln!("  FAIL [{}]: re-decode failed", row.id);
                    failed += 1;
                }
            }
        }
    }

    eprintln!("\nencode matrix: {passed}/{total} passed, {failed} failed, {skipped} skipped");
    if failed > 0 {
        panic!("{failed} encode test(s) failed");
    }
}

// ── Manifest Coverage ────────────────────────────────────────────────────

#[test]
fn test_coverage_matrix() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let matrix_path = manifest_dir
        .join("tests")
        .join("fixtures")
        .join("coverage_matrix.json");

    let matrix: CoverageMatrix = if matrix_path.exists() {
        serde_json::from_str(&fs::read_to_string(&matrix_path).unwrap()).unwrap()
    } else {
        eprintln!("SKIP: no matrix");
        return;
    };

    let s = &matrix.summary;
    eprintln!(
        "Coverage: {}/{} decode active, {} planned, {} encode not wired, {} assets",
        s.decode_active, s.decode_rows, s.decode_planned, s.encode_not_wired, s.assets_available
    );

    assert!(s.total_rows > 0, "Matrix must have rows");
    assert_eq!(s.total_rows, s.decode_rows + s.encode_rows);
}
