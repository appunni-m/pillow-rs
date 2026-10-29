// Image.putalpha(mask): keep RGBA pixels and the L mask in native storage,
// replacing only the alpha byte in each output word.
struct Params {
    width: u32,
    height: u32,
    mask_byte_len: u32,
    _reserved: u32,
}

@group(0) @binding(0) var<storage, read> rgba: array<u32>;
@group(0) @binding(1) var<storage, read> mask: array<u32>;
@group(0) @binding(2) var<storage, read_write> output: array<u32>;
@group(0) @binding(3) var<uniform> params: Params;

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
    output[index] = (rgba[index] & 0x00ffffffu) | (read_mask_byte(index) << 24u);
}
