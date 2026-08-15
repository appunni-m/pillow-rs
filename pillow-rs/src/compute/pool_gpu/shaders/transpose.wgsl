// Transpose: coordinate remap for rotate/transpose operations
// Mode-aware: for L/LA (0/1) only copies R channel from transposed source.
// op_code: 0=FLIP_LEFT_RIGHT, 1=FLIP_TOP_BOTTOM, 2=ROTATE_90,
//          3=ROTATE_180, 4=ROTATE_270, 5=TRANSPOSE, 6=TRANSVERSE
// For ops 2,4,5,6 output dimensions are swapped (HxW) vs input (WxH).
// params.width/height are OUTPUT dimensions.
// Input width = select(width, height, swap), Input height = select(height, width, swap).

struct Params {
    width: u32,
    height: u32,
    mode: u32,     // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    op_code: u32,
    _pad2: u32,
    _pad3: u32,
    _pad4: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

fn get_src_coord(x: u32, y: u32, src_w: u32, src_h: u32, op: u32) -> vec2<u32> {
    var sx = x;
    var sy = y;
    switch op {
        case 0u: { sx = src_w - 1u - x; } // FLIP_LEFT_RIGHT
        case 1u: { sy = src_h - 1u - y; } // FLIP_TOP_BOTTOM
        // Pillow ROTATE_90 is counter-clockwise.
        case 2u: { sx = src_w - 1u - y; sy = x; } // ROTATE_90 (swap)
        case 3u: { sx = src_w - 1u - x; sy = src_h - 1u - y; } // ROTATE_180
        case 4u: { sx = y; sy = src_h - 1u - x; } // ROTATE_270 (swap)
        case 5u: { sx = y; sy = x; } // TRANSPOSE (swap)
        case 6u: { sx = src_w - 1u - y; sy = src_h - 1u - x; } // TRANSVERSE (swap)
        default: {}
    }
    return vec2<u32>(sx, sy);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let w = params.width;
    let h = params.height;
    let op = params.op_code;

    // Ops 2,4,5,6 swap input dimensions
    let swap = op == 2u || op == 4u || op == 5u || op == 6u;
    let in_w = select(w, h, swap);
    let in_h = select(h, w, swap);

    let src = get_src_coord(gid.x, gid.y, in_w, in_h, op);
    let src_idx = src.y * in_w + src.x;
    let dst_idx = gid.y * w + gid.x;

    let src_pixel = input[src_idx];
    let src_r = src_pixel & 0xffu;
    let src_g = (src_pixel >> 8u) & 0xffu;
    let src_b = (src_pixel >> 16u) & 0xffu;
    let src_a = (src_pixel >> 24u) & 0xffu;

    // Mode-aware: for L/LA, only copy R; zero G/B, A=255 for non-alpha modes
    let out_r = src_r;
    let out_g = select(0u, src_g, mode_has_g(params.mode));
    let out_b = select(0u, src_b, mode_has_b(params.mode));
    let out_a = select(255u, src_a, mode_has_a(params.mode));

    output[dst_idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}
