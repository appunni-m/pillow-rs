# Shader Migration Guide — Subagent Prompt Templates

Use these templates when dispatching parallel subagents to migrate shader batches.
Each template includes the exact shader pattern to follow and category-specific instructions.

## Standard Pattern (invert.wgsl — canonical reference)

```wgsl
// Operation: description of what the shader does.
// Mode-aware: only processes channels present in the image mode.
// Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA
// Packed u32 RGBA: byte0=R, byte1=G, byte2=B, byte3=A

struct Params {
    width: u32,
    height: u32,
    mode: u32,    // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
}

// ── Mode helpers ──

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

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

    // Compute new values for active channels
    let out_r = /* computed — ALWAYS process R (carries luma in L/LA) */;
    let out_g = select(g, /* computed_g */, mode_has_g(params.mode));
    let out_b = select(b, /* computed_b */, mode_has_b(params.mode));
    let out_a = select(255u, a, mode_has_a(params.mode));

    output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}
```

## Category A: 2-binding → 3-binding Conversion

**Files:** invert.wgsl, grayscale.wgsl, flip.wgsl, mirror.wgsl, duplicate.wgsl, invert_chops.wgsl

**Changes:**
- Replace `num_workgroups`-based dimension computation with `params.width/params.height` bounds check
- Add `Params` struct with `{width, height, mode, _pad}`
- Add `mode_has_g`, `mode_has_b`, `mode_has_a` helpers
- Add `@binding(2) var<uniform> params: Params;`
- Make pixel processing mode-aware using `select(original, computed, mode_has_*(params.mode))`

**Agent prompt template:**

```
Update these shader files in <SHADER_DIR>/ to be mode-aware 3-binding shaders
following the exact pattern from invert.wgsl:

FILES TO UPDATE:
1. grayscale.wgsl
2. flip.wgsl
3. mirror.wgsl
4. duplicate.wgsl
5. invert_chops.wgsl

Every shader must follow this exact structure:
- Params struct: {width, height, mode, _pad}
- Mode helpers: mode_has_g, mode_has_b, mode_has_a
- @binding(0): input (storage, read)
- @binding(1): output (storage, read_write)
- @binding(2): params (uniform)
- Bounds check via params.width/params.height
- Channel decomposition: r = pixel & 0xffu, g = (pixel >> 8u) & 0xffu, etc.
- Mode-aware output: select(original, computed, mode_has_*(params.mode)) for G/B/A

SPECIFIC RULES:
- grayscale: BT.601 luma = (299*r + 587*g + 114*b + 500)/1000. Output luma to R always, to G,B only if active. Preserve alpha.
- flip: output[y][x] = input[H-1-y][x]. Preserve channels, mode-aware alpha.
- mirror: output[y][x] = input[y][W-1-x]. Same as flip.
- duplicate: identity copy. output[idx] = input[idx]. Mode-aware alpha.
- invert_chops: same as invert — 255-val for active channels.

Read each file first, then REWRITE using Write tool. Do NOT use Edit.
```

## Category B: 3-binding + Mode Field

**Files:** solarize.wgsl, posterize.wgsl, brightness.wgsl, contrast.wgsl, color_saturation.wgsl, colorize.wgsl, constant.wgsl, offset.wgsl

**Changes:**
- Add `mode` and `_pad` fields to existing `Params` struct (after height, before op-specific params)
- Add mode helper functions
- Make pixel processing mode-aware

**Agent prompt template:**

```
Update these shader files in <SHADER_DIR>/ to add mode-awareness. These already
have 3 bindings and a Params struct — ADD mode and _pad fields to Params, add
mode helper functions, and make pixel processing mode-aware.

FILES TO UPDATE:
1. solarize.wgsl, 2. posterize.wgsl, 3. brightness.wgsl, 4. contrast.wgsl,
5. color_saturation.wgsl, 6. colorize.wgsl, 7. constant.wgsl, 8. offset.wgsl

STANDARD Params HEADER:
{width, height, mode, _pad} + op-specific params AFTER _pad

KEY RULES:
- R channel is ALWAYS processed (carries luma in L/LA)
- G/B: select(original, new_value, mode_has_g/b(params.mode))
- Alpha: select(255u, a, mode_has_a(params.mode))
- Solarize: threshold param AFTER mode,_pad. if ch >= threshold → 255-ch
- Posterize: bits param AFTER mode,_pad. quantize to bits levels
- Brightness/Contrast/ColorSaturation: factor as fixed-point u32 AFTER mode,_pad
- Colorize: black, white packed colors AFTER mode,_pad
- Constant: value AFTER mode,_pad
- Offset: dx, dy AFTER mode,_pad

Read each file, then REWRITE with Write tool. Do NOT use Edit.
```

## Category C: 4-binding Dual-Input

**Files:** add.wgsl, subtract.wgsl, multiply.wgsl, screen.wgsl, darker.wgsl, lighter.wgsl, difference.wgsl, overlay.wgsl, hard_light.wgsl, soft_light.wgsl, add_modulo.wgsl, subtract_modulo.wgsl, logical_and.wgsl, logical_or.wgsl, logical_xor.wgsl, blend.wgsl, blend_module.wgsl

**Changes:**
- Add `mode` and `_pad` to Params struct
- Add mode helpers
- Make BOTH input pixels mode-aware

**Agent prompt template:**

```
Update these dual-input shader files in <SHADER_DIR>/ to add mode-awareness.

4-BINDING LAYOUT:
- @binding(0): input_a (storage, read) — primary image
- @binding(1): input_b (storage, read) — secondary image
- @binding(2): output (storage, read_write)
- @binding(3): params (uniform)

STANDARD Params HEADER: {width, height, mode, _pad} + op-specific params

Read BOTH input_a[idx] and input_b[idx]. Process R from both always.
G/B: select(original, new_value, mode_has_*(params.mode))
Alpha: select(255u, computed_a, mode_has_a(params.mode))

CRITICAL: Keep @binding declarations! The @group(0) @binding(N) lines must be
present between the mode helpers and @compute. Do NOT drop them during rewrite.

Read each file, then REWRITE with Write tool. Do NOT use Edit.
```

## Category D: Spatial/Filter

**Files:** filter_3x3.wgsl, filter_5x5.wgsl, sharpen.wgsl, median_filter.wgsl, max_filter.wgsl, min_filter.wgsl, rank_filter.wgsl, effect_spread.wgsl, resize_bilinear.wgsl, resize_nearest.wgsl, scale.wgsl, transpose.wgsl

**Changes:**
- Add mode to Params, add helpers
- For convolution ops: always convolve R, conditional G/B
- For resize/transpose: always sample R from source, conditional G/B
- For rank filters: always rank on R channel values

**Key pattern for spatial output:**
```wgsl
let out_r = /* always computed from source samples */;
let out_g = select(0u, /* computed */, mode_has_g(params.mode));
let out_b = select(0u, /* computed */, mode_has_b(params.mode));
let out_a = select(255u, /* computed */, mode_has_a(params.mode));
```

Use `select(0u, ...)` not `select(original_g, ...)` for spatial ops because the output pixel position may not correspond to the same input position (resize, scale, transpose change dimensions).

## Category E: Multi-Pass + Paste/Composite

**Files:** box_blur*.wgsl, equalize*.wgsl, autocontrast*.wgsl, gaussian_blur.wgsl, paste.wgsl, alpha_composite.wgsl, composite.wgsl, composite_module.wgsl, histogram_clear.wgsl, eval.wgsl, point_op.wgsl

**Multi-pass considerations:**
- Each sub-shader in the pipeline must have the SAME Params struct layout
- Histogram shaders: conditionally accumulate R/G/B based on mode
- CDF/remap shaders: use mode-aware channel count

**Paste/Composite (5-binding):**
These shaders use 5 bindings (3 inputs + output + params). The current `build_pipeline()`
only supports 2-4 bindings. Keep these as CPU-only until `build_pipeline()` is extended.

**Eval/PointOp:** LUT is 1024 bytes — too large for uniform buffer. Keep CPU-only.

## Post-Migration Verification

After ALL agents complete, run these checks:

```bash
# 1. Missing binding declarations
for f in gpu_shaders/*.wgsl; do
  if ! grep -q "@binding" "$f"; then echo "MISSING BINDINGS: $f"; fi
done

# 2. Missing mode helpers
for f in gpu_shaders/*.wgsl; do
  if ! grep -q "mode_has" "$f"; then
    if ! grep -q "PLACEHOLDER\|pass-through\|histogram\|cdf\|cutoff\|clear" "$f"; then
      echo "MISSING MODE: $f"
    fi
  fi
done

# 3. Missing Params struct
for f in gpu_shaders/*.wgsl; do
  if ! grep -q "struct Params" "$f"; then echo "MISSING PARAMS: $f"; fi
done

# 4. Count shaders with bindings
grep -l "@binding" gpu_shaders/*.wgsl | wc -l

# 5. Build and test
cargo check -p pillow-rs
maturin develop --manifest-path pillow-rs-py/Cargo.toml --release
```
