// Byte relocation in native L, LA, RGB and RGBA storage. Each invocation owns
// one complete output word, including partial rows and odd channel counts.
// No image is expanded to a convenience RGBA representation.
struct Params {
    src_w: u32, src_h: u32, dst_w: u32, dst_h: u32,
    channels: u32, opcode: u32, offset_x: u32, offset_y: u32,
    fill: u32,
}
@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;
fn component_fill(component: u32) -> u32 {
    if params.channels == 1u { return params.fill & 255u; }
    if params.channels == 2u {
        if component == 0u { return params.fill & 255u; }
        return params.fill >> 24u;
    }
    return (params.fill >> (component * 8u)) & 255u;
}
fn source_byte(pixel: u32, component: u32) -> u32 {
    let x = pixel % params.dst_w;
    let y = pixel / params.dst_w;
    var sx = x;
    var sy = y;
    switch params.opcode {
        case 0u: { sx = params.src_w - 1u - x; }
        case 1u: { sy = params.src_h - 1u - y; }
        case 2u: { sx = params.src_w - 1u - y; sy = x; }
        case 3u: { sx = params.src_w - 1u - x; sy = params.src_h - 1u - y; }
        case 4u: { sx = y; sy = params.src_h - 1u - x; }
        case 5u: { sx = y; sy = x; }
        case 6u: { sx = params.src_w - 1u - y; sy = params.src_h - 1u - x; }
        case 7u: { sx = x + params.offset_x; sy = y + params.offset_y; }
        case 8u: {
            if x < params.offset_x || y < params.offset_y { return component_fill(component); }
            sx = x - params.offset_x; sy = y - params.offset_y;
        }
        default: {}
    }
    if sx >= params.src_w || sy >= params.src_h { return component_fill(component); }
    let index = (sy * params.src_w + sx) * params.channels + component;
    return (input[index >> 2u] >> ((index & 3u) * 8u)) & 255u;
}
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let word = gid.x + gid.y * groups.x * 64u;
    let bytes = params.dst_w * params.dst_h * params.channels;
    let words = bytes / 4u + select(0u, 1u, bytes % 4u != 0u);
    if word >= words { return; }
    var packed = 0u;
    for (var lane = 0u; lane < 4u; lane++) {
        let index = word * 4u + lane;
        if index < bytes {
            packed |= source_byte(index / params.channels, index % params.channels) << (lane * 8u);
        }
    }
    output[word] = packed;
}
