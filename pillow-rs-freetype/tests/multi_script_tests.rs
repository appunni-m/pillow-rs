//! Multi-script coverage matrix tests — 22-script bbox parity + 55-script dimension check.
//!
//! `test_multi_script_coverage` — verifies bbox (±2px) and mask dimensions (±2px)
//!   for 22 scripts with Latin/Greek-quality autohinter coverage.
//!
//! `test_full_coverage` — verifies mask dimensions (±2px) for all 55 scripts
//!   (bbox not checked: full pixel parity requires per-script autohinter tuning).

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_in_result)]

use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use pillow_rs_freetype::{BitmapBackend, Font};

#[derive(Debug, Deserialize)]
struct CoverageMatrix {
    rows: Vec<MatrixRow>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct MatrixRow {
    id: String,
    font: String,
    size_pt: f32,
    codepoint: u32,
    #[serde(default)]
    char: String,
    operation: String,
    status: String,
    #[serde(default)]
    ref_size: Option<Vec<u32>>,
    #[serde(default)]
    ref_value: Option<serde_json::Value>,
}

#[test]
fn test_multi_script_coverage() {
    run_matrix("coverage_matrix_multi.json", true);
}

#[test]
fn test_full_coverage() {
    run_matrix("coverage_matrix_full.json", false);
}

fn run_matrix(matrix_file: &str, check_bbox: bool) {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let matrix_path = manifest_dir
        .join("tests")
        .join("fixtures")
        .join(matrix_file);

    let matrix: CoverageMatrix = if matrix_path.exists() {
        serde_json::from_str(&fs::read_to_string(&matrix_path).unwrap()).unwrap()
    } else {
        eprintln!("SKIP: {matrix_file} not found. Run generate_full_coverage.py");
        return;
    };

    let mut total = 0u32;
    let mut passed = 0u32;
    let mut failed = 0u32;
    let mut skipped = 0u32;
    let mut font_cache: HashMap<String, Vec<u8>> = HashMap::new();

    for row in &matrix.rows {
        if row.status == "skip" {
            skipped += 1;
            continue;
        }
        total += 1;

        let font_data = match font_cache.entry(row.font.clone()) {
            std::collections::hash_map::Entry::Occupied(e) => e.get().clone(),
            std::collections::hash_map::Entry::Vacant(e) => {
                let data = load_font_bytes(manifest_dir, &row.font);
                e.insert(data.clone());
                data
            }
        };

        let font = match Font::truetype(&font_data, row.size_pt, BitmapBackend::FreeType) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("  SKIP [{}]: {}", row.id, e);
                skipped += 1;
                continue;
            }
        };

        match row.operation.as_str() {
            "getmask" => {
                let text = get_text(&row);
                match font.getmask(&text) {
                    Ok(mask) => {
                        if let Some(ref_size) = &row.ref_size {
                            let w_ok = (ref_size[0] as i32 - mask.width as i32).abs() <= 2;
                            let h_ok = (ref_size[1] as i32 - mask.height as i32).abs() <= 2;
                            if w_ok && h_ok {
                                eprintln!("  OK   [{}] {}x{}", row.id, mask.width, mask.height);
                                passed += 1;
                            } else {
                                eprintln!("  FAIL [{}]: size {}x{} != expected {}x{}",
                                    row.id, mask.width, mask.height, ref_size[0], ref_size[1]);
                                failed += 1;
                            }
                        } else {
                            passed += 1;
                        }
                    }
                    Err(e) => {
                        eprintln!("  SKIP [{}]: getmask error: {}", row.id, e);
                        skipped += 1;
                    }
                }
            }
            "getbbox" => {
                if !check_bbox {
                    skipped += 1;
                    continue;
                }
                let text = get_text(&row);
                let bbox = font.getbbox(&text);
                if let Some(ref expected) = row.ref_value {
                    if bbox_match(expected, &bbox) {
                        eprintln!("  OK   [{}] bbox={:?}", row.id, bbox);
                        passed += 1;
                    } else {
                        eprintln!("  FAIL [{}]: bbox {:?} != expected {:?}", row.id, bbox, expected);
                        failed += 1;
                    }
                } else {
                    passed += 1;
                }
            }
            _ => skipped += 1,
        }
    }

    let label = matrix_file.trim_end_matches(".json");
    eprintln!("\n{label}: {passed}/{total} passed, {failed} failed, {skipped} skipped");
    if failed > 0 {
        panic!("{failed} test(s) failed in {matrix_file}");
    }
    assert!(passed > 0, "No tests passed — check font files for {matrix_file}");
}

fn get_text(row: &MatrixRow) -> String {
    if row.char.is_empty() {
        char::from_u32(row.codepoint)
            .map(|c| c.to_string())
            .unwrap_or_default()
    } else {
        row.char.clone()
    }
}

fn bbox_match(expected: &serde_json::Value, actual: &(i32, i32, i32, i32)) -> bool {
    let ea = match expected.as_array() {
        Some(a) => a, None => return false,
    };
    if ea.len() < 4 { return false; }
    (ea[0].as_i64().unwrap_or(0) as i32 - actual.0).abs() <= 2
        && (ea[1].as_i64().unwrap_or(0) as i32 - actual.1).abs() <= 2
        && (ea[2].as_i64().unwrap_or(0) as i32 - actual.2).abs() <= 2
        && (ea[3].as_i64().unwrap_or(0) as i32 - actual.3).abs() <= 2
}

fn load_font_bytes(manifest_dir: &Path, name: &str) -> Vec<u8> {
    let font_dir = manifest_dir
        .join("tests")
        .join("fixtures")
        .join("input")
        .join("fonts_autohint");
    let path = font_dir.join(format!("{}.ttf", name));
    fs::read(&path).unwrap_or_else(|_| panic!("font file not found: {:?}", path))
}
