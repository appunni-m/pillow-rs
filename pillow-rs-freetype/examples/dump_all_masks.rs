/// Dump multiple glyph masks for comparison with PIL.
/// Usage: cargo run --example dump_all_masks -- <font_file> <size_pt>
/// Outputs NDJSON with one JSON object per glyph.

use pillow_rs_freetype::font::Font;
use sha2::{Digest, Sha256};
use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: dump_all_masks <font_file> <size_pt>");
        process::exit(1);
    }

    let font_path = &args[1];
    let size_pt: f32 = args[2].parse().expect("invalid size_pt");
    let font_data = std::fs::read(font_path).unwrap();
    let font = Font::truetype(&font_data, size_pt).unwrap();

    // Test a range of glyphs: 33 ('!') through 126 ('~')
    for cp in 33u32..=126 {
        let ch = char::from_u32(cp).unwrap();
        let text = ch.to_string();

        let mask = match font.getmask(&text) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let bbox = font.getbbox(&text);

        let sha = {
            let mut h = Sha256::new();
            h.update(&mask.pixels);
            h.finalize().iter().map(|b| format!("{:02x}", b)).collect::<String>()
        };

        let output = serde_json::json!({
            "codepoint": cp,
            "char": ch.to_string(),
            "width": mask.width,
            "height": mask.height,
            "bbox": [bbox.0, bbox.1, bbox.2, bbox.3],
            "pixels_hex": mask.pixels.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>(),
            "sha256": sha,
            "nonzero": mask.pixels.iter().filter(|&&b| b > 0).count(),
        });

        println!("{}", serde_json::to_string(&output).unwrap());
    }
}
