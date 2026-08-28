# GPU Backend — What We Achieved, What We Tried, What Failed

2026-06-17

## 2026-08-12 resolution

The historical full-process hang is resolved in the active implementation.
The old trace established that the process stopped inside wgpu's public
`CommandEncoder::copy_buffer_to_buffer` call while recording a second,
readback-only encoder. It did **not** establish that `vkCmdCopyBuffer` itself
or command-pool exhaustion was the underlying native cause; wgpu performs
locking, resource tracking, encoder opening, and barrier work before reaching
the backend call.

The active path now records the compute passes and the final storage-to-
readback copy in one command encoder and submits that command buffer once.
It also keeps mutable image buffers local to each lazy batch, bounds shader
work, and places every GPU parity child in an isolated process group with a
hard 300-second outer deadline. A native call that wedges cannot be recovered
inside Rust, so the parent kills and reaps the complete child group.

Managed adapter-backed verification now runs the complete corpus in one
process, well beyond the old roughly 80-case failure point. The final run
executed all 3,674 cases in 11.75 seconds with zero hangs, crashes, timeouts,
not-run cases, or infrastructure errors. It passed 3,673 cases; the sole
failure is the backend-independent
`PIL.ImageFont.FreeTypeFont.set_variation_by_axes.nuanced.variable-font-positive-axis-overflow`
parity mismatch. All GPU-specific cases pass, including the nonzero RGBA green
band and fractional Gaussian/UnsharpMask regressions.

The remainder of this document preserves the original investigation as
historical evidence. Its driver-level explanation is a hypothesis, not a
confirmed root cause, and batching tests into processes of 50 is no longer the
maintained workaround.

## What We Achieved

### Architecture cleanup

Moved `gpu_shaders/` into `pool_gpu/shaders/` so all three backends follow the same structure:
```
pool_cpu/ops/       — CPU reference implementations
pool_gpu/shaders/   — GPU WGSL compute shaders
pool_simd/ops/      — SIMD scalar functions (x86/ARM dispatch)
```

### Multi-backend registry

`OpEntry` now has fields for all backends: `cpu_fn`, `gpu_shader`, `gpu_source`, `simd_fn`. Three macros register ops: `cpu_only!`, `gpu_entry!`, `simd_entry!`. No separate registries.

### Mode-aware shader architecture

Single WGSL file per op with mode as uniform parameter (not 4 files per mode). Every shader uses the same `Params {width, height, mode, _pad}` header. Mode helpers (`mode_has_g`, `mode_has_b`, `mode_has_a`) are inlined in every shader. 62 shaders follow this pattern consistently.

### Infrastructure that works

- **5-binding support**: Extended `build_pipeline()` from max-4 to max-5 bindings. Added `buf_img3` for mask images. Works for Paste, Composite, CompositeModule.
- **LUT storage buffer**: 4-binding variant where `@binding(3)` is `storage, read` instead of uniform. Eval/PointOp LUTs (1024 bytes) fit without the 16-byte stride waste that uniform buffers impose.
- **Dynamic dimension tracking**: `execute_batch_impl` tracks `(cur_w, cur_h)` through size-changing ops (Resize, Crop, Reduce, Scale). Readback uses final dimensions.
- **gpu_log! macro**: Writes to `/tmp/gpu_debug.log` when `RSPIL_GPU_DEBUG=1` is set. Every pipeline step is logged with immediate flush. This was essential for debugging.

### Same-input GPU/WGSL coverage

`make test` selects one public parity corpus and sends that same case scope
through the CPU, SIMD, GPU, Python, and JS/WASM lanes. The GPU lanes enable a
bounded execution collector and write
`build/migration-parity/all-backends/gpu-wgsl-coverage.json` (plus the smoke
artifact). The artifact inventories every checked-in WGSL file and records
which files and registry variants actually dispatched, including dispatch and
workgroup counts.

This is intentionally execution coverage, not source coverage. WGSL line and
branch percentages are reported as `not_measured` until an instrumented shader
build can record branches without changing the normal shader bindings or
parity outputs. The Node JS/WASM lane also reports shader coverage as
`not_measured` because the current package is built without the native GPU
feature and this Node environment does not provide a WebGPU adapter. The same
corpus now runs in a real browser WASM page as `browser-wasm-parity`; that
lane records `navigator.gpu`/adapter/device availability separately, but does
not call a capability probe a Pillow shader dispatch. An instrumented WGSL
variant and an asynchronous browser GPU API are still required to prove
browser shader execution.

The execution boundary is also important when interpreting this receipt. A
lazy pipeline stays queued until its terminal observation. When the complete
batch is GPU-compatible, the GPU executor uploads the host image once, records
the whole batch, and performs one final device-to-host readback. If the batch
cannot run on GPU, routing selects the next supported backend. CPU and SIMD
share host-resident image buffers, so a SIMD→CPU fallback needs no copy or
materialization. If a future planner splits a pipeline around a GPU-only
segment, only the GPU↔host transitions should count as full-frame copy
boundaries; the shader receipt should continue to report only dispatches that
actually occurred.

`make migration-parity-test-gpu-strict` is available when a GPU-only
capability audit is intentional. It keeps strict target locking and is not
part of `make test`; the normal test must measure parity with the documented
fallback behavior.

### Subagent-driven op migration

20+ subagents were dispatched across GPU shader creation and SIMD function writing. Each agent handled 3-5 ops, following a standard pattern. This worked well — 71 SIMD functions and 18 new GPU shaders were written by agents in parallel.

## What We Tried And Failed

### The GPU test hang

**The symptom**: Full 564-test GPU suite hangs after ~80 tests. Individual tests pass. Batches of 50 pass. Batches of 100+ hang.

**The debug process**: We added streaming file-based logging (`gpu_log!` macro writing to `/tmp/gpu_debug.log` with flush). This revealed the exact hang point every time:
```
[GPU] readback: create_encoder start
[GPU] readback: encoder created, copy_buffer_to_buffer start
-- hangs here, never reaches "copy submitted, map_async start" --
```

The `create_command_encoder` succeeds. `encoder.copy_buffer_to_buffer(src, 0, staging, 0, size)` blocks forever. This is a CPU-side command recording call that should never block.

**Attempted fixes (all failed)**:

1. **Double-poll** — Added second `device.poll(Wait)` after unmap. Theory: single poll might not drain all pending work. Result: no difference.

2. **Poll after every submit** — Every `queue.submit()` immediately followed by `device.poll(Wait)`. Theory: ActiveSubmission queue grows unboundedly between polls. Result: no difference. Hang still at same line, same test count.

3. **Reusable staging buffer pool** — Volta-style `HashMap<u64, Vec<Buffer>>` with 64-buffer cap. Buffers acquired from pool, never dropped, returned after use. Theory: `device.create_buffer`/drop cycles exhaust wgpu's internal tracking. Result: no difference. Hang at same spot.

4. **Fresh staging buffer per readback** — Back to `create_buffer` with exact image size. Removed the pool entirely. Theory: recycled buffers might be in a bad state. Result: no difference.

5. **Empty submit between cycles** — `queue.submit([])` after unmap (wgpu Issue #5173). Theory: pending writes need flushing before next map_async. Result: no difference.

6. **Back-pressure with 16-submission cap** — `submission_count` tracked, forced `poll(Wait)` every 16 submits. Theory: unbounded ActiveSubmission vec exhaustion. Result: no difference.

7. **Deep research** — Ran a multi-agent research workflow searching wgpu issues, Vulkan command pool limits, staging pool patterns, and test suite strategies. Found wgpu Issue #5969 (fence value bump before submission success) but this is fixed in wgpu 24 which we use. Found Volta's StagingBufferPool pattern which we already implemented. No definitive solution found.

**What we know**:
- The encoder creates fine, but `copy_buffer_to_buffer` blocks
- All polls return successfully before this point
- Staging buffer lifecycle doesn't matter (fresh or pooled, same result)
- The hang is deterministic — always ~80 tests in, always at the same code line
- It's not a wgpu version issue (we're on wgpu 24, the latest)
- The deep research confirmed we're not missing any obvious pattern

**Leading theory**: GPU driver-level command pool exhaustion. The Vulkan driver on this machine (Intel integrated or software renderer) has a finite command buffer pool that fills after ~80 encoder→submit→poll cycles. Once full, `vkCmdCopyBuffer` blocks until a command buffer is freed, but there's no mechanism to free them faster than the GPU completes them. This is a driver limitation, not a code bug.

**Workaround**: Run tests in batches of ≤50 per process. This isn't a fix, but it's the only thing that works.

## What We Didn't Get To

### Combine dispatch + copy into single encoder

Currently each op does 3 `queue.submit()` calls per batch: one for the compute dispatch, one for the staging copy, one empty flush. Combining dispatch + copy into one encoder would reduce submits to 1 per op. This might help with the hang by reducing command pool pressure, but the root cause (driver pool limit) would still exist.

### Test on different GPU hardware

All testing done on the same machine. The hang might not occur on NVIDIA (proprietary driver), AMD (RADV), or Apple Metal. This needs testing.

### Try non-Vulkan wgpu backend

wgpu supports `Backends::GL` (OpenGL) and `Backends::METAL`. The hang might be Vulkan-specific. Worth testing with a different backend.

### WASM/WebGPU testing

The browser WASM parity lane now exercises the same public workflows through a
real headless browser. The current package is CPU/fallback-only: the core GPU
implementation uses synchronous initialization/readback, while browser
WebGPU requires async initialization and mapping. A browser GPU lane needs an
async API before it can honestly claim dispatch or WGSL coverage.

### SIMD intrinsics

The SIMD backend currently uses scalar loops with auto-vectorization hints. No actual SSE/AVX/NEON intrinsics are written. The `x86.rs` and `arm.rs` files just re-export `scalar.rs` via `pub use super::scalar::*`. Real intrinsics would give 4-8x speedup on point ops.

### Naming audit

We found `sharpen.wgsl` registered for `Sharpness` PipelineOp (different algorithms — fixed kernel vs factor-based enhance). This was fixed. But a full audit of all 71 shader names vs registry keys vs SIMD function names hasn't been done. The snake_case/CamelCase mapping is mostly consistent but may have other mismatches like `CropBorder`/`crop_border`, `Sharpness`/`sharpness`, etc.
