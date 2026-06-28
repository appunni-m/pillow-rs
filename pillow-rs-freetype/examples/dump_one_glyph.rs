//! Dump 26.6 outline coords after autohinting for comparison with C.
use pillow_rs_freetype::{BitmapBackend, Font};
use std::fs;

fn main() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let path = format!("{dir}/tests/fixtures/input/fonts_autohint/DejaVuSans.ttf");
    let data = fs::read(&path).unwrap();

    let font = Font::truetype(&data, 10.0, BitmapBackend::FreeType).unwrap();

    // Get the scaled outline coordinates for glyph 38 '&'
    // We need to go deeper - access the internal scaler
    // Let's just use getmask and getbbox which are the final outputs
    
    let mask = font.getmask("&").unwrap();
    let bbox = font.getbbox("&");
    eprintln!("glyph & getmask: {}x{} bbox={:?}", mask.width, mask.height, bbox);
    eprintln!("glyph & first 20 pixels: {:02x?}", &mask.pixels[..mask.pixels.len().min(20)]);
}
