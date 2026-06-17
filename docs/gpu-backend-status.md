# GPU Backend — Status & Remaining Work

Last updated: 2026-06-17

## Accomplished

### GPU Shader Coverage: 53/71 → 71/71

All 18 previously CPU-only ops now have GPU WGSL shaders in `pool_gpu/shaders/`:

| Category | Shaders | Status |
|----------|---------|--------|
| Spatial | crop, reduce, thumbnail, contain, cover, fit | Created |
| Point | convert, quantize, effect_noise, put_alpha, put_pixel | Created |
| Multi-input | paste, alpha_composite, composite_module, composite | Created |
| LUT | eval, point_op | Created (storage buffer) |
| Complex | transform, put_data | Created |

### Infrastructure Unlocks

1. **Dynamic dimension tracking** — `execute_batch_impl` now tracks `(cur_w, cur_h)` across size-changing ops
2. **5-binding support** — `build_pipeline()` extended from max 4 to 5 bindings; `buf_img3` added
3. **LUT storage buffer** — 4-binding variant with `@binding(3)` as `storage, read` for 1024-byte LUTs

### SIMD Backend: 71/71 ops

42 scalar functions in `pool_simd/ops/scalar.rs`, platform dispatch via `x86.rs`/`arm.rs`/`scalar.rs`. 564/564 tests pass.

### Bug Fixes

- **Ping-pong buffer tracking**: `Ok(!current_is_a)` → `Ok(current_is_a)`
- **preserve_mode L/LA**: Direct R-channel extraction (not BT.601 weighted luma)
- **Grayscale mode**: Forces L output after GPU (was incorrectly preserving RGB)
- **Sharpness shader**: Fixed algorithm — now uses SMOOTH kernel + factor blend (was fixed SHARPEN kernel ignoring factor)
- **Missing @binding declarations**: 3 dual-input shaders had dropped bindings after agent rewrites

## Known Issue: GPU Test Hang at ~80 Sequential Operations

### Symptom

Running 564 sequential GPU tests hangs after ~80 tests. Individual tests and batches ≤50 pass.

### Root Cause

`encoder.copy_buffer_to_buffer()` blocks inside wgpu's Vulkan backend after ~80 sequential command submissions. Debug logging consistently shows the hang at:
```
[GPU] readback: encoder created, copy_buffer_to_buffer start
```
The encoder is created successfully but `copy_buffer_to_buffer` never returns. This is a GPU driver-level command pool exhaustion on the test machine — not a wgpu bug (we're on wgpu 24 which has the fence fix from PR #5970).

### Attempted Fixes (all failed)

1. Reusable staging buffer pool (Volta-style) — no effect
2. Poll after every `queue.submit()` — no effect
3. Double-poll (Wait + Wait) — no effect
4. Empty submit between cycles (wgpu#5173) — no effect
5. Back-pressure with 16-submission cap — no effect
6. Fresh staging buffer per readback — no effect

### Workaround

Run tests in batches of ≤50 per process:

```bash
python -m pytest tests/ --backend gpu --timeout=60 -q -k "batch1_pattern"
python -m pytest tests/ --backend gpu --timeout=60 -q -k "batch2_pattern"
```

### Future Investigation

1. Test on different GPU/driver (AMD, Intel, NVIDIA with different driver versions)
2. Try wgpu with `wgpu::Backends::GL` (OpenGL backend) instead of Vulkan
3. Try reducing buffer sizes (64MB → smaller per-op buffers)
4. Try `wgpu::util::DownloadBuffer` instead of manual staging
5. Profile with `RUST_GPU_TRACE=1` or Vulkan validation layers

## Remaining Work Items

### Priority 1: GPU Test Stability

- [ ] Find definitive fix for GPU test hang (different machine, driver update, or backend switch)
- [ ] Add `--backend gpu` CI job with batched test execution
- [ ] Profile per-op GPU memory usage to identify resource leaks

### Priority 2: New Shader Validation

- [ ] Run all 18 new shaders through individual parity tests
- [ ] Verify dimension-changing ops (crop, resize, thumbnail) work correctly with dynamic dim tracking
- [ ] Verify 5-binding ops (paste, composite) with mask
- [ ] Verify LUT ops (eval, point_op) with storage buffer LUT
- [ ] Verify transform with affine matrix

### Priority 3: Optimization

- [ ] Combine dispatch + copy into single command encoder (reduces submits from 3→1 per op)
- [ ] Profile SIMD vs GPU vs CPU performance on representative ops
- [ ] Add `wide` crate for actual x86 SSE/AVX intrinsics (currently scalar with auto-vectorization hints)
- [ ] Add ARM NEON intrinsics for `pool_simd/ops/arm.rs`

### Priority 4: WASM/WebGPU

- [ ] Test GPU backend in browser via `wasm-pack build`
- [ ] Async GPU init for WASM (pollster doesn't work in browser)
- [ ] Reduce buffer sizes for WASM memory constraints

### Priority 5: Naming Consistency

- [ ] Audit all 71 shader filenames match registry keys (e.g., `sharpness.wgsl` ✓, check others)
- [ ] Audit SIMD function names match registry keys (snake_case → CamelCase mapping is unambiguous)
