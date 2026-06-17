// Mirror: horizontal coordinate inversion
// output[y][x] = input[y][W-1-x]
// Mode-aware: preserves pixel channels, sets alpha=255 for non-alpha modes.
// Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA
// Packed u32 RGBA: byte0=R, byte1=G, byte2=B, byte3=A

struct Params {
    width: u32,
    height: u32,
    mode: u32,    // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
}

// ── Mode helpers ──

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let w = params.width;
    let h = params.height;

    // Source: horizontally mirrored
    let src_x = w - 1u - gid.x;
    let src_idx = gid.y * w + src_x;
    let dst_idx = gid.y * w + gid.x;

    let pixel = input[src_idx];
    let r = pixel & 0xffu;
    let g = (pixel >> 8u) & 0xffu;
    let b = (pixel >> 16u) & 0xffu;
    let a = (pixel >> 24u) & 0xffu;

    // Preserve all pixel channels; clamp alpha to 255 for non-alpha modes.
    let out_r = r;
    let out_g = g;
    let out_b = b;
    let out_a = select(255u, a, mode_has_a(params.mode));

    output[dst_idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}
