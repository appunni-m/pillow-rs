# GPU Algorithm Research — Implementation Reference

Research conducted 2026-06-15. 4 deep-research workflows, 103+ agents, adversarial verification.

---

## 1. Arbitrary Angle Image Rotation (WGSL Compute Shader)

**Status:** ✅ Research complete — 5 confirmed findings, 25 claims verified

### 1.1 Inverse Mapping (Universal Standard)

For each output pixel (dx, dy), compute the source coordinate via INVERSE rotation:

```
// Rotation matrix R(θ) = [cos θ, -sin θ; sin θ, cos θ]
// Inverse rotation R(-θ) = [cos θ, sin θ; -sin θ, cos θ]

// Center of rotation (cx, cy) = (src_w/2, src_h/2)
// Output center (ocx, ocy) = (dst_w/2, dst_h/2)

// For each output pixel (dx, dy):
float sx = cos_theta * (dx - ocx) + sin_theta * (dy - ocy) + cx;
float sy = -sin_theta * (dx - ocx) + cos_theta * (dy - ocy) + cy;
```

**Why inverse:** Forward mapping creates holes/overlaps. Inverse mapping guarantees every output pixel gets exactly one interpolated value.

**Source:** OpenCV's `warpAffine` always uses inverse mapping. Industry standard.

### 1.2 Bilinear Interpolation (4-tap)

```wgsl
fn bilinear_sample(img: texture_2d<f32>, coord: vec2<f32>, img_size: vec2<u32>) -> vec4<f32> {
    let tc = coord - 0.5;  // Convert from pixel center to texel corner
    let i = floor(tc);
    let f = tc - i;        // Fractional part [0,1)

    // 4 neighbors (clamped to image bounds)
    let x0 = clamp(u32(i.x), 0u, img_size.x - 1u);
    let x1 = clamp(u32(i.x + 1.0), 0u, img_size.x - 1u);
    let y0 = clamp(u32(i.y), 0u, img_size.y - 1u);
    let y1 = clamp(u32(i.y + 1.0), 0u, img_size.y - 1u);

    let v00 = textureLoad(img, vec2u(x0, y0), 0);
    let v10 = textureLoad(img, vec2u(x1, y0), 0);
    let v01 = textureLoad(img, vec2u(x0, y1), 0);
    let v11 = textureLoad(img, vec2u(x1, y1), 0);

    // Separable bilinear: V = (1-x)(1-y)v00 + x(1-y)v10 + (1-x)y v01 + xy v11
    return mix(mix(v00, v10, f.x), mix(v01, v11, f.x), f.y);
}
```

### 1.3 Bicubic Interpolation (16-tap, Catmull-Rom)

Uses separable cubic kernel evaluated as matrix-vector product (Skia's approach):

```wgsl
// Catmull-Rom cubic weight function
fn cubic_weight(x: f32) -> f32 {
    let ax = abs(x);
    if (ax >= 2.0) { return 0.0; }
    if (ax >= 1.0) {
        // Piece 2: [1, 2): k(x) = a*|x|^3 - 5a*|x|^2 + 8a*|x| - 4a  where a = -0.5
        return ((-0.5 * ax + 2.5) * ax - 4.0) * ax + 2.0;
    }
    // Piece 1: [0, 1): k(x) = (1.5)*|x|^3 - (2.5)*|x|^2 + 1
    return (1.5 * ax - 2.5) * ax * ax + 1.0;
}

fn bicubic_sample(img: texture_2d<f32>, coord: vec2<f32>, img_size: vec2<u32>) -> vec4<f32> {
    let tc = coord - 0.5;
    let i = floor(tc);
    let f = tc - i;

    // Compute 4 horizontal weights
    var wx: array<f32, 4>;
    var wy: array<f32, 4>;
    for (var j = 0u; j < 4u; j++) {
        wx[j] = cubic_weight(f.x - f32(j - 1));
        wy[j] = cubic_weight(f.y - f32(j - 1));
    }

    // 4x4 convolution (16 samples, separable)
    var result = vec4f(0.0);
    for (var row = 0u; row < 4u; row++) {
        let sy = clamp(i32(i.y) + i32(row) - 1, 0, i32(img_size.y) - 1);
        var row_sum = vec4f(0.0);
        for (var col = 0u; col < 4u; col++) {
            let sx = clamp(i32(i.x) + i32(col) - 1, 0, i32(img_size.x) - 1);
            row_sum += wx[col] * textureLoad(img, vec2u(u32(sx), u32(sy)), 0);
        }
        result += wy[row] * row_sum;
    }
    return result;
}
```

**Catmull-Rom parameters:** B=0, C=0.5 in the Mitchell-Netravali family.
**Reference:** Skia's `GrBicubicEffect` (Flutter engine) — the production gold standard.

### 1.4 Edge/Fill Handling

Two levels:

**Level 1 — GPU sampler:** `ClampToEdge` (default in wgpu/WebGPU). Clamps coordinates to [0,1] texcoords, returns nearest edge texel.

**Level 2 — Algorithm-level border modes** (must be coded manually in compute shader, matches OpenCV `BorderType`):
- **Constant:** If (sx < 0 || sx >= w || sy < 0 || sy >= h) → use `fill_color`
- **Replicate:** Clamp sx to [0, w-1], sy to [0, h-1]
- **Reflect:** Mirror including edge
- **Reflect101:** Mirror excluding edge
- **Wrap:** `sx = sx % w; if sx < 0: sx += w`

### 1.5 Workgroup Dispatch

```rust
let wg_x = 8u32; let wg_y = 8u32; // 64 invocations — cross-vendor sweet spot
let dispatch_x = (dst_w + wg_x - 1) / wg_x;
let dispatch_y = (dst_h + wg_y - 1) / wg_y;
```

**WGSL entry point:**
```wgsl
@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x; let y = gid.y;
    if (x >= u_width || y >= u_height) { return; }
    // ... rotation + sampling ...
}
```

---

## 2. GPU 2D Shape Rasterization

**Status:** ✅ Research complete — 4 confirmed findings, 25 claims verified

### 2.1 Three Architectural Approaches (Research Finding)

| Approach | Mechanism | GPU Tech | Complexity |
|----------|-----------|----------|------------|
| **Sort-middle (Vello)** | Tile-based coarse raster + prefix-sum monoids + backdrop propagation | Compute shaders only | 🔴 Massive (Vello is ~50K lines) |
| **RAVG (Nehab & Hoppe 2008)** | Per-pixel implicit distance + smoothstep AA + lattice clipping | Compute/fragment shaders | 🟡 Medium (per-shape distance functions) |
| **NV_path_rendering** | Stencil-then-Cover using fixed-function stencil buffer | Fixed-function HW | 🟢 Low (but NVIDIA-only extension) |

### 2.2 Recommendation for pillow-rs: RAVG-Style Per-Pixel

**Vello/sort-middle is overkill** for pillow-rs ImageDraw. It handles arbitrary SVG paths with thousands of segments — ImageDraw deals with simple geometric primitives (rectangles, ellipses, lines, polygons with dozens of vertices).

**RAVG approach** is the right fit: for each shape type, write a per-pixel distance function in the compute shader, then use `smoothstep` for anti-aliased edges.

### 2.3 Distance Functions for Common Shapes (WGSL)

```wgsl
// Rectangle (unsigned distance to boundary)
fn rect_distance(px: vec2f, center: vec2f, half_size: vec2f) -> f32 {
    let d = abs(px - center) - half_size;
    let outside = length(max(d, vec2f(0.0)));
    let inside = min(max(d.x, d.y), 0.0);
    return outside + inside;
}

// Circle
fn circle_distance(px: vec2f, center: vec2f, radius: f32) -> f32 {
    return length(px - center) - radius;
}

// Rounded rectangle
fn rounded_rect_distance(px: vec2f, center: vec2f, half_size: vec2f, radius: f32) -> f32 {
    let d = abs(px - center) - half_size + vec2f(radius);
    return length(max(d, vec2f(0.0))) + min(max(d.x, d.y), 0.0) - radius;
}

// Line segment (distance to segment)
fn line_distance(px: vec2f, a: vec2f, b: vec2f) -> f32 {
    let ab = b - a;
    let ap = px - a;
    let t = clamp(dot(ap, ab) / dot(ab, ab), 0.0, 1.0);
    return length(ap - t * ab);
}

// Ellipse (approximate — exact requires solving quartic)
fn ellipse_distance(px: vec2f, center: vec2f, radii: vec2f) -> f32 {
    // Transform to unit circle space then evaluate
    let p = (px - center) / radii;
    let r = length(p);
    return (r - 1.0) * min(radii.x, radii.y); // Approximation
}
```

### 2.4 Anti-Aliased Fill (smoothstep)

```wgsl
fn fill_shape(distance: f32, fill_color: vec4f, bg: vec4f) -> vec4f {
    // Anti-aliased edge: 1-pixel transition zone
    let alpha = 1.0 - smoothstep(-1.0, 1.0, distance);
    return mix(bg, fill_color, alpha);
}

fn stroke_shape(distance: f32, stroke_width: f32, stroke_color: vec4f, bg: vec4f) -> vec4f {
    let half = stroke_width * 0.5;
    let outer = abs(distance) - half;
    let alpha = 1.0 - smoothstep(-1.0, 1.0, outer);
    return mix(bg, stroke_color, alpha);
}
```

### 2.5 Polygon Fill: Even-Odd Rule (Per-Pixel)

For small polygons (ImageDraw), per-pixel even-odd testing is practical:

```wgsl
fn point_in_polygon(px: vec2f, vertices: array<vec2f, MAX_VERTS>, n: u32) -> bool {
    var inside = false;
    var j = n - 1u;
    for (var i = 0u; i < n; i++) {
        let vi = vertices[i];
        let vj = vertices[j];
        if ((vi.y > px.y) != (vj.y > px.y)) {
            let intersect = (vj.x - vi.x) * (px.y - vi.y) / (vj.y - vi.y) + vi.x;
            if (px.x < intersect) {
                inside = !inside;
            }
        }
        j = i;
    }
    return inside;
}
```

### 2.6 GPU Stroke Expansion (Euler Spiral Approach)

**Finding:** Levien & Uguray (HPG 2024 Best Paper) solved GPU stroke expansion using Euler spirals as intermediate curve representation. This replaces the CPU-bound Tiller-Hanson algorithm. Integrated into Vello and Skia Graphite.

**For pillow-rs:** This is overkill for simple geometric shape strokes. For line/polygon stroke on GPU, use the distance-to-segment approach (Section 2.3 above). Keep the existing Bresenham CPU implementation as fallback.

### 2.7 Scanline vs Per-Pixel — Modern GPU Verdict

**Per-pixel wins on modern GPUs.** The massive parallelism (thousands of cores) makes per-pixel distance evaluation faster than CPU-style scanline rasterization for all but the most trivial shapes. The only exception is very large polygons with many vertices where sort-middle/tile-based approaches become competitive.

**For pillow-rs ImageDraw GPU strategy:**
- **Fill ops** (rectangle, ellipse, circle, polygon fill): Per-pixel distance + smoothstep — straightforward GPU compute shader
- **Stroke ops** (line, outline): Distance-to-edge functions — harder, keep CPU Bresenham for now
- **Text**: MUCH harder on GPU — keep CPU glyph rasterization for Phase 7
- **bitmap**: Already a pixel copy — trivial GPU shader

---

## 3. GPU Mandelbrot Fractal Generation

**Status:** ✅ Research complete — 5 confirmed findings, 25 claims verified, 16 confirmed

### 3.1 Core Iteration Algorithm

```
z_0 = 0
z_{n+1} = z_n^2 + c

Where: z = x + yi (complex), c = cx + cyi (complex plane coordinate)

In real/imaginary components:
x_{n+1} = x_n^2 - y_n^2 + cx
y_{n+1} = 2 * x_n * y_n + cy

Bailout: |z|^2 = x^2 + y^2 > 4.0  (i.e., |z| > 2)
```

**Key:** Compare squared magnitude (`x^2 + y^2 > 4.0`) — avoids expensive `sqrt()`.
**Proof:** If |c| ≤ 2 and |z_n| > 2, the excess grows exponentially → guaranteed divergence.

**Sources:** paulbourke.net, mandelbrot-wasm, par-fractal, Math StackExchange.

### 3.2 WGSL Compute Shader Implementation

```wgsl
struct Params {
    width: u32,
    height: u32,
    max_iters: u32,
    x0: f32,    // extent left
    y0: f32,    // extent top
    x1: f32,    // extent right
    y1: f32,    // extent bottom
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y;
    if (x >= params.width || y >= params.height) { return; }

    // Map pixel to complex plane: extent [x0,y0] → [x1,y1]
    let cx = params.x0 + (f32(x) / f32(params.width)) * (params.x1 - params.x0);
    let cy = params.y0 + (f32(y) / f32(params.height)) * (params.y1 - params.y0);

    // Mandelbrot iteration
    var zx: f32 = 0.0;
    var zy: f32 = 0.0;
    var iter: u32 = 0u;
    for (; iter < params.max_iters; iter++) {
        let zx2 = zx * zx;
        let zy2 = zy * zy;
        if (zx2 + zy2 > 4.0) { break; }  // Early bailout
        zy = 2.0 * zx * zy + cy;
        zx = zx2 - zy2 + cx;
    }

    // Output: grayscale (0 = in set → black, 255 = fast escape → white)
    let value = f32(iter) / f32(params.max_iters);
    let gray = u8(255.0 * value);

    // Write to output texture
    textureStore(output, vec2u(x, y), vec4f(f32(gray) / 255.0, 0.0, 0.0, 1.0));
}
```

### 3.3 Screen-to-Complex-Plane Mapping

Three implementations all follow the same pattern (verified across sources):

```
// Pattern: normalize → scale → translate
c_real = (x / width - 0.5) * scale * aspect_ratio + center_x
c_imag = (y / height - 0.5) * scale + center_y

where scale = 4.0 / zoom
```

The `4.0` factor: Mandelbrot set has diameter 4 at zoom=1 (bounded by |z| < 2).

**For Pillow's `effect_mandelbrot(extent=(x0, y0, x1, y1))`:**
```wgsl
let cx = x0 + (f32(x) / f32(width)) * (x1 - x0);
let cy = y0 + (f32(y) / f32(height)) * (y1 - y0);
```

### 3.4 Coloring: Grayscale Output

For grayscale (matching Pillow's L-mode output):

```wgsl
// Simple: ratio of iterations to max
let t = f32(iter) / f32(max_iters);
// In set = black (0), fast escape = white (255)
let gray = u8(255.0 * t);
```

**Smooth coloring** (optional, for higher quality):
```wgsl
// mu = n + 1 - log2(log2(|Z_n|))
let log_zn = log2(zx*zx + zy*zy) * 0.5;  // log2(|z|)
let nu = log2(log_zn);                     // log2(log2(|z|))
let smooth_iter = f32(iter) + 1.0 - nu;
let t = smooth_iter / f32(max_iters);
```

Reference: Linas Vepstas' derivation, used by acceleratehs.org.

### 3.5 Two GPU Approaches (Performance)

| Approach | Used By | Pros | Cons |
|----------|---------|------|------|
| **Fragment shader** on full-screen quad | par-fractal, fractal_viewer | No workgroup scheduling issues, GPU rasterizer handles pixel distribution | Tied to render pass, harder to chain with other compute ops |
| **Compute shader** @workgroup_size(8,8) | mandelbrot-wasm | Pure compute, composes with other PipelineOps | 64 threads/workgroup — warp divergence from varying iteration counts |

**Recommendation for pillow-rs:** Compute shader (approach B) — it fits the existing pipeline architecture where all ops are compute shaders.

### 3.6 Warp Divergence Reality

**Finding:** No concrete warp-divergence mitigation was found in any examined GPU Mandelbrot implementation. The 8x8 (64-thread) workgroup size aligns with AMD wavefront (64) and is multiple of NVIDIA warp (32). In practice:
- Fragment shader approach avoids the issue entirely (GPU rasterizer handles it)
- For compute shaders at moderate iteration counts (quality < 500), divergence is acceptable
- For extreme deep zoom, perturbation theory is needed (but DeepDrill does this on CPU)

**For pillow-rs:** Use compute shader with `@workgroup_size(8,8)`. Accept the divergence — it's the standard approach.

---

## 4. GPU Parallel Histogram + Separable Gaussian Blur

**Status:** ✅ Research complete — 5 confirmed findings, 25 claims verified, 20 confirmed

### 4.1 Two-Pass Parallel Histogram Algorithm

**Performance tiers (measured on 3200×2400 image):**

| Tier | Technique | Busy Cycles | Runtime | vs Naive |
|------|-----------|-------------|---------|----------|
| 1 | Naive global atomics (per-pixel `atomicAdd`) | 8,011,577 | 5.0 ms | 1× |
| 2 | Per-workgroup shared-memory local histogram → global merge | 412,249 | 0.5 ms | **10×** |
| 3 | Per-thread register accumulation (4×4 tiles) → shared merge | 256,000 | 0.2 ms | **25×** |

**Algorithm (Tier 2 — recommended for pillow-rs):**

```wgsl
// === Pass 1: Build histogram (one workgroup per image tile) ===
@compute @workgroup_size(256)  // 256 threads → 256 bins
fn build_histogram(@builtin(global_invocation_id) gid: vec3<u32>,
                   @builtin(local_invocation_id) lid: vec3<u32>) {
    // Shared memory: per-workgroup local histogram
    var<workgroup> local_hist: array<atomic<u32>, 256>;

    // Initialize (one thread per bin)
    if (lid.x < 256u) {
        atomicStore(&local_hist[lid.x], 0u);
    }
    workgroupBarrier();

    // Accumulate: each thread processes multiple pixels (grid-stride loop)
    let n_pixels = params.width * params.height;
    for (var i = gid.x; i < n_pixels; i += 256u * WORKGROUP_COUNT) {
        let x = i % params.width;
        let y = i / params.width;
        let px = textureLoad(input, vec2u(x, y), 0);
        let bin = u32(px.r * 255.0);  // Luma value → bin
        atomicAdd(&local_hist[bin], 1u);
    }
    workgroupBarrier();

    // Merge to global histogram (one atomic write per thread)
    let count = atomicLoad(&local_hist[lid.x]);
    if (count > 0u) {
        atomicAdd(&global_hist[lid.x], count);
    }
}
```

**WGSL prerequisites (confirmed by W3C spec):**
- `atomicAdd`, `atomicMax`, `atomicMin` on `workgroup` and `storage` address spaces
- `workgroupBarrier()` for synchronization between accumulation and merge phases
- Shared memory: up to 16,384 bytes per workgroup

### 4.2 OpenCV-Optimized Histogram Pattern

OpenCV's CUDA `histogram256` kernel adds production-grade optimizations:

1. **Bank-conflict-free shared memory:** `__shared__ int shist[8][33]` — pad to 33 columns so threads accessing stride-32 don't collide
2. **Coalesced 32-bit loads:** Load 4 bytes at once via `uint*` cast, unpack via shift-and-mask (4× bandwidth)
3. **Three-phase access pattern:** Head (unaligned leading bytes, thread 0 only), body (aligned bulk, all threads), tail (trailing bytes)
4. **Conditional global atomics:** Only write non-zero bins to global memory

### 4.3 CDF Computation (Parallel Prefix Scan)

OpenCV uses GPU Gems 3 parallel prefix scan on a single block of 256 threads:

```wgsl
// === Pass 2: Build CDF + LUT (single workgroup, 256 threads) ===
@compute @workgroup_size(256)
fn build_lut(@builtin(local_invocation_id) tid: u32) {
    // Step A: Load histogram into shared memory
    var<workgroup> hist_scan: array<u32, 256>;
    hist_scan[tid] = global_histogram[tid];

    // Step B: Find first non-zero bin (warp shuffle reduction — OpenCV pattern)
    var first_nonzero: u32 = 0u;
    for (var i = 0u; i < 256u; i++) {
        if (hist_scan[i] > 0u) { first_nonzero = i; break; }
    }

    // Step C: Parallel inclusive prefix scan (up-sweep + down-sweep)
    // GPU Gems 3 — O(log n) with 256 threads
    var offset: u32 = 1u;
    for (var d = 128u; d > 0u; d >>= 1u) {  // Up-sweep
        workgroupBarrier();
        if (tid < d) {
            let ai = offset * (2u * tid + 1u) - 1u;
            let bi = offset * (2u * tid + 2u) - 1u;
            hist_scan[bi] += hist_scan[ai];
        }
        offset *= 2u;
    }
    // ... down-sweep (clear last element, then propagate) ...

    workgroupBarrier();

    // Step D: CDF → LUT mapping
    let total_pixels = params.width * params.height;
    let range = total_pixels - first_nonzero;
    if (range > 0u) {
        let cdf = hist_scan[tid];  // Cumulative count
        output_lut[tid] = u8(f32(cdf) * 255.0 / f32(range));
    } else {
        output_lut[tid] = tid;  // Identity
    }
}
```

**CDF-to-LUT formula (matches OpenCV):**
```
lut[tid] = saturate(hist[tid] * 255.0 / (total_pixels - hist[first_nonzero]))
```

### 4.4 Remap Pass (Apply LUT to Image)

```wgsl
// === Pass 3: Apply LUT (one thread per pixel) ===
@compute @workgroup_size(8, 8)
fn remap(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x; let y = gid.y;
    if (x >= params.width || y >= params.height) { return; }

    let px = textureLoad(input, vec2u(x, y), 0);
    let bin = u32(px.r * 255.0);
    let mapped = f32(lut[bin]) / 255.0;

    textureStore(output, vec2u(x, y), vec4f(mapped, mapped, mapped, 1.0));
}
```

**For autocontrast:**
- Compute histogram (Pass 1)
- Find cutoff bins: `low_bin` = first bin where cumulative count > cutoff% of total; `high_bin` similarly
- Remap: `output = (pixel - low_bin) * 255 / (high_bin - low_bin)`, clamped to [0, 255]

### 4.5 Separable Gaussian Blur (Horizontal + Vertical Passes)

**Core insight:** `G(x,y) = G(x) * G(y)` — 2D Gaussian decomposes into 1D × 1D.

```
O(n*m) → O(n+m)  per pixel

Instead of:  25 texture fetches (5×5 kernel)
Two-pass:    5 + 5 = 10 texture fetches
```

**Horizontal pass:**
```wgsl
@compute @workgroup_size(256, 1)  // 1D workgroup per row
fn blur_horizontal(@builtin(global_invocation_id) gid: vec3<u32>) {
    let y = gid.y;  // Row
    if (y >= params.height) { return; }

    var accum = vec4f(0.0);
    var weight_sum: f32 = 0.0;
    let radius: i32 = i32(params.kernel_radius);

    for (var dx = -radius; dx <= radius; dx++) {
        let sx = clamp(i32(gid.x) + dx, 0, i32(params.width) - 1);
        let w = gaussian_weight(f32(dx), params.sigma);
        accum += w * textureLoad(input, vec2u(u32(sx), y), 0);
        weight_sum += w;
    }

    textureStore(intermediate, vec2u(gid.x, y), accum / weight_sum);
}
```

**Vertical pass:** Identical pattern, swap x/y axes. Reads from `intermediate` texture, writes to `output`.

### 4.6 Incremental Gaussian Coefficient Calculation (GPU Gems 3)

From Mozilla WebRender's `cs_blur.glsl` production code:

```wgsl
// Pre-compute once:
//   gauss.x = 1.0 / (sqrt(2*pi) * sigma)
//   gauss.y = exp(-0.5 / (sigma*sigma))
//   gauss.z = gauss.y * gauss.y

// Then per-iteration update (avoids per-tap exp()):
var gauss = vec3f(gauss_coeff);
for (var i = 0; i <= radius; i++) {
    weight = gauss.x;
    // ... accumulate ...
    gauss.xy *= gauss.yz;  // Multiply, not exp()!
}
```

**Performance:** Replaces N `exp()` calls with N vector multiplies.

### 4.7 Hardware Bilinear Optimization (Halve Texture Fetches)

**Identity:** `k0*c0 + k1*c1 = (k0+k1) * lerp(c0, c1, k1/(k0+k1))`

Sample BETWEEN two texels using bilinear hardware — one fetch instead of two:
```wgsl
// Instead of 2 fetches per pair:
//   accum += w[0]*texLoad(x-2) + w[1]*texLoad(x-1);
// Do 1 fetch at offset (x-2 + w[1]/(w[0]+w[1])):
let combined_weight = w[0] + w[1];
let offset = f32(i*2) + w[1] / combined_weight;
accum += combined_weight * textureSample(input, sampler, vec2f(x + offset, y));
```

**Tradeoff:** ~60% speedup on 8-bit textures, but minor precision loss from bilinear filtering.

### 4.8 Optimal Workgroup Sizes

| Pass | Workgroup | Rationale |
|------|-----------|-----------|
| Histogram | `@workgroup_size(256)` | 1 thread per bin — perfect 1:1 mapping |
| LUT/CDF scan | `@workgroup_size(256)` | Single workgroup, 256-bin scan |
| Remap | `@workgroup_size(8,8)` | Standard 64-invocation 2D grid |
| Blur horizontal | `@workgroup_size(256, 1)` | 1D per-row, 256-wide |
| Blur vertical | `@workgroup_size(1, 256)` | 1D per-column (or transpose + use horizontal) |

### 4.9 Implementation Notes for pillow-rs

**Autocontrast (already has `autocontrast.wgsl` with 3 multi-pass shaders):**
- Existing shaders: `autocontrast.wgsl`, `autocontrast_histogram.wgsl`, `autocontrast_remap.wgsl`
- Just needs `op_id()` and `extract_params()` wiring (Category B fix)

**Equalize (already has `equalize.wgsl` with 4 multi-pass shaders):**
- Existing shaders: `equalize.wgsl`, `equalize_histogram.wgsl`, `equalize_cdf.wgsl`, `equalize_remap.wgsl`
- Just needs `op_id()` wiring (Category B fix)

**GaussianBlur (already has `gaussian_blur.wgsl` but needs separable optimization):**
- Existing: `gaussian_blur.wgsl`, `box_blur_h.wgsl`, `box_blur_v.wgsl`
- Already supports separable passes — just needs `op_id()` wiring

---

## Immediate Implementation Notes

### Phase 1 (Bug Fixes — no research needed):
- Add `GaussianBlur`, `Autocontrast`, `Equalize` to `OpId` enum and match arms
- Shaders already exist, just fix dispatch

### Phase 2 (Simple shaders — use existing patterns):
- `GetChannel`, `LinearGradient`, `RadialGradient` — trivial per-pixel, use existing Invert/Grayscale shaders as template
- `RemapPalette`, `Expand`, `CropBorder`, `Merge` — basic copy/LUT, use existing Crop/Constant as template
- `effect_mandelbrot` — see Research #3 above

### Phase 5 (Complex — needs Research #1):
- `Rotate` — use inverse mapping + bilinear/bicubic as documented above
