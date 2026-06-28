fn main() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let p = format!("{dir}/tests/fixtures/input/fonts_autohint/DejaVuSans.ttf");
    let d = std::fs::read(&p).unwrap();
    let f = pillow_rs_freetype::Font::truetype(&d, 10.0, pillow_rs_freetype::BitmapBackend::FreeType).unwrap();
    f.getmask("&").unwrap();
}
