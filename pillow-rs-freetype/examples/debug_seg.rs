//! Debug binary: select a specific glyph, call reload + compute_segments VERT,
//! dump every point's in_dir/out_dir/fx/fy for comparison with C.
//! Run: cargo run --example debug_seg -- <font.ttf> <size_pt> <char>

use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: debug_seg <font.ttf> <size_pt> <char>");
        return;
    }
    let path = &args[1];
    let _size_pt: f32 = args[2].parse().unwrap();
    let _ch = args[3].chars().next().unwrap();
    let _data = fs::read(path).unwrap();
    
    // This would need internal access. Skip for now.
    eprintln!("Need internal access to reload/compute_segments");
}
