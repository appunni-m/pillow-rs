// Equalize histogram gather.
//
// Equalize derives a separate LUT for each active byte channel. Keep the
// channel histograms disjoint so the LUT pass can reproduce Pillow's per-band
// integer step calculation exactly.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
}

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read> _mask: array<u32>;
@group(0) @binding(2) var<storage, read_write> histogram: array<atomic<u32>, 1024>;
@group(0) @binding(3) var<uniform> params: Params;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let tid = gid.x;
    let total_pixels = params.width * params.height;
    let pixels_per_thread = (total_pixels + 255u) / 256u;
    let start = tid * pixels_per_thread;
    let end = min(start + pixels_per_thread, total_pixels);

    for (var i = start; i < end; i = i + 1u) {
        let pixel = input[i];
        let r = pixel & 0xffu;
        atomicAdd(&histogram[r], 1u);

        if mode_has_g(params.mode) {
            let g = (pixel >> 8u) & 0xffu;
            atomicAdd(&histogram[256u + g], 1u);
        }
        if mode_has_b(params.mode) {
            let b = (pixel >> 16u) & 0xffu;
            atomicAdd(&histogram[512u + b], 1u);
        }
    }
}
