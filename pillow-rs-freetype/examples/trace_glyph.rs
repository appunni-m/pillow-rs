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
    let ch = args[3].chars().next().expect("char");

    let data = fs::read(font_path).expect("read font");
    let backend = if args.get(4).is_some_and(|arg| arg == "pil") {
        pillow_rs_freetype::BitmapBackend::PIL
    } else {
        pillow_rs_freetype::BitmapBackend::FreeType
    };
    let font = pillow_rs_freetype::Font::truetype(&data, size_pt, backend).expect("load font");

    if env::var_os("FT_RS_DUMP_OUTLINE").is_some() {
        let glyph = font.char_index(ch as u32);
        let metrics_cache = font.face_globals.get_metrics(glyph);
        let scaled = match backend {
            pillow_rs_freetype::BitmapBackend::PIL => {
                pillow_rs_freetype::scaler::scale_glyph_native_default(
                    &font.data,
                    glyph,
                    None,
                    font.is_italic,
                )
            }
            pillow_rs_freetype::BitmapBackend::FreeType => pillow_rs_freetype::scaler::scale_glyph(
                &font.data,
                glyph,
                metrics_cache.as_ref(),
                font.is_italic,
            ),
        }
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
