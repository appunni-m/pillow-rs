// Mandelbrot set fractal generation — Research §3.
// Per-pixel compute: z_{n+1} = z_n^2 + c, bailout at |z|^2 > 4.
// Each pixel is completely independent — embarrassingly parallel.

struct Params {
    width: u32,
    height: u32,
    max_iters: u32,
    // Screen-to-complex-plane mapping: extent [x0,y0] → [x1,y1]
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
}

@group(0) @binding(0) var<storage, read_write> output: array<u32>;
@group(0) @binding(1) var<uniform> params: Params;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    // Map pixel to complex plane (Research §3.3)
    let cx = params.x0 + (f32(gid.x) / f32(params.width)) * (params.x1 - params.x0);
    let cy = params.y0 + (f32(gid.y) / f32(params.height)) * (params.y1 - params.y0);

    // Mandelbrot iteration: z = z^2 + c (Research §3.1)
    var zx: f32 = 0.0;
    var zy: f32 = 0.0;
    var iter: u32 = 0u;
    for (; iter < params.max_iters; iter++) {
        let zx2 = zx * zx;
        let zy2 = zy * zy;
        if (zx2 + zy2 > 4.0) { break; }
        zy = 2.0 * zx * zy + cy;
        zx = zx2 - zy2 + cx;
    }

    // Output grayscale (Research §3.4)
    let t = f32(iter) / f32(params.max_iters);
    let value = u32(t * 255.0);
    let idx = gid.y * params.width + gid.x;
    output[idx] = value | (value << 8u) | (value << 16u) | 0xff000000u;
}
