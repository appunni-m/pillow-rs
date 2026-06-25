use sha2::{Digest, Sha256};
use std::path::Path;

use pillow_rs_font::Font;

fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

fn main() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let font_dir = manifest_dir
        .join("tests")
        .join("fixtures")
        .join("input")
        .join("fonts");

    // 5 failing test cases
    let cases: Vec<(&str, f32, u32, &str)> = vec![
        ("DejaVuSans", 12.0, 33, "!"),
        ("DejaVuSans", 12.0, 65, "A"),
        ("DejaVuSans", 12.0, 103, "g"),
        ("LiberationSerif", 12.0, 65, "A"),
        ("DejaVuSans", 24.0, 65, "A"),
    ];

    for (font_name, size, codepoint, glyph_char) in &cases {
        println!(
            "══════════════════════════════════════════════════════════"
        );
        println!(
            "  {font_name}  {size}pt  U+{cp:04X} '{ch}'",
            font_name = font_name,
            size = size,
            cp = codepoint,
            ch = glyph_char
        );

        let path = font_dir.join(format!("{}.ttf", font_name));
        let data = std::fs::read(&path).unwrap();
        let font = Font::truetype(&data, *size).unwrap();

        // Get bbox
        let bbox = font.getbbox(glyph_char);
        println!("  bbox:    ({}, {}, {}, {})", bbox.0, bbox.1, bbox.2, bbox.3);

        // Get mask
        let mask = font.getmask(glyph_char).unwrap();
        let sha = sha256_hex(&mask.pixels);
        println!(
            "  mask:    {w}x{h}  ({nz} non-zero pixels)",
            w = mask.width,
            h = mask.height,
            nz = mask.pixels.iter().filter(|&&b| b > 0).count()
        );
        println!("  sha256:  {sha}");

        // Print ascii art of the mask
        if mask.width > 0 && mask.height > 0 {
            println!("  ┌{}┐", "─".repeat(mask.width as usize));
            for row in 0..mask.height {
                print!("  │");
                for col in 0..mask.width {
                    let idx = (row as usize) * (mask.width as usize) + (col as usize);
                    let val = mask.pixels[idx];
                    print!(
                        "{}",
                        match val {
                            0 => ' ',
                            1..=63 => '·',
                            64..=127 => '░',
                            128..=191 => '▒',
                            _ => '▓',
                        }
                    );
                }
                println!("│");
            }
            println!("  └{}┘", "─".repeat(mask.width as usize));
        }
        println!();
    }
}
