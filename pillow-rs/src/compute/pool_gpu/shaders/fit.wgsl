// Fit: resample Pillow's fractional source crop box directly into the target
// dimensions.  The crop box is computed by the Rust control plane using the
// ImageOps.fit bleed/centering contract and transported as f32 bit patterns.
// The pixel-centre mapping below matches the scalar boxed-resize path.
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
    crop_left: u32,  // f32 bits
    crop_top: u32,   // f32 bits
    crop_w: u32,     // f32 bits
    crop_h: u32,     // f32 bits
    filter_code: u32, // 0=nearest, all other values use bilinear fallback
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

fn nearest_sample(cx: f32, cy: f32) -> u32 {
    let sx = u32(clamp(cx, 0.0, f32(params.width) - 1.0));
    let sy = u32(clamp(cy, 0.0, f32(params.height) - 1.0));
    let p = input[sy * params.width + sx];
    let r = p & 0xffu;
    let g = (p >> 8u) & 0xffu;
    let b = (p >> 16u) & 0xffu;
    let a = (p >> 24u) & 0xffu;
    return r
        | (select(0u, g, mode_has_g(params.mode)) << 8u)
        | (select(0u, b, mode_has_b(params.mode)) << 16u)
        | (select(255u, a, mode_has_a(params.mode)) << 24u);
}

fn bilinear_sample(dx: u32, dy: u32) -> u32 {
    let src_w = params.width;
    let src_h = params.height;
    let crop_left = bitcast<f32>(params.crop_left);
    let crop_top = bitcast<f32>(params.crop_top);
    let crop_w = bitcast<f32>(params.crop_w);
    let crop_h = bitcast<f32>(params.crop_h);

    // Map each output pixel centre into the fractional source crop box.
    let cx = crop_left + (f32(dx) + 0.5) * crop_w / f32(params.dst_w);
    let cy = crop_top + (f32(dy) + 0.5) * crop_h / f32(params.dst_h);

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
    if params.filter_code == 0u {
        let crop_left = bitcast<f32>(params.crop_left);
        let crop_top = bitcast<f32>(params.crop_top);
        let crop_w = bitcast<f32>(params.crop_w);
        let crop_h = bitcast<f32>(params.crop_h);
        let cx = crop_left + (f32(gid.x) + 0.5) * crop_w / f32(params.dst_w);
        let cy = crop_top + (f32(gid.y) + 0.5) * crop_h / f32(params.dst_h);
        output[idx] = nearest_sample(cx, cy);
    } else {
        output[idx] = bilinear_sample(gid.x, gid.y);
    }
}
