// ── Progressive JPEG Reconstruction ──────────────────────────────────────
// Port of libjpeg-turbo 3.1.4.1 jdphuff.c progressive Huffman decoding.
//
// The four scan routines (DC_first, DC_refine, AC_first, AC_refine)
// operate ONE BLOCK AT A TIME, matching IJG's decode_mcu_xxx function pattern.
// State (EOBRUN, DC predictors) is managed in ProgressiveState and
// persists across blocks within the entire scan segment.
//
// Key difference from the old implementation: blocks are processed in
// IJG-specified MCU_membership order. For 4:2:0 Y component scans (Ah!=0),
// this means 4 blocks per MCU (2×2), processed in raster order within the MCU:
// (0,0), (0,1), (1,0), (1,1).

use super::bit_reader::BitReader;
use super::decode::extract_entropy_segments;
use super::idct::{JPEG_NATURAL_ORDER, YccColorConverter, extend, jpeg_idct_islow};
use super::parser::JpegInfo;
use super::upsample::{crop_component, fancy_upsample};
use crate::types::{ColorType, DecodedImage};

struct ProgressiveState {
    eobrun: u32,
    dc_predictors: Vec<i32>,
}

impl ProgressiveState {
    fn new(num_components: usize) -> Self {
        ProgressiveState {
            eobrun: 0,
            dc_predictors: vec![0; num_components],
        }
    }
    fn reset(&mut self) {
        self.eobrun = 0;
        for p in &mut self.dc_predictors {
            *p = 0;
        }
    }
}

/// Process one DC-first block (decode_mcu_DC_first).
fn dc_first_block(
    br: &mut BitReader,
    dc_table: &super::huffman::HuffTable,
    dc_pred: &mut i32,
    al: u8,
) -> Option<i32> {
    let dc_cat = dc_table.decode(br)?;
    if dc_cat > 0 {
        let bits = br.read_bits(dc_cat as u32)?;
        *dc_pred += extend(bits, dc_cat);
    }
    Some(*dc_pred << al)
}

/// Process one DC-refinement block (decode_mcu_DC_refine).
fn dc_refine_block(coeff: &mut i32, p1: i32) {
    // One more bit of precision.  The caller reads the bit.
    *coeff |= p1;
}

/// Process one AC-first block (decode_mcu_AC_first).
/// Updates eobrun.  Returns the number of coefficients decoded (for debugging).
fn ac_first_block(
    br: &mut BitReader,
    ac_table: &super::huffman::HuffTable,
    ss: u8,
    se: u8,
    al: u8,
    coeffs: &mut [i32; 64],
    eobrun: &mut u32,
) -> Option<usize> {
    if *eobrun > 0 {
        *eobrun -= 1;
        return Some(0); // entire block zero in this band
    }
    let ss = ss as usize;
    let se = se as usize;
    let mut k = ss;
    let mut ncoeffs = 0;
    while k <= se && k < 64 {
        let sym = ac_table.decode(br)?;
        let run = (sym >> 4) as usize;
        let size = sym & 0x0F;
        if size == 0 {
            if run == 15 {
                k += 16; // ZRL
                continue;
            }
            // EOB: EOBRUN = (1<<run) + extra_bits
            *eobrun = 1u32 << run;
            if run > 0 {
                *eobrun += br.read_bits(run as u32)?;
            }
            *eobrun -= 1; // this block consumes one from the run
            break;
        }
        // Coefficient at position k + run
        k += run;
        if k > se || k >= 64 {
            break;
        }
        let bits = br.read_bits(size as u32)?;
        coeffs[k] = extend(bits, size) << al;
        ncoeffs += 1;
        k += 1;
    }
    Some(ncoeffs)
}

/// Process one AC-refinement block (decode_mcu_AC_refine).
/// Updates eobrun.
fn ac_refine_block(
    br: &mut BitReader,
    ac_table: &super::huffman::HuffTable,
    ss: u8,
    se: u8,
    al: u8,
    coeffs: &mut [i32; 64],
    eobrun: &mut u32,
) -> Option<()> {
    let p1 = 1i32 << al;
    let m1 = (-1i32) << al;
    let ss = ss as usize;
    let se = se as usize;
    let mut k = ss;

    // Phase 1: Huffman decode when EOBRUN == 0
    if *eobrun == 0 {
        while k <= se && k < 64 {
            let sym = ac_table.decode(br)?;
            let mut r = (sym >> 4) as i32;
            let size = sym & 0x0F;

            // New coefficient value
            let new_val = if size != 0 {
                let bit = br.read_bits(1)?;
                Some(if bit != 0 { p1 } else { m1 })
            } else {
                if r != 15 {
                    *eobrun = 1u32 << r;
                    if r > 0 {
                        *eobrun += br.read_bits(r as u32)?;
                    }
                    break; // → Phase 2
                }
                None // ZRL
            };

            // do-while: traverse, refine non-zeros, count zeros
            loop {
                if k > se || k >= 64 {
                    break;
                }
                if coeffs[k] != 0 {
                    let bit = br.read_bits(1)?;
                    if bit != 0 && (coeffs[k] & p1) == 0 {
                        coeffs[k] += if coeffs[k] >= 0 { p1 } else { m1 };
                    }
                } else {
                    r -= 1;
                    if r < 0 {
                        break;
                    }
                }
                k += 1;
            }

            if let Some(val) = new_val {
                if k <= se && k < 64 {
                    coeffs[k] = val;
                }
            }
            k += 1;
        }
    }

    // Phase 2: EOBRUN handler — refine remaining non-zero coeffs
    if *eobrun > 0 {
        while k <= se && k < 64 {
            if coeffs[k] != 0 {
                let bit = br.read_bits(1)?;
                if bit != 0 && (coeffs[k] & p1) == 0 {
                    coeffs[k] += if coeffs[k] >= 0 { p1 } else { m1 };
                }
            }
            k += 1;
        }
        *eobrun -= 1;
    }

    Some(())
}

pub(super) fn progressive_reconstruct(info: &JpegInfo, data: &[u8]) -> Option<DecodedImage> {
    // Run the test on first call for debugging
    #[cfg(test)]
    {
        // This is a hack — we can't run tests from here.
        // Instead, the test module above will be compiled.
    }

    let mcu_width = (info.max_h_samp as u32) * 8;
    let mcu_height = (info.max_v_samp as u32) * 8;
    let num_mcus_x = ((info.width as u32) + mcu_width - 1) / mcu_width;
    let num_mcus_y = ((info.height as u32) + mcu_height - 1) / mcu_height;

    // Component buffer dimensions
    let comp_buf_width: Vec<usize> = info
        .components
        .iter()
        .map(|c| num_mcus_x as usize * c.h_samp as usize * 8)
        .collect();
    let comp_buf_height: Vec<usize> = info
        .components
        .iter()
        .map(|c| num_mcus_y as usize * c.v_samp as usize * 8)
        .collect();
    let comp_num_blocks: Vec<usize> = info
        .components
        .iter()
        .enumerate()
        .map(|(i, _)| (comp_buf_width[i] / 8) * (comp_buf_height[i] / 8))
        .collect();

    // Coefficient storage: [component][block_idx][64] in zigzag order
    let mut coeff_storage: Vec<Vec<[i32; 64]>> = info
        .components
        .iter()
        .enumerate()
        .map(|(i, _)| vec![[0i32; 64]; comp_num_blocks[i]])
        .collect();
    let mut comp_buffers: Vec<Vec<u8>> = info
        .components
        .iter()
        .enumerate()
        .map(|(i, _)| vec![128u8; comp_buf_width[i] * comp_buf_height[i]])
        .collect();

    // ── Process scans ────────────────────────────────────────────────────
    // Per IJG jcmaster.c per_scan_setup: a scan with a single component is
    // NON-interleaved — it iterates that component's own block raster (1 block
    // per MCU, MCUs_per_row = width_in_blocks). A scan with >1 component is
    // interleaved over the image MCU grid (max_h_samp*8 × max_v_samp*8), with
    // h_samp*v_samp blocks per component per MCU.  Failing to distinguish
    // these scrambles block order for subsampled components (e.g. 4:2:0 chroma).
    for scan in info.scans.iter() {
        let segs = extract_entropy_segments(data, scan.entropy_start, scan.entropy_end);
        if segs.segments.is_empty() {
            continue;
        }

        let is_dc_scan = scan.ss == 0 && scan.se == 0;
        let is_dc_first = is_dc_scan && scan.ah == 0;
        let is_dc_refine = is_dc_scan && scan.ah > 0;
        let is_ac_first = !is_dc_scan && scan.ah == 0;
        let is_ac_refine = !is_dc_scan && scan.ah > 0;

        let interleaved = scan.components.len() > 1;

        // For non-interleaved scans, the "MCU" is a single block of the one
        // component, iterated over that component's block grid.
        let (scan_mcus_x, scan_mcus_y): (usize, usize) = if interleaved {
            (num_mcus_x as usize, num_mcus_y as usize)
        } else {
            let ci = scan.components[0].comp_index;
            (comp_buf_width[ci] / 8, comp_buf_height[ci] / 8)
        };
        let scan_total_mcus = scan_mcus_x * scan_mcus_y;

        let mcus_per_seg = if info.restart_interval > 0 {
            info.restart_interval as usize
        } else {
            scan_total_mcus
        };

        // State persists across segments but resets at each restart
        let mut state = ProgressiveState::new(info.num_components as usize);

        for seg_idx in 0..segs.segments.len() {
            let (seg_start, seg_end) = segs.segments[seg_idx];
            let mut br = BitReader::new(data, seg_start, seg_end);
            let mcu_offset = seg_idx * mcus_per_seg;

            // Reset state at restart boundary (IJG process_restart)
            state.reset();

            for mcu_idx in 0..mcus_per_seg {
                let absolute_mcu = mcu_offset + mcu_idx;
                if absolute_mcu >= scan_total_mcus {
                    break;
                }
                let mcu_y = absolute_mcu / scan_mcus_x;
                let mcu_x = absolute_mcu % scan_mcus_x;

                for scan_comp in &scan.components {
                    let comp_idx = scan_comp.comp_index;
                    let comp = &info.components[comp_idx];
                    let blocks_per_row = comp_buf_width[comp_idx] / 8;

                    // Compute the list of block indices this MCU covers.
                    // Interleaved: h_samp × v_samp blocks offset by the MCU's
                    //   top-left block (mcu_x*h_samp, mcu_y*v_samp).
                    // Non-interleaved: a single block at (mcu_x, mcu_y).
                    let block_list: Vec<usize> = if interleaved {
                        let mut v =
                            Vec::with_capacity((comp.h_samp as usize) * (comp.v_samp as usize));
                        for by in 0..comp.v_samp as usize {
                            for bx in 0..comp.h_samp as usize {
                                v.push(
                                    (mcu_y * comp.v_samp as usize + by) * blocks_per_row
                                        + (mcu_x * comp.h_samp as usize + bx),
                                );
                            }
                        }
                        v
                    } else {
                        vec![mcu_y * blocks_per_row + mcu_x]
                    };

                    if is_dc_first {
                        let dc_table = scan.dc_huff_tables[scan_comp.dc_tbl as usize].as_ref()?;
                        for &block_idx in &block_list {
                            coeff_storage[comp_idx][block_idx][0] = dc_first_block(
                                &mut br,
                                dc_table,
                                &mut state.dc_predictors[comp_idx],
                                scan.al,
                            )?;
                        }
                    } else if is_dc_refine {
                        let p1 = 1i32 << scan.al;
                        for &block_idx in &block_list {
                            // DC refine: read 1 bit, OR into coefficient
                            let bit = br.read_bits(1)?;
                            if bit != 0 {
                                dc_refine_block(&mut coeff_storage[comp_idx][block_idx][0], p1);
                            }
                        }
                    } else if is_ac_first {
                        let ac_table = scan.ac_huff_tables[scan_comp.ac_tbl as usize].as_ref()?;
                        for &block_idx in &block_list {
                            ac_first_block(
                                &mut br,
                                ac_table,
                                scan.ss,
                                scan.se,
                                scan.al,
                                &mut coeff_storage[comp_idx][block_idx],
                                &mut state.eobrun,
                            )?;
                        }
                    } else if is_ac_refine {
                        let ac_table = scan.ac_huff_tables[scan_comp.ac_tbl as usize].as_ref()?;
                        for &block_idx in &block_list {
                            ac_refine_block(
                                &mut br,
                                ac_table,
                                scan.ss,
                                scan.se,
                                scan.al,
                                &mut coeff_storage[comp_idx][block_idx],
                                &mut state.eobrun,
                            )?;
                        }
                    }
                }
            }
        }
    }

    // ── Final IDCT + assembly ────────────────────────────────────────────
    let mut block_natural = [0i32; 64];
    let mut workspace = [0i32; 64];
    for comp_idx in 0..info.num_components as usize {
        let comp = &info.components[comp_idx];
        let buf_w = comp_buf_width[comp_idx];
        let blocks_x = buf_w / 8;
        let quant_table = info.quant_tables[comp.quant_tbl as usize].as_ref()?;
        for (block_idx, coeffs) in coeff_storage[comp_idx].iter().enumerate() {
            for i in 0..64 {
                block_natural[JPEG_NATURAL_ORDER[i]] = coeffs[i] * quant_table[i] as i32;
            }
            jpeg_idct_islow(&mut block_natural, &mut workspace);
            let block_y = (block_idx / blocks_x) * 8;
            let block_x = (block_idx % blocks_x) * 8;
            for row in 0..8 {
                for col in 0..8 {
                    let px = block_natural[row * 8 + col].clamp(0, 255) as u8;
                    let bi = (block_y + row) * buf_w + (block_x + col);
                    if bi < comp_buffers[comp_idx].len() {
                        comp_buffers[comp_idx][bi] = px;
                    }
                }
            }
        }
    }

    // ── Assemble output ──────────────────────────────────────────────────
    let w = info.width as usize;
    let h = info.height as usize;
    let converter = YccColorConverter::new();
    if info.num_components == 1 {
        let y_buf = &comp_buffers[0];
        let y_w = comp_buf_width[0];
        let mut pixels = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                pixels.push(y_buf[y * y_w + x]);
            }
        }
        Some(DecodedImage::new(
            info.width as u32,
            info.height as u32,
            pixels,
            ColorType::L8,
        ))
    } else if info.num_components == 3 {
        let y_buf = &comp_buffers[0];
        let y_w = comp_buf_width[0];
        let h_ratio = (info.max_h_samp / info.components[1].h_samp) as usize;
        let v_ratio = (info.max_v_samp / info.components[1].v_samp) as usize;
        let chroma_src_w = (w + h_ratio - 1) / h_ratio;
        let chroma_src_h = (h + v_ratio - 1) / v_ratio;
        let cb_cropped = crop_component(
            &comp_buffers[1],
            comp_buf_width[1],
            comp_buf_height[1],
            chroma_src_w,
            chroma_src_h,
        );
        let cr_cropped = crop_component(
            &comp_buffers[2],
            comp_buf_width[2],
            comp_buf_height[2],
            chroma_src_w,
            chroma_src_h,
        );
        let cb_up = fancy_upsample(
            &cb_cropped,
            chroma_src_w,
            chroma_src_h,
            h_ratio,
            v_ratio,
            w,
            h,
        );
        let cr_up = fancy_upsample(
            &cr_cropped,
            chroma_src_w,
            chroma_src_h,
            h_ratio,
            v_ratio,
            w,
            h,
        );
        let chroma_stride = chroma_src_w * h_ratio;
        let mut pixels = Vec::with_capacity(w * h * 3);
        for y in 0..h {
            for x in 0..w {
                let (r, g, b) = converter.ycc_to_rgb(
                    y_buf[y * y_w + x],
                    cb_up[y * chroma_stride + x],
                    cr_up[y * chroma_stride + x],
                );
                pixels.push(r);
                pixels.push(g);
                pixels.push(b);
            }
        }
        Some(DecodedImage::new(
            info.width as u32,
            info.height as u32,
            pixels,
            ColorType::Rgb8,
        ))
    } else {
        None
    }
}
