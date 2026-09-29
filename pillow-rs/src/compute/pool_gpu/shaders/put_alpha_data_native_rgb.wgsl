// Image.putalpha(mask): consume native three-byte RGB and one-byte L samples,
// then write the public four-byte RGBA result directly.
struct Params {
    width: u32,
    height: u32,
    rgb_byte_len: u32,
    mask_byte_len: u32,
}

@group(0) @binding(0) var<storage, read> rgb: array<u32>;
@group(0) @binding(1) var<storage, read> mask: array<u32>;
@group(0) @binding(2) var<storage, read_write> output: array<u32>;
@group(0) @binding(3) var<uniform> params: Params;

fn read_rgb_byte(byte_offset: u32) -> u32 {
    if byte_offset >= params.rgb_byte_len {
        return 0u;
    }
    let word = rgb[byte_offset >> 2u];
    let shift = (byte_offset & 3u) * 8u;
    return (word >> shift) & 0xffu;
}

fn read_mask_byte(pixel_index: u32) -> u32 {
    if pixel_index >= params.mask_byte_len {
        return 0u;
    }
    let word = mask[pixel_index >> 2u];
    let shift = (pixel_index & 3u) * 8u;
    return (word >> shift) & 0xffu;
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height {
        return;
    }

    let index = gid.y * params.width + gid.x;
    let source_offset = index * 3u;
    let red = read_rgb_byte(source_offset);
    let green = read_rgb_byte(source_offset + 1u);
    let blue = read_rgb_byte(source_offset + 2u);
    let alpha = read_mask_byte(index);
    output[index] = red | (green << 8u) | (blue << 16u) | (alpha << 24u);
}
