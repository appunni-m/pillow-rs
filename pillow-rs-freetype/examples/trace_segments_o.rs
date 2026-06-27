//! Trace compute_segments for 'o' glyph at identity scale.
//! Compare segment positions and stem-pair distances with C reference.
use pillow_rs_freetype::autohint::{latin, loader, AfLatinMetrics, GlyphHints};
use pillow_rs_freetype::autohint::Dimension;
use pillow_rs_freetype::tt;

fn main() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let font_path = format!("{}/tests/fixtures/input/fonts_autohint/LiberationSerif-Regular.ttf", dir);
    let data = std::fs::read(&font_path).unwrap();

    let table_dir = pillow_rs_freetype::tt::parse_table_directory(&data).unwrap();
    use pillow_rs_freetype::tt::tag;
    let head = tt::head::parse_head(table_dir.find(&data, tag(b"head")).unwrap()).unwrap();
    let maxp = tt::maxp::parse_maxp(table_dir.find(&data, tag(b"maxp")).unwrap()).unwrap();
    let cmap = tt::cmap::parse_cmap(table_dir.find(&data, tag(b"cmap")).unwrap()).unwrap();
    let hhea = tt::hhea::parse_hhea(table_dir.find(&data, tag(b"hhea")).unwrap()).unwrap();
    let hmtx = tt::hmtx::parse_hmtx(table_dir.find(&data, tag(b"hmtx")).unwrap(),
                                    hhea.num_hmetrics, maxp.num_glyphs).unwrap();
    let os2 = table_dir.find(&data, tag(b"OS/2")).and_then(tt::os2::parse_os2);
    let loca_data = table_dir.find(&data, tag(b"loca")).unwrap().to_vec();
    let glyf_data = table_dir.find(&data, tag(b"glyf")).unwrap().to_vec();
    let name = tt::name::NameTable { family: "LiberationSerif".into(), subfamily: "Regular".into() };

    let font_data = pillow_rs_freetype::tables::FontData {
        cmap, head, hhea, hmtx, maxp, name, os2, loca_data, glyf_data, size_pt: 10.0,
    };

    let upem = font_data.head.units_per_em as i32;

    // 'o' glyph
    let char_glyph = font_data.cmap.char_index('o' as u32).unwrap_or(0);
    println!("=== 'o' glyph index: {} ===", char_glyph);

    if char_glyph == 0 { return; }

    let outline_raw = pillow_rs_freetype::tt::glyf::load_glyph(
        &font_data.glyf_data, &font_data.loca_data,
        font_data.head.index_to_loc_format, char_glyph,
    ).unwrap();

    let scaled_points: Vec<pillow_rs_freetype::outline::OutlinePoint> = outline_raw
        .points.iter().map(|p| pillow_rs_freetype::outline::OutlinePoint {
            x: p.x, y: p.y, on_curve: p.on_curve,
        }).collect();

    // Build hints at identity scale (0x10000)
    let mut hints = GlyphHints::new(0x10000, 0x10000, 0, 0);
    let metrics = AfLatinMetrics::new(upem);
    hints.metrics = Some(metrics.clone());
    hints.other_flags = (1 << 0) | (1 << 1) | (1 << 2); // HORZ_SNAP | VERT_SNAP | STEM_ADJUST
    loader::reload(&mut hints, &outline_raw, &scaled_points);

    println!("cw_orientation: {} (CW=PostScript)", hints.cw_orientation);
    println!("Total points: {}", hints.num_points());
    println!("Contours: {:?}", hints.contours);

    // Dump raw points
    println!("\n=== Raw points (fx, fy, out_dir, in_dir) ===");
    for (i, pt) in hints.points.iter().enumerate() {
        println!("  p{:2}: fx={:6} fy={:6} out={:?} in={:?} flags=0x{:04x}",
                 i, pt.fx, pt.fy, pt.out_dir, pt.in_dir, pt.flags);
    }

    // Run compute_segments for VERT
    println!("\n=== VERT compute_segments ===");
    latin::compute_segments(&mut hints, Dimension::Vert);

    let axis = &hints.axis[Dimension::Vert as usize];
    println!("VERT major_dir: {:?}", axis.major_dir);
    println!("VERT segments count: {}", axis.segments.len());
    for (i, seg) in axis.segments.iter().enumerate() {
        let pts = &hints.points;
        let first_dir = pts[seg.first].out_dir;
        println!("  seg[{}]: pos={:4} min_coord={:4} max_coord={:4} first={:2} last={:2} dir={:?} size={}"
                 , i, seg.pos, seg.min_coord, seg.max_coord, seg.first, seg.last,
                 first_dir, seg.max_coord - seg.min_coord);
    }

    // Now run HORZ for comparison
    println!("\n=== HORZ compute_segments ===");
    let mut hints_h = GlyphHints::new(0x10000, 0x10000, 0, 0);
    let met_clone = AfLatinMetrics::new(upem);
    hints_h.metrics = Some(met_clone);
    hints_h.other_flags = (1 << 0) | (1 << 1) | (1 << 2);
    loader::reload(&mut hints_h, &outline_raw, &scaled_points);
    latin::compute_segments(&mut hints_h, Dimension::Horz);

    let axis_h = &hints_h.axis[Dimension::Horz as usize];
    println!("HORZ major_dir: {:?}", axis_h.major_dir);
    println!("HORZ segments count: {}", axis_h.segments.len());
    for (i, seg) in axis_h.segments.iter().enumerate() {
        println!("  seg[{}]: pos={:4} min_coord={:4} max_coord={:4} first={:2} last={:2} dir={:?} size={}",
                 i, seg.pos, seg.min_coord, seg.max_coord, seg.first, seg.last,
                 seg.dir, seg.max_coord - seg.min_coord);
    }

    // Now compute stem-pair distances for VERT
    println!("\n=== VERT link_segments (no widths) ===");
    let mut hints_v2 = GlyphHints::new(0x10000, 0x10000, 0, 0);
    let met_clone2 = AfLatinMetrics::new(upem);
    hints_v2.metrics = Some(met_clone2);
    hints_v2.other_flags = (1 << 0) | (1 << 1) | (1 << 2);
    loader::reload(&mut hints_v2, &outline_raw, &scaled_points);
    latin::compute_segments(&mut hints_v2, Dimension::Vert);
    latin::link_segments_inner(&mut hints_v2, Dimension::Vert, 0, &[]);

    let axis_v2 = &hints_v2.axis[Dimension::Vert as usize];
    let segs = &axis_v2.segments;
    for i in 0..segs.len() {
        let link = segs[i].link;
        if link != usize::MAX && i == segs[link].link && link > i {
            let dist = (segs[i].pos as i32 - segs[link].pos as i32).abs();
            println!("  stem pair: seg[{}]<->seg[{}]  distance={}  pos={}↔{}",
                     i, link, dist, segs[i].pos, segs[link].pos);
        }
    }

    // Run full metrics_init_widths and print VERT widths
    println!("\n=== Full metrics_init_widths ===");
    let mut metrics_full = AfLatinMetrics::new(upem);
    latin::metrics_init_widths(&mut metrics_full, char_glyph, &outline_raw, &scaled_points);

    for dim in 0..2 {
        let a = &metrics_full.axis[dim];
        print!("{} widths: stdw={}", if dim==0 {"HORZ"} else {"VERT"}, a.standard_width);
        for i in 0..a.width_count {
            print!(" w[{}].org={}", i, a.widths[i].org);
        }
        println!();
    }
}
