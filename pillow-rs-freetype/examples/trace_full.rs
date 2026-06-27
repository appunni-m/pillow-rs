//! Full trace of the autohinter for one glyph. Dump segments, edges, positions.
use pillow_rs_freetype::{
    autohint::{self, Dimension}, fixed::ft_mul_fix,
    outline::{Outline, OutlinePoint}, scaler::ScaleMetrics,
    tables::FontData, tt,
};
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = &args[1]; let size: f32 = args[2].parse().unwrap();
    let ch = args[3].chars().next().unwrap();
    let data = std::fs::read(path).unwrap();
    
    let dir = tt::parse_table_directory(&data).unwrap();
    let head = tt::head::parse_head(dir.find(&data, tt::tag(b"head")).unwrap()).unwrap();
    let maxp = tt::maxp::parse_maxp(dir.find(&data, tt::tag(b"maxp")).unwrap()).unwrap();
    let cmap = tt::cmap::parse_cmap(dir.find(&data, tt::tag(b"cmap")).unwrap()).unwrap();
    let hhea = tt::hhea::parse_hhea(dir.find(&data, tt::tag(b"hhea")).unwrap()).unwrap();
    let hmtx = tt::hmtx::parse_hmtx(dir.find(&data, tt::tag(b"hmtx")).unwrap(), hhea.num_hmetrics, maxp.num_glyphs).unwrap();
    let os2 = dir.find(&data, tt::tag(b"OS/2")).and_then(tt::os2::parse_os2);
    let loca = dir.find(&data, tt::tag(b"loca")).unwrap().to_vec();
    let glyf = dir.find(&data, tt::tag(b"glyf")).unwrap().to_vec();
    let upem = head.units_per_em;
    let scale = ScaleMetrics::new(size, upem);
    let gid = cmap.char_index(ch as u32).unwrap_or(0);
    let raw = tt::glyf::load_glyph(&glyf, &loca, head.index_to_loc_format, gid).unwrap();
    
    // --- Scale to 26.6 ---
    let pts: Vec<OutlinePoint> = raw.points.iter().map(|p| OutlinePoint {
        x: scale.scale_x(p.x), y: scale.scale_y(p.y), on_curve: p.on_curve,
    }).collect();
    
    // --- Build metrics ---
    let mut lm = autohint::AfLatinMetrics::new(upem as i32);
    let og = cmap.char_index('o' as u32).unwrap_or(0);
    if let Ok(o2) = tt::glyf::load_glyph(&glyf, &loca, head.index_to_loc_format, og) {
        let sp: Vec<OutlinePoint> = o2.points.iter().map(|p| OutlinePoint { x: p.x, y: p.y, on_curve: p.on_curve }).collect();
        autohint::latin::metrics_init_widths(&mut lm, og, &o2, &sp);
    }
    let fd = FontData { cmap, head, hhea, hmtx, maxp,
        name: tt::name::NameTable { family: "".into(), subfamily: "".into() },
        os2, loca_data: loca, glyf_data: glyf, size_pt: size };
    autohint::latin::metrics_init_blues(&mut lm, &fd);
    let (_xs, ys) = autohint::latin::metrics_scale_dim(&mut lm, scale.x_scale, scale.y_scale, 0, 0);
    
    println!("x_scale=0x{:08X} y_adj=0x{:08X} ppem={}", scale.x_scale as u32, ys as u32, scale.ppem);
    println!("Std width: HORZ={} VERT={}", lm.axis[0].standard_width, lm.axis[1].standard_width);
    println!("Widths HORZ:"); for i in 0..lm.axis[0].width_count { let w=&lm.axis[0].widths[i]; println!("  [{}] org={} cur={} fit={}", i, w.org, w.cur, w.fit); }
    println!("Widths VERT:"); for i in 0..lm.axis[1].width_count { let w=&lm.axis[1].widths[i]; println!("  [{}] org={} cur={} fit={}", i, w.org, w.cur, w.fit); }
    
    // Re-scale Y with adjusted scale
    let mut pts_hinted = pts.clone();
    for p in &mut pts_hinted { p.y = ft_mul_fix(p.y, ys); }
    
    let mut outline = Outline {
        n_contours: raw.num_contours as i32,
        contours: raw.end_pts_of_contours.iter().map(|&e| e as i16).collect(),
        points: pts_hinted.clone(), flags: 0,
        cbox_x_min: 0, cbox_y_min: 0, cbox_x_max: 1, cbox_y_max: 1,
    };
    
    // --- Run the autohinter with tracing ---
    // We need to replicate apply_hints manually to dump intermediates.
    use autohint::latin::*;
    
    let mut hints = autohint::GlyphHints::new(scale.x_scale, ys, 0, 0);
    hints.metrics = Some(lm.clone());
    hints.other_flags = autohint::AF_LATIN_HINTS_STEM_ADJUST;
    
    autohint::reload_hints(&mut hints, &raw, &outline.points);
    
    // Process both dimensions
    for dim in [Dimension::Horz, Dimension::Vert] {
        let dname = if dim == Dimension::Horz { "HORZ" } else { "VERT" };
        println!("\n=== {} DIMENSION ===", dname);
        
        compute_segments_traced(&mut hints, dim, dname);
        
        let (wc, widths) = extract_widths(&hints, dim);
        println!("Extracted {} widths, max_width_org={}", wc, if wc>0 { widths.last().map(|w|w.org).unwrap_or(0) } else { 0 });
        
        link_segments_inner(&mut hints, dim, wc, &widths);
        
        compute_edges(&mut hints, dim);
        let ax = &hints.axis[dim as usize];
        println!("EDGES ({}):", ax.edges.len());
        for (i, e) in ax.edges.iter().enumerate() {
            println!("  edge[{}]: fpos={:4} opos={:5} pos={:5}  link={} serif={} flags=0x{:04X} blue={}",
                i, e.fpos, e.opos, e.pos, e.link, e.serif, e.flags,
                if e.blue_edge.is_some() { "YES" } else { "no" });
        }
        
        if dim == Dimension::Vert {
            compute_blue_edges(&mut hints);
        }
    }
    
    // Now hint and trace final positions
    let vert_widths: Vec<i32> = { let (wc, w) = extract_widths(&hints, Dimension::Vert); w.iter().take(wc).map(|w|w.cur).collect() };
    let horz_widths: Vec<i32> = { let (wc, w) = extract_widths(&hints, Dimension::Horz); w.iter().take(wc).map(|w|w.cur).collect() };
    
    hint_edges(&mut hints, Dimension::Vert, &vert_widths);
    hint_edges(&mut hints, Dimension::Horz, &horz_widths);
    align_edge_points(&mut hints, Dimension::Vert);
    align_strong_points(&mut hints, Dimension::Vert);
    align_weak_points(&mut hints, Dimension::Vert);
    align_edge_points(&mut hints, Dimension::Horz);
    align_strong_points(&mut hints, Dimension::Horz);
    align_weak_points(&mut hints, Dimension::Horz);
    hints.save_to_outline(&mut outline);
    
    // Print final hinted coords
    println!("\n=== FINAL HINTED POINTS ===");
    let fb = outline.points.clone();
    for (i, (orig, hinted)) in pts_hinted.iter().zip(fb.iter()).enumerate() {
        let dx = hinted.x - orig.x; let dy = hinted.y - orig.y;
        let marker = if dx != 0 || dy != 0 { format!(" Δ({:+4},{:+4})", dx, dy) } else { String::new() };
        println!("  pt[{}]: ({:5},{:5}) px({:3},{:3}){}", i, hinted.x, hinted.y, hinted.x>>6, hinted.y>>6, marker);
    }
}

// Copy of compute_segments with trace output
fn compute_segments_traced(hints: &mut autohint::GlyphHints, dim: Dimension, dname: &str) {
    // This is a simplified re-implementation just for tracing.
    // We need the actual function to have side effects.
    // Instead call the real one and dump after.
    use autohint::latin::*;
    compute_segments(hints, dim);
    let ax = &hints.axis[dim as usize];
    println!("SEGMENTS ({}):", ax.segments.len());
    for (i, s) in ax.segments.iter().enumerate() {
        println!("  seg[{}]: first={:3} last={:3} pos={:5} min={:5} max={:5} dir={:?} link={} serif={}",
            i, s.first, s.last, s.pos, s.min_coord, s.max_coord, s.dir, s.link, s.serif);
    }
}
