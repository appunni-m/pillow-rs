// CompositeModule: Image.composite(image1, image2, mask)
// 3-input blend with mask.
//   input_a = image1 (img)
//   input_b = image2 (other)
//   mask    = blend factor
// Formula: out = a * mask + b * (255 - mask)
// The output canvas and palette belong to input_b. input_a is blended into its
// top-left overlap, exactly like image2.copy().paste(image1, mask).
//
// 5-binding layout (buf_img3): input_a, input_b, mask, output, params.
//   Same layout as paste.wgsl (dst=a, src=b, mask).
//
// Mode-aware: blend every active channel.
// L: blend R; LA: blend R,A; RGB: blend R,G,B; RGBA: blend R,G,B,A.
// Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA

struct Params {
    width: u32,       // input_a width
    height: u32,      // input_a height
    mode: u32,
    _pad: u32,
    mask_alpha: u32,  // LA/RGBA/RGBa masks use alpha rather than luma
    dst_width: u32,   // input_b/output width
    dst_height: u32,  // input_b/output height
}

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
// CMYK uses the same four-byte transport as RGBA, but byte three is K rather
// than alpha. It is still an active channel for Image.composite and must be
// blended instead of being replaced with an opaque padding value.
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u || m == 4u; }

@group(0) @binding(0) var<storage, read> input_a: array<u32>;
@group(0) @binding(1) var<storage, read> input_b: array<u32>;
@group(0) @binding(2) var<storage, read> input_mask: array<u32>;
@group(0) @binding(3) var<storage, read_write> output: array<u32>;
@group(0) @binding(4) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.dst_width || gid.y >= params.dst_height { return; }

    let dst_idx = gid.y * params.dst_width + gid.x;
    let pb = input_b[dst_idx];
    if gid.x >= params.width || gid.y >= params.height {
        output[dst_idx] = pb;
        return;
    }

    let src_idx = gid.y * params.width + gid.x;
    let pa = input_a[src_idx];
    let pm = input_mask[src_idx];

    let ar = pa & 0xffu;
    let ag = (pa >> 8u) & 0xffu;
    let ab = (pa >> 16u) & 0xffu;
    let aa = (pa >> 24u) & 0xffu;

    let br = pb & 0xffu;
    let bg = (pb >> 8u) & 0xffu;
    let bb = (pb >> 16u) & 0xffu;

    let mr = select(
        pm & 0xffu,
        (pm >> 24u) & 0xffu,
        params.mask_alpha == 1u,
    );
    let inv_mask = 255u - mr;

    // Pillow's BLEND macro rounds the division by adding 127.
    let mode = params.mode;
    let out_r = (ar * mr + br * inv_mask + 127u) / 255u;
    let out_g_raw = (ag * mr + bg * inv_mask + 127u) / 255u;
    let out_b_raw = (ab * mr + bb * inv_mask + 127u) / 255u;

    let out_g = select(out_r, out_g_raw, mode_has_g(mode));
    let out_b = select(out_r, out_b_raw, mode_has_b(mode));
    let ba = (pb >> 24u) & 0xffu;
    let out_a_raw = (aa * mr + ba * inv_mask + 127u) / 255u;
    let out_a = select(255u, out_a_raw, mode_has_a(mode));

    output[dst_idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}
