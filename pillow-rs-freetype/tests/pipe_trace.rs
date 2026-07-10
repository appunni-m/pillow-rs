//! Pipeline trace test: call autohinter stages directly for one glyph.
//! Interactive debug only — ignored by default. Enable with --include-ignored.
//! Run with environment variables, for example:
//! `PIPE_FONT=DejaVuSerif-Bold PIPE_SIZE=10 PIPE_CHAR='$' RUST_LOG=autohint::pipeline=trace make test-pipe-trace`

#![allow(unused_variables)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::unwrap_used)]
#![allow(unused_crate_dependencies)]

use fontdone::autohint::latin;
use fontdone::{Font, RenderMode, scaler, tt};

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

    let font_path = format!("tests/fixtures/deprecated/fonts_autohint/{font_name}.ttf");
    let data = std::fs::read(&font_path).unwrap();
    let font = Font::truetype(&data, size_pt).unwrap();
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
        bbox_xmin: raw.bbox_xmin,
        is_composite: raw.is_composite,
        sub_lsb: raw.sub_lsb,
        instructions: raw.instructions.clone(),
        components: Vec::new(),
    };

    let metrics = font.face_globals.get_metrics(gid);
    let mut m_clone = metrics.as_deref().cloned();
    let (x_adj, y_adj_new) = if let Some(ref mut m) = m_clone {
        latin::metrics_scale_dim(m, base_scale.x_scale, base_scale.y_scale, 0, 0)
    } else {
        (base_scale.x_scale, base_scale.y_scale)
    };

    let is_italic = (fd.head.mac_style & 2) != 0;
    let mut outline = fontdone::outline::Outline::default();
    latin::apply_hints(
        &mut outline,
        &shifted_raw,
        x_adj,
        y_adj_new,
        0,
        0,
        gid as u16,
        metrics.as_deref(),
        is_italic,
        false,
        true,
        false,
        false,
        Some(&fd),
        false,
    );

    let font2 = Font::truetype(&data, size_pt).unwrap();
    let mask = font2.getmask(&ch.to_string()).unwrap();

    let sha = sha256(&mask.pixels);
    eprintln!("PIXEL_SHA: {sha}");
    print!("PIXELS:");
    for b in &mask.pixels {
        print!(" {:02x}", b);
    }
    println!();
}

#[test]
#[ignore = "interactive debug: dump one render-mode glyph"]
fn dump_render_mode_glyph() {
    let font_name = std::env::var("PIPE_FONT").unwrap_or("DejaVuSans".into());
    let size_pt: f32 = std::env::var("PIPE_SIZE")
        .unwrap_or("10".into())
        .parse()
        .unwrap();
    let ch: char = std::env::var("PIPE_CHAR")
        .unwrap_or("\u{625}".into())
        .chars()
        .next()
        .unwrap();
    let mode = match std::env::var("PIPE_MODE").unwrap_or("mono".into()).as_str() {
        "normal" => RenderMode::Normal,
        "mono" => RenderMode::Mono,
        "lcd" => RenderMode::Lcd,
        "lcd_v" => RenderMode::LcdV,
        "sdf" => RenderMode::Sdf,
        other => panic!("unknown PIPE_MODE {other}"),
    };

    let font_path = format!("tests/fixtures/deprecated/fonts_autohint/{font_name}.ttf");
    let data = std::fs::read(&font_path).unwrap();
    let font = Font::truetype(&data, size_pt).unwrap();
    let gid = font.data.cmap.char_index(ch as u32).unwrap_or(0);
    let metrics_cache = font.face_globals.get_metrics(gid);
    let scaled = match mode {
        RenderMode::Normal | RenderMode::Sdf => {
            scaler::scale_glyph_native_default(&font.data, gid, None, font.is_italic).unwrap()
        }
        RenderMode::Mono => {
            scaler::scale_glyph_mono(&font.data, gid, metrics_cache.as_deref(), font.is_italic)
                .unwrap()
        }
        RenderMode::Lcd => {
            scaler::scale_glyph_lcd(&font.data, gid, metrics_cache.as_deref(), font.is_italic)
                .unwrap()
        }
        RenderMode::LcdV => {
            scaler::scale_glyph_lcd_v(&font.data, gid, metrics_cache.as_deref(), font.is_italic)
                .unwrap()
        }
    };
    let bitmap = font.render_char_mode(ch, mode).unwrap();

    eprintln!(
        "SCALED gid={} cbox=({}, {}, {}, {}) bbox=({}, {}, {}, {}) outline_cbox=({}, {}, {}, {})",
        gid,
        scaled.cbox_x_min,
        scaled.cbox_y_min,
        scaled.cbox_x_max,
        scaled.cbox_y_max,
        scaled.bbox_x_min,
        scaled.bbox_y_min,
        scaled.bbox_x_max,
        scaled.bbox_y_max,
        scaled.outline_cbox_x_min,
        scaled.outline_cbox_y_min,
        scaled.outline_cbox_x_max,
        scaled.outline_cbox_y_max,
    );
    eprintln!(
        "BITMAP mode={:?} width={} rows={} pitch={} left={} top={} sha={} len={}",
        mode,
        bitmap.width,
        bitmap.rows,
        bitmap.pitch,
        bitmap.left,
        bitmap.top,
        sha256(&bitmap.buffer),
        bitmap.buffer.len()
    );
    let pitch = usize::try_from(bitmap.pitch).unwrap();
    for row in 0..bitmap.rows as usize {
        print!("ROW {row:02}:");
        let start = row * pitch;
        for byte in &bitmap.buffer[start..start + pitch] {
            print!(" {byte:02x}");
        }
        println!();
    }
}
