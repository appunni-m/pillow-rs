// Convert packed source samples to Pillow-compatible L and pack four output
// pixels per word. CMYK remains C/M/Y/K in the input word and is expanded to
// RGB only in registers before luma; no host RGBA image is materialized.

struct Params {
    width: u32,
    height: u32,
    mode: u32, // 0=L, 1=LA, 2=RGB, 3=RGBA, 4=CMYK
    _pad: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn muldiv255(a: u32, b: u32) -> u32 {
    let value = a * b + 128u;
    return ((value >> 8u) + value) >> 8u;
}

fn pixel_luma(pixel: u32) -> u32 {
    let first = pixel & 0xffu;
    let second = (pixel >> 8u) & 0xffu;
    let third = (pixel >> 16u) & 0xffu;
    if params.mode == 4u {
        let k = (pixel >> 24u) & 0xffu;
        let ink = 255u - k;
        let red = ink - muldiv255(first, ink);
        let green = ink - muldiv255(second, ink);
        let blue = ink - muldiv255(third, ink);
        return (19595u * red + 38470u * green + 7471u * blue + 32768u) >> 16u;
    }
    // L and LA uploads replicate their luma value across RGB. RGBA ignores
    // alpha, matching Pillow's ImageOps.grayscale contract.
    return (19595u * first + 38470u * second + 7471u * third + 32768u) >> 16u;
}

@compute @workgroup_size(64, 1, 1)
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(num_workgroups) workgroups: vec3<u32>,
) {
    let pixel_count = params.width * params.height;
    let output_word_count = pixel_count / 4u + select(0u, 1u, pixel_count % 4u != 0u);
    let output_word = gid.x + gid.y * workgroups.x * 64u;
    if output_word >= output_word_count { return; }

    var packed = 0u;
    for (var lane = 0u; lane < 4u; lane += 1u) {
        let pixel_index = output_word * 4u + lane;
        if pixel_index < pixel_count {
            packed |= pixel_luma(input[pixel_index]) << (lane * 8u);
        }
    }
    output[output_word] = packed;
}
