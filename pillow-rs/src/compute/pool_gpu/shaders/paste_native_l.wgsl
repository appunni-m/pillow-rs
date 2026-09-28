// Unmasked L-mode Paste with one stored byte per sample. Each invocation
// writes one packed output word so adjacent bytes never race on a u32 store.

struct Params {
    width: u32,
    height: u32,
    source_width: u32,
    source_height: u32,
    byte_length: u32,
    word_count: u32,
    paste_x: i32,
    paste_y: i32,
}

@group(0) @binding(0) var<storage, read> input_destination: array<u32>;
@group(0) @binding(1) var<storage, read> input_source: array<u32>;
@group(0) @binding(2) var<storage, read_write> output: array<u32>;
@group(0) @binding(3) var<uniform> params: Params;

fn read_destination(byte_index: u32) -> u32 {
    let word = input_destination[byte_index / 4u];
    return (word >> ((byte_index & 3u) * 8u)) & 0xffu;
}

fn read_source(byte_index: u32) -> u32 {
    let word = input_source[byte_index / 4u];
    return (word >> ((byte_index & 3u) * 8u)) & 0xffu;
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let word_index = gid.x;
    if word_index >= params.word_count {
        return;
    }

    var packed = 0u;
    for (var lane = 0u; lane < 4u; lane += 1u) {
        let destination_index = word_index * 4u + lane;
        if destination_index >= params.byte_length {
            continue;
        }

        let destination_x = destination_index % params.width;
        let destination_y = destination_index / params.width;
        var inside_x = true;
        var inside_y = true;
        var source_x = 0u;
        var source_y = 0u;
        if params.paste_x >= 0i {
            let origin_x = u32(params.paste_x);
            if destination_x < origin_x {
                inside_x = false;
            } else {
                source_x = destination_x - origin_x;
            }
        } else {
            let distance_x = 0u - bitcast<u32>(params.paste_x);
            if distance_x > 0xffffffffu - destination_x {
                inside_x = false;
            } else {
                source_x = destination_x + distance_x;
            }
        }
        if params.paste_y >= 0i {
            let origin_y = u32(params.paste_y);
            if destination_y < origin_y {
                inside_y = false;
            } else {
                source_y = destination_y - origin_y;
            }
        } else {
            let distance_y = 0u - bitcast<u32>(params.paste_y);
            if distance_y > 0xffffffffu - destination_y {
                inside_y = false;
            } else {
                source_y = destination_y + distance_y;
            }
        }

        var value = read_destination(destination_index);
        if inside_x
            && inside_y
            && source_x < params.source_width
            && source_y < params.source_height
        {
            let source_index = source_y * params.source_width + source_x;
            value = read_source(source_index);
        }
        packed |= value << (lane * 8u);
    }
    output[word_index] = packed;
}
