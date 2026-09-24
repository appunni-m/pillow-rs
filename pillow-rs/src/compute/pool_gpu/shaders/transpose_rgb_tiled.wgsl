// Native RGB axis swaps with output rows divisible by four pixels. A lane
// owns four adjacent output pixels (three complete u32 words). A padded
// 16x16 tile makes both the input loads and output stores contiguous.
// params.width/height are OUTPUT dimensions; op_code matches transpose.wgsl.
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

var<workgroup> tile: array<u32, 272>;

fn source_coord(x: u32, y: u32, width: u32, height: u32, method: u32) -> vec2<u32> {
    switch method {
        case 2u: { return vec2<u32>(width - 1u - y, x); }
        case 4u: { return vec2<u32>(y, height - 1u - x); }
        case 5u: { return vec2<u32>(y, x); }
        case 6u: { return vec2<u32>(width - 1u - y, height - 1u - x); }
        default: { return vec2<u32>(y, x); }
    }
}

fn load_rgb(pixel: u32) -> u32 {
    let byte_index = pixel * 3u;
    let word_index = byte_index / 4u;
    let offset = byte_index % 4u;
    var value = input[word_index] >> (offset * 8u);
    if offset > 1u {
        value |= input[word_index + 1u] << ((4u - offset) * 8u);
    }
    return value & 0x00ffffffu;
}

@compute @workgroup_size(4, 16)
fn main(
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(workgroup_id) group: vec3<u32>,
) {
    let width = params.width;
    let height = params.height;
    let origin = group.xy * 16u;
    let local_x = lid.x * 4u;

    // Exchanging the local axes makes every lane's four source pixels
    // adjacent (possibly reversed). Store each at its output tile position.
    for (var lane = 0u; lane < 4u; lane += 1u) {
        let x = origin.x + lid.y;
        let y = origin.y + local_x + lane;
        var color = 0u;
        if x < width && y < height {
            let point = source_coord(x, y, height, width, params.op_code);
            color = load_rgb(point.y * height + point.x);
        }
        tile[(local_x + lane) * 17u + lid.y] = color;
    }
    // Edge lanes initialize their slots and reach this barrier too. A valid
    // output only reads slots whose corresponding source coordinate was valid.
    workgroupBarrier();

    let x = origin.x + local_x;
    let y = origin.y + lid.y;
    if x >= width || y >= height { return; }
    let start = lid.y * 17u + local_x;
    let color0 = tile[start];
    let color1 = tile[start + 1u];
    let color2 = tile[start + 2u];
    let color3 = tile[start + 3u];
    // width and x are divisible by four, so all four pixels exist and the
    // first byte is word-aligned. No other lane writes any of these words.
    let first_word = (y * width + x) * 3u / 4u;
    output[first_word] = color0 | (color1 << 24u);
    output[first_word + 1u] = (color1 >> 8u) | (color2 << 16u);
    output[first_word + 2u] = (color2 >> 16u) | (color3 << 8u);
}
