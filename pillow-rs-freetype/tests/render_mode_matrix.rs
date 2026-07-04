#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(missing_docs)]
#![allow(unused_crate_dependencies)]

use pillow_rs_freetype::{BitmapBackend, Font, PixelMode, RenderMode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
struct Matrix {
    version: String,
    generator: String,
    rows: Vec<Row>,
}

#[derive(Debug, Serialize, Deserialize)]
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

    if std::env::var_os("PILLOW_RS_UPDATE_RENDER_FIXTURES").is_some() {
        write_fixture(manifest_dir, &fixture_path);
        return;
    }

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

fn write_fixture(manifest_dir: &Path, fixture_path: &Path) {
    let raw_dir = manifest_dir
        .join("tests")
        .join("fixtures")
        .join("outputs")
        .join("render_modes");
    fs::create_dir_all(&raw_dir).unwrap();

    let cases = [
        ("DejaVuSans", 20.0, 'A'),
        ("DejaVuSans", 20.0, 'g'),
        ("LiberationSans-Regular", 16.0, 'Q'),
        ("NotoSans-Bold", 24.0, '8'),
    ];
    let modes = [
        RenderMode::Normal,
        RenderMode::Mono,
        RenderMode::Lcd,
        RenderMode::LcdV,
    ];

    let mut rows = Vec::new();
    for (font_name, size_pt, ch) in cases {
        let font = load_font(manifest_dir, font_name, size_pt);
        for mode in modes {
            let bitmap = font.render_char_mode(ch, mode).unwrap();
            let id = format!(
                "{}_{}_{}_render_{}",
                font_name,
                size_pt as u32,
                ch as u32,
                mode.fixture_name()
            );
            let raw_name = format!("{id}.bin");
            fs::write(raw_dir.join(&raw_name), &bitmap.buffer).unwrap();
            rows.push(Row {
                id,
                font: font_name.to_string(),
                size_pt,
                codepoint: ch as u32,
                mode: mode.fixture_name().to_string(),
                pixel_mode: bitmap.pixel_mode.fixture_name().to_string(),
                width: bitmap.width,
                rows: bitmap.rows,
                pitch: bitmap.pitch,
                left: bitmap.left,
                top: bitmap.top,
                ref_sha256: sha256_hex(&bitmap.buffer),
                ref_raw: format!("outputs/render_modes/{raw_name}"),
            });
        }
    }

    let matrix = Matrix {
        version: "1.0.0".to_string(),
        generator:
            "pillow-rs-freetype render_mode_matrix.rs; FreeType render mode metadata parity fixture"
                .to_string(),
        rows,
    };
    fs::write(fixture_path, serde_json::to_string_pretty(&matrix).unwrap()).unwrap();
}

fn load_font(manifest_dir: &Path, name: &str, size_pt: f32) -> Font {
    let path = manifest_dir
        .join("tests")
        .join("fixtures")
        .join("input")
        .join("fonts_autohint")
        .join(format!("{name}.ttf"));
    let data = fs::read(&path).unwrap_or_else(|_| panic!("missing font {}", path.display()));
    Font::truetype(&data, size_pt, BitmapBackend::FreeType).unwrap()
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

#[allow(dead_code)]
fn _fixture_path(_: PathBuf) {}
