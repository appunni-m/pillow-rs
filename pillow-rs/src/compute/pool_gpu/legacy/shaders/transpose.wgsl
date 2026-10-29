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
    mode: u32,     // 0=L, 1=LA, 2=RGB, 3=RGBA, 6=RGBX
    _pad: u32,
    op_code: u32,
    _pad2: u32,
    _pad3: u32,
    _pad4: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

// Swapped-axis methods stage a 16x16 tile with a padded row stride before
// writing output rows. The padding avoids a power-of-two stride when lanes
// read the tile's columns; storage contains complete packed pixel words.
var<workgroup> tile: array<u32, 272>;

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u || m == 4u || m == 5u || m == 6u || m == 7u || m == 8u; }

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
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(workgroup_id) group: vec3<u32>,
) {
    let w = params.width;
    let h = params.height;
    let op = params.op_code;

    // Ops 2,4,5,6 swap input dimensions
    let swap = op == 2u || op == 4u || op == 5u || op == 6u;
    let in_w = select(w, h, swap);
    let in_h = select(h, w, swap);

    var src_pixel = 0u;
    if swap {
        // Exchanging local output axes makes adjacent X lanes read adjacent
        // (possibly reversed) source pixels instead of separate source rows.
        let load_coord = group.xy * 16u + lid.yx;
        var loaded = 0u;
        if load_coord.x < w && load_coord.y < h {
            let src = get_src_coord(load_coord.x, load_coord.y, in_w, in_h, op);
            loaded = input[src.y * in_w + src.x];
        }
        tile[lid.y * 17u + lid.x] = loaded;
        // Every invocation, including the inactive edge lanes, must reach
        // this barrier. A valid output reads the slot whose loader checked
        // that exact output coordinate, so it never consumes edge padding.
        workgroupBarrier();
        if gid.x >= w || gid.y >= h { return; }
        src_pixel = tile[lid.x * 17u + lid.y];
    } else {
        // Flips and Rotate180 already read contiguous source rows and do
        // not benefit from workgroup staging or synchronization.
        if gid.x >= w || gid.y >= h { return; }
        let src = get_src_coord(gid.x, gid.y, in_w, in_h, op);
        src_pixel = input[src.y * in_w + src.x];
    }

    let dst_idx = gid.y * w + gid.x;
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
