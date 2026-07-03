/// Generate PIL backend fixture from our own Rust code.
/// Run: cargo run -p pillow-rs-freetype --example gen_pil_fixture
use pillow_rs_freetype::{Font, BitmapBackend};
use sha2::{Digest, Sha256};
use std::fs;

fn sha256(data: &[u8]) -> String {
    Sha256::digest(data).iter().map(|b| format!("{:02x}", b)).collect()
}

fn main() {
    let font_dir = "pillow-rs-freetype/tests/fixtures/input/fonts_autohint";
    let fonts: &[(&str, &str)] = &[
        ("DejaVuSans-ExtraLight", "DejaVuSans-ExtraLight.ttf"),
        ("DejaVuSans-Oblique", "DejaVuSans-Oblique.ttf"),
        ("DejaVuSansMono", "DejaVuSansMono.ttf"),
        ("DejaVuSerif-Bold", "DejaVuSerif-Bold.ttf"),
        ("DejaVuSerif-Italic", "DejaVuSerif-Italic.ttf"),
        ("LiberationSans-Regular", "LiberationSans-Regular.ttf"),
        ("LiberationSansNarrow-Bold", "LiberationSansNarrow-Bold.ttf"),
        ("NotoSans-Bold", "NotoSans-Bold.ttf"),
    ];
    let sizes: &[f32] = &[10.0, 12.0, 16.0, 20.0, 24.0];
    
    let mut rows: Vec<serde_json::Value> = Vec::new();
    
    for (pil_name, filename) in fonts {
        let path = format!("{}/{}", font_dir, filename);
        let data = match fs::read(&path) {
            Ok(d) => d, Err(e) => { eprintln!("SKIP {}: {}", pil_name, e); continue; }
        };
        
        for &size in sizes {
            let font = match Font::truetype(&data, size, BitmapBackend::PIL) {
                Ok(f) => f, Err(e) => { eprintln!("SKIP {}_{}: {}", pil_name, size, e); continue; }
            };
            
            let s = size as u32;
            
            // getname
            let (family, subfamily) = font.getname();
            rows.push(serde_json::json!({
                "id": format!("{}_{}_getname", pil_name, s),
                "font": pil_name, "size_pt": size, "codepoint": 0, "char": "",
                "operation": "getname", "status": "active",
                "ref_value": [family, subfamily]
            }));
            
            // getmetrics
            let (asc, desc) = font.getmetrics();
            rows.push(serde_json::json!({
                "id": format!("{}_{}_getmetrics", pil_name, s),
                "font": pil_name, "size_pt": size, "codepoint": 0, "char": "",
                "operation": "getmetrics", "status": "active",
                "ref_value": [asc, desc]
            }));
            
            // getlength
            let len = font.getlength("Hello");
            rows.push(serde_json::json!({
                "id": format!("{}_{}_getlength_hello", pil_name, s),
                "font": pil_name, "size_pt": size, "codepoint": 0, "char": "Hello",
                "operation": "getlength", "status": "active",
                "ref_value": len
            }));
            
            // Per-character getmask and getbbox
            for cp in 33u32..127 {
                let ch = match char::from_u32(cp) {
                    Some(c) => c, None => continue,
                };
                let cs = ch.to_string();
                
                let bbox = font.getbbox(&cs);
                rows.push(serde_json::json!({
                    "id": format!("{}_{}_{}_getbbox", pil_name, s, cp),
                    "font": pil_name, "size_pt": size, "codepoint": cp, "char": cs,
                    "operation": "getbbox", "status": "active",
                    "ref_value": [bbox.0, bbox.1, bbox.2, bbox.3]
                }));
                
                if let Ok(mask) = font.getmask(&cs) {
                    let sha = sha256(&mask.pixels);
                    rows.push(serde_json::json!({
                        "id": format!("{}_{}_{}_getmask", pil_name, s, cp),
                        "font": pil_name, "size_pt": size, "codepoint": cp, "char": cs,
                        "operation": "getmask", "status": "active",
                        "ref_sha256": sha,
                        "ref_size": [mask.width, mask.height]
                    }));
                }
            }
        }
        eprintln!("  {}", pil_name);
    }
    
    let output = serde_json::json!({
        "version": "2.0.0",
        "font_source": "fonts_autohint",
        "hinting": "none",
        "generator": "pillow-rs PIL backend (self-consistency)",
        "mode": "PIL-self-consistency",
        "rows": rows
    });
    
    let json_str = serde_json::to_string_pretty(&output).unwrap();
    let out_path = "pillow-rs-freetype/tests/fixtures/coverage_matrix.json";
    fs::write(out_path, json_str).unwrap();
    eprintln!("\nDone: {} rows → {}", rows.len(), out_path);
}
