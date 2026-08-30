// Autocontrast LUT derivation.
//
// The host computes the percentile ranks with the same truncation as Pillow;
// this pass performs only integer histogram lookup and integer LUT mapping.
// That keeps the result independent of device f32 division and matches
// `autocontrast_lut`'s `int(ix * scale + offset)` contract exactly.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    cutoff_bits: u32,
    cutoff_low: u32,
    cutoff_high: u32,
    selected_pixels: u32,
}

@group(0) @binding(0) var<storage, read> histogram_data: array<u32>;
@group(0) @binding(1) var<storage, read_write> lut: array<u32, 256>;
@group(0) @binding(2) var<uniform> params: Params;

fn histogram_value(base: u32, rank: u32) -> u32 {
    var remaining = rank;
    for (var i = 0u; i < 256u; i = i + 1u) {
        let count = histogram_data[base + i];
        if remaining < count {
            return i;
        }
        remaining = remaining - count;
    }
    return 0u;
}

fn remap(value: u32, lo: u32, hi: u32) -> u32 {
    if value <= lo {
        return 0u;
    }
    if value >= hi {
        return 255u;
    }
    return ((value - lo) * 255u) / (hi - lo);
}

@compute @workgroup_size(256)
fn main(@builtin(local_invocation_id) lid: vec3<u32>) {
    if lid.x != 0u {
        return;
    }

    // Identity is also the correct result for an all-zero mask and for a
    // channel whose selected samples all have the same value.
    for (var i = 0u; i < 256u; i = i + 1u) {
        lut[i] = i | (i << 8u) | (i << 16u) | 0xff000000u;
    }
    if params.selected_pixels == 0u {
        return;
    }

    let low_rank = params.cutoff_low;
    let high_rank = params.cutoff_high;
    let red_lo = histogram_value(0u, low_rank);
    let red_hi = histogram_value(0u, high_rank);
    if red_hi > red_lo {
        for (var i = 0u; i < 256u; i = i + 1u) {
            let value = remap(i, red_lo, red_hi);
            lut[i] = (lut[i] & 0xffffff00u) | value;
        }
    }

    if params.mode >= 2u {
        let green_lo = histogram_value(256u, low_rank);
        let green_hi = histogram_value(256u, high_rank);
        if green_hi > green_lo {
            for (var i = 0u; i < 256u; i = i + 1u) {
                let value = remap(i, green_lo, green_hi);
                lut[i] = (lut[i] & 0xffff00ffu) | (value << 8u);
            }
        }

        let blue_lo = histogram_value(512u, low_rank);
        let blue_hi = histogram_value(512u, high_rank);
        if blue_hi > blue_lo {
            for (var i = 0u; i < 256u; i = i + 1u) {
                let value = remap(i, blue_lo, blue_hi);
                lut[i] = (lut[i] & 0xff00ffffu) | (value << 16u);
            }
        }
    }
}
