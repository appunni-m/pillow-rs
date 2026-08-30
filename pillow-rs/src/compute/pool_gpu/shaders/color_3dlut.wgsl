// Color3DLUT: Pillow's signed 12.4 table and 18.15 trilinear interpolation.
// The second storage binding contains one table value per u32. Values are
// sign-extended i16 samples in the low 16 bits; the host prepares these using
// the same float32/rounding rules as ImageFilter.Color3DLUT.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    size_x: u32,
    size_y: u32,
    size_z: u32,
    channels: u32,
    scale_x: u32,
    scale_y: u32,
    scale_z: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read> table: array<u32>;
@group(0) @binding(2) var<storage, read_write> output: array<u32>;
@group(0) @binding(3) var<uniform> params: Params;

fn signed_table(index: u32) -> i32 {
    let raw = table[index] & 0xffffu;
    return select(i32(raw), i32(raw) - 65536i, raw >= 32768u);
}

fn interpolate(a: i32, b: i32, shift: i32) -> i32 {
    let value = a * (32768i - shift) + b * shift;
    return value >> 15u;
}

fn table_at(x: u32, y: u32, z: u32, channel: u32) -> i32 {
    let index = (x + y * params.size_x + z * params.size_x * params.size_y)
        * params.channels + channel;
    return signed_table(index);
}

fn lut_channel(x: u32, y: u32, z: u32, sx: i32, sy: i32, sz: i32, channel: u32) -> u32 {
    let x1 = min(x + 1u, params.size_x - 1u);
    let y1 = min(y + 1u, params.size_y - 1u);
    let z1 = min(z + 1u, params.size_z - 1u);
    let ll = interpolate(table_at(x, y, z, channel), table_at(x1, y, z, channel), sx);
    let lr = interpolate(table_at(x, y1, z, channel), table_at(x1, y1, z, channel), sx);
    let l = interpolate(ll, lr, sy);
    let rl = interpolate(table_at(x, y, z1, channel), table_at(x1, y, z1, channel), sx);
    let rr = interpolate(table_at(x, y1, z1, channel), table_at(x1, y1, z1, channel), sx);
    let r = interpolate(rl, rr, sy);
    let value = interpolate(l, r, sz);
    return u32(clamp((value + 2i) >> 4u, 0i, 255i));
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }
    let idx = gid.y * params.width + gid.x;
    let pixel = input[idx];
    let r = pixel & 0xffu;
    let g = (pixel >> 8u) & 0xffu;
    let b = (pixel >> 16u) & 0xffu;
    let a = (pixel >> 24u) & 0xffu;
    let ix = r * params.scale_x;
    let iy = g * params.scale_y;
    let iz = b * params.scale_z;
    let sx = i32((ix & 0x3ffffu) >> 3u);
    let sy = i32((iy & 0x3ffffu) >> 3u);
    let sz = i32((iz & 0x3ffffu) >> 3u);
    let bx = min(ix >> 18u, params.size_x - 1u);
    let by = min(iy >> 18u, params.size_y - 1u);
    let bz = min(iz >> 18u, params.size_z - 1u);
    let out_r = lut_channel(bx, by, bz, sx, sy, sz, 0u);
    let out_g = lut_channel(bx, by, bz, sx, sy, sz, 1u);
    let out_b = lut_channel(bx, by, bz, sx, sy, sz, 2u);
    if params.channels == 4u {
        output[idx] = out_r | (out_g << 8u) | (out_b << 16u)
            | (lut_channel(bx, by, bz, sx, sy, sz, 3u) << 24u);
    } else {
        let out_a = select(255u, a, params.mode == 3u || params.mode == 4u);
        output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
    }
}
