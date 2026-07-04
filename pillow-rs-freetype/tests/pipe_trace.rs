//! Pipeline trace test: call autohinter stages directly for one glyph.
//! Interactive debug only — ignored by default. Enable with --include-ignored.
//! Run: PIPE_FONT=DejaVuSerif-Bold PIPE_SIZE=10 PIPE_CHAR='$' \
//!   RUST_LOG=autohint::pipeline=trace \
//!   cargo test -p pillow-rs-freetype --test pipe_trace -- --nocapture --include-ignored

#![allow(unused_variables)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::unwrap_used)]
#![allow(unused_crate_dependencies)]

use pillow_rs_freetype::autohint::latin;
use pillow_rs_freetype::{scaler, tt, BitmapBackend, Font};

fn sha256(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

#[test]
#[ignore = "interactive debug: requires env vars and specific font"]
fn trace_one_glyph() {
    let font_name = std::env::var("PIPE_FONT").unwrap_or("DejaVuSerif-Bold".into());
    let size_pt: f32 = std::env::var("PIPE_SIZE")
        .unwrap_or("10".into())
        .parse()
        .unwrap();
    let ch: char = std::env::var("PIPE_CHAR")
        .unwrap_or("$".into())
        .chars()
        .next()
        .unwrap();

    let font_path = format!("tests/fixtures/input/fonts_autohint/{font_name}.ttf");
    let data = std::fs::read(&font_path).unwrap();
    let font = Font::truetype(&data, size_pt, BitmapBackend::FreeType).unwrap();
    let fd = &font.data;
    let gid = fd.cmap.char_index(ch as u32).unwrap();

    let raw = tt::glyf::load_glyph(
        &fd.glyf_data,
        &fd.loca_data,
        fd.head.index_to_loc_format,
        gid,
        &fd.hmtx,
    )
    .unwrap();

    let base_scale = scaler::ScaleMetrics::new(size_pt, fd.head.units_per_em);
    let pp1x_fu = raw.xmin - fd.hmtx.get(gid).lsb as i32;
    let shifted_raw = tt::glyf::GlyphOutline {
        num_contours: raw.num_contours,
        end_pts_of_contours: raw.end_pts_of_contours.clone(),
        points: raw
            .points
            .iter()
            .map(|p| tt::glyf::OutlinePoint {
                x: p.x - pp1x_fu,
                ..*p
            })
            .collect(),
        xmin: 0,
        ymin: 0,
        xmax: 0,
        ymax: 0,
        is_composite: raw.is_composite,
        sub_lsb: raw.sub_lsb,
        instructions: raw.instructions.clone(),
    };

    let metrics = font.face_globals.get_metrics(gid);
    let mut m_clone = metrics.clone();
    let (x_adj, y_adj_new) = if let Some(ref mut m) = m_clone {
        latin::metrics_scale_dim(m, base_scale.x_scale, base_scale.y_scale, 0, 0)
    } else {
        (base_scale.x_scale, base_scale.y_scale)
    };

    let is_italic = (fd.head.mac_style & 2) != 0;
    let mut outline = pillow_rs_freetype::outline::Outline::default();
    latin::apply_hints(
        &mut outline,
        &shifted_raw,
        x_adj,
        y_adj_new,
        0,
        0,
        gid as u16,
        metrics.as_ref(),
        is_italic,
        false,
        true,
        Some(&fd),
    );

    let font2 = Font::truetype(&data, size_pt, BitmapBackend::FreeType).unwrap();
    let mask = font2.getmask(&ch.to_string()).unwrap();

    let sha = sha256(&mask.pixels);
    eprintln!("PIXEL_SHA: {sha}");
    print!("PIXELS:");
    for b in &mask.pixels {
        print!(" {:02x}", b);
    }
    println!();
}
