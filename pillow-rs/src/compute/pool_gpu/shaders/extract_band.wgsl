// ExtractBand: extract single channel from multi-band image.
// Research §2: Simple per-pixel copy, trivially parallel.
// channel: 0=R/luma, 1=G/A, 2=B, 3=A

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    channel: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let first_pixel = gid.x * 4u;
    let pixel_count = params.width * params.height;
    if first_pixel >= pixel_count { return; }

    // LA stores its alpha band in byte 3; RGB/RGBA use the normal byte
    // offsets. `getchannel(1)` must therefore select byte 3 only for the
    // two-band transport; RGBA channel 1 remains green.
    let alpha_band = params.mode == 1u && params.channel == 1u;
    let shift = select(params.channel * 8u, 24u, alpha_band);
    var packed = 0u;
    for (var lane = 0u; lane < 4u; lane += 1u) {
        let pixel_index = first_pixel + lane;
        if pixel_index < pixel_count {
            let value = (input[pixel_index] >> shift) & 0xffu;
            packed |= value << (lane * 8u);
        }
    }
    // Four L samples share one storage word. The result buffer remains
    // word-aligned while transfer/readback moves one byte per output pixel.
    output[gid.x] = packed;
}
