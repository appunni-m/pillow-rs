// Duplicate: identity copy
// Modes 0-3 copy the generic RGBA transport. Mode 9 copies packed native
// bytes, four bytes per word, so L/LA/RGB/RGBA and indexed data stay native.

struct Params {
    width: u32,
    height: u32,
    mode: u32,    // 0-3=generic RGBA modes; 9=packed native bytes
    _pad: u32,
}

// ── Mode helpers ──

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u || m == 4u || m == 5u || m == 7u || m == 8u; }

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let idx = gid.y * params.width + gid.x;
    if params.mode == 9u {
        if idx >= params._pad { return; }
        output[idx] = input[idx];
        return;
    }

    let pixel = input[idx];
    let r = pixel & 0xffu;
    let g = (pixel >> 8u) & 0xffu;
    let b = (pixel >> 16u) & 0xffu;
    let a = (pixel >> 24u) & 0xffu;

    // Identity copy; clamp alpha to 255 for non-alpha modes.
    let out_r = r;
    let out_g = g;
    let out_b = b;
    let out_a = select(255u, a, mode_has_a(params.mode));

    output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}
