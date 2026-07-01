//! Debug tool: compare Our output vs FreeType 2.14.3 pixel by pixel.
use pillow_rs_freetype::{BitmapBackend, Font};
use sha2::{Digest, Sha256};
use std::process::Command;
use std::fs;

fn sha256(data: &[u8]) -> String {
    Sha256::digest(data).iter().map(|b| format!("{:02x}", b)).collect()
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("trace")).init();
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: debug_glyph <font.ttf> <size_pt> <codepoint_hex>");
        return;
    }
    let font_path = &args[1];
    let size: f32 = args[2].parse().unwrap();
    let cp: u32 = u32::from_str_radix(&args[3], 16).unwrap();
    let ch = char::from_u32(cp).unwrap_or('?');

    let data = fs::read(font_path).unwrap();
    let font = Font::truetype(&data, size, BitmapBackend::FreeType).unwrap();
    let mask = font.getmask(&ch.to_string()).unwrap();
    let bbox = font.getbbox(&ch.to_string());

    let our_sha = sha256(&mask.pixels);
    eprintln!("OUR: {}x{} bbox={:?} SHA={}", mask.width, mask.height, bbox, &our_sha[..16]);

    // Run FreeType
    let result = Command::new("/tmp/gen_refs_v4")
        .arg(font_path).arg(&format!("{:04X}", cp)).arg(&format!("{}", size))
        .env("LD_LIBRARY_PATH", "pillow-rs-freetype/freetype/build")
        .output().unwrap();

    let out = String::from_utf8_lossy(&result.stdout).trim().to_string();
    if out.is_empty() || out == "NO_GLYPH" {
        eprintln!("FT: NO_GLYPH"); return;
    }
    let parts: Vec<&str> = out.split_whitespace().collect();
    if parts.len() < 5 { eprintln!("FT parse fail"); return; }
    let (w, h, l, t) = (
        parts[0].parse::<i32>().unwrap(), parts[1].parse::<i32>().unwrap(),
        parts[2].parse::<i32>().unwrap(), parts[3].parse::<i32>().unwrap());
    let ft_px: Vec<u8> = (0..parts[4].len()).step_by(2)
        .map(|i| u8::from_str_radix(&parts[4][i..i+2], 16).unwrap()).collect();
    let ft_sha = sha256(&ft_px);
    let ft_bbox = (l, t - h, l + w, t);

    eprintln!("FT:  {}x{} bbox={:?} SHA={}", w, h, ft_bbox, &ft_sha[..16]);
    if our_sha == ft_sha {
        eprintln!("MATCH");
    } else {
        eprintln!("DIFF:");
        let min = mask.pixels.len().min(ft_px.len());
        for i in 0..min {
            if mask.pixels[i] != ft_px[i] {
                eprintln!("  [{}] our=0x{:02x} ft=0x{:02x}", i, mask.pixels[i], ft_px[i]);
            }
        }

        // Print both bitmaps side by side for visual comparison
        eprintln!("\n  OUR BITMAP (hex rows):");
        for y in 0..mask.height as usize {
            eprint!("    ");
            for x in 0..mask.width as usize {
                eprint!("{:02x}", mask.pixels[y * mask.width as usize + x]);
            }
            eprintln!();
        }
        eprintln!("\n  FT BITMAP (hex rows):");
        for y in 0..(h as usize).min(ft_px.len() / w as usize) {
            eprint!("    ");
            for x in 0..w as usize {
                eprint!("{:02x}", ft_px[y * w as usize + x]);
            }
            eprintln!();
        }
    }
}
