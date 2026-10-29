// Compact native-L affine-nearest transform. Four contiguous output samples
// share one storage word, so each output word has exactly one writer and
// readback stays byte-packed instead of expanding every sample to RGBA.
struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    dst_w: u32,
    dst_h: u32,
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    e: f32,
    f: f32,
    fill_color: u32,
    filter_code: u32,
    premultiply: u32,
    method: u32,
    g: f32,
    h: f32,
    mesh0: f32,
    mesh1: f32,
    mesh2: f32,
    mesh3: f32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn source_luma(pixel_index: u32) -> u32 {
    let word = input[pixel_index >> 2u];
    return (word >> ((pixel_index & 3u) * 8u)) & 0xffu;
}

fn sample_luma_at_fixed_coordinates(sx: i32, sy: i32) -> u32 {
    if sx < 0 || sy < 0 {
        return params.fill_color & 0xffu;
    }
    let source_x = u32(sx >> 16);
    let source_y = u32(sy >> 16);
    if source_x >= params.width || source_y >= params.height {
        return params.fill_color & 0xffu;
    }
    return source_luma(source_y * params.width + source_x);
}

fn transformed_luma_at(x: u32, y: u32) -> u32 {
    let step_x = bitcast<i32>(params.a);
    let step_y = bitcast<i32>(params.b);
    let origin_x = bitcast<i32>(params.c);
    let step_x_y = bitcast<i32>(params.d);
    let step_y_y = bitcast<i32>(params.e);
    let origin_y = bitcast<i32>(params.f);
    let sx = origin_x + i32(x) * step_x + i32(y) * step_y;
    let sy = origin_y + i32(x) * step_x_y + i32(y) * step_y_y;
    return sample_luma_at_fixed_coordinates(sx, sy);
}

@compute @workgroup_size(16, 16)
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(num_workgroups) num_workgroups: vec3<u32>,
) {
    let total_pixels = params.dst_w * params.dst_h;
    let output_word = gid.x + (num_workgroups.x * 16u) * gid.y;
    let output_word_count = (total_pixels >> 2u) + select(0u, 1u, (total_pixels & 3u) != 0u);
    if output_word >= output_word_count {
        return;
    }

    var packed = 0u;
    if (params.dst_w & 3u) == 0u {
        // Aligned rows keep each packed output word within one row. Divide
        // once per word, calculate its first affine sample once, and advance
        // the fixed-point coordinates across the four byte lanes.
        let words_per_row = params.dst_w >> 2u;
        let y = output_word / words_per_row;
        let x = (output_word - y * words_per_row) << 2u;
        let step_x = bitcast<i32>(params.a);
        let step_y = bitcast<i32>(params.b);
        let origin_x = bitcast<i32>(params.c);
        let step_x_y = bitcast<i32>(params.d);
        let step_y_y = bitcast<i32>(params.e);
        let origin_y = bitcast<i32>(params.f);
        let sx = origin_x + i32(x) * step_x + i32(y) * step_y;
        let sy = origin_y + i32(x) * step_x_y + i32(y) * step_y_y;
        for (var lane = 0u; lane < 4u; lane += 1u) {
            let lane_offset = i32(lane);
            packed |= sample_luma_at_fixed_coordinates(
                sx + lane_offset * step_x,
                sy + lane_offset * step_x_y,
            ) << (lane * 8u);
        }
    } else {
        // Odd and non-four-aligned widths can make a packed word cross a row
        // boundary, so retain per-pixel indexing for those exact layouts.
        for (var lane = 0u; lane < 4u; lane += 1u) {
            let pixel_index = output_word * 4u + lane;
            if pixel_index < total_pixels {
                let x = pixel_index % params.dst_w;
                let y = pixel_index / params.dst_w;
                packed |= transformed_luma_at(x, y) << (lane * 8u);
            }
        }
    }
    output[output_word] = packed;
}
