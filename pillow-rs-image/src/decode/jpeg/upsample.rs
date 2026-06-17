// ── Chroma Upsampling (libjpeg-exact triangle filter) ─────────────────────

/// 2x1 fancy upsampling — exact match of IJG libjpeg h2v1_fancy_upsample.
pub(super) fn h2v1_fancy_upsample(src: &[u8], src_w: usize, src_h: usize) -> Vec<u8> {
    let dst_w = src_w * 2;
    let mut out = vec![0u8; dst_w * src_h];
    for y in 0..src_h {
        let in_row = y * src_w;
        let out_row = y * dst_w;

        let mut invalue = src[in_row] as i32;
        out[out_row] = invalue as u8;
        if src_w > 1 {
            out[out_row + 1] = ((invalue * 3 + src[in_row + 1] as i32 + 2) >> 2) as u8;
        } else {
            out[out_row + 1] = invalue as u8;
        }

        for col in 1..src_w - 1 {
            invalue = src[in_row + col] as i32 * 3;
            out[out_row + col * 2] = ((invalue + src[in_row + col - 1] as i32 + 1) >> 2) as u8;
            out[out_row + col * 2 + 1] = ((invalue + src[in_row + col + 1] as i32 + 2) >> 2) as u8;
        }

        if src_w > 1 {
            invalue = src[in_row + src_w - 1] as i32;
            out[out_row + (src_w - 1) * 2] =
                ((invalue * 3 + src[in_row + src_w - 2] as i32 + 1) >> 2) as u8;
            out[out_row + (src_w - 1) * 2 + 1] = invalue as u8;
        }
    }
    out
}

/// 2x2 fancy upsampling — exact match of IJG libjpeg h2v2_fancy_upsample.
pub(super) fn h2v2_fancy_upsample(src: &[u8], src_w: usize, src_h: usize) -> Vec<u8> {
    let dst_w = src_w * 2;
    let dst_h = src_h * 2;
    let mut out = vec![0u8; dst_w * dst_h];
    let mut inrow = 0usize;
    let mut outrow = 0usize;

    while outrow < dst_h {
        for v in 0..2 {
            if outrow >= dst_h {
                break;
            }

            let inptr0 = &src[inrow * src_w..];
            let inptr1 = if v == 0 {
                if inrow > 0 {
                    &src[(inrow - 1) * src_w..]
                } else {
                    &src[inrow * src_w..]
                }
            } else {
                if inrow + 1 < src_h {
                    &src[(inrow + 1) * src_w..]
                } else {
                    &src[inrow * src_w..]
                }
            };

            let out_row = outrow * dst_w;

            let mut thiscolsum = inptr0[0] as i32 * 3 + inptr1[0] as i32;
            let mut nextcolsum = if src_w > 1 {
                inptr0[1] as i32 * 3 + inptr1[1] as i32
            } else {
                thiscolsum
            };
            out[out_row] = ((thiscolsum * 4 + 8) >> 4) as u8;
            out[out_row + 1] = ((thiscolsum * 3 + nextcolsum + 7) >> 4) as u8;
            let mut lastcolsum = thiscolsum;
            thiscolsum = nextcolsum;

            for col in 1..src_w - 1 {
                nextcolsum = inptr0[col + 1] as i32 * 3 + inptr1[col + 1] as i32;
                out[out_row + col * 2] = ((thiscolsum * 3 + lastcolsum + 8) >> 4) as u8;
                out[out_row + col * 2 + 1] = ((thiscolsum * 3 + nextcolsum + 7) >> 4) as u8;
                lastcolsum = thiscolsum;
                thiscolsum = nextcolsum;
            }

            if src_w > 1 {
                out[out_row + (src_w - 1) * 2] = ((thiscolsum * 3 + lastcolsum + 8) >> 4) as u8;
                out[out_row + (src_w - 1) * 2 + 1] = ((thiscolsum * 4 + 7) >> 4) as u8;
            } else {
                out[out_row] = ((thiscolsum * 4 + 8) >> 4) as u8;
                if dst_w > 1 {
                    out[out_row + 1] = ((thiscolsum * 4 + 7) >> 4) as u8;
                }
            }

            outrow += 1;
        }
        inrow += 1;
    }
    out
}

/// Crop a component buffer to the valid image-derived dimensions.
///
/// The component buffer is padded to MCU-aligned boundaries. Chroma data
/// beyond the actual image area must not be fed into the upsampler,
/// or the triangle filter blends garbage padding values at image edges.
pub(super) fn crop_component(
    buf: &[u8],
    buf_w: usize,
    _buf_h: usize,
    crop_w: usize,
    crop_h: usize,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(crop_w * crop_h);
    for y in 0..crop_h {
        let src_off = y * buf_w;
        out.extend_from_slice(&buf[src_off..src_off + crop_w]);
    }
    out
}

/// Dispatch to libjpeg-exact chroma upsampling based on ratios.
pub(super) fn fancy_upsample(
    src: &[u8],
    src_w: usize,
    src_h: usize,
    h_ratio: usize,
    v_ratio: usize,
    _dst_w: usize,
    _dst_h: usize,
) -> Vec<u8> {
    match (h_ratio, v_ratio) {
        (1, 1) => {
            let mut out = Vec::with_capacity(src_w * src_h);
            for y in 0..src_h {
                let row = y * src_w;
                for x in 0..src_w {
                    out.push(src[row + x]);
                }
            }
            out
        }
        (2, 1) => h2v1_fancy_upsample(src, src_w, src_h),
        (2, 2) => h2v2_fancy_upsample(src, src_w, src_h),
        _ => {
            // Integer-only nearest-neighbor fallback for other ratios
            let out_w = src_w * h_ratio;
            let out_h = src_h * v_ratio;
            let mut out = vec![0u8; out_w * out_h];
            for y in 0..out_h {
                let sy = y / v_ratio;
                for x in 0..out_w {
                    let sx = x / h_ratio;
                    out[y * out_w + x] = src[sy * src_w + sx];
                }
            }
            out
        }
    }
}
