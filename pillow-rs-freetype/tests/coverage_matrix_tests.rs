//! Coverage matrix tests — one per backend.
//!
//! `test_font_coverage_matrix_pil` — compares against PIL 12.2.0 references
//!   (`coverage_matrix.json`).  Uses `BitmapBackend::PIL` — padded mask,
//!   ascender-relative bbox.
//!
//! `test_font_coverage_matrix_freetype` — compares against raw FreeType
//!   2.14.3 references (`coverage_matrix_ft.json`).  Uses
//!   `BitmapBackend::FreeType` — raw bitmap, FreeType bbox coords.

// Tests may unwrap/expect.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_in_result)]

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use pillow_rs_freetype::{BitmapBackend, Font};

#[derive(Debug, Deserialize)]
struct CoverageMatrix {
    rows: Vec<MatrixRow>,
    #[allow(dead_code)]
    summary: Option<Summary>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Summary {
    total_rows: usize,
    active_rows: usize,
    fonts: usize,
    sizes: usize,
    glyphs: usize,
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
    ref_sha256: Option<String>,
    #[serde(default)]
    ref_value: Option<serde_json::Value>,
    #[serde(default)]
    #[allow(dead_code)]
    ref_size: Option<Vec<u32>>,
}

fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
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

// ── Test entry points ─────────────────────────────────────────────────────

#[test]
fn test_font_coverage_matrix_pil() {
    run_matrix(BitmapBackend::PIL, "coverage_matrix.json");
}

#[test]
fn test_font_coverage_matrix_freetype() {
    run_matrix(BitmapBackend::FreeType, "coverage_matrix_ft.json");
}

// ── Shared runner ─────────────────────────────────────────────────────────

fn run_matrix(backend: BitmapBackend, matrix_file: &str) {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let matrix_path = manifest_dir
        .join("tests")
        .join("fixtures")
        .join(matrix_file);

    let label = match backend {
        BitmapBackend::PIL => "PIL",
        BitmapBackend::FreeType => "FreeType",
    };

    let matrix: CoverageMatrix = if matrix_path.exists() {
        serde_json::from_str(&fs::read_to_string(&matrix_path).unwrap()).unwrap()
    } else {
        eprintln!("SKIP [{label}]: {matrix_file} not found. Run scripts/generate_font_refs.py");
        return;
    };

    let mut total = 0u32;
    let mut passed = 0u32;
    let mut failed = 0u32;
    let mut skipped = 0u32;
    let mut font_cache: HashMap<String, Vec<u8>> = HashMap::new();

    for row in &matrix.rows {
        if row.status != "active" {
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

        let font = match Font::truetype(&font_data, row.size_pt, backend) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("  FAIL [{}]: font load error: {}", row.id, e);
                failed += 1;
                continue;
            }
        };

        match row.operation.as_str() {
            "getmask" => {
                let text = if row.char.is_empty() {
                    char::from_u32(row.codepoint)
                        .map(|c| c.to_string())
                        .unwrap_or_default()
                } else {
                    row.char.clone()
                };
                match font.getmask(&text) {
                    Ok(mask) => {
                        if let Some(ref expected_hash) = row.ref_sha256 {
                            let actual = sha256_hex(&mask.pixels);
                            if actual == *expected_hash {
                                eprintln!("  OK   [{}] {}x{}", row.id, mask.width, mask.height);
                                passed += 1;
                            } else {
                                eprintln!(
                                    "  FAIL [{}]: SHA-256 mismatch (expected {}... got {}...)",
                                    row.id,
                                    &expected_hash[..16],
                                    &actual[..16],
                                );
                                failed += 1;
                            }
                        } else {
                            passed += 1;
                        }
                    }
                    Err(e) => {
                        eprintln!("  FAIL [{}]: getmask error: {}", row.id, e);
                        failed += 1;
                    }
                }
            }
            "getbbox" => {
                let text: String = if row.char.is_empty() {
                    char::from_u32(row.codepoint)
                        .map(|c| c.to_string())
                        .unwrap_or_default()
                } else {
                    row.char.clone()
                };
                let bbox = font.getbbox(&text);
                if let Some(ref expected) = row.ref_value {
                    let actual_val = serde_json::json!([bbox.0, bbox.1, bbox.2, bbox.3]);
                    if &actual_val == expected {
                        eprintln!("  OK   [{}] bbox={:?}", row.id, bbox);
                        passed += 1;
                    } else {
                        eprintln!(
                            "  FAIL [{}]: bbox {:?} != expected {:?}",
                            row.id, bbox, expected
                        );
                        failed += 1;
                    }
                }
            }
            "getmetrics" => {
                let (asc, desc) = font.getmetrics();
                if let Some(ref expected) = row.ref_value {
                    let actual_val = serde_json::json!([asc, desc]);
                    if &actual_val == expected {
                        eprintln!("  OK   [{}] metrics=({},{})", row.id, asc, desc);
                        passed += 1;
                    } else {
                        eprintln!(
                            "  FAIL [{}]: metrics ({},{}) != expected {:?}",
                            row.id, asc, desc, expected
                        );
                        failed += 1;
                    }
                }
            }
            "getname" => {
                let (family, style) = font.getname();
                if let Some(ref expected) = row.ref_value {
                    let actual_val = serde_json::json!([family, style]);
                    if &actual_val == expected {
                        eprintln!("  OK   [{}] name=(\"{}\",\"{}\")", row.id, family, style);
                        passed += 1;
                    } else {
                        eprintln!(
                            "  FAIL [{}]: name (\"{}\",\"{}\") != expected {:?}",
                            row.id, family, style, expected
                        );
                        failed += 1;
                    }
                }
            }
            "getlength" => {
                let text = if row.char.is_empty() {
                    "Hello"
                } else {
                    &row.char
                };
                let length = font.getlength(text);
                if let Some(ref expected) = row.ref_value {
                    if let Some(expected_f) = expected.as_f64() {
                        if (length - expected_f as f32).abs() < 0.01 {
                            eprintln!("  OK   [{}] length={}", row.id, length);
                            passed += 1;
                        } else {
                            eprintln!(
                                "  FAIL [{}]: length {} != expected {}",
                                row.id, length, expected_f
                            );
                            failed += 1;
                        }
                    }
                }
            }
            _ => {
                skipped += 1;
            }
        }
    }

    eprintln!("\nfont matrix [{label}]: {passed}/{total} passed, {failed} failed, {skipped} skipped");
    if failed > 0 {
        panic!("{failed} font test(s) failed");
    }
    assert!(
        passed > 0,
        "No tests passed -- check font files and references"
    );
}
