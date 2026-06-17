---
name: compute-backend
description: This skill should be used when the user asks to "create a compute backend", "add GPU/SIMD/WebGPU backend", "implement pool_simd", "migrate shaders", "make shaders mode-aware", "add GPU support for ops", or mentions compute pipeline architecture, wgpu shader development, or backend pool implementation for pillow-rs.
---

# Compute Backend Development — pillow-rs

Follow this workflow to implement or extend a compute backend (GPU, SIMD, WebGPU) in the pillow-rs pipeline system. The process was proven during GPU backend development, achieving 100% parity across all modes and operations.

## Architecture Overview

The compute module uses a pool-based dispatch architecture:

- **`compute/mod.rs`** — `Backend` enum (Cpu, Gpu, Simd), `BackendImpl` trait, router
- **`compute/registry.rs`** — Single source of truth: maps `PipelineOp` → `OpEntry` (cpu_fn + gpu_shader)
- **`compute/pool_cpu/`** — CPU reference implementation (universal fallback)
- **`compute/pool_gpu/`** — GPU wgpu-based implementation
- **`compute/gpu_shaders/`** — WGSL compute shaders, one per operation

**Iron rule:** CPU is the universal fallback. Every op registers a CPU function. GPU/SIMD entries are optional additions to the same `OpEntry`.

**Pipeline flow:**
1. Image operations record `PipelineOp` variants (lazy, no execution)
2. On `materialize()`, router picks best active backend that supports ALL ops
3. Backend executes entire batch in one pass (upload → dispatch all ops → readback)
4. `preserve_mode()` converts result back to original color type

## Backend Development Workflow

### Phase 1: Explore Architecture

Read these files in order to understand the current state:

1. **`pillow-rs/src/compute/mod.rs`** — Backend enum, `BackendImpl` trait, router, activation
2. **`pillow-rs/src/compute/registry.rs`** — OpEntry, gpu_entry! macro, variant_key, extract_params
3. **`pillow-rs/src/compute/pool_gpu/mod.rs`** — Full GPU backend: BufferPool, GpuInner, bind group layout, dispatch loop
4. **`pillow-rs/src/compute/pool_cpu/mod.rs`** — CPU pool reference
5. **`pillow-rs/src/compute/gpu_shaders/invert.wgsl`** — Canonical shader example
6. **`pillow-rs/src/pipeline.rs`** — All PipelineOp variants
7. **`pillow-rs/src/image.rs`** — `materialize()`, `preserve_mode()`, `push_op()`
8. **`manifest.yaml`** — `supported_targets` field per function (cpu, gpu, wasm, wasm_gpu)

Identify gaps: which ops are CPU-only in the registry, which shaders exist but aren't registered, which ops need mode-aware variants.

### Phase 2: Design Approach

Present a design covering:

- **Binding layout**: Standardize on N-binding pattern (2=input+output, 3=+params, 4=+second_input)
- **Mode strategy**: Single WGSL file per op with mode as uniform parameter (0=L, 1=LA, 2=RGB, 3=RGBA). Separate files per mode only if algorithms diverge completely.
- **Params struct**: Every shader uses identical header: `{width, height, mode, _pad}` + op-specific params
- **Mode helpers**: Include `mode_has_g()`, `mode_has_b()`, `mode_has_a()` in every shader
- **Buffer management**: Ping-pong buffers (buf_a ↔ buf_b) for single-input ops, dedicated buf_img2 for dual-input
- **Logging**: Use `eprintln!` for backend-path confirmation (the `log` crate has no default logger)

Document the design in `docs/superpowers/specs/YYYY-MM-DD-backend-name-design.md`.

### Phase 3: Implement Rust Infrastructure

Make backend-agnostic infrastructure changes first:

1. **Add/update pool module** — Create `pool_simd/mod.rs` (or extend `pool_gpu/`) implementing `BackendImpl`
2. **Update uniform buffer layout** — If adding mode-awareness: change `[w, h, pad0, pad1]` → `[w, h, mode, _pad]`
3. **Fix buffer tracking** — Verify ping-pong logic: after N ops, `current_is_a` tracks final buffer location; return `Ok(current_is_a)`, NOT `Ok(!current_is_a)`
4. **Add execution logging** — `eprintln!("[{:?}] {} ops {}x{} mode={}: {:?}", backend_name, ...)`
5. **Pre-materialize secondary images** — For dual-input ops, scan ops upfront to materialize second images before dispatch loop
6. **Update `preserve_mode()`** — For L/LA modes, extract R channel directly (not `to_luma8()` which does BT.601 weighted averaging across potentially stale G/B channels)
7. **Register new ops** — Change `OpEntry::cpu_only(...)` → `gpu_entry!(..., "shader.wgsl")` for ops with working shaders
8. **Update `manifest.yaml`** — Add `gpu`/`simd`/`wasm_gpu` to `supported_targets` for relevant functions

Build after each change: `maturin develop --manifest-path pillow-rs-py/Cargo.toml --release`

### Phase 4: Implement Operations — One Subagent Per Op, 10 Concurrent

**Strategy:** Dispatch one subagent per operation, running exactly 10 agents concurrently at a time. This maximizes parallelism while staying within agent concurrency limits. Each agent handles a single op end-to-end: research the CPU implementation, write the backend code, register it, and verify.

#### Step 4a: Build the Op Worklist

Enumerate every op that needs a backend implementation. For each op, capture:
- **Op key** (registry variant name, e.g. `"Invert"`, `"BoxBlur"`)
- **PipelineOp variant** with fields
- **CPU reference** (file path + function name in `pool_cpu/ops/`)
- **Complexity** (simple point-op / spatial / dual-input / multi-pass)
- **Binding count** (2/3/4)

Write the worklist to a tracking file so progress is visible across subagent batches:
```
# simd_worklist.txt — one op per line, mark [DONE] when complete
[    ] Invert          — simple point-op, 3-binding
[    ] Grayscale       — simple point-op, 3-binding
[    ] Solarize        — simple point-op, 3-binding (params: threshold)
...
```

#### Step 4b: Dispatch Batch of 10 Subagents

Use the `Agent` tool with `run_in_background: true`. Dispatch exactly 10 agents per batch (or fewer for the final batch). Each agent gets:

1. **The canonical reference** — path to `invert.wgsl` (or equivalent reference implementation)
2. **The CPU reference code** — the exact function in `pool_cpu/ops/` to match
3. **The standard pattern** — Params struct, mode helpers, binding layout
4. **A single op to implement** — one shader file or one SIMD function

**Agent prompt template (per op):**
```
Implement the <OP_NAME> operation for the <BACKEND> backend.

REFERENCE: Read <CANONICAL_REFERENCE_FILE> for the exact pattern to follow.

CPU REFERENCE: Read <CPU_OPS_FILE> function <FN_NAME> for the algorithm.

STANDARD PATTERN (must match exactly):
- Params struct: {width, height, mode, _pad, ...op_params}
- Mode helpers: mode_has_g, mode_has_b, mode_has_a
- Binding layout: <N>-binding (see reference)
- Bounds check: if gid.x >= params.width || gid.y >= params.height { return; }
- Channel output: select(original, computed, mode_has_*(params.mode))

OP-SPECIFIC:
- Algorithm: <brief description from CPU reference>
- Params: <list of op-specific uniform params after _pad>
- Edge cases: <any mode-specific behavior differences>

OUTPUT: Write the complete implementation to <OUTPUT_PATH>.
Do NOT use Edit — rewrite the entire file with Write.
```

#### Step 4c: Wait for Batch, Verify, Mark Progress

After all 10 agents complete (watch for task notifications):
1. Scan each output file for correctness (bindings present, mode helpers present, Params struct present)
2. Update the worklist: mark completed ops `[DONE]`
3. If any agent produced incorrect output, re-dispatch that single op
4. Dispatch the next batch of 10

#### Step 4d: Post-Migration Verification (all batches complete)

```bash
# For shader-based backends:
grep -L "@binding" gpu_shaders/*.wgsl    # Missing bindings
grep -L "mode_has" gpu_shaders/*.wgsl    # Missing mode helpers
grep -L "struct Params" gpu_shaders/*.wgsl  # Missing Params

# For code-based backends (SIMD):
grep -L "mode_has" pool_simd/ops/*.rs    # Missing mode helpers
cargo check -p pillow-rs            # Must compile clean
```

### Phase 5: Fix Bugs Found During Testing

Common bugs to watch for:

1. **Ping-pong buffer return inverted** — `Ok(!current_is_a)` should be `Ok(current_is_a)`. Trace: after 1 op, result is in buf_b, so `current_is_a = false`, return `false` (not `true`).
2. **Missing @binding declarations** — Subagents rewriting shaders may drop the `@group(0) @binding(N)` lines. Scan after every migration.
3. **`preserve_mode()` uses `to_luma8()`** — This computes BT.601 weighted luma (0.299R + 0.587G + 0.114B). After mode-aware GPU processing, G/B may be stale. Extract R channel directly for L mode.
4. **Shader validation failures** — If `build_pipeline()` returns `None`, the shader is silently skipped. Check pipeline count at startup to verify all expected shaders compiled.
5. **5-binding shaders** — `build_pipeline()` rejects `num_bindings > 4`. Keep ops with 5+ bindings as CPU-only until extended.

### Phase 6: Verify Parity

Run tests with the new backend enabled and confirm:

```bash
# Single op verification
python -m pytest tests/test_fixture_parity.py --backend=gpu --timeout=180 -q \
  -k "invert" --capture=no 2>&1 | grep "\[GPU\]"

# Full comparison with CPU baseline
python -m pytest tests/test_fixture_parity.py --backend=gpu --timeout=300 -q --runxfail
python -m pytest tests/test_fixture_parity.py --backend=cpu --timeout=300 -q --runxfail
```

**Acceptance criteria:**
- `[GPU]` (or backend-specific) messages appear in test stderr output
- Pass/fail counts match between new backend and CPU
- No test regressions (CPU tests remain passing)
- 0 UNTRACKED tests, 100% TRUSTED functions

### Phase 7: Commit

Use conventional commit format:
```
feat: <backend-name> mode-aware compute pipeline — N ops, single-pass mode-as-param

- <key infrastructure changes>
- <shader migration summary>
- <bug fixes>
- <verification results>

Co-Authored-By: Claude <noreply@anthropic.com>
```

## Key Patterns

### Shader Mode Helpers (standard boilerplate)

```wgsl
struct Params {
    width: u32,
    height: u32,
    mode: u32,    // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    // ... op-specific params
}

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }
```

### Mode-Aware Channel Output

```wgsl
let out_r = /* always computed — carries luma in L/LA modes */;
let out_g = select(original_g, computed_g, mode_has_g(params.mode));
let out_b = select(original_b, computed_b, mode_has_b(params.mode));
let out_a = select(255u, original_a, mode_has_a(params.mode));
```

### Uniform Buffer Layout (Rust → WGSL)

```rust
// Rust: upload_params builds [w, h, mode, _pad, ...op_params]
let mut buf = vec![w, h, mode, 0u32];
buf.extend_from_slice(op_params);
queue.write_buffer(&self.params, 0, bytemuck::cast_slice(&buf));
```

### OpEntry Registration

```rust
// CPU-only → GPU-enabled:
m.insert("OpName", gpu_entry!(|img, op, mode| { cpu_fn(img, op, mode) }, "shader.wgsl"));
```

## Troubleshooting Common Issues

### GPU path not executing (tests pass but no [GPU] messages)

Check that GPU is enabled: `core.enable_backend('gpu')`. Verify with `core.active_backends()`.
If GPU is enabled but messages don't appear, an op in the pipeline may not be GPU-supported,
causing router to fall back to CPU. Check router logic: all ops must be in the GPU registry.

### Shader produces same output as input

First suspect: ping-pong buffer tracking. Verify `execute_batch_impl` returns `Ok(current_is_a)`,
NOT `Ok(!current_is_a)`. Add a `device.poll(wgpu::Maintain::Wait)` before readback for safety.

### Shader compiles but output is wrong for L/LA modes

Check `preserve_mode()`. If it uses `to_luma8()`, the BT.601 weighted average will corrupt
results when G/B channels are stale after mode-aware processing. Extract R channel directly.

### Missing @binding declarations after agent rewrites

Subagents rewriting shaders may drop the `@group(0) @binding(N)` lines. Always run the
post-migration verification scans listed in `references/shader-migration-guide.md`.

### 5-binding shaders rejected

`build_pipeline()` has `if num_bindings < 2 || num_bindings > 4 { return None; }`.
Composite, Paste, CompositeModule use 5 bindings (3 inputs + output + params).
Keep these CPU-only or extend `build_pipeline()` to handle 5+ bindings with a third image buffer.

### No logger output from `log::info!`

The `log` crate is a facade — it needs a backend (env_logger, console_log). Use `eprintln!`
instead for backend execution logging, which works in both native and WASM environments.

## Cross-Backend Patterns

When implementing a new backend (SIMD, WebGPU-native), follow these invariants:

1. **Same OpEntry registration** — Add a new field to `OpEntry` (e.g., `simd_fn`) instead of creating a separate registry
2. **Same PipelineOp variants** — Backends consume the same ops; only execution differs
3. **Same mode encoding** — 0=L, 1=LA, 2=RGB, 3=RGBA across ALL backends
4. **Same uniform layout** — `[w, h, mode, _pad, ...params]` for consistency
5. **Same output contract** — Every backend returns `DynamicImage`; `preserve_mode()` handles color type conversion
6. **CPU as universal fallback** — Route falls back to CPU if no other backend supports all ops in the pipeline

## Additional Resources

### Reference Files
- **`references/shader-migration-guide.md`** — Complete subagent prompt templates for each shader category
- **`references/backend-architecture.md`** — Detailed architecture: bind groups, buffer layout, dispatch loop

### Example Files
- **`examples/pool_gpu_module.rs`** — Annotated `pool_gpu/mod.rs` with 10 key design decisions explained
- **`examples/canonical_shader.wgsl`** — `invert.wgsl` as the reference mode-aware shader template
