#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::arithmetic_side_effects)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::unnecessary_cast)]
#![allow(missing_docs)]
#![allow(unused_crate_dependencies)]

use pillow_rs_freetype::{Font, PixelMode, RenderMode};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct Matrix {
    #[allow(dead_code)]
    version: String,
    #[allow(dead_code)]
    generator: String,
    rows: Vec<Row>,
}

#[derive(Debug, Deserialize)]
struct Row {
    id: String,
    font: String,
    size_pt: f32,
    codepoint: u32,
    mode: String,
    pixel_mode: String,
    width: u32,
    rows: u32,
    pitch: i32,
    left: i32,
    top: i32,
    ref_sha256: String,
    ref_raw: String,
}

#[test]
fn render_modes_match_static_fixture_matrix() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = manifest_dir
        .join("tests")
        .join("fixtures")
        .join("render_mode_matrix.json");

    let matrix: Matrix = serde_json::from_str(&fs::read_to_string(&fixture_path).unwrap()).unwrap();
    let mut failures = Vec::new();

    for row in &matrix.rows {
        let font = load_font(manifest_dir, &row.font, row.size_pt);
        let bitmap = font
            .render_char_mode(
                char::from_u32(row.codepoint).unwrap(),
                parse_mode(&row.mode),
            )
            .unwrap();
        let expected_raw = fs::read(
            manifest_dir
                .join("tests")
                .join("fixtures")
                .join(&row.ref_raw),
        )
        .unwrap_or_else(|_| panic!("missing raw fixture {}", row.ref_raw));
        let actual_sha = sha256_hex(&bitmap.buffer);

        if bitmap.width != row.width
            || bitmap.rows != row.rows
            || bitmap.pitch != row.pitch
            || bitmap.left != row.left
            || bitmap.top != row.top
            || bitmap.pixel_mode.fixture_name() != row.pixel_mode
            || actual_sha != row.ref_sha256
            || bitmap.buffer != expected_raw
        {
            failures.push(format!(
                "{} actual mode={} {}x{} pitch={} left={} top={} sha={} len={} expected mode={} {}x{} pitch={} left={} top={} sha={} len={}",
                row.id,
                bitmap.pixel_mode.fixture_name(),
                bitmap.width,
                bitmap.rows,
                bitmap.pitch,
                bitmap.left,
                bitmap.top,
                actual_sha,
                bitmap.buffer.len(),
                row.pixel_mode,
                row.width,
                row.rows,
                row.pitch,
                row.left,
                row.top,
                row.ref_sha256,
                expected_raw.len(),
            ));
        }

        assert_render_mode_layout(row, bitmap.pixel_mode);
    }

    eprintln!(
        "render_mode_matrix: {}/{} passed",
        matrix.rows.len().saturating_sub(failures.len()),
        matrix.rows.len()
    );
    if !failures.is_empty() {
        for failure in failures.iter().take(30) {
            eprintln!("{failure}");
        }
        panic!("{} render mode fixture mismatches", failures.len());
    }
}

fn load_font(manifest_dir: &Path, name: &str, size_pt: f32) -> Font {
    let path = manifest_dir
        .join("tests")
        .join("fixtures")
        .join("input")
        .join("fonts_autohint")
        .join(format!("{name}.ttf"));
    let data = fs::read(&path).unwrap_or_else(|_| panic!("missing font {}", path.display()));
    Font::truetype(&data, size_pt).unwrap()
}

fn parse_mode(mode: &str) -> RenderMode {
    match mode {
        "normal" => RenderMode::Normal,
        "mono" => RenderMode::Mono,
        "lcd" => RenderMode::Lcd,
        "lcd_v" => RenderMode::LcdV,
        _ => panic!("unknown render mode {mode}"),
    }
}

fn assert_render_mode_layout(row: &Row, pixel_mode: PixelMode) {
    match pixel_mode {
        PixelMode::Gray => {
            assert_eq!(row.pitch, row.width as i32, "{} gray pitch", row.id);
        }
        PixelMode::Mono => {
            let expected_pitch = (((row.width as usize + 15) >> 4) << 1) as i32;
            assert_eq!(row.pitch, expected_pitch, "{} mono pitch", row.id);
        }
        PixelMode::Lcd => {
            assert_eq!(row.width % 3, 0, "{} LCD width is subpixel tripled", row.id);
            let expected_pitch = ((row.width as i32 + 3) & !3) as i32;
            assert_eq!(row.pitch, expected_pitch, "{} LCD pitch", row.id);
        }
        PixelMode::LcdV => {
            assert_eq!(
                row.rows % 3,
                0,
                "{} LCD_V rows are subpixel tripled",
                row.id
            );
            assert_eq!(row.pitch, row.width as i32, "{} LCD_V pitch", row.id);
        }
    }
}

fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}
