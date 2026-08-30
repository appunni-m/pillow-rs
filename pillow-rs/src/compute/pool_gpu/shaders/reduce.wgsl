// Reduce: downsample by integer factor using Pillow's box averaging contract.
// Output dimensions = ceil(w/factor) x ceil(h/factor).
// Pillow uses a truncated fixed-point reciprocal for division and averages
// color channels premultiplied by alpha for LA/RGBA before unpremultiplying.
// Mode-aware: only averages channels present in the image mode.
// Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA
// Packed u32 RGBA: byte0=R, byte1=G, byte2=B, byte3=A

struct Params {
    width: u32,    // source width
    height: u32,   // source height
    mode: u32,     // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    x_factor: u32, // horizontal downsample factor (0 is treated as 1)
    y_factor: u32, // vertical downsample factor (0 is treated as 1)
    dst_w: u32,
    dst_h: u32,
}

// Host validation keeps ordinary requests within this bound. The shader
// clamps as a second line of defense so a malformed uniform cannot create an
// unbounded loop on a native queue.
const MAX_FACTOR: u32 = 64u;

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }
fn mode_has_fourth(m: u32) -> bool { return m == 1u || m == 3u || m == 4u; }

// This matches Reduce.c's division_UINT32(divider, 8) path:
// multiplier = floor(2^32 / (256 * divider)) = floor(2^24 / divider),
// result = ((sum + divider/2) * multiplier) >> 24.
// The host bounds factors at 64, so the product remains within u32 for
// byte-valued samples and every valid block size.
fn pil_average(sum: u32, count: u32) -> u32 {
    let multiplier = 16777216u / count;
    return ((sum + count / 2u) * multiplier) >> 24u;
}

fn unpremultiply(value: u32, alpha: u32) -> u32 {
    if alpha == 0u { return 0u; }
    return (value * 255u) / alpha;
}

fn reduce_pixel(dx: u32, dy: u32) -> u32 {
    let src_w = params.width;
    let src_h = params.height;
    let x_factor = min(max(params.x_factor, 1u), MAX_FACTOR);
    let y_factor = min(max(params.y_factor, 1u), MAX_FACTOR);

    // Source block top-left corner
    let sx0 = dx * x_factor;
    let sy0 = dy * y_factor;

    // Clamp block extent to source bounds (for edge tiles)
    let sx_end = min(sx0 + x_factor, src_w);
    let sy_end = min(sy0 + y_factor, src_h);

    var sum_r: u32 = 0u;
    var sum_g: u32 = 0u;
    var sum_b: u32 = 0u;
    var sum_a: u32 = 0u;
    var count: u32 = 0u;

    // Accumulate factor x factor block
    var sy = sy0;
    loop {
        if sy >= sy_end { break; }
        var sx = sx0;
        loop {
            if sx >= sx_end { break; }
            let pixel = input[sy * src_w + sx];
            let alpha = (pixel >> 24u) & 0xffu;
            let red = pixel & 0xffu;
            let green = (pixel >> 8u) & 0xffu;
            let blue = (pixel >> 16u) & 0xffu;
            // The packed transport uses opaque alpha for L/RGB, so this
            // branch is harmless there and preserves the native LA/RGBA
            // premultiplied-alpha semantics.
            if mode_has_a(params.mode) {
                sum_r = sum_r + (red * alpha + 127u) / 255u;
                sum_g = sum_g + (green * alpha + 127u) / 255u;
                sum_b = sum_b + (blue * alpha + 127u) / 255u;
            } else {
                sum_r = sum_r + red;
                sum_g = sum_g + green;
                sum_b = sum_b + blue;
            }
            sum_a = sum_a + alpha;
            count = count + 1u;
            sx = sx + 1u;
        }
        sy = sy + 1u;
    }

    // Host-side dimension checks make an empty block unreachable for valid
    // public inputs. Keep the shader total as well: malformed factors must
    // not turn the final channel divisions into a zero-denominator operation.
    if count == 0u { return 0u; }

    let average_r = pil_average(sum_r, count);
    let average_g = pil_average(sum_g, count);
    let average_b = pil_average(sum_b, count);
    let average_a = pil_average(sum_a, count);
    let out_r = select(
        average_r,
        unpremultiply(average_r, average_a),
        mode_has_a(params.mode),
    );
    let out_g = select(
        0u,
        unpremultiply(average_g, average_a),
        mode_has_g(params.mode),
    );
    let out_b = select(
        0u,
        unpremultiply(average_b, average_a),
        mode_has_b(params.mode),
    );
    let out_a = select(255u, average_a, mode_has_fourth(params.mode));

    return out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    // The host computes ceil(source / factor), matching Pillow's partial edge
    // blocks. The zero-factor normalization keeps this shader total even if a
    // malformed PipelineOp reaches an explicitly selected GPU backend.
    let dst_w = params.dst_w;
    let dst_h = params.dst_h;
    if gid.x >= dst_w || gid.y >= dst_h { return; }
    let idx = gid.y * dst_w + gid.x;
    output[idx] = reduce_pixel(gid.x, gid.y);
}
