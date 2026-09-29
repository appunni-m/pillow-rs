// Native-L Sharpness reads and writes four luminance samples per storage
// word. This keeps the GPU transfer compact and avoids filtering duplicate
// RGB channels for grayscale input.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    factor: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn native_luma(pixel_index: u32) -> u32 {
    return (input[pixel_index >> 2u] >> ((pixel_index & 3u) * 8u)) & 0xffu;
}

fn blend_luma(center: u32, weighted: u32) -> u32 {
    // Match Pillow's rounded SMOOTH result before its truncating fixed-point
    // blend: floor((2 * weighted + 13) / 26).
    let blurred = (weighted * 2u + 13u) / 26u;
    let factor = i32(params.factor);
    let blended = (i32(blurred) * (1000i - factor) + i32(center) * factor) / 1000i;
    return u32(clamp(blended, 0i, 255i));
}

fn sharpen_pixel(pixel_index: u32) -> u32 {
    let w = params.width;
    let h = params.height;
    let center = native_luma(pixel_index);
    if params.factor == 1000u || w < 3u || h < 3u {
        return center;
    }

    let x = pixel_index % w;
    let y = pixel_index / w;
    if x == 0u || x >= w - 1u || y == 0u || y >= h - 1u {
        return center;
    }

    let above = (y - 1u) * w;
    let row = y * w;
    let below = (y + 1u) * w;
    let weighted = native_luma(above + x - 1u)
        + native_luma(above + x)
        + native_luma(above + x + 1u)
        + native_luma(row + x - 1u)
        + center * 5u
        + native_luma(row + x + 1u)
        + native_luma(below + x - 1u)
        + native_luma(below + x)
        + native_luma(below + x + 1u);
    return blend_luma(center, weighted);
}

fn luma_triplet(before: u32, current: u32, after: u32, lane: u32) -> vec3<u32> {
    let shift = lane * 8u;
    var left = 0u;
    var right = 0u;
    if lane == 0u {
        left = (before >> 24u) & 0xffu;
    } else {
        left = (current >> ((lane - 1u) * 8u)) & 0xffu;
    }
    if lane == 3u {
        right = after & 0xffu;
    } else {
        right = (current >> ((lane + 1u) * 8u)) & 0xffu;
    }
    return vec3<u32>(left, (current >> shift) & 0xffu, right);
}

// One aligned output word contains four horizontally adjacent pixels. Reuse
// the same three packed source words per row across those pixels instead of
// issuing nine independently indexed sample reads for every output pixel.
fn sharpen_interior_word(x: u32, y: u32) -> u32 {
    let w = params.width;
    let top_word = ((y - 1u) * w + x) >> 2u;
    let middle_word = (y * w + x) >> 2u;
    let bottom_word = ((y + 1u) * w + x) >> 2u;

    let top_before = input[top_word - 1u];
    let top_current = input[top_word];
    let top_after = input[top_word + 1u];
    let middle_before = input[middle_word - 1u];
    let middle_current = input[middle_word];
    let middle_after = input[middle_word + 1u];
    let bottom_before = input[bottom_word - 1u];
    let bottom_current = input[bottom_word];
    let bottom_after = input[bottom_word + 1u];

    var packed = 0u;
    for (var lane = 0u; lane < 4u; lane += 1u) {
        let top = luma_triplet(top_before, top_current, top_after, lane);
        let middle = luma_triplet(middle_before, middle_current, middle_after, lane);
        let bottom = luma_triplet(bottom_before, bottom_current, bottom_after, lane);
        let weighted = top.x + top.y + top.z + middle.x + middle.y * 5u + middle.z
            + bottom.x + bottom.y + bottom.z;
        packed |= blend_luma(middle.y, weighted) << (lane * 8u);
    }
    return packed;
}

@compute @workgroup_size(16, 16, 1)
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(num_workgroups) workgroups: vec3<u32>,
) {
    if params.width == 0u || params.height == 0u {
        return;
    }
    let pixel_count = params.width * params.height;
    let output_word_count = pixel_count / 4u + select(0u, 1u, pixel_count % 4u != 0u);
    var output_word = 0u;
    var first_pixel = 0u;
    if params._pad != 0u {
        // Row-aligned plans dispatch output words and rows directly. The
        // fourth fixed uniform word selects this layout only when the planner
        // confirms both group dimensions fit the adapter.
        let words_per_row = params.width / 4u;
        let word_x = gid.x;
        let row = gid.y;
        if word_x >= words_per_row || row >= params.height {
            return;
        }
        output_word = row * words_per_row + word_x;
        first_pixel = row * params.width + word_x * 4u;
        if params.factor != 1000u
            && word_x > 0u
            && word_x + 1u < words_per_row
            && row > 0u
            && row + 1u < params.height
        {
            output[output_word] = sharpen_interior_word(word_x * 4u, row);
            return;
        }
    } else {
        // Unaligned widths keep a flattened packed-word mapping because an
        // output word can straddle two image rows.
        output_word = gid.x + gid.y * workgroups.x * 16u;
        if output_word >= output_word_count {
            return;
        }
        first_pixel = output_word * 4u;
    }

    var packed = 0u;
    for (var lane = 0u; lane < 4u; lane += 1u) {
        let pixel_index = first_pixel + lane;
        if pixel_index < pixel_count {
            packed |= sharpen_pixel(pixel_index) << (lane * 8u);
        }
    }
    // Each invocation owns one output word. Zero-initializing `packed` makes
    // unused bytes in the final word deterministic without touching pixels
    // outside the image.
    output[output_word] = packed;
}
