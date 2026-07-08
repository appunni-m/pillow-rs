//! fontdone FFI vs C ABI benchmark — runtime comparison.
//! 30 iterations/op, 10× warmup. Reports ns per operation and ratio.
//! Run from parent crate: cargo run --release

#![allow(unsafe_code, unused_must_use, clippy::all)]

use std::hint::black_box;
use std::time::Instant;

use fontdone::ffi as r;
use fontdone_ffi_c as c;

const WARMUP: u64 = 10;
const ITERS: u64 = 30;

// ── helpers ──
fn t_rust<R>(mut f: impl FnMut() -> R) -> u128 {
    for _ in 0..WARMUP { black_box(f()); }
    let s = Instant::now();
    for _ in 0..ITERS { black_box(f()); }
    s.elapsed().as_nanos() / ITERS as u128
}
fn t_c(mut f: impl FnMut()) -> u128 {
    for _ in 0..WARMUP { black_box(f()); }
    let s = Instant::now();
    for _ in 0..ITERS { black_box(f()); }
    s.elapsed().as_nanos() / ITERS as u128
}

fn row(name: &str, detail: &str, r_ns: u128, c_ns: u128) {
    let ratio = if r_ns > 0 && c_ns > 0 { r_ns as f64 / c_ns as f64 } else { 0.0 };
    println!("  {:<28} {:<16} {:>9} ns  {:>9} ns  {:>7.2}x", name, detail, r_ns, c_ns, ratio);
}

// ── font ──
fn load(name: &str) -> Vec<u8> {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().join("tests/fixtures/input/fonts");
    std::fs::read(dir.join(name)).unwrap()
}
fn open_r(d: &[u8], sz: f32) -> (r::FT_Library, r::FT_Face) {
    let l = r::FT_Init_FreeType();
    let f = r::FT_New_Memory_Face(&l, d, 0, sz).unwrap();
    (l, f)
}
fn open_c(d: &[u8], sz: f32) -> (c::FT_Library, c::FT_Face) {
    let mut l: c::FT_Library = std::ptr::null_mut();
    let mut f: c::FT_Face = std::ptr::null_mut();
    unsafe {
        c::FT_Init_FreeType(&mut l);
        c::FT_New_Memory_Face(l, d.as_ptr(), d.len() as i64, 0, &mut f);
        c::FT_Set_Pixel_Sizes(f, 0, sz as u32);
    }
    (l, f)
}
fn close_c(l: c::FT_Library, f: c::FT_Face) {
    unsafe { c::FT_Done_Face(f); c::FT_Done_FreeType(l); }
}

// ── flags ──
const D: i32 = 0;         // FT_LOAD_DEFAULT
const NH: i32 = 2;        // FT_LOAD_NO_HINTING
const RDR: i32 = 4;       // FT_LOAD_RENDER
const FA: i32 = 32;       // FT_LOAD_FORCE_AUTOHINT

fn main() {
    let d = load("DejaVuSans.ttf");
    let sizes = [10.0f32, 16.0, 24.0, 36.0, 72.0];
    let ga: u32 = 36;
    let gc: u32 = 192;
    let g10: Vec<u32> = (36..46).collect();
    let faa = RDR; // FT_LOAD_RENDER + target_normal (0) = just render normal
    let fmono = RDR | 0x20000;

    println!("\n  fontdone FFI vs C ABI — {} iters, {}× warmup, DejaVuSans {} sizes", ITERS, WARMUP, sizes.len());
    println!("  {:=<80}", "");
    println!("  {:<28} {:<16} {:>9} {:>9} {:>7}", "Operation", "Variant", "rust_ffi", "c_abi", "ratio");
    println!("  {:-<80}", "");

    for &sz in &sizes {
        let lb = format!("{}pt", sz as u32);
        let (_rl, rf) = open_r(&d, sz);
        let (cl, cf) = open_c(&d, sz);

        row("FT_New+SetPixelSizes", &lb,
            t_rust(|| { let l = r::FT_Init_FreeType(); r::FT_New_Memory_Face(&l, &d, 0, sz); }),
            t_c(|| {
                let mut l: c::FT_Library = std::ptr::null_mut();
                let mut f: c::FT_Face = std::ptr::null_mut();
                unsafe {
                    c::FT_Init_FreeType(&mut l);
                    c::FT_New_Memory_Face(l, d.as_ptr(), d.len() as i64, 0, &mut f);
                    c::FT_Set_Pixel_Sizes(f, 0, sz as u32);
                    c::FT_Done_Face(f); c::FT_Done_FreeType(l);
                }
            }),
        );

        row("FT_Load_Glyph (outline)", &lb,
            t_rust(|| { r::FT_Load_Glyph(&rf, ga, D); }),
            t_c(|| unsafe { c::FT_Load_Glyph(cf, ga, D); }));

        row("FT_Load_Glyph (no hint)", &lb,
            t_rust(|| { r::FT_Load_Glyph(&rf, ga, NH); }),
            t_c(|| unsafe { c::FT_Load_Glyph(cf, ga, NH); }));

        row("FT_Load_Glyph (autohint)", &lb,
            t_rust(|| { r::FT_Load_Glyph(&rf, ga, FA); }),
            t_c(|| unsafe { c::FT_Load_Glyph(cf, ga, FA); }));

        row("FT_Load_Glyph (composite)", &lb,
            t_rust(|| { r::FT_Load_Glyph(&rf, gc, D); }),
            t_c(|| unsafe { c::FT_Load_Glyph(cf, gc, D); }));

        row("FT_Load+Render (AA)", &lb,
            t_rust(|| { r::FT_Load_Glyph(&rf, ga, faa); }),
            t_c(|| unsafe { c::FT_Load_Glyph(cf, ga, faa); }));

        row("FT_Load+Render (mono)", &lb,
            t_rust(|| { r::FT_Load_Glyph(&rf, ga, fmono); }),
            t_c(|| unsafe { c::FT_Load_Glyph(cf, ga, fmono); }));

        row("FT_Load_Char (A=65)", &lb,
            t_rust(|| { r::FT_Load_Char(&rf, 65, D); }),
            t_c(|| unsafe { c::FT_Load_Char(cf, 65, D); }));

        row("FT_Get_Char_Index", &lb,
            t_rust(|| { black_box(r::FT_Get_Char_Index(&rf, 65)); }),
            t_c(|| unsafe { black_box(c::FT_Get_Char_Index(cf, 65)); }));

        row("FT_Get_Advance", &lb,
            t_rust(|| { black_box(r::FT_Get_Advance(&rf, ga, D)); }),
            t_c(|| unsafe { let mut a: i64 = 0; c::FT_Get_Advance(cf, ga, D, &mut a); black_box(a); }));

        row("FT_Get_Kerning", &lb,
            t_rust(|| { let mut v = r::FT_Vector::default(); r::FT_Get_Kerning(Some(&rf), 36, 37, 0, Some(&mut v)); black_box(v); }),
            t_c(|| unsafe { let mut v = c::FT_Vector::default(); c::FT_Get_Kerning(cf, 36, 37, 0, &mut v); black_box(v); }));

        row("FT_Get_FSType_Flags", &lb,
            t_rust(|| { black_box(r::FT_Get_FSType_Flags(Some(&rf))); }),
            t_c(|| unsafe { black_box(c::FT_Get_FSType_Flags(cf)); }));

        row("FT_Sfnt_Name_Count", &lb,
            t_rust(|| { black_box(r::FT_Get_Sfnt_Name_Count(Some(&rf))); }),
            t_c(|| unsafe { black_box(c::FT_Get_Sfnt_Name_Count(cf)); }));

        row("10-Glyph load", &lb,
            t_rust(|| { for &g in &g10 { r::FT_Load_Glyph(&rf, g, D); } }),
            t_c(|| unsafe { for &g in &g10 { c::FT_Load_Glyph(cf, g, D); } }));

        row("10-Glyph load+render AA", &lb,
            t_rust(|| { for &g in &g10 { r::FT_Load_Glyph(&rf, g, faa); } }),
            t_c(|| unsafe { for &g in &g10 { c::FT_Load_Glyph(cf, g, faa); } }));

        let txt = "The quick brown fox jumps over the lazy dog.";
        let chs: Vec<u64> = txt.chars().map(|c| c as u64).collect();
        row(&format!("String render {}ch", chs.len()), &lb,
            t_rust(|| { for &cp in &chs { let g = r::FT_Get_Char_Index(&rf, cp); if g > 0 { r::FT_Load_Glyph(&rf, g, faa); } } }),
            t_c(|| unsafe { for &cp in &chs { let g = c::FT_Get_Char_Index(cf, cp); if g > 0 { c::FT_Load_Glyph(cf, g, faa); } } }));

        let para = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt.";
        let pchs: Vec<u64> = para.chars().map(|c| c as u64).collect();
        row(&format!("Para shape {}ch", pchs.len()), &lb,
            t_rust(|| { for &cp in &pchs { let g = r::FT_Get_Char_Index(&rf, cp); if g > 0 { r::FT_Load_Glyph(&rf, g, faa); black_box(r::FT_Get_Advance(&rf, g, D)); } } }),
            t_c(|| unsafe { for &cp in &pchs { let g = c::FT_Get_Char_Index(cf, cp); if g > 0 { c::FT_Load_Glyph(cf, g, faa); let mut a: i64 = 0; c::FT_Get_Advance(cf, g, D, &mut a); black_box(a); } } }));

        close_c(cl, cf);
    }

    // ── fixed-point math ──
    println!("  {:-<80}", "");
    println!("  {:<28} {:<16} {:>9} {:>9} {:>7}", "Fixed-point / Trig", "(independent)", "rust_ffi", "c_abi", "ratio");
    println!("  {:-<80}", "");

    row("FT_MulFix", "12345×67890",
        t_rust(|| { black_box(r::FT_MulFix(12345, 67890)); }),
        t_c(|| unsafe { black_box(c::FT_MulFix(12345, 67890)); }));
    row("FT_DivFix", "12345÷678",
        t_rust(|| { black_box(r::FT_DivFix(12345, 678)); }),
        t_c(|| unsafe { black_box(c::FT_DivFix(12345, 678)); }));
    row("FT_Ceil+Floor+Round", "12345",
        t_rust(|| { black_box(r::FT_CeilFix(12345)); black_box(r::FT_FloorFix(12345)); black_box(r::FT_RoundFix(12345)); }),
        t_c(|| unsafe { black_box(c::FT_CeilFix(12345)); black_box(c::FT_FloorFix(12345)); black_box(c::FT_RoundFix(12345)); }));
    row("FT_Sin+FT_Cos", "angle=16384",
        t_rust(|| { black_box(r::FT_Sin(16384)); black_box(r::FT_Cos(16384)); }),
        t_c(|| unsafe { black_box(c::FT_Sin(16384)); black_box(c::FT_Cos(16384)); }));
    row("FT_Vector_Unit", "angle=16384",
        t_rust(|| { let mut v = r::FT_Vector::default(); r::FT_Vector_Unit(Some(&mut v), 16384); black_box(v); }),
        t_c(|| unsafe { let mut v = c::FT_Vector::default(); c::FT_Vector_Unit(&mut v, 16384); black_box(v); }));
    row("FT_Vector_Length", "vec(16384,0)",
        t_rust(|| { let v = r::FT_Vector { x: 16384, y: 0 }; black_box(r::FT_Vector_Length(Some(&v))); }),
        t_c(|| unsafe { let mut v = c::FT_Vector { x: 16384, y: 0 }; black_box(c::FT_Vector_Length(&mut v)); }));
    row("FT_Matrix_Multiply", "identity×skew",
        t_rust(|| { let a = r::FT_Matrix { xx: 65536, xy: 0, yx: 0, yy: 65536 }; let mut b = r::FT_Matrix { xx: 32768, xy: 16384, yx: 16384, yy: 32768 }; r::FT_Matrix_Multiply(Some(&a), Some(&mut b)); black_box(b); }),
        t_c(|| unsafe { let a = c::FT_Matrix { xx: 65536, xy: 0, yx: 0, yy: 65536 }; let mut b = c::FT_Matrix { xx: 32768, xy: 16384, yx: 16384, yy: 32768 }; c::FT_Matrix_Multiply(&a, &mut b); black_box(b); }));
    row("FT_Matrix_Invert", "identity",
        t_rust(|| { let mut m = r::FT_Matrix { xx: 65536, xy: 0, yx: 0, yy: 65536 }; black_box(r::FT_Matrix_Invert(Some(&mut m))); }),
        t_c(|| unsafe { let mut m = c::FT_Matrix { xx: 65536, xy: 0, yx: 0, yy: 65536 }; black_box(c::FT_Matrix_Invert(&mut m)); }));

    // ── Autohint path ──
    println!("  {:-<80}", "");
    println!("  {:<28} {:<16} {:>9} {:>9} {:>7}", "NotoSans @24pt hinting", "Latin glyph A=36", "rust_ffi", "c_abi", "ratio");
    println!("  {:-<80}", "");
    if let Ok(noto) = std::fs::read(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
            .join("tests/fixtures/input/fonts/NotoSans-Regular.ttf")
    ) {
        let (_rl, rf) = open_r(&noto, 24.0);
        let (cl, cf) = open_c(&noto, 24.0);
        row("Force autohint", "A 36",
            t_rust(|| { r::FT_Load_Glyph(&rf, 36, RDR | FA); }),
            t_c(|| unsafe { c::FT_Load_Glyph(cf, 36, RDR | FA); }));
        row("Bytecode default", "A 36",
            t_rust(|| { r::FT_Load_Glyph(&rf, 36, RDR); }),
            t_c(|| unsafe { c::FT_Load_Glyph(cf, 36, RDR); }));
        row("No hinting", "A 36",
            t_rust(|| { r::FT_Load_Glyph(&rf, 36, RDR | NH); }),
            t_c(|| unsafe { c::FT_Load_Glyph(cf, 36, RDR | NH); }));
        close_c(cl, cf);
    }

    // ── Critical path aggregate (20 glyphs × 30 iters) ──
    println!("\n  {:-<80}", "");
    println!("  {:<28} {:<16} {:>9} {:>9} {:>7}", "Critical path aggregate", "20 glyphs load+render+adv", "rust_ffi", "c_abi", "ratio");
    println!("  {:-<80}", "");
    let g20: Vec<u32> = (36..56).collect();
    for &sz in &sizes {
        let lb = format!("{}pt", sz as u32);
        let (_rl, rf) = open_r(&d, sz);
        let (cl, cf) = open_c(&d, sz);
        let r_s = Instant::now();
        for &g in &g20 { for _ in 0..ITERS { r::FT_Load_Glyph(&rf, g, faa); black_box(r::FT_Get_Advance(&rf, g, D)); } }
        let r_ns = r_s.elapsed().as_nanos() / (ITERS as u128 * g20.len() as u128);
        let c_s = Instant::now();
        for &g in &g20 { for _ in 0..ITERS { unsafe { c::FT_Load_Glyph(cf, g, faa); let mut a: i64 = 0; c::FT_Get_Advance(cf, g, D, &mut a); black_box(a); } } }
        let c_ns = c_s.elapsed().as_nanos() / (ITERS as u128 * g20.len() as u128);
        row("Load+Render+Advance", &lb, r_ns, c_ns);
        close_c(cl, cf);
    }
    println!("\n  Done.\n");
}
