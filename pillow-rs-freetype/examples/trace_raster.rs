//! Trace the autohinter coordinate pipeline.

use pillow_rs_freetype::autohint;
use pillow_rs_freetype::outline::{Outline, OutlinePoint};
use pillow_rs_freetype::scaler::ScaleMetrics;

fn main() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let data = std::fs::read(format!(
        "{}/tests/fixtures/input/fonts_nohint/DejaVuSans.ttf", dir
    )).unwrap();
    let font = pillow_rs_freetype::Font::truetype(&data, 10.0, Default::default()).unwrap();
    let gid = font.data.cmap.char_index('|' as u32).unwrap_or(0);
    let scale = ScaleMetrics::new(font.size_pt, font.data.head.units_per_em);

    let raw = pillow_rs_freetype::tt::glyf::load_glyph(
        &font.data.glyf_data, &font.data.loca_data,
        font.data.head.index_to_loc_format, gid,
    ).unwrap();

    // Scale to 26.6
    let mut pts: Vec<OutlinePoint> = raw.points.iter().map(|p| OutlinePoint {
        x: scale.scale_x(p.x), y: scale.scale_y(p.y), on_curve: p.on_curve,
    }).collect();

    println!("scaled 26.6 (before autohint):");
    for (i, p) in pts.iter().enumerate() {
        println!("  [{i}] x={:4} y={:4}  (pixel: x={:3} y={:3})", p.x, p.y, p.x>>6, p.y>>6);
    }

    // Apply autohinting
    let mut outline = Outline {
        n_contours: raw.num_contours as i32,
        contours: raw.end_pts_of_contours.iter().map(|&e| e as i16).collect(),
        points: pts.clone(),
        flags: 0, cbox_x_min: 0, cbox_y_min: 0, cbox_x_max: 1, cbox_y_max: 1,
    };
    autohint::apply_hints(&mut outline, &raw, scale.x_scale, scale.y_scale, 0, 0, None);

    println!("\nafter autohint (26.6):");
    for (i, p) in outline.points.iter().enumerate() {
        let changed = if p.x != pts[i].x || p.y != pts[i].y { " ← HINTED" } else { "" };
        println!("  [{i}] x={:4} y={:4}  (pixel: x={:3} y={:3}){}", p.x, p.y, p.x>>6, p.y>>6, changed);
    }
}
