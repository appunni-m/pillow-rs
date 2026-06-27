/// Dump single-glyph mask pixels as hex for comparison with freetype-py reference.
/// Usage: cargo run --example dump_mask_compare -- <font_file> <size_pt> <codepoint>
/// Output: JSON with mask dimensions, pixel hex bytes, and sha256.

use pillow_rs_freetype::font::Font;
use sha2::{Digest, Sha256};
use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: dump_mask_compare <font_file> <size_pt> <codepoint>");
        process::exit(1);
    }

    let font_path = &args[1];
    let size_pt: f32 = args[2].parse().expect("invalid size_pt");
    let codepoint: u32 = args[3].parse().expect("invalid codepoint");
    let ch = char::from_u32(codepoint).unwrap_or('?');

    let font_data = std::fs::read(font_path).unwrap();
    let font = Font::truetype(&font_data, size_pt, Default::default()).unwrap();

    let text = ch.to_string();
    let mask = font.getmask(&text).unwrap();
    let bbox = font.getbbox(&text);

    let sha = {
        let mut h = Sha256::new();
        h.update(&mask.pixels);
        h.finalize().iter().map(|b| format!("{:02x}", b)).collect::<String>()
    };

    let pixels_hex: Vec<String> = mask.pixels.iter().map(|b| format!("{:02x}", b)).collect();

    // Output JSON for easy parsing
    let output = serde_json::json!({
        "char": ch.to_string(),
        "codepoint": codepoint,
        "width": mask.width,
        "height": mask.height,
        "bbox": [bbox.0, bbox.1, bbox.2, bbox.3],
        "pixels_hex": pixels_hex,
        "pixels_len": mask.pixels.len(),
        "sha256": sha,
        "nonzero_pixels": mask.pixels.iter().filter(|&&b| b > 0).count(),
    });

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}
