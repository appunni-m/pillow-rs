// Native-RGB affine-nearest transform. Four sampled RGB pixels are packed
// into three storage words so the GPU readback stays at three bytes per pixel.
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

fn source_rgb(pixel_index: u32) -> u32 {
    let byte_offset = pixel_index * 3u;
    let word_index = byte_offset >> 2u;
    let shift = (byte_offset & 3u) * 8u;
    var packed = input[word_index] >> shift;
    if shift > 8u {
        packed = packed | (input[word_index + 1u] << (32u - shift));
    }
    return packed & 0x00ffffffu;
}

fn sample_rgb_at_fixed_coordinates(sx: i32, sy: i32) -> u32 {
    if sx < 0 || sy < 0 {
        return params.fill_color & 0x00ffffffu;
    }
    let source_x = u32(sx >> 16);
    let source_y = u32(sy >> 16);
    if source_x >= params.width || source_y >= params.height {
        return params.fill_color & 0x00ffffffu;
    }
    return source_rgb(source_y * params.width + source_x);
}

fn transformed_rgb_at(x: u32, y: u32) -> u32 {
    let step_x = bitcast<i32>(params.a);
    let step_y = bitcast<i32>(params.b);
    let origin_x = bitcast<i32>(params.c);
    let step_x_y = bitcast<i32>(params.d);
    let step_y_y = bitcast<i32>(params.e);
    let origin_y = bitcast<i32>(params.f);
    let sx = origin_x + i32(x) * step_x + i32(y) * step_y;
    let sy = origin_y + i32(x) * step_x_y + i32(y) * step_y_y;
    return sample_rgb_at_fixed_coordinates(sx, sy);
}

@compute @workgroup_size(16, 16)
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(num_workgroups) num_workgroups: vec3<u32>,
) {
    let total_pixels = params.dst_w * params.dst_h;
    var output_group = gid.x + (num_workgroups.x * 16u) * gid.y;
    let output_group_count = (total_pixels >> 2u) + select(0u, 1u, (total_pixels & 3u) != 0u);
    let row_tiled = (params._pad & 16u) != 0u;
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
        // Four-pixel groups remain row-local at aligned widths. Compute the
        // fixed affine base once and advance through the four output samples.
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
        pixel0 = sample_rgb_at_fixed_coordinates(sx, sy);
        pixel1 = sample_rgb_at_fixed_coordinates(sx + step_x, sy + step_x_y);
        pixel2 = sample_rgb_at_fixed_coordinates(sx + 2 * step_x, sy + 2 * step_x_y);
        pixel3 = sample_rgb_at_fixed_coordinates(sx + 3 * step_x, sy + 3 * step_x_y);
    } else {
        // At widths not divisible by four, packed groups can cross row edges.
        for (var lane = 0u; lane < 4u; lane += 1u) {
            let pixel_index = output_group * 4u + lane;
            if pixel_index < total_pixels {
                let x = pixel_index % params.dst_w;
                let y = pixel_index / params.dst_w;
                let pixel = transformed_rgb_at(x, y);
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

    let output_bytes = total_pixels * 3u;
    let output_word_count = (output_bytes >> 2u) + select(0u, 1u, (output_bytes & 3u) != 0u);
    let word = output_group * 3u;
    if word < output_word_count {
        output[word] = (pixel0 & 0x00ffffffu) | ((pixel1 & 0x000000ffu) << 24u);
    }
    if word + 1u < output_word_count {
        output[word + 1u] = ((pixel1 >> 8u) & 0x0000ffffu) | ((pixel2 & 0x0000ffffu) << 16u);
    }
    if word + 2u < output_word_count {
        output[word + 2u] = ((pixel2 >> 16u) & 0x000000ffu) | ((pixel3 & 0x00ffffffu) << 8u);
    }
}
