// Transform: affine image transform.
// Map destination pixels to source coordinates via affine transform:
//   sx = a*dx + b*dy + c
//   sy = d*dx + e*dy + f
// Where (dx, dy) are destination pixel coordinates and (sx, sy) are
// source pixel coordinates (floating-point).
//
// Sampling: nearest-neighbor (filter_code=0) or bilinear (filter_code=1).
// Out-of-bounds source coordinates are filled with fill_color (packed u32 RGBA).
//
// Dispatch at dst_w x dst_h (output image dimensions).
//
// 3-binding layout: input (source), output, params.
//
// Mode-aware: channel selection per mode.
// Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA

struct Params {
    width: u32,      // source width
    height: u32,     // source height
    mode: u32,
    _pad: u32,
    dst_w: u32,      // output width
    dst_h: u32,      // output height
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    e: f32,
    f: f32,
    fill_color: u32, // packed RGBA: R|G<<8|B<<16|A<<24
    filter_code: u32, // 0=nearest, 1=bilinear
}

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    return a * (1.0 - t) + b * t;
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn get_fill_pixel() -> u32 {
    let fc = params.fill_color;
    let fr = fc & 0xffu;
    let fg = (fc >> 8u) & 0xffu;
    let fb = (fc >> 16u) & 0xffu;
    let fa = (fc >> 24u) & 0xffu;
    // Mode-aware: only keep channels present in the image mode
    let out_g = select(0u, fg, mode_has_g(params.mode));
    let out_b = select(0u, fb, mode_has_b(params.mode));
    let out_a = select(255u, fa, mode_has_a(params.mode));
    return fr | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}

fn sample_nearest(sx: f32, sy: f32) -> u32 {
    let ix = i32(round(sx));
    let iy = i32(round(sy));
    if ix < 0 || iy < 0 || u32(ix) >= params.width || u32(iy) >= params.height {
        return get_fill_pixel();
    }
    let idx = u32(iy) * params.width + u32(ix);
    let pixel = input[idx];
    let sr = pixel & 0xffu;
    let sg = (pixel >> 8u) & 0xffu;
    let sb = (pixel >> 16u) & 0xffu;
    let sa = (pixel >> 24u) & 0xffu;

    let out_g = select(0u, sg, mode_has_g(params.mode));
    let out_b = select(0u, sb, mode_has_b(params.mode));
    let out_a = select(255u, sa, mode_has_a(params.mode));

    return sr | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}

fn sample_bilinear(sx: f32, sy: f32) -> u32 {
    let src_w_f = f32(params.width);
    let src_h_f = f32(params.height);

    // Out-of-bounds check
    if sx < 0.0 || sx >= src_w_f || sy < 0.0 || sy >= src_h_f {
        return get_fill_pixel();
    }

    // Floor and fractional parts
    let x0_f = floor(sx);
    let y0_f = floor(sy);
    let fx = sx - x0_f;
    let fy = sy - y0_f;

    // Clamp to valid source pixel indices
    let x0 = u32(clamp(x0_f, 0.0, src_w_f - 1.0));
    let y0 = u32(clamp(y0_f, 0.0, src_h_f - 1.0));
    let x1 = u32(clamp(x0_f + 1.0, 0.0, src_w_f - 1.0));
    let y1 = u32(clamp(y0_f + 1.0, 0.0, src_h_f - 1.0));

    // Load 4 neighboring pixels
    let p00 = input[y0 * params.width + x0];
    let p10 = input[y0 * params.width + x1];
    let p01 = input[y1 * params.width + x0];
    let p11 = input[y1 * params.width + x1];

    // Extract channels
    let r00 = f32(p00 & 0xffu);
    let g00 = f32((p00 >> 8u) & 0xffu);
    let b00 = f32((p00 >> 16u) & 0xffu);
    let a00 = f32((p00 >> 24u) & 0xffu);

    let r10 = f32(p10 & 0xffu);
    let g10 = f32((p10 >> 8u) & 0xffu);
    let b10 = f32((p10 >> 16u) & 0xffu);
    let a10 = f32((p10 >> 24u) & 0xffu);

    let r01 = f32(p01 & 0xffu);
    let g01 = f32((p01 >> 8u) & 0xffu);
    let b01 = f32((p01 >> 16u) & 0xffu);
    let a01 = f32((p01 >> 24u) & 0xffu);

    let r11 = f32(p11 & 0xffu);
    let g11 = f32((p11 >> 8u) & 0xffu);
    let b11 = f32((p11 >> 16u) & 0xffu);
    let a11 = f32((p11 >> 24u) & 0xffu);

    // Bilinear: top row, then bottom row, then vertical mix
    let top_r = lerp(r00, r10, fx);
    let top_g = lerp(g00, g10, fx);
    let top_b = lerp(b00, b10, fx);
    let top_a = lerp(a00, a10, fx);

    let bot_r = lerp(r01, r11, fx);
    let bot_g = lerp(g01, g11, fx);
    let bot_b = lerp(b01, b11, fx);
    let bot_a = lerp(a01, a11, fx);

    let out_r_f = lerp(top_r, bot_r, fy);
    let out_g_f = lerp(top_g, bot_g, fy);
    let out_b_f = lerp(top_b, bot_b, fy);
    let out_a_f = lerp(top_a, bot_a, fy);

    // Round to u8 with clamping and mode-aware channel selection
    let out_r = u32(clamp(out_r_f + 0.5, 0.0, 255.0));
    let out_g = select(0u, u32(clamp(out_g_f + 0.5, 0.0, 255.0)), mode_has_g(params.mode));
    let out_b = select(0u, u32(clamp(out_b_f + 0.5, 0.0, 255.0)), mode_has_b(params.mode));
    let out_a = select(255u, u32(clamp(out_a_f + 0.5, 0.0, 255.0)), mode_has_a(params.mode));

    return out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.dst_w || gid.y >= params.dst_h { return; }

    let dx = f32(gid.x);
    let dy = f32(gid.y);

    // Affine mapping: dest -> source
    let sx = params.a * dx + params.b * dy + params.c;
    let sy = params.d * dx + params.e * dy + params.f;

    let idx = gid.y * params.dst_w + gid.x;

    if params.filter_code == 0u {
        output[idx] = sample_nearest(sx, sy);
    } else {
        output[idx] = sample_bilinear(sx, sy);
    }
}
