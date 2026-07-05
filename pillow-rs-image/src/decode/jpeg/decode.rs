use crate::types::{ColorType, DecodedImage};

use super::bit_reader::BitReader;
use super::huffman::HuffTable;
use super::idct::{self, YccColorConverter, extend, jpeg_idct_islow};
use super::parser::{JpegInfo, parse_jpeg};
use super::progressive::progressive_reconstruct;
use super::upsample::{crop_component, fancy_upsample};

// ── Entropy Decoding ──────────────────────────────────────────────────────

pub(super) fn decode_block(
    br: &mut BitReader,
    dc_table: &HuffTable,
    ac_table: &HuffTable,
    last_dc: &mut i32,
    block_zigzag: &mut [i32; 64],
) -> bool {
    for coeff in block_zigzag.iter_mut() {
        *coeff = 0;
    }

    let dc_cat = match dc_table.decode(br) {
        Some(cat) => cat,
        None => return false,
    };
    if dc_cat > 0 {
        let bits = match br.read_bits(dc_cat as u32) {
            Some(b) => b,
            None => return false,
        };
        *last_dc += extend(bits, dc_cat);
    }
    block_zigzag[0] = *last_dc;

    let mut k = 1usize;
    while k < 64 {
        let sym = match ac_table.decode(br) {
            Some(s) => s,
            None => return false,
        };
        if sym == 0x00 {
            break;
        }
        let run = (sym >> 4) as usize;
        let size = sym & 0x0F;
        if size == 0 && run == 15 {
            k += 16;
            continue;
        }
        if size > 0 {
            k += run;
            if k >= 64 {
                break;
            }
            let bits = match br.read_bits(size as u32) {
                Some(b) => b,
                None => return false,
            };
            block_zigzag[k] = extend(bits, size);
            k += 1;
        }
    }
    true
}

// ── Image Reconstruction (baseline) ───────────────────────────────────────

pub(super) fn reconstruct_image(info: &JpegInfo, data: &[u8]) -> Option<DecodedImage> {
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

    let mut comp_buffers: Vec<Vec<u8>> = info
        .components
        .iter()
        .enumerate()
        .map(|(i, _)| vec![128u8; comp_buf_width[i] * comp_buf_height[i]])
        .collect();

    let mut dc_predictors: Vec<i32> = vec![0; info.num_components as usize];
    let mut block_zigzag = [0i32; 64];
    let mut block_natural = [0i32; 64];
    let mut workspace = [0i32; 64];
    let converter = YccColorConverter::new();

    // Extract entropy segments (between RST markers)
    let entropy_segments = extract_entropy_segments(data, info.entropy_start, info.eoi_pos);
    if entropy_segments.segments.is_empty() {
        return None;
    }

    let total_mcus = (num_mcus_x * num_mcus_y) as usize;
    let mut segment_iter = entropy_segments.segments.iter().peekable();
    let mut seg_idx = 0;
    let mcus_per_seg = if info.restart_interval > 0 {
        info.restart_interval as usize
    } else {
        total_mcus
    };

    while let Some(&(seg_start, seg_end)) = segment_iter.next() {
        let mut br = BitReader::new(data, seg_start, seg_end);
        let mcu_offset = seg_idx * mcus_per_seg;

        for mcu_idx in 0..mcus_per_seg {
            let absolute_mcu = mcu_offset + mcu_idx;
            if absolute_mcu >= total_mcus {
                break;
            }
            let mcu_y = absolute_mcu / num_mcus_x as usize;
            let mcu_x = absolute_mcu % num_mcus_x as usize;

            for scan_comp in &info.scan_components {
                let comp = &info.components[scan_comp.comp_index];
                let dc_table = info.dc_huff_tables[scan_comp.dc_tbl as usize].as_ref()?;
                let ac_table = info.ac_huff_tables[scan_comp.ac_tbl as usize].as_ref()?;
                let quant_table = info.quant_tables[comp.quant_tbl as usize].as_ref()?;

                for by in 0..comp.v_samp as usize {
                    for bx in 0..comp.h_samp as usize {
                        if !decode_block(
                            &mut br,
                            dc_table,
                            ac_table,
                            &mut dc_predictors[scan_comp.comp_index],
                            &mut block_zigzag,
                        ) {
                            return None;
                        }
                        // Dequantize and IDCT
                        for i in 0..64 {
                            block_zigzag[i] *= quant_table[i] as i32;
                        }
                        for i in 0..64 {
                            block_natural[idct::JPEG_NATURAL_ORDER[i]] = block_zigzag[i];
                        }
                        jpeg_idct_islow(&mut block_natural, &mut workspace);

                        let buf_w = comp_buf_width[scan_comp.comp_index];
                        let block_x = (mcu_x * comp.h_samp as usize + bx) * 8;
                        let block_y = (mcu_y * comp.v_samp as usize + by) * 8;
                        for row in 0..8 {
                            for col in 0..8 {
                                let px = block_natural[row * 8 + col].clamp(0, 255) as u8;
                                let bi = (block_y + row) * buf_w + (block_x + col);
                                if bi < comp_buffers[scan_comp.comp_index].len() {
                                    comp_buffers[scan_comp.comp_index][bi] = px;
                                }
                            }
                        }
                    }
                }
            }

            // Handle RST at segment boundaries (except the last segment)
            if mcu_idx + 1 >= mcus_per_seg && segment_iter.peek().is_some() {
                for pred in dc_predictors.iter_mut() {
                    *pred = 0;
                }
                seg_idx += 1;
            }
        }
    }

    // ── Assemble output image ──
    let w = info.width as usize;
    let h = info.height as usize;

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

        // Image-derived chroma dimensions (not MCU-padded)
        let chroma_src_w = (w + h_ratio_us - 1) / h_ratio_us;
        let chroma_src_h = (h + v_ratio_us - 1) / v_ratio_us;

        // Crop then upsample
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
        let cb_upsampled = fancy_upsample(
            &cb_cropped,
            chroma_src_w,
            chroma_src_h,
            h_ratio_us,
            v_ratio_us,
            w,
            h,
        );
        let cr_upsampled = fancy_upsample(
            &cr_cropped,
            chroma_src_w,
            chroma_src_h,
            h_ratio_us,
            v_ratio_us,
            w,
            h,
        );

        let chroma_stride = chroma_src_w * h_ratio_us;
        let mut pixels = Vec::with_capacity(w * h * 3);
        for y in 0..h {
            for x in 0..w {
                let (r, g, b) = converter.ycc_to_rgb(
                    y_buf[y * y_w + x],
                    cb_upsampled[y * chroma_stride + x],
                    cr_upsampled[y * chroma_stride + x],
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

// ── Progressive JPEG Reconstruction ───────────────────────────────────────

pub(super) fn extract_entropy_segments(
    data: &[u8],
    start: usize,
    end_hint: usize,
) -> EntropySegments {
    let mut segments = Vec::new();
    let mut seg_start = start;
    let mut pos = start;
    let mut eoi_pos = 0;

    while pos < end_hint {
        if data[pos] == 0xFF {
            if pos + 1 >= end_hint {
                break;
            }
            match data[pos + 1] {
                0x00 => {
                    pos += 2;
                }
                0xD0..=0xD7 => {
                    segments.push((seg_start, pos));
                    pos += 2;
                    seg_start = pos;
                }
                0xD9 => {
                    segments.push((seg_start, pos));
                    eoi_pos = pos;
                    break;
                }
                _ => {
                    pos += 2;
                    if pos + 1 < end_hint {
                        let len = (data[pos] as u16) << 8 | data[pos + 1] as u16;
                        pos += len as usize;
                    }
                }
            }
        } else {
            pos += 1;
        }
    }

    if seg_start < end_hint && eoi_pos == 0 {
        segments.push((seg_start, end_hint));
    }

    EntropySegments { segments, eoi_pos }
}

/// Entropy segment information (between RST/EOI markers).
pub(super) struct EntropySegments {
    pub(super) segments: Vec<(usize, usize)>,
    #[allow(dead_code)]
    eoi_pos: usize,
}

// ── Public API ────────────────────────────────────────────────────────────

/// Decode JPEG bytes into a DecodedImage (pixel-perfect with libjpeg).
///
/// Supports baseline JPEG (SOF0) and progressive JPEG (SOF2) with:
/// - 8-bit precision
/// - 4:2:0, 4:2:2, 4:4:4 and 4:1:1 chroma subsampling
/// - Grayscale (1 component) and YCbCr (3 components)
/// - Restart markers (DRI)
/// - Progressive: DC first, DC refine, AC first, AC refine scans
pub fn decode(data: &[u8]) -> Option<DecodedImage> {
    let info = parse_jpeg(data)?;

    if info.scan_components.is_empty() {
        return None;
    }

    for comp in &info.components {
        if info.quant_tables.len() <= comp.quant_tbl as usize
            || info.quant_tables[comp.quant_tbl as usize].is_none()
        {
            return None;
        }
    }

    if info.progressive {
        progressive_reconstruct(&info, data)
    } else {
        for scan_comp in &info.scan_components {
            if info.dc_huff_tables.len() <= scan_comp.dc_tbl as usize
                || info.dc_huff_tables[scan_comp.dc_tbl as usize].is_none()
            {
                return None;
            }
            if info.ac_huff_tables.len() <= scan_comp.ac_tbl as usize
                || info.ac_huff_tables[scan_comp.ac_tbl as usize].is_none()
            {
                return None;
            }
        }
        reconstruct_image(&info, data)
    }
}
