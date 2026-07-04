#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::arithmetic_side_effects)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(missing_docs)]
#![allow(unused_crate_dependencies)]

use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use env_logger as _;
use log as _;
use pillow_rs_freetype as _;
use sha2 as _;
use thiserror as _;

#[derive(Debug, Deserialize)]
struct CoverageMatrix {
    rows: Vec<CoverageRow>,
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
struct CoverageRow {
    id: String,
    operation: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    ref_raw: Option<String>,
    #[serde(default)]
    ref_size: Option<Vec<u32>>,
}

#[derive(Debug, Deserialize)]
struct RenderModeMatrix {
    rows: Vec<RenderModeRow>,
}

#[derive(Debug, Deserialize)]
struct RenderModeRow {
    id: String,
    mode: String,
    pixel_mode: String,
    width: u32,
    rows: u32,
    pitch: i32,
    ref_sha256: String,
    ref_raw: String,
}

fn default_assert_pixel_parity() -> bool {
    true
}

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn read_coverage_matrix(name: &str) -> CoverageMatrix {
    let path = fixture_dir().join(name);
    assert!(path.exists(), "required matrix missing: {}", path.display());
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn read_render_mode_matrix() -> RenderModeMatrix {
    let path = fixture_dir().join("render_mode_matrix.json");
    assert!(path.exists(), "required matrix missing: {}", path.display());
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn operation_counts(rows: &[CoverageRow]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for row in rows {
        if row.status == "skip" {
            continue;
        }
        *counts.entry(row.operation.clone()).or_insert(0) += 1;
    }
    counts
}

fn render_mode_counts(rows: &[RenderModeRow]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for row in rows {
        *counts.entry(row.mode.clone()).or_insert(0) += 1;
    }
    counts
}

fn raw_pixel_paths(row: &CoverageRow) -> Vec<PathBuf> {
    let fixture_dir = fixture_dir();
    let raw_dir = fixture_dir.join("outputs").join("raws");
    let mut paths = Vec::new();

    if let Some(ref_raw) = &row.ref_raw {
        paths.push(fixture_dir.join(ref_raw));
    }
    paths.push(raw_dir.join(format!("{}.bin", row.id)));

    paths
}

fn assert_coverage_header(matrix_name: &str, matrix: &CoverageMatrix) {
    assert!(
        !matrix.fixture_family.is_empty(),
        "{matrix_name} is missing fixture_family"
    );
    assert!(
        !matrix.generator.is_empty(),
        "{matrix_name} is missing generator"
    );
    assert!(
        !matrix.load_flags.is_empty(),
        "{matrix_name} is missing load_flags"
    );
    assert!(
        !matrix.render_mode.is_empty(),
        "{matrix_name} is missing render_mode"
    );
}

#[test]
fn exact_parity_matrices_are_broad_and_byte_backed() {
    let matrix = read_coverage_matrix("force_autohint_matrix.json");
    assert_coverage_header("force_autohint_matrix.json", &matrix);
    assert!(
        matrix.assert_pixel_parity,
        "force_autohint_matrix.json must remain an exact parity gate"
    );
    assert_eq!(
        matrix.rows.len(),
        22_168,
        "force_autohint_matrix.json coverage changed; only update with an intentional C-oracle refresh"
    );

    let counts = operation_counts(&matrix.rows);
    assert_eq!(counts.get("getmask"), Some(&11_084));
    assert_eq!(counts.get("getbbox"), Some(&11_084));

    for row in matrix.rows.iter().filter(|row| row.operation == "getmask") {
        assert!(
            row.ref_size.as_ref().is_some_and(|size| size.len() >= 2),
            "{} is missing exact bitmap dimensions",
            row.id
        );
        assert!(
            raw_pixel_paths(row).iter().any(|path| path.exists()),
            "{} is missing exact raw byte fixture",
            row.id
        );
    }
}

#[test]
fn render_mode_matrix_is_static_c_oracle_data() {
    let matrix = read_render_mode_matrix();
    assert_eq!(
        matrix.rows.len(),
        16,
        "render_mode_matrix.json coverage changed; refresh from the C oracle intentionally"
    );

    let counts = render_mode_counts(&matrix.rows);
    assert_eq!(counts.get("normal"), Some(&4));
    assert_eq!(counts.get("mono"), Some(&4));
    assert_eq!(counts.get("lcd"), Some(&4));
    assert_eq!(counts.get("lcd_v"), Some(&4));

    for row in &matrix.rows {
        assert!(
            ["gray", "mono", "lcd", "lcd_v"].contains(&row.pixel_mode.as_str()),
            "{} has invalid pixel mode {}",
            row.id,
            row.pixel_mode
        );
        assert!(row.width > 0, "{} has empty bitmap width", row.id);
        assert!(row.rows > 0, "{} has empty bitmap rows", row.id);
        assert_ne!(row.pitch, 0, "{} has zero pitch", row.id);
        assert_eq!(
            row.ref_sha256.len(),
            64,
            "{} has invalid SHA-256 fixture",
            row.id
        );
        assert!(
            fixture_dir().join(&row.ref_raw).exists(),
            "{} is missing raw render-mode bytes",
            row.id
        );
    }
}

#[test]
fn incomplete_threshold_matrices_cannot_pose_as_parity_gates() {
    let matrix = read_coverage_matrix("native_tt_default_matrix.json");
    assert_coverage_header("native_tt_default_matrix.json", &matrix);
    assert!(
        !matrix.assert_pixel_parity,
        "native_tt_default_matrix.json must stay marked incomplete until it is exact"
    );
    assert_eq!(
        matrix.rows.len(),
        7_640,
        "native_tt_default_matrix.json coverage changed; update the threshold baseline intentionally"
    );

    let counts = operation_counts(&matrix.rows);
    assert_eq!(counts.get("getmask"), Some(&3_760));
    assert_eq!(counts.get("getbbox"), Some(&3_760));
    assert_eq!(counts.get("getmetrics"), Some(&40));
    assert_eq!(counts.get("getname"), Some(&40));
    assert_eq!(counts.get("getlength"), Some(&40));

    for name in ["render_mono_matrix.json", "render_lcd_matrix.json"] {
        let matrix = read_coverage_matrix(name);
        assert_coverage_header(name, &matrix);
        assert!(
            !matrix.assert_pixel_parity,
            "{name} must stay marked incomplete until its executed baseline is exact"
        );
        assert_eq!(
            matrix.rows.len(),
            8,
            "{name} coverage changed; update the executed baseline intentionally"
        );

        let counts = operation_counts(&matrix.rows);
        assert_eq!(counts.get("getmask"), Some(&8));
        for row in matrix.rows.iter().filter(|row| row.operation == "getmask") {
            assert!(
                row.ref_size.as_ref().is_some_and(|size| size.len() >= 2),
                "{} is missing bitmap dimensions",
                row.id
            );
            assert!(
                raw_pixel_paths(row).iter().any(|path| path.exists()),
                "{} is missing raw byte fixture",
                row.id
            );
        }
    }

    for (name, operation) in [
        ("metrics_only_matrix.json", "metrics_only"),
        ("outline_cbox_matrix.json", "outline_cbox"),
    ] {
        let matrix = read_coverage_matrix(name);
        assert_coverage_header(name, &matrix);
        assert!(
            !matrix.assert_pixel_parity,
            "{name} must stay marked incomplete until its executed baseline is exact"
        );
        assert_eq!(
            matrix.rows.len(),
            8,
            "{name} coverage changed; update the executed baseline intentionally"
        );

        let counts = operation_counts(&matrix.rows);
        assert_eq!(counts.get(operation), Some(&8));
    }
}

#[test]
fn all_committed_supplemental_matrices_have_executed_status() {
    let no_hinting = read_coverage_matrix("no_hinting_matrix.json");
    assert_coverage_header("no_hinting_matrix.json", &no_hinting);
    assert!(
        !no_hinting.assert_pixel_parity,
        "no_hinting_matrix.json is an executed 8/8 baseline but not yet a broad exact parity gate"
    );
    assert_eq!(
        no_hinting.rows.len(),
        8,
        "no_hinting_matrix.json coverage changed; update the executed baseline intentionally"
    );

    let counts = operation_counts(&no_hinting.rows);
    assert_eq!(counts.get("getmask"), Some(&8));
    for row in no_hinting
        .rows
        .iter()
        .filter(|row| row.operation == "getmask")
    {
        assert!(
            row.ref_size.as_ref().is_some_and(|size| size.len() >= 2),
            "{} is missing bitmap dimensions",
            row.id
        );
        assert!(
            raw_pixel_paths(row).iter().any(|path| path.exists()),
            "{} is missing raw byte fixture",
            row.id
        );
    }
}
