// Exact EffectSpread gather. The host reproduces Pillow's process-global
// random scatter loop and uploads one source-pixel index for each destination
// pixel. The device only performs the deterministic packed gather.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
// One packed little-endian u32 per mapping entry. The synthetic image is
// width*height by one pixel so the ordinary auxiliary uploader can carry it.
@group(0) @binding(1) var<storage, read> mapping: array<u32>;
@group(0) @binding(2) var<storage, read_write> output: array<u32>;
@group(0) @binding(3) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }
    let index = gid.y * params.width + gid.x;
    output[index] = input[mapping[index]];
}
