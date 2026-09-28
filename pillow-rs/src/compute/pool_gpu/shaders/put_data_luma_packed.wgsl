// Image.putdata for native L samples. The compact source and replacement
// buffers hold four adjacent bytes in each u32. A byte-prefix write preserves
// every source sample after data_len and zeroes only transport padding.

struct Params {
    word_count: u32,
    _pad0: u32,
    _pad1: u32,
    image_len: u32,
    data_len: u32,
    _pad2: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read> data: array<u32>;
@group(0) @binding(2) var<storage, read_write> output: array<u32>;
@group(0) @binding(3) var<uniform> params: Params;

fn replace_byte(pixel: u32, lane: u32, value: u32) -> u32 {
    let shift = lane * 8u;
    return (pixel & ~(0xffu << shift)) | (value << shift);
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let word_index = gid.x;
    if word_index >= params.word_count {
        return;
    }

    let sample_start = word_index * 4u;
    var packed = 0u;
    if params.data_len < params.image_len && sample_start + 4u > params.data_len {
        // Complete words before the replacement prefix need no source read.
        // This condition loads only words that contain preserved pixels.
        packed = input[word_index];
    }

    if sample_start < params.data_len {
        let replacement = data[word_index];
        for (var lane = 0u; lane < 4u; lane++) {
            let sample = sample_start + lane;
            if sample < params.data_len {
                let value = (replacement >> (lane * 8u)) & 0xffu;
                packed = replace_byte(packed, lane, value);
            } else if sample >= params.image_len {
                packed = replace_byte(packed, lane, 0u);
            }
        }
    } else {
        for (var lane = 0u; lane < 4u; lane++) {
            if sample_start + lane >= params.image_len {
                packed = replace_byte(packed, lane, 0u);
            }
        }
    }

    output[word_index] = packed;
}
