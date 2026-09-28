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
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(num_workgroups) workgroups: vec3<u32>,
) {
    let pixel_count = params.width * params.height;
    let output_word_count = pixel_count / 4u + select(0u, 1u, pixel_count % 4u != 0u);
    let output_word = gid.x + gid.y * workgroups.x * 64u;
    if output_word >= output_word_count { return; }
    let first_pixel = output_word * 4u;

    // The pure-operation fast path uploads original 1/2/3/4-byte pixels and
    // marks the otherwise-unused fourth uniform word. Generic batches retain
    // packed RGBA input, including LA alpha in byte three.
    var native_channels = 4u;
    if params.mode <= 2u {
        native_channels = params.mode + 1u;
    }
    var packed = 0u;
    for (var lane = 0u; lane < 4u; lane += 1u) {
        let pixel_index = first_pixel + lane;
        if pixel_index < pixel_count {
            var value = 0u;
            if params._pad != 0u {
                let byte_index = pixel_index * native_channels + params.channel;
                let input_word = input[byte_index / 4u];
                value = (input_word >> ((byte_index % 4u) * 8u)) & 0xffu;
            } else {
                // LA stores its alpha band in byte 3; RGBA channel 1 remains
                // green in the generic four-byte transport.
                let alpha_band = params.mode == 1u && params.channel == 1u;
                let shift = select(params.channel * 8u, 24u, alpha_band);
                value = (input[pixel_index] >> shift) & 0xffu;
            }
            packed |= value << (lane * 8u);
        }
    }
    // Four L samples share one storage word. The result buffer remains
    // word-aligned while transfer/readback moves one byte per output pixel.
    output[output_word] = packed;
}
