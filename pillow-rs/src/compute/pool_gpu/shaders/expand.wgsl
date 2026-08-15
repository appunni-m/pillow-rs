// Expand: add a border of `fill` color around the source image.
// Output dimensions = (w + 2*border) x (h + 2*border)
// Border pixels = fill_color; inner region = source image.
// Mode-aware: fill color respects image mode channels.
// Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA
// Packed u32 RGBA: byte0=R, byte1=G, byte2=B, byte3=A
//
// Source dimensions come from header width/height (= cur_w/cur_h).
// Output dimensions computed as (width + 2*border, height + 2*border).

struct Params {
    width: u32,    // source width (from header = cur_w)
    height: u32,   // source height (from header = cur_h)
    mode: u32,     // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    border: u32,   // border width in pixels
    fill: u32,     // packed fill color (0xAABBGGRR)
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
    // Keep the dimension arithmetic total even if a malformed uniform is
    // submitted without the host's checked output-dimension preflight.
    if b > (0xffffffffu - params.width) / 2u
        || b > (0xffffffffu - params.height) / 2u {
        return;
    }
    let out_w = params.width + 2u * b;
    let out_h = params.height + 2u * b;
    if gid.x >= out_w || gid.y >= out_h { return; }

    let idx = gid.y * out_w + gid.x;

    // Check if pixel is in the border region
    if gid.x < b || gid.x >= b + params.width || gid.y < b || gid.y >= b + params.height {
        // Border pixel: use fill color
        let f = params.fill;
        let fr = f & 0xffu;
        let fg = (f >> 8u) & 0xffu;
        let fb = (f >> 16u) & 0xffu;
        let fa = (f >> 24u) & 0xffu;

        let out_r = fr;
        let out_g = select(0u, fg, mode_has_g(params.mode));
        let out_b = select(0u, fb, mode_has_b(params.mode));
        let out_a = select(255u, fa, mode_has_a(params.mode));

        output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
    } else {
        // Inner pixel: copy from source
        let src_x = gid.x - b;
        let src_y = gid.y - b;
        let pixel = input[src_y * params.width + src_x];

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
}
