//! Trace single glyph through autohinter — dump reload coords, edges, hinted output.
//! Usage: RUST_LOG=autohint::pipeline=trace cargo run --example trace_glyph -- <font.ttf> <size_pt> <char>
#![allow(clippy::expect_used)]
#![allow(unused_crate_dependencies)]

use std::env;
use std::fs;

fn main() {
    env_logger::init();
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: trace_glyph <font.ttf> <size_pt> <char>");
        return;
    }
    let font_path = &args[1];
    let size_pt: f32 = args[2].parse().expect("size_pt");
    let glyph_override = args[3]
        .strip_prefix("gid:")
        .map(|gid| gid.parse::<u16>().expect("glyph index"));
    let ch = if glyph_override.is_some() {
        '\0'
    } else if let Some(hex) = args[3].strip_prefix("U+") {
        let codepoint = u32::from_str_radix(hex, 16).expect("hex codepoint");
        char::from_u32(codepoint).expect("valid codepoint")
    } else {
        args[3].chars().next().expect("char")
    };

    let data = fs::read(font_path).expect("read font");
    let load_mode = match env::var("FT_RS_LOAD_MODE").ok().as_deref() {
        Some("force-autohint") => fontdone::LoadMode::ForceAutoHint,
        Some("target-light") => fontdone::LoadMode::TargetLight,
        Some("no-hinting") => fontdone::LoadMode::NoHinting,
        Some("no-autohint") => fontdone::LoadMode::NoAutoHint,
        Some("default") | None => fontdone::LoadMode::Default,
        Some(other) => panic!("unknown FT_RS_LOAD_MODE={other}"),
    };
    let font =
        fontdone::Font::truetype_with_load_mode(&data, size_pt, load_mode).expect("load font");

    if env::var_os("FT_RS_DUMP_METRICS").is_some() {
        let glyph = glyph_override.unwrap_or_else(|| font.char_index(ch as u32));
        let metrics_scaled =
            fontdone::scaler::scale_glyph_for_metrics(&font.data, glyph, font.is_italic)
                .expect("scale glyph for metrics");
        eprintln!(
            "[R METRICS SCALED] glyph={} cbox=({}, {}, {}, {}) outline_cbox=({}, {}, {}, {}) outline_bbox=({}, {}, {}, {}) bitmap=({}, {}, {}, {}) advance={} slot_advance={} lsb={}",
            glyph,
            metrics_scaled.cbox_x_min,
            metrics_scaled.cbox_y_min,
            metrics_scaled.cbox_x_max,
            metrics_scaled.cbox_y_max,
            metrics_scaled.outline_cbox_x_min,
            metrics_scaled.outline_cbox_y_min,
            metrics_scaled.outline_cbox_x_max,
            metrics_scaled.outline_cbox_y_max,
            metrics_scaled.outline_bbox_x_min,
            metrics_scaled.outline_bbox_y_min,
            metrics_scaled.outline_bbox_x_max,
            metrics_scaled.outline_bbox_y_max,
            metrics_scaled.bbox_x_min,
            metrics_scaled.bbox_y_min,
            metrics_scaled.bbox_x_max,
            metrics_scaled.bbox_y_max,
            metrics_scaled.advance_width,
            metrics_scaled.slot_advance_width,
            metrics_scaled.lsb
        );
        let outline_scaled = fontdone::scaler::scale_glyph(&font.data, glyph, None, font.is_italic)
            .expect("scale glyph for outline");
        eprintln!(
            "[R OUTLINE SCALED] glyph={} cbox=({}, {}, {}, {}) outline_cbox=({}, {}, {}, {}) outline_bbox=({}, {}, {}, {}) bitmap=({}, {}, {}, {}) advance={} slot_advance={} lsb={}",
            glyph,
            outline_scaled.cbox_x_min,
            outline_scaled.cbox_y_min,
            outline_scaled.cbox_x_max,
            outline_scaled.cbox_y_max,
            outline_scaled.outline_cbox_x_min,
            outline_scaled.outline_cbox_y_min,
            outline_scaled.outline_cbox_x_max,
            outline_scaled.outline_cbox_y_max,
            outline_scaled.outline_bbox_x_min,
            outline_scaled.outline_bbox_y_min,
            outline_scaled.outline_bbox_x_max,
            outline_scaled.outline_bbox_y_max,
            outline_scaled.bbox_x_min,
            outline_scaled.bbox_y_min,
            outline_scaled.bbox_x_max,
            outline_scaled.bbox_y_max,
            outline_scaled.advance_width,
            outline_scaled.slot_advance_width,
            outline_scaled.lsb
        );
        if glyph_override.is_none() {
            let metrics = font.glyph_metrics(ch as u32).expect("glyph metrics");
            eprintln!(
                "[R METRICS] width={} height={} hbx={} hby={} hadv={} vbx={} vby={} vadv={}",
                metrics.width,
                metrics.height,
                metrics.hori_bearing_x,
                metrics.hori_bearing_y,
                metrics.hori_advance,
                metrics.vert_bearing_x,
                metrics.vert_bearing_y,
                metrics.vert_advance
            );
        }
    }

    if env::var_os("FT_RS_DUMP_OUTLINE").is_some() {
        let glyph = glyph_override.unwrap_or_else(|| font.char_index(ch as u32));
        let scaled = fontdone::scaler::scale_glyph(&font.data, glyph, None, font.is_italic)
            .expect("scale glyph");
        eprintln!(
            "[R OUTLINE] glyph={} contours={} points={} cbox=({}, {}, {}, {})",
            glyph,
            scaled.outline.n_contours,
            scaled.outline.points.len(),
            scaled.outline.cbox_x_min,
            scaled.outline.cbox_y_min,
            scaled.outline.cbox_x_max,
            scaled.outline.cbox_y_max
        );
        for (idx, point) in scaled.outline.points.iter().enumerate() {
            eprintln!(
                "[R POINT {idx:02}] x={} y={} on={}",
                point.x, point.y, point.on_curve
            );
        }
    }

    if glyph_override.is_some() {
        return;
    }

    let mask = font.getmask(&ch.to_string()).expect("getmask");

    eprintln!(
        "Bitmap: {}x{} left={} top={} advance={}",
        mask.width, mask.height, mask.xmin, mask.ymin, mask.advance_width
    );
    print!("PIXELS:");
    for b in &mask.pixels {
        print!(" {:02x}", b);
    }
    println!();
}
