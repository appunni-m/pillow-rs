// Image.putalpha: promote the logical Pillow mode and set a uniform alpha.
//
// PixelMode codes:
//   0=L, 1=LA, 2=RGB, 3=RGBA, 4=P, 5=PA, 6=CMYK.
// Packed GPU transport is u32 RGBA: byte0=R/index/C, byte1=G/M,
// byte2=B/Y, byte3=A/K.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    alpha: u32,
    source_mode: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

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
    let first = pixel & 0xffu;
    let second = (pixel >> 8u) & 0xffu;
    let third = (pixel >> 16u) & 0xffu;
    let fourth = (pixel >> 24u) & 0xffu;

    if params.source_mode == 6u {
        // Pillow Convert.c:cmyk2rgb:
        // nk - MULDIV255(component, nk), followed by alpha replacement.
        let nk = 255u - fourth;
        let red = nk - muldiv255(first, nk);
        let green = nk - muldiv255(second, nk);
        let blue = nk - muldiv255(third, nk);
        output[index] =
            red | (green << 8u) | (blue << 16u) | (params.alpha << 24u);
        return;
    }

    // L/P are transported with their sample in byte 0. LA/PA already use
    // byte 0 for luma/index and byte 3 for alpha. RGB/RGBA retain RGB bytes.
    output[index] = (pixel & 0x00ffffffu) | (params.alpha << 24u);
}
