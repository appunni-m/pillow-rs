//! Unified coverage matrix test: one runner for static FreeType oracle matrices.
//!
//! Exact parity gates require raw pixel bytes when the endpoint renders a
//! bitmap, and exact scalar/geometry values for non-rendering endpoints.
//! Failures include byte-level diff stats for rasterizer parity debugging.
//!
//! Summary shows per-script pass/fail clearly.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_in_result)]
#![allow(clippy::arithmetic_side_effects)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::unnecessary_map_or)]
#![allow(unused_crate_dependencies)]

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, hash_map::Entry};
use std::fs;
use std::path::{Path, PathBuf};

use env_logger as _;
use fontdone::{Font, LoadMode, RenderMode, grays, scaler};
use log as _;
use thiserror as _;

#[derive(Debug, Deserialize)]
struct CoverageMatrix {
    rows: Vec<MatrixRow>,
    #[allow(dead_code)]
    summary: Option<serde_json::Value>,
    #[serde(default)]
    fixture_family: String,
    #[serde(default)]
    generator: String,
    #[serde(default)]
    load_flags: Vec<String>,
    #[serde(default)]
    render_mode: String,
    #[serde(default = "default_assert_pixel_parity")]
    assert_pixel_parity: bool,
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
    #[serde(default)]
    ref_raw: Option<String>,
    #[serde(default)]
    fixture_family: String,
    #[serde(default)]
    generator: String,
    #[serde(default)]
    load_flags: Vec<String>,
    #[serde(default)]
    render_mode: String,
    #[serde(default)]
    glyph_index: Option<u32>,
    #[serde(default)]
    script: Option<String>,
    #[serde(default)]
    metrics: serde_json::Value,
    #[serde(default)]
    bbox: serde_json::Value,
    #[serde(default)]
    bitmap: serde_json::Value,
    #[serde(default)]
    bitmap_placement: serde_json::Value,
    #[serde(default)]
    raw_pixels: String,
}

fn default_assert_pixel_parity() -> bool {
    true
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

fn get_text(row: &MatrixRow) -> String {
    if row.char.is_empty() {
        char::from_u32(row.codepoint)
            .map(|c| c.to_string())
            .unwrap_or_default()
    } else {
        row.char.clone()
    }
}

#[derive(Debug)]
struct PixelDiff {
    diff_count: u32,
    max_diff: u32,
    total_abs_diff: u64,
    first_diff: Option<usize>,
    size_delta: i32,
    width_delta: i32,
    height_delta: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum FailureStage {
    GlyphIndex,
    LoadError,
    RawOutline,
    ScaledOutline,
    HintedOutline,
    Metrics,
    Bbox,
    BitmapPlacement,
    PixelCoverage,
}

impl FailureStage {
    fn label(self) -> &'static str {
        match self {
            FailureStage::GlyphIndex => "glyph index",
            FailureStage::LoadError => "load error",
            FailureStage::RawOutline => "raw outline",
            FailureStage::ScaledOutline => "scaled outline",
            FailureStage::HintedOutline => "hinted outline",
            FailureStage::Metrics => "metrics",
            FailureStage::Bbox => "bbox",
            FailureStage::BitmapPlacement => "bitmap placement",
            FailureStage::PixelCoverage => "pixel coverage",
        }
    }
}

fn classify_pixel_failure(diff: &PixelDiff) -> FailureStage {
    if diff.size_delta != 0 || diff.width_delta != 0 || diff.height_delta != 0 {
        FailureStage::BitmapPlacement
    } else {
        FailureStage::PixelCoverage
    }
}

fn raw_pixel_paths(manifest_dir: &Path, row: &MatrixRow) -> Vec<PathBuf> {
    let fixture_dir = manifest_dir.join("tests").join("fixtures");
    let raw_dir = fixture_dir.join("outputs").join("raws");
    let mut paths = Vec::new();

    if let Some(ref_raw) = &row.ref_raw {
        paths.push(fixture_dir.join(ref_raw));
    }
    paths.push(raw_dir.join(format!("{}.bin", row.id)));

    if row.operation == "getmask" {
        let size = row.size_pt.round() as u32;
        paths.push(raw_dir.join(format!(
            "{}_{}_{}_getmask.bin",
            row.font, size, row.codepoint
        )));
    }

    paths
}

fn load_raw_pixels(manifest_dir: &Path, row: &MatrixRow) -> Option<(PathBuf, Vec<u8>)> {
    raw_pixel_paths(manifest_dir, row)
        .into_iter()
        .find_map(|path| fs::read(&path).ok().map(|pixels| (path, pixels)))
}

fn validate_fixture_provenance(matrix_file: &str, matrix: &CoverageMatrix) {
    assert!(
        !matrix.fixture_family.is_empty(),
        "{matrix_file} is missing fixture_family"
    );
    assert!(
        !matrix.generator.is_empty(),
        "{matrix_file} is missing generator"
    );
    assert!(
        !matrix.load_flags.is_empty(),
        "{matrix_file} is missing load_flags"
    );
    assert!(
        !matrix.render_mode.is_empty(),
        "{matrix_file} is missing render_mode"
    );

    for row in &matrix.rows {
        assert_eq!(
            row.fixture_family, matrix.fixture_family,
            "{} has mismatched fixture_family",
            row.id
        );
        assert!(!row.generator.is_empty(), "{} is missing generator", row.id);
        assert!(
            !row.load_flags.is_empty(),
            "{} is missing load_flags",
            row.id
        );
        assert!(
            !row.render_mode.is_empty(),
            "{} is missing render_mode",
            row.id
        );
        assert!(row.size_pt > 0.0, "{} is missing size", row.id);
        assert!(!row.font.is_empty(), "{} is missing font", row.id);
        assert!(
            row.metrics.is_object(),
            "{} is missing metrics object",
            row.id
        );
        assert!(row.bbox.is_object(), "{} is missing bbox object", row.id);
        assert!(
            row.bitmap.is_object(),
            "{} is missing bitmap object",
            row.id
        );
        assert!(
            row.bitmap_placement.is_object(),
            "{} is missing bitmap placement object",
            row.id
        );
        if row.operation == "getmask" {
            assert!(
                row.glyph_index.is_some() || row.codepoint > 0,
                "{} is missing glyph/codepoint identity",
                row.id
            );
            assert!(
                row.ref_raw.is_some() || !row.raw_pixels.is_empty(),
                "{} is missing raw pixels",
                row.id
            );
        }
    }
}

fn pixel_diff(
    actual: &[u8],
    expected: &[u8],
    actual_w: u32,
    actual_h: u32,
    expected_w: u32,
    expected_h: u32,
) -> PixelDiff {
    let min_len = actual.len().min(expected.len());
    let mut diff_count = actual.len().max(expected.len()).saturating_sub(min_len) as u32;
    let mut max_diff = 0u32;
    let mut total_abs_diff = 0u64;
    let mut first_diff = if actual.len() == expected.len() {
        None
    } else {
        Some(min_len)
    };

    for i in 0..min_len {
        let diff = (actual[i] as i32 - expected[i] as i32).unsigned_abs();
        if diff != 0 {
            diff_count += 1;
            max_diff = max_diff.max(diff);
            total_abs_diff += diff as u64;
            if first_diff.is_none() {
                first_diff = Some(i);
            }
        }
    }

    PixelDiff {
        diff_count,
        max_diff,
        total_abs_diff,
        first_diff,
        size_delta: actual.len() as i32 - expected.len() as i32,
        width_delta: actual_w as i32 - expected_w as i32,
        height_delta: actual_h as i32 - expected_h as i32,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

/// Full 55-script pixel-level comparison against the live pinned C binary is
/// handled by `tests/direct_ft_compare.rs` — no static fixtures needed.
///
/// The native TrueType test below compares our Rust native bytecode path
/// against a FreeType-compatible default TrueType fixture.

#[test]
#[ignore = "deprecated: replaced by manifest-driven unified public API parity test"]
fn test_native_tt_default_matrix_exact_parity() {
    // Native TrueType default parity: FreeType's default load/render path runs
    // embedded TrueType bytecode instead of forcing the autohinter.
    run_unified("native_tt_default_matrix.json", LoadMode::Default, None);
}

#[test]
#[ignore = "deprecated: replaced by manifest-driven unified public API parity test"]
fn test_coverage_matrix_force_autohint() {
    // Static FT parity: checks raw pixel refs generated from pinned FreeType.
    run_unified("force_autohint_matrix.json", LoadMode::ForceAutoHint, None);
}

#[test]
#[ignore = "deprecated: replaced by manifest-driven unified public API parity test"]
fn test_render_mono_matrix_exact_parity() {
    run_unified("render_mono_matrix.json", LoadMode::ForceAutoHint, None);
}

#[test]
#[ignore = "deprecated: replaced by manifest-driven unified public API parity test"]
fn test_render_lcd_matrix_exact_parity() {
    run_unified("render_lcd_matrix.json", LoadMode::ForceAutoHint, None);
}

#[test]
#[ignore = "deprecated: replaced by manifest-driven unified public API parity test"]
fn test_no_hinting_matrix_exact_parity() {
    run_unified("no_hinting_matrix.json", LoadMode::Default, None);
}

#[test]
#[ignore = "deprecated: replaced by manifest-driven unified public API parity test"]
fn test_metrics_only_matrix_exact_parity() {
    run_unified("metrics_only_matrix.json", LoadMode::Default, None);
}

#[test]
#[ignore = "deprecated: replaced by manifest-driven unified public API parity test"]
fn test_outline_cbox_matrix_exact_parity() {
    run_unified("outline_cbox_matrix.json", LoadMode::Default, None);
}

#[test]
#[ignore = "deprecated: replaced by manifest-driven unified public API parity test"]
fn test_fixture_matrix_provenance() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture_dir = manifest_dir.join("tests").join("fixtures");
    let expected = [
        "native_tt_default_matrix.json",
        "force_autohint_matrix.json",
        "no_hinting_matrix.json",
        "metrics_only_matrix.json",
        "outline_cbox_matrix.json",
        "render_mono_matrix.json",
        "render_lcd_matrix.json",
    ];

    for matrix_file in expected {
        let matrix_path = fixture_dir.join(matrix_file);
        assert!(matrix_path.exists(), "{matrix_file} not found");
        let matrix: CoverageMatrix =
            serde_json::from_str(&fs::read_to_string(&matrix_path).unwrap()).unwrap();
        validate_fixture_provenance(matrix_file, &matrix);
    }
}

// ── Single runner ─────────────────────────────────────────────────────────

fn run_unified(matrix_file: &str, load_mode: LoadMode, expected_partial: Option<(u32, u32)>) {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let matrix_path = manifest_dir
        .join("tests")
        .join("fixtures")
        .join(matrix_file);

    assert!(matrix_path.exists(), "{matrix_file} not found");

    let matrix: CoverageMatrix =
        serde_json::from_str(&fs::read_to_string(&matrix_path).unwrap()).unwrap();
    validate_fixture_provenance(matrix_file, &matrix);
    let exact_pixel_gate = matrix.assert_pixel_parity && expected_partial.is_none();

    let mut total = 0u32;
    let mut passed = 0u32;
    let mut failed = 0u32;

    #[derive(Default)]
    struct ScriptCounts {
        sha_ok: u32,
        sha_fail: u32,
    }
    let mut script_counts: BTreeMap<String, ScriptCounts> = BTreeMap::new();
    let mut font_cache: HashMap<(String, u32), Font> = HashMap::new();
    let mut failures: Vec<String> = Vec::new();
    let mut stage_counts: BTreeMap<FailureStage, u32> = BTreeMap::new();

    for row in &matrix.rows {
        if row.status == "skip" {
            continue;
        }
        total += 1;

        // Extract script tag from row id: "FontName_10_1234_scripttag_getmask"
        let script = row.script.clone().unwrap_or_else(|| {
            let parts: Vec<&str> = row.id.rsplit('_').collect();
            if parts.len() >= 3
                && parts[1].len() <= 6
                && parts[1].chars().all(|c| c.is_alphabetic() || c == '-')
            {
                parts[1].to_string()
            } else {
                "latin".to_string()
            }
        });
        let counts = script_counts.entry(script.clone()).or_default();

        let font = match font_cache.entry((row.font.clone(), row.size_pt.to_bits())) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let font_data = load_font_bytes(manifest_dir, &row.font);
                match Font::truetype_with_load_mode(&font_data, row.size_pt, load_mode) {
                    Ok(font) => entry.insert(font),
                    Err(e) => {
                        failed += 1;
                        *stage_counts.entry(FailureStage::LoadError).or_default() += 1;
                        failures.push(format!(
                            "{} stage={} error={}",
                            row.id,
                            FailureStage::LoadError.label(),
                            e
                        ));
                        continue;
                    }
                }
            }
        };

        match row.operation.as_str() {
            "getmask" => {
                if matrix.fixture_family == "no_hinting" {
                    match compare_no_hinting_row(font, row, manifest_dir) {
                        Ok(()) => {
                            counts.sha_ok += 1;
                            passed += 1;
                        }
                        Err((stage, failure)) => {
                            *stage_counts.entry(stage).or_default() += 1;
                            failures.push(failure);
                            counts.sha_fail += 1;
                            failed += 1;
                        }
                    }
                    continue;
                }

                if let Some(mode) = render_mode_for_family(&matrix.fixture_family) {
                    match compare_render_mode_row(font, row, manifest_dir, mode) {
                        Ok(()) => {
                            counts.sha_ok += 1;
                            passed += 1;
                        }
                        Err((stage, failure)) => {
                            *stage_counts.entry(stage).or_default() += 1;
                            failures.push(failure);
                            counts.sha_fail += 1;
                            failed += 1;
                        }
                    }
                    continue;
                }

                let text = get_text(row);
                let mask = match font.getmask(&text) {
                    Ok(m) => m,
                    Err(e) => {
                        failed += 1;
                        *stage_counts.entry(FailureStage::LoadError).or_default() += 1;
                        failures.push(format!(
                            "{} stage={} error={}",
                            row.id,
                            FailureStage::LoadError.label(),
                            e
                        ));
                        continue;
                    }
                };

                let expected_size = row.ref_size.as_ref().and_then(|s| {
                    if s.len() >= 2 {
                        Some((s[0], s[1]))
                    } else {
                        None
                    }
                });
                let actual_sha = sha256_hex(&mask.pixels);

                let raw_pixels = load_raw_pixels(manifest_dir, row);
                if exact_pixel_gate && raw_pixels.is_none() {
                    failed += 1;
                    *stage_counts.entry(FailureStage::PixelCoverage).or_default() += 1;
                    failures.push(format!(
                        "{} stage={} raw=missing exact_pixel_gate=true",
                        row.id,
                        FailureStage::PixelCoverage.label(),
                    ));
                    counts.sha_fail += 1;
                    continue;
                }

                if let Some((raw_path, expected_pixels)) = raw_pixels {
                    let (expected_w, expected_h) =
                        expected_size.unwrap_or((mask.width, mask.height));
                    if mask.pixels == expected_pixels
                        && mask.width == expected_w
                        && mask.height == expected_h
                    {
                        counts.sha_ok += 1;
                        passed += 1;
                    } else {
                        let diff = pixel_diff(
                            &mask.pixels,
                            &expected_pixels,
                            mask.width,
                            mask.height,
                            expected_w,
                            expected_h,
                        );
                        let stage = classify_pixel_failure(&diff);
                        *stage_counts.entry(stage).or_default() += 1;
                        failures.push(format!(
                            "{} stage={} actual_sha={} raw={} actual={}x{} expected={}x{} diffs={} max={} total_abs={} first={:?} size_delta={} width_delta={} height_delta={}",
                            row.id,
                            stage.label(),
                            actual_sha,
                            raw_path.display(),
                            mask.width,
                            mask.height,
                            expected_w,
                            expected_h,
                            diff.diff_count,
                            diff.max_diff,
                            diff.total_abs_diff,
                            diff.first_diff,
                            diff.size_delta,
                            diff.width_delta,
                            diff.height_delta,
                        ));
                        counts.sha_fail += 1;
                        failed += 1;
                    }
                } else if let Some(expected_sha) = &row.ref_sha256 {
                    if actual_sha == *expected_sha {
                        counts.sha_ok += 1;
                        passed += 1;
                    } else {
                        *stage_counts.entry(FailureStage::PixelCoverage).or_default() += 1;
                        failures.push(format!(
                            "{} stage={} actual_sha={} expected_sha={} raw=missing",
                            row.id,
                            FailureStage::PixelCoverage.label(),
                            actual_sha,
                            expected_sha,
                        ));
                        counts.sha_fail += 1;
                        failed += 1;
                    }
                } else if let Some((expected_w, expected_h)) = expected_size {
                    if expected_w == mask.width && expected_h == mask.height {
                        passed += 1;
                    } else {
                        *stage_counts
                            .entry(FailureStage::BitmapPlacement)
                            .or_default() += 1;
                        failures.push(format!(
                            "{} stage={} actual={}x{} expected={}x{} raw=missing sha=missing",
                            row.id,
                            FailureStage::BitmapPlacement.label(),
                            mask.width,
                            mask.height,
                            expected_w,
                            expected_h,
                        ));
                        counts.sha_fail += 1;
                        failed += 1;
                    }
                }
            }

            "metrics_only" => match compare_metrics_only_row(font, row) {
                Ok(()) => {
                    counts.sha_ok += 1;
                    passed += 1;
                }
                Err((stage, failure)) => {
                    *stage_counts.entry(stage).or_default() += 1;
                    failures.push(failure);
                    counts.sha_fail += 1;
                    failed += 1;
                }
            },

            "outline_cbox" => match compare_outline_cbox_row(font, row) {
                Ok(()) => {
                    counts.sha_ok += 1;
                    passed += 1;
                }
                Err((stage, failure)) => {
                    *stage_counts.entry(stage).or_default() += 1;
                    failures.push(failure);
                    counts.sha_fail += 1;
                    failed += 1;
                }
            },

            "getbbox" => {
                let text = get_text(row);
                let bbox = font.getbbox(&text);

                if let Some(ref expected) = row.ref_value {
                    let ea = expected.as_array().unwrap();
                    let expect = (
                        ea[0].as_i64().unwrap() as i32,
                        ea[1].as_i64().unwrap() as i32,
                        ea[2].as_i64().unwrap() as i32,
                        ea[3].as_i64().unwrap() as i32,
                    );

                    if bbox == expect {
                        counts.sha_ok += 1;
                        passed += 1;
                    } else {
                        *stage_counts.entry(FailureStage::Bbox).or_default() += 1;
                        failures.push(format!(
                            "{} stage={} actual={:?} expected={:?}",
                            row.id,
                            FailureStage::Bbox.label(),
                            bbox,
                            expect
                        ));
                        counts.sha_fail += 1;
                        failed += 1;
                    }
                }
            }

            "getmetrics" | "getname" | "getlength" => {
                let mut detail = String::new();
                let ok = match row.operation.as_str() {
                    "getmetrics" => {
                        let (a, d) = font.getmetrics();
                        let actual = serde_json::json!([a, d]);
                        let expected = row.ref_value.as_ref();
                        detail = format!(" actual={actual} expected={:?}", expected);
                        expected == Some(&actual)
                    }
                    "getname" => {
                        let (f, s) = font.getname();
                        let actual = serde_json::json!([f, s]);
                        let expected = row.ref_value.as_ref();
                        detail = format!(" actual={actual} expected={:?}", expected);
                        expected == Some(&actual)
                    }
                    "getlength" => {
                        let len = font.getlength("Hello");
                        let expected = row.ref_value.as_ref().and_then(|ev| ev.as_f64());
                        detail = format!(" actual={len:.6} expected={expected:?}");
                        expected.is_some_and(|ef| (len - ef as f32).abs() < 0.5)
                    }
                    _ => false,
                };
                if ok {
                    passed += 1;
                } else {
                    failed += 1;
                    *stage_counts.entry(FailureStage::Metrics).or_default() += 1;
                    failures.push(format!(
                        "{} stage={}{}",
                        row.id,
                        FailureStage::Metrics.label(),
                        detail
                    ));
                }
            }

            other => {
                panic!(
                    "{matrix_file} row {} has unsupported operation {other}",
                    row.id
                );
            }
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
        if total_s == 0 {
            continue;
        }
        if counts.sha_fail == 0 {
            passing_scripts.push((script.clone(), total_s));
        } else {
            failing_scripts.push((script.clone(), counts.sha_ok, total_s));
        }
    }

    passing_scripts.sort_by(|a, b| b.0.cmp(&a.0)); // reverse alphabetical
    failing_scripts.sort_by(|a, b| {
        let pa = (b.2 - b.1) as f64 / b.2 as f64;
        let pb = (a.2 - a.1) as f64 / a.2 as f64;
        pa.partial_cmp(&pb).unwrap_or(std::cmp::Ordering::Equal)
    });

    if !passing_scripts.is_empty() {
        eprintln!(
            "║  PASSING (pixel match) — {} scripts",
            passing_scripts.len()
        );
        for (s, total_s) in &passing_scripts {
            eprintln!("║    {s} {total_s}/{total_s}");
        }
    }
    if !failing_scripts.is_empty() {
        eprintln!("║  FAILING — {} scripts", failing_scripts.len());
        for (s, ok, total_s) in &failing_scripts {
            let fail_pct = if *total_s > 0 {
                100.0 * (*total_s - ok) as f64 / *total_s as f64
            } else {
                0.0
            };
            eprintln!("║    {s} {ok}/{total_s} passed ({fail_pct:.0}% fail)");
        }
    }

    // Print failure IDs
    if failed > 0 {
        eprintln!("╠══════════════════════════════════════════════════════════════╣");
        eprintln!("║  Failure stages:");
        for (stage, count) in &stage_counts {
            eprintln!("║    {} {count}", stage.label());
        }
        eprintln!("╠══════════════════════════════════════════════════════════════╣");
        eprintln!("║  Failure IDs (first 50 of {failed}):");
        for f in failures.iter().take(50) {
            eprintln!("║  {f}");
        }
        if failures.len() > 50 {
            eprintln!(
                "║  ... and {} more (see FAILURE_IDS for full list)",
                failures.len() - 50
            );
        }
        // Write full failure list to file for analysis
        let report_path = "/tmp/freetype_failure_ids.txt";
        std::fs::write(report_path, failures.join("\n")).ok();
        eprintln!("║  Full list: {report_path}");
    }
    eprintln!("╚══════════════════════════════════════════════════════════════╝");

    if let Some((min_passed, expected_total)) = expected_partial {
        assert_eq!(
            total, expected_total,
            "{matrix_file} total changed; refresh the executed baseline intentionally"
        );
        assert!(
            passed >= min_passed,
            "{matrix_file} regressed below executed baseline: {passed}/{total} < {min_passed}/{expected_total}"
        );
        if failed > 0 {
            return;
        }
    }

    if failed > 0 {
        if !matrix.assert_pixel_parity {
            return;
        }
        panic!("{failed}/{total} pixel mismatches in {matrix_file}");
    }
    assert!(
        passed > 0,
        "No tests passed — check font files for {matrix_file}"
    );
}

fn render_mode_for_family(fixture_family: &str) -> Option<RenderMode> {
    match fixture_family {
        "render_mono" => Some(RenderMode::Mono),
        "render_lcd" => Some(RenderMode::Lcd),
        _ => None,
    }
}

fn compare_render_mode_row(
    font: &Font,
    row: &MatrixRow,
    manifest_dir: &Path,
    mode: RenderMode,
) -> Result<(), (FailureStage, String)> {
    let bitmap = font
        .render_char_mode(char::from_u32(row.codepoint).unwrap_or('\0'), mode)
        .map_err(|err| {
            (
                FailureStage::LoadError,
                format!(
                    "{} stage={} error={}",
                    row.id,
                    FailureStage::LoadError.label(),
                    err
                ),
            )
        })?;

    let Some((raw_path, expected_pixels)) = load_raw_pixels(manifest_dir, row) else {
        return Err((
            FailureStage::PixelCoverage,
            format!(
                "{} stage={} raw=missing render_mode={}",
                row.id,
                FailureStage::PixelCoverage.label(),
                mode.fixture_name()
            ),
        ));
    };

    let expected_width = bitmap_u32(row, "width").unwrap_or(bitmap.width);
    let expected_rows = bitmap_u32(row, "rows").unwrap_or(bitmap.rows);
    let expected_pitch = bitmap_i32(row, "pitch").unwrap_or(bitmap.pitch);
    let expected_left = bitmap_i32(row, "left").unwrap_or(bitmap.left);
    let expected_top = bitmap_i32(row, "top").unwrap_or(bitmap.top);

    if bitmap.buffer == expected_pixels
        && bitmap.width == expected_width
        && bitmap.rows == expected_rows
        && bitmap.pitch == expected_pitch
        && bitmap.left == expected_left
        && bitmap.top == expected_top
    {
        return Ok(());
    }

    let diff = pixel_diff(
        &bitmap.buffer,
        &expected_pixels,
        bitmap.width,
        bitmap.rows,
        expected_width,
        expected_rows,
    );
    let stage = if bitmap.pitch != expected_pitch
        || bitmap.left != expected_left
        || bitmap.top != expected_top
    {
        FailureStage::BitmapPlacement
    } else {
        classify_pixel_failure(&diff)
    };

    Err((
        stage,
        format!(
            "{} stage={} render_mode={} actual={}x{} pitch={} left={} top={} raw={} expected={}x{} pitch={} left={} top={} diffs={} max={} total_abs={} first={:?} size_delta={} width_delta={} height_delta={}",
            row.id,
            stage.label(),
            mode.fixture_name(),
            bitmap.width,
            bitmap.rows,
            bitmap.pitch,
            bitmap.left,
            bitmap.top,
            raw_path.display(),
            expected_width,
            expected_rows,
            expected_pitch,
            expected_left,
            expected_top,
            diff.diff_count,
            diff.max_diff,
            diff.total_abs_diff,
            diff.first_diff,
            diff.size_delta,
            diff.width_delta,
            diff.height_delta,
        ),
    ))
}

fn bitmap_u32(row: &MatrixRow, key: &str) -> Option<u32> {
    row.bitmap
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
}

fn bitmap_i32(row: &MatrixRow, key: &str) -> Option<i32> {
    row.bitmap
        .get(key)
        .and_then(serde_json::Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
}

fn compare_no_hinting_row(
    font: &Font,
    row: &MatrixRow,
    manifest_dir: &Path,
) -> Result<(), (FailureStage, String)> {
    let glyph = font.data.cmap.char_index(row.codepoint).unwrap_or(0);
    let scaled =
        scaler::scale_glyph_no_hinting(&font.data, glyph, font.is_italic).map_err(|err| {
            (
                FailureStage::LoadError,
                format!(
                    "{} stage={} error={}",
                    row.id,
                    FailureStage::LoadError.label(),
                    err
                ),
            )
        })?;
    let raster = grays::rasterize(scaled.outline).map_err(|err| {
        (
            FailureStage::PixelCoverage,
            format!(
                "{} stage={} error={}",
                row.id,
                FailureStage::PixelCoverage.label(),
                err
            ),
        )
    })?;

    let Some((raw_path, expected_pixels)) = load_raw_pixels(manifest_dir, row) else {
        return Err((
            FailureStage::PixelCoverage,
            format!(
                "{} stage={} raw=missing",
                row.id,
                FailureStage::PixelCoverage.label()
            ),
        ));
    };

    let actual_width = u32_from_usize_for_test(raster.width);
    let actual_rows = u32_from_usize_for_test(raster.height);
    let actual_pitch = i32_from_usize_for_test(raster.width);
    let actual_left = scaled.bbox_x_min;
    let actual_top = scaled.bbox_y_max;
    let expected_width = bitmap_u32(row, "width").unwrap_or(actual_width);
    let expected_rows = bitmap_u32(row, "rows").unwrap_or(actual_rows);
    let expected_pitch = bitmap_i32(row, "pitch").unwrap_or(actual_pitch);
    let expected_left = bitmap_i32(row, "left").unwrap_or(actual_left);
    let expected_top = bitmap_i32(row, "top").unwrap_or(actual_top);

    if raster.pixels == expected_pixels
        && actual_width == expected_width
        && actual_rows == expected_rows
        && actual_pitch == expected_pitch
        && actual_left == expected_left
        && actual_top == expected_top
    {
        return Ok(());
    }

    let diff = pixel_diff(
        &raster.pixels,
        &expected_pixels,
        actual_width,
        actual_rows,
        expected_width,
        expected_rows,
    );
    let stage = if actual_pitch != expected_pitch
        || actual_left != expected_left
        || actual_top != expected_top
    {
        FailureStage::BitmapPlacement
    } else {
        classify_pixel_failure(&diff)
    };
    Err((
        stage,
        format!(
            "{} stage={} actual={}x{} pitch={} left={} top={} raw={} expected={}x{} pitch={} left={} top={} diffs={} max={} total_abs={} first={:?} size_delta={} width_delta={} height_delta={}",
            row.id,
            stage.label(),
            actual_width,
            actual_rows,
            actual_pitch,
            actual_left,
            actual_top,
            raw_path.display(),
            expected_width,
            expected_rows,
            expected_pitch,
            expected_left,
            expected_top,
            diff.diff_count,
            diff.max_diff,
            diff.total_abs_diff,
            diff.first_diff,
            diff.size_delta,
            diff.width_delta,
            diff.height_delta,
        ),
    ))
}

fn compare_metrics_only_row(font: &Font, row: &MatrixRow) -> Result<(), (FailureStage, String)> {
    let actual = rust_slot_metrics(font, row)?;
    let expected = row.ref_value.as_ref().unwrap_or(&row.metrics);
    if &actual == expected {
        Ok(())
    } else {
        Err((
            FailureStage::Metrics,
            format!(
                "{} stage={} actual={} expected={}",
                row.id,
                FailureStage::Metrics.label(),
                actual,
                expected
            ),
        ))
    }
}

fn compare_outline_cbox_row(font: &Font, row: &MatrixRow) -> Result<(), (FailureStage, String)> {
    let glyph = font.data.cmap.char_index(row.codepoint).unwrap_or(0);
    let scaled = scaler::scale_glyph(&font.data, glyph, None, font.is_italic).map_err(|err| {
        (
            FailureStage::LoadError,
            format!(
                "{} stage={} error={}",
                row.id,
                FailureStage::LoadError.label(),
                err
            ),
        )
    })?;
    let actual = serde_json::json!({
        "outline_cbox_26_6": {
            "x_min": scaled.outline_cbox_x_min,
            "y_min": scaled.outline_cbox_y_min,
            "x_max": scaled.outline_cbox_x_max,
            "y_max": scaled.outline_cbox_y_max,
        },
        "outline_bbox_26_6": {
            "x_min": scaled.outline_bbox_x_min,
            "y_min": scaled.outline_bbox_y_min,
            "x_max": scaled.outline_bbox_x_max,
            "y_max": scaled.outline_bbox_y_max,
        },
        "bitmap_pixels": {
            "x_min": scaled.bbox_x_min,
            "y_min": scaled.bbox_y_min,
            "x_max": scaled.bbox_x_max,
            "y_max": scaled.bbox_y_max,
        },
    });
    let expected = row.ref_value.as_ref().unwrap_or(&row.bbox);
    if &actual == expected {
        Ok(())
    } else {
        Err((
            FailureStage::Bbox,
            format!(
                "{} stage={} actual={} expected={}",
                row.id,
                FailureStage::Bbox.label(),
                actual,
                expected
            ),
        ))
    }
}

fn rust_slot_metrics(
    font: &Font,
    row: &MatrixRow,
) -> Result<serde_json::Value, (FailureStage, String)> {
    let metrics = font.glyph_metrics(row.codepoint).map_err(|err| {
        (
            FailureStage::LoadError,
            format!(
                "{} stage={} error={}",
                row.id,
                FailureStage::LoadError.label(),
                err
            ),
        )
    })?;
    Ok(serde_json::json!({
        "width": metrics.width,
        "height": metrics.height,
        "hori_bearing_x": metrics.hori_bearing_x,
        "hori_bearing_y": metrics.hori_bearing_y,
        "hori_advance": metrics.hori_advance,
        "vert_bearing_x": metrics.vert_bearing_x,
        "vert_bearing_y": metrics.vert_bearing_y,
        "vert_advance": metrics.vert_advance,
    }))
}

fn u32_from_usize_for_test(value: usize) -> u32 {
    u32::try_from(value).unwrap()
}

fn i32_from_usize_for_test(value: usize) -> i32 {
    i32::try_from(value).unwrap()
}

#[test]
#[ignore = "deprecated: replaced by manifest-driven unified public API parity test"]
fn stage_failure_classification_covers_native_tt_pipeline() {
    let stages = [
        FailureStage::GlyphIndex,
        FailureStage::LoadError,
        FailureStage::RawOutline,
        FailureStage::ScaledOutline,
        FailureStage::HintedOutline,
        FailureStage::Metrics,
        FailureStage::Bbox,
        FailureStage::BitmapPlacement,
        FailureStage::PixelCoverage,
    ];
    assert_eq!(stages.len(), 9);

    let placement = PixelDiff {
        diff_count: 1,
        max_diff: 0,
        total_abs_diff: 0,
        first_diff: None,
        size_delta: 1,
        width_delta: 0,
        height_delta: 0,
    };
    assert_eq!(
        classify_pixel_failure(&placement),
        FailureStage::BitmapPlacement
    );

    let coverage = PixelDiff {
        size_delta: 0,
        width_delta: 0,
        height_delta: 0,
        ..placement
    };
    assert_eq!(
        classify_pixel_failure(&coverage),
        FailureStage::PixelCoverage
    );
}
