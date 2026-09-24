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
@group(0) @binding(1) var<storage, read> mask: array<u32>;
@group(0) @binding(2) var<storage, read_write> histogram: array<atomic<u32>, 1024>;
@group(0) @binding(3) var<uniform> params: Params;

// Accumulate privately per workgroup before merging the occupied bins. This
// bounds global atomic contention even for an image containing one color.
var<workgroup> local_histogram: array<atomic<u32>, 768>;

@compute @workgroup_size(256)
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(num_workgroups) groups: vec3<u32>,
) {
    atomicStore(&local_histogram[lid.x], 0u);
    atomicStore(&local_histogram[256u + lid.x], 0u);
    atomicStore(&local_histogram[512u + lid.x], 0u);
    workgroupBarrier();

    let total_pixels = params.width * params.height;
    let stride = groups.x * 256u;
    // Adjacent lanes read adjacent pixels. Fold repeated samples within each
    // lane before touching shared atomics: flat regions otherwise serialize
    // every lane on the same three counters. A changed sample flushes the
    // complete run, so this remains exact for arbitrary image content.
    var previous = 0u;
    var run = 0u;
    for (var i = gid.x; i < total_pixels; i = i + stride) {
        // The mask chooses histogram samples, not output pixels. Skipping a
        // sample can keep the current run open because its population is not
        // incremented until a selected sample is visited.
        if params._pad != 0u && (mask[i] & 0xffu) == 0u { continue; }
        let pixel = input[i];
        if run > 0u && pixel != previous {
            atomicAdd(&local_histogram[previous & 0xffu], run);
            if mode_has_g(params.mode) {
                atomicAdd(&local_histogram[256u + ((previous >> 8u) & 0xffu)], run);
            }
            if mode_has_b(params.mode) {
                atomicAdd(&local_histogram[512u + ((previous >> 16u) & 0xffu)], run);
            }
            run = 0u;
        }
        previous = pixel;
        run = run + 1u;
    }
    if run > 0u {
        atomicAdd(&local_histogram[previous & 0xffu], run);
        if mode_has_g(params.mode) {
            atomicAdd(&local_histogram[256u + ((previous >> 8u) & 0xffu)], run);
        }
        if mode_has_b(params.mode) {
            atomicAdd(&local_histogram[512u + ((previous >> 16u) & 0xffu)], run);
        }
    }
    workgroupBarrier();

    let red = atomicLoad(&local_histogram[lid.x]);
    if red > 0u { atomicAdd(&histogram[lid.x], red); }
    if mode_has_g(params.mode) {
        let green = atomicLoad(&local_histogram[256u + lid.x]);
        if green > 0u { atomicAdd(&histogram[256u + lid.x], green); }
    }
    if mode_has_b(params.mode) {
        let blue = atomicLoad(&local_histogram[512u + lid.x]);
        if blue > 0u { atomicAdd(&histogram[512u + lid.x], blue); }
    }
}
