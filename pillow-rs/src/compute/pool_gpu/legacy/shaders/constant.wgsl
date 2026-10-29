// Constant: produce a new 8-bit L image filled with one value. Pack four
// consecutive luma samples into each storage word for compact readback.

struct Params {
    width: u32,
    height: u32,
    mode: u32,    // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    value: u32,
}

@group(0) @binding(0) var<storage, read_write> output: array<u32>;
@group(0) @binding(1) var<uniform> params: Params;

@compute @workgroup_size(64, 1, 1)
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(num_workgroups) workgroups: vec3<u32>,
) {
    let pixel_count = params.width * params.height;
    let output_word_count = pixel_count / 4u + select(0u, 1u, pixel_count % 4u != 0u);
    let output_word = gid.x + gid.y * workgroups.x * 64u;
    if output_word >= output_word_count { return; }

    let value = params.value & 0xffu;
    output[output_word] = value * 0x01010101u;
}
