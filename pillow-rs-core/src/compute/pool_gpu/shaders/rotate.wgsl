// Rotate: arbitrary-angle image rotation with bilinear interpolation.
// Inverse mapping: for each output pixel, compute source coordinate via rotation matrix.
// Rotation about image center: src_x = cos*(dx-cx) + sin*(dy-cy) + scx
//                            src_y = -sin*(dx-cx) + cos*(dy-cy) + scy
// Out-of-bounds pixels filled with fill color. Expand mode changes output size.
// Mode-aware. Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA

struct Params {
    width: u32,        // output width
    height: u32,       // output height
    mode: u32,         // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    src_w: u32,        // source width
    src_h: u32,        // source height
    cos_theta: u32,    // f32 bits of cos(angle)
    sin_theta: u32,    // f32 bits of sin(angle)
    fill: u32,         // packed fill color for out-of-bounds pixels
    expand: u32,       // 1 = expand output to fit rotated image
}

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn bilinear_sample(sx: f32, sy: f32, w: u32, h: u32) -> u32 {
    let x = clamp(sx, -8192.0, 8192.0);
    let y = clamp(sy, -8192.0, 8192.0);

    let x0 = u32(max(0.0, floor(x)));
    let y0 = u32(max(0.0, floor(y)));
    let x1 = min(x0 + 1u, w - 1u);
    let y1 = min(y0 + 1u, h - 1u);

    let fx = clamp(x - f32(x0), 0.0, 1.0);
    let fy = clamp(y - f32(y0), 0.0, 1.0);

    let p00 = f32(input[y0 * w + x0]);
    let p10 = f32(input[y0 * w + x1]);
    let p01 = f32(input[y1 * w + x0]);
    let p11 = f32(input[y1 * w + x1]);

    // Single-channel bilinear for L-mode (pixel value already packed)
    // We interpolate the full u32 as f32 (approximate, fine for display)
    let top = p00 + (p10 - p00) * fx;
    let bot = p01 + (p11 - p01) * fx;
    return u32(top + (bot - top) * fy);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let cos_t = bitcast<f32>(params.cos_theta);
    let sin_t = bitcast<f32>(params.sin_theta);

    // Output center
    let ocx = f32(params.width) * 0.5;
    let ocy = f32(params.height) * 0.5;

    // Source center
    let scx = f32(params.src_w) * 0.5;
    let scy = f32(params.src_h) * 0.5;

    // Inverse rotation: map output → source
    let dx = f32(gid.x) - ocx;
    let dy = f32(gid.y) - ocy;

    let sx = cos_t * dx + sin_t * dy + scx;
    let sy = -sin_t * dx + cos_t * dy + scy;

    let idx = gid.y * params.width + gid.x;

    // Check if source coordinate is within source bounds
    if sx >= 0.0 && sx < f32(params.src_w) && sy >= 0.0 && sy < f32(params.src_h) {
        // Bilinear sample from source
        let x0 = u32(floor(sx));
        let y0 = u32(floor(sy));
        let x1 = min(x0 + 1u, params.src_w - 1u);
        let y1 = min(y0 + 1u, params.src_h - 1u);

        let fx = sx - f32(x0);
        let fy = sy - f32(y0);

        // Sample 4 neighbors and interpolate per channel
        let p00 = input[y0 * params.src_w + x0];
        let p10 = input[y0 * params.src_w + x1];
        let p01 = input[y1 * params.src_w + x0];
        let p11 = input[y1 * params.src_w + x1];

        var result: u32 = 0u;
        for (var c = 0u; c < 4u; c++) {
            let shift = c * 8u;
            let v00 = f32((p00 >> shift) & 0xffu);
            let v10 = f32((p10 >> shift) & 0xffu);
            let v01 = f32((p01 >> shift) & 0xffu);
            let v11 = f32((p11 >> shift) & 0xffu);

            let top = v00 + (v10 - v00) * fx;
            let bot = v01 + (v11 - v01) * fx;
            let val = u32(clamp(top + (bot - top) * fy, 0.0, 255.0));
            result |= val << shift;
        }

        let r = result & 0xffu;
        let g = (result >> 8u) & 0xffu;
        let b2 = (result >> 16u) & 0xffu;
        let a = (result >> 24u) & 0xffu;

        let out_r = r;
        let out_g = select(0u, g, mode_has_g(params.mode));
        let out_b = select(0u, b2, mode_has_b(params.mode));
        let out_a = select(255u, a, mode_has_a(params.mode));

        output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
    } else {
        // Out of bounds: fill color
        let f = params.fill;
        output[idx] = f;
    }
}
