// Convert packed source samples to Pillow-compatible L and pack four output
// pixels per word. CMYK remains C/M/Y/K in the input word and is expanded to
// RGB only in registers before luma; no host RGBA image is materialized.

struct Params {
    width: u32,
    height: u32,
    mode: u32, // 0=L, 1=LA, 2=RGB/YCbCr, 3=RGBA, 4=CMYK
    _pad: u32, // 0=packed RGBA words, 1=native RGB triples, 2=native YCbCr triples.
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn muldiv255(a: u32, b: u32) -> u32 {
    let value = a * b + 128u;
    return ((value >> 8u) + value) >> 8u;
}

fn rgb_luma(red: u32, green: u32, blue: u32) -> u32 {
    return (19595u * red + 38470u * green + 7471u * blue + 32768u) >> 16u;
}

fn cmyk_luma(pixel: u32) -> u32 {
    let first = pixel & 0xffu;
    let second = (pixel >> 8u) & 0xffu;
    let third = (pixel >> 16u) & 0xffu;
    let k = (pixel >> 24u) & 0xffu;
    let ink = 255u - k;
    let red = ink - muldiv255(first, ink);
    let green = ink - muldiv255(second, ink);
    let blue = ink - muldiv255(third, ink);
    return rgb_luma(red, green, blue);
}

fn pixel_luma(pixel: u32) -> u32 {
    if params.mode == 4u {
        return cmyk_luma(pixel);
    }
    let first = pixel & 0xffu;
    let second = (pixel >> 8u) & 0xffu;
    let third = (pixel >> 16u) & 0xffu;
    // L and LA uploads replicate their luma value across RGB. RGBA ignores
    // alpha, matching Pillow's ImageOps.grayscale contract.
    return rgb_luma(first, second, third);
}

fn native_cmyk_group_luma(output_word: u32) -> u32 {
    // CMYK uses the ordinary four-byte transport. One output word covers four
    // adjacent pixels, so load those source words directly and skip the
    // per-pixel loop and tail bounds checks for complete groups.
    let first_pixel = output_word * 4u;
    let l0 = cmyk_luma(input[first_pixel]);
    let l1 = cmyk_luma(input[first_pixel + 1u]);
    let l2 = cmyk_luma(input[first_pixel + 2u]);
    let l3 = cmyk_luma(input[first_pixel + 3u]);
    return l0 | (l1 << 8u) | (l2 << 16u) | (l3 << 24u);
}

fn native_triple(pixel_index: u32) -> u32 {
    // RGB and YCbCr pixels are three bytes wide, so a pixel may straddle two u32 words.
    // Admission checks bound width*height*3 to u32 and upload pads the final
    // word; when shift is 16 or 24, the pixel's third byte guarantees the
    // following word is present.
    let byte_offset = pixel_index * 3u;
    let word_index = byte_offset >> 2u;
    let shift = (byte_offset & 3u) * 8u;
    var packed = input[word_index] >> shift;
    if shift > 8u {
        packed |= input[word_index + 1u] << (32u - shift);
    }
    return packed;
}

fn native_ycbcr_y(pixel_index: u32) -> u32 {
    // The Y byte always fits in one word even when the remaining triple
    // crosses a boundary, so avoid loading and joining the chroma bytes.
    let byte_offset = pixel_index * 3u;
    let word_index = byte_offset >> 2u;
    let shift = (byte_offset & 3u) * 8u;
    return (input[word_index] >> shift) & 0xffu;
}

fn native_rgb_group_luma(output_word: u32) -> u32 {
    // One output word contains four adjacent L samples. Their twelve RGB
    // bytes are exactly three aligned input words, so load each source word
    // once instead of having four independent 3-byte gathers reread overlaps.
    let first = input[output_word * 3u];
    let second = input[output_word * 3u + 1u];
    let third = input[output_word * 3u + 2u];

    let l0 = rgb_luma(first & 0xffu, (first >> 8u) & 0xffu, (first >> 16u) & 0xffu);
    let l1 = rgb_luma(first >> 24u, second & 0xffu, (second >> 8u) & 0xffu);
    let l2 = rgb_luma((second >> 16u) & 0xffu, second >> 24u, third & 0xffu);
    let l3 = rgb_luma((third >> 8u) & 0xffu, (third >> 16u) & 0xffu, third >> 24u);
    return l0 | (l1 << 8u) | (l2 << 16u) | (l3 << 24u);
}

fn source_luma(pixel_index: u32) -> u32 {
    if params._pad == 1u {
        return pixel_luma(native_triple(pixel_index));
    }
    if params._pad == 2u {
        // Pillow converts YCbCr to L by copying the Y band without RGB
        // conversion or a second weighted-luma calculation.
        return native_ycbcr_y(pixel_index);
    }
    return pixel_luma(input[pixel_index]);
}

@compute @workgroup_size(256, 1, 1)
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(num_workgroups) workgroups: vec3<u32>,
) {
    let pixel_count = params.width * params.height;
    let output_word_count = pixel_count / 4u + select(0u, 1u, pixel_count % 4u != 0u);
    let output_word = gid.x + gid.y * workgroups.x * 256u;
    if output_word >= output_word_count { return; }

    var packed = 0u;
    if params.mode == 4u && output_word < pixel_count / 4u {
        packed = native_cmyk_group_luma(output_word);
    } else if params._pad == 1u && output_word < pixel_count / 4u {
        // The complete four-pixel groups use three aligned RGB loads. A final
        // partial group stays on the bounds-checked scalar gather below.
        packed = native_rgb_group_luma(output_word);
    } else {
        for (var lane = 0u; lane < 4u; lane += 1u) {
            let pixel_index = output_word * 4u + lane;
            if pixel_index < pixel_count {
                packed |= source_luma(pixel_index) << (lane * 8u);
            }
        }
    }
    output[output_word] = packed;
}
