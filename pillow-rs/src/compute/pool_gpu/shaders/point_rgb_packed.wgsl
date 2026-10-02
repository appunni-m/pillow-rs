// Native RGB point remap. Four three-byte pixels are read from compact input
// and emitted as the three u32 words that contain their 12 output bytes.
struct Params {
    width: u32,
    height: u32,
    _reserved: u32,
    flags: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;
@group(0) @binding(3) var<storage, read> lut: array<u32, 256>;

fn remap_rgb(pixel: u32) -> u32 {
    let red = lut[pixel & 0xffu] & 0xffu;
    let green = (lut[(pixel >> 8u) & 0xffu] >> 8u) & 0xffu;
    let blue = (lut[(pixel >> 16u) & 0xffu] >> 16u) & 0xffu;
    return red | (green << 8u) | (blue << 16u);
}

@compute @workgroup_size(16, 16)
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(num_workgroups) num_workgroups: vec3<u32>,
) {
    let total_pixels = params.width * params.height;
    var output_group = gid.x + (num_workgroups.x * 16u) * gid.y;
    if (params.flags & 16u) != 0u {
        let groups_per_row = params.width >> 2u;
        if gid.x >= groups_per_row || gid.y >= params.height {
            return;
        }
        output_group = gid.y * groups_per_row + gid.x;
    }
    let output_group_count = (total_pixels >> 2u) + select(0u, 1u, (total_pixels & 3u) != 0u);
    if output_group >= output_group_count {
        return;
    }

    let first_pixel = output_group * 4u;
    let first_word = output_group * 3u;
    let packed0 = input[first_word];
    var packed1 = 0u;
    var packed2 = 0u;
    if first_pixel + 1u < total_pixels {
        packed1 = input[first_word + 1u];
    }
    if first_pixel + 2u < total_pixels {
        packed2 = input[first_word + 2u];
    }

    var pixel0 = 0u;
    var pixel1 = 0u;
    var pixel2 = 0u;
    var pixel3 = 0u;
    if first_pixel < total_pixels {
        pixel0 = remap_rgb(packed0 & 0x00ffffffu);
    }
    if first_pixel + 1u < total_pixels {
        let source = (packed0 >> 24u) | ((packed1 & 0x0000ffffu) << 8u);
        pixel1 = remap_rgb(source);
    }
    if first_pixel + 2u < total_pixels {
        let source = (packed1 >> 16u) | ((packed2 & 0x000000ffu) << 16u);
        pixel2 = remap_rgb(source);
    }
    if first_pixel + 3u < total_pixels {
        pixel3 = remap_rgb((packed2 >> 8u) & 0x00ffffffu);
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
