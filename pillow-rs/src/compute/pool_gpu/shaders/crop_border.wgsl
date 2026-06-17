// CropBorder: remove `border` pixels from all four edges.
// Output dimensions = (w - 2*border) x (h - 2*border)
// Mode-aware: preserves pixel channels, sets alpha=255 for non-alpha modes.
// Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA
// Packed u32 RGBA: byte0=R, byte1=G, byte2=B, byte3=A

struct Params {
    width: u32,    // source width
    height: u32,   // source height
    mode: u32,     // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    border: u32,   // pixels to crop from each edge
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
    let b = params.border;
    let out_w = params.width - 2u * b;
    let out_h = params.height - 2u * b;
    if gid.x >= out_w || gid.y >= out_h { return; }

    let src_x = b + gid.x;
    let src_y = b + gid.y;
    let pixel = input[src_y * params.width + src_x];

    let r = pixel & 0xffu;
    let g = (pixel >> 8u) & 0xffu;
    let b2 = (pixel >> 16u) & 0xffu;
    let a = (pixel >> 24u) & 0xffu;

    let out_r = r;
    let out_g = select(0u, g, mode_has_g(params.mode));
    let out_b = select(0u, b2, mode_has_b(params.mode));
    let out_a = select(255u, a, mode_has_a(params.mode));

    let idx = gid.y * out_w + gid.x;
    output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}
