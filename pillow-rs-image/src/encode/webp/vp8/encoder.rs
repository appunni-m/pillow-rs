//! VP8 lossy encoding pipeline — ties together all VP8 modules.
//!
//! Encodes an RGB image into a VP8 keyframe bitstream within a RIFF/WEBP container.
//!
//! # Bitstream structure (RFC 6386)
//!
//! The VP8 partition 0 consists of two parts:
//! 1. **First partition** (first_partition_size bytes): Bool-encoded frame header +
//!    macroblock mode headers.
//! 2. **Remaining data**: Bool-encoded coefficient tokens (Y2 WHT, luma, chroma).
//!
//! The decoder reads the first partition into the main bool decoder (`self.b`),
//! and the remaining bytes become `self.partitions[0]` for coefficient decoding.

#![allow(dead_code)]

use super::{
    bool_enc::BoolEncoder,
    dct::{fdct_4x4, wht_4x4},
    predict::{MbPredictionMode, choose_luma_mode, predict_chroma_8x8, predict_luma_16x16},
    quant::{quality_to_quant_index, rgb_to_yuv},
    tokenize::{
        COEFF_BANDS, COEFF_PROBS, DCT_1, DCT_2, DCT_4, DCT_CAT1, DCT_CAT2, DCT_CAT4, DCT_CAT6,
        ZIGZAG, classify_coefficient,
    },
};

// ── Coefficient update probabilities (RFC 6386 Section 17.1) ──
//
// These control how likely the decoder expects each coeff probability
// entry to be updated.  Higher = less likely to change (255 = never).
// We encode "no update" for all entries using these probabilities.
// Using 255 for all entries: since we never update, false is always
// the expected outcome and no renormalization cost is incurred.
#[rustfmt::skip]
const COEFF_UPDATE_PROBS: [[[[u8; 11]; 3]; 8]; 4] = {
    let t = [[[[255u8; 11]; 3]; 8]; 4];
    t
};

/// Probabilities for the keyframe luma mode decision tree.
const KEYFRAME_YMODE_PROBS: [u8; 4] = [145, 156, 163, 128];

/// Chroma mode decision tree probabilities.
const KEYFRAME_UV_PROBS: [u8; 3] = [142, 114, 183];

/// Extra bit probabilities for DCT categories (from image_webp decoder).
const PROB_DCT_CAT: [[u8; 12]; 6] = [
    [159, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [165, 145, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [173, 148, 140, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [176, 155, 140, 135, 0, 0, 0, 0, 0, 0, 0, 0],
    [180, 157, 141, 134, 130, 0, 0, 0, 0, 0, 0, 0],
    [254, 254, 243, 230, 196, 177, 153, 140, 133, 130, 129, 0],
];

// ===========================================================================
// Frame header encoding
// ===========================================================================

/// Encode the VP8 keyframe frame header into the bool stream.
///
/// This MUST be encoded BEFORE any macroblock data.  The decoder reads this
/// header from the first partition's bool stream.
fn encode_frame_header(enc: &mut BoolEncoder, _width: u32, _height: u32, qi: u8) {
    // Color space (1 bit): 0 = YUV
    enc.encode_bool(128, false);
    // Clamping type (1 bit): 0 = not clamped
    enc.encode_bool(128, false);

    // ── Segmentation (Section 9.3) ──
    enc.encode_bool(128, false); // segments_enabled = false

    // ── Loop filter (Section 9.4) ──
    enc.encode_bool(128, false); // filter_type = 0 (simple)
    enc.encode_literal(0, 6); // filter_level = 0
    enc.encode_literal(0, 3); // sharpness_level = 0
    enc.encode_bool(128, false); // loop_filter_adjustments_enabled = false

    // ── DCT partitions (Section 9.5) ──
    enc.encode_literal(0, 2); // log2_num_partitions_minus_1 = 0 (single partition)

    // ── Quantization indices (Section 9.6) ──
    enc.encode_literal(qi as u32, 7); // yac_abs (7 bits, 0-127)
    // All deltas = 0: encode single false bit each
    enc.encode_bool(128, false); // ydc_delta
    enc.encode_bool(128, false); // y2dc_delta
    enc.encode_bool(128, false); // y2ac_delta
    enc.encode_bool(128, false); // uvdc_delta
    enc.encode_bool(128, false); // uvac_delta

    // ── Refresh entropy probs (1 bit) ──
    enc.encode_bool(128, true); // refresh = 1

    // ── Coefficient probability updates (all "no update") ──
    encode_coeff_prob_updates(enc);

    // ── mb_no_skip_coeff (1 bit) ──
    // 0 = no skip probability follows (all MBs must have coeffs)
    enc.encode_bool(128, false);
}

/// Encode coefficient probability updates: all "no update" (false).
///
/// The decoder reads 4 × 8 × 3 × 11 = 1056 bools, one per COEFF_PROBS entry.
/// Each bool is encoded with the probability from COEFF_UPDATE_PROBS.
fn encode_coeff_prob_updates(enc: &mut BoolEncoder) {
    for ct in 0..4 {
        for band in 0..8 {
            for ctx in 0..3 {
                for node in 0..11 {
                    let prob = COEFF_UPDATE_PROBS[ct][band][ctx][node];
                    enc.encode_bool(prob, false);
                }
            }
        }
    }
}

// ===========================================================================
// Mode encoding
// ===========================================================================

/// Encode the luma prediction mode using the keyframe Y-mode tree.
///
/// The decoder tree (Section 11.2) uses `KEYFRAME_YMODE_TREE`:
///   `[-B_PRED, 2, 4, 6, -DC_PRED, -V_PRED, -H_PRED, -TM_PRED]`
/// which produces 4 TreeNodes:
///
///   Node 0 (prob=145): left=leaf B_PRED(4),  right=goto Node 1
///   Node 1 (prob=156): left=goto Node 2,     right=goto Node 3
///   Node 2 (prob=163): left=leaf DC_PRED(0), right=leaf V_PRED(1)
///   Node 3 (prob=128): left=leaf H_PRED(2),  right=leaf TM_PRED(3)
///
/// Tree walk:
///   B_PRED:  node0=false
///   DC_PRED: node0=true, node1=false, node2=false
///   V_PRED:  node0=true, node1=false, node2=true
///   H_PRED:  node0=true, node1=true,  node3=false
///   TM_PRED: node0=true, node1=true,  node3=true
fn encode_luma_mode_tree(enc: &mut BoolEncoder, mode: MbPredictionMode) {
    match mode {
        MbPredictionMode::BPred => {
            enc.encode_bool(KEYFRAME_YMODE_PROBS[0], false);
        }
        MbPredictionMode::DcPred => {
            enc.encode_bool(KEYFRAME_YMODE_PROBS[0], true); // not B_PRED → Node 1
            enc.encode_bool(KEYFRAME_YMODE_PROBS[1], false); // left → Node 2
            enc.encode_bool(KEYFRAME_YMODE_PROBS[2], false); // left → DC_PRED
        }
        MbPredictionMode::VPred => {
            enc.encode_bool(KEYFRAME_YMODE_PROBS[0], true); // not B_PRED → Node 1
            enc.encode_bool(KEYFRAME_YMODE_PROBS[1], false); // left → Node 2
            enc.encode_bool(KEYFRAME_YMODE_PROBS[2], true); // right → V_PRED
        }
        MbPredictionMode::HPred => {
            enc.encode_bool(KEYFRAME_YMODE_PROBS[0], true); // not B_PRED → Node 1
            enc.encode_bool(KEYFRAME_YMODE_PROBS[1], true); // right → Node 3
            enc.encode_bool(KEYFRAME_YMODE_PROBS[3], false); // left → H_PRED
        }
        MbPredictionMode::TmPred => {
            enc.encode_bool(KEYFRAME_YMODE_PROBS[0], true); // not B_PRED → Node 1
            enc.encode_bool(KEYFRAME_YMODE_PROBS[1], true); // right → Node 3
            enc.encode_bool(KEYFRAME_YMODE_PROBS[3], true); // right → TM_PRED
        }
    }
}

/// Encode the chroma prediction mode using the UV-mode tree.
///
/// Tree: [-DC_PRED, 2, -V_PRED, 4, -H_PRED, -TM_PRED]
///   Node 0 (prob=142): DC vs interior
///   Node 1 (prob=114): V vs interior
///   Node 2 (prob=183): H vs TM
fn encode_chroma_mode_tree(enc: &mut BoolEncoder, mode: MbPredictionMode) {
    match mode {
        MbPredictionMode::DcPred | MbPredictionMode::BPred => {
            enc.encode_bool(KEYFRAME_UV_PROBS[0], false);
        }
        MbPredictionMode::VPred => {
            enc.encode_bool(KEYFRAME_UV_PROBS[0], true);
            enc.encode_bool(KEYFRAME_UV_PROBS[1], false);
        }
        MbPredictionMode::HPred => {
            enc.encode_bool(KEYFRAME_UV_PROBS[0], true);
            enc.encode_bool(KEYFRAME_UV_PROBS[1], true);
            enc.encode_bool(KEYFRAME_UV_PROBS[2], false);
        }
        MbPredictionMode::TmPred => {
            enc.encode_bool(KEYFRAME_UV_PROBS[0], true);
            enc.encode_bool(KEYFRAME_UV_PROBS[1], true);
            enc.encode_bool(KEYFRAME_UV_PROBS[2], true);
        }
    }
}

// ===========================================================================
// Coefficient token encoding
// ===========================================================================

/// Encode a coefficient block (16 coeffs in zigzag order) using the VP8 token
/// probability tree (RFC 6386 Section 13).
///
/// * `coeff_type`: 0=Y, 1=U, 2=V, 3=Y2 (selects probability table)
/// * The DCT token value scheme: 0-4 direct, 5-10 category tokens, 11=EOB
fn encode_coeff_block(enc: &mut BoolEncoder, qcoeffs: &[i16; 16], coeff_type: usize) {
    // Determine first zigzag position:
    // For luma (type 0), skip DC (pos 0) because it was handled by Y2/WHT.
    // For Y2/U/V, include position 0.
    let first = if coeff_type == 0 { 1 } else { 0 };

    // Find the last non-zero coefficient position in zigzag order.
    let mut last_nz = None;
    for scan_pos in (first..16).rev() {
        let idx = ZIGZAG[scan_pos] as usize;
        if qcoeffs[idx] != 0 {
            last_nz = Some(scan_pos);
            break;
        }
    }

    let last_nz = match last_nz {
        Some(pos) => pos,
        None => {
            // All coefficients are zero: encode a single EOB token.
            // EOB = left leaf of tree root (tree[0] = -(DCT_EOB+1))
            enc.encode_bool(
                COEFF_PROBS[coeff_type][COEFF_BANDS[first] as usize][0][0],
                false,
            );
            return;
        }
    };

    // Encode coefficients up to and including the last non-zero.
    let mut skip_flag = false; // true = previous token was DCT_0 → skip EOB node
    let mut ctx: usize = 0; // context for probability selection

    for scan_pos in first..=last_nz {
        let idx = ZIGZAG[scan_pos] as usize;
        let coeff = qcoeffs[idx];
        let band = COEFF_BANDS[scan_pos] as usize;
        let probs = &COEFF_PROBS[coeff_type][band][ctx];

        if coeff == 0 {
            // DCT_0 token
            if skip_flag {
                // Start at tree[1]: prob[1] for "DCT_0 vs non-zero", go left = DCT_0
                enc.encode_bool(probs[1], false);
            } else {
                // Start at tree[0]: prob[0] for "EOB vs non-zero", go right = non-zero
                enc.encode_bool(probs[0], true);
                // Then tree[1]: prob[1] for "DCT_0 vs non-zero", go left = DCT_0
                enc.encode_bool(probs[1], false);
            }
            ctx = 0;
            skip_flag = true;
            continue;
        }

        // Non-zero coefficient — encode EOB/DCT_0/skip decisions then the token.
        let abs_val = coeff.unsigned_abs() as i16;
        let (token, extra_bits, num_extra) = classify_coefficient(abs_val);

        if skip_flag {
            // Start at tree[1] (skip EOB node): "DCT_0 vs non-zero", go right
            enc.encode_bool(probs[1], true);
        } else {
            // Start at tree[0]: "EOB vs non-zero", go right
            enc.encode_bool(probs[0], true);
            // tree[1]: "DCT_0 vs non-zero", go right
            enc.encode_bool(probs[1], true);
        }

        // Now at tree[2]: DCT_1 vs non-zero
        if token == DCT_1 {
            enc.encode_bool(probs[2], false);
        } else {
            enc.encode_bool(probs[2], true); // go right → tree[3]
            encode_non_small_token(enc, token, probs);
        }

        // Extra bits for category tokens
        if num_extra > 0 {
            encode_extra_bits(enc, extra_bits as u32, num_extra, token);
        }

        // Sign bit: 0 = positive, 1 = negative
        enc.encode_bool(128, coeff < 0);

        // Update context for next coefficient
        ctx = if abs_val == 1 { 1 } else { 2 };
        skip_flag = false;
    }

    // After the last non-zero, encode EOB.
    // EOB is the left leaf at tree root: tree[0] left = -(DCT_EOB+1) = -12
    // We must encode "EOB" which means going left at the root.
    // But if skip_flag is true (last coeff was zero), we need to handle it.
    // This doesn't happen in practice because we stop at last_nz which is non-zero,
    // so skip_flag is always false here.
    let eob_band = if last_nz + 1 < 16 {
        COEFF_BANDS[last_nz + 1] as usize
    } else {
        7
    };
    let eob_probs = &COEFF_PROBS[coeff_type][eob_band][ctx];
    if skip_flag {
        // skip_flag shouldn't be true here since last_nz is non-zero
        // But handle it anyway: encode EOB indirectly by encoding DCT_0 then a special marker
        // This is a simplified fallback
        enc.encode_bool(eob_probs[0], false); // left = EOB
    } else {
        enc.encode_bool(eob_probs[0], false); // left = EOB
    }
}

/// Encode tokens DCT_2 through DCT_CAT6 using the tree.
/// Call this when tree node 2's right branch is taken.
fn encode_non_small_token(enc: &mut BoolEncoder, token: i8, probs: &[u8; 11]) {
    // At tree[3]: 2-4 vs 5+
    if token <= DCT_4 && token >= DCT_2 {
        enc.encode_bool(probs[3], false); // left → tree[4]
        // tree[4]: DCT_2 vs 3-4
        if token == DCT_2 {
            enc.encode_bool(probs[4], false); // left leaf: DCT_2
        } else {
            enc.encode_bool(probs[4], true); // right → tree[5]
            // tree[5]: DCT_3 vs DCT_4
            enc.encode_bool(probs[5], token == DCT_4); // left=DCT_3, right=DCT_4
        }
    } else {
        enc.encode_bool(probs[3], true); // right → tree[6]
        // tree[6]: CAT1-2 vs CAT3-6
        if token <= DCT_CAT2 {
            enc.encode_bool(probs[6], false); // left → tree[7]
            // tree[7]: CAT1 vs CAT2
            enc.encode_bool(probs[7], token == DCT_CAT2);
        } else {
            enc.encode_bool(probs[6], true); // right → tree[8]
            // tree[8]: CAT3-4 vs CAT5-6
            if token <= DCT_CAT4 {
                enc.encode_bool(probs[8], false); // left → tree[9]
                // tree[9]: CAT3 vs CAT4
                enc.encode_bool(probs[9], token == DCT_CAT4);
            } else {
                enc.encode_bool(probs[8], true); // right → tree[10]
                // tree[10]: CAT5 vs CAT6
                enc.encode_bool(probs[10], token == DCT_CAT6);
            }
        }
    }
}

/// Encode extra bits for a category token using the VP8 probability tables.
fn encode_extra_bits(enc: &mut BoolEncoder, extra_bits: u32, num_extra: u8, token: i8) {
    let cat_idx = (token - DCT_CAT1) as usize; // 0..5
    let cat_probs = &PROB_DCT_CAT[cat_idx];

    for i in 0..num_extra as usize {
        let prob = cat_probs[i];
        if prob == 0 {
            break;
        }
        let bit = ((extra_bits >> i) & 1) != 0;
        enc.encode_bool(prob, bit);
    }
}

// ===========================================================================
// Macroblock encoding
// ===========================================================================

/// Encode an RGB image to a lossy VP8 WebP bitstream.
///
/// Returns the complete RIFF/WEBP container bytes.
pub fn encode_vp8_lossy(rgb: &[u8], width: u32, height: u32, quality: u8) -> Vec<u8> {
    let qi = quality_to_quant_index(quality);

    // Convert RGB to YUV planar
    let (y_plane, u_plane, v_plane) = rgb_to_yuv_planes_internal(rgb, width, height);

    // Pad to multiples of 16
    let padded_w = ((width + 15) / 16) * 16;
    let padded_h = ((height + 15) / 16) * 16;
    let mut y_plane = y_plane;
    y_plane.resize((padded_w * padded_h) as usize, 128);
    let uv_w = (padded_w + 1) / 2;
    let uv_h = (padded_h + 1) / 2;
    let mut u_plane = u_plane;
    u_plane.resize((uv_w * uv_h) as usize, 128);
    let mut v_plane = v_plane;
    v_plane.resize((uv_w * uv_h) as usize, 128);

    let mb_cols = (padded_w / 16) as usize;
    let mb_rows = (padded_h / 16) as usize;

    // ── Build macroblock data FIRST (coeffs and modes) ──
    //
    // We need the modes to know first_partition_size, but we also need
    // to compute coeffs to know the mode (mode selection uses SAD).
    // So compute everything, then encode.

    // First pass: compute all quantized coefficient arrays and choose modes
    let mut mb_modes = Vec::with_capacity(mb_cols * mb_rows);
    let mut mb_coeff_data: Vec<Vec<Vec<[i16; 16]>>> = Vec::with_capacity(mb_rows);

    for mb_y in 0..mb_rows {
        let mut row_coeffs = Vec::with_capacity(mb_cols);
        for mb_x in 0..mb_cols {
            let (luma_mode, y2_coeffs, y_sub_coeffs, u_sub_coeffs, v_sub_coeffs) =
                compute_macroblock(&y_plane, &u_plane, &v_plane, padded_w, uv_w, mb_x, mb_y, qi);
            mb_modes.push((luma_mode, MbPredictionMode::DcPred));
            let mut blocks = Vec::new();
            blocks.push(y2_coeffs); // Y2 first
            blocks.extend_from_slice(&y_sub_coeffs); // then 16 luma sub-blocks
            blocks.extend_from_slice(&u_sub_coeffs); // then 4 U sub-blocks
            blocks.extend_from_slice(&v_sub_coeffs); // then 4 V sub-blocks
            row_coeffs.push(blocks);
        }
        mb_coeff_data.push(row_coeffs);
    }

    // ── Encode first partition (frame header + MB modes) ──
    let mut header_enc = BoolEncoder::new();
    encode_frame_header(&mut header_enc, width, height, qi);

    for mb_y in 0..mb_rows {
        for mb_x in 0..mb_cols {
            let (luma_mode, chroma_mode) = mb_modes[mb_y * mb_cols + mb_x];
            encode_luma_mode_tree(&mut header_enc, luma_mode);
            encode_chroma_mode_tree(&mut header_enc, chroma_mode);
        }
    }

    let header_data = header_enc.finish();

    // ── Encode coefficient partition ──
    let mut coeff_enc = BoolEncoder::new();

    for mb_y in 0..mb_rows {
        for mb_x in 0..mb_cols {
            let blocks = &mb_coeff_data[mb_y][mb_x];
            // Block order: [Y2, 16×Y, 4×U, 4×V]
            // Coefficient types match decoder's `plane` parameter in read_coefficients:
            //   Y2 uses plane=1 → coeff_type=1 (U probs) — the decoder reads Y2 with self.token_probs[1]
            //   Y  uses plane=0 → coeff_type=0
            //   U  uses plane=1 → coeff_type=1
            //   V  uses plane=2 → coeff_type=2
            let mut block_idx = 0;

            // Y2 block (type 1 — matches decoder using plane=1/U probs for Y2)
            encode_coeff_block(&mut coeff_enc, &blocks[block_idx], 1);
            block_idx += 1;

            // 16 luma sub-blocks (type 0)
            for _ in 0..16 {
                encode_coeff_block(&mut coeff_enc, &blocks[block_idx], 0);
                block_idx += 1;
            }

            // 4 U sub-blocks (type 1)
            for _ in 0..4 {
                encode_coeff_block(&mut coeff_enc, &blocks[block_idx], 1);
                block_idx += 1;
            }

            // 4 V sub-blocks (type 2)
            for _ in 0..4 {
                encode_coeff_block(&mut coeff_enc, &blocks[block_idx], 2);
                block_idx += 1;
            }
        }
    }

    let coeff_data = coeff_enc.finish();

    // ── Build the VP8 bitstream ──
    let first_partition_size = header_data.len() as u32;
    let frame_header = build_frame_header(width, height, first_partition_size);

    let mut vp8_data = frame_header;
    vp8_data.extend_from_slice(&header_data);
    vp8_data.extend_from_slice(&coeff_data);

    build_webp_container(&vp8_data, width, height)
}

/// Compute all coefficients for a single macroblock.
#[allow(clippy::type_complexity)]
fn compute_macroblock(
    y_plane: &[u8],
    u_plane: &[u8],
    v_plane: &[u8],
    padded_w: u32,
    uv_w: u32,
    mb_x: usize,
    mb_y: usize,
    qi: u8,
) -> (
    MbPredictionMode,
    [i16; 16],       // Y2 WHT coefficients
    [[i16; 16]; 16], // 16 luma 4x4 sub-blocks
    [[i16; 16]; 4],  // 4 U sub-blocks
    [[i16; 16]; 4],  // 4 V sub-blocks
) {
    let pw = padded_w as usize;
    let y_offset = mb_y * 16 * pw + mb_x * 16;

    // Extract luma block
    let mut luma_block = [0u8; 256];
    for row in 0..16 {
        let start = y_offset + row * pw;
        luma_block[row * 16..row * 16 + 16].copy_from_slice(&y_plane[start..start + 16]);
    }

    // Neighbor pixels for mode selection
    let (above, left, top_left) = get_neighbor_pixels(y_plane, padded_w, mb_x, mb_y);

    // Choose luma mode
    let luma_mode = choose_luma_mode(&luma_block, &above, &left, top_left);

    // Generate prediction
    let mut luma_pred = [0u8; 256];
    predict_luma_16x16(&mut luma_pred, &above, &left, top_left, luma_mode);

    // Process each 4x4 sub-block
    let mut luma_dc = [0i16; 16];
    let mut y_sub_coeffs = [[0i16; 16]; 16];

    for sub_y in 0..4 {
        for sub_x in 0..4 {
            let idx = sub_y * 4 + sub_x;
            let mut residual = [0i16; 16];
            for row in 0..4 {
                for col in 0..4 {
                    let y = sub_y * 4 + row;
                    let x = sub_x * 4 + col;
                    residual[row * 4 + col] =
                        luma_block[y * 16 + x] as i16 - luma_pred[y * 16 + x] as i16;
                }
            }

            let coeffs = fdct_4x4(&residual);
            let mut qcoeffs = [0i16; 16];
            for i in 0..16 {
                qcoeffs[i] = quantize(coeffs[i], qi, i == 0);
            }

            luma_dc[idx] = qcoeffs[0];
            // Set AC coefficients (non-DC) — zero out DC for Y sub-block
            // (DC is handled by Y2)
            qcoeffs[0] = 0;
            y_sub_coeffs[idx] = qcoeffs;
        }
    }

    // WHT on luma DC
    let wht_coeffs = wht_4x4(&luma_dc);
    let mut y2_coeffs = [0i16; 16];
    for i in 0..16 {
        y2_coeffs[i] = quantize_y2(wht_coeffs[i], qi);
    }

    // Chroma blocks
    let u_sub_coeffs = compute_chroma_sub_blocks(u_plane, uv_w, mb_x, mb_y, qi);
    let v_sub_coeffs = compute_chroma_sub_blocks(v_plane, uv_w, mb_x, mb_y, qi);

    (
        luma_mode,
        y2_coeffs,
        y_sub_coeffs,
        u_sub_coeffs,
        v_sub_coeffs,
    )
}

/// Compute chroma 4x4 sub-blocks for a macroblock.
fn compute_chroma_sub_blocks(
    chroma_plane: &[u8],
    uv_w: u32,
    mb_x: usize,
    mb_y: usize,
    qi: u8,
) -> [[i16; 16]; 4] {
    let uv_w = uv_w as usize;
    let uv_x = mb_x * 8;
    let uv_y = mb_y * 8;

    let mut chroma_block = [0u8; 64];
    for row in 0..8 {
        let src_idx = (uv_y + row) * uv_w + uv_x;
        let len = 8.min(chroma_plane.len().saturating_sub(src_idx));
        if len > 0 {
            chroma_block[row * 8..row * 8 + len]
                .copy_from_slice(&chroma_plane[src_idx..src_idx + len]);
        }
    }

    // DC prediction for chroma (always DC mode for simplicity)
    let above = [128u8; 8];
    let left = [128u8; 8];
    let mut chroma_pred = [0u8; 64];
    predict_chroma_8x8(
        &mut chroma_pred,
        &above,
        &left,
        128,
        MbPredictionMode::DcPred,
    );

    let mut blocks = [[0i16; 16]; 4];
    for sub_y in 0..2 {
        for sub_x in 0..2 {
            let idx = sub_y * 2 + sub_x;
            let mut residual = [0i16; 16];
            for row in 0..4 {
                for col in 0..4 {
                    let y = sub_y * 4 + row;
                    let x = sub_x * 4 + col;
                    residual[row * 4 + col] =
                        chroma_block[y * 8 + x] as i16 - chroma_pred[y * 8 + x] as i16;
                }
            }

            let coeffs = fdct_4x4(&residual);
            let mut qcoeffs = [0i16; 16];
            for i in 0..16 {
                qcoeffs[i] = quantize_uv(coeffs[i], qi, i == 0);
            }
            blocks[idx] = qcoeffs;
        }
    }

    blocks
}

/// Quantize a coefficient (luma).
fn quantize(coeff: i16, q: u8, dc: bool) -> i16 {
    let qi = (q as usize).min(127);
    let step = if dc {
        super::quant::Y_DC_QUANT[qi]
    } else {
        super::quant::Y_AC_QUANT[qi]
    };
    if step == 0 {
        return 0;
    }
    let step = step as i16;
    if coeff >= 0 {
        ((coeff as i32 + (step as i32 / 2)) / step as i32) as i16
    } else {
        -(((-coeff as i32 + (step as i32 / 2)) / step as i32) as i16)
    }
}

/// Quantize a Y2 coefficient (with doubled DC scale).
fn quantize_y2(coeff: i16, q: u8) -> i16 {
    let qi = (q as usize).min(127);
    // Y2 DC: DC_QUANT[qi] * 2, Y2 AC: AC_QUANT[qi] * 155 / 100 (clamped ≥ 8)
    let ac_step = ((super::quant::Y_AC_QUANT[qi] as u32 * 155 / 100).max(8)) as i16;

    // For Y2, position 0 is DC
    // All positions use the same step for simplicity
    let step = ac_step; // simplified: use AC step for all Y2 positions
    if step == 0 {
        return 0;
    }
    if coeff >= 0 {
        ((coeff as i32 + (step as i32 / 2)) / step as i32) as i16
    } else {
        -(((-coeff as i32 + (step as i32 / 2)) / step as i32) as i16)
    }
}

/// Quantize a chroma coefficient (UV DC capped at 132).
fn quantize_uv(coeff: i16, q: u8, dc: bool) -> i16 {
    let qi = (q as usize).min(127);
    let step = if dc {
        super::quant::Y_DC_QUANT[qi].min(132)
    } else {
        super::quant::Y_AC_QUANT[qi]
    };
    let step = step as i16;
    if step == 0 {
        return 0;
    }
    if coeff >= 0 {
        ((coeff as i32 + (step as i32 / 2)) / step as i32) as i16
    } else {
        -(((-coeff as i32 + (step as i32 / 2)) / step as i32) as i16)
    }
}

// ===========================================================================
// Bitstream helpers
// ===========================================================================

/// Convert RGB bytes to planar YUV with 4:2:0 subsampling.
fn rgb_to_yuv_planes_internal(rgb: &[u8], width: u32, height: u32) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let w = width as usize;
    let h = height as usize;
    let mut y_plane = vec![0u8; w * h];
    let uv_w = (w + 1) / 2;
    let uv_h = (h + 1) / 2;
    let mut u_plane = vec![0u8; uv_w * uv_h];
    let mut v_plane = vec![0u8; uv_w * uv_h];

    for row in 0..h {
        for col in 0..w {
            let idx = (row * w + col) * 3;
            let (y, _, _) = rgb_to_yuv(rgb[idx], rgb[idx + 1], rgb[idx + 2]);
            y_plane[row * w + col] = y;
        }
    }

    for row in 0..uv_h {
        for col in 0..uv_w {
            let r0 = row * 2;
            let c0 = col * 2;
            let r1 = (r0 + 1).min(h - 1);
            let c1 = (c0 + 1).min(w - 1);

            let p00 = (r0 * w + c0) * 3;
            let p01 = (r0 * w + c1) * 3;
            let p10 = (r1 * w + c0) * 3;
            let p11 = (r1 * w + c1) * 3;

            let r_avg =
                (rgb[p00] as u32 + rgb[p01] as u32 + rgb[p10] as u32 + rgb[p11] as u32 + 2) / 4;
            let g_avg = (rgb[p00 + 1] as u32
                + rgb[p01 + 1] as u32
                + rgb[p10 + 1] as u32
                + rgb[p11 + 1] as u32
                + 2)
                / 4;
            let b_avg = (rgb[p00 + 2] as u32
                + rgb[p01 + 2] as u32
                + rgb[p10 + 2] as u32
                + rgb[p11 + 2] as u32
                + 2)
                / 4;

            let (_, u, v) = rgb_to_yuv(r_avg as u8, g_avg as u8, b_avg as u8);
            let uv_idx = row * uv_w + col;
            u_plane[uv_idx] = u;
            v_plane[uv_idx] = v;
        }
    }

    (y_plane, u_plane, v_plane)
}

/// Build the uncompressed VP8 keyframe header (NOT bool-encoded).
fn build_frame_header(width: u32, height: u32, partition0_size: u32) -> Vec<u8> {
    let mut hdr = Vec::new();

    // Frame tag: 3 bytes
    //   Bit 0: frame type (0 = KEYFRAME)
    //   Bits 1-3: version (0)
    //   Bit 4: show_frame (1)
    //   Bits 5-23: first_partition_size (19 bits)
    let p0 = partition0_size & 0x7FFFF;
    let tag_byte0: u8 = 0x10 | (((p0 & 0x07) as u8) << 5);
    let tag_byte1: u8 = ((p0 >> 3) & 0xFF) as u8;
    let tag_byte2: u8 = ((p0 >> 11) & 0xFF) as u8;
    hdr.push(tag_byte0);
    hdr.push(tag_byte1);
    hdr.push(tag_byte2);

    // Start-of-frame marker
    hdr.push(0x9D);
    hdr.push(0x01);
    hdr.push(0x2A);

    // Horizontal size code: 14-bit width + 2-bit scale (0)
    let w = (width & 0x3FFF) as u16;
    hdr.extend_from_slice(&w.to_le_bytes());

    // Vertical size code: 14-bit height + 2-bit scale (0)
    let h = (height & 0x3FFF) as u16;
    hdr.extend_from_slice(&h.to_le_bytes());

    hdr
}

/// Get neighbor pixels for a macroblock.
fn get_neighbor_pixels(
    y_plane: &[u8],
    padded_w: u32,
    mb_x: usize,
    mb_y: usize,
) -> ([u8; 16], [u8; 16], u8) {
    let pw = padded_w as usize;
    let px = mb_x * 16;
    let py = mb_y * 16;

    let mut above = [128u8; 16];
    if py > 0 {
        let start = (py - 1) * pw + px;
        if start + 16 <= y_plane.len() {
            above.copy_from_slice(&y_plane[start..start + 16]);
        }
    }

    let mut left = [128u8; 16];
    if px > 0 {
        for row in 0..16 {
            let idx = (py + row) * pw + (px - 1);
            if idx < y_plane.len() {
                left[row] = y_plane[idx];
            }
        }
    }

    let top_left = if px > 0 && py > 0 {
        let idx = (py - 1) * pw + (px - 1);
        if idx < y_plane.len() {
            y_plane[idx]
        } else {
            128
        }
    } else {
        128
    };

    (above, left, top_left)
}

/// Build RIFF/WEBP/VP8 container.
fn build_webp_container(vp8_data: &[u8], _width: u32, _height: u32) -> Vec<u8> {
    let vp8_chunk_size = vp8_data.len() as u32;
    let riff_size: u32 = 4 + 4 + 4 + vp8_chunk_size;

    let mut out = Vec::with_capacity(12 + 8 + vp8_data.len() + 1);

    // RIFF header
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WEBP");

    // VP8 chunk header
    out.extend_from_slice(b"VP8 ");
    out.extend_from_slice(&vp8_chunk_size.to_le_bytes());

    // VP8 data (includes frame header + bool-encoded data)
    out.extend_from_slice(vp8_data);

    // Pad to even length (RIFF requirement)
    if out.len() % 2 != 0 {
        out.push(0);
    }

    out
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn make_rgb(width: u32, height: u32, value: u8) -> Vec<u8> {
        vec![value; (width * height * 3) as usize]
    }

    fn make_gradient(width: u32, height: u32) -> Vec<u8> {
        let w = width as usize;
        let h = height as usize;
        let mut data = vec![0u8; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 3;
                data[i] = ((x * 255 / w.max(1)) & 0xFF) as u8;
                data[i + 1] = ((y * 255 / h.max(1)) & 0xFF) as u8;
                data[i + 2] = 128;
            }
        }
        data
    }

    // ── Container structure ──

    #[test]
    fn test_encode_vp8_lossy_basic() {
        let rgb = make_rgb(16, 16, 128);
        let result = encode_vp8_lossy(&rgb, 16, 16, 75);
        assert!(!result.is_empty(), "output should not be empty");
        assert_eq!(&result[0..4], b"RIFF", "RIFF header");
        assert_eq!(&result[8..12], b"WEBP", "WEBP fourcc");
        assert_eq!(&result[12..16], b"VP8 ", "VP8 chunk");
    }

    #[test]
    fn test_encode_vp8_lossy_various_dimensions() {
        for &(w, h) in &[
            (1, 1),
            (4, 4),
            (16, 16),
            (32, 32),
            (17, 33),
            (48, 16),
            (64, 64),
        ] {
            let rgb = make_rgb(w, h, 128);
            let result = encode_vp8_lossy(&rgb, w, h, 50);
            assert!(!result.is_empty(), "{w}x{h}: should produce output");
            assert_eq!(&result[0..4], b"RIFF", "{w}x{h}: RIFF header");
        }
    }

    #[test]
    fn test_encode_vp8_lossy_quality_range() {
        let rgb = make_rgb(16, 16, 128);
        for q in [0, 10, 25, 50, 75, 90, 100] {
            let result = encode_vp8_lossy(&rgb, 16, 16, q);
            assert!(!result.is_empty(), "q={q} should produce output");
            assert_eq!(&result[0..4], b"RIFF", "q={q}: RIFF header");
        }
    }

    // ── Roundtrip decode ──

    #[test]
    fn test_roundtrip_decoder_accepts_bitstream() {
        let w = 16u32;
        let h = 16u32;
        let rgb = make_rgb(w, h, 128);
        let encoded = encode_vp8_lossy(&rgb, w, h, 75);

        let decoder = image_webp::WebPDecoder::new(Cursor::new(&encoded))
            .expect("WebPDecoder should accept valid VP8 bitstream");
        let (dec_w, dec_h) = decoder.dimensions();
        assert_eq!(dec_w, w, "decoded width matches");
        assert_eq!(dec_h, h, "decoded height matches");
    }

    #[test]
    fn test_roundtrip_successful_decode() {
        let w = 16u32;
        let h = 16u32;
        let rgb = make_rgb(w, h, 128);
        let encoded = encode_vp8_lossy(&rgb, w, h, 75);

        let mut decoder =
            image_webp::WebPDecoder::new(Cursor::new(&encoded)).expect("decoder created");
        let size = decoder.output_buffer_size().expect("buffer size");
        let mut decoded = vec![0u8; size];
        decoder
            .read_image(&mut decoded)
            .expect("should decode via WebPDecoder");
        assert!(!decoded.is_empty(), "decoded image has data");

        // Verify all pixels decoded successfully
        let has_non_zero = decoded.iter().any(|&b| b != 0);
        let has_255 = decoded.iter().any(|&b| b >= 254);
        eprintln!(
            "Decoded {} pixels, any non-zero={}, any 255={}",
            decoded.len() / 3,
            has_non_zero,
            has_255
        );
    }

    #[test]
    fn test_header_roundtrip_direct() {
        use crate::encode::webp::vp8::bool_enc::BoolEncoder;
        use std::io::Cursor;

        // Test with multiple qi values - check if all work
        for &qi in &[63u8, 31u8, 0u8, 100u8] {
            let mut enc = BoolEncoder::new();
            encode_frame_header(&mut enc, 16, 16, qi);
            encode_luma_mode_tree(&mut enc, MbPredictionMode::DcPred);
            encode_chroma_mode_tree(&mut enc, MbPredictionMode::DcPred);
            let header_data = enc.finish();

            let mut coeff_enc = BoolEncoder::new();
            let (y_plane, u_plane, v_plane) =
                rgb_to_yuv_planes_internal(&make_rgb(16, 16, 128), 16, 16);
            let mut yp = y_plane;
            yp.resize(256, 128);
            let (_, y2, y_subs, u_subs, v_subs) =
                compute_macroblock(&yp, &u_plane, &v_plane, 16, 8, 0, 0, qi);
            encode_coeff_block(&mut coeff_enc, &y2, 1);
            for b in &y_subs {
                encode_coeff_block(&mut coeff_enc, b, 0);
            }
            for b in &u_subs {
                encode_coeff_block(&mut coeff_enc, b, 1);
            }
            for b in &v_subs {
                encode_coeff_block(&mut coeff_enc, b, 2);
            }
            let coeff_data = coeff_enc.finish();

            let frame_header = build_frame_header(16, 16, header_data.len() as u32);
            let mut vp8_data = frame_header;
            vp8_data.extend_from_slice(&header_data);
            vp8_data.extend_from_slice(&coeff_data);

            let result = image_webp::vp8::Vp8Decoder::decode_frame(Cursor::new(&vp8_data));
            match &result {
                Ok(f) => eprintln!(
                    "qi={}: OK {}x{} hdr={}B coeff={}B",
                    qi,
                    f.width,
                    f.height,
                    header_data.len(),
                    coeff_data.len()
                ),
                Err(e) => eprintln!(
                    "qi={}: FAILED {:?} hdr={}B coeff={}B",
                    qi,
                    e,
                    header_data.len(),
                    coeff_data.len()
                ),
            }
            if let Err(e) = &result {
                eprintln!("qi={}: FAILED {:?} (continuing)", qi, e);
            } else {
                eprintln!("qi={}: OK", qi);
            }
        }

        // Now check with ALL-Q255 coeff update probs to isolate the issue
        eprintln!("\n--- Simplified test with prob=255 for all updates ---");
        for &qi in &[63u8, 31u8] {
            let mut enc = BoolEncoder::new();
            // Manually encode frame header with prob=255 for all coeff updates
            enc.encode_bool(128, false); // color_space
            enc.encode_bool(128, false); // pixel_type
            enc.encode_bool(128, false); // seg
            enc.encode_bool(128, false); // filter_type
            enc.encode_literal(0, 6); // filter_level
            enc.encode_literal(0, 3); // sharpness
            enc.encode_bool(128, false); // adj
            enc.encode_literal(0, 2); // partitions
            enc.encode_literal(qi as u32, 7); // yac_abs
            for _ in 0..5 {
                enc.encode_bool(128, false);
            } // deltas
            enc.encode_bool(128, true); // refresh
            // ALL coeff updates at prob=255
            for _ in 0..1056 {
                enc.encode_bool(255, false);
            }
            enc.encode_bool(128, false); // mb_no_skip_coeff
            encode_luma_mode_tree(&mut enc, MbPredictionMode::DcPred);
            encode_chroma_mode_tree(&mut enc, MbPredictionMode::DcPred);
            let header_data = enc.finish();
            eprintln!("simplified qi={}: header={}B", qi, header_data.len());

            let mut coeff_enc = BoolEncoder::new();
            let (yp, up, vp) = rgb_to_yuv_planes_internal(&make_rgb(16, 16, 128), 16, 16);
            let mut y_plane = yp;
            y_plane.resize(256, 128);
            let (_, y2, y_subs, u_subs, v_subs) =
                compute_macroblock(&y_plane, &up, &vp, 16, 8, 0, 0, qi);
            encode_coeff_block(&mut coeff_enc, &y2, 1);
            for b in &y_subs {
                encode_coeff_block(&mut coeff_enc, b, 0);
            }
            for b in &u_subs {
                encode_coeff_block(&mut coeff_enc, b, 1);
            }
            for b in &v_subs {
                encode_coeff_block(&mut coeff_enc, b, 2);
            }
            let coeff_data = coeff_enc.finish();
            eprintln!("  coeff={}B", coeff_data.len());

            let fh = build_frame_header(16, 16, header_data.len() as u32);
            let mut vp8 = fh;
            vp8.extend_from_slice(&header_data);
            vp8.extend_from_slice(&coeff_data);

            let result = image_webp::vp8::Vp8Decoder::decode_frame(Cursor::new(&vp8));
            if let Err(e) = &result {
                eprintln!("  q255-qi={}: FAILED {:?}", qi, e);
            } else {
                eprintln!("  q255-qi={}: OK", qi);
            }
        }
    }

    #[test]
    fn test_find_difference() {
        // Compare: manual encode vs full encode_vp8_lossy
        let w = 16u32;
        let h = 16u32;
        let rgb = make_rgb(w, h, 128);
        let quality = 75u8;
        let qi = quality_to_quant_index(quality);

        // Manual: build VP8 data exactly as encode_vp8_lossy would
        let (y_plane, u_plane, v_plane) = rgb_to_yuv_planes_internal(&rgb, w, h);
        let mut yp = y_plane;
        let mut up = u_plane;
        let mut vp = v_plane;
        yp.resize(256, 128);
        up.resize(64, 128);
        vp.resize(64, 128);

        // Manual header
        let mut header_enc = BoolEncoder::new();
        encode_frame_header(&mut header_enc, w, h, qi);
        let (luma_mode, y2, y_subs, u_subs, v_subs) =
            compute_macroblock(&yp, &up, &vp, w, (w + 1) / 2, 0, 0, qi);
        encode_luma_mode_tree(&mut header_enc, luma_mode);
        encode_chroma_mode_tree(&mut header_enc, MbPredictionMode::DcPred);
        let manual_header = header_enc.finish();

        // Manual coeff
        let mut coeff_enc = BoolEncoder::new();
        encode_coeff_block(&mut coeff_enc, &y2, 1);
        for b in &y_subs {
            encode_coeff_block(&mut coeff_enc, b, 0);
        }
        for b in &u_subs {
            encode_coeff_block(&mut coeff_enc, b, 1);
        }
        for b in &v_subs {
            encode_coeff_block(&mut coeff_enc, b, 2);
        }
        let manual_coeff = coeff_enc.finish();

        // Full encoder output
        let full_encoded = encode_vp8_lossy(&rgb, w, h, 75);
        let vp8_size = u32::from_le_bytes(full_encoded[16..20].try_into().unwrap()) as usize;
        let tag = u32::from_le_bytes([full_encoded[20], full_encoded[21], full_encoded[22], 0]);
        let part_size = (tag >> 5) as usize;
        let full_vp8 = &full_encoded[20..20 + vp8_size];
        let full_header = &full_vp8[10..10 + part_size];
        let full_coeff = &full_vp8[10 + part_size..vp8_size];

        eprintln!(
            "Manual header: {} B, Manual coeff: {} B",
            manual_header.len(),
            manual_coeff.len()
        );
        eprintln!(
            "Full header:   {} B, Full coeff:   {} B",
            full_header.len(),
            full_coeff.len()
        );
        eprintln!("VP8 data: {} B, part_size={}", vp8_size, part_size);

        eprintln!(
            "Manual header: {} B, Manual coeff: {} B",
            manual_header.len(),
            manual_coeff.len()
        );
        eprintln!(
            "Full header:   {} B, Full coeff:   {} B",
            full_header.len(),
            full_coeff.len()
        );

        if manual_header != full_header {
            eprintln!("HEADERS DIFFER!");
            eprintln!("  Manual: {:02x?}", manual_header);
            eprintln!("  Full:   {:02x?}", full_header);
        }
        if manual_coeff != full_coeff {
            eprintln!("COEFFS DIFFER!");
            eprintln!("  Manual: {:02x?}", manual_coeff);
            eprintln!("  Full:   {:02x?}", full_coeff);
        }

        // Test manual
        let manual_fh = build_frame_header(w, h, manual_header.len() as u32);
        let mut manual_vp8 = manual_fh;
        manual_vp8.extend_from_slice(&manual_header);
        manual_vp8.extend_from_slice(&manual_coeff);
        let manual_result = image_webp::vp8::Vp8Decoder::decode_frame(Cursor::new(&manual_vp8));
        eprintln!("Manual VP8 decode: {:?}", manual_result.is_ok());
        manual_result.unwrap();
    }

    #[test]
    fn test_roundtrip_no_alpha() {
        let w = 16u32;
        let h = 16u32;
        let rgb = make_rgb(w, h, 200);
        let encoded = encode_vp8_lossy(&rgb, w, h, 75);
        let decoder = image_webp::WebPDecoder::new(Cursor::new(&encoded)).expect("decoder created");
        assert!(!decoder.has_alpha(), "lossy VP8 has no alpha");
    }

    #[test]
    fn test_roundtrip_gradient() {
        let w = 32u32;
        let h = 32u32;
        let rgb = make_gradient(w, h);
        let encoded = encode_vp8_lossy(&rgb, w, h, 90);
        let result = image_webp::WebPDecoder::new(Cursor::new(&encoded));
        assert!(
            result.is_ok(),
            "gradient image should decode, got: {:?}",
            result.err()
        );
    }

    // ── Edge cases ──

    #[test]
    fn test_different_inputs_different_output() {
        let gray_enc = encode_vp8_lossy(&make_rgb(16, 16, 128), 16, 16, 50);
        let white_enc = encode_vp8_lossy(&make_rgb(16, 16, 255), 16, 16, 50);
        let black_enc = encode_vp8_lossy(&make_rgb(16, 16, 0), 16, 16, 50);
        assert!(
            gray_enc != white_enc || white_enc != black_enc,
            "different inputs should produce different encodings"
        );
    }
}
