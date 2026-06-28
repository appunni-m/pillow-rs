/// Trace hint_edges for a specific glyph
/// Usage: cargo run --example trace_hint_edges -- <font.ttf> <size_pt> <char>

use pillow_rs_freetype::{BitmapBackend, font::Font};
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: {} <font.ttf> <size_pt> <char>", args[0]);
        std::process::exit(1);
    }

    let font_path = &args[1];
    let size_pt: f32 = args[2].parse().expect("invalid size_pt");
    let ch = args[3].chars().next().expect("char expected");

    let font_data = std::fs::read(font_path).unwrap();
    let font = Font::truetype(&font_data, size_pt, BitmapBackend::FreeType).unwrap();

    let text = ch.to_string();
    eprintln!("[RUST DRIVER] Loading glyph '{}' at {}pt", ch, size_pt);

    let mask = font.getmask(&text).unwrap();
    eprintln!("[RUST DRIVER] Bitmap: {}x{} xmin={} ymin={} advance={}",
              mask.width, mask.height, mask.xmin, mask.ymin, mask.advance_width);
}
