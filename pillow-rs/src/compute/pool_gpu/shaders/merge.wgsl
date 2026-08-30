// Merge: interleave single-channel band images into one multi-channel output.
// Extra bands (1-3) are packed sequentially in the second input at offsets:
//   band1 at offset 0, band2 at offset n, band3 at offset 2*n (where n = width*height)
// Uses the standard dual-input 4-binding layout used by the GPU pool:
// [band0(read), extra_bands(read), output(read_write), params(uniform)].
// Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA

struct Params {
    width: u32,
    height: u32,
    mode: u32,        // output mode: 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    num_bands: u32,   // number of input band images (1-4)
}

@group(0) @binding(0) var<storage, read> band0: array<u32>;   // R / L channel
@group(0) @binding(1) var<storage, read> extra_bands: array<u32>;  // bands 1-3 packed
@group(0) @binding(2) var<storage, read_write> output: array<u32>;
@group(0) @binding(3) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let idx = gid.y * params.width + gid.x;
    let nb = params.num_bands;
    let n = params.width * params.height;

    // Band 0 is always present (R or L)
    let r_val = band0[idx] & 0xffu;

    var g_val = 0u;
    var b_val = 0u;
    var a_val = 255u;

    if nb == 2u {
        // LA mode: band1 is A channel (at offset 0 in extra_bands)
        a_val = extra_bands[idx] & 0xffu;
    } else if nb == 3u {
        // RGB mode: band1=G (offset 0), band2=B (offset n)
        g_val = extra_bands[idx] & 0xffu;
        b_val = extra_bands[n + idx] & 0xffu;
    } else if nb >= 4u {
        // RGBA mode: band1=G (offset 0), band2=B (offset n), band3=A (offset 2n)
        g_val = extra_bands[idx] & 0xffu;
        b_val = extra_bands[n + idx] & 0xffu;
        a_val = extra_bands[2u * n + idx] & 0xffu;
    }

    output[idx] = r_val | (g_val << 8u) | (b_val << 16u) | (a_val << 24u);
}
