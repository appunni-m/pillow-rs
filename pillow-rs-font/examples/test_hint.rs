use sha2::{Digest, Sha256};
use pillow_rs_font::Font;
use std::path::Path;

fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

fn main() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let font_path = manifest_dir.join("tests/fixtures/input/fonts/DejaVuSans.ttf");
    let font_data = std::fs::read(&font_path).unwrap();

    // With hinting
    let font = Font::truetype(&font_data, 10.0).unwrap();
    let mask_hinted = font.getmask("!").unwrap();
    let bbox_hinted = font.getbbox("!");
    let hash_hinted = sha256_hex(&mask_hinted.pixels);

    // Without hinting
    let font_no_hint = font.font_variant_no_hint();
    let mask_unhinted = font_no_hint.getmask("!").unwrap();
    let bbox_unhinted = font_no_hint.getbbox("!");
    let hash_unhinted = sha256_hex(&mask_unhinted.pixels);

    println!("=== DejaVuSans '!' at 10pt ===");
    println!("HINTED:");
    println!("  bbox: {:?}", bbox_hinted);
    println!("  mask: {}x{}, xmin={}, ymin={}", mask_hinted.width, mask_hinted.height, mask_hinted.xmin, mask_hinted.ymin);
    println!("  sha: {}", &hash_hinted[..20]);

    println!("UNHINTED:");
    println!("  bbox: {:?}", bbox_unhinted);
    println!("  mask: {}x{}, xmin={}, ymin={}", mask_unhinted.width, mask_unhinted.height, mask_unhinted.xmin, mask_unhinted.ymin);
    println!("  sha: {}", &hash_unhinted[..20]);

    println!("\nPIL expected:");
    println!("  bbox: (0, 3, 4, 10)");
    println!("  mask: 4x7, offset (0, 3)");
    println!("  sha: d017cd2e78fb72e7...");
}
