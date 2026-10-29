// Crop: extract a sub-region from the source image.
// Output dimensions = crop_w x crop_h (independent of source dimensions).
// Map: src_x = left + gid.x, src_y = top + gid.y
// Mode-aware: preserves pixel channels, sets alpha=255 for non-alpha modes.
// Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA
// Packed u32 RGBA: byte0=R, byte1=G, byte2=B, byte3=A

struct Params {
    width: u32,    // source width
    height: u32,   // source height
    mode: u32,     // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    left: u32,     // crop region left edge (source coordinate)
    top: u32,      // crop region top edge (source coordinate)
    crop_w: u32,   // output / crop width
    crop_h: u32,   // output / crop height
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u || m == 4u || m == 5u || m == 7u || m == 8u; }

fn crop_pixel(dx: u32, dy: u32) -> u32 {
    let src_w = params.width;
    let src_h = params.height;

    // Validate before addition so malformed uniforms cannot wrap a source
    // coordinate and turn an out-of-bounds crop into an unrelated in-range
    // storage read. Public Image::crop normalizes this box on the host; this
    // is the shader-side totality guard for direct/hostile pipeline inputs.
    if params.left > src_w || params.top > src_h {
        return 0u;
    }
    if dx >= src_w - params.left || dy >= src_h - params.top {
        return 0u;
    }

    // Map output position (dx, dy) to source position after the checked
    // extent tests above.
    let src_x = params.left + dx;
    let src_y = params.top + dy;

    let pixel = input[src_y * src_w + src_x];
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
    if gid.x >= params.crop_w || gid.y >= params.crop_h { return; }
    let idx = gid.y * params.crop_w + gid.x;
    output[idx] = crop_pixel(gid.x, gid.y);
}
