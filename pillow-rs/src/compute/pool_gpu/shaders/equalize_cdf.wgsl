// Equalize LUT derivation using a workgroup-wide integer prefix scan.
// Pillow maps each bin with the exclusive population plus half a step, where
// step = (total - last_nonzero_count) / 255. Keep counts and rounding integral;
// a zero step preserves the identity LUT. The remap remains in point_op.wgsl.

struct Params { width: u32, height: u32, mode: u32, _pad: u32 }
@group(0) @binding(0) var<storage, read> histogram_data: array<u32>;
@group(0) @binding(1) var<storage, read_write> lut: array<u32, 256>;
@group(0) @binding(2) var<uniform> params: Params;
var<workgroup> prefix: array<vec3<u32>, 256>;
var<workgroup> last_index: array<atomic<u32>, 3>;
fn map_value(value: u32, sum: u32, step: u32) -> u32 {
    if step == 0u { return value; }
    return min((sum + step / 2u) / step, 255u);
}
@compute @workgroup_size(256)
fn main(@builtin(local_invocation_id) lid: vec3<u32>) {
    let i = lid.x;
    let rgb = params.mode >= 2u;
    let counts = vec3<u32>(histogram_data[i], histogram_data[256u + i], histogram_data[512u + i]);
    prefix[i] = counts;
    if i < 3u { atomicStore(&last_index[i], 0u); }
    workgroupBarrier();
    if counts.x > 0u { atomicMax(&last_index[0], i); }
    if counts.y > 0u { atomicMax(&last_index[1], i); }
    if counts.z > 0u { atomicMax(&last_index[2], i); }
    // Each lane owns one bin. Finish every read before updating the shared
    // prefix, then publish all updates before the next scan level reads them.
    for (var offset = 1u; offset < 256u; offset = offset * 2u) {
        var add = vec3<u32>(0u);
        if i >= offset { add = prefix[i - offset]; }
        workgroupBarrier();
        prefix[i] = prefix[i] + add;
        workgroupBarrier();
    }
    // The final prefix is the selected population, including an empty mask.
    // Image dimensions would incorrectly count pixels excluded by the mask.
    let step = (prefix[255u] - vec3<u32>(
        histogram_data[atomicLoad(&last_index[0])],
        histogram_data[256u + atomicLoad(&last_index[1])],
        histogram_data[512u + atomicLoad(&last_index[2])])) / vec3<u32>(255u);
    // The scan is inclusive; subtract this bin before applying Pillow's
    // half-step rule. Including this bin shifts the equalized result.
    let exclusive = prefix[i] - counts;
    let r = map_value(i, exclusive.x, step.x);
    var g = i;
    var b = i;
    if rgb {
        g = map_value(i, exclusive.y, step.y);
        b = map_value(i, exclusive.z, step.z);
    }
    lut[i] = r | (g << 8u) | (b << 16u) | 0xff000000u;
}
