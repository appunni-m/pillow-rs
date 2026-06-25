use std::path::Path;
use pillow_rs_font::Font;

fn main() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests").join("fixtures").join("input").join("fonts")
        .join("DejaVuSans.ttf");
    let data = std::fs::read(&path).unwrap();

    for size in [10.0, 12.0, 14.0, 18.0, 24.0, 36.0, 48.0, 72.0] {
        let font = Font::truetype(&data, size).unwrap();
        let m = font.getmask("a").unwrap();
        eprintln!("  {}px: {}x{}", size as u32, m.width, m.height);
    }
}
