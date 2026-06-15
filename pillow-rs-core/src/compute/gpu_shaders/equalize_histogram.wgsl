// Equalize histogram: atomic accumulation of R/G/B channel values.
// Identical algorithm to autocontrast_histogram. Separate shader for pipeline
// clarity and so the registry can reference a unique entry point.
//
// CPU reference (image.rs:2108): histogram accumulation, same pattern.
// 1 workgroup (256 threads), each thread processes a chunk of pixels.
//
// Mode-aware: L/LA only accumulate R channel (luma); RGB/RGBA accumulate R,G,B.
// Params: standard header.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
}

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> histogram: array<atomic<u32>, 256>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let tid = gid.x;
    let total_pixels = params.width * params.height;
    let pixels_per_thread = (total_pixels + 255u) / 256u;
    let start = tid * pixels_per_thread;
    let end = min(start + pixels_per_thread, total_pixels);

    let mode = params.mode;

    for (var i = start; i < end; i = i + 1u) {
        let pixel = input[i];
        let r = pixel & 0xffu;
        atomicAdd(&histogram[r], 1u);

        if mode_has_g(mode) {
            let g = (pixel >> 8u) & 0xffu;
            atomicAdd(&histogram[g], 1u);
        }
        if mode_has_b(mode) {
            let b = (pixel >> 16u) & 0xffu;
            atomicAdd(&histogram[b], 1u);
        }
    }
}
