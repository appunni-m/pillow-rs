//! Emit JSONL timing rows for `pillow-rs-freetype` operations.
//!
//! This example is intentionally dependency-light and deterministic. It is the
//! Rust side of `scripts/bench_freetype.py`; C FreeType comparison is handled
//! by a separate helper so the runtime crate remains pure Rust.

#![allow(clippy::expect_used)]
#![allow(clippy::print_stdout)]
#![allow(clippy::unwrap_used)]
#![allow(unused_crate_dependencies)]

use std::collections::HashMap;
use std::fs;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::Instant;

use pillow_rs_freetype::{Font, LoadMode, RenderMode};
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Deserialize)]
struct Matrix {
    rows: Vec<Row>,
}

#[derive(Debug, Deserialize)]
struct Row {
    id: String,
    operation: String,
    font: String,
    size: f32,
    #[serde(default)]
    text: String,
    #[serde(default)]
    load_mode: String,
    #[serde(default)]
    render_mode: String,
    iterations: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct FontKey {
    font_index: usize,
    size_bits: u32,
    load_mode: u8,
}

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let matrix_path = std::env::args().nth(1).map_or_else(
        || manifest_dir.join("tests/fixtures/perf_operation_matrix.json"),
        PathBuf::from,
    );
    let matrix = read_matrix(&matrix_path);
    let mut font_bytes = Vec::new();
    let mut font_index = HashMap::<String, usize>::new();
    let mut fonts = HashMap::<FontKey, Font>::new();

    for row in &matrix.rows {
        let font_id = *font_index.entry(row.font.clone()).or_insert_with(|| {
            let path = manifest_dir.join(&row.font);
            let bytes = fs::read(&path)
                .unwrap_or_else(|err| panic!("failed to read font {}: {err}", path.display()));
            let index = font_bytes.len();
            font_bytes.push(bytes);
            index
        });
        let mode = parse_load_mode(&row.load_mode);
        let key = FontKey {
            font_index: font_id,
            size_bits: row.size.to_bits(),
            load_mode: load_mode_key(mode),
        };
        if row.operation != "load_font" && !fonts.contains_key(&key) {
            let font = Font::truetype_with_load_mode(&font_bytes[font_id], row.size, mode)
                .unwrap_or_else(|err| panic!("failed to load font for {}: {err}", row.id));
            fonts.insert(key, font);
        }

        let output = run_row(row, font_bytes[font_id].as_slice(), fonts.get(&key));
        println!("{}", output.to_json(row));
    }
}

fn read_matrix(path: &Path) -> Matrix {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read matrix {}: {err}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|err| panic!("invalid matrix JSON: {err}"))
}

fn parse_load_mode(value: &str) -> LoadMode {
    match value {
        "force_autohint" | "force-autohint" => LoadMode::ForceAutoHint,
        _ => LoadMode::Default,
    }
}

fn load_mode_key(mode: LoadMode) -> u8 {
    match mode {
        LoadMode::Default => 0,
        LoadMode::ForceAutoHint => 1,
    }
}

fn parse_render_mode(value: &str) -> RenderMode {
    match value {
        "mono" => RenderMode::Mono,
        "lcd" => RenderMode::Lcd,
        "lcd_v" | "lcd-v" => RenderMode::LcdV,
        _ => RenderMode::Normal,
    }
}

#[derive(Debug)]
struct BenchOutput {
    elapsed_ns: u128,
    iterations: u64,
    output_len: usize,
    output_sha256: String,
}

impl BenchOutput {
    fn to_json(&self, row: &Row) -> String {
        format!(
            "{{\"id\":\"{}\",\"operation\":\"{}\",\"font\":\"{}\",\"size\":{},\"text\":\"{}\",\"load_mode\":\"{}\",\"render_mode\":\"{}\",\"iterations\":{},\"rust_ns_total\":{},\"rust_ns_per_iter\":{},\"output_len\":{},\"output_sha256\":\"{}\"}}",
            json_escape(&row.id),
            json_escape(&row.operation),
            json_escape(&row.font),
            row.size,
            json_escape(&row.text),
            json_escape(if row.load_mode.is_empty() { "default" } else { &row.load_mode }),
            json_escape(if row.render_mode.is_empty() { "normal" } else { &row.render_mode }),
            self.iterations,
            self.elapsed_ns,
            self.elapsed_ns
                .checked_div(u128::from(self.iterations.max(1)))
                .unwrap_or(0),
            self.output_len,
            self.output_sha256
        )
    }
}

fn run_row(row: &Row, font_bytes: &[u8], font: Option<&Font>) -> BenchOutput {
    let mut last = Vec::new();
    if row.operation != "load_font" {
        run_once_into(row, font_bytes, font, &mut last);
        black_box(&last);
    }
    let start = Instant::now();
    for _ in 0..row.iterations {
        run_once_into(row, font_bytes, font, &mut last);
        black_box(&last);
    }
    BenchOutput {
        elapsed_ns: start.elapsed().as_nanos(),
        iterations: row.iterations,
        output_len: last.len(),
        output_sha256: sha256_hex(&last),
    }
}

fn run_once_into(row: &Row, font_bytes: &[u8], font: Option<&Font>, out: &mut Vec<u8>) {
    out.clear();
    match row.operation.as_str() {
        "load_font" => {
            let font = Font::truetype_with_load_mode(
                font_bytes,
                row.size,
                parse_load_mode(&row.load_mode),
            )
            .unwrap_or_else(|err| panic!("load_font failed for {}: {err}", row.id));
            let info = font.face_info();
            out.extend_from_slice(&info.num_glyphs.to_le_bytes());
        }
        "getname" => {
            let (family, style) = font.expect("font is cached").getname();
            out.extend_from_slice(family.as_bytes());
            out.extend_from_slice(style.as_bytes());
        }
        "getmetrics" => {
            let (ascent, descent) = font.expect("font is cached").getmetrics();
            out.extend_from_slice(&ascent.to_le_bytes());
            out.extend_from_slice(&descent.to_le_bytes());
        }
        "getlength" => {
            out.extend_from_slice(
                &font
                    .expect("font is cached")
                    .getlength(&row.text)
                    .to_le_bytes(),
            );
        }
        "getbbox" => {
            let bbox = font.expect("font is cached").getbbox(&row.text);
            pack_i32s_into(out, [bbox.0, bbox.1, bbox.2, bbox.3]);
        }
        "getmask" => {
            let mask = font
                .expect("font is cached")
                .getmask(&row.text)
                .unwrap_or_else(|err| panic!("getmask failed for {}: {err}", row.id));
            pack_mask_into(
                out,
                mask.width,
                mask.height,
                mask.xmin,
                mask.ymin,
                &mask.pixels,
            );
        }
        "glyph_metrics" => {
            let ch = first_char(&row.text);
            let metrics = font
                .expect("font is cached")
                .glyph_metrics(ch as u32)
                .unwrap_or_else(|err| panic!("glyph_metrics failed for {}: {err}", row.id));
            pack_i32s_into(
                out,
                [
                    metrics.width,
                    metrics.height,
                    metrics.hori_bearing_x,
                    metrics.hori_bearing_y,
                    metrics.hori_advance,
                    metrics.vert_bearing_x,
                    metrics.vert_bearing_y,
                    metrics.vert_advance,
                ],
            );
        }
        "render_mode" => {
            let bitmap = font
                .expect("font is cached")
                .render_mode(&row.text, parse_render_mode(&row.render_mode))
                .unwrap_or_else(|err| panic!("render_mode failed for {}: {err}", row.id));
            pack_mask_into(
                out,
                bitmap.width,
                bitmap.rows,
                bitmap.left,
                bitmap.top,
                &bitmap.buffer,
            );
        }
        other => panic!("unsupported benchmark operation {other}"),
    }
}

fn first_char(text: &str) -> char {
    text.chars().next().unwrap_or('\0')
}

fn pack_i32s_into<const N: usize>(out: &mut Vec<u8>, values: [i32; N]) {
    out.reserve(N.checked_mul(4).unwrap_or(0));
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
}

fn pack_mask_into(out: &mut Vec<u8>, width: u32, height: u32, left: i32, top: i32, pixels: &[u8]) {
    out.reserve(16usize.checked_add(pixels.len()).unwrap_or(0));
    out.extend_from_slice(&width.to_le_bytes());
    out.extend_from_slice(&height.to_le_bytes());
    out.extend_from_slice(&left.to_le_bytes());
    out.extend_from_slice(&top.to_le_bytes());
    out.extend_from_slice(pixels);
}

fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped
}
