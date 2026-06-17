use crate::types::{ColorType, DecodedImage};

use super::bit_reader::BitReader;
use super::decode::extract_entropy_segments;
use super::idct::{self, extend, jpeg_idct_islow, YccColorConverter};
use super::parser::JpegInfo;
use super::upsample::{crop_component, fancy_upsample};

pub(super) fn progressive_reconstruct(info: &JpegInfo, data: &[u8]) -> Option<DecodedImage> {
    let mcu_width = (info.max_h_samp as u32) * 8;
    let mcu_height = (info.max_v_samp as u32) * 8;
    let num_mcus_x = ((info.width as u32) + mcu_width - 1) / mcu_width;
    let num_mcus_y = ((info.height as u32) + mcu_height - 1) / mcu_height;

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

    // Allocate coefficient storage: [component][block_index][64 coefficients]
    // Coefficients are stored in ZIGZAG order (as decoded from Huffman symbols).
    // The final dequantize step converts zigzag → natural via JPEG_NATURAL_ORDER.
    let mut coeff_storage: Vec<Vec<[i32; 64]>> = info
        .components
        .iter()
        .enumerate()
        .map(|(i, _)| vec![[0i32; 64]; comp_num_blocks[i]])
        .collect();
    // Component pixel buffers (padded to MCU boundaries, filled during final IDCT pass)
    let mut comp_buffers: Vec<Vec<u8>> = info
        .components
        .iter()
        .enumerate()
        .map(|(i, _)| vec![128u8; comp_buf_width[i] * comp_buf_height[i]])
        .collect();

    // Process each scan in order
    for (_scan_idx, scan) in info.scans.iter().enumerate() {
        // Extract entropy segments (split at RST markers within the scan data)
        let segs = extract_entropy_segments(data, scan.entropy_start, scan.entropy_end);
        if segs.segments.is_empty() {
            continue;
        }

        let is_dc_scan = scan.ss == 0 && scan.se == 0;
        let is_dc_first = is_dc_scan && scan.ah == 0;
        let is_dc_refine = is_dc_scan && scan.ah > 0;
        let is_ac_first = !is_dc_scan && scan.ah == 0;
        let is_ac_refine = !is_dc_scan && scan.ah > 0;

        let mcus_in_segment = if info.restart_interval > 0 {
            info.restart_interval as usize
        } else {
            (num_mcus_x * num_mcus_y) as usize
        };
        let max_mcus = (num_mcus_x * num_mcus_y) as usize;

        // Per-component EOBRUN state: persists across ALL blocks in this scan segment.
        // Follows IJG savable_state.EOBRUN — NOT reset per-MCU or per-block.
        let mut ac_refine_eobrun: u32 = 0;
        // DC predictors (reset at each RST segment boundary)
        let mut dc_predictors: Vec<i32> = vec![0; info.num_components as usize];

        for seg_idx in 0..segs.segments.len() {
            let (seg_start, seg_end) = segs.segments[seg_idx];
            let mut br = BitReader::new(data, seg_start, seg_end);
            let mcu_offset = seg_idx * mcus_in_segment;

            for mcu_idx in 0..mcus_in_segment {
                let absolute_mcu = mcu_offset + mcu_idx;
                if absolute_mcu >= max_mcus {
                    break;
                }
                let mcu_y = absolute_mcu / num_mcus_x as usize;
                let mcu_x = absolute_mcu % num_mcus_x as usize;

                for scan_comp in &scan.components {
                    let comp_idx = scan_comp.comp_index;
                    let comp = &info.components[comp_idx];

                    if is_dc_first {
                        // ── DC first scan (Huffman-coded DC values) ──
                        let dc_table = scan.dc_huff_tables[scan_comp.dc_tbl as usize].as_ref()?;
                        for by in 0..comp.v_samp as usize {
                            for bx in 0..comp.h_samp as usize {
                                let block_idx = (mcu_y * comp.v_samp as usize + by)
                                    * (comp_buf_width[comp_idx] / 8)
                                    + (mcu_x * comp.h_samp as usize + bx);
                                let dc_cat = dc_table.decode(&mut br)?;
                                if dc_cat > 0 {
                                    let bits = br.read_bits(dc_cat as u32)?;
                                    dc_predictors[comp_idx] += extend(bits, dc_cat);
                                }
                                coeff_storage[comp_idx][block_idx][0] =
                                    dc_predictors[comp_idx] << scan.al;
                            }
                        }
                    } else if is_dc_refine {
                        // ── DC refinement scan (1 raw bit per block) ──
                        let bit = 1i32 << scan.al;
                        for by in 0..comp.v_samp as usize {
                            for bx in 0..comp.h_samp as usize {
                                let block_idx = (mcu_y * comp.v_samp as usize + by)
                                    * (comp_buf_width[comp_idx] / 8)
                                    + (mcu_x * comp.h_samp as usize + bx);
                                let raw_bit = br.read_bits(1)?;
                                if raw_bit != 0 {
                                    coeff_storage[comp_idx][block_idx][0] |= bit;
                                }
                            }
                        }
                    } else if is_ac_first {
                        // ── AC first scan — IJG jdphuff.c decode_mcu_AC_first ──
                        // EOBRUN shared with AC_refine, persists across blocks.
                        // When EOBRUN > 0: entire block is zero in this band → skip.
                        let ac_table = scan.ac_huff_tables[scan_comp.ac_tbl as usize].as_ref()?;
                        for by in 0..comp.v_samp as usize {
                            for bx in 0..comp.h_samp as usize {
                                let block_idx = (mcu_y * comp.v_samp as usize + by)
                                    * (comp_buf_width[comp_idx] / 8)
                                    + (mcu_x * comp.h_samp as usize + bx);
                                let mut k = scan.ss as usize;
                                let se = scan.se as usize;

                                if ac_refine_eobrun > 0 {
                                    ac_refine_eobrun -= 1;
                                    continue; // entire block is zero band
                                }

                                while k <= se && k < 64 {
                                    let sym = ac_table.decode(&mut br)?;
                                    let run = (sym >> 4) as usize;
                                    let size = (sym & 0x0F) as u8;
                                    if size == 0 && run == 15 {
                                        k += 16; continue; // ZRL
                                    }
                                    if size == 0 {
                                        // EOB: EOBRUN = (1<<run) | extra, consume one for this block
                                        ac_refine_eobrun = (1u32 << run) as u32;
                                        if run > 0 { ac_refine_eobrun |= br.read_bits(run as u32)?; }
                                        ac_refine_eobrun -= 1;
                                        break;
                                    }
                                    k += run;
                                    if k > se || k >= 64 { break; }
                                    let bits = br.read_bits(size as u32)?;
                                    let val = extend(bits, size);
                                    coeff_storage[comp_idx][block_idx][k] = val << scan.al;
                                    k += 1;
                                }
                            }
                        }
                    } else if is_ac_refine {
                        // ── AC refinement scan — matches IJG jdphuff.c decode_mcu_AC_refine ──
                        // Two-phase per-block: Phase 1 Huffman-decodes (only when EOBRUN==0),
                        // Phase 2 refines remaining non-zero coefficients and EOBRUN--.
                        // EOBRUN is a BLOCK counter (decremented once per block), not position.
                        // See: docs/jdphuff-vs-ours.md

                        let ac_table = scan.ac_huff_tables[scan_comp.ac_tbl as usize].as_ref()?;
                        let p1 = 1i32 << scan.al;
                        let m1 = (-1i32) << scan.al;
                        let ss = scan.ss as usize;
                        let se = scan.se as usize;
                        for by in 0..comp.v_samp as usize {
                            for bx in 0..comp.h_samp as usize {
                                let block_idx = (mcu_y * comp.v_samp as usize + by)
                                    * (comp_buf_width[comp_idx] / 8)
                                    + (mcu_x * comp.h_samp as usize + bx);
                                let coeffs = &mut coeff_storage[comp_idx][block_idx];
                                let mut k = ss;

                                // Phase 1: Huffman decode (only when EOBRUN == 0)
                                if ac_refine_eobrun == 0 {
                                    while k <= se && k < 64 {
                                        if coeffs[k] != 0 {
                                            // Refine existing non-zero
                                            let bit = br.read_bits(1)?;
                                            if bit != 0 {
                                                coeffs[k] += if coeffs[k] >= 0 { p1 } else { m1 };
                                            }
                                            k += 1;
                                        } else {
                                            let sym = ac_table.decode(&mut br)?;
                                            let run = (sym >> 4) as usize;
                                            let size = (sym & 0x0F) as u8;
                                            if size == 0 && run == 15 {
                                                // ZRL: skip 16 positions, refine non-zeros on the way
                                                let end = (k + 16).min(se + 1).min(64);
                                                while k < end {
                                                    if coeffs[k] != 0 {
                                                        let bit = br.read_bits(1)?;
                                                        if bit != 0 {
                                                            coeffs[k] += if coeffs[k] >= 0 { p1 } else { m1 };
                                                        }
                                                    }
                                                    k += 1;
                                                }
                                            } else if size == 0 {
                                                // EOB: EOBRUN = (1<<run) | extra_bits, then break → Phase 2
                                                ac_refine_eobrun = (1u32 << run) as u32;
                                                if run > 0 {
                                                    ac_refine_eobrun |= br.read_bits(run as u32)?;
                                                }
                                                break;
                                            } else {
                                                // New non-zero: skip `run` zeros (refining non-zeros), place ±p1
                                                let bit = br.read_bits(1)?;
                                                let val = if bit != 0 { p1 } else { m1 };
                                                let mut r = run;
                                                loop {
                                                    if k > se || k >= 64 { break; }
                                                    if coeffs[k] != 0 {
                                                        let bit = br.read_bits(1)?;
                                                        if bit != 0 {
                                                            coeffs[k] += if coeffs[k] >= 0 { p1 } else { m1 };
                                                        }
                                                    } else {
                                                        if r == 0 { break; }
                                                        r -= 1;
                                                    }
                                                    k += 1;
                                                }
                                                if k <= se && k < 64 {
                                                    coeffs[k] = val;
                                                    k += 1;
                                                }
                                            }
                                        }
                                    }
                                }

                                // Phase 2: EOBRUN handler — refine remaining non-zero coefficients
                                if ac_refine_eobrun > 0 {
                                    while k <= se && k < 64 {
                                        if coeffs[k] != 0 {
                                            let bit = br.read_bits(1)?;
                                            if bit != 0 {
                                                coeffs[k] += if coeffs[k] >= 0 { p1 } else { m1 };
                                            }
                                        }
                                        k += 1;
                                    }
                                    ac_refine_eobrun -= 1; // one BLOCK consumed
                                }
                            }
                        }
                    }
                }

                // RST handling at segment boundaries
                if mcu_idx + 1 >= mcus_in_segment && seg_idx + 1 < segs.segments.len() {
                    for pred in dc_predictors.iter_mut() {
                        *pred = 0;
                    }
                }
            }
        }
    }

    // ── Final pass: dequantize, IDCT, build component buffers ──
    let mut block_natural = [0i32; 64];
    let mut workspace = [0i32; 64];

    for comp_idx in 0..info.num_components as usize {
        let comp = &info.components[comp_idx];
        let buf_w = comp_buf_width[comp_idx];
        let blocks_x = buf_w / 8;
        let total_blocks = comp_num_blocks[comp_idx];
        let quant_table = info.quant_tables[comp.quant_tbl as usize].as_ref()?;

        for block_idx in 0..total_blocks {
            let coeffs = &coeff_storage[comp_idx][block_idx];
            for i in 0..64 {
                block_natural[idct::JPEG_NATURAL_ORDER[i]] = coeffs[i] * quant_table[i] as i32;
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

    // ── Assemble output image ──
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
        let h_ratio = info.max_h_samp / info.components[1].h_samp;
        let v_ratio = info.max_v_samp / info.components[1].v_samp;
        let h_ratio_us = h_ratio as usize;
        let v_ratio_us = v_ratio as usize;
        let chroma_src_w = (w + h_ratio_us - 1) / h_ratio_us;
        let chroma_src_h = (h + v_ratio_us - 1) / v_ratio_us;
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
            h_ratio_us,
            v_ratio_us,
            w,
            h,
        );
        let cr_up = fancy_upsample(
            &cr_cropped,
            chroma_src_w,
            chroma_src_h,
            h_ratio_us,
            v_ratio_us,
            w,
            h,
        );
        let mut pixels = Vec::with_capacity(w * h * 3);
        let chroma_stride = chroma_src_w * h_ratio_us;
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
