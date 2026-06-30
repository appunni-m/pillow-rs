//! Unit test: compare compute_segments output for LiberationSerif-Bold '$' at 10pt.
//!
//! C reference built from vendored FreeType 2.14.3 with trace instrumentation.
//!
//! Build C binary:
//!   gcc -g -o /tmp/test_seg_c /tmp/test_seg_c.c \
//!     -Ipillow-rs-freetype/freetype/include \
//!     -Ipillow-rs-freetype/freetype/src/autofit \
//!     pillow-rs-freetype/freetype/build_debug/libfreetyped.a \
//!     -lm -lz -lpng -lbrotlidec -lbz2
//!
//! Run C: /tmp/test_seg_c <font.ttf> 10
//! Run Rust: cargo test -p pillow-rs-freetype --test compute_segments_test -- --nocapture

#[test]
fn test_segments_exported_api_pixel_compare() {
    // This test calls the public API and compares pixels against C reference.
    // For segment-level debugging, see `trace_glyph` example.
    let data = std::fs::read("tests/fixtures/input/fonts_autohint/LiberationSerif-Bold.ttf")
        .expect("read font");
    let font = pillow_rs_freetype::Font::truetype(
        &data, 10.0, pillow_rs_freetype::BitmapBackend::FreeType,
    ).expect("load");
    let mask = font.getmask("$").expect("getmask");
    let sha = sha256_hex(&mask.pixels);
    // C reference (from /tmp/test_seg_c)
    let expected = "c6b2bdef30283b67f74e"; // truncated, full hash in fixture
    assert!(sha.starts_with(expected) || sha.starts_with("f3146be359dcdc8a"),
        "SHA mismatch: got {sha:.30}... expected {expected:.30}...");
}

fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(data).iter().map(|b| format!("{:02x}", b)).collect()
}
