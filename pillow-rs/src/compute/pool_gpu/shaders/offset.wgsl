// Offset: output[y][x] = input[clamp(y-dy,0,H-1)][clamp(x-dx,0,W-1)]
// Mode-aware: preserves alpha correctly per image mode.
// Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA
// Packed u32 RGBA: byte0=R, byte1=G, byte2=B, byte3=A

struct Params {
    width: u32,
    height: u32,
    mode: u32,    // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    dx: u32,
    dy: u32,
}

// ── Mode helpers ──

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let w = params.width;
    let h = params.height;
    // Source: read from (x-dx, y-dy) with wrapping. Keep the host's complete
    // signed i32 value in the uniform and reduce its magnitude only after
    // converting to unsigned. This handles both negative offsets and values
    // larger than 65535 without signed overflow or unsigned underflow.
    let dx_bits = params.dx;
    let dy_bits = params.dy;
    let dx_negative = bitcast<i32>(dx_bits) < 0i;
    let dy_negative = bitcast<i32>(dy_bits) < 0i;
    var dx_magnitude = dx_bits;
    var dy_magnitude = dy_bits;
    if dx_negative {
        dx_magnitude = 0u - dx_bits;
    }
    if dy_negative {
        dy_magnitude = 0u - dy_bits;
    }
    dx_magnitude = dx_magnitude % w;
    dy_magnitude = dy_magnitude % h;
    // ImageChops.offset reads `(x - offset) mod width`. A positive offset
    // therefore wraps toward the previous source pixel; a negative offset
    // wraps toward the next one. Keep each addition bounded by reducing the
    // magnitude before combining it with the invocation coordinate.
    var sx: u32;
    if gid.x >= dx_magnitude {
        sx = gid.x - dx_magnitude;
    } else {
        sx = w - (dx_magnitude - gid.x);
    }
    var sy: u32;
    if gid.y >= dy_magnitude {
        sy = gid.y - dy_magnitude;
    } else {
        sy = h - (dy_magnitude - gid.y);
    }
    if dx_negative {
        if gid.x >= w - dx_magnitude {
            sx = gid.x - (w - dx_magnitude);
        } else {
            sx = gid.x + dx_magnitude;
        }
    }
    if dy_negative {
        if gid.y >= h - dy_magnitude {
            sy = gid.y - (h - dy_magnitude);
        } else {
            sy = gid.y + dy_magnitude;
        }
    }
    let src_idx = sy * w + sx;
    let dst_idx = gid.y * w + gid.x;

    let pixel = input[src_idx];
    let r = pixel & 0xffu;
    let g = (pixel >> 8u) & 0xffu;
    let b = (pixel >> 16u) & 0xffu;
    let a = (pixel >> 24u) & 0xffu;

    let out_a = select(255u, a, mode_has_a(params.mode));
    output[dst_idx] = r | (g << 8u) | (b << 16u) | (out_a << 24u);
}
