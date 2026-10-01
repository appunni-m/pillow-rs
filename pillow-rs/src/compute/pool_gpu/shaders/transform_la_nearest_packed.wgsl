// Native-LA affine-nearest transform. Four luma/alpha pixels occupy two
// output words, so every invocation owns complete words without RGBA staging.
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

fn source_la(pixel_index: u32) -> u32 {
    let byte_offset = pixel_index * 2u;
    let word = input[byte_offset >> 2u];
    return (word >> ((byte_offset & 2u) * 8u)) & 0xffffu;
}

fn fill_la() -> u32 {
    return (params.fill_color & 0xffu) | (((params.fill_color >> 24u) & 0xffu) << 8u);
}

fn sample_la_at_fixed_coordinates(sx: i32, sy: i32) -> u32 {
    if sx < 0 || sy < 0 {
        return fill_la();
    }
    let source_x = u32(sx >> 16);
    let source_y = u32(sy >> 16);
    if source_x >= params.width || source_y >= params.height {
        return fill_la();
    }
    return source_la(source_y * params.width + source_x);
}

fn transformed_la_at(x: u32, y: u32) -> u32 {
    let step_x = bitcast<i32>(params.a);
    let step_y = bitcast<i32>(params.b);
    let origin_x = bitcast<i32>(params.c);
    let step_x_y = bitcast<i32>(params.d);
    let step_y_y = bitcast<i32>(params.e);
    let origin_y = bitcast<i32>(params.f);
    let sx = origin_x + i32(x) * step_x + i32(y) * step_y;
    let sy = origin_y + i32(x) * step_x_y + i32(y) * step_y_y;
    return sample_la_at_fixed_coordinates(sx, sy);
}

@compute @workgroup_size(16, 16)
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(num_workgroups) num_workgroups: vec3<u32>,
) {
    let total_pixels = params.dst_w * params.dst_h;
    let output_group_count = (total_pixels >> 2u) + select(0u, 1u, (total_pixels & 3u) != 0u);
    var output_group = gid.x + (num_workgroups.x * 16u) * gid.y;
    let row_tiled = (params._pad & 32u) != 0u;
    if row_tiled {
        let groups_per_row = params.dst_w >> 2u;
        if gid.x >= groups_per_row || gid.y >= params.dst_h {
            return;
        }
        output_group = gid.y * groups_per_row + gid.x;
    }
    if output_group >= output_group_count {
        return;
    }

    var pixel0 = 0u;
    var pixel1 = 0u;
    var pixel2 = 0u;
    var pixel3 = 0u;
    if (params.dst_w & 3u) == 0u {
        // Aligned widths keep four-pixel groups within rows. Share the affine
        // base across lanes and advance fixed coordinates without division.
        let groups_per_row = params.dst_w >> 2u;
        var x = 0u;
        var y = 0u;
        if row_tiled {
            x = gid.x << 2u;
            y = gid.y;
        } else {
            y = output_group / groups_per_row;
            x = (output_group - y * groups_per_row) << 2u;
        }
        let step_x = bitcast<i32>(params.a);
        let step_y = bitcast<i32>(params.b);
        let origin_x = bitcast<i32>(params.c);
        let step_x_y = bitcast<i32>(params.d);
        let step_y_y = bitcast<i32>(params.e);
        let origin_y = bitcast<i32>(params.f);
        let sx = origin_x + i32(x) * step_x + i32(y) * step_y;
        let sy = origin_y + i32(x) * step_x_y + i32(y) * step_y_y;
        pixel0 = sample_la_at_fixed_coordinates(sx, sy);
        pixel1 = sample_la_at_fixed_coordinates(sx + step_x, sy + step_x_y);
        pixel2 = sample_la_at_fixed_coordinates(sx + 2 * step_x, sy + 2 * step_x_y);
        pixel3 = sample_la_at_fixed_coordinates(sx + 3 * step_x, sy + 3 * step_x_y);
    } else {
        // Odd widths can split a four-pixel group across rows. Address the
        // image as a flat pixel stream while retaining one owner per pair of
        // output words.
        for (var lane = 0u; lane < 4u; lane += 1u) {
            let pixel_index = output_group * 4u + lane;
            if pixel_index < total_pixels {
                let x = pixel_index % params.dst_w;
                let y = pixel_index / params.dst_w;
                let pixel = transformed_la_at(x, y);
                if lane == 0u {
                    pixel0 = pixel;
                } else if lane == 1u {
                    pixel1 = pixel;
                } else if lane == 2u {
                    pixel2 = pixel;
                } else {
                    pixel3 = pixel;
                }
            }
        }
    }

    let output_word = output_group * 2u;
    let output_word_count = (total_pixels >> 1u) + select(0u, 1u, (total_pixels & 1u) != 0u);
    if output_word < output_word_count {
        output[output_word] = (pixel0 & 0xffffu) | ((pixel1 & 0xffffu) << 16u);
    }
    if output_word + 1u < output_word_count {
        output[output_word + 1u] = (pixel2 & 0xffffu) | ((pixel3 & 0xffffu) << 16u);
    }
}
