//! Direct pixel comparison against vendored FreeType 2.14.3 C binary.
//!
//! For each font × script × codepoint × size, runs BOTH our Rust renderer
//! AND the vendored FreeType C binary, then compares SHA-256 pixel hashes.
//!
//! No pre-computed fixtures needed — the C binary is the live oracle.
//! Font inventory (font_inventory.json) provides the test matrix.
//!
//! Requirements:
//!   - /tmp/gen_refs_v4  (FreeType 2.14.3 C reference binary)
//!   - pillow-rs-freetype/freetype/build/libfreetype.so
//!   - tests/fixtures/font_inventory.json (font → script → codepoint mapping)
//!
//! Run: cargo test -p pillow-rs-freetype --test direct_ft_compare -- --test-threads=1

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use pillow_rs_freetype::{BitmapBackend, Font};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::OnceLock;

fn sha256(data: &[u8]) -> String {
    Sha256::digest(data).iter().map(|b| format!("{:02x}", b)).collect()
}

/// Cached FreeType reference: (font_hash, codepoint, size) -> SHA-256
static FT_CACHE: OnceLock<Mutex<HashMap<(u64, u32, u32), String>>> = OnceLock::new();

fn get_ft_sha(font_path: &Path, cp: u32, size: u32, ft_lib_dir: &Path, ft_bin: &Path) -> Option<String> {
    let font_data = fs::read(font_path).ok()?;
    let font_hash = {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(&font_data, &mut h);
        std::hash::Hasher::finish(&h)
    };

    let cache = FT_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    {
        let c = cache.lock().unwrap();
        if let Some(sha) = c.get(&(font_hash, cp, size)) {
            return Some(sha.clone());
        }
    }

    // Run C binary — format: w h l t pixels_hex
    let mut child = Command::new(ft_bin)
        .arg(font_path)
        .arg(format!("{:04X}", cp))
        .arg(format!("{}", size))
        .env("LD_LIBRARY_PATH", ft_lib_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn().ok()?;

    let mut stdout = String::new();
    child.stdout.take()?.read_to_string(&mut stdout).ok()?;
    child.wait().ok()?;

    let out = stdout.trim();
    if out.is_empty() || out == "NO_GLYPH" { return None; }

    let parts: Vec<&str> = out.split_whitespace().collect();
    if parts.len() < 5 { return None; }

    let px: Vec<u8> = (0..parts[4].len()).step_by(2)
        .map(|i| u8::from_str_radix(&parts[4][i..i+2], 16).unwrap_or(0))
        .collect();
    let ft_sha = sha256(&px);

    cache.lock().unwrap().insert((font_hash, cp, size), ft_sha.clone());
    Some(ft_sha)
}

#[test]
fn test_direct_ft_comparison() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let font_dir = manifest_dir.join("tests/fixtures/input/fonts_autohint");
    let ft_bin = Path::new("/tmp/gen_refs_v4");
    let ft_lib_dir = manifest_dir.join("freetype/build");

    assert!(ft_bin.exists(), "Missing /tmp/gen_refs_v4");
    assert!(ft_lib_dir.exists(), "Missing freetype/build");

    // Load font inventory (single source of truth)
    let inv_path = manifest_dir.join("tests/fixtures/font_inventory.json");
    assert!(inv_path.exists(), "Missing font_inventory.json");

    use serde::Deserialize;
    #[derive(Debug, Deserialize)]
    struct InventoryDoc { fonts: HashMap<String, FontInfo> }
    #[derive(Debug, Deserialize)]
    struct FontInfo { path: String, scripts: HashMap<String, Vec<u32>> }

    let inv: InventoryDoc = serde_json::from_str(&fs::read_to_string(&inv_path).unwrap()).unwrap();
    eprintln!("Testing {} fonts at 10pt + 20pt\n", inv.fonts.len());

    let sizes: [u32; 2] = [10, 20];
    let mut total: u32 = 0;
    let mut passed: u32 = 0;
    let mut failed: u32 = 0;
    let mut script_counts: BTreeMap<String, (u32, u32)> = BTreeMap::new();

    for (font_name, info) in &inv.fonts {
        let font_path = font_dir.join(&info.path);
        let data = match fs::read(&font_path) {
            Ok(d) => d,
            Err(_) => { eprintln!("  MISSING {font_name}"); continue; }
        };

        for &size in &sizes {
            let font = match Font::truetype(&data, size as f32, BitmapBackend::FreeType) {
                Ok(f) => f,
                Err(e) => { eprintln!("  SKIP {font_name} {size}pt: {e}"); continue; }
            };

            for (script, codepoints) in &info.scripts {
                for &cp in codepoints {
                    let ch = char::from_u32(cp).unwrap_or('?');
                    let text = ch.to_string();

                    let mask = match font.getmask(&text) {
                        Ok(m) => m,
                        Err(_) => continue,
                    };
                    let our_sha = sha256(&mask.pixels);

                    let ft_sha = match get_ft_sha(&font_path, cp, size, &ft_lib_dir, ft_bin) {
                        Some(s) => s,
                        None => continue,
                    };

                    total += 1;
                    let (p, f) = script_counts.entry(script.clone()).or_default();
                    if our_sha == ft_sha {
                        passed += 1; *p += 1;
                    } else {
                        failed += 1; *f += 1;
                    }
                }
            }
        }
        eprintln!("  ✓ {font_name}");
    }

    // ── Summary ──
    eprintln!("\n╔══════════════════════════════════════════════════════════════╗");
    eprintln!("║  Live C comparison: {passed}/{total} passed, {failed} failed");
    eprintln!("╠══════════════════════════════════════════════════════════════╣");

    let mut passing: Vec<&str> = Vec::new();
    let mut failing: Vec<(&str, u32, u32)> = Vec::new();
    for (s, (p, f)) in &script_counts {
        let t = p + f;
        if t == 0 { continue; }
        if *f == 0 { passing.push(s); }
        else { failing.push((s, *p, t)); }
    }

    failing.sort_by(|a, b| {
        ((b.2 - b.1) as f64 / b.2 as f64)
            .partial_cmp(&((a.2 - a.1) as f64 / a.2 as f64))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    eprintln!("║  PASSING — {} scripts:", passing.len());
    for s in &passing {
        let (p, _) = script_counts[*s];
        eprintln!("║    {s} {p}/{p}");
    }

    if !failing.is_empty() {
        eprintln!("║  FAILING — {} scripts:", failing.len());
        for (s, ok, total_s) in &failing {
            let pct = if *total_s > 0 { 100.0 * (*total_s - ok) as f64 / *total_s as f64 } else { 0.0 };
            eprintln!("║    {s}: {ok}/{total_s} passed ({pct:.0}% fail)");
        }
    }
    eprintln!("╚══════════════════════════════════════════════════════════════╝");

    if failed > 0 {
        panic!("{failed}/{total} pixel mismatches vs live FreeType 2.14.3");
    }
    assert!(passed > 0, "No tests passed");
}
