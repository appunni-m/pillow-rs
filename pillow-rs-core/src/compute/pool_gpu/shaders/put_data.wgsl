// PutData: replace pixel data from storage buffer.
// Copy data buffer values directly to output.
// input buffer is unused (present for layout compatibility).
//
// 4-binding layout: input (unused), output, params, data.
//
// Mode-aware alpha clamp:
//   For modes without alpha (L=0, RGB=2): force alpha to 255.
//   For modes with alpha (LA=1, RGBA=3): preserve alpha from data.
// Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA

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
@group(0) @binding(3) var<storage, read> data: array<u32>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let idx = gid.y * params.width + gid.x;
    let pixel = data[idx];

    // Mode-aware alpha clamp
    let a = (pixel >> 24u) & 0xffu;
    let out_a = select(255u, a, mode_has_a(params.mode));

    // Copy pixel data with clamped alpha
    output[idx] = (pixel & 0x00ffffffu) | (out_a << 24u);
}
