use pillow_rs_freetype::Font;
use std::io::Write;

fn main() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let font_path = format!("{}/tests/fixtures/input/fonts_nohint/DejaVuSans.ttf", dir);
    let data = std::fs::read(&font_path).unwrap();
    let font = Font::truetype(&data, 10.0).unwrap();

    // Test several visually distinct glyphs: '!', 'g', 'A', '|', '.'
    let test_chars = ['!', 'g', 'A', '|', '.', 'H', 'o'];
    let mut stdout = std::io::stdout();

    for ch in &test_chars {
        let mask = font.getmask(&ch.to_string()).unwrap();
        let bbox = font.getbbox(&ch.to_string());
        writeln!(stdout, "══════ U+{:04X} '{}' ══════", *ch as u32, ch).ok();
        writeln!(stdout, "  bbox: {:?}  mask: {}×{}, {} nonzero", bbox, mask.width, mask.height,
                 mask.pixels.iter().filter(|&&b| b > 0).count()).ok();

        // Print the mask as ASCII art + pixel values
        for y in 0..mask.height {
            let off = (y * mask.width) as usize;
            let row = &mask.pixels[off..off + mask.width as usize];
            // ASCII art: ██ = dense, ░░ = light, ·· = trace, '  ' = blank
            let art: String = row.iter().map(|&b| {
                if b >= 200 { "██" } else if b >= 128 { "▓▓" } else if b >= 64 { "▒▒" }
                else if b >= 16 { "░░" } else if b >= 4 { "··" } else { "  " }
            }).collect();
            let vals: String = row.iter().map(|b| format!("{:3}", b)).collect();
            writeln!(stdout, "  {}  {}", vals, art).ok();
        }
        writeln!(stdout, "").ok();
    }
}
