// Gaussian blur (2D direct, not separable).
// For each pixel: weighted sum over (2*radius+1)² window using Gaussian weights.
// Weight: exp(-(dx²+dy²) / (2*sigma²)), sigma = radius / 3.0
// Normalized by sum of weights within the kernel window.
//
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

fn gaussian_weight_sq(dist_sq: f32, sigma_sq: f32) -> f32 {
    return exp(-dist_sq / (2.0 * sigma_sq));
}

fn gaussian_blur_pixel(x: u32, y: u32) -> u32 {
    let w = params.width;
    let h = params.height;
    let radius = params.radius;
    let mode = params.mode;
    let idx = y * w + x;

    if radius == 0u {
        return input[idx];
    }

    let orig = input[idx];
    let orig_r = orig & 0xffu;
    let orig_g = (orig >> 8u) & 0xffu;
    let orig_b = (orig >> 16u) & 0xffu;
    let orig_a = (orig >> 24u) & 0xffu;

    let sigma = f32(radius) / 3.0;
    let sigma_sq = sigma * sigma;
    let r_i32 = i32(radius);
    let w_i32 = i32(w);
    let h_i32 = i32(h);
    let x_i32 = i32(x);
    let y_i32 = i32(y);

    var sum_r: f32 = 0.0;
    var sum_g: f32 = 0.0;
    var sum_b: f32 = 0.0;
    var sum_a: f32 = 0.0;
    var total_wgt: f32 = 0.0;

    // 2D Gaussian-weighted sum over the square window
    for (var dy = -r_i32; dy <= r_i32; dy++) {
        let sy = clamp(y_i32 + dy, 0, h_i32 - 1);
        for (var dx = -r_i32; dx <= r_i32; dx++) {
            let sx = clamp(x_i32 + dx, 0, w_i32 - 1);
            let dist_sq = f32(dx * dx + dy * dy);
            let wgt = gaussian_weight_sq(dist_sq, sigma_sq);
            let sample = input[u32(sy) * w + u32(sx)];
            sum_r = sum_r + wgt * f32(sample & 0xffu);
            sum_g = sum_g + wgt * f32((sample >> 8u) & 0xffu);
            sum_b = sum_b + wgt * f32((sample >> 16u) & 0xffu);
            sum_a = sum_a + wgt * f32((sample >> 24u) & 0xffu);
            total_wgt = total_wgt + wgt;
        }
    }

    // Normalize, round, and clamp
    let blurred_r = u32(clamp(sum_r / total_wgt + 0.5, 0.0, 255.0));
    let blurred_g = u32(clamp(sum_g / total_wgt + 0.5, 0.0, 255.0));
    let blurred_b = u32(clamp(sum_b / total_wgt + 0.5, 0.0, 255.0));
    let blurred_a = u32(clamp(sum_a / total_wgt + 0.5, 0.0, 255.0));

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
    output[idx] = gaussian_blur_pixel(gid.x, gid.y);
}
