//! Render one glyph via Font::getmask and print the pixel grid.
use pillow_rs_freetype::Font;

fn main() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let args: Vec<String> = std::env::args().collect();
    let font = args.get(1).cloned().unwrap_or("DejaVuSans".into());
    let size: f32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10.0);
    let ch = args.get(3).cloned().unwrap_or("|".into());
    let path = format!("{}/tests/fixtures/input/fonts_nohint/{}.ttf", dir,
        if font == "LiberationSerif" { "LiberationSerif-Regular" } else { &font });
    let data = std::fs::read(&path).unwrap();
    let f = Font::truetype(&data, size).unwrap();
    let m = f.getmask(&ch).unwrap();
    println!("Rust {} '{}' {}x{}:", font, ch, m.width, m.height);
    for r in 0..m.height {
        let off = (r * m.width) as usize;
        let row = &m.pixels[off..off + m.width as usize];
        println!(" {}", row.iter().map(|v| format!("{:3}", v)).collect::<Vec<_>>().join(" "));
    }
    // sha
    use sha2::{Sha256, Digest};
    let sha = Sha256::digest(&m.pixels).iter().map(|b| format!("{:02x}", b)).collect::<String>();
    println!("sha256: {}", sha);
}
