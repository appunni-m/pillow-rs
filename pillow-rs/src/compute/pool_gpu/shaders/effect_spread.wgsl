// Effect spread: random pixel displacement within a given distance.
// Mode-aware: for L/LA (0/1) only copies R channel from source position.
// Per-pixel dispatch (16x16 workgroups). Each output pixel gathers a new
// value from a randomly-chosen source location within distance.
//
// CPU reference (image.rs:2570): For each pixel:
//   xx = x + (rand() % distance) - distance/2
//   yy = y + (rand() % distance) - distance/2
//   Swap out[y][x] with out[yy][xx] (if yy,xx in bounds)
//
// GPU note: PIL uses libc rand() which is sequential and nondeterministic.
// This shader uses a deterministic Wang hash for reproducible pseudo-random
// offsets. Each thread independently gathers from its random source location,
// avoiding data races inherent in a true parallel swap.
//
// Params: distance (u32), seed (u32), mode

struct Params {
    width: u32,
    height: u32,
    mode: u32,    // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    distance: u32,
    seed: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

// Wang hash: fast deterministic integer hash.
fn wang_hash(seed: u32) -> u32 {
    var s = seed;
    s = (s ^ 61u) ^ (s >> 16u);
    s = s * 9u;
    s = s ^ (s >> 4u);
    s = s * 0x27d4eb2du;
    s = s ^ (s >> 15u);
    return s;
}

// Generate a deterministic pseudo-random u32 from pixel coordinates and a seed.
fn rand_u32(gid_x: u32, gid_y: u32, width: u32, seed: u32) -> u32 {
    let pixel_seed = gid_x + gid_y * width + seed;
    return wang_hash(pixel_seed);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let idx = gid.y * params.width + gid.x;

    if params.distance > 0u {
        let dist = params.distance;
        let r = rand_u32(gid.x, gid.y, params.width, params.seed);

        // Compute random offset within [-distance/2, distance/2)
        let dx = i32(r % dist) - i32(dist / 2u);
        let dy = i32((r >> 16u) % dist) - i32(dist / 2u);

        let sx = i32(gid.x) + dx;
        let sy = i32(gid.y) + dy;

        // Clamp to image bounds
        if sx >= 0 && sx < i32(params.width) && sy >= 0 && sy < i32(params.height) {
            let src_idx = u32(sy) * params.width + u32(sx);
            let src_pixel = input[src_idx];
            let src_r = src_pixel & 0xffu;
            let src_g = (src_pixel >> 8u) & 0xffu;
            let src_b = (src_pixel >> 16u) & 0xffu;
            let src_a = (src_pixel >> 24u) & 0xffu;

            // Mode-aware: for L/LA, only copy R channel; zero G/B in non-GB modes
            let out_r = src_r;
            let out_g = select(0u, src_g, mode_has_g(params.mode));
            let out_b = select(0u, src_b, mode_has_b(params.mode));
            let out_a = select(255u, src_a, mode_has_a(params.mode));

            output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
        } else {
            // Out of bounds: copy input pixel with mode-awareness
            let in_pixel = input[idx];
            let in_r = in_pixel & 0xffu;
            let in_g = (in_pixel >> 8u) & 0xffu;
            let in_b = (in_pixel >> 16u) & 0xffu;
            let in_a = (in_pixel >> 24u) & 0xffu;

            let out_r = in_r;
            let out_g = select(0u, in_g, mode_has_g(params.mode));
            let out_b = select(0u, in_b, mode_has_b(params.mode));
            let out_a = select(255u, in_a, mode_has_a(params.mode));

            output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
        }
    } else {
        // distance == 0: copy verbatim with mode-awareness
        let in_pixel = input[idx];
        let in_r = in_pixel & 0xffu;
        let in_g = (in_pixel >> 8u) & 0xffu;
        let in_b = (in_pixel >> 16u) & 0xffu;
        let in_a = (in_pixel >> 24u) & 0xffu;

        let out_r = in_r;
        let out_g = select(0u, in_g, mode_has_g(params.mode));
        let out_b = select(0u, in_b, mode_has_b(params.mode));
        let out_a = select(255u, in_a, mode_has_a(params.mode));

        output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
    }
}
