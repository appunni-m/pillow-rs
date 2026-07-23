// Paste: composite source image into destination at (paste_x, paste_y)
// with optional mask blending.
// Per-pixel dispatch (16x16 workgroups) over the destination image.
//
// CPU reference: paste source image into destination at (x,y)
// with optional mask. For each destination pixel in paste rect:
//   if mask provided: out = (src * mask + dst * (255-mask)) / 255
//   else: direct copy of src over dst (same as mask=255)
//
// 5-binding layout (buf_img3): input_dst, input_src, input_mask, output, params.
//   input_dst, input_src, input_mask are storage read.
//   output is storage read_write.
//
// Mode-aware: only paste active channels.
// Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    src_w: u32,
    src_h: u32,
    paste_x: i32,
    paste_y: i32,
    has_mask: u32,
    mask_alpha: u32,
}

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

@group(0) @binding(0) var<storage, read> input_dst: array<u32>;
@group(0) @binding(1) var<storage, read> input_src: array<u32>;
@group(0) @binding(2) var<storage, read> input_mask: array<u32>;
@group(0) @binding(3) var<storage, read_write> output: array<u32>;
@group(0) @binding(4) var<uniform> params: Params;

fn blend_pixel(src: u32, dst: u32, mask: u32, mode: u32) -> u32 {
    let sr = src & 0xffu;
    let sg = (src >> 8u) & 0xffu;
    let sb = (src >> 16u) & 0xffu;
    let sa = (src >> 24u) & 0xffu;

    let dr = dst & 0xffu;
    let dg = (dst >> 8u) & 0xffu;
    let db = (dst >> 16u) & 0xffu;
    let da = (dst >> 24u) & 0xffu;

    // Pillow's DIV255 macro rounds this weighted sum to nearest.
    let out_r = (sr * mask + dr * (255u - mask) + 127u) / 255u;
    let out_g = select(dg, (sg * mask + dg * (255u - mask) + 127u) / 255u, mode_has_g(mode));
    let out_b = select(db, (sb * mask + db * (255u - mask) + 127u) / 255u, mode_has_b(mode));
    let out_a = select(da, (sa * mask + da * (255u - mask) + 127u) / 255u, mode_has_a(mode));

    return out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let dx = i32(gid.x);
    let dy = i32(gid.y);
    let dst_idx = gid.y * params.width + gid.x;
    let dst_pixel = input_dst[dst_idx];

    // Compute corresponding source pixel coordinate
    let sx = dx - params.paste_x;
    let sy = dy - params.paste_y;

    // Check if this destination pixel overlaps the source rectangle
    if sx >= 0 && sy >= 0 && u32(sx) < params.src_w && u32(sy) < params.src_h {
        let src_idx = u32(sy) * params.src_w + u32(sx);
        let src_pixel = input_src[src_idx];

        var mask_val: u32 = 255u;
        if params.has_mask == 1u {
            let mask_pixel = input_mask[src_idx];
            mask_val = select(
                mask_pixel & 0xffu,
                (mask_pixel >> 24u) & 0xffu,
                params.mask_alpha == 1u,
            );
        }

        output[dst_idx] = blend_pixel(src_pixel, dst_pixel, mask_val, params.mode);
    } else {
        // Outside paste region: pass destination through unchanged
        output[dst_idx] = dst_pixel;
    }
}
