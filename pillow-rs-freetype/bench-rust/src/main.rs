//! fontdone FFI vs C FreeType 2.14.3 benchmark.
//!
//! Compares Rust fontdone::ffi and fontdone-ffi-c (C ABI wrapper) against
//! pinned C FreeType 2.14.3.  The C baseline is printed first by bench-c/bench-ft;
//! this binary prints Rust FFI and C ABI columns so all three can be compared.
//!
//! Run:
//!   cargo run --release -- tests/fixtures/input/fonts/DejaVuSans.ttf

#![allow(unsafe_code, unused_must_use)]

use std::ffi::{c_int, c_long};
use std::hint::black_box;
use std::ptr;
use std::time::Instant;

use fontdone::ffi as r;
use fontdone_ffi_c as c;

// ── Timing ───────────────────────────────────────────────────────────────

const WARMUP: u64 = 10;
const ITERS: u64 = 30;

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
    let ratio = if c_ns > 0 { r_ns as f64 / c_ns as f64 } else { 0.0 };
    println!("  {:<34} {:<16} {:>9} ns {:>9} ns {:>7.2}x",
        name, detail, r_ns, c_ns, ratio);
}

fn row3(name: &str, detail: &str, ffi_ns: u128, cabi_ns: u128, c_ft_ns: f64) {
    let ffi_r = if c_ft_ns > 0.0 { ffi_ns as f64 / c_ft_ns } else { 0.0 };
    let cabi_r = if c_ft_ns > 0.0 { cabi_ns as f64 / c_ft_ns } else { 0.0 };
    println!("  {:<28} {:<16} {:>9}ns {:>9}ns {:>9}ns {:>6.2}x {:>6.2}x",
        name, detail, ffi_ns, cabi_ns, c_ft_ns as u128, ffi_r, cabi_r);
}

// ── Flags ────────────────────────────────────────────────────────────────

const D: i32 = 0;          // FT_LOAD_DEFAULT
const NH: i32 = 2;         // FT_LOAD_NO_HINTING
const FA: i32 = 32;        // FT_LOAD_FORCE_AUTOHINT
const RDR: i32 = 4;        // FT_LOAD_RENDER
const TAA: i32 = 0;        // FT_LOAD_TARGET_NORMAL
const TM: i32 = 0x20000;   // FT_LOAD_TARGET_MONO

// ── Font loading ─────────────────────────────────────────────────────────

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

unsafe fn open_c(d: &[u8], sz: f32) -> (c::FT_Library, c::FT_Face) {
    let mut l: c::FT_Library = ptr::null_mut();
    let mut f: c::FT_Face = ptr::null_mut();
    c::FT_Init_FreeType(&mut l);
    c::FT_New_Memory_Face(l, d.as_ptr(), d.len() as i64, 0, &mut f);
    c::FT_Set_Pixel_Sizes(f, 0, sz as u32);
    (l, f)
}
unsafe fn close_c(l: c::FT_Library, f: c::FT_Face) {
    c::FT_Done_Face(f); c::FT_Done_FreeType(l);
}

// ── Main ─────────────────────────────────────────────────────────────────

fn main() {
    let font_path = std::env::args().nth(1)
        .unwrap_or_else(|| "tests/fixtures/input/fonts/DejaVuSans.ttf".to_string());
    let d = std::fs::read(&font_path).unwrap_or_else(|e| panic!("read {}: {e}", font_path));
    let sizes = [10.0f32, 16.0, 24.0, 36.0, 72.0];
    let ga: u32 = 36; let gc: u32 = 192;
    let g10: Vec<u32> = (36..46).collect();
    let faa = RDR | TAA;
    let fmono = RDR | TM;

    println!("\n  fontdone FFI vs C ABI — {} iters, {}× warmup", ITERS, WARMUP);
    println!("  {:<28} {:<16} {:>9} {:>9} {:>7}",
        "Operation", "Detail", "Rust FFI", "C ABI", "ratio");
    println!("  {:-<76}", "");

    for &sz in &sizes {
        let lb = format!("{}pt", sz as u32);
        let (_rl, rf) = open_r(&d, sz);
        let (cl, cf) = unsafe { open_c(&d, sz) };

        row("FT_Load_Glyph (default)", &lb,
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
        row("FT_Get_Kerning", &lb,
            t_rust(|| { let mut v = r::FT_Vector::default(); r::FT_Get_Kerning(Some(&rf), 36, 37, 0, Some(&mut v)); black_box(v); }),
            t_c(|| unsafe { let mut v = c::FT_Vector::default(); c::FT_Get_Kerning(cf, 36, 37, 0, &mut v); black_box(v); }));
        row("FT_Get_Advance", &lb,
            t_rust(|| { black_box(r::FT_Get_Advance(&rf, ga, D)); }),
            t_c(|| unsafe { let mut a: i64 = 0; c::FT_Get_Advance(cf, ga, D, &mut a); black_box(a); }));
        row("FT_Sfnt_Table_Info", &lb,
            t_rust(|| { black_box(r::FT_Sfnt_Table_Info(&rf, 0)); }),
            t_c(|| unsafe { let mut t: u64 = 0; let mut l: u64 = 0; c::FT_Sfnt_Table_Info(cf, 0, &mut t, &mut l); black_box((t, l)); }));
        row("FT_Load_Sfnt_Table", &lb,
            t_rust(|| { let mut l: u64 = 54; black_box(r::FT_Load_Sfnt_Table(&rf, 0x68656164, 0, Some(&mut l))); }),
            t_c(|| unsafe { let mut l: u64 = 54; c::FT_Load_Sfnt_Table(cf, 0x68656164, 0, ptr::null_mut(), &mut l); black_box(l); }));

        // Batch ops
        row("10-Glyph load", &lb,
            t_rust(|| { for &g in &g10 { r::FT_Load_Glyph(&rf, g, D); } }),
            t_c(|| unsafe { for &g in &g10 { c::FT_Load_Glyph(cf, g, D); } }));
        row("10-Glyph load+render AA", &lb,
            t_rust(|| { for &g in &g10 { r::FT_Load_Glyph(&rf, g, faa); } }),
            t_c(|| unsafe { for &g in &g10 { c::FT_Load_Glyph(cf, g, faa); } }));

        let txt = "The quick brown fox jumps over the lazy dog.";
        let chs: Vec<u64> = txt.chars().map(|c| c as u64).collect();
        row(&format!("Str render {}ch", chs.len()), &lb,
            t_rust(|| { for &cp in &chs { let g = r::FT_Get_Char_Index(&rf, cp); if g > 0 { r::FT_Load_Glyph(&rf, g, faa); } } }),
            t_c(|| unsafe { for &cp in &chs { let g = c::FT_Get_Char_Index(cf, cp); if g > 0 { c::FT_Load_Glyph(cf, g, faa); } } }));
        row(&format!("Str advance {}ch", chs.len()), &lb,
            t_rust(|| { for &cp in &chs { let g = r::FT_Get_Char_Index(&rf, cp); if g > 0 { black_box(r::FT_Get_Advance(&rf, g, D)); } } }),
            t_c(|| unsafe { for &cp in &chs { let g = c::FT_Get_Char_Index(cf, cp); if g > 0 { let mut a: i64 = 0; c::FT_Get_Advance(cf, g, D, &mut a); black_box(a); } } }));

        let para = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt.";
        let pchs: Vec<u64> = para.chars().map(|c| c as u64).collect();
        row(&format!("Para render {}ch", pchs.len()), &lb,
            t_rust(|| { for &cp in &pchs { let g = r::FT_Get_Char_Index(&rf, cp); if g > 0 { r::FT_Load_Glyph(&rf, g, faa); } } }),
            t_c(|| unsafe { for &cp in &pchs { let g = c::FT_Get_Char_Index(cf, cp); if g > 0 { c::FT_Load_Glyph(cf, g, faa); } } }));
        row(&format!("Para advance {}ch", pchs.len()), &lb,
            t_rust(|| { for &cp in &pchs { let g = r::FT_Get_Char_Index(&rf, cp); if g > 0 { black_box(r::FT_Get_Advance(&rf, g, D)); } } }),
            t_c(|| unsafe { for &cp in &pchs { let g = c::FT_Get_Char_Index(cf, cp); if g > 0 { let mut a: i64 = 0; c::FT_Get_Advance(cf, g, D, &mut a); black_box(a); } } }));

        unsafe { close_c(cl, cf); }
    }

    // ──── NotoSans autohint ────
    println!("  {:-<76}", "");
    println!("  {:<28} {:<16} {:>9} {:>9} {:>7}", "Hinting (NotoSans 24pt)", "Latin A", "Rust FFI", "C ABI", "ratio");
    println!("  {:-<76}", "");
    let noto_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().join("tests/fixtures/input/fonts/NotoSans-Regular.ttf");
    if let Ok(noto) = std::fs::read(&noto_path) {
        let (_l, rf) = open_r(&noto, 24.0);
        let (cl, cf) = unsafe { open_c(&noto, 24.0) };
        row("Auto-hint force", "A 36",
            t_rust(|| { r::FT_Load_Glyph(&rf, 36, RDR | FA); }),
            t_c(|| unsafe { c::FT_Load_Glyph(cf, 36, RDR | FA); }));
        row("Bytecode default", "A 36",
            t_rust(|| { r::FT_Load_Glyph(&rf, 36, RDR); }),
            t_c(|| unsafe { c::FT_Load_Glyph(cf, 36, RDR); }));
        row("No hinting", "A 36",
            t_rust(|| { r::FT_Load_Glyph(&rf, 36, RDR | NH); }),
            t_c(|| unsafe { c::FT_Load_Glyph(cf, 36, RDR | NH); }));
        unsafe { close_c(cl, cf); }
    }
    println!("\n  Done.\n");
}
