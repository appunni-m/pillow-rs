//! Pipeline trace test: call autohinter stages directly for one glyph.
//! Run: RUST_LOG=autohint::pipeline=trace cargo test -p pillow-rs-freetype --test pipe_trace -- --nocapture
//! Args via env: PIPE_FONT=LiberationSerif-Bold PIPE_SIZE=10 PIPE_CHAR='$'

use pillow_rs_freetype::autohint::types::*;
use pillow_rs_freetype::autohint::{loader, latin};
use pillow_rs_freetype::{Font, BitmapBackend, outline::OutlinePoint, fixed, tt, scaler};

fn sha256(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(data).iter().map(|b| format!("{:02x}", b)).collect()
}

#[test]
fn trace_one_glyph() {
    let font_name = std::env::var("PIPE_FONT").unwrap_or("LiberationSerif-Bold".into());
    let size_pt: f32 = std::env::var("PIPE_SIZE").unwrap_or("10".into()).parse().unwrap();
    let ch: char = std::env::var("PIPE_CHAR").unwrap_or("$".into()).chars().next().unwrap();
    
    let font_path = format!("tests/fixtures/input/fonts_autohint/{font_name}.ttf");
    let data = std::fs::read(&font_path).unwrap();
    let font = Font::truetype(&data, size_pt, BitmapBackend::FreeType).unwrap();
    let fd = &font.data;
    let gid = fd.cmap.char_index(ch as u32).unwrap();
    
    // Load unscaled outline
    let raw = tt::glyf::load_glyph(
        &fd.glyf_data, &fd.loca_data, fd.head.index_to_loc_format, gid
    ).unwrap();
    
    let base_scale = scaler::ScaleMetrics::new(size_pt, fd.head.units_per_em);
    
    // Compute pp1x shift (matches scaler.rs)
    let pp1x_fu = raw.xmin - fd.hmtx.get(gid).lsb as i32;
    let shifted_raw = tt::glyf::GlyphOutline {
        num_contours: raw.num_contours,
        end_pts_of_contours: raw.end_pts_of_contours.clone(),
        points: raw.points.iter().map(|p| tt::glyf::OutlinePoint {
            x: p.x - pp1x_fu, ..*p
        }).collect(),
        xmin: 0, ymin: 0, xmax: 0, ymax: 0,
    };
    
    // Scale points
    let y_adj = font.latin_metrics.as_ref()
        .and_then(|m| { let s = m.axis[1].scale; if s != 0 { Some(s) } else { None } })
        .unwrap_or(base_scale.y_scale);
    
    let scaled_pts: Vec<OutlinePoint> = raw.points.iter().map(|p| OutlinePoint {
        x: fixed::ft_mul_fix(p.x - pp1x_fu, base_scale.x_scale),
        y: fixed::ft_mul_fix(p.y, y_adj),
        on_curve: p.on_curve,
    }).collect();
    
    // Build hints and run pipeline directly (no 'o' metric load)
    let upem = fd.head.units_per_em as i32;
    let is_italic = (fd.head.mac_style & 2) != 0;
    
    let (_x_adj, y_adj_new) = if let Some(ref mut m) = font.latin_metrics.clone() {
        latin::metrics_scale_dim(&mut m, base_scale.x_scale, base_scale.y_scale, 0, 0)
    } else {
        (base_scale.x_scale, base_scale.y_scale)
    };
    
    let mut outline = pillow_rs_freetype::outline::Outline::default();
    latin::apply_hints(
        &mut outline, &shifted_raw,
        base_scale.x_scale, y_adj_new, 0, 0,
        gid as u16, font.latin_metrics.as_ref(), is_italic,
    );
    
    // Now get the mask via the normal scaler path for pixel comparison
    let font2 = Font::truetype(&data, size_pt, BitmapBackend::FreeType).unwrap();
    let mask = font2.getmask(&ch.to_string()).unwrap();
    
    let sha = sha256(&mask.pixels);
    eprintln!("PIXEL_SHA: {sha}");
    
    // Print pixels for C comparison
    print!("PIXELS:");
    for b in &mask.pixels { print!(" {:02x}", b); }
    println!();
}
