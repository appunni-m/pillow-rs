//! Unit test: verify compute_segments segment merging matches C for failing glyphs.
//!
//! Hypothesis: In C's aflatin.c:1803-1804, when the previous segment is longer
//! and the current segment is discarded, C copies min_pos/max_pos from the
//! discarded segment's boundaries into prev_min_pos/prev_max_pos.  Our port
//! was missing this, causing wrong segment positions for multi-segment merges.
//!
//! Build C reference binary:
//!   gcc -o /tmp/test_seg_c test_seg_c.c -I<freetype>/include -I<freetype>/src/autofit \
//!     <freetype>/build_debug/libfreetyped.a -lm -lz -lpng -lbrotlidec -lbz2
//!
//! Run: cargo test -p pillow-rs-freetype --test compute_segments_test -- --nocapture

use std::process::Command;
use sha2::{Digest, Sha256};

fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data).iter().map(|b| format!("{:02x}", b)).collect()
}

/// Run C reference binary and return (hinted_coords, pixels_sha)
fn c_reference(font_name: &str, size_pt: u32, ch: char) -> Option<(String, String)> {
    let font_path = format!(
        "tests/fixtures/input/fonts_autohint/{}.ttf", font_name
    );
    let output = Command::new("/tmp/test_seg_c")
        .arg(&font_path)
        .arg(size_pt.to_string())
        .arg(ch.to_string())
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut hinted = String::new();
    let mut pixels = String::new();
    for line in stdout.lines() {
        if line.starts_with("HINTED:") {
            hinted = line.trim_start_matches("HINTED:").to_string();
        }
        if line.starts_with("PIXELS:") {
            pixels = line.trim_start_matches("PIXELS:").to_string();
        }
    }
    Some((hinted, pixels))
}

#[test]
fn test_liberation_serif_bold_dollar_segments() {
    // LiberationSerif-Bold '$' at 10pt — one of the 3 remaining failures.
    // This glyph has complex contour topology where 3+ segments merge
    // consecutively at a shared start point, triggering the keep-prev path.
    
    let data = std::fs::read(
        "tests/fixtures/input/fonts_autohint/LiberationSerif-Bold.ttf"
    ).expect("read font");
    
    let font = pillow_rs_freetype::Font::truetype(
        &data, 10.0, pillow_rs_freetype::BitmapBackend::FreeType,
    ).expect("load");
    
    let mask = font.getmask("$").expect("getmask");
    
    // Compare against C reference
    if let Some((c_hinted, c_pixels)) = c_reference("LiberationSerif-Bold", 10, '$') {
        // Parse C hinted coords
        let c_pts: Vec<(i32,i32)> = c_hinted.split_whitespace()
            .filter_map(|s| {
                let mut parts = s.split(',');
                let x = parts.next()?.parse().ok()?;
                let y = parts.next()?.parse().ok()?;
                Some((x, y))
            }).collect();
        
        // Get Rust hinted coords via internal pipeline
        // (We can't easily extract internal state, so compare pixels)
        let rust_sha = sha256_hex(&mask.pixels);
        let c_pixels_bytes: Vec<u8> = c_pixels.split_whitespace()
            .filter_map(|s| u8::from_str_radix(s, 16).ok()).collect();
        let c_sha = sha256_hex(&c_pixels_bytes);
        
        if rust_sha == c_sha {
            println!("PASS: LiberationSerif-Bold '$' pixel SHA matches C");
        } else {
            println!("FAIL: C SHA={c_sha} Rust SHA={rust_sha}");
            println!("  C hinted: {} points", c_pts.len());
            // Dump pixel diffs
            let mask_pixels = &mask.pixels;
            println!("  C pixels: {} bytes, Rust pixels: {} bytes", 
                c_pixels_bytes.len(), mask_pixels.len());
            let diff_count: usize = c_pixels_bytes.iter().zip(mask_pixels.iter())
                .filter(|(c, r)| c != r)
                .count();
            println!("  {diff_count} pixel differences");
        }
    } else {
        println!("SKIP: C reference binary not available at /tmp/test_seg_c");
    }
}
