// Autocontrast cutoff: find lo/hi after removing cutoff percent of extreme pixels.
// 1 workgroup (256 threads), reads histogram from atomic storage into shared memory,
// walks bins to locate cutoff boundaries, computes linear stretch parameters.
//
// CPU reference: Walk histogram bins to find lo/hi after cutoff removal.
//   cutoff_low = total_pixels * cutoff / 100
//   Walk bins 0->255 subtracting bin counts until cutoff_low exhausted -> lo
//   Walk bins 255->0 subtracting until cutoff_low exhausted -> hi
//   scale = 255.0 / (hi - lo), offset = -lo * scale
//
// Results: [lo, hi, scale_f32_bits, offset_f32_bits] written to storage at binding(1)
//
// Mode-aware: total_pixels adjusts for active channels.
//   L/LA: 1 channel (R only); RGB/RGBA: 3 channels (R,G,B).
// Params: cutoff_bits (f32 stored in u32) after standard header.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    cutoff_bits: u32,
}

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

@group(0) @binding(0) var<storage, read> histogram_data: array<u32>;
@group(0) @binding(1) var<storage, read_write> results: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

var<workgroup> shared_hist: array<u32, 256>;

@compute @workgroup_size(256)
fn main(@builtin(local_invocation_id) lid: vec3<u32>) {
    let tid = lid.x;

    // Copy atomic histogram into shared memory for fast access
    shared_hist[tid] = histogram_data[tid];
    workgroupBarrier();

    // Single thread performs the cutoff calculation
    if tid == 0u {
        // L/LA: 1 channel histogram (R only); RGB/RGBA: 3 channels (R+G+B)
        let num_channels = select(1u, 3u, params.mode >= 2u);
        let total_pixels = params.width * params.height * num_channels;
        let cutoff = bitcast<f32>(params.cutoff_bits);
        let cutoff_low = u32(f32(total_pixels) * cutoff / 100.0);

        // Walk forward 0..255 to find lo
        var accum: u32 = 0u;
        var lo: u32 = 0u;
        var found_lo: bool = false;
        for (var i = 0u; i < 256u; i = i + 1u) {
            accum = accum + shared_hist[i];
            if !found_lo && accum >= cutoff_low {
                lo = i;
                found_lo = true;
            }
        }

        // Walk backward 255..0 to find hi
        accum = 0u;
        var hi: u32 = 0u;
        var found_hi: bool = false;
        for (var i = 0u; i < 256u; i = i + 1u) {
            let idx = 255u - i;
            accum = accum + shared_hist[idx];
            if !found_hi && accum >= cutoff_low {
                hi = idx;
                found_hi = true;
            }
        }

        // Edge case: all pixels in same bin -> expand range
        if hi == lo {
            if hi < 255u { hi = hi + 1u; } else { lo = lo - 1u; }
        }

        // Compute scale and offset as f32, store u32 bit patterns
        let scale = 255.0 / f32(hi - lo);
        let offset = -f32(lo) * scale;

        results[0] = lo;
        results[1] = hi;
        results[2] = bitcast<u32>(scale);
        results[3] = bitcast<u32>(offset);
    }
}
