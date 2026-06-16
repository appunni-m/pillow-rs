// Rotate: arbitrary-angle image rotation with bilinear interpolation.
// Inverse mapping (center-centered rotation):
//   For each output pixel, compute source coordinate via rotation matrix.
//   Rotation center = source center in source space, output center in output space.
//   When expand=false: output center = source center (same dimensions).
//   When expand=true: output dimensions are the rotated bounding box.
//
// Out-of-bounds pixels filled with fill color.
// Mode-aware. Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA
//
// NOTE: For expand=true, the CPU rotates around the top-left corner while
// the GPU rotates around the center. Results may differ by <1 pixel near edges.
//
// Params layout after executor appends:
//   [header:src_w,src_h,mode,0 | cos_t,sin_t,fill,expand | dst_w,dst_h]

struct Params {
    width: u32,        // source width (from header = cur_w)
    height: u32,       // source height (from header = cur_h)
    mode: u32,         // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    cos_theta: u32,    // f32 bits of cos(angle)
    sin_theta: u32,    // f32 bits of sin(angle)
    fill: u32,         // packed fill color for out-of-bounds pixels
    expand: u32,       // 1 = expand output to fit rotated image
    dst_w: u32,        // output width (appended by executor)
    dst_h: u32,        // output height (appended by executor)
}

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn bilinear_sample_channel(sx: f32, sy: f32, src_w: u32, src_h: u32, shift: u32) -> f32 {
    let x = clamp(sx, 0.0, f32(src_w) - 1.0);
    let y = clamp(sy, 0.0, f32(src_h) - 1.0);

    let x0 = u32(floor(x));
    let y0 = u32(floor(y));
    let x1 = min(x0 + 1u, src_w - 1u);
    let y1 = min(y0 + 1u, src_h - 1u);

    let fx = x - f32(x0);
    let fy = y - f32(y0);

    let p00 = f32((input[y0 * src_w + x0] >> shift) & 0xffu);
    let p10 = f32((input[y0 * src_w + x1] >> shift) & 0xffu);
    let p01 = f32((input[y1 * src_w + x0] >> shift) & 0xffu);
    let p11 = f32((input[y1 * src_w + x1] >> shift) & 0xffu);

    let top = p00 + (p10 - p00) * fx;
    let bot = p01 + (p11 - p01) * fx;
    return top + (bot - top) * fy;
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.dst_w || gid.y >= params.dst_h { return; }

    let cos_t = bitcast<f32>(params.cos_theta);
    let sin_t = bitcast<f32>(params.sin_theta);

    // Source center (in source image coordinates)
    let scx = f32(params.width) * 0.5;
    let scy = f32(params.height) * 0.5;

    // Output center (in output image coordinates)
    let ocx = f32(params.dst_w) * 0.5;
    let ocy = f32(params.dst_h) * 0.5;

    // Inverse rotation: map output pixel → source coordinate
    let dx = f32(gid.x) - ocx;
    let dy = f32(gid.y) - ocy;

    let sx = cos_t * dx + sin_t * dy + scx;
    let sy = -sin_t * dx + cos_t * dy + scy;

    let idx = gid.y * params.dst_w + gid.x;

    // Check if source coordinate is within source bounds
    if sx >= 0.0 && sx < f32(params.width) && sy >= 0.0 && sy < f32(params.height) {
        // Per-channel bilinear interpolation
        let out_r = u32(clamp(bilinear_sample_channel(sx, sy, params.width, params.height, 0u), 0.0, 255.0));
        let g = u32(clamp(bilinear_sample_channel(sx, sy, params.width, params.height, 8u), 0.0, 255.0));
        let b2 = u32(clamp(bilinear_sample_channel(sx, sy, params.width, params.height, 16u), 0.0, 255.0));
        let a_val = u32(clamp(bilinear_sample_channel(sx, sy, params.width, params.height, 24u), 0.0, 255.0));

        let out_g = select(0u, g, mode_has_g(params.mode));
        let out_b = select(0u, b2, mode_has_b(params.mode));
        let out_a = select(255u, a_val, mode_has_a(params.mode));

        output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
    } else {
        // Out of bounds: fill color
        output[idx] = params.fill;
    }
}
