// Equalize histogram gather from native packed L storage.
//
// Four one-byte pixels share each input word. Read only the logical gray
// sample; do not widen L to RGBA merely to reuse the multichannel histogram.

struct Params {
    width: u32,
    height: u32,
    _mode: u32,
    _pad: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> histogram: array<atomic<u32>, 1024>;
@group(0) @binding(2) var<uniform> params: Params;

var<workgroup> local_histogram: array<atomic<u32>, 256>;

@compute @workgroup_size(256)
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(num_workgroups) groups: vec3<u32>,
) {
    atomicStore(&local_histogram[lid.x], 0u);
    workgroupBarrier();

    let total_pixels = params.width * params.height;
    let stride = groups.x * 256u;
    var previous = 0u;
    var run = 0u;
    for (var i = gid.x; i < total_pixels; i = i + stride) {
        let packed = input[i >> 2u];
        let shift = (i & 3u) * 8u;
        let pixel = (packed >> shift) & 0xffu;
        if run > 0u && pixel != previous {
            atomicAdd(&local_histogram[previous], run);
            run = 0u;
        }
        previous = pixel;
        run = run + 1u;
    }
    if run > 0u {
        atomicAdd(&local_histogram[previous], run);
    }
    workgroupBarrier();

    let count = atomicLoad(&local_histogram[lid.x]);
    if count > 0u {
        atomicAdd(&histogram[lid.x], count);
    }
}
