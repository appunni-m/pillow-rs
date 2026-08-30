// Image.putalpha(mask): replace the destination alpha with an L/1 mask.
//
// PixelMode codes:
//   0=L, 1=LA, 2=RGB, 3=RGBA, 4=P, 5=PA, 6=CMYK.
// Packed GPU transport is u32 RGBA: byte0=R/index/C, byte1=G/M,
// byte2=B/Y, byte3=A/K. The mask is packed through the ordinary L path, so
// its sample is always in byte zero.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    _unused: u32,
    source_mode: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read> _unused_source: array<u32>;
@group(0) @binding(2) var<storage, read> mask: array<u32>;
@group(0) @binding(3) var<storage, read_write> output: array<u32>;
@group(0) @binding(4) var<uniform> params: Params;

fn muldiv255(a: u32, b: u32) -> u32 {
    let value = a * b + 128u;
    return ((value >> 8u) + value) >> 8u;
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height {
        return;
    }

    let index = gid.y * params.width + gid.x;
    let pixel = input[index];
    let alpha = mask[index] & 0xffu;
    let first = pixel & 0xffu;
    let second = (pixel >> 8u) & 0xffu;
    let third = (pixel >> 16u) & 0xffu;
    let fourth = (pixel >> 24u) & 0xffu;

    if params.source_mode == 6u {
        // Pillow Convert.c:cmyk2rgb: promote CMYK through the integer RGB
        // inverse, then replace the new alpha band with the mask sample.
        let nk = 255u - fourth;
        let red = nk - muldiv255(first, nk);
        let green = nk - muldiv255(second, nk);
        let blue = nk - muldiv255(third, nk);
        output[index] = red | (green << 8u) | (blue << 16u) | (alpha << 24u);
        return;
    }

    // L/P keep the one-byte sample in byte zero; RGB keeps its three color
    // bytes. LA/PA's old alpha and RGBA's old alpha are replaced identically.
    output[index] = (pixel & 0x00ffffffu) | (alpha << 24u);
}
