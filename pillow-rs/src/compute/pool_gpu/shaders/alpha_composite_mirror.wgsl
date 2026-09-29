// Exact full-frame RGBA AlphaComposite followed by horizontal Mirror.
//
// The intermediate composite image is never written. Each destination
// invocation reads the original destination and source at the mirrored
// coordinate, applies Pillow's alpha-composite integer math, and writes one
// unique output pixel. The binding layout is
// [destination(read), source(read), output(rw), params].

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    src_w: u32,
    src_h: u32,
}

@group(0) @binding(0) var<storage, read> destination: array<u32>;
@group(0) @binding(1) var<storage, read> source: array<u32>;
@group(0) @binding(2) var<storage, read_write> output: array<u32>;
@group(0) @binding(3) var<uniform> params: Params;

fn alpha_composite_pixel(src_pixel: u32, dst_pixel: u32) -> u32 {
    let sr = src_pixel & 0xffu;
    let sg = (src_pixel >> 8u) & 0xffu;
    let sb = (src_pixel >> 16u) & 0xffu;
    let sa = (src_pixel >> 24u) & 0xffu;

    let dr = dst_pixel & 0xffu;
    let dg = (dst_pixel >> 8u) & 0xffu;
    let db = (dst_pixel >> 16u) & 0xffu;
    let da = (dst_pixel >> 24u) & 0xffu;

    // Match Pillow's libImaging/AlphaComposite.c 7-bit fixed-point path.
    // A transparent source copies all destination bytes, including hidden RGB.
    if sa == 0u {
        return dst_pixel;
    }
    let inv_sa = 255u - sa;
    let blend = da * inv_sa;
    let outa255 = sa * 255u + blend;
    let coef1 = sa * 255u * 255u * (1u << 7u) / outa255;
    let coef2 = (255u << 7u) - coef1;
    let round_bias = 0x80u << 7u;

    let r_tmp = sr * coef1 + dr * coef2 + round_bias;
    let g_tmp = sg * coef1 + dg * coef2 + round_bias;
    let b_tmp = sb * coef1 + db * coef2 + round_bias;
    let out_r = (((r_tmp >> 8u) + r_tmp) >> 8u) >> 7u;
    let out_g = (((g_tmp >> 8u) + g_tmp) >> 8u) >> 7u;
    let out_b = (((b_tmp >> 8u) + b_tmp) >> 8u) >> 7u;
    let out_a = (((outa255 + 0x80u) >> 8u) + (outa255 + 0x80u)) >> 8u;
    return out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }
    if params.src_w != params.width || params.src_h != params.height { return; }

    let source_x = params.width - 1u - gid.x;
    let source_index = gid.y * params.width + source_x;
    let output_index = gid.y * params.width + gid.x;
    output[output_index] = alpha_composite_pixel(
        source[source_index],
        destination[source_index],
    );
}
