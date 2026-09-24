// Native RGB transpose. Four consecutive output pixels occupy three u32
// words, so one invocation owns every byte of its stores without atomics or
// overlapping writes between rows whose byte strides are not word-aligned.
// Input/output buffers contain RGB bytes plus at most three zero padding bytes.
// params.width/height are OUTPUT dimensions; op_code follows transpose.wgsl.
struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    op_code: u32,
    _pad2: u32,
    _pad3: u32,
    _pad4: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn source_coord(x: u32, y: u32, width: u32, height: u32, method: u32) -> vec2<u32> {
    switch method {
        case 0u: { return vec2<u32>(width - 1u - x, y); }
        case 1u: { return vec2<u32>(x, height - 1u - y); }
        case 2u: { return vec2<u32>(width - 1u - y, x); }
        case 3u: { return vec2<u32>(width - 1u - x, height - 1u - y); }
        case 4u: { return vec2<u32>(y, height - 1u - x); }
        case 5u: { return vec2<u32>(y, x); }
        case 6u: { return vec2<u32>(width - 1u - y, height - 1u - x); }
        default: { return vec2<u32>(x, y); }
    }
}

fn load_rgb(pixel: u32) -> u32 {
    let byte_index = pixel * 3u;
    let word_index = byte_index / 4u;
    let offset = byte_index % 4u;
    var value = input[word_index] >> (offset * 8u);
    if offset > 1u {
        // A valid three-byte pixel crosses into the next word only here.
        // Its last byte is still inside the checked, padded upload range.
        value |= input[word_index + 1u] << ((4u - offset) * 8u);
    }
    return value & 0x00ffffffu;
}

@compute @workgroup_size(128)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let width = params.width;
    let height = params.height;
    let pixels = width * height;
    let first_pixel = gid.x * 4u;
    if first_pixel >= pixels { return; }

    let method = params.op_code;
    let swap = method == 2u || method == 4u || method == 5u || method == 6u;
    let source_width = select(width, height, swap);
    let source_height = select(height, width, swap);
    var colors: array<u32, 4>;
    for (var lane = 0u; lane < 4u; lane += 1u) {
        let pixel = first_pixel + lane;
        if pixel < pixels {
            let point = source_coord(pixel % width, pixel / width, source_width, source_height, method);
            colors[lane] = load_rgb(point.y * source_width + point.x);
        } else {
            colors[lane] = 0u;
        }
    }

    let first_word = gid.x * 3u;
    let words = (pixels * 3u + 3u) / 4u;
    if first_word < words {
        output[first_word] = colors[0] | (colors[1] << 24u);
    }
    if first_word + 1u < words {
        output[first_word + 1u] = (colors[1] >> 8u) | (colors[2] << 16u);
    }
    if first_word + 2u < words {
        output[first_word + 2u] = (colors[2] >> 16u) | (colors[3] << 8u);
    }
}
