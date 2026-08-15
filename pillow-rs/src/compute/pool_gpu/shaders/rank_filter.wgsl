// Rank filter: window k-th element for each channel independently.
// Mode-aware: for L/LA (0/1) only computes rank on R channel (luma).
// CPU reference (image.rs:2785): rank_filter_impl(img, size, rank)
// For each pixel: load size×size window, sort per-channel values,
// output element at given rank index.
//
// Max window: 9×9 = 81 elements. Uses per-thread insertion sort.
// Border pixels: clamp source coordinates to image bounds (matching PIL).
// Pixel format: packed u32 RGBA.

struct Params {
    width: u32,
    height: u32,
    mode: u32,    // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    size: u32,
    rank: u32,
}

const MAX_WINDOW: u32 = 81u;

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

// Insertion sort on a fixed-size array (only first `len` elements are meaningful)
fn sort_arr(arr: ptr<function, array<u32, MAX_WINDOW>>, len: u32) {
    for (var i = 1u; i < len; i++) {
        var j = i;
        while j > 0u && (*arr)[j] < (*arr)[j - 1u] {
            let tmp = (*arr)[j];
            (*arr)[j] = (*arr)[j - 1u];
            (*arr)[j - 1u] = tmp;
            if j == 0u { break; }
            j = j - 1u;
        }
    }
}

fn rank_pixel(x: u32, y: u32) -> u32 {
    let w = params.width;
    let h = params.height;
    let size = min(params.size, 9u);
    let idx = y * w + x;
    if size == 0u || size % 2u == 0u {
        return input[idx];
    }
    let rank = params.rank;
    let half = i32(size) / 2i;
    let area = size * size;

    var chan_r: array<u32, MAX_WINDOW>;
    var chan_g: array<u32, MAX_WINDOW>;
    var chan_b: array<u32, MAX_WINDOW>;
    var chan_a: array<u32, MAX_WINDOW>;

    var n: u32 = 0u;
    let y_i32 = i32(y);
    let x_i32 = i32(x);
    let w_i32 = i32(w);
    let h_i32 = i32(h);

    // Load window elements (clamped to image bounds, matching PIL)
    for (var dy = -half; dy <= half; dy++) {
        let sy = clamp(y_i32 + dy, 0, h_i32 - 1);
        for (var dx = -half; dx <= half; dx++) {
            let sx = clamp(x_i32 + dx, 0, w_i32 - 1);
            let sample = input[u32(sy) * w + u32(sx)];
            chan_r[n] = sample & 0xffu;
            chan_g[n] = (sample >> 8u) & 0xffu;
            chan_b[n] = (sample >> 16u) & 0xffu;
            chan_a[n] = (sample >> 24u) & 0xffu;
            n++;
        }
    }

    // Clamp rank to valid range [0, area-1]
    let r = min(rank, area - 1u);

    // Sort each channel independently
    sort_arr(&chan_r, area);
    sort_arr(&chan_g, area);
    sort_arr(&chan_b, area);
    sort_arr(&chan_a, area);

    // Mode-aware output: for L/LA modes, only R is computed; G/B/A preserved from input
    let in_pixel = input[y * w + x];
    let in_g = (in_pixel >> 8u) & 0xffu;
    let in_b = (in_pixel >> 16u) & 0xffu;
    let in_a = (in_pixel >> 24u) & 0xffu;

    let out_r = chan_r[r];
    let out_g = select(in_g, chan_g[r], mode_has_g(params.mode));
    let out_b = select(in_b, chan_b[r], mode_has_b(params.mode));
    let out_a = select(255u, chan_a[r], mode_has_a(params.mode));

    return out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }
    let idx = gid.y * params.width + gid.x;
    output[idx] = rank_pixel(gid.x, gid.y);
}
