// Reduce: downsample by integer factor using box averaging.
// Output dimensions = w/factor x h/factor.
// Each output pixel averages a factor x factor block from the source.
// Accumulate in u32, divide with rounding: (sum + count/2) / count.
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
            sum_r = sum_r + (pixel & 0xffu);
            sum_g = sum_g + ((pixel >> 8u) & 0xffu);
            sum_b = sum_b + ((pixel >> 16u) & 0xffu);
            sum_a = sum_a + ((pixel >> 24u) & 0xffu);
            count = count + 1u;
            sx = sx + 1u;
        }
        sy = sy + 1u;
    }

    // Host-side dimension checks make an empty block unreachable for valid
    // public inputs. Keep the shader total as well: malformed factors must
    // not turn the final channel divisions into a zero-denominator operation.
    if count == 0u { return 0u; }

    // Divide with rounding: (sum + count/2) / count
    let half = count / 2u;
    let out_r = (sum_r + half) / count;
    let out_g = select(0u, (sum_g + half) / count, mode_has_g(params.mode));
    let out_b = select(0u, (sum_b + half) / count, mode_has_b(params.mode));
    let out_a = select(255u, (sum_a + half) / count, mode_has_a(params.mode));

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
