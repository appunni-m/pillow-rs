//! Debug: Trace the EXACT coordinate transformation chain from font units to
//! rasterizer subpixel coordinates for the '|' glyph in DejaVuSans at 10pt.
//!
//! Chain: font units → ft_mul_fix(scale 16.16) → 26.6 outline → ft_pix_floor
//! translation → UPSCALE(×4) → subpixel → render_line DDA

use pillow_rs_freetype::fixed::{ft_div_fix, ft_mul_fix};
use pillow_rs_freetype::scaler::{self, pixel_ceil, pixel_floor, pixel_round, ScaleMetrics};
use pillow_rs_freetype::tables::FontData;
use pillow_rs_freetype::tt;

fn main() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let font_path = format!(
        "{}/tests/fixtures/input/fonts_nohint/DejaVuSans.ttf",
        dir
    );
    let data = std::fs::read(&font_path).unwrap();

    // Parse all required tables
    let table_dir = tt::parse_table_directory(&data).unwrap();

    let head_bytes = table_dir.find(&data, tt::tag(b"head")).unwrap();
    let head = tt::head::parse_head(head_bytes).unwrap();

    let maxp_bytes = table_dir.find(&data, tt::tag(b"maxp")).unwrap();
    let maxp = tt::maxp::parse_maxp(maxp_bytes).unwrap();

    let cmap_bytes = table_dir.find(&data, tt::tag(b"cmap")).unwrap();
    let cmap = tt::cmap::parse_cmap(cmap_bytes).unwrap();

    let hhea_bytes = table_dir.find(&data, tt::tag(b"hhea")).unwrap();
    let hhea = tt::hhea::parse_hhea(hhea_bytes).unwrap();

    let hmtx_bytes = table_dir.find(&data, tt::tag(b"hmtx")).unwrap();
    let hmtx = tt::hmtx::parse_hmtx(hmtx_bytes, hhea.num_hmetrics, maxp.num_glyphs).unwrap();

    let os2 = table_dir
        .find(&data, tt::tag(b"OS/2"))
        .and_then(tt::os2::parse_os2);

    let loca_data = table_dir.find(&data, tt::tag(b"loca")).unwrap().to_vec();
    let glyf_data = table_dir.find(&data, tt::tag(b"glyf")).unwrap().to_vec();

    let font_data = FontData {
        cmap,
        head,
        hhea,
        hmtx,
        maxp,
        name: tt::name::NameTable {
            family: "DejaVuSans".into(),
            subfamily: "Book".into(),
        },
        os2,
        loca_data,
        glyf_data,
        size_pt: 10.0,
    };

    // Find glyph index for '|' (codepoint 124)
    let gid = font_data.cmap.char_index(124).unwrap_or(0);
    println!("============================================");
    println!("  '|' glyph (codepoint 124) = gid {}", gid);
    println!("============================================");

    let upem = font_data.head.units_per_em;
    println!("\n--- Font Header ---");
    println!("  units_per_em = {}", upem);
    println!("  index_to_loc_format = {}", font_data.head.index_to_loc_format);

    // Step 1: Scale metrics
    let scale = ScaleMetrics::new(10.0, upem);
    println!("\n--- Scale Metrics (Step 1: Size → ppem → 16.16 scale) ---");
    println!("  size_pt = 10.0");
    println!("  ppem    = {}", scale.ppem);
    println!("  ppem<<6 (26.6) = 0x{:08X} ({})", scale.ppem << 6, scale.ppem << 6);
    println!(
        "  x_scale (16.16) = 0x{:08X} ({})",
        scale.x_scale, scale.x_scale
    );
    println!(
        "  y_scale (16.16) = 0x{:08X} ({})",
        scale.y_scale, scale.y_scale
    );

    // Verify: FT_DivFix(ppem<<6, upem) = scale
    let computed_scale = ft_div_fix(scale.ppem << 6, upem as i32);
    println!(
        "  Verify ft_div_fix({}, {}): 0x{:08X} ({})",
        scale.ppem << 6,
        upem,
        computed_scale,
        computed_scale
    );

    // Step 2: Dump raw glyf outline in font units
    println!("\n--- Step 2: RAW glyf outline (font units) ---");
    let outline_raw =
        pillow_rs_freetype::tt::glyf::load_glyph(&font_data.glyf_data, &font_data.loca_data, font_data.head.index_to_loc_format, gid).unwrap();

    println!("  num_contours = {}", outline_raw.num_contours);
    println!("  num_points   = {}", outline_raw.points.len());
    println!("  end_pts      = {:?}", outline_raw.end_pts_of_contours);
    println!(
        "  glyf header bbox: ({}, {}, {}, {})",
        outline_raw.xmin, outline_raw.ymin, outline_raw.xmax, outline_raw.ymax
    );

    println!("\n  Points (font units):");
    for (i, pt) in outline_raw.points.iter().enumerate() {
        println!(
            "    pt[{}]: x={:6}, y={:6}, on_curve={}",
            i, pt.x, pt.y, pt.on_curve
        );
    }

    // Step 3: Scale each point to 26.6
    println!("\n--- Step 3: Points after ft_mul_fix (26.6) ---");
    let mut scaled_points: Vec<(i32, i32)> = Vec::new();
    for (i, pt) in outline_raw.points.iter().enumerate() {
        let sx = ft_mul_fix(pt.x, scale.x_scale);
        let sy = ft_mul_fix(pt.y, scale.y_scale);
        scaled_points.push((sx, sy));
        println!(
            "    pt[{}]: 26.6_x = {:6}  (0x{:08X}),  26.6_y = {:6}  (0x{:08X})",
            i, sx, sx as u32, sy, sy as u32
        );
        // Detail: show the computation
        let ab_x = (pt.x as i64).wrapping_mul(scale.x_scale as i64);
        let ab_y = (pt.y as i64).wrapping_mul(scale.y_scale as i64);
        println!(
            "      detail fu={:6}, scale=0x{:08X}, product=0x{:016X} ({}), product+0x8000={}, +sign={}, >>16={}",
            pt.x,
            scale.x_scale as u32,
            ab_x as u64,
            ab_x,
            ab_x.wrapping_add(0x8000),
            ab_x.wrapping_add(0x8000).wrapping_add(ab_x >> 63),
            (ab_x.wrapping_add(0x8000).wrapping_add(ab_x >> 63) >> 16) as i64
        );
        println!(
            "      detail fu={:6}, scale=0x{:08X}, product=0x{:016X} ({}), product+0x8000={}, +sign={}, >>16={}",
            pt.y,
            scale.y_scale as u32,
            ab_y as u64,
            ab_y,
            ab_y.wrapping_add(0x8000),
            ab_y.wrapping_add(0x8000).wrapping_add(ab_y >> 63),
            (ab_y.wrapping_add(0x8000).wrapping_add(ab_y >> 63) >> 16) as i64
        );
    }

    // Step 4: 26.6 bbox before translation
    let mut x_min = scaled_points[0].0;
    let mut y_min = scaled_points[0].1;
    let mut x_max = scaled_points[0].0;
    let mut y_max = scaled_points[0].1;
    for &(sx, sy) in &scaled_points {
        x_min = x_min.min(sx);
        y_min = y_min.min(sy);
        x_max = x_max.max(sx);
        y_max = y_max.max(sy);
    }
    println!("\n--- Step 4: 26.6 bbox (before translation) ---");
    println!(
        "  x_min={} (0x{:08X}), y_min={} (0x{:08X})",
        x_min, x_min as u32, y_min, y_min as u32
    );
    println!(
        "  x_max={} (0x{:08X}), y_max={} (0x{:08X})",
        x_max, x_max as u32, y_max, y_max as u32
    );

    // Step 5: ft_pix_floor for translation
    let floor_x = scaler::ft_pix_floor(x_min);
    let floor_y = scaler::ft_pix_floor(y_min);
    println!("\n--- Step 5: ft_pix_floor (translation offsets) ---");
    println!(
        "  ft_pix_floor(x_min={}) = {}  (0x{:08X})",
        x_min, floor_x, floor_x as u32
    );
    println!(
        "  ft_pix_floor(y_min={}) = {}  (0x{:08X})",
        y_min, floor_y, floor_y as u32
    );
    println!("  Translation moves origin to pixel ({}, {})", floor_x >> 6, floor_y >> 6);

    // Step 6: Points after translation (still 26.6)
    println!("\n--- Step 6: Points after translation (26.6, origin at 0) ---");
    let mut translated_points: Vec<(i32, i32)> = Vec::new();
    for (i, &(sx, sy)) in scaled_points.iter().enumerate() {
        let tx = sx - floor_x;
        let ty = sy - floor_y;
        translated_points.push((tx, ty));
        println!(
            "    pt[{}]: x={:6} (0x{:08X}), y={:6} (0x{:08X})  [subpix x={:3}, y={:3}]",
            i,
            tx,
            tx as u32,
            ty,
            ty as u32,
            tx & 63,
            ty & 63
        );
    }

    // Step 7: UPSCALE (×4) — converts 26.6 to subpixel units
    println!("\n--- Step 7: UPSCALE (×4) to subpixel units ---");
    println!("  UPSCALE = ONE_PIXEL >> 6 = 256 >> 6 = 4");
    for (i, &(tx, ty)) in translated_points.iter().enumerate() {
        let up_x = (tx as i64) * 4;
        let up_y = (ty as i64) * 4;
        let subpix_x = up_x & 255; // ONE_PIXEL - 1
        let subpix_y = up_y & 255;
        let pix_x = up_x >> 8;
        let pix_y = up_y >> 8;
        println!(
            "    pt[{}]: up_x={:6} (subpix={:3}, pix={}), up_y={:6} (subpix={:3}, pix={})",
            i,
            up_x,
            subpix_x,
            pix_x,
            up_y,
            subpix_y,
            pix_y
        );
    }

    // Step 8: Pixel bbox
    let px_x_min = pixel_floor(x_min);
    let px_y_min = pixel_floor(y_min);
    let px_x_max = pixel_ceil(x_max);
    let px_y_max = pixel_ceil(y_max);
    println!("\n--- Step 8: Pixel bbox (FT_GLYPH_BBOX_PIXELS) ---");
    println!("  px_x_min = {} (floor)", px_x_min);
    println!("  px_y_min = {} (floor)", px_y_min);
    println!("  px_x_max = {} (ceil)", px_x_max);
    println!("  px_y_max = {} (ceil)", px_y_max);
    println!("  bitmap width  = {}", px_x_max - px_x_min);
    println!("  bitmap height = {}", px_y_max - px_y_min);

    // Step 9: HMTX metrics
    let h_metric = font_data.hmtx.get(gid);
    let advance_26 = ft_mul_fix(h_metric.advance_width as i32, scale.x_scale);
    let lsb_26 = ft_mul_fix(h_metric.lsb as i32, scale.x_scale);
    println!("\n--- Step 9: HMTX metrics ---");
    println!("  advance_width (FU) = {}", h_metric.advance_width);
    println!("  advance (26.6)     = {}", advance_26);
    println!("  advance (pixels)   = {}", pixel_round(advance_26));
    println!("  lsb (FU)           = {}", h_metric.lsb);
    println!("  lsb (26.6)         = {}", lsb_26);

    // Step 10: render_line for vertical edges
    println!("\n--- Step 10: render_line vertical edge (dx=0 branch) ---");
    // For the '|' glyph, the vertical edges are at a constant x.
    // The dx==0 branch in render_line is:
    //
    //   two_fx = fx1 << 1;
    //   if dy > 0 {
    //     loop {
    //       fy2 = ONE_PIXEL;
    //       integrate(fy2 - fy1, two_fx);
    //       fy1 = 0; ey1 += 1;
    //       set_cell(ex1, ey1);
    //       if ey1 == ey2 break;
    //     }
    //   }
    //
    // Where fx1 = fractal part of x (from UPSCALE), fy1 = fractal part of y.
    // The subpixel x determines the "two_fx" used as area for each scanline.
    // The subpixel y-start determines which fractional-integrate step starts.

    // Find the left and right edges of the '|' bar
    let mut left_x = i32::MAX;
    let mut right_x = i32::MIN;
    let mut bottom_y = i32::MAX;
    let mut top_y = i32::MIN;
    for &(tx, ty) in &translated_points {
        left_x = left_x.min(tx);
        right_x = right_x.max(tx);
        bottom_y = bottom_y.min(ty);
        top_y = top_y.max(ty);
    }

    let left_subpix = left_x & 63;
    let right_subpix = right_x & 63;
    let _bottom_subpix = bottom_y & 63;
    let _top_subpix = top_y & 63;
    let two_fx_left = (left_subpix as i64) * 2 * 4; // subpix_x * 2 (where subpix_x = (left_x & 63) * 4)
    let two_fx_right = (right_subpix as i64) * 2 * 4;
    let two_fx_left_s = ((left_x & 63) << 1) * 4;
    let two_fx_right_s = ((right_x & 63) << 1) * 4;

    println!(
        "  left edge:  26.6_x={:6} (0x{:08X}), subpix_x={:3}, pix={:3}",
        left_x,
        left_x as u32,
        left_x & 63,
        left_x >> 6
    );
    println!(
        "  right edge: 26.6_x={:6} (0x{:08X}), subpix_x={:3}, pix={:3}",
        right_x,
        right_x as u32,
        right_x & 63,
        right_x >> 6
    );
    println!(
        "  bottom:     26.6_y={:6} (0x{:08X}), subpix_y={:3}, pix={:3}",
        bottom_y,
        bottom_y as u32,
        bottom_y & 63,
        bottom_y >> 6
    );
    println!(
        "  top:        26.6_y={:6} (0x{:08X}), subpix_y={:3}, pix={:3}",
        top_y,
        top_y as u32,
        top_y & 63,
        top_y >> 6
    );

    println!("\n  For render_line vertical (dx==0) branch:");
    println!("    left two_fx = left_subpix << 1 * 4 = {} << 1 * 4 = {}", left_x & 63, two_fx_left_s);
    println!(
        "    right two_fx = right_subpix << 1 * 4 = {} << 1 * 4 = {}",
        right_x & 63,
        two_fx_right_s
    );
    println!(
        "    UPSCALE'd two_fx_left  = {}  [the area per scanline for left edge]",
        two_fx_left
    );
    println!(
        "    UPSCALE'd two_fx_right = {}  [the area per scanline for right edge]",
        two_fx_right
    );

    // Step 11: Full scaler path
    println!("\n--- Step 11: Full scale_glyph result ---");
    let scaled = scaler::scale_glyph(&font_data, gid).unwrap();
    println!(
        "  bbox: ({}, {}, {}, {})",
        scaled.bbox_x_min, scaled.bbox_y_min, scaled.bbox_x_max, scaled.bbox_y_max
    );
    println!("  outline.n_contours = {}", scaled.outline.n_contours);
    println!("  outline.contours = {:?}", scaled.outline.contours);
    println!("  outline.points (after translation):");
    for (i, pt) in scaled.outline.points.iter().enumerate() {
        let up_x = (pt.x as i64) * 4;
        let up_y = (pt.y as i64) * 4;
        println!(
            "    pt[{}]: x={:6} (0x{:08X}, subpix={:3}, up={:6}), y={:6} (0x{:08X}, subpix={:3}, up={:6}), on={}",
            i,
            pt.x,
            pt.x as u32,
            pt.x & 63,
            up_x,
            pt.y,
            pt.y as u32,
            pt.y & 63,
            up_y,
            pt.on_curve
        );
    }
    println!(
        "  cbox: ({}, {}, {}, {})",
        scaled.outline.cbox_x_min,
        scaled.outline.cbox_y_min,
        scaled.outline.cbox_x_max,
        scaled.outline.cbox_y_max
    );
}
