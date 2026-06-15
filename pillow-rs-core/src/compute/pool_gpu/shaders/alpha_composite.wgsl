// Alpha composite: blend source image into destination with alpha compositing
// over specified rectangular regions.
// Per-pixel dispatch (16x16 workgroups) over the destination region.
//
// CPU reference (image.rs:2837): Blend source into destination with source/dest rectangles.
//   Read source pixel at (src_x + dx, src_y + dy)
//   Read dest pixel at (dest_x + dx, dest_y + dy)
//   src_alpha = src_pixel >> 24
//   out = (src * src_alpha + dst * (255 - src_alpha)) / 255
//
// Binding 0: input_src (storage, read)
// Binding 1: output_dst (storage, read_write) — read destination, write composited result
// Binding 2: params (uniform)
//
// Mode-aware: only composite active channels.
// L/LA: composite R (+A for LA); RGB: composite R,G,B; RGBA: composite R,G,B,A.
//
// Params: width=dst_w, height=dst_h, mode, _pad, src_x, src_y, src_w, src_h,
//         dest_x, dest_y

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    src_x: u32,
    src_y: u32,
    src_w: u32,
    src_h: u32,
    dest_x: u32,
    dest_y: u32,
}

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

@group(0) @binding(0) var<storage, read> input_src: array<u32>;
@group(0) @binding(1) var<storage, read_write> output_dst: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn alpha_blend(src_pixel: u32, dst_pixel: u32, mode: u32) -> u32 {
    let sr = src_pixel & 0xffu;
    let sg = (src_pixel >> 8u) & 0xffu;
    let sb = (src_pixel >> 16u) & 0xffu;
    let sa = (src_pixel >> 24u) & 0xffu;

    let dr = dst_pixel & 0xffu;
    let dg = (dst_pixel >> 8u) & 0xffu;
    let db = (dst_pixel >> 16u) & 0xffu;
    let da = (dst_pixel >> 24u) & 0xffu;

    // Standard alpha blending: out = src * src_alpha + dst * (1 - src_alpha)
    let out_r = (sr * sa + dr * (255u - sa)) / 255u;
    let out_g = select(dg, (sg * sa + dg * (255u - sa)) / 255u, mode_has_g(mode));
    let out_b = select(db, (sb * sa + db * (255u - sa)) / 255u, mode_has_b(mode));
    let out_a = select(da, (sa * sa + da * (255u - sa)) / 255u, mode_has_a(mode));

    return out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let dx = gid.x;
    let dy = gid.y;

    // Compute source buffer index: (src_y + dy) * src_stride + (src_x + dx)
    // src_w and dst_w are used as buffer strides (full image widths).
    // Clamp source coordinate to source buffer bounds.
    let src_buf_x = min(params.src_x + dx, params.src_w - 1u);
    let src_buf_y = min(params.src_y + dy, params.src_h - 1u);
    let src_idx = src_buf_y * params.src_w + src_buf_x;
    let src_pixel = input_src[src_idx];

    // Compute destination buffer index
    let dst_buf_x = params.dest_x + dx;
    let dst_buf_y = params.dest_y + dy;
    let dst_idx = dst_buf_y * params.width + dst_buf_x;
    let dst_pixel = output_dst[dst_idx];

    // Blend source over destination
    output_dst[dst_idx] = alpha_blend(src_pixel, dst_pixel, params.mode);
}
