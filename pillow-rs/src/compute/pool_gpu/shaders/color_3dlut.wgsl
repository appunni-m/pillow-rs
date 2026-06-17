// Color3DLut: 3D color lookup table for color grading.
// Basic pass-through implementation — CPU fallback handles full LUT logic.
// Will be enhanced with proper trilinear interpolation in follow-up.
// Mode-aware. Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
}

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let idx = gid.y * params.width + gid.x;
    let pixel = input[idx];

    let r = pixel & 0xffu;
    let g = (pixel >> 8u) & 0xffu;
    let b2 = (pixel >> 16u) & 0xffu;
    let a = (pixel >> 24u) & 0xffu;

    let out_r = r;
    let out_g = select(0u, g, mode_has_g(params.mode));
    let out_b = select(0u, b2, mode_has_b(params.mode));
    let out_a = select(255u, a, mode_has_a(params.mode));

    output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}
