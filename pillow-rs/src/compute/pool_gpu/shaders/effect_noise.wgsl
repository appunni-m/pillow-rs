// Exact EffectNoise gather. Pillow's rejection sampling, floating-point
// Box-Muller transform, and process-global RNG are reproduced by the host;
// this shader only copies the generated L-mode byte through packed storage.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read> noise: array<u32>;
@group(0) @binding(2) var<storage, read_write> output: array<u32>;
@group(0) @binding(3) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }
    let index = gid.y * params.width + gid.x;
    output[index] = noise[index];
}
