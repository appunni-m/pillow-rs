use std::path::Path;
use pillow_rs_font::Font;

fn main() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let font_path = manifest_dir
        .join("tests").join("fixtures").join("input").join("fonts_nohint")
        .join("DejaVuSans.ttf");
    let data = std::fs::read(&font_path).unwrap();

    // Render '!' at 12pt — simple 2-contour glyph
    let font = Font::truetype(&data, 12.0).unwrap();
    let mask = font.getmask("!").unwrap();
    let bbox = font.getbbox("!");

    println!("'!' bbox: {:?}", bbox);
    println!("mask: {}x{}", mask.width, mask.height);
    println!();

    // Print hex values in a grid for pixel-by-pixel comparison
    print!("    ");
    for x in 0..mask.width { print!(" {:2x} ", x); }
    println!();

    for y in 0..mask.height {
        print!("{:2x}: ", y);
        for x in 0..mask.width {
            let idx = (y * mask.width + x) as usize;
            let v = mask.pixels[idx];
            if v == 0 {
                print!(" .. ");
            } else {
                print!(" {:02x} ", v);
            }
        }
        println!();
    }
    println!();

    // Also print as human-readable coverage (0-9 scale)
    println!("Coverage (0-9):");
    for y in 0..mask.height {
        for x in 0..mask.width {
            let idx = (y * mask.width + x) as usize;
            let v = mask.pixels[idx];
            if v == 0 {
                print!(".");
            } else {
                print!("{}", (v as u32 * 10 / 255).min(9));
            }
        }
        println!();
    }
}
