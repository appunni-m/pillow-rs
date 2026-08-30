// Equalize LUT derivation.
//
// This is the integer Pillow algorithm from `op_equalize`: for each channel,
// `step = (total - last_nonzero_count) / 255`, followed by a half-step CDF
// accumulator. The one control invocation deliberately does the small 256-bin
// reduction; the pixel remap remains in point_op.wgsl.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
}

@group(0) @binding(0) var<storage, read> histogram_data: array<u32>;
@group(0) @binding(1) var<storage, read_write> lut: array<u32, 256>;
@group(0) @binding(2) var<uniform> params: Params;

fn equalize_value(base: u32, value: u32, total: u32) -> u32 {
    var nonzero_bins = 0u;
    var last_nonzero_count = 0u;
    for (var i = 0u; i < 256u; i = i + 1u) {
        let count = histogram_data[base + i];
        if count > 0u {
            nonzero_bins = nonzero_bins + 1u;
            last_nonzero_count = count;
        }
    }
    if nonzero_bins <= 1u {
        return value;
    }

    let step = (total - last_nonzero_count) / 255u;
    if step == 0u {
        return value;
    }

    var n = step / 2u;
    for (var i = 0u; i < value; i = i + 1u) {
        n = n + histogram_data[base + i];
    }
    return min(n / step, 255u);
}

@compute @workgroup_size(256)
fn main(@builtin(local_invocation_id) lid: vec3<u32>) {
    if lid.x != 0u {
        return;
    }

    let total = params.width * params.height;
    let rgb = params.mode >= 2u;
    for (var i = 0u; i < 256u; i = i + 1u) {
        let red = equalize_value(0u, i, total);
        var green = i;
        var blue = i;
        if rgb {
            green = equalize_value(256u, i, total);
            blue = equalize_value(512u, i, total);
        }
        lut[i] = red | (green << 8u) | (blue << 16u) | 0xff000000u;
    }
}
