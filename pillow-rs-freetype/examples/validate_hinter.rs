//! Validates the auto-hinter against FreeType C reference.
//! Traces the '|' glyph at DejaVuSans 10pt.

use pillow_rs_freetype::autohint;
use pillow_rs_freetype::fixed::ft_mul_fix;
use pillow_rs_freetype::outline::{Outline, OutlinePoint};
use pillow_rs_freetype::scaler::ScaleMetrics;
use pillow_rs_freetype::tables::FontData;
use pillow_rs_freetype::tt;

fn main() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let font_path = format!("{}/tests/fixtures/input/fonts_nohint/DejaVuSans.ttf", dir);
    let data = std::fs::read(&font_path).unwrap();

    let table_dir = tt::parse_table_directory(&data).unwrap();
    let head = tt::head::parse_head(table_dir.find(&data, tt::tag(b"head")).unwrap()).unwrap();
    let maxp = tt::maxp::parse_maxp(table_dir.find(&data, tt::tag(b"maxp")).unwrap()).unwrap();
    let cmap = tt::cmap::parse_cmap(table_dir.find(&data, tt::tag(b"cmap")).unwrap()).unwrap();
    let hhea = tt::hhea::parse_hhea(table_dir.find(&data, tt::tag(b"hhea")).unwrap()).unwrap();
    let hmtx = tt::hmtx::parse_hmtx(
        table_dir.find(&data, tt::tag(b"hmtx")).unwrap(),
        hhea.num_hmetrics, maxp.num_glyphs,
    ).unwrap();
    let os2 = table_dir.find(&data, tt::tag(b"OS/2")).and_then(tt::os2::parse_os2);
    let loca_data = table_dir.find(&data, tt::tag(b"loca")).unwrap().to_vec();
    let glyf_data = table_dir.find(&data, tt::tag(b"glyf")).unwrap().to_vec();

    let font_data = FontData {
        cmap, head, hhea, hmtx, maxp,
        name: tt::name::NameTable { family: "DejaVuSans".into(), subfamily: "Book".into() },
        os2, loca_data, glyf_data, size_pt: 10.0,
    };

    let size_pt = 10.0_f32;
    let upem = font_data.head.units_per_em;
    let scale = ScaleMetrics::new(size_pt, upem);
    let gid = font_data.cmap.char_index(124).unwrap_or(0);

    println!("============================================================");
    println!("  AUTOHINTER VALIDATION: '|' glyph, DejaVuSans {}pt", size_pt);
    println!("============================================================");
    println!("units_per_em: {}", upem);
    println!("gid: {}", gid);
    println!("x_scale: 0x{:08X} = {:.6}", scale.x_scale as u32, scale.x_scale as f64 / 65536.0);
    println!("y_scale: 0x{:08X} = {:.6}", scale.y_scale as u32, scale.y_scale as f64 / 65536.0);
    println!("ppem: {}", scale.ppem);

    let raw = pillow_rs_freetype::tt::glyf::load_glyph(
        &font_data.glyf_data, &font_data.loca_data,
        font_data.head.index_to_loc_format, gid,
    ).unwrap();

    println!("\n=== Raw font-unit outline ===");
    println!("num_contours={}, num_points={}", raw.num_contours, raw.points.len());
    println!("end_pts={:?}", &raw.end_pts_of_contours);
    for (i, p) in raw.points.iter().enumerate() {
        println!("  [{:2}] x={:5} y={:5} on={}", i, p.x, p.y, p.on_curve);
    }

    // Scale to 26.6
    let pts: Vec<OutlinePoint> = raw.points.iter().map(|p| OutlinePoint {
        x: scale.scale_x(p.x),
        y: scale.scale_y(p.y),
        on_curve: p.on_curve,
    }).collect();

    println!("\n=== Scaled 26.6 (BEFORE hinting) ===");
    let sc_x_min = pts.iter().map(|p| p.x).min().unwrap();
    let sc_y_min = pts.iter().map(|p| p.y).min().unwrap();
    let sc_x_max = pts.iter().map(|p| p.x).max().unwrap();
    let sc_y_max = pts.iter().map(|p| p.y).max().unwrap();
    println!("CBox: ({},{}) -> ({},{})", sc_x_min, sc_y_min, sc_x_max, sc_y_max);
    println!("CBox px: x={}..{}, y={}..{}", sc_x_min>>6, sc_x_max>>6, sc_y_min>>6, sc_y_max>>6);
    for (i, p) in pts.iter().enumerate() {
        println!("  [{:2}] x={:5} y={:5}  px({:+3}.{:02},{:+3}.{:02})",
            i, p.x, p.y, p.x>>6, ((p.x&63)*100/64), p.y>>6, ((p.y&63)*100/64));
    }

    // Apply autohint
    let mut outline = Outline {
        n_contours: raw.num_contours as i32,
        contours: raw.end_pts_of_contours.iter().map(|&e| e as i16).collect(),
        points: pts.clone(),
        flags: 0,
        cbox_x_min: 0, cbox_y_min: 0, cbox_x_max: 1, cbox_y_max: 1,
    };
    // Build font-wide metrics (mirrors Font::truetype).
    let mut lm = pillow_rs_freetype::autohint::AfLatinMetrics::new(upem as i32);
    let char_glyph = font_data.cmap.char_index('o' as u32).unwrap_or(0);
    if char_glyph > 0 {
        if let Ok(o2) = pillow_rs_freetype::tt::glyf::load_glyph(
            &font_data.glyf_data, &font_data.loca_data,
            font_data.head.index_to_loc_format, char_glyph,
        ) {
            let sp: Vec<OutlinePoint> = o2.points.iter().map(|p| OutlinePoint {
                x: p.x, y: p.y, on_curve: p.on_curve,
            }).collect();
            pillow_rs_freetype::autohint::latin::metrics_init_widths(&mut lm, char_glyph, &o2, &sp);
        }
    }
    pillow_rs_freetype::autohint::latin::metrics_init_blues(&mut lm, &font_data);
    let (_xs, ys) = pillow_rs_freetype::autohint::latin::metrics_scale_dim(
        &mut lm, scale.x_scale, scale.y_scale, 0, 0);
    // Re-scale pts Y with adjusted scale before hinting.
    for p in outline.points.iter_mut() {
        p.y = pillow_rs_freetype::fixed::ft_mul_fix(p.y, ys);
    }
    autohint::apply_hints(&mut outline, &raw, scale.x_scale, ys, 0, 0, Some(&lm));

    println!("\n=== After autohint (hinted 26.6) ===");
    let hx_min = outline.points.iter().map(|p| p.x).min().unwrap();
    let hy_min = outline.points.iter().map(|p| p.y).min().unwrap();
    let hx_max = outline.points.iter().map(|p| p.x).max().unwrap();
    let hy_max = outline.points.iter().map(|p| p.y).max().unwrap();
    println!("CBox: ({},{}) -> ({},{})", hx_min, hy_min, hx_max, hy_max);
    println!("CBox px: x={}..{}, y={}..{} (w={}, h={})",
        hx_min>>6, hx_max>>6, hy_min>>6, hy_max>>6,
        hx_max/64 - hx_min/64, hy_max/64 - hy_min/64);
    for (i, p) in outline.points.iter().enumerate() {
        let dx = p.x - pts[i].x;
        let dy = p.y - pts[i].y;
        let chg = if dx != 0 || dy != 0 { format!("  Δ({:+4},{:+4})", dx, dy) } else { String::new() };
        println!("  [{:2}] x={:5} y={:5}  px({:+3}.{:02},{:+3}.{:02}){}",
            i, p.x, p.y, p.x>>6, ((p.x&63)*100/64), p.y>>6, ((p.y&63)*100/64), chg);
    }

    println!("\n=== Change analysis ===");
    let x_changed: Vec<_> = outline.points.iter().enumerate()
        .filter(|(i, p)| p.x != pts[*i].x).map(|(i, p)| (i, pts[i].x, p.x, p.x - pts[i].x)).collect();
    let y_changed: Vec<_> = outline.points.iter().enumerate()
        .filter(|(i, p)| p.y != pts[*i].y).map(|(i, p)| (i, pts[i].y, p.y, p.y - pts[i].y)).collect();
    println!("X changed:");
    for (i, ox, nx, d) in &x_changed { println!("  pt[{}]: {} -> {} (delta={})", i, ox, nx, d); }
    println!("Y changed:");
    for (i, oy, ny, d) in &y_changed { println!("  pt[{}]: {} -> {} (delta={})", i, oy, ny, d); }

    // Unique Y values
    let mut y_vals: Vec<i32> = outline.points.iter().map(|p| p.y).collect();
    y_vals.sort(); y_vals.dedup();
    println!("\nUnique Y after hinting: {:?}", y_vals);
    let mut x_vals: Vec<i32> = outline.points.iter().map(|p| p.x).collect();
    x_vals.sort(); x_vals.dedup();
    println!("Unique X after hinting: {:?}", x_vals);

    // Check: what would the autohinted getmask produce?
    let font = pillow_rs_freetype::Font::truetype(&std::fs::read(&font_path).unwrap(), 10.0, Default::default()).unwrap();
    match font.getmask("|") {
        Ok(m) => {
            println!("\n=== getmask from Font ===");
            println!("width={} height={}", m.width, m.height);
            if !m.pixels.is_empty() {
                for y in 0..m.height {
                    let off = (y * m.width) as usize;
                    let row: Vec<String> = m.pixels[off..off + m.width as usize]
                        .iter().map(|b| format!("{:3}", b)).collect();
                    println!("  {}", row.join(" "));
                }
            } else {
                println!("  EMPTY");
            }
        }
        Err(e) => println!("getmask error: {:?}", e),
    }
    let bbox = font.getbbox("|");
    println!("getbbox: {:?}", bbox);

    // Verify scale computation
    println!("\n=== Verification ===");
    println!("ft_div_fix(ppem<<6, upem) = ft_div_fix({},{}) = 0x{:08X}",
        scale.ppem << 6, upem,
        pillow_rs_freetype::fixed::ft_div_fix(scale.ppem << 6, upem as i32) as u32);
    for (i, rp) in raw.points.iter().enumerate() {
        let sx = ft_mul_fix(rp.x, scale.x_scale);
        let sy = ft_mul_fix(rp.y, scale.y_scale);
        let m = if sx==pts[i].x&&sy==pts[i].y{"OK"}else{"MISMATCH"};
        println!("  fu=({},{}) -> 26.6=({},{}) {}", rp.x, rp.y, sx, sy, m);
    }

    // Compute what FreeType would produce for edge positions
    // In 26.6, FT_PIX_ROUND(value) = (value + 32) & !63
    println!("\n=== Predicted FreeType edge positions ===");
    // For '|' glyph, edges are at y=489 (top) and y=-151 (bottom)
    // And x=134 (right) and x=81 (left)
    let top_round = (489 + 32) & !63;
    let bot_round = (-151 + 32) & !63;
    let right_round = (134 + 32) & !63;
    let left_round = (81 + 32) & !63;
    println!("FT_PIX_ROUND(489) = {} (px={})", top_round, top_round>>6);
    println!("FT_PIX_ROUND(-151) = {} (px={})", bot_round, bot_round>>6);
    println!("FT_PIX_ROUND(134) = {} (px={})", right_round, right_round>>6);
    println!("FT_PIX_ROUND(81) = {} (px={})", left_round, left_round>>6);

    // When there are no stems (no stem links), FreeType's algorithm:
    // - First non-stem edge gets FT_PIX_ROUND and becomes anchor
    // - Subsequent edges use anchor + ((opos - anchor_opos + 16) & !31)
    println!("\n  Non-stem hinting (all edges lack links):");
    // Anchor is first edge in order. Vert dim: edges sorted by fpos (increasing)
    // For '|' with 4 points (0-top, 1-bottom, 2-bottom, 3-top):
    // Y edges: bottom edge at fy=-483, top edge at fy=1565
    // Anchor = bottom edge (1st), gets FT_PIX_ROUND(-151) = -128
    // Top edge = -128 + ((-151 - 489?) actually let me think about this more carefully)
    // Actually edges are sorted by fpos which is in font units.
    // Bottom segments have fy ≈ -483, top segments have fy ≈ 1565.
    // So bottom edge is first (anchor), top edge is second.
    // top_pos = anchor_pos + ((top_opos - anchor_opos + 16) & !31)
    //          = -128 + ((489 - (-151) + 16) & !31)
    //          = -128 + ((640 + 16) & !31) = -128 + (656 & !31) = -128 + 640 = 512
    // Result: bottom edge at -128 (px -2), top edge at 512 (px 8)
    // X edges: left at fx=260, right at fx=430
    // left opos = ft_mul_fix(260, 0x5000) = 81 (in 26.6)
    // right opos = ft_mul_fix(430, 0x5000) = 134
    // left edge first (anchor): FT_PIX_ROUND(81) = 64
    // right edge: 64 + ((134 - 81 + 16) & !31) = 64 + (69 & !31) = 64 + 64 = 128
    // Result: left edge at 64 (px 1), right edge at 128 (px 2)
    let bottom_o = -151i32;
    let top_o = 489i32;
    let anchor_y = (bottom_o + 32) & !63;
    let predicted_top = anchor_y + ((top_o - bottom_o + 16) & !31);
    println!("  Vert anchor (bottom): FT_PIX_ROUND({}) = {}", bottom_o, anchor_y);
    println!("  Predicted top: {} + (({} - {} + 16) & !31)", anchor_y, top_o, bottom_o);
    println!("               = {} + ({} & !31) = {}", anchor_y, top_o-bottom_o+16, predicted_top);
    println!("  Actual values: bottom={}, top={}", hy_min, hy_max);
    println!("  Match: {} (expected bottom={}, top={})",
        hy_min == anchor_y && hy_max == predicted_top,
        anchor_y, predicted_top);

    // X-axis prediction
    let left_o = 81i32;
    let right_o = 134i32;
    let anchor_x = (left_o + 32) & !63;
    let predicted_right = anchor_x + ((right_o - left_o + 16) & !31);
    println!("  Horz anchor (left): FT_PIX_ROUND({}) = {}", left_o, anchor_x);
    println!("  Predicted right: {} + (({} - {} + 16) & !31)", anchor_x, right_o, left_o);
    println!("  Actual values: left={}, right={}", hx_min, hx_max);
    println!("  Match: {} (expected left={}, right={})",
        hx_min == anchor_x && hx_max == predicted_right,
        anchor_x, predicted_right);
}
