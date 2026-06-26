//! Dump the Latin autohinter metrics (widths + blue zones) for a font.
//! Compare against FreeType's FT_TRACE5 output.
use pillow_rs_freetype::autohint::latin;

fn main() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let font_path = format!("{}/tests/fixtures/input/fonts_nohint/DejaVuSans.ttf", dir);
    let data = std::fs::read(&font_path).unwrap();

    // Mirror Font::truetype metrics computation.
    let table_dir = pillow_rs_freetype::tt::parse_table_directory(&data).unwrap();
    use pillow_rs_freetype::tt::{self, tag};
    let head = tt::head::parse_head(table_dir.find(&data, tag(b"head")).unwrap()).unwrap();
    let maxp = tt::maxp::parse_maxp(table_dir.find(&data, tag(b"maxp")).unwrap()).unwrap();
    let cmap = tt::cmap::parse_cmap(table_dir.find(&data, tag(b"cmap")).unwrap()).unwrap();
    let hhea = tt::hhea::parse_hhea(table_dir.find(&data, tag(b"hhea")).unwrap()).unwrap();
    let hmtx = tt::hmtx::parse_hmtx(table_dir.find(&data, tag(b"hmtx")).unwrap(),
                                    hhea.num_hmetrics, maxp.num_glyphs).unwrap();
    let os2 = table_dir.find(&data, tag(b"OS/2")).and_then(tt::os2::parse_os2);
    let loca_data = table_dir.find(&data, tag(b"loca")).unwrap().to_vec();
    let glyf_data = table_dir.find(&data, tag(b"glyf")).unwrap().to_vec();
    let name = tt::name::NameTable { family: "DejaVuSans".into(), subfamily: "Book".into() };

    let font_data = pillow_rs_freetype::tables::FontData {
        cmap, head, hhea, hmtx, maxp, name, os2, loca_data, glyf_data, size_pt: 10.0,
    };

    let upem = font_data.head.units_per_em as i32;
    let mut metrics = pillow_rs_freetype::autohint::AfLatinMetrics::new(upem);

    let char_glyph = font_data.cmap.char_index('o' as u32).unwrap_or(0);
    println!("standard char glyph (o): {}", char_glyph);
    if char_glyph > 0 {
        if let Ok(outline_raw) = pillow_rs_freetype::tt::glyf::load_glyph(
            &font_data.glyf_data, &font_data.loca_data,
            font_data.head.index_to_loc_format, char_glyph,
        ) {
            let scaled_points: Vec<pillow_rs_freetype::outline::OutlinePoint> = outline_raw
                .points.iter().map(|p| pillow_rs_freetype::outline::OutlinePoint {
                    x: p.x, y: p.y, on_curve: p.on_curve,
                }).collect();
            latin::metrics_init_widths(&mut metrics, char_glyph, &outline_raw, &scaled_points);
        }
    }
    println!("=== widths ===");
    for dim in 0..2 {
        let a = &metrics.axis[dim];
        print!("{} widths: {} (standard)", if dim==0 {"horizontal"} else {"vertical"}, a.standard_width);
        for i in 1..a.width_count { print!(" {}", a.widths[i].org); }
        println!();
    }

    latin::metrics_init_blues(&mut metrics, &font_data);
    println!("=== blue zones (raw) ===");
    let vaxis = &metrics.axis[1];
    println!("blue_count: {}", vaxis.blue_count);
    for (i, b) in vaxis.blues.iter().enumerate() {
        use pillow_rs_freetype::autohint::types::*;
        let kind = if b.flags & AF_LATIN_BLUE_ADJUSTMENT != 0 { " (x-height/ADJUSTMENT)" }
                   else if b.flags & AF_LATIN_BLUE_TOP != 0 { " (top)" }
                   else { "" };
        println!("blue zone {}: ref={} shoot={}{}", i, b.ref_width.org, b.shoot_width.org, kind);
    }
}
