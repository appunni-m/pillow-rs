// Cover: scale to cover target box (maintaining aspect ratio) then center-crop.
// 1. Scale source to nw x nh so it completely covers the target box.
// 2. Center-crop to dst_w x dst_h starting at (crop_x, crop_y).
// Uses PIL pixel-centered bilinear sampling (same as resize_bilinear.wgsl).
// Mode-aware: for L/LA (0/1) only interpolates R channel (luma).
// Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA
// Packed u32 RGBA: byte0=R, byte1=G, byte2=B, byte3=A

struct Params {
    width: u32,    // source width
    height: u32,   // source height
    mode: u32,     // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    dst_w: u32,    // output / target width
    dst_h: u32,    // output / target height
    nw: u32,       // scaled (cover) width
    nh: u32,       // scaled (cover) height
    crop_x: u32,   // center-crop X offset in cover image
    crop_y: u32,   // center-crop Y offset in cover image
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

// Linear interpolation between two f32 values
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    return a * (1.0 - t) + b * t;
}

fn bilinear_sample(dx: u32, dy: u32) -> u32 {
    let src_w = params.width;
    let src_h = params.height;
    let nw = params.nw;
    let nh = params.nh;

    // Map output pixel (dx, dy) to cover-image position then to source.
    // cover_pos = (crop_x + dx, crop_y + dy)
    // Then PIL pixel-centered inverse-map to source:
    let cx = (f32(params.crop_x + dx) + 0.5) * f32(src_w) / f32(nw);
    let cy = (f32(params.crop_y + dy) + 0.5) * f32(src_h) / f32(nh);

    // Floor and fractional parts
    let sx_floor = floor(cx);
    let sy_floor = floor(cy);
    let fx = cx - sx_floor;
    let fy = cy - sy_floor;

    // Source pixel indices, clamped to [0, src_dim-1]
    let x0 = u32(clamp(sx_floor, 0.0, f32(src_w - 1u)));
    let y0 = u32(clamp(sy_floor, 0.0, f32(src_h - 1u)));
    let x1 = u32(clamp(sx_floor + 1.0, 0.0, f32(src_w - 1u)));
    let y1 = u32(clamp(sy_floor + 1.0, 0.0, f32(src_h - 1u)));

    // Load 4 neighboring pixels
    let p00 = input[y0 * src_w + x0];
    let p10 = input[y0 * src_w + x1];
    let p01 = input[y1 * src_w + x0];
    let p11 = input[y1 * src_w + x1];

    // Extract channel bytes from each pixel
    let r00 = f32(p00 & 0xffu);        let g00 = f32((p00 >> 8u) & 0xffu);
    let b00 = f32((p00 >> 16u) & 0xffu); let a00 = f32((p00 >> 24u) & 0xffu);
    let r10 = f32(p10 & 0xffu);        let g10 = f32((p10 >> 8u) & 0xffu);
    let b10 = f32((p10 >> 16u) & 0xffu); let a10 = f32((p10 >> 24u) & 0xffu);
    let r01 = f32(p01 & 0xffu);        let g01 = f32((p01 >> 8u) & 0xffu);
    let b01 = f32((p01 >> 16u) & 0xffu); let a01 = f32((p01 >> 24u) & 0xffu);
    let r11 = f32(p11 & 0xffu);        let g11 = f32((p11 >> 8u) & 0xffu);
    let b11 = f32((p11 >> 16u) & 0xffu); let a11 = f32((p11 >> 24u) & 0xffu);

    // Bilinear interpolation: top row
    let top_r = lerp(r00, r10, fx);
    let top_g = lerp(g00, g10, fx);
    let top_b = lerp(b00, b10, fx);
    let top_a = lerp(a00, a10, fx);

    // bottom row
    let bot_r = lerp(r01, r11, fx);
    let bot_g = lerp(g01, g11, fx);
    let bot_b = lerp(b01, b11, fx);
    let bot_a = lerp(a01, a11, fx);

    // vertical mix
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
    let idx = gid.y * params.dst_w + gid.x;
    output[idx] = bilinear_sample(gid.x, gid.y);
}
