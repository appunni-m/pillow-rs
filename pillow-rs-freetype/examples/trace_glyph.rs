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
    let mut font =
        fontdone::Font::truetype_with_load_mode(&data, size_pt, load_mode).expect("load font");
    let pixel_size = env::var_os("FT_RS_PIXEL_SIZE").map(|_| pixel_size_from_points(size_pt));
    if let Some(pixel_size) = pixel_size {
        font.set_pixel_sizes(0, pixel_size);
    }

    if env::var_os("FT_RS_DUMP_FACE_LOAD").is_some() {
        let mut face = fontdone::Face::from_memory(&data, 0, size_pt).expect("load face");
        if let Some(pixel_size) = pixel_size {
            face.set_pixel_sizes(0, pixel_size);
        }
        let flags = match load_mode {
            fontdone::LoadMode::Default => fontdone::LoadFlags::DEFAULT,
            fontdone::LoadMode::ForceAutoHint => fontdone::LoadFlags::FORCE_AUTOHINT,
            fontdone::LoadMode::TargetLight => fontdone::LoadFlags::TARGET_LIGHT,
            fontdone::LoadMode::NoHinting => fontdone::LoadFlags::NO_HINTING,
            fontdone::LoadMode::NoAutoHint => fontdone::LoadFlags::NO_AUTOHINT,
        };
        let glyph = glyph_override.unwrap_or_else(|| face.get_char_index(ch as u32));
        let slot = if glyph_override.is_some() {
            face.load_glyph(glyph, flags).expect("load glyph")
        } else {
            face.load_char(ch as u32, flags).expect("load char")
        };
        eprintln!(
            "[R FACE LOAD] glyph={} width={} height={} hbx={} hby={} hadv={} vbx={} vby={} vadv={} advance=({}, {})",
            slot.glyph_index,
            slot.metrics.width,
            slot.metrics.height,
            slot.metrics.hori_bearing_x,
            slot.metrics.hori_bearing_y,
            slot.metrics.hori_advance,
            slot.metrics.vert_bearing_x,
            slot.metrics.vert_bearing_y,
            slot.metrics.vert_advance,
            slot.advance.x,
            slot.advance.y
        );
    }

    if env::var_os("FT_RS_DUMP_METRICS").is_some() {
        let glyph = glyph_override.unwrap_or_else(|| font.char_index(ch as u32));
        let metrics_cache = font.face_globals.get_metrics(glyph);
        let metrics_scaled = match load_mode {
            fontdone::LoadMode::ForceAutoHint => {
                fontdone::scaler::scale_glyph_for_metrics_with_autohint(
                    &font.data,
                    glyph,
                    metrics_cache.as_deref(),
                    font.is_italic,
                )
                .expect("scale glyph for force-autohint metrics")
            }
            fontdone::LoadMode::TargetLight => fontdone::scaler::scale_glyph_for_metrics_light(
                &font.data,
                glyph,
                metrics_cache.as_deref(),
                font.is_italic,
            )
            .expect("scale glyph for target-light metrics"),
            fontdone::LoadMode::NoHinting => {
                fontdone::scaler::scale_glyph_no_hinting(&font.data, glyph, font.is_italic)
                    .expect("scale glyph for no-hinting metrics")
            }
            fontdone::LoadMode::Default | fontdone::LoadMode::NoAutoHint => {
                fontdone::scaler::scale_glyph_for_metrics(&font.data, glyph, font.is_italic)
                    .expect("scale glyph for metrics")
            }
        };
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

fn pixel_size_from_points(size_pt: f32) -> u32 {
    assert!(
        size_pt.is_finite() && size_pt >= 0.0,
        "FT_RS_PIXEL_SIZE requires a non-negative finite size"
    );
    format!("{:.0}", size_pt.round())
        .parse()
        .expect("pixel size")
}
