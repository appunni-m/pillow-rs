//! Trace single glyph through autohinter — dump reload coords, edges, hinted output.
//! Usage: RUST_LOG=autohint::pipeline=trace cargo run --example trace_glyph -- <font.ttf> <size_pt> <char>
#![allow(clippy::expect_used)]
#![allow(unused_crate_dependencies)]

use std::env;
use std::fs;

fn main() {
    env_logger::init();
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: trace_glyph <font.ttf> <size_pt> <char>");
        return;
    }
    let font_path = &args[1];
    let size_pt: f32 = args[2].parse().expect("size_pt");
    let ch = args[3].chars().next().expect("char");

    let data = fs::read(font_path).expect("read font");
    let backend = if args.get(4).is_some_and(|arg| arg == "pil") {
        pillow_rs_freetype::BitmapBackend::PIL
    } else {
        pillow_rs_freetype::BitmapBackend::FreeType
    };
    let font = pillow_rs_freetype::Font::truetype(&data, size_pt, backend).expect("load font");

    let mask = font.getmask(&ch.to_string()).expect("getmask");

    eprintln!(
        "Bitmap: {}x{} left={} top={} advance={}",
        mask.width, mask.height, mask.xmin, mask.ymin, mask.advance_width
    );
    print!("PIXELS:");
    for b in &mask.pixels {
        print!(" {:02x}", b);
    }
    println!();
}
