// Native-LA 5x5 convolution. Each u32 stores two adjacent [L, A] pixels;
// each invocation computes both channels independently and writes one word.
// The coefficient traversal and FMA order match filter_5x5_luma_packed.wgsl.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    k00: u32, k01: u32, k02: u32, k03: u32, k04: u32,
    k10: u32, k11: u32, k12: u32, k13: u32, k14: u32,
    k20: u32, k21: u32, k22: u32, k23: u32, k24: u32,
    k30: u32, k31: u32, k32: u32, k33: u32, k34: u32,
    k40: u32, k41: u32, k42: u32, k43: u32, k44: u32,
    offset_val: i32,
    rational_denominator: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn sample_la(pixel_index: u32, channel: u32) -> f32 {
    let packed_pixel = (input[pixel_index >> 1u] >> ((pixel_index & 1u) * 16u)) & 0xffffu;
    return f32((packed_pixel >> (channel * 8u)) & 0xffu);
}

fn row_5(
    p0: f32, p1: f32, p2: f32, p3: f32, p4: f32,
    k0: f32, k1: f32, k2: f32, k3: f32, k4: f32,
) -> f32 {
    var sum = p1 * k1;
    sum = fma(p0, k0, sum);
    sum = fma(p2, k2, sum);
    sum = fma(p3, k3, sum);
    sum = fma(p4, k4, sum);
    return sum;
}

fn clip8(value: f32) -> u32 {
    if value <= 0.0 { return 0u; }
    if value >= 255.0 { return 255u; }
    return u32(value);
}

fn filter_pixel_channel(x: u32, y: u32, channel: u32) -> u32 {
    let width = params.width;
    let height = params.height;
    let index = y * width + x;
    if width < 5u || height < 5u
        || x < 2u || x >= width - 2u
        || y < 2u || y >= height - 2u {
        return u32(sample_la(index, channel));
    }

    let k00 = bitcast<f32>(params.k00); let k01 = bitcast<f32>(params.k01);
    let k02 = bitcast<f32>(params.k02); let k03 = bitcast<f32>(params.k03);
    let k04 = bitcast<f32>(params.k04);
    let k10 = bitcast<f32>(params.k10); let k11 = bitcast<f32>(params.k11);
    let k12 = bitcast<f32>(params.k12); let k13 = bitcast<f32>(params.k13);
    let k14 = bitcast<f32>(params.k14);
    let k20 = bitcast<f32>(params.k20); let k21 = bitcast<f32>(params.k21);
    let k22 = bitcast<f32>(params.k22); let k23 = bitcast<f32>(params.k23);
    let k24 = bitcast<f32>(params.k24);
    let k30 = bitcast<f32>(params.k30); let k31 = bitcast<f32>(params.k31);
    let k32 = bitcast<f32>(params.k32); let k33 = bitcast<f32>(params.k33);
    let k34 = bitcast<f32>(params.k34);
    let k40 = bitcast<f32>(params.k40); let k41 = bitcast<f32>(params.k41);
    let k42 = bitcast<f32>(params.k42); let k43 = bitcast<f32>(params.k43);
    let k44 = bitcast<f32>(params.k44);

    let row0 = row_5(
        sample_la((y + 2u) * width + (x - 2u), channel),
        sample_la((y + 2u) * width + (x - 1u), channel),
        sample_la((y + 2u) * width + x, channel),
        sample_la((y + 2u) * width + (x + 1u), channel),
        sample_la((y + 2u) * width + (x + 2u), channel),
        k00, k01, k02, k03, k04,
    );
    let row1 = row_5(
        sample_la((y + 1u) * width + (x - 2u), channel),
        sample_la((y + 1u) * width + (x - 1u), channel),
        sample_la((y + 1u) * width + x, channel),
        sample_la((y + 1u) * width + (x + 1u), channel),
        sample_la((y + 1u) * width + (x + 2u), channel),
        k10, k11, k12, k13, k14,
    );
    let row2 = row_5(
        sample_la(y * width + (x - 2u), channel),
        sample_la(y * width + (x - 1u), channel),
        sample_la(y * width + x, channel),
        sample_la(y * width + (x + 1u), channel),
        sample_la(y * width + (x + 2u), channel),
        k20, k21, k22, k23, k24,
    );
    let row3 = row_5(
        sample_la((y - 1u) * width + (x - 2u), channel),
        sample_la((y - 1u) * width + (x - 1u), channel),
        sample_la((y - 1u) * width + x, channel),
        sample_la((y - 1u) * width + (x + 1u), channel),
        sample_la((y - 1u) * width + (x + 2u), channel),
        k30, k31, k32, k33, k34,
    );
    let row4 = row_5(
        sample_la((y - 2u) * width + (x - 2u), channel),
        sample_la((y - 2u) * width + (x - 1u), channel),
        sample_la((y - 2u) * width + x, channel),
        sample_la((y - 2u) * width + (x + 1u), channel),
        sample_la((y - 2u) * width + (x + 2u), channel),
        k40, k41, k42, k43, k44,
    );

    var sum = f32(params.offset_val) + 0.5;
    sum = fma(row0, 1.0, sum);
    sum = fma(row1, 1.0, sum);
    sum = fma(row2, 1.0, sum);
    sum = fma(row3, 1.0, sum);
    sum = fma(row4, 1.0, sum);
    return clip8(sum);
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel_count = params.width * params.height;
    let packed_word_count = pixel_count / 2u + select(0u, 1u, (pixel_count & 1u) != 0u);
    if gid.x >= packed_word_count { return; }

    let first_pixel = gid.x * 2u;
    var packed = 0u;
    for (var lane = 0u; lane < 2u; lane += 1u) {
        let pixel_index = first_pixel + lane;
        if pixel_index < pixel_count {
            let x = pixel_index % params.width;
            let y = pixel_index / params.width;
            let luma = filter_pixel_channel(x, y, 0u);
            let alpha = filter_pixel_channel(x, y, 1u);
            let pixel = luma | (alpha << 8u);
            packed = packed | (pixel << (lane * 16u));
        }
    }
    output[gid.x] = packed;
}
