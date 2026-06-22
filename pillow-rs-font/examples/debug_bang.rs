use std::path::Path;
use pillow_rs_font::Font;

fn main() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let font_dir = manifest_dir
        .join("tests")
        .join("fixtures")
        .join("input")
        .join("fonts");

    // Debug '!' at 12pt
    let path = font_dir.join("DejaVuSans.ttf");
    let data = std::fs::read(&path).unwrap();
    let font = Font::truetype(&data, 12.0).unwrap();

    // We need to peek at the internal ScaledGlyph. Let's get it via public API.
    // Actually, let me just check the Font internals. We don't have direct access
    // to ScaledGlyph but we can look at what the rasterizer receives.

    // Let me add a debug path: get the glyph outline
    // Font has getmask which calls scaler → rasterizer
    // The internals aren't public...

    // Let me use a different approach: recreate the rasterization with known issues
    // Get mask and bbox for '!'
    let mask = font.getmask("!").unwrap();
    let bbox = font.getbbox("!");

    println!("'!' bbox: {:?}", bbox);
    println!("'!' mask: {}x{} with {} non-zero pixels",
        mask.width, mask.height,
        mask.pixels.iter().filter(|&&b| b > 0).count()
    );

    // Now let's check the actual point coordinates by looking at the scaler
    // We can't access the internal ScaledGlyph directly...
    // Let me print what we can from the public API

    // Check if any mask pixels are non-zero
    if mask.width > 0 && mask.height > 0 {
        println!("Mask content (hex):");
        for row in 0..mask.height {
            print!("  ");
            for col in 0..mask.width {
                let idx = (row as usize) * (mask.width as usize) + (col as usize);
                print!("{:02x} ", mask.pixels[idx]);
            }
            println!();
        }
    }
}
