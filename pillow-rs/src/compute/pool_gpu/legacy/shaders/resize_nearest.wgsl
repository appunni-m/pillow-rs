// Nearest-neighbor resize.
// Mode-aware: for L/LA (0/1) only copies R channel from source position.
// CPU reference (ops/pil_resize.rs:408-436): PIL pixel-centered coordinates.
//   source_center = (dx + 0.5) * src_w / dst_w
//   sx = floor(source_center), clamped to [0, src_w-1]
//
// NOTE: This shader dispatches with source-image workgroup counts.
//       The backend must handle output buffer sizing for resize ops.
// Pixel format: packed u32 RGBA (R | G<<8 | B<<16 | A<<24).

struct Params {
    width: u32,    // source width
    height: u32,   // source height
    mode: u32,     // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    dst_w: u32,    // output width
    dst_h: u32,    // output height
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
// Modes 4+ use all four packed bytes as data (CMYK, RGBX, I, and F), not an
// alpha channel.  Nearest resize is a raw sample relocation for those modes,
// so the fourth byte must be copied instead of being replaced with 255.
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u || m >= 4u; }

fn nearest_sample(dx: u32, dy: u32) -> u32 {
    let src_w = params.width;
    let src_h = params.height;
    let dst_w = params.dst_w;
    let dst_h = params.dst_h;

    // PIL: source_center = (dx + 0.5) * src_w / dst_w, no -0.5 correction
    let cx = (f32(dx) + 0.5) * f32(src_w) / f32(dst_w);
    let cy = (f32(dy) + 0.5) * f32(src_h) / f32(dst_h);

    // Nearest neighbor via floor
    let sx = u32(clamp(floor(cx), 0.0, f32(src_w - 1u)));
    let sy = u32(clamp(floor(cy), 0.0, f32(src_h - 1u)));

    let pixel = input[sy * src_w + sx];
    let src_r = pixel & 0xffu;
    let src_g = (pixel >> 8u) & 0xffu;
    let src_b = (pixel >> 16u) & 0xffu;
    let src_a = (pixel >> 24u) & 0xffu;

    // Mode-aware: for L/LA, only copy R; zero G/B, A=255 for non-alpha modes
    let out_r = src_r;
    let out_g = select(0u, src_g, mode_has_g(params.mode));
    let out_b = select(0u, src_b, mode_has_b(params.mode));
    let out_a = select(255u, src_a, mode_has_a(params.mode));

    return out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.dst_w || gid.y >= params.dst_h { return; }
    let idx = gid.y * params.dst_w + gid.x;
    output[idx] = nearest_sample(gid.x, gid.y);
}
