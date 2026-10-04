// Merge: interleave single-channel band images into one multi-channel output.
// Extra bands (1-3) are packed sequentially in the second input at offsets:
//   band1 at offset 0, band2 at offset n, band3 at offset 2*n (where n = width*height)
// Uses the standard dual-input 4-binding layout used by the GPU pool:
// [band0(read), extra_bands(read), output(read_write), params(uniform)].
// Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA

struct Params {
    width: u32,
    height: u32,
    mode: u32,        // output mode; NATIVE_RGB_LUMA selects compact L-plane input
    _pad: u32,
    num_bands: u32,   // number of input band images (1-4), or pixels in compact mode
}

const NATIVE_RGB_LUMA: u32 = 0x4d524742u;

@group(0) @binding(0) var<storage, read> band0: array<u32>;   // R / L channel
@group(0) @binding(1) var<storage, read> extra_bands: array<u32>;  // bands 1-3 packed
@group(0) @binding(2) var<storage, read_write> output: array<u32>;
@group(0) @binding(3) var<uniform> params: Params;

fn native_band0_byte(byte_index: u32) -> u32 {
    let word = band0[byte_index / 4u];
    return (word >> ((byte_index % 4u) * 8u)) & 0xffu;
}

fn native_extra_byte(byte_index: u32) -> u32 {
    let word = extra_bands[byte_index / 4u];
    return (word >> ((byte_index % 4u) * 8u)) & 0xffu;
}

fn native_rgb_luma_pixel(pixel: u32) -> vec3<u32> {
    if pixel >= params.num_bands {
        return vec3<u32>(0u);
    }
    return vec3<u32>(
        native_band0_byte(pixel),
        native_extra_byte(pixel),
        native_extra_byte(params.num_bands + pixel),
    );
}

fn write_native_rgb_luma_group(group: u32) {
    let pixel_count = params.num_bands;
    let output_word_count = params._pad;
    let first_pixel = group * 4u;
    let p0 = native_rgb_luma_pixel(first_pixel);
    let p1 = native_rgb_luma_pixel(first_pixel + 1u);
    let p2 = native_rgb_luma_pixel(first_pixel + 2u);
    let p3 = native_rgb_luma_pixel(first_pixel + 3u);
    let first_word = group * 3u;

    if first_word < output_word_count {
        output[first_word] = p0.x | (p0.y << 8u) | (p0.z << 16u) | (p1.x << 24u);
    }
    if first_word + 1u < output_word_count {
        output[first_word + 1u] = p1.y | (p1.z << 8u) | (p2.x << 16u) | (p2.y << 24u);
    }
    if first_word + 2u < output_word_count {
        output[first_word + 2u] = p2.z | (p3.x << 8u) | (p3.y << 16u) | (p3.z << 24u);
    }
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if params.mode == NATIVE_RGB_LUMA {
        let group = gid.x + gid.y * params.width;
        if group >= params.height { return; }
        write_native_rgb_luma_group(group);
        return;
    }

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
