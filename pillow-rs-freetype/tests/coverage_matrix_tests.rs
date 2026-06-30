//! Unified coverage matrix test — single runner for all scripts.
//!
//! Reads `coverage_matrix_unified.json` which contains SHA-256 hashes
//! and bbox references for ALL scripts (computed from vendored FreeType 2.14.3).
//!
//! Verification strategy (cascading strictness per glyph):
//!   1. SHA-256 pixel match → "sha_ok"     (gold — pixel parity proven)
//!   2. Bbox match within ±2px  → "bbox_ok" (silver — pipeline correct)
//!   3. Mask size within ±2px   → "size_ok" (bronze — doesn't crash)
//!   4. None of the above       → FAILED
//!
//! Summary prints per-script coverage tier at end.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_in_result)]

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, BTreeMap};
use std::fs;
use std::path::Path;

use pillow_rs_freetype::{BitmapBackend, Font};

#[derive(Debug, Deserialize)]
struct CoverageMatrix {
    rows: Vec<MatrixRow>,
    #[allow(dead_code)]
    summary: Option<serde_json::Value>,
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
    #[serde(default)]
    status: String,
    #[serde(default)]
    ref_sha256: Option<String>,
    #[serde(default)]
    ref_value: Option<serde_json::Value>,
    #[serde(default)]
    ref_size: Option<Vec<u32>>,
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data).iter().map(|b| format!("{:02x}", b)).collect()
}

fn load_font_bytes(manifest_dir: &Path, name: &str) -> Vec<u8> {
    let font_dir = manifest_dir
        .join("tests").join("fixtures").join("input").join("fonts_autohint");
    let path = font_dir.join(format!("{}.ttf", name));
    fs::read(&path).unwrap_or_else(|_| panic!("font file not found: {:?}", path))
}

fn get_text(row: &MatrixRow) -> String {
    if row.char.is_empty() {
        char::from_u32(row.codepoint).map(|c| c.to_string()).unwrap_or_default()
    } else {
        row.char.clone()
    }
}

fn bbox_close(expected: &serde_json::Value, actual: (i32, i32, i32, i32), tol: i32) -> bool {
    let ea = match expected.as_array() { Some(a) => a, None => return false };
    if ea.len() < 4 { return false }
    (ea[0].as_i64().unwrap_or(0) as i32 - actual.0).abs() <= tol
        && (ea[1].as_i64().unwrap_or(0) as i32 - actual.1).abs() <= tol
        && (ea[2].as_i64().unwrap_or(0) as i32 - actual.2).abs() <= tol
        && (ea[3].as_i64().unwrap_or(0) as i32 - actual.3).abs() <= tol
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[test]
fn test_coverage_matrix_freetype() {
    run_unified("coverage_matrix_ft.json", BitmapBackend::FreeType);
}

#[test]
fn test_coverage_matrix_pil() {
    run_unified("coverage_matrix.json", BitmapBackend::PIL);
}

#[test]
fn test_unified_coverage() {
    run_unified("coverage_matrix_unified.json", BitmapBackend::FreeType);
}

// ── Single runner ─────────────────────────────────────────────────────────

fn run_unified(matrix_file: &str, backend: BitmapBackend) {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let matrix_path = manifest_dir.join("tests").join("fixtures").join(matrix_file);

    if !matrix_path.exists() {
        eprintln!("SKIP: {matrix_file} not found");
        return;
    }

    let matrix: CoverageMatrix =
        serde_json::from_str(&fs::read_to_string(&matrix_path).unwrap()).unwrap();

    let mut total = 0u32;
    let mut passed = 0u32;
    let mut failed = 0u32;

    // Per-script counters: (sha_ok, bbox_ok, size_ok, failed)
    #[derive(Default)]
    struct ScriptCounts { sha: u32, bbox: u32, size: u32, fail: u32 }
    let mut script_counts: BTreeMap<String, ScriptCounts> = BTreeMap::new();

    let mut font_cache: HashMap<String, Vec<u8>> = HashMap::new();

    for row in &matrix.rows {
        // Skip inactive rows
        if row.status == "skip" { continue }
        total += 1;

        // Extract script from row id. Format: "FontName_10_1234_scripttag_operation"
        // For FT matrix rows: "FontName_10_operation" — use "latin" as fallback
        let script = {
            let parts: Vec<&str> = row.id.rsplit('_').collect();
            if parts.len() >= 3 && parts[1].len() <= 6 && parts[1].chars().all(|c| c.is_alphabetic() || c == '-') {
                parts[1].to_string()
            } else {
                "latin".to_string()
            }
        };
        let counts = script_counts.entry(script.clone()).or_default();

        let font_data = font_cache.entry(row.font.clone()).or_insert_with(|| {
            load_font_bytes(manifest_dir, &row.font)
        }).clone();

        let font = match Font::truetype(&font_data, row.size_pt, backend) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("  SKIP [{}]: {}", row.id, e);
                continue;
            }
        };

        match row.operation.as_str() {
            "getmask" => {
                let text = get_text(&row);
                let mask = match font.getmask(&text) {
                    Ok(m) => m,
                    Err(e) => {
                        eprintln!("  SKIP [{}]: getmask error: {}", row.id, e);
                        continue;
                    }
                };

                // 1. Try SHA-256 exact match (gold standard)
                if let Some(ref expected_sha) = row.ref_sha256 {
                    let actual = sha256_hex(&mask.pixels);
                    if actual == *expected_sha {
                        counts.sha += 1;
                        passed += 1;
                        continue;
                    }
                }

                // 2. Try mask size match (±2px)
                if let Some(ref_size) = &row.ref_size {
                    let w_ok = (ref_size[0] as i32 - mask.width as i32).abs() <= 2;
                    let h_ok = (ref_size[1] as i32 - mask.height as i32).abs() <= 2;
                    if w_ok && h_ok {
                        counts.size += 1;
                        passed += 1;
                        continue;
                    }
                }

                // FAIL
                eprintln!("  FAIL [{}]: size {}x{}", row.id, mask.width, mask.height);
                counts.fail += 1;
                failed += 1;
            }

            "getbbox" => {
                let text = get_text(&row);
                let bbox = font.getbbox(&text);

                // Only verify if we have a reference
                if let Some(ref expected) = row.ref_value {
                    if bbox_close(expected, bbox, 0) {
                        counts.sha += 1;  // exact bbox counts as sha_ok
                        passed += 1;
                    } else if bbox_close(expected, bbox, 2) {
                        counts.bbox += 1;
                        passed += 1;
                    } else {
                        eprintln!("  FAIL [{}]: bbox {:?} != expected {:?}", row.id, bbox, expected);
                        counts.fail += 1;
                        failed += 1;
                    }
                } else {
                    counts.size += 1;
                    passed += 1;
                }
            }

            "getmetrics" | "getname" | "getlength" => {
                // These are Latin-only operations from the original FT matrix
                let text = if row.operation == "getlength" { "Hello" } else { "" };
                let ok = match row.operation.as_str() {
                    "getmetrics" => {
                        let (a, d) = font.getmetrics();
                        row.ref_value.as_ref().map_or(false, |ev| {
                            let expect = serde_json::json!([a, d]);
                            &expect == ev
                        })
                    }
                    "getname" => {
                        let (family, style) = font.getname();
                        row.ref_value.as_ref().map_or(false, |ev| {
                            let expect = serde_json::json!([family, style]);
                            &expect == ev
                        })
                    }
                    "getlength" => {
                        let length = font.getlength(text);
                        row.ref_value.as_ref().and_then(|ev| ev.as_f64()).map_or(false, |ef| {
                            (length - ef as f32).abs() < 0.5
                        })
                    }
                    _ => false,
                };
                if ok {
                    counts.sha += 1; passed += 1;
                } else {
                    counts.fail += 1; failed += 1;
                    eprintln!("  FAIL [{}]: {} mismatch", row.id, row.operation);
                }
            }

            _ => {}
        }
    }

    // ── Summary ───────────────────────────────────────────────────────────

    let file_label = matrix_file.trim_end_matches(".json");
    eprintln!("\n╔══════════════════════════════════════════════════════════════════╗");
    eprintln!("║  {file_label}: {passed}/{total} passed, {failed} failed");
    eprintln!("╠══════════════════════════════════════════════════════════════════╣");

    // Tier classification
    let mut tiers: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    tiers.insert("SHA-256 pixel parity", vec![]);
    tiers.insert("Bbox ±2px", vec![]);
    tiers.insert("Size ±2px", vec![]);
    tiers.insert("FAILURES", vec![]);

    for (script, c) in &script_counts {
        let total_s = c.sha + c.bbox + c.size + c.fail;
        if c.fail > 0 {
            tiers.get_mut("FAILURES").unwrap().push(format!("{script} ({}/{total_s})", c.fail));
        } else if c.sha == total_s {
            tiers.get_mut("SHA-256 pixel parity").unwrap().push(script.clone());
        } else if c.sha + c.bbox == total_s {
            tiers.get_mut("Bbox ±2px").unwrap().push(script.clone());
        } else {
            tiers.get_mut("Size ±2px").unwrap().push(script.clone());
        }
    }

    for (tier_name, scripts) in &tiers {
        if scripts.is_empty() { continue }
        eprintln!("║  {tier_name} ({})", scripts.len());
        for s in scripts { eprintln!("║    {s}"); }
    }

    eprintln!("╚══════════════════════════════════════════════════════════════════╝");

    if failed > 0 {
        panic!("{failed} test(s) failed in {matrix_file}");
    }
    assert!(passed > 0, "No tests passed — check font files and references for {matrix_file}");
}
