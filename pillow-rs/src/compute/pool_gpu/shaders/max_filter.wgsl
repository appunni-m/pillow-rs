// Max filter: window maximum for each channel independently.
// Mode-aware: for L/LA (0/1) only computes max on R channel (luma).
// CPU reference (image.rs:2783): rank_filter_impl(img, size, size*size - 1)
// For each pixel: find maximum value in size×size window per channel.
// Border pixels: clamp source coordinates to image bounds (matching PIL).

struct Params {
    width: u32,
    height: u32,
    mode: u32,    // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    size: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

fn max_pixel(x: u32, y: u32) -> u32 {
    let w = params.width;
    let h = params.height;
    let size = min(params.size, 9u);
    let idx = y * w + x;
    if size == 0u || size % 2u == 0u {
        return input[idx];
    }
    let half = i32(size) / 2i;

    var max_r: u32 = 0u;
    var max_g: u32 = 0u;
    var max_b: u32 = 0u;
    var max_a: u32 = 0u;

    let y_i32 = i32(y);
    let x_i32 = i32(x);
    let w_i32 = i32(w);
    let h_i32 = i32(h);

    for (var dy = -half; dy <= half; dy++) {
        let sy = clamp(y_i32 + dy, 0, h_i32 - 1);
        for (var dx = -half; dx <= half; dx++) {
            let sx = clamp(x_i32 + dx, 0, w_i32 - 1);
            let sample = input[u32(sy) * w + u32(sx)];
            max_r = max(max_r, sample & 0xffu);
            max_g = max(max_g, (sample >> 8u) & 0xffu);
            max_b = max(max_b, (sample >> 16u) & 0xffu);
            max_a = max(max_a, (sample >> 24u) & 0xffu);
        }
    }

    // Mode-aware output: for L/LA modes, only R is computed; G/B/A preserved from input
    let in_pixel = input[y * w + x];
    let in_g = (in_pixel >> 8u) & 0xffu;
    let in_b = (in_pixel >> 16u) & 0xffu;
    let in_a = (in_pixel >> 24u) & 0xffu;

    let out_r = max_r;
    let out_g = select(in_g, max_g, mode_has_g(params.mode));
    let out_b = select(in_b, max_b, mode_has_b(params.mode));
    let out_a = select(255u, max_a, mode_has_a(params.mode));

    return out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }
    let idx = gid.y * params.width + gid.x;
    output[idx] = max_pixel(gid.x, gid.y);
}
