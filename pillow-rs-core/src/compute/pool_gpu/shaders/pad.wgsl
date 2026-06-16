// Pad: place source image into a larger canvas with centering.
// If source fits: place at center, fill border with color.
// If source too large: resize to fit (bilinear), then pad.
// Output dimensions = dst_w x dst_h (from params, also dispatch size)
// Source dimensions come from the header width/height (= cur_w/cur_h).
// Mode-aware. Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA

struct Params {
    width: u32,    // source width (from header = cur_w)
    height: u32,   // source height (from header = cur_h)
    mode: u32,     // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    dst_w: u32,    // output width
    dst_h: u32,    // output height
    fill: u32,     // packed fill color (0xAABBGGRR)
    centering_x: u32,  // fixed-point centering (0.0-1.0) * 65536
    centering_y: u32,  // fixed-point centering (0.0-1.0) * 65536
    scale_x: u32,      // fixed-point scale factor * 65536
    scale_y: u32,      // fixed-point scale factor * 65536
}

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn bilinear_sample(sx: f32, sy: f32, src_w: u32, src_h: u32) -> u32 {
    let x = clamp(sx, 0.0, f32(src_w) - 1.0);
    let y = clamp(sy, 0.0, f32(src_h) - 1.0);

    let x0 = u32(floor(x));
    let y0 = u32(floor(y));
    let x1 = min(x0 + 1u, src_w - 1u);
    let y1 = min(y0 + 1u, src_h - 1u);

    let fx = x - f32(x0);
    let fy = y - f32(y0);

    let p00 = input[y0 * src_w + x0];
    let p10 = input[y0 * src_w + x1];
    let p01 = input[y1 * src_w + x0];
    let p11 = input[y1 * src_w + x1];

    // Bilinear interpolation per channel
    var result: u32 = 0u;
    for (var c = 0u; c < 4u; c++) {
        let shift = c * 8u;
        let v00 = f32((p00 >> shift) & 0xffu);
        let v10 = f32((p10 >> shift) & 0xffu);
        let v01 = f32((p01 >> shift) & 0xffu);
        let v11 = f32((p11 >> shift) & 0xffu);

        let top = mix(v00, v10, fx);
        let bot = mix(v01, v11, fx);
        let val = u32(clamp(mix(top, bot, fy), 0.0, 255.0));
        result |= val << shift;
    }
    return result;
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.dst_w || gid.y >= params.dst_h { return; }

    let cx = f32(params.centering_x) / 65536.0;
    let cy = f32(params.centering_y) / 65536.0;

    // Compute source coordinate for this output pixel
    let placed_w = f32(params.width) * f32(params.scale_x) / 65536.0;
    let placed_h = f32(params.height) * f32(params.scale_y) / 65536.0;

    let offset_x = (f32(params.dst_w) - placed_w) * cx;
    let offset_y = (f32(params.dst_h) - placed_h) * cy;

    let sx = (f32(gid.x) - offset_x) * 65536.0 / f32(params.scale_x);
    let sy = (f32(gid.y) - offset_y) * 65536.0 / f32(params.scale_y);

    let idx = gid.y * params.dst_w + gid.x;

    // Check if source coordinate is within source bounds
    if sx >= 0.0 && sx < f32(params.width) && sy >= 0.0 && sy < f32(params.height) {
        let pixel = bilinear_sample(sx, sy, params.width, params.height);
        let r = pixel & 0xffu;
        let g = (pixel >> 8u) & 0xffu;
        let b2 = (pixel >> 16u) & 0xffu;
        let a = (pixel >> 24u) & 0xffu;

        let out_r = r;
        let out_g = select(0u, g, mode_has_g(params.mode));
        let out_b = select(0u, b2, mode_has_b(params.mode));
        let out_a = select(255u, a, mode_has_a(params.mode));

        output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
    } else {
        // Fill border
        let f = params.fill;
        output[idx] = f;
    }
}
