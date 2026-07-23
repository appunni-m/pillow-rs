// Image.putdata: replace logical mode samples until either data or image ends.
//
// The auxiliary buffer contains one packed logical pixel per u32. Short data
// never zero-fills the destination: untouched samples come from input.
//
// PixelMode codes:
//   0=L, 1=LA, 2=RGB, 3=RGBA, 4=P, 5=PA, 6=CMYK, 7=1,
//   8=YCbCr, 9=HSV, 10=I, 11=F.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    data_len: u32,
    data_mode: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read> data: array<u32>;
@group(0) @binding(2) var<storage, read_write> output: array<u32>;
@group(0) @binding(3) var<uniform> params: Params;

fn channel_count(mode: u32) -> u32 {
    if mode == 0u || mode == 4u || mode == 7u {
        return 1u;
    }
    if mode == 1u || mode == 5u {
        return 2u;
    }
    if mode == 2u || mode == 8u || mode == 9u {
        return 3u;
    }
    return 4u;
}

fn replace_byte(pixel: u32, channel: u32, value: u32) -> u32 {
    let shift = channel * 8u;
    return (pixel & ~(0xffu << shift)) | (value << shift);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height {
        return;
    }

    let index = gid.y * params.width + gid.x;
    let channels = channel_count(params.data_mode);
    let sample_start = index * channels;
    var pixel = input[index];

    if sample_start >= params.data_len {
        output[index] = pixel;
        return;
    }

    let replacement = data[index];
    if channels == 1u {
        let value = replacement & 0xffu;
        output[index] =
            value | (value << 8u) | (value << 16u) | 0xff000000u;
        return;
    }

    if channels == 2u {
        let luma = replacement & 0xffu;
        pixel = (pixel & 0xff000000u) | luma | (luma << 8u) | (luma << 16u);
        if sample_start + 1u < params.data_len {
            pixel = replace_byte(pixel, 3u, (replacement >> 24u) & 0xffu);
        }
        output[index] = pixel;
        return;
    }

    for (var channel = 0u; channel < channels; channel++) {
        if sample_start + channel < params.data_len {
            let value = (replacement >> (channel * 8u)) & 0xffu;
            pixel = replace_byte(pixel, channel, value);
        }
    }
    if channels == 3u {
        pixel = (pixel & 0x00ffffffu) | 0xff000000u;
    }
    output[index] = pixel;
}
