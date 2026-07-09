//! fontdone pipeline profiler — per-stage nanosecond timing.
//!
//! Uses the public `fontdone::api` and `fontdone::ffi` layers to time
//! individual pipeline stages.  Accesses font internals via the `api` crate
//! (which is in the same workspace, so pub(crate) is accessible).
//!
//! Run: cargo run --release --bin profiler

use std::time::Instant;
use fontdone::api::{self, LoadFlags};
use fontdone::ffi;

const LOOPS: u64 = 20_000;
const WARMUP: u64 = 50;

fn time<R>(label: &str, mut f: impl FnMut() -> R) -> u128 {
    for _ in 0..WARMUP { let _ = f(); }
    let s = Instant::now();
    for _ in 0..LOOPS { let _ = f(); }
    let ns = s.elapsed().as_nanos() / LOOPS as u128;
    println!("  {:>35} {:>8} ns", label, ns);
    ns
}

fn open_face(d: &[u8], pt: f32) -> api::Face {
    api::Library::init().new_memory_face(d, 0, pt).expect("open face")
}

fn main() {
    let font_path = std::env::args().nth(1)
        .unwrap_or_else(|| "../tests/fixtures/input/fonts/DejaVuSans.ttf".into());
    let d = std::fs::read(&font_path).expect("read font");

    let face = open_face(&d, 24.0);
    let font = face.font();
    let data = &font.data;
    let scale = fontdone::scaler::ScaleMetrics::new(24.0, data.head.units_per_em);

    let ga: u16 = 36;
    let ga_u32: u32 = u32::from(ga);

    // Warm caches
    let outline_a = data.load_glyph_outline(ga).expect("load A");
    let metrics = font.face_globals.get_metrics(ga);
    for _ in 0..20 {
        let _ = face.load_glyph(ga, LoadFlags::FORCE_AUTOHINT);
        let _ = face.load_glyph(ga, LoadFlags::default());
    }

    println!("\n╔════════════════════════════════════════════════════╗");
    println!("║  fontdone pipeline profiler — {} iters, {} warmup  ║", LOOPS, WARMUP);
    println!("╚════════════════════════════════════════════════════╝\n");

    // ── 1. GLYF PARSE (warm Rc cache) ──
    println!("── 1. Glyf Parse (Rc cache hit) ──");
    let glyf_ns = time("glyf A=36 (50 pts)", || {
        let _ = data.load_glyph_outline(ga);
    });

    // ── 2. POINT SCALING ──
    println!("\n── 2. Point Scaling (font units → 26.6) ──");
    let mut buf: Vec<fontdone::outline::OutlinePoint> = Vec::with_capacity(256);
    let scale_ns = time("scale all points (50 pts)", || {
        buf.clear();
        for p in outline_a.points.iter() {
            buf.push(fontdone::outline::OutlinePoint {
                x: scale.scale_x(p.x),
                y: scale.scale_y(p.y),
                on_curve: p.on_curve,
            });
        }
    });

    // ── 3. FULL AUTOHINT ──
    println!("\n── 3. Autohint (apply_hints) ──");
    buf.clear();
    for p in outline_a.points.iter() {
        buf.push(fontdone::outline::OutlinePoint {
            x: scale.scale_x(p.x),
            y: scale.scale_y(p.y),
            on_curve: p.on_curve,
        });
    }
    let work = fontdone::outline::Outline {
        n_contours: outline_a.num_contours as i32,
        contours: outline_a.end_pts_of_contours.iter().map(|&e| e as i16).collect(),
        points: buf.clone(),
        tags: vec![],
        contour_dropouts: vec![],
        flags: 0,
        cbox_x_min: 0, cbox_y_min: 0, cbox_x_max: 20, cbox_y_max: 30,
    };
    let mut work_outline = work.clone();
    let ah_ns = time("apply_hints A=36", || {
        work_outline = work.clone();
        let _ = fontdone::autohint::apply_hints(
            &mut work_outline, &outline_a,
            scale.x_scale, scale.y_scale, 0, 0, ga,
            metrics.as_deref(),
            false, false, true, false, false,
            Some(data), false, 0,
        );
    });

    // ── 4. RENDER ──
    println!("\n── 4. Render — grays::rasterize ──");
    let render_ns = time("rasterize A=36 (AA)", || {
        let _ = fontdone::grays::rasterize(work_outline.clone());
    });

    // ── 5. API + FFI BENCH ──
    println!("\n── 5. API Core Load ──");
    let api_ah = time("Face::load_glyph (autohint)", || {
        let _ = face.load_glyph(ga, LoadFlags::FORCE_AUTOHINT);
    });
    let api_nh = time("Face::load_glyph (no hint)", || {
        let _ = face.load_glyph(ga, LoadFlags::NO_HINTING);
    });
    let api_aa = time("Face::load_glyph (load+render)", || {
        let _ = face.load_glyph(ga, LoadFlags::FORCE_AUTOHINT | LoadFlags::RENDER);
    });
    let api_df = time("Face::load_glyph (default)", || {
        let _ = face.load_glyph(ga, LoadFlags::default());
    });

    println!("\n── 6. FFI Wrapper ──");
    let ffi_face = ffi::FT_New_Memory_Face(
        &ffi::FT_Init_FreeType(), &d, 0, 24.0
    ).expect("ffi face");
    for _ in 0..20 {
        let _ = ffi::FT_Load_Glyph(&ffi_face, ga_u32, 32);
    }
    let ffi_ah = time("FT_Load_Glyph (autohint)", || {
        let _ = ffi::FT_Load_Glyph(&ffi_face, ga_u32, 32);
    });
    let ffi_nh = time("FT_Load_Glyph (no hint)", || {
        let _ = ffi::FT_Load_Glyph(&ffi_face, ga_u32, 2);
    });
    let ffi_aa = time("FT_Load_Glyph (load+render)", || {
        let _ = ffi::FT_Load_Glyph(&ffi_face, ga_u32, 4);
    });
    let ffi_df = time("FT_Load_Glyph (default)", || {
        let _ = ffi::FT_Load_Glyph(&ffi_face, ga_u32, 0);
    });

    // ── 7. CMAP ──
    println!("\n── 7. CMAP ──");
    let cmap_ns = time("FT_Get_Char_Index('A')", || {
        let _ = ffi::FT_Get_Char_Index(&ffi_face, 65);
    });

    // ── 8. METRICS ──
    println!("\n── 8. Metrics Cache ──");
    let met_ns = time("get_metrics(A=36)", || {
        let _ = font.face_globals.get_metrics(ga);
    });

    // ── COST MODEL ──
    println!("\n═══ PIPELINE COST MODEL (24pt A=36) ═══");
    println!("  glyf parse (cached Rc)      {:>6} ns", glyf_ns);
    println!("  point scale (50 pts)        {:>6} ns", scale_ns);
    println!("  metrics lookup              {:>6} ns", met_ns);
    println!("  autohint (apply_hints)      {:>6} ns", ah_ns);
    println!("  render (grays rasterize)    {:>6} ns", render_ns);
    println!("  ─────────────────────────────────────");
    println!("  Core Load autohint           {:>6} ns", api_ah);
    println!("  Core Load no-hint            {:>6} ns", api_nh);
    println!("  Core Load+Render AA          {:>6} ns", api_aa);
    println!("  Core Load default            {:>6} ns", api_df);
    println!("  ─────────────────────────────────────");
    println!("  FFI Load autohint            {:>6} ns", ffi_ah);
    println!("  FFI Load no-hint             {:>6} ns", ffi_nh);
    println!("  FFI Load+Render AA           {:>6} ns", ffi_aa);
    println!("  FFI Load default             {:>6} ns", ffi_df);
    println!("  FFI wrapper overhead (avg)   {:>6} ns", ffi_ah.saturating_sub(api_ah));
    println!("  cmap lookup                  {:>6} ns", cmap_ns);

    println!("\n═══ Done. ═══\n");
}
