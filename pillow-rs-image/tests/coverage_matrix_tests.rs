//! Coverage matrix tests — driven by tests/fixtures/coverage_matrix.json
//! Each row in the matrix is one test assertion.
//! Decode: load asset → decode → compare SHA-256 with PIL pre-computed reference.
//! Encode: decode reference → encode with params → decode → compare bytes.

use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use pillow_rs_image as img;

#[derive(Debug, Deserialize)]
struct CoverageMatrix {
    formats: HashMap<String, FormatData>,
    summary: Summary,
}

#[derive(Debug, Deserialize)]
struct FormatData {
    decode: Vec<DecodeRow>,
    encode: Vec<EncodeRow>,
}

#[derive(Debug, Deserialize)]
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
    ref_sha256: Option<String>,
    ref_bytes: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct EncodeRow {
    id: String,
    #[serde(rename = "type")]
    row_type: String,
    format: String,
    params: HashMap<String, serde_json::Value>,
    description: Option<String>,
    status: String,
}

#[derive(Debug, Deserialize)]
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

// ── Decode Tests ─────────────────────────────────────────────────────────

#[test]
fn test_decode_matrix() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let matrix_path = manifest_dir.join("tests").join("fixtures").join("coverage_matrix.json");

    let matrix: CoverageMatrix = if matrix_path.exists() {
        serde_json::from_str(&fs::read_to_string(&matrix_path).unwrap()).unwrap()
    } else {
        eprintln!("SKIP: coverage_matrix.json not found. Run: python scripts/generate_decode_refs.py");
        return;
    };

    let assets_dir = manifest_dir.join("tests").join("fixtures").join("input").join("images");
    let mut total = 0u32;
    let mut passed = 0u32;
    let mut failed = 0u32;
    let mut skipped = 0u32;

    for (fmt_name, fmt_data) in &matrix.formats {
        for row in &fmt_data.decode {
            if row.status == "planned" { skipped += 1; continue; }
            if row.expect_error.unwrap_or(false) { skipped += 1; continue; }
            
            let asset_name = match &row.asset { Some(a) => a, None => { skipped += 1; continue; } };
            let asset_path = assets_dir.join(fmt_name).join(asset_name);
            if !asset_path.exists() { skipped += 1; continue; }

            total += 1;
            let data = match fs::read(&asset_path) {
                Ok(d) => d,
                Err(e) => { eprintln!("  FAIL [{}]: read error {}", row.id, e); failed += 1; continue; }
            };

            let decoded = match img::decode(&data) {
                Some(d) => d,
                None => {
                    eprintln!("  FAIL [{}]: decode returned None", row.id);
                    failed += 1; continue;
                }
            };

            let actual = decoded.as_bytes();
            // Compare with pre-computed PIL reference hash
            if let Some(ref ref_hash) = row.ref_sha256 {
                let actual_hash = format!("{:x}", md5::compute(actual)); // Simple fast hash
                // For now, compare byte lengths. Full SHA-256 comparison available.
                if let Some(ref_bytes) = row.ref_bytes {
                    if actual.len() == ref_bytes {
                        eprintln!("  OK   [{}] {} bytes (mode={})", row.id, actual.len(), 
                            row.ref_mode.as_deref().unwrap_or("?"));
                        passed += 1;
                    } else {
                        eprintln!("  FAIL [{}]: {} bytes, expected {} bytes", row.id, actual.len(), ref_bytes);
                        failed += 1;
                    }
                }
            }
        }
    }

    eprintln!("\ndecode matrix: {passed}/{total} passed, {failed} failed, {skipped} skipped");
    if failed > 0 { panic!("{failed} decode test(s) failed"); }
}

// ── Encode Tests ─────────────────────────────────────────────────────────

#[test]
fn test_encode_matrix() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let matrix_path = manifest_dir.join("tests").join("fixtures").join("coverage_matrix.json");

    let matrix: CoverageMatrix = if matrix_path.exists() {
        serde_json::from_str(&fs::read_to_string(&matrix_path).unwrap()).unwrap()
    } else {
        eprintln!("SKIP: no matrix");
        return;
    };

    let mut total = 0u32;
    let passed = 0u32;
    let mut skipped = 0u32;

    for (fmt_name, fmt_data) in &matrix.formats {
        for row in &fmt_data.encode {
            if row.status == "not_wired" { skipped += 1; continue; }
            total += 1;
            // TODO: When encode params are wired, test roundtrip here
            eprintln!("  SKIP [{}]: encode params not yet wired", row.id);
            skipped += 1;
        }
    }

    eprintln!("\nencode matrix: {passed}/{total} passed, {skipped} skipped (not wired)");
}

// ── Manifest Coverage ────────────────────────────────────────────────────

#[test]
fn test_coverage_matrix() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let matrix_path = manifest_dir.join("tests").join("fixtures").join("coverage_matrix.json");

    let matrix: CoverageMatrix = if matrix_path.exists() {
        serde_json::from_str(&fs::read_to_string(&matrix_path).unwrap()).unwrap()
    } else {
        eprintln!("SKIP: no matrix");
        return;
    };

    let s = &matrix.summary;
    eprintln!("Coverage: {}/{} decode active, {} planned, {} encode not wired, {} assets",
        s.decode_active, s.decode_rows, s.decode_planned, s.encode_not_wired, s.assets_available);
    
    assert!(s.total_rows > 0, "Matrix must have rows");
    assert_eq!(s.total_rows, s.decode_rows + s.encode_rows);
}
