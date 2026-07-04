//! Direct pixel comparison against vendored FreeType 2.14.3 C binary.
//!
//! Per-pixel diff: diff_count, max_diff, first divergent byte, size delta.
//! Per-script stats: avg_failing_diffs and max_diff for error-rate analysis.
//!
//! No pre-computed fixtures — the C binary is the live oracle.
//! Run: cargo test -p pillow-rs-freetype --test direct_ft_compare -- --test-threads=1

#![allow(clippy::unwrap_used, clippy::expect_used)]

use pillow_rs_freetype::{BitmapBackend, Font};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};

fn sha256(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

fn font_key(data: &[u8]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    data.hash(&mut h);
    h.finish()
}

// (sha, pixels, width, height)
type CachedEntry = (String, Vec<u8>, usize, usize);
static FT_CACHE: OnceLock<Mutex<HashMap<(u64, u32, u32), CachedEntry>>> = OnceLock::new();

fn get_ft_ref(
    path: &Path,
    font_key: u64,
    cp: u32,
    sz: u32,
    lib_dir: &Path,
    bin: &Path,
) -> Option<CachedEntry> {
    let cache = FT_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(e) = cache.lock().unwrap().get(&(font_key, cp, sz)) {
        return Some(e.clone());
    }

    let mut child = Command::new(bin)
        .arg(path)
        .arg(format!("{:04X}", cp))
        .arg(format!("{}", sz))
        .env("LD_LIBRARY_PATH", lib_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut out = String::new();
    child.stdout.take()?.read_to_string(&mut out).ok()?;
    child.wait().ok()?;
    let out = out.trim();
    if out.is_empty() || out == "NO_GLYPH" {
        return None;
    }
    let p: Vec<&str> = out.split_whitespace().collect();
    if p.len() < 5 {
        return None;
    }
    let w: usize = p[0].parse().unwrap_or(0);
    let h: usize = p[1].parse().unwrap_or(0);
    let px: Vec<u8> = (0..p[4].len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&p[4][i..i + 2], 16).unwrap_or(0))
        .collect();
    let sha = sha256(&px);
    let entry = (sha, px, w, h);
    cache
        .lock()
        .unwrap()
        .insert((font_key, cp, sz), entry.clone());
    Some(entry)
}

#[test]
#[ignore = "live C oracle; static fixture pixel tests are the default CI coverage"]
fn test_direct_ft_comparison() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let font_dir = manifest_dir.join("tests/fixtures/input/fonts_autohint");
    let ft_bin = Path::new("/tmp/gen_refs_v4");
    let ft_lib = manifest_dir.join("freetype/build");
    assert!(ft_bin.exists() && ft_lib.exists());

    let inv_path = manifest_dir.join("tests/fixtures/font_inventory.json");
    assert!(inv_path.exists());

    use serde::Deserialize;
    #[derive(Debug, Deserialize)]
    struct Inv {
        fonts: HashMap<String, FontInfo>,
    }
    #[derive(Debug, Deserialize)]
    struct FontInfo {
        path: String,
        scripts: HashMap<String, Vec<u32>>,
    }
    let inv: Inv = serde_json::from_str(&fs::read_to_string(&inv_path).unwrap()).unwrap();
    eprintln!("Testing {} fonts at 10pt+20pt\n", inv.fonts.len());

    let sizes = [10u32, 20];
    let (mut total, mut passed, mut failed) = (0u32, 0u32, 0u32);
    let mut script_counts: BTreeMap<String, (u32, u32)> = BTreeMap::new();

    // Per-script pixel error statistics
    struct PxStats {
        diffs: u32,
        max_diff: u32,
        total_mag: f64,
        count: u32,
    }
    let mut script_stats: BTreeMap<String, PxStats> = BTreeMap::new();

    for (font_name, info) in &inv.fonts {
        let font_path = font_dir.join(&info.path);
        let data = match fs::read(&font_path) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let key = font_key(&data);

        for &size in &sizes {
            let font = match Font::truetype(&data, size as f32, BitmapBackend::FreeType) {
                Ok(f) => f,
                Err(_) => continue,
            };
            for (script, cps) in &info.scripts {
                for &cp in cps {
                    let mask = match font.getmask(&char::from_u32(cp).unwrap_or('?').to_string()) {
                        Ok(m) => m,
                        Err(_) => continue,
                    };
                    let ft_ref = match get_ft_ref(&font_path, key, cp, size, &ft_lib, ft_bin) {
                        Some(r) => r,
                        None => continue,
                    };
                    let (_ft_sha, ref ft_px, ft_w, ft_h) = ft_ref;
                    total += 1;
                    let (p, f) = script_counts.entry(script.clone()).or_default();

                    if mask.width as usize == ft_w
                        && mask.height as usize == ft_h
                        && mask.pixels == *ft_px
                    {
                        passed += 1;
                        *p += 1;
                    } else {
                        failed += 1;
                        *f += 1;
                        let min = mask.pixels.len().min(ft_px.len());
                        let (mut dc, mut maxd, mut first) = (0u32, 0u32, None::<usize>);
                        for i in 0..min {
                            let d = (mask.pixels[i] as i32 - ft_px[i] as i32).unsigned_abs();
                            if d > 0 {
                                dc += 1;
                                if d > maxd {
                                    maxd = d;
                                }
                                if first.is_none() {
                                    first = Some(i);
                                }
                            }
                        }
                        let sd = mask.pixels.len() as i32 - min as i32;
                        let st = script_stats.entry(script.clone()).or_insert(PxStats {
                            diffs: 0,
                            max_diff: 0,
                            total_mag: 0.0,
                            count: 0,
                        });
                        st.diffs += dc;
                        if maxd > st.max_diff {
                            st.max_diff = maxd;
                        }
                        st.total_mag += dc as f64;
                        st.count += 1;
                        eprintln!("  FAIL [{}_{}_{}_{}] our={}x{} ft={}x{} diffs={} max={} first={:?} size_delta={}",
                            font_name, size, cp, script, mask.width, mask.height, ft_w, ft_h, dc, maxd, first, sd);
                    }
                }
            }
        }
        eprintln!("  ✓ {font_name}");
    }

    // Summary
    eprintln!("\n╔══════════════════════════════════════════════════════════════╗");
    eprintln!("║  Live C: {passed}/{total} passed, {failed} failed");
    eprintln!("╠══════════════════════════════════════════════════════════════╣");

    let mut pass: Vec<&str> = vec![];
    let mut fail: Vec<(&str, u32, u32)> = vec![];
    for (s, (p, f)) in &script_counts {
        let t = p + f;
        if t == 0 {
            continue;
        }
        if *f == 0 {
            pass.push(s);
        } else {
            fail.push((s, *p, t));
        }
    }

    // Sort failing scripts by failure rate (worst first)
    fail.sort_by(|a, b| {
        let ra = (b.2 - b.1) as f64 / b.2 as f64;
        let rb = (a.2 - a.1) as f64 / a.2 as f64;
        ra.partial_cmp(&rb).unwrap_or(std::cmp::Ordering::Equal)
    });

    if !pass.is_empty() {
        eprintln!("║  PASSING — {} scripts:", pass.len());
        for s in &pass {
            let (p2, _) = script_counts[*s];
            eprintln!("║    {s} {p2}/{p2}");
        }
    }
    if !fail.is_empty() {
        eprintln!("║  FAILING — {} scripts (pixel error stats):", fail.len());
        for (s, ok, total_s) in &fail {
            let fail_pct = if *total_s > 0 {
                100.0 * (*total_s - ok) as f64 / *total_s as f64
            } else {
                0.0
            };
            let st = script_stats.get(*s);
            let (avg, maxd) = st.map_or((0.0, 0), |s| {
                let a = if s.count > 0 {
                    s.total_mag / s.count as f64
                } else {
                    0.0
                };
                (a, s.max_diff)
            });
            eprintln!("║    {s}: {ok}/{total_s} passed ({fail_pct:.0}% fail) avg_diffs={avg:.1} max_diff={maxd}");
        }
    }
    eprintln!("╚══════════════════════════════════════════════════════════════╝");

    if failed > 0 {
        panic!("{failed}/{total} pixel mismatches vs live FreeType 2.14.3");
    }
    assert!(passed > 0, "No tests passed");
}
