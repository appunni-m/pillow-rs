// Standalone test: parse_simple_glyph on gi=1996 raw bytes.
// Compare output byte-by-byte with C's TT_Load_Simple_Glyph.
fn main() {
    // Raw glyf bytes for gi=1996 (DejaVuSerif-BoldItalic)
    let d = include_bytes!("/tmp/gi1996_glyf.bin");
    println!("Data len={}", d.len());

    let nc = i16::from_be_bytes([d[0], d[1]]);
    println!("nc={} xmin={} ymin={} xmax={} ymax={}",
        nc, 
        i16::from_be_bytes([d[2], d[3]]) as i32,
        i16::from_be_bytes([d[4], d[5]]) as i32,
        i16::from_be_bytes([d[6], d[7]]) as i32,
        i16::from_be_bytes([d[8], d[9]]) as i32);

    // Parse end_pts
    let mut p = 10usize;
    let mut n_pts: usize = 0;
    for i in 0..nc as usize {
        let ep = u16::from_be_bytes([d[p], d[p+1]]) as usize;
        p += 2;
        if i == nc as usize - 1 { n_pts = ep + 1; }
    }
    let n_ins = u16::from_be_bytes([d[p], d[p+1]]) as usize;
    p += 2 + n_ins;
    println!("nPts={} nIns={} flags at p={}", n_pts, n_ins, p);

    // Parse flags (exact Rust logic from parse_simple_glyph)
    let mut flags = Vec::with_capacity(n_pts);
    while flags.len() < n_pts {
        let flag = d[p]; p += 1;
        flags.push(flag);
        if flag & 0x08 != 0 {
            let repeat = d[p] as usize; p += 1;
            for _ in 0..repeat {
                if flags.len() >= n_pts { break; }
                flags.push(flag);
            }
        }
    }
    println!("Flags: {} bytes, expanded to {}", p - 10 - nc as usize*2 - 2 - n_ins, flags.len());
    println!("flag[0]=0x{0:02X} flag[1]=0x{1:02X} flag[2]=0x{2:02X}", flags[0], flags[1], flags[2]);

    // Decode X — EXACT Rust parse_simple_glyph logic
    let mut x: i32 = 0;
    println!("\nX decode (from p={}):", p);
    for i in 0..15.min(flags.len()) {
        let flag = flags[i];
        if flag & 0x02 != 0 {
            let dx = d[p] as i32; p += 1;
            if flag & 0x10 != 0 { x += dx; } else { x -= dx; }
            println!("  [{:2}] f=0x{:02X} SHORT d={:<6} x={:<6}", i, flag, if flag & 0x10 != 0 { dx } else { -dx }, x);
        } else if flag & 0x10 == 0 {
            x += i16::from_be_bytes([d[p], d[p+1]]) as i32;
            let delta = i16::from_be_bytes([d[p], d[p+1]]) as i32;
            p += 2;
            println!("  [{:2}] f=0x{:02X} LONG  d={:<6} x={:<6}", i, flag, delta, x);
        } else {
            println!("  [{:2}] f=0x{:02X} SAME             x={:<6}", i, flag, x);
        }
    }
    println!("\nFINAL: x[0]={}", x);
}
