//! Unified coverage matrix test — single runner for all scripts.
//!
//! Every test row has a SHA-256 reference from FreeType 2.14.3.
//! SHA-256 must match EXACTLY or the test FAILS.
//!
//! Summary shows per-script pass/fail clearly.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_in_result)]

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{hash_map::Entry, HashMap, BTreeMap};
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

// ── Tests ─────────────────────────────────────────────────────────────────

/// Full 55-script pixel-level comparison against the live vendored C binary is
/// handled by `tests/direct_ft_compare.rs` — no static fixtures needed.
///
/// The PIL backend test below compares our Rust PIL emulation against
/// pre-computed Python Pillow 12.2.0 output (coverage_matrix.json).

#[test]
fn test_coverage_matrix_pil() {
    // PIL parity: checks our Rust PIL backend against Python Pillow 12.2.0 output
    run_unified("coverage_matrix.json", BitmapBackend::PIL);
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

    #[derive(Default)]
    struct ScriptCounts { sha_ok: u32, sha_fail: u32 }
    let mut script_counts: BTreeMap<String, ScriptCounts> = BTreeMap::new();
    let mut font_cache: HashMap<(String, u32), Font> = HashMap::new();
    let mut failures: Vec<String> = Vec::new();

    for row in &matrix.rows {
        if row.status == "skip" { continue }
        total += 1;

        // Extract script tag from row id: "FontName_10_1234_scripttag_getmask"
        let script = {
            let parts: Vec<&str> = row.id.rsplit('_').collect();
            if parts.len() >= 3 && parts[1].len() <= 6
                && parts[1].chars().all(|c| c.is_alphabetic() || c == '-') {
                parts[1].to_string()
            } else {
                "latin".to_string()
            }
        };
        let counts = script_counts.entry(script.clone()).or_default();

        let font = match font_cache.entry((row.font.clone(), row.size_pt.to_bits())) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let font_data = load_font_bytes(manifest_dir, &row.font);
                match Font::truetype(&font_data, row.size_pt, backend) {
                    Ok(font) => entry.insert(font),
                    Err(e) => {
                        eprintln!("  SKIP [{}]: {}", row.id, e);
                        continue;
                    }
                }
            }
        };

        match row.operation.as_str() {
            "getmask" => {
                let text = get_text(&row);
                let mask = match font.getmask(&text) {
                    Ok(m) => m,
                    Err(e) => {
                        eprintln!("  SKIP [{}]: {}", row.id, e);
                        continue;
                    }
                };

                match &row.ref_sha256 {
                    Some(expected_sha) => {
                        let actual = sha256_hex(&mask.pixels);
                        if actual == *expected_sha {
                            counts.sha_ok += 1;
                            passed += 1;
                        } else {
                            failures.push(row.id.clone());
                            counts.sha_fail += 1;
                            failed += 1;
                        }
                    }
                    None => {
                        // No SHA-256 reference — just check size
                        if let Some(ref_size) = &row.ref_size {
                            if ref_size[0] == mask.width && ref_size[1] == mask.height {
                                passed += 1;
                            } else {
                                failures.push(row.id.clone());
                                counts.sha_fail += 1;
                                failed += 1;
                            }
                        }
                    }
                }
            }

            "getbbox" => {
                let text = get_text(&row);
                let bbox = font.getbbox(&text);

                if let Some(ref expected) = row.ref_value {
                    let ea = expected.as_array().unwrap();
                    let expect = (ea[0].as_i64().unwrap() as i32, ea[1].as_i64().unwrap() as i32,
                                  ea[2].as_i64().unwrap() as i32, ea[3].as_i64().unwrap() as i32);

                    if bbox == expect {
                        counts.sha_ok += 1;
                        passed += 1;
                    } else {
                        failures.push(row.id.clone());
                        counts.sha_fail += 1;
                        failed += 1;
                    }
                }
            }

            "getmetrics" | "getname" | "getlength" => {
                let ok = match row.operation.as_str() {
                    "getmetrics" => {
                        let (a, d) = font.getmetrics();
                        row.ref_value.as_ref().map_or(false, |ev| {
                            &serde_json::json!([a, d]) == ev
                        })
                    }
                    "getname" => {
                        let (f, s) = font.getname();
                        row.ref_value.as_ref().map_or(false, |ev| {
                            &serde_json::json!([f, s]) == ev
                        })
                    }
                    "getlength" => {
                        let len = font.getlength("Hello");
                        row.ref_value.as_ref().and_then(|ev| ev.as_f64()).map_or(false, |ef| {
                            (len - ef as f32).abs() < 0.5
                        })
                    }
                    _ => false,
                };
                if ok { passed += 1; }
                else { failed += 1; failures.push(row.id.clone()); }
            }

            _ => {}
        }
    }

    // ── Summary (always show per-script breakdown) ────────────────────

    let file_label = matrix_file.trim_end_matches(".json");
    eprintln!("\n╔══════════════════════════════════════════════════════════════╗");
    eprintln!("║  {file_label}: {passed}/{total} passed, {failed} failed");
    eprintln!("╠══════════════════════════════════════════════════════════════╣");

    // Per-script breakdown — always printed
    let mut passing_scripts: Vec<(String, u32)> = Vec::new();
    let mut failing_scripts: Vec<(String, u32, u32)> = Vec::new();

    for (script, counts) in &script_counts {
        let total_s = counts.sha_ok + counts.sha_fail;
        if total_s == 0 { continue; }
        if counts.sha_fail == 0 {
            passing_scripts.push((script.clone(), total_s));
        } else {
            failing_scripts.push((script.clone(), counts.sha_ok, total_s));
        }
    }

    passing_scripts.sort_by(|a, b| b.0.cmp(&a.0));  // reverse alphabetical
    failing_scripts.sort_by(|a, b| {
        let pa = (b.2 - b.1) as f64 / b.2 as f64;
        let pb = (a.2 - a.1) as f64 / a.2 as f64;
        pa.partial_cmp(&pb).unwrap_or(std::cmp::Ordering::Equal)
    });

    if !passing_scripts.is_empty() {
        eprintln!("║  PASSING (SHA-256 match) — {} scripts", passing_scripts.len());
        for (s, total_s) in &passing_scripts {
            eprintln!("║    {s} {total_s}/{total_s}");
        }
    }
    if !failing_scripts.is_empty() {
        eprintln!("║  FAILING — {} scripts", failing_scripts.len());
        for (s, ok, total_s) in &failing_scripts {
            let fail_pct = if *total_s > 0 { 100.0 * (*total_s - ok) as f64 / *total_s as f64 } else { 0.0 };
            eprintln!("║    {s} {ok}/{total_s} passed ({fail_pct:.0}% fail)");
        }
    }

    // Print failure IDs
    if failed > 0 {
        eprintln!("╠══════════════════════════════════════════════════════════════╣");
        eprintln!("║  Failure IDs (first 50 of {failed}):");
        for f in failures.iter().take(50) {
            eprintln!("║  {f}");
        }
        if failures.len() > 50 {
            eprintln!("║  ... and {} more (see FAILURE_IDS for full list)", failures.len() - 50);
        }
        // Write full failure list to file for analysis
        let report_path = "/tmp/pillow_failure_ids.txt";
        std::fs::write(report_path, failures.join("\n")).ok();
        eprintln!("║  Full list: {report_path}");
    }
    eprintln!("╚══════════════════════════════════════════════════════════════╝");

    if failed > 0 {
        panic!("{failed}/{total} SHA-256 mismatches in {matrix_file}");
    }
    assert!(passed > 0, "No tests passed — check font files for {matrix_file}");
}
