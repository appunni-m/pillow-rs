// Equalize CDF: compute cumulative distribution function from histogram
// and build a 256-entry look-up table for histogram equalization.
// 1 workgroup (256 threads), single-thread computes the CDF.
//
// CPU reference: step = (total - last_bin_count) / 255
//   n = step / 2  (start at half-step for rounding)
//   for i in 0..256:
//     lut[i] = clamp(n / step, 0, 255)
//     n += histogram[i]
//
// Results: lut as array<u32, 256> in storage buffer at binding(1)
//
// Mode-aware: total_pixels adjusts for active channels.
//   L/LA: 1 channel (R only)
//   RGB/RGBA: 3 channels (R,G,B)
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

@group(0) @binding(0) var<storage, read> histogram_data: array<u32>;
@group(0) @binding(1) var<storage, read_write> lut: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

var<workgroup> shared_hist: array<u32, 256>;

@compute @workgroup_size(256)
fn main(@builtin(local_invocation_id) lid: vec3<u32>) {
    let tid = lid.x;

    // Copy histogram to shared memory for fast access
    shared_hist[tid] = histogram_data[tid];
    workgroupBarrier();

    // Single thread builds the CDF LUT
    if tid == 0u {
        // L/LA: 1 channel histogram (R only); RGB/RGBA: 3 channels (R+G+B)
        let num_channels = select(1u, 3u, params.mode >= 2u);
        let total_pixels = f32(params.width * params.height * num_channels);
        let last_bin_count = f32(shared_hist[255]);
        let step = max((total_pixels - last_bin_count) / 255.0, 1.0);
        var n: f32 = step / 2.0;

        for (var i = 0u; i < 256u; i = i + 1u) {
            let val = u32(clamp(n / step, 0.0, 255.0));
            lut[i] = val;
            n = n + f32(shared_hist[i]);
        }
    }
}
