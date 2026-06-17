// Alpha composite: Porter-Duff Over compositing.
// Blend source image into destination with full alpha compositing.
// Per-pixel dispatch (16x16 workgroups).
//
// CPU reference: Porter-Duff Over operator.
//   out_A = sa + da*(255-sa)/255
//   out_RGB = (sr*sa + dr*da*(255-sa)/255) / out_A
// Integer math with rounding.
//
// 3-binding layout: input_src, output_dst, params.
//
// Mode-aware: only composite active channels.
// Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    src_w: u32,
    src_h: u32,
}

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

@group(0) @binding(0) var<storage, read> input_src: array<u32>;
@group(0) @binding(1) var<storage, read_write> output_dst: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn alpha_composite_pixel(src_pixel: u32, dst_pixel: u32, mode: u32) -> u32 {
    let sr = src_pixel & 0xffu;
    let sg = (src_pixel >> 8u) & 0xffu;
    let sb = (src_pixel >> 16u) & 0xffu;
    let sa = (src_pixel >> 24u) & 0xffu;

    let dr = dst_pixel & 0xffu;
    let dg = (dst_pixel >> 8u) & 0xffu;
    let db = (dst_pixel >> 16u) & 0xffu;
    let da = (dst_pixel >> 24u) & 0xffu;

    // Porter-Duff Over with integer rounding
    // out_A = sa + da*(255-sa)/255  (rounded)
    let inv_sa = 255u - sa;
    let dst_contrib = (da * inv_sa + 127u) / 255u;
    let out_a_val = sa + dst_contrib;

    var out_r: u32 = 0u;
    var out_g: u32 = 0u;
    var out_b: u32 = 0u;

    if out_a_val > 0u {
        // out_RGB = (sr*sa + dr*da*(255-sa)/255) / out_A  (rounded inner and outer)
        let r_num = sr * sa + (dr * da * inv_sa + 127u) / 255u;
        let g_num = sg * sa + (dg * da * inv_sa + 127u) / 255u;
        let b_num = sb * sa + (db * da * inv_sa + 127u) / 255u;
        out_r = (r_num + out_a_val / 2u) / out_a_val;
        out_g = (g_num + out_a_val / 2u) / out_a_val;
        out_b = (b_num + out_a_val / 2u) / out_a_val;
    }

    // Mode-aware channel selection
    let final_g = select(dg, out_g, mode_has_g(mode));
    let final_b = select(db, out_b, mode_has_b(mode));
    let final_a = select(da, out_a_val, mode_has_a(mode));

    return out_r | (final_g << 8u) | (final_b << 16u) | (final_a << 24u);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let idx = gid.y * params.width + gid.x;
    let src_pixel = input_src[idx];
    let dst_pixel = output_dst[idx];

    output_dst[idx] = alpha_composite_pixel(src_pixel, dst_pixel, params.mode);
}
