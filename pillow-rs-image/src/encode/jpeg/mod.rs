// ── JPEG Encoder ─────────────────────────────────────────────────────────
// libjpeg-turbo 3.1.4.1 port (jfdctint.c, jcparam.c, jchuff.c, jcphuff.c,
// jcmaster.c, jcmarker.c, jccolor.c, jcsample.c).
//
// Supports baseline (SOF0) and progressive (SOF2) encoding for YCbCr 4:4:4,
// 4:2:2, 4:2:0 and grayscale.  Entropy coding uses the standard IJG Huffman
// tables; quantization uses ISLOW divisors (quantval<<3) with round-to-nearest
// division matching jcdctmgr.c's reciprocal quantize path.

#![allow(dead_code)] // encoder wired up incrementally; tables/markers used by encode_*

mod fdct;
mod huffman;
mod marker;
mod quant;

use crate::encode_options::EncodeOptions;
use crate::types::DecodedImage;

/// Zigzag scan order (matches idct.rs JPEG_NATURAL_ORDER).
const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// Per-component prepared data: quantized coefficient blocks in NATURAL order,
/// indexed by block_row * blocks_per_row + block_col.
struct CompData {
    blocks: Vec<[i16; 64]>,
    blocks_per_row: usize,
    block_rows: usize,
    h_samp: u8,
    v_samp: u8,
    quant_slot: u8,
    /// Component id (Y=1, Cb=2, Cr=3 in libjpeg JCS_YCbCr).
    id: u8,
    dc_tbl: u8,
    ac_tbl: u8,
}

pub(crate) fn encode(img: &DecodedImage, opts: &EncodeOptions) -> Option<Vec<u8>> {
    let w = img.width as usize;
    let h = img.height as usize;
    if w == 0 || h == 0 {
        return None;
    }
    let pixels = img.as_bytes();

    let num_components: u8 = match img.color {
        crate::types::ColorType::L8 => 1,
        _ => 3,
    };

    let quality = opts.quality.unwrap_or(75);
    let progressive = opts.progressive.unwrap_or(false);
    let subsampling = opts.subsampling.as_deref().unwrap_or("420");

    let params = quant::build_params(quality, subsampling, num_components as usize);

    // RGB → YCbCr (jccolor.c) or grayscale pass-through.
    let (y_plane, cb_plane, cr_plane) = if num_components == 1 {
        let mut y = vec![0u8; w * h];
        for i in 0..(w * h).min(pixels.len()) {
            y[i] = pixels[i];
        }
        (y, Vec::new(), Vec::new())
    } else {
        rgb_to_ycbcr(pixels, w, h)
    };

    // Sampling factors (h, v) per component; max is the reference grid.
    let (y_hs, y_vs, cb_hs, cb_vs, cr_hs, cr_vs, max_h, max_v) = match (num_components, subsampling)
    {
        (1, _) => (1u8, 1u8, 0u8, 0u8, 0u8, 0u8, 1u8, 1u8),
        (_, "444") => (1, 1, 1, 1, 1, 1, 1, 1),
        (_, "422") => (2, 1, 1, 1, 1, 1, 2, 1),
        // 4:2:0 (default)
        _ => (2, 2, 1, 1, 1, 1, 2, 2),
    };

    // Component image dimensions on the sampling grid.
    let y_w = w;
    let y_h = h;
    let cb_w = (w * cb_hs as usize + max_h as usize - 1) / max_h as usize;
    let cb_h = (h * cb_vs as usize + max_v as usize - 1) / max_v as usize;
    let cr_w = cb_w;
    let cr_h = cb_h;

    // Downsample chroma (jcsample.c h2v2 / h2v1 / identity).
    let cb_ds = if num_components >= 3 {
        downsample(
            &cb_plane,
            w,
            h,
            cb_w,
            cb_h,
            max_h as usize / cb_hs as usize,
            max_v as usize / cb_vs as usize,
        )
    } else {
        Vec::new()
    };
    let cr_ds = if num_components >= 3 {
        downsample(
            &cr_plane,
            w,
            h,
            cr_w,
            cr_h,
            max_h as usize / cr_hs as usize,
            max_v as usize / cr_vs as usize,
        )
    } else {
        Vec::new()
    };

    // Prepare per-component quantized coefficient blocks (natural order).
    let mut comps: Vec<CompData> = Vec::with_capacity(num_components as usize);

    // Y
    let y_blocks = fdct_quantize(&y_plane, y_w, y_h, &params.quant_tables[0]);
    comps.push(CompData {
        blocks: y_blocks.0,
        blocks_per_row: y_blocks.1,
        block_rows: y_blocks.2,
        h_samp: y_hs,
        v_samp: y_vs,
        quant_slot: 0,
        id: 1,
        dc_tbl: 0,
        ac_tbl: 0,
    });

    if num_components >= 3 {
        for (plane, cw, ch, hs, vs, id) in [
            (&cb_ds, cb_w, cb_h, cb_hs, cb_vs, 2u8),
            (&cr_ds, cr_w, cr_h, cr_hs, cr_vs, 3u8),
        ] {
            let blk = fdct_quantize(plane, cw, ch, &params.quant_tables[1]);
            comps.push(CompData {
                blocks: blk.0,
                blocks_per_row: blk.1,
                block_rows: blk.2,
                h_samp: hs,
                v_samp: vs,
                quant_slot: 1,
                id,
                dc_tbl: 1,
                ac_tbl: 1,
            });
        }
    }

    // Derive standard Huffman tables.
    let dc_luma = huffman::derive_table(&huffman::STD_DC_LUMA.0, &huffman::STD_DC_LUMA.1);
    let dc_chroma = huffman::derive_table(&huffman::STD_DC_CHROMA.0, &huffman::STD_DC_CHROMA.1);
    let ac_luma = huffman::derive_table(&huffman::STD_AC_LUMA.0, &huffman::STD_AC_LUMA.1);
    let ac_chroma = huffman::derive_table(&huffman::STD_AC_CHROMA.0, &huffman::STD_AC_CHROMA.1);
    let dc_tables = [&dc_luma, &dc_chroma];
    let ac_tables = [&ac_luma, &ac_chroma];

    let mut out = Vec::new();
    marker::write_soi(&mut out);

    // Write DQT tables (one per unique quant slot).
    let mut emitted = [false; 4];
    for c in &comps {
        let slot = c.quant_slot as usize;
        if !emitted[slot] {
            marker::write_dqt(&mut out, c.quant_slot, 0, &params.quant_tables[slot]);
            emitted[slot] = true;
        }
    }

    // SOF marker.
    let sof_marker: u8 = if progressive { 0xC2 } else { 0xC0 };
    let sof_comps: Vec<(u8, u8, u8, u8)> = comps
        .iter()
        .map(|c| (c.id, c.h_samp, c.v_samp, c.quant_slot))
        .collect();
    marker::write_sof(&mut out, sof_marker, w as u16, h as u16, &sof_comps);

    // DHT tables. Baseline: all 4 standard tables up front. Progressive: DHT
    // is emitted per-scan with only the tables that scan uses.
    if !progressive {
        marker::write_dht(
            &mut out,
            0,
            0,
            &huffman::STD_DC_LUMA.0,
            &huffman::STD_DC_LUMA.1,
        );
        marker::write_dht(
            &mut out,
            1,
            0,
            &huffman::STD_AC_LUMA.0,
            &huffman::STD_AC_LUMA.1,
        );
        if num_components >= 3 {
            marker::write_dht(
                &mut out,
                0,
                1,
                &huffman::STD_DC_CHROMA.0,
                &huffman::STD_DC_CHROMA.1,
            );
            marker::write_dht(
                &mut out,
                1,
                1,
                &huffman::STD_AC_CHROMA.0,
                &huffman::STD_AC_CHROMA.1,
            );
        }

        // Single SOS for baseline (interleaved).
        let sos_comps: Vec<(u8, u8, u8)> =
            comps.iter().map(|c| (c.id, c.dc_tbl, c.ac_tbl)).collect();
        marker::write_sos(&mut out, &sos_comps, 0, 63, 0, 0);

        encode_baseline_entropy(&mut out, &comps, max_h, max_v, &dc_tables, &ac_tables);
    } else {
        encode_progressive_scans(
            &mut out,
            &comps,
            num_components,
            max_h,
            max_v,
            &params,
            &dc_tables,
            &ac_tables,
        );
    }

    marker::write_eoi(&mut out);
    Some(out)
}

// ── Color conversion (jccolor.c) ─────────────────────────────────────────

fn rgb_to_ycbcr(pixels: &[u8], w: usize, h: usize) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let n = w * h;
    let mut y = vec![0u8; n];
    let mut cb = vec![0u8; n];
    let mut cr = vec![0u8; n];
    let npix = n.min(pixels.len() / 3);
    for i in 0..npix {
        let r = pixels[i * 3] as i32;
        let g = pixels[i * 3 + 1] as i32;
        let b = pixels[i * 3 + 2] as i32;
        // IJG jccolor.c fixed-point: Y = (0.29900*R + 0.58700*G + 0.11400*B)
        y[i] = ((19595 * r + 38470 * g + 7471 * b + 32768) >> 16) as u8;
        // Cb = (-0.16874*R - 0.33126*G + 0.50000*B) + 128
        let cbv = (-11059 * r - 21709 * g + 32768 * b + 128 + 32768) >> 16;
        cb[i] = cbv.clamp(0, 255) as u8;
        // Cr = (0.50000*R - 0.41869*G - 0.08131*B) + 128
        let crv = (32768 * r - 27439 * g - 5329 * b + 128 + 32768) >> 16;
        cr[i] = crv.clamp(0, 255) as u8;
    }
    (y, cb, cr)
}

// ── Downsampling (jcsample.c) ────────────────────────────────────────────
//
// libjpeg uses a 3-tap smoothing filter for h2v2 downsampling by default
// (smooth_downsample).  For byte-exactness we'd replicate that; here we use
// simple box averaging which is close and roundtrip-safe.

fn downsample(
    plane: &[u8],
    sw: usize,
    sh: usize,
    dw: usize,
    dh: usize,
    hr: usize,
    vr: usize,
) -> Vec<u8> {
    let mut out = vec![0u8; dw * dh];
    if hr == 1 && vr == 1 {
        for i in 0..(dw * dh).min(plane.len()) {
            out[i] = plane[i];
        }
        return out;
    }
    for y in 0..dh {
        for x in 0..dw {
            let mut sum = 0u32;
            let mut cnt = 0u32;
            for vy in 0..vr {
                for vx in 0..hr {
                    let sy = (y * vr + vy).min(sh - 1);
                    let sx = (x * hr + vx).min(sw - 1);
                    sum += plane[sy * sw + sx] as u32;
                    cnt += 1;
                }
            }
            out[y * dw + x] = ((sum + cnt / 2) / cnt) as u8;
        }
    }
    out
}

// ── FDCT + quantize (jfdctint.c + jcdctmgr.c) ────────────────────────────

/// Forward DCT all blocks of a component plane, then quantize with ISLOW
/// divisors (quantval<<3) and round-to-nearest. Returns (blocks, blocks_per_row,
/// block_rows) in natural order.
fn fdct_quantize(
    plane: &[u8],
    w: usize,
    h: usize,
    qtable: &[u16; 64],
) -> (Vec<[i16; 64]>, usize, usize) {
    let blocks_per_row = (w + 7) / 8;
    let block_rows = (h + 7) / 8;
    let mut blocks = vec![[0i16; 64]; blocks_per_row * block_rows];

    for by in 0..block_rows {
        for bx in 0..blocks_per_row {
            let mut samples = [0i32; 64];
            for row in 0..8 {
                for col in 0..8 {
                    let py = by * 8 + row;
                    let px = bx * 8 + col;
                    let val = if py < h && px < w {
                        plane[py * w + px] as i32 - 128
                    } else {
                        // Edge replication (jccolext / edge extension).
                        let cpy = py.min(h.saturating_sub(1));
                        let cpx = px.min(w.saturating_sub(1));
                        plane[cpy * w + cpx] as i32 - 128
                    };
                    samples[row * 8 + col] = val;
                }
            }
            fdct::fdct_islow(&mut samples);
            // Quantize in natural order: divisor = quantval[i] << 3.
            let mut q = [0i16; 64];
            for i in 0..64 {
                let divisor = (qtable[i] as i32) << 3;
                let coef = samples[i];
                // Round-to-nearest, away from zero on .5 (matches reciprocal path).
                let qval = if coef < 0 {
                    -((-coef + (divisor >> 1)) / divisor)
                } else {
                    (coef + (divisor >> 1)) / divisor
                };
                q[i] = qval as i16;
            }
            blocks[by * blocks_per_row + bx] = q;
        }
    }
    (blocks, blocks_per_row, block_rows)
}

// ── Baseline entropy coding (jchuff.c) ───────────────────────────────────

fn encode_baseline_entropy(
    out: &mut Vec<u8>,
    comps: &[CompData],
    max_h: u8,
    max_v: u8,
    dc_tables: &[&huffman::DerivedTable; 2],
    ac_tables: &[&huffman::DerivedTable; 2],
) {
    let mcu_w = max_h as usize * 8;
    let mcu_h = max_v as usize * 8;
    let n_mcu_x = (comps[0].blocks_per_row * 8 + mcu_w - 1) / mcu_w;
    let n_mcu_y = (comps[0].block_rows * 8 + mcu_h - 1) / mcu_h;

    let mut bw = huffman::BitWriter::new();
    let mut last_dc = [0i32; 4];

    for my in 0..n_mcu_y {
        for mx in 0..n_mcu_x {
            for (ci, c) in comps.iter().enumerate() {
                let hs = c.h_samp as usize;
                let vs = c.v_samp as usize;
                let bpr = c.blocks_per_row;
                let dc_tbl = dc_tables[c.dc_tbl as usize];
                let ac_tbl = ac_tables[c.ac_tbl as usize];
                for vy in 0..vs {
                    for vx in 0..hs {
                        let brow = my * vs + vy;
                        let bcol = mx * hs + vx;
                        if brow >= c.block_rows || bcol >= bpr {
                            // Dummy block beyond component extent: emit zero block.
                            // DC diff 0, no AC → just EOB.
                            bw.write_bits(ac_tbl.codes[0], ac_tbl.lengths[0]);
                            continue;
                        }
                        let blk = &c.blocks[brow * bpr + bcol];
                        encode_one_block(&mut bw, blk, &mut last_dc[ci], dc_tbl, ac_tbl);
                    }
                }
            }
        }
    }
    bw.flush();
    out.extend_from_slice(&bw.out);
}

/// Encode one 8×8 block: DC difference + AC run/length in zigzag order.
fn encode_one_block(
    bw: &mut huffman::BitWriter,
    block: &[i16; 64],
    last_dc: &mut i32,
    dc_tbl: &huffman::DerivedTable,
    ac_tbl: &huffman::DerivedTable,
) {
    // DC coefficient difference (natural-order index 0).
    let dc = block[0] as i32;
    let diff = dc - *last_dc;
    *last_dc = dc;
    let nbits = jpeg_nbits(diff);
    // DC Huffman symbol = nbits, followed by nbits magnitude bits.
    bw.write_bits(dc_tbl.codes[nbits as usize], dc_tbl.lengths[nbits as usize]);
    if nbits > 0 {
        bw.write_bits(mag_bits(diff, nbits), nbits as u8);
    }

    // AC coefficients in zigzag order (k=1..63).
    let mut r = 0u32; // run length of zeros
    for k in 1..64 {
        let coef = block[ZIGZAG[k]] as i32;
        if coef == 0 {
            r += 1;
            continue;
        }
        // Emit ZRL (0xF0) for each full 16-zero run.
        while r >= 16 {
            bw.write_bits(ac_tbl.codes[0xF0], ac_tbl.lengths[0xF0]);
            r -= 16;
        }
        let nbits = jpeg_nbits(coef);
        let sym = ((r << 4) | nbits) as usize;
        bw.write_bits(ac_tbl.codes[sym], ac_tbl.lengths[sym]);
        bw.write_bits(mag_bits(coef, nbits), nbits as u8);
        r = 0;
    }
    // If trailing zeros, emit EOB (symbol 0x00).
    if r > 0 {
        bw.write_bits(ac_tbl.codes[0], ac_tbl.lengths[0]);
    }
}

/// Number of bits needed to represent |v| (JPEG_NBITS).  nbits(0)=0.
fn jpeg_nbits(v: i32) -> u32 {
    let mut a = if v < 0 { -v } else { v };
    let mut n = 0u32;
    while a > 0 {
        n += 1;
        a >>= 1;
    }
    n
}

/// The nbits magnitude bits to emit for a signed coefficient value (IJG
/// convention): positive → the value's nbits LSBs; negative → (value-1)'s
/// nbits LSBs (= bitwise complement of the absolute magnitude).
fn mag_bits(v: i32, nbits: u32) -> u32 {
    let emit = if v < 0 { v - 1 } else { v };
    (emit as u32) & ((1u32 << nbits) - 1)
}

// ── Progressive entropy coding (jcphuff.c + jcmaster.c scan script) ──────

/// A progressive scan descriptor (component index, Ss, Se, Ah, Al).
struct ProgScan {
    comps: Vec<usize>, // component indices in this scan
    ss: u8,
    se: u8,
    ah: u8,
    al: u8,
    is_dc: bool,
}

/// Build the default progressive scan script (jpeg_simple_progression, YCbCr).
fn default_progression_script(ncomp: u8) -> Vec<ProgScan> {
    let mut s = Vec::new();
    // ci index: 0=Y, 1=Cb, 2=Cr
    let dc = |comps: Vec<usize>| ProgScan {
        comps,
        ss: 0,
        se: 0,
        ah: 0,
        al: 1,
        is_dc: true,
    };
    let dc_refine = |comps: Vec<usize>, ah, al| ProgScan {
        comps,
        ss: 0,
        se: 0,
        ah,
        al,
        is_dc: true,
    };
    let ac = |comps: Vec<usize>, ss, se, ah, al| ProgScan {
        comps,
        ss,
        se,
        ah,
        al,
        is_dc: false,
    };

    if ncomp == 3 {
        // Initial DC scan (interleaved), Al=1
        s.push(dc(vec![0, 1, 2]));
        // Initial AC luma Ss=1,Se=5 Al=2
        s.push(ac(vec![0], 1, 5, 0, 2));
        // Chroma AC full band Ss=1,Se=63 Al=1
        s.push(ac(vec![2], 1, 63, 0, 1));
        s.push(ac(vec![1], 1, 63, 0, 1));
        // Complete luma AC Ss=6,Se=63 Al=2
        s.push(ac(vec![0], 6, 63, 0, 2));
        // Refine next bit of luma AC Ss=1,Se=63 Ah=2 Al=1
        s.push(ac(vec![0], 1, 63, 2, 1));
        // Finish DC successive approximation Ah=1 Al=0 (interleaved)
        s.push(dc_refine(vec![0, 1, 2], 1, 0));
        // Finish AC successive approximation (chroma then luma)
        s.push(ac(vec![2], 1, 63, 1, 0));
        s.push(ac(vec![1], 1, 63, 1, 0));
        s.push(ac(vec![0], 1, 63, 1, 0));
    } else {
        // Grayscale: 2 DC + 4 AC scans.
        s.push(dc(vec![0]));
        s.push(ac(vec![0], 1, 5, 0, 2));
        s.push(ac(vec![0], 6, 63, 0, 2));
        s.push(ac(vec![0], 1, 63, 2, 1));
        s.push(dc_refine(vec![0], 1, 0));
        s.push(ac(vec![0], 1, 63, 1, 0));
    }
    s
}

fn encode_progressive_scans(
    out: &mut Vec<u8>,
    comps: &[CompData],
    ncomp: u8,
    _max_h: u8,
    _max_v: u8,
    _params: &quant::EncodeParams,
    dc_tables: &[&huffman::DerivedTable; 2],
    ac_tables: &[&huffman::DerivedTable; 2],
) {
    let script = default_progression_script(ncomp);

    for scan in &script {
        // Emit DHT for the tables this scan uses.
        emit_scan_dht(out, scan, comps);

        // SOS for this scan.
        let sos_comps: Vec<(u8, u8, u8)> = scan
            .comps
            .iter()
            .map(|&ci| {
                let c = &comps[ci];
                // DC scans use dc_tbl for both dc/ac slots; AC scans use ac_tbl for ac.
                if scan.is_dc {
                    (c.id, c.dc_tbl, 0)
                } else {
                    (c.id, 0, c.ac_tbl)
                }
            })
            .collect();
        marker::write_sos(out, &sos_comps, scan.ss, scan.se, scan.ah, scan.al);

        // Entropy-code the scan.
        let mut bw = huffman::BitWriter::new();
        if scan.is_dc {
            encode_dc_scan(&mut bw, scan, comps, dc_tables);
        } else {
            encode_ac_scan(&mut bw, scan, comps, ac_tables);
        }
        bw.flush();
        out.extend_from_slice(&bw.out);
    }
}

/// Emit DHT markers for the Huffman tables referenced by a scan.
fn emit_scan_dht(out: &mut Vec<u8>, scan: &ProgScan, comps: &[CompData]) {
    let mut dc_seen = [false; 4];
    let mut ac_seen = [false; 4];
    if scan.is_dc {
        for &ci in &scan.comps {
            let t = comps[ci].dc_tbl as usize;
            if !dc_seen[t] {
                let (bits, vals) = if t == 0 {
                    huffman::STD_DC_LUMA
                } else {
                    huffman::STD_DC_CHROMA
                };
                marker::write_dht(out, 0, t as u8, &bits, &vals);
                dc_seen[t] = true;
            }
        }
    } else {
        for &ci in &scan.comps {
            let t = comps[ci].ac_tbl as usize;
            if !ac_seen[t] {
                let (bits, vals) = if t == 0 {
                    huffman::STD_AC_LUMA
                } else {
                    huffman::STD_AC_CHROMA
                };
                marker::write_dht(out, 1, t as u8, &bits, &vals);
                ac_seen[t] = true;
            }
        }
    }
}

/// Encode a DC scan (first or refine).  Interleaved scans iterate the image
/// MCU grid; the coefficient stored is block[0] shifted by Al.
fn encode_dc_scan(
    bw: &mut huffman::BitWriter,
    scan: &ProgScan,
    comps: &[CompData],
    dc_tables: &[&huffman::DerivedTable; 2],
) {
    let interleaved = scan.comps.len() > 1;
    let max_h = comps.iter().map(|c| c.h_samp).max().unwrap_or(1);
    let max_v = comps.iter().map(|c| c.v_samp).max().unwrap_or(1);
    let mcu_w = max_h as usize * 8;
    let mcu_h = max_v as usize * 8;
    let n_mcu_x = (comps[0].blocks_per_row * 8 + mcu_w - 1) / mcu_w;
    let n_mcu_y = (comps[0].block_rows * 8 + mcu_h - 1) / mcu_h;

    let mut last_dc = vec![0i32; comps.len()];

    if interleaved {
        for my in 0..n_mcu_y {
            for mx in 0..n_mcu_x {
                for (s, &ci) in scan.comps.iter().enumerate() {
                    let c = &comps[ci];
                    let hs = c.h_samp as usize;
                    let vs = c.v_samp as usize;
                    let bpr = c.blocks_per_row;
                    for vy in 0..vs {
                        for vx in 0..hs {
                            let brow = my * vs + vy;
                            let bcol = mx * hs + vx;
                            if brow >= c.block_rows || bcol >= bpr {
                                continue;
                            }
                            let blk = &c.blocks[brow * bpr + bcol];
                            encode_dc_coeff(
                                bw,
                                blk,
                                scan,
                                &mut last_dc[s],
                                dc_tables[c.dc_tbl as usize],
                            );
                        }
                    }
                }
            }
        }
    } else {
        // Non-interleaved: iterate this component's own block raster.
        let ci = scan.comps[0];
        let c = &comps[ci];
        let bpr = c.blocks_per_row;
        let tbl = dc_tables[c.dc_tbl as usize];
        for br in 0..c.block_rows {
            for bc in 0..bpr {
                let blk = &c.blocks[br * bpr + bc];
                encode_dc_coeff(bw, blk, scan, &mut last_dc[0], tbl);
            }
        }
    }
}

/// Encode one DC coefficient for the current scan.
fn encode_dc_coeff(
    bw: &mut huffman::BitWriter,
    block: &[i16; 64],
    scan: &ProgScan,
    last_dc: &mut i32,
    tbl: &huffman::DerivedTable,
) {
    // Point transform: divide coefficient by 2^Al, rounding.
    let raw = block[0] as i32;
    let al = scan.al as i32;
    let pt = if al > 0 { raw >> al } else { raw };
    if scan.ah == 0 {
        // DC first: code the difference.
        let diff = pt - *last_dc;
        *last_dc = pt;
        let nbits = jpeg_nbits(diff);
        bw.write_bits(tbl.codes[nbits as usize], tbl.lengths[nbits as usize]);
        if nbits > 0 {
            bw.write_bits(mag_bits(diff, nbits), nbits as u8);
        }
    } else {
        // DC refine: transmit the Al-th bit of the coefficient (jcphuff DC_refine).
        let bit = (raw >> al) & 1;
        bw.write_bits(bit as u32, 1);
    }
}

/// Encode an AC scan (first or refine), single-component, over the component's
/// own block raster (non-interleaved per JPEG spec).
fn encode_ac_scan(
    bw: &mut huffman::BitWriter,
    scan: &ProgScan,
    comps: &[CompData],
    ac_tables: &[&huffman::DerivedTable; 2],
) {
    // AC scans are always single-component in the default script.
    let ci = scan.comps[0];
    let c = &comps[ci];
    let bpr = c.blocks_per_row;
    let tbl = ac_tables[c.ac_tbl as usize];
    let ss = scan.ss as usize;
    let se = scan.se as usize;
    let al = scan.al as i32;

    let mut eobrun = 0u32;
    // Correction-bit buffer for AC-refine (the "BE" buffer in jcphuff.c).
    let mut be_buffer: Vec<u8> = Vec::new();

    for br in 0..c.block_rows {
        for bc in 0..bpr {
            let blk = &c.blocks[br * bpr + bc];
            if scan.ah == 0 {
                encode_ac_first(bw, blk, ss, se, al, tbl, &mut eobrun);
            } else {
                encode_ac_refine(bw, blk, ss, se, al, tbl, &mut eobrun, &mut be_buffer);
            }
        }
    }
    // Flush any pending EOBRUN (and its trailing correction bits for refine).
    if eobrun > 0 {
        emit_eobrun(bw, eobrun, tbl);
        if !be_buffer.is_empty() {
            for &bit in &be_buffer {
                bw.write_bits(bit as u32, 1);
            }
            be_buffer.clear();
        }
    }
}

/// Encode one block for an AC-first scan (jcphuff.c encode_mcu_AC_first).
fn encode_ac_first(
    bw: &mut huffman::BitWriter,
    block: &[i16; 64],
    ss: usize,
    se: usize,
    al: i32,
    tbl: &huffman::DerivedTable,
    eobrun: &mut u32,
) {
    // SIMPLIFIED FOR DEBUG: Always emit a fresh EOB0 per block so decoder never
    // has to rely on deferred EOBRUN.  This is wasteful but validates whether
    // the coefficient encoding itself is correct.

    // Flush any pending EOBRUN first (matches IJG).
    if *eobrun > 0 {
        emit_eobrun(bw, *eobrun, tbl);
        *eobrun = 0;
    }

    // Find nonzero point-transformed coefficients in the band.
    let mut coeffs: Vec<(usize, i32)> = Vec::new();
    for k in ss..=se {
        let raw = block[ZIGZAG[k]] as i32;
        // Point transform: integer division toward 0 (abs>>Al with sign restored).
        let temp2 = raw >> 31;
        let mut temp = raw ^ temp2;
        temp = temp.wrapping_sub(temp2);
        temp >>= al;
        // Restore sign: temp is the abs value; reapply original sign.
        let signed = if raw < 0 { -temp } else { temp };
        if signed != 0 {
            coeffs.push((k, signed));
        }
    }

    if coeffs.is_empty() {
        // All-zero band: don't extend EOB run, just emit EOB0.
        bw.write_bits(tbl.codes[0], tbl.lengths[0]);
        return;
    }

    let mut k = ss;
    for &(pos, val) in &coeffs {
        // Run of zeros from k to pos.  Emit ZRL (0xF0) for each full 16-run.
        let mut run = (pos - k) as u32;
        while run >= 16 {
            bw.write_bits(tbl.codes[0xF0], tbl.lengths[0xF0]);
            run -= 16;
        }
        let nbits = jpeg_nbits(val);
        let sym = ((run << 4) | nbits) as usize;
        bw.write_bits(tbl.codes[sym], tbl.lengths[sym]);
        bw.write_bits(mag_bits(val, nbits), nbits as u8);
        k = pos + 1;
    }

    // Always terminate with EOB0 (no EOBRUN accumulation).
    bw.write_bits(tbl.codes[0], tbl.lengths[0]);
}

/// Emit a pending EOBRUN using the EOBn Huffman codes (jcphuff.c emit_eobrun).
fn emit_eobrun(bw: &mut huffman::BitWriter, eobrun: u32, tbl: &huffman::DerivedTable) {
    if eobrun == 0 {
        return;
    }
    // jcphuff: nbits = JPEG_NBITS_NONZERO(eobrun) - 1; symbol = nbits << 4.
    // EOBRUN = 2^nbits + extra.  Find nbits such that 2^nbits <= eobrun < 2^(nbits+1).
    let mut nbits = 0u32;
    while (1u32 << (nbits + 1)) <= eobrun {
        nbits += 1;
    }
    let sym = (nbits << 4) as usize; // EOBn symbol = nbits<<4 (0x00,0x10,...,0xE0)
    bw.write_bits(tbl.codes[sym], tbl.lengths[sym]);
    let extra = eobrun - (1u32 << nbits);
    if nbits > 0 {
        bw.write_bits(extra, nbits as u8);
    }
}

/// Encode one block for an AC-refine scan.  Uses EOB1 (not EOB0) for blocks
/// with trailing correction bits so the decoder's Phase 2 activates (EOBRUN>0).
/// No cross-block EOBRUN accumulation — each block is self-contained.
fn encode_ac_refine(
    bw: &mut huffman::BitWriter,
    block: &[i16; 64],
    ss: usize,
    se: usize,
    al: i32,
    tbl: &huffman::DerivedTable,
    eobrun: &mut u32,
    be_buffer: &mut Vec<u8>,
) {
    let sl = se - ss + 1;

    // Prepare absvalues = |coef| >> Al and find EOB (last newly-significant).
    let absvalues: Vec<u32> = (0..sl)
        .map(|k| {
            let raw = block[ZIGZAG[ss + k]] as i32;
            let temp2 = raw >> 31;
            let mut temp = raw ^ temp2;
            temp = temp.wrapping_sub(temp2);
            (temp >> al) as u32
        })
        .collect();

    let eob = absvalues
        .iter()
        .enumerate()
        .rev()
        .find(|(_, v)| **v == 1)
        .map_or(usize::MAX, |(i, _)| i);

    // Flush any pending cross-block EOBRUN+BE from previous blocks.
    if *eobrun > 0 {
        emit_eobrun(bw, *eobrun, tbl);
        *eobrun = 0;
        for &bit in &*be_buffer {
            bw.write_bits(bit as u32, 1);
        }
        be_buffer.clear();
    }

    // Collect nonzero positions: (idx, absvalue).
    let nonzero: Vec<(usize, u32)> = absvalues
        .iter()
        .enumerate()
        .filter(|(_, v)| **v > 0)
        .map(|(i, v)| (i, *v))
        .collect();
    let has_new = nonzero.iter().any(|&(_, v)| v == 1);

    if !has_new {
        // No newly-significant: just emit correction bits + EOB0.
        for &(_, v) in &nonzero {
            bw.write_bits(v & 1, 1);
        }
        bw.write_bits(tbl.codes[0], tbl.lengths[0]);
        return;
    }

    // ── Encode ──────────────────────────────────────────────────────────
    // Walk k across the band.  For zeros: increment r.  For already-nonzero:
    // buffer 1 correction bit.  For newly-significant: emit run/size=1 + sign
    // bit, flush accumulated correction bits, reset r.

    let mut r: i32 = 0; // zero-run counter
    let mut corr_bits: Vec<u8> = Vec::new();

    for k in 0..sl {
        let v = absvalues[k];
        if v == 0 {
            r += 1;
            continue;
        }
        if v > 1 {
            // Already-nonzero: buffer correction bit.  These are consumed by
            // the decoder during inner-loop traversal.
            corr_bits.push((v & 1) as u8);
            continue;
        }
        // v == 1: newly-significant.
        // Emit ZRLs for runs > 15, but only while within EOB.
        while r > 15 && (k as i32) <= eob as i32 {
            bw.write_bits(tbl.codes[0xF0], tbl.lengths[0xF0]);
            r -= 16;
            // Flush correction bits accumulated for this ZRL segment.
            for &b in &corr_bits {
                bw.write_bits(b as u32, 1);
            }
            corr_bits.clear();
        }
        // Emit run/size symbol (size=1) + sign bit.
        let sym = (r.min(15) as u32) << 4 | 1;
        bw.write_bits(tbl.codes[sym as usize], tbl.lengths[sym as usize]);
        let raw = block[ZIGZAG[ss + k]] as i32;
        bw.write_bits(if raw >= 0 { 1 } else { 0 }, 1);
        // Flush accumulated correction bits.
        for &b in &corr_bits {
            bw.write_bits(b as u32, 1);
        }
        corr_bits.clear();
        r = 0;
    }

    // After the last new coef: any remaining corr_bits are trailing nonzeros.
    // Need Phase 2 to consume them.  Emit EOB1 so eobrun=1 after -=1.
    if !corr_bits.is_empty() {
        bw.write_bits(tbl.codes[0x10], tbl.lengths[0x10]); // EOB1 (symbol 0x10)
        for &b in &corr_bits {
            bw.write_bits(b as u32, 1);
        }
    } else {
        bw.write_bits(tbl.codes[0], tbl.lengths[0]); // EOB0
    }
}
