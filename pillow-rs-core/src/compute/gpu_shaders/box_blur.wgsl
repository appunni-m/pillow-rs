// Combined box blur (both horizontal and vertical passes).
// For each pixel: sum (2*radius+1) pixels in the square window (clamped to bounds),
// then apply i32 fixed-point division: (sum * 65536 / window_sq + 32768) >> 16.
//
// Equivalent to PIL's separable H then V, computed as a direct 2D sum.
// For typical small radii this is efficient; for large radii consider
// using box_blur_h.wgsl + box_blur_v.wgsl in two separate dispatches.
//
// CPU reference (image.rs:2737-2781): two-pass separable box blur.
// Mode-aware: L/LA only output blurred R channel; RGB output R,G,B; RGBA output R,G,B,A.
// Params: radius (u32) after standard header.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    radius: u32,
}

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn box_blur_pixel(x: u32, y: u32) -> u32 {
    let w = params.width;
    let h = params.height;
    let radius = params.radius;
    let mode = params.mode;
    let window = 2u * radius + 1u;
    let window_sq = window * window;
    let idx = y * w + x;

    if radius == 0u {
        return input[idx];
    }

    let weight = 65536i / i32(window_sq);
    let bias = 32768i;

    let orig = input[idx];
    let orig_r = orig & 0xffu;
    let orig_g = (orig >> 8u) & 0xffu;
    let orig_b = (orig >> 16u) & 0xffu;
    let orig_a = (orig >> 24u) & 0xffu;

    var sum_r: i32 = 0;
    var sum_g: i32 = 0;
    var sum_b: i32 = 0;
    var sum_a: i32 = 0;

    let x_i32 = i32(x);
    let y_i32 = i32(y);
    let w_i32 = i32(w);
    let h_i32 = i32(h);
    let r_i32 = i32(radius);

    // 2D box sum over the square window
    for (var dy = -r_i32; dy <= r_i32; dy++) {
        let sy = clamp(y_i32 + dy, 0, h_i32 - 1);
        for (var dx = -r_i32; dx <= r_i32; dx++) {
            let sx = clamp(x_i32 + dx, 0, w_i32 - 1);
            let sample = input[u32(sy) * w + u32(sx)];
            sum_r = sum_r + i32(sample & 0xffu);
            sum_g = sum_g + i32((sample >> 8u) & 0xffu);
            sum_b = sum_b + i32((sample >> 16u) & 0xffu);
            sum_a = sum_a + i32((sample >> 24u) & 0xffu);
        }
    }

    let blurred_r = u32(clamp((sum_r * weight + bias) >> 16, 0, 255));
    let blurred_g = u32(clamp((sum_g * weight + bias) >> 16, 0, 255));
    let blurred_b = u32(clamp((sum_b * weight + bias) >> 16, 0, 255));
    let blurred_a = u32(clamp((sum_a * weight + bias) >> 16, 0, 255));

    let out_r = blurred_r;
    let out_g = select(orig_g, blurred_g, mode_has_g(mode));
    let out_b = select(orig_b, blurred_b, mode_has_b(mode));
    let out_a = select(orig_a, blurred_a, mode_has_a(mode));

    return out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }
    let idx = gid.y * params.width + gid.x;
    output[idx] = box_blur_pixel(gid.x, gid.y);
}
