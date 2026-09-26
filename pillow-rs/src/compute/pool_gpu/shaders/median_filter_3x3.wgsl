// Exact 3x3 byte median using one packed four-channel sample per sort lane.
// Sorting component-wise keeps every channel's independent rank semantics,
// while a fixed nine-value network avoids the generic shader's 4 x 225-value
// private arrays and data-dependent insertion-sort branches.

struct Params {
    width: u32,
    height: u32,
    mode: u32, // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    size: u32,
}

const WINDOW: u32 = 9u;

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn unpack_channels(pixel: u32) -> vec4<u32> {
    return vec4<u32>(
        pixel & 0xffu,
        (pixel >> 8u) & 0xffu,
        (pixel >> 16u) & 0xffu,
        (pixel >> 24u) & 0xffu,
    );
}

fn mode_has_g(mode: u32) -> bool { return mode >= 2u; }
fn mode_has_b(mode: u32) -> bool { return mode >= 2u; }
fn mode_has_a(mode: u32) -> bool { return mode == 1u || mode == 3u; }

fn sort_nine_samples(values: ptr<function, array<vec4<u32>, WINDOW>>) {
    // Odd-even transposition sorts all nine scalar channel values in parallel
    // across the vector lanes. Nine fixed passes also define behavior for
    // duplicate byte values without data-dependent control flow.
    for (var phase = 0u; phase < WINDOW; phase++) {
        var left = phase & 1u;
        loop {
            if left + 1u >= WINDOW {
                break;
            }
            let lower = min((*values)[left], (*values)[left + 1u]);
            let upper = max((*values)[left], (*values)[left + 1u]);
            (*values)[left] = lower;
            (*values)[left + 1u] = upper;
            left += 2u;
        }
    }
}

fn median_pixel(x: u32, y: u32) -> u32 {
    let width = i32(params.width);
    let height = i32(params.height);
    let x_i32 = i32(x);
    let y_i32 = i32(y);
    var values: array<vec4<u32>, WINDOW>;
    var index = 0u;

    for (var dy = -1; dy <= 1; dy++) {
        let source_y = u32(clamp(y_i32 + dy, 0, height - 1));
        for (var dx = -1; dx <= 1; dx++) {
            let source_x = u32(clamp(x_i32 + dx, 0, width - 1));
            values[index] = unpack_channels(input[source_y * params.width + source_x]);
            index += 1u;
        }
    }

    sort_nine_samples(&values);
    let selected = values[4u];
    let input_pixel = input[y * params.width + x];
    let input_g = (input_pixel >> 8u) & 0xffu;
    let input_b = (input_pixel >> 16u) & 0xffu;
    let input_a = (input_pixel >> 24u) & 0xffu;

    let out_r = selected.x;
    let out_g = select(input_g, selected.y, mode_has_g(params.mode));
    let out_b = select(input_b, selected.z, mode_has_b(params.mode));
    let out_a = select(255u, selected.w, mode_has_a(params.mode));
    return out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }
    output[gid.y * params.width + gid.x] = median_pixel(gid.x, gid.y);
}
