// Effect noise: additive Gaussian noise using Box-Muller transform.
// For each pixel, generates two independent uniform random values from
// a deterministic Wang hash seeded by pixel position, then transforms
// to a Gaussian deviate via Box-Muller:
//   z0 = sqrt(-2*ln(u1)) * cos(2*PI*u2)
//   out = clamp(ch + sigma * z0, 0, 255)   (PIL uses truncation, not rounding)
//
// sigma_bits is the f32 bit pattern of sigma (passed as u32 for uniform layout).
// Use: let sigma = bitcast<f32>(params.sigma_bits);
// Mode-aware: only processes channels present in the image mode.
// Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA
// Packed u32 RGBA: byte0=R, byte1=G, byte2=B, byte3=A
// Per-pixel dispatch (16x16 workgroups).

const PI2: f32 = 6.283185307179586; // 2 * PI

struct Params {
    width: u32,
    height: u32,
    mode: u32,       // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    sigma_bits: u32, // bit pattern of f32 sigma
    seed: u32,       // deterministic seed
}

// ── Mode helpers ──

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

// Generate a Gaussian noise sample N(0, sigma) for a given pixel position.
// Uses Box-Muller on two uniform values from Wang hash.
fn gaussian_noise(x: u32, y: u32, w: u32, seed: u32, sigma: f32) -> f32 {
    // Two independent hashes for the two uniform values
    let pixel_seed = x + y * w + seed;
    let h1 = wang_hash(pixel_seed);
    let h2 = wang_hash(h1 + 0x9e3779b9u);

    // Convert to f32 in (0, 1] range.
    // Use 23 mantissa bits for precision; clamp away from 0 to avoid log(0).
    let u1 = max(f32(h1 & 0x007FFFFFu) / f32(0x00800000u), 0.0000001);
    let u2 = f32(h2 & 0x007FFFFFu) / f32(0x00800000u);

    // Box-Muller: z0 ~ N(0,1)
    let r = sqrt(-2.0 * log(u1));
    let theta = PI2 * u2;
    let z0 = r * cos(theta);

    return sigma * z0;
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let idx = gid.y * params.width + gid.x;
    let pixel = input[idx];
    let r = pixel & 0xffu;
    let g = (pixel >> 8u) & 0xffu;
    let b = (pixel >> 16u) & 0xffu;
    let a = (pixel >> 24u) & 0xffu;

    let sigma = bitcast<f32>(params.sigma_bits);

    // Generate noise for this pixel
    let noise = gaussian_noise(gid.x, gid.y, params.width, params.seed, sigma);

    // Add noise to each active channel. PIL uses truncation (WGSL u32 cast truncates).
    // R channel is always present.
    let out_r = u32(clamp(f32(r) + noise, 0.0, 255.0));
    let out_g = select(g, u32(clamp(f32(g) + noise, 0.0, 255.0)), mode_has_g(params.mode));
    let out_b = select(b, u32(clamp(f32(b) + noise, 0.0, 255.0)), mode_has_b(params.mode));
    let out_a = select(255u, a, mode_has_a(params.mode));

    output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}
