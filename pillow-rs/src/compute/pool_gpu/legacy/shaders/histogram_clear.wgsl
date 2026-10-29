// Histogram clear: zero the histogram/auxiliary storage buffer.
// 1 workgroup (256 threads x 4 iterations = 1024 u32 slots cleared).
//
// Utility shader: sets all values in the target storage buffer (binding 1)
// to zero. The backend uses this to reset the histogram buffer between
// multi-pass pipeline executions.
//
// Mode-aware: Params struct uses standard header for consistency, but mode
// is not used (clearing is mode-independent).
// Params: standard header (mode ignored).

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
}

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

@group(0) @binding(0) var<storage, read> _input: array<u32>;
@group(0) @binding(1) var<storage, read_write> histogram: array<u32>;
@group(0) @binding(2) var<uniform> _params: Params;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let tid = gid.x;

    // 256 threads each clear 4 slots = 1024 total
    // Binding 1 is declared as array<u32> with no explicit size,
    // the backend binds the full buffer.
    histogram[tid] = 0u;
    histogram[tid + 256u] = 0u;
    histogram[tid + 512u] = 0u;
    histogram[tid + 768u] = 0u;
}
