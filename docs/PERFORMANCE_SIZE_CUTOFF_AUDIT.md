# Audit of size-dependent performance choices and retained memory

**Audit date:** 2026-09-27
**Purpose:** inventory existing workload-size decisions across `pillow-rs`, `fontdone`, and `image-slash-star`; identify unmeasured or over-specialized paths; propose a smaller set of data-structure and policy patterns.

## Scope and snapshot

This is a source-level audit of the current checkouts, including optimizations that predate 2026-09-26. I searched the full relevant Rust source trees for size/work thresholds, byte and item capacities, parallel cutoffs, tile sizes, cache bounds, and cancellation checkpoint schedules, then followed the main code paths. Repeated uses of the same policy are grouped into one inventory row. Ordinary checked-dimension arithmetic, format field widths, codec-mandated domains, and security/resource limits are called out separately rather than treated as optimization cutoffs.

| Project | Revision reviewed | Worktree state |
| --- | --- | --- |
| `pillow-rs` | `496f478e` (2026-09-27) | GPU extraction planner/shader have uncommitted edits; this audit includes their working-tree state. `CLAUDE.md` also has an unrelated existing edit and was left alone. |
| `fontdone` | `e77b7120` (2026-09-26), `/Users/lazytrot/work/fontdone-e77-variant` | Existing untracked `docs/heic-implementation-plan.md` was left alone. |
| `image-slash-star` | `96303c0e` (2026-09-20) | Clean at audit time; no commits in this checkout after 2026-09-20. |

This document does not claim that recorded benchmark numbers were rerun. The current `pillow-rs` performance campaign is used as historical evidence where noted. This audit adds documentation only; it does not change runtime code.

## Findings at a glance

1. **The most duplicated tuning is in `pillow-rs`.** Many CPU and SIMD operations choose serial versus parallel execution using separate pixel or byte constants. The repeated `512 * 512` CPU cutoff is a maintenance smell, but one cutoff should not be forced onto kernels with different work per pixel.
2. **Several choices are sound and not over-optimization.** A dense table for a small bounded key domain, a sparse map for a large domain, fixed arrays for format-bounded filter windows, and byte-bounded coefficient caches fit their access and memory patterns.
3. **There is one clear retained-state defect in fontdone.** The process-global charmap registry is keyed by raw addresses and `FT_Done_Face` does not remove a face's entries. This retains stale metadata without a lifecycle bound. Its exact-key lookups also do not need sorted ordering.
4. **There is a material memory cost in image-slash-star's sequence cache.** It retains a complete decoded sequence and returns an owned clone of it. For large animations, the cached copy and returned copy can coexist.
5. **Do not collapse unrelated limits into one magic number.** GPU resource ceilings, cancellation cadence, algorithmic window sizes, and parallel crossover points solve different problems. They need separate names and evidence.

## Inventory and recommendations

### `pillow-rs`

#### Counting colors: dense versus sparse tables

`pillow-rs/src/image.rs` selects `PillowColorCounts::Direct(Vec<u64>)` when Pillow's color-table mask is at most 1,023; this allocates 1,024 slots at most, or 8 KiB of payload. Larger domains use `HashMap<u32, (u32, u32)>`, then sort occupied slots to preserve Pillow's observable order. Both paths stop after the `(maxcolors + 1)`th distinct value. For `maxcolors <= 256`, inputs of at least 64 KiB pre-reserve at most 257 sparse entries.

**Assessment: keep the dense/sparse split; audit the 64 KiB reserve cutoff.** The split reflects domain density and exact ordering requirements, rather than an arbitrary image-size specialization. The direct table has a fixed, small upper memory bound. The separate 64 KiB reservation condition is a lower-confidence heuristic: retain it only if complete-call benchmarks show that the allocation savings beat the upfront reservation for representative small-`maxcolors` workloads. Name the direct-table bound in bytes and derive slots from `DIRECT_TABLE_MAX_BYTES / size_of::<u64>()`; do not grow a direct table in proportion to an untrusted `maxcolors` value.

The repository's 2026-09-27 campaign records 3.625 to 2.125 microseconds for the standard 16x16 RGB call after the direct table, while varied 16x16 RGB remains about 2.01x Pillow and the 1024x768 high-cardinality early-exit case is about 1.08x Pillow. These are campaign records, not fresh audit measurements. The campaign also records that raw-byte iteration regressed and was reverted, and recommends profiling the remaining latency before further attempts. This is a good example of stopping after the measured, bounded improvement rather than adding another input-specific loop.

#### CPU and SIMD scheduling cutoffs

| Current decision | Source locations | Recommendation |
| --- | --- | --- |
| CPU pointwise and related paths use `512 * 512` pixels. Separate copies appear in `imageops.rs`, `enhance.rs`, `chops.rs`, `effects.rs`, and `geometry.rs` (including reduce). | `pillow-rs/src/compute/pool_cpu/ops/{imageops,enhance,chops,effects,geometry}.rs` | Replace duplicated constants with a small shared scheduling helper and a few workload classes. Use bytes plus a rough per-pixel work estimate; preserve per-kernel overrides only where paired measurements show a different crossover. |
| CPU Fit uses `32 * 32` pixels; SIMD blur, reduce, and fit use the same `32 * 32` pixel threshold. | CPU `ops/imageops.rs`; SIMD `ops/adapters.rs` | Re-evaluate by estimated work and destination size. A 1,024-pixel image can be cheap for a byte copy and expensive for a multi-tap filter. |
| CPU blur uses a 256 KiB parallel minimum. SIMD native row transforms use 256 KiB. SIMD blend and fused paths use 4 MiB; SoftLight and alpha composite use 1 MiB. These paths usually split output into 64 KiB tiles. | CPU `ops/filter.rs`; SIMD `ops/adapters.rs` | Use one named tile/grain helper for independent byte ranges. Keep a small number of operation classes (bytewise, neighborhood, resampling) with byte-based scheduling costs. Do not keep four equivalent copies of a 64 KiB tiling loop. |
| CPU transpose uses 32-pixel tiles and starts tiled work at 256 Ki pixels. SIMD transpose grouping also starts at 256 Ki pixels. Additional SIMD row-cache transpose paths admit only specific channel counts, aligned dimensions, roughly 1–8 MiB or 2–8 MiB outputs, and selected aspect ratios. | CPU `ops/geometry.rs`; SIMD `ops/adapters.rs` (`transpose_native_collect_admitted`, `transpose_native_odd_collect_admitted`) | The regular tiled path is a conventional locality optimization. The narrow row-cache admission logic is the strongest over-specialization candidate: keep it only if each admitted and excluded geometry has repeatable end-to-end wins and the evidence is stored next to the code. Otherwise remove the special admission paths and keep the simpler tiled/direct kernels. |

The recent `getchannel` work is not a size-cutoff optimization. The campaign says compact GPU output cuts readback by 4x, but standalone GPU extraction remains much slower due to transfer and launch costs; vector shuffles improved the earlier SIMD path, while a separate attempt to parallelize the cheap gather was rejected and reverted. Keep the compact transfer and sequential vector path only with their parity evidence. Do not add another pixel-size parallel branch without new phase-level evidence.

#### Scan-before-fastpath and algorithm-window cutoffs

| Current decision | Source locations | Recommendation |
| --- | --- | --- |
| Uniform-image scans for CPU box blur and small uniform byte images stop at 4,096 pixels. SIMD has separate 4,096-pixel uniform-neighborhood, 65,536-pixel zero-neighborhood, and 65,536-pixel convolution scan caps. GPU uniform blur scans at most 1,048,576 pixels before copying instead of dispatching blur passes. | CPU `ops/filter.rs`; SIMD `ops/adapters.rs`; GPU `pool_gpu/mod.rs` | These fastpaths pay a scan to skip later work. Compare scan bytes against the estimated work actually avoided (passes, radius/taps, channels); keep separate caps only where that cost differs. The 1 MP GPU cap is a performance crossover, not a device limit, and deserves a representative GPU benchmark before further adjustment. |
| Integer order-statistic filters use a sorting network through size 5 and histogram selection through size 15/area 225. Float windows have a size-9/area-81 bound. | SIMD `ops/adapters.rs` | These are algorithmic crossovers with fixed scratch arrays. Keep the separate integer and float bounds, explain their complexity, and benchmark adjacent sizes. A single image-pixel threshold cannot replace these window-size decisions. |
| Boxed resize scans for all-zero input only through 65,536 pixels. Vertical resize transpose begins at 262,144 intermediate samples. | `pillow-rs/src/ops/pil_resize.rs` | Both are plausible scan/copy-versus-compute crossovers. Measure by source and destination size, channel count, and filter; account for the extra full-frame temporary in transpose's peak memory. |

#### GPU admission, resource bounds, and caches

The GPU code has several values that look like “size thresholds” but serve distinct purposes:

| Bound | Current use | Assessment and better representation |
| --- | --- | --- |
| `GPU_BUFFER_CAPACITY = 4096 * 4096` pixels | Repeated input/output admission checks across many ops; a packed `u32` buffer at the ceiling is 64 MiB. | Valid as a conservative fallback envelope, but it is a coarse global pixel limit. Prefer per-operation checked byte requirements against actual device buffer/storage limits, with CPU fallback when the request does not fit. Keep one shared planner instead of re-deriving pixel caps at call sites. |
| `MAX_GPU_SHADER_WORK_ITEMS = 2 * 1024^3`; blur radius 64; filter size 15; reduce factor 64; resize tap/binding/tile bounds (15/32/8,388,607 taps, 128 MiB coefficients, 16,384 taps per tile, 8 rows, 32,768 pixels). | Bounds GPU shader loops, representation/layout, and one dispatch's work. | These are safety, queue-fairness, or shader-layout bounds, not algorithm crossovers. Keep them explicit and validate all byte arithmetic before allocation/dispatch. Where possible, derive adapter-facing limits from device limits. |
| `GPU_F_AFFINE_PROOF_MAX_PIXELS = 1,048,576` | Caps an exhaustive host-side proof before GPU admission. | A bounded proof is appropriate. Keep it distinct from image capacity, because it caps host validation work rather than device storage. |
| `GPU_UNIFORM_BLUR_MAX_PIXELS = 1,048,576` | Caps a host scan that tries to skip GPU blur work on constant input. | This is a performance heuristic. Keep only with a measured scan-versus-dispatch crossover; pixel count should be converted to actual bytes/work for each image mode. |
| Resource reuse and cache limits: 4 working sets/128 MiB, 2 staging buffers/64 MiB, 64 MiB auxiliary cache, and a maximum 4:1 buffer reuse ratio. Submission is split at 256 ops and 64 MiB of auxiliary resources (a single large op is allowed by the current code). | Device memory retention and queue batching. | The independent retained-byte ceilings can add to about 256 MiB before active/in-flight allocations. Prefer a shared `GpuCacheBudget` that tracks retained bytes across working sets, staging, and auxiliary data; keep entry/count limits as secondary guards. Record active and idle bytes separately. Retain the 4:1 reuse rule only if target GPU measurements support it. |

The current uncommitted `ExtractBand` change plans a near-square 2D grid and checks the adapter's `max_compute_workgroups_per_dimension`, `u32` shader indexing, and padded-grid arithmetic. That is a correct dispatch/resource plan, not an overfit image-size optimization. It addresses the one-dimensional workgroup ceiling while keeping the shader's packed indexing bounded. This audit did not run or validate those edits.

#### Font and coefficient caches in the Pillow binding

- `pillow-rs/src/font/imagingft.rs` keeps one small source-face entry and one variation-metadata entry per thread, admitting raw font inputs up to 256 KiB. This bounds source bytes per thread but not the size of parsed metadata; total retained state grows with thread count. Keep the cache request/thread scoped or account its full retained bytes if it becomes a process-wide memory concern.
- The same file caps glyph-index, advance, and kerning maps at 512 entries each per font object. Since keys and values are fixed-size, this is a simple reasonable entry bound. If these entries change to own larger data, add a byte cap rather than copying the 512-entry rule.
- `pillow-rs/src/ops/pil_resize.rs` caps filter-coefficient caching at 16 entries and 8 MiB, with FIFO/LRU-style order in `VecDeque`. This is the strongest existing cache policy: it bounds both item count and retained payload bytes. Preserve byte accounting if new coefficient forms are added.

### `fontdone`

#### Per-face glyph cache

`src/tables.rs` lazily stores TrueType/CFF2 outlines in `HashMap<u16, Rc<GlyphOutline>>` per font face. Random glyph indices need exact lookup; a map avoids reserving a 65,536-slot array when only a small subset is used. The key domain is naturally finite, but outline memory varies with contour complexity, so a key-count ceiling alone does not tightly bound bytes. Variation-coordinate clones correctly reset this cache, and stateful CFF outline loads bypass caching.

**Recommendation:** keep the sparse per-face map. If profiling shows long-lived faces accumulate too much memory, track estimated outline bytes and evict or clear at a per-face budget. Do not replace it with a fixed 65,536-element vector unless actual usage is dense enough to justify the empty-slot cost.

#### Global charmap metadata registry

`src/ffi/handles.rs` uses a process-global `BTreeMap<usize, (FT_Long, FT_ULong, FT_Int)>` from raw charmap addresses to metadata. This is used for exact lookups; ordered traversal is not needed. `charmaps_to_ffi` registers entries, while `FT_Done_Face` currently consumes the face without unregistering its charmaps.

**Assessment: invalid lifetime management and unbounded stale metadata.** Entries can outlive their owning face and accumulate across repeated font opens. Address reuse may overwrite some stale keys, but does not provide a memory bound or ownership guarantee.

**Recommendation:** make registration face-owned and remove every registered key as part of face destruction (an RAII registry guard is one workable approach). Ensure every cloned face's new charmap addresses are registered. Use `HashMap` for exact-address lookup unless sorted/range behavior is added. Add lifecycle coverage that repeatedly opens, clones, and drops faces and asserts the registry returns to baseline. Fixing ownership is more important than the map swap.

#### Small variable-coordinate scratch buffer

`INLINE_AXIS_CAPACITY = 8` uses a stack `[i32; 8]` for up to eight variation coordinates and a `Vec` above that. This is 32 bytes of stack scratch on a 32-bit `i32` representation and avoids a heap allocation for common small axis counts.

**Assessment: reasonable, not a 100-case specialization.** Keep the simple branch. A `SmallVec<[i32; 8]>` would make sense only if several APIs need the same inline/heap behavior and the dependency/abstraction reduces total complexity.

Recent parsed-face reuse for named/static font variants is also not size-specific dispatch. Reusing immutable parsed tables while resetting or replacing per-face variation state is structurally sound when all face-dependent caches and metadata follow the new face's lifecycle.

### `image-slash-star`

#### Cancellation and progress checkpoints

The large checkpoint inventory is documented in `src/cancel.rs` and implemented across codec modules. These values control when cancellation/progress is polled; they do not select a different image algorithm.

| Codec/work family | Existing checkpoint patterns |
| --- | --- |
| BMP and PNG | BMP row conversion and payload-copy checkpoints at 1,024 pixels/bytes; PNG filter/output work at 1,024 bytes. |
| JPEG | RGB conversion, downsampling, coefficients, MCUs, progressive blocks/events, and output bytes commonly checkpoint at 1,024 units. |
| GIF | Quantization pixels, octree cells, median-cut items, and nearest-palette work commonly checkpoint at 1,024 units. |
| WebP preparation and analysis | Pixels, macroblocks, blocks, symbols, candidates, and palette values use 64, 256, or 1,024-unit intervals depending on stage. |
| WebP bitstreams | VP8L/VP8 residual and partition paths have multiple logical-bit boundaries from 8 bits up through 2,097,152 bits, plus 1,024-byte output intervals. |
| WebP reference/cost processing | Candidate trials and palette offsets use 64; run/cache/backfill scans commonly use 256; token/entry scans commonly use 1,024. |

This is many named cadence constants, but it is not evidence that every codec has a separate algorithm. A byte, pixel, MCU, Huffman node, and candidate trial have very different costs. `ProgressEvent` counts accepted checkpoints, so cadence changes can also alter observable callback frequency.

**Recommendation:** centralize checkpoint bookkeeping, not all cadence values. Use a small typed `WorkUnit`/`CheckpointCounter` abstraction that charges work in bytes, pixels, blocks, tokens, or candidates and polls when its configured interval is reached. Keep a few cadence profiles by approximate unit cost and retain stage-specific intervals only when they bound cancellation latency for unusually expensive inner loops. Preserve callback-frequency behavior where it is part of the public contract. Do not replace this with one global “every N pixels” constant.

#### Owned decode caches

`src/source.rs` stores both an optional still decode and an optional full sequence decode in `OnceLock`s owned by `EncodedImageInner`; clones share those caches. The borrowed `EncodedImageView` deliberately caches only verification. Still `decode()` returns a reference to its retained result, but `decode_sequence()` clones the retained `DecodedSequence` into an owned result. A sequence contains a `Vec` of full decoded frames, so the cached frames and cloned return value can coexist and the clone cost scales with all decoded pixels.

**Recommendation:** introduce a shared/borrowed sequence-returning API (for example, `Arc<Decoded<DecodedSequence>>` or a borrow tied to the source) and implement the owned API as a compatibility wrapper. Consider a streaming or frame-iterator API for very large sequences. If many encoded sources remain live, expose or enforce an aggregate retained-decoded-byte budget; a per-source `OnceLock` does not bound process-wide retained image memory. Avoid sharing still and sequence materializations twice if both are requested for the same single-frame input.

#### WebP reusable scratch and bounded codec tables

`src/codecs/webp/native/encoder.rs` keeps at most two `Vec<Token>` buffers in a per-encoder `result_pool` across sequential frames. This prevents repeated allocation, but the count limit does not cap retained bytes; two vectors can each retain capacity from a large prior frame. `backward_refs.rs` sizes length-cost tables using `pixel_count.min(MAX_LENGTH)`, where `MAX_LENGTH` is the format's 4,095-pixel maximum match length. The latter is a useful bounded working set.

**Recommendation:** retain sequential scratch reuse, but cap the sum of `capacity * size_of::<Token>()` and drop/shrink oversized vectors after the frame or sequence. Keep format-bounded tables as fixed/compact vectors where indexing is dense and the codec domain is small.

### Limits that should not be removed as “over-optimization”

These choices encode correctness, format, host/device, or security constraints rather than workload tuning:

- `pillow-rs` checked dimensions and maximum-pixel/resource admission.
- GPU dispatch dimension, storage-buffer, and shader-indexing bounds; the workgroup limit should come from the actual adapter when possible.
- Filter window/algorithm limits that bound fixed stack arrays or a shader algorithm's complexity.
- WebP match length and color-cache bit widths; JPEG block/MCU alignment rules; palette sizes fixed by file formats.
- Cancellation checkpoints needed to enforce work budgets and responsiveness.

Keep these named and documented. Replace constants with derived values only when the actual governing limit can be queried or calculated safely; never relax a limit just to remove a branch.

## Smaller, reusable design for future changes

The objective should be a handful of well-named policies, not one threshold per call site and not one global cutoff for unrelated work.

### 1. Choose storage by key-space density and required order

| Access pattern | Preferred representation | Example here |
| --- | --- | --- |
| Small bounded integer domain with frequent direct access | `Vec`/fixed array indexed by key; derive its maximum from an explicit byte budget | `PillowColorCounts::Direct` for up to 1,024 slots |
| Sparse keys or unpredictable/unbounded domain | `HashMap` with reserve only when a realistic upper bound is known | Large `getcolors` domain; font glyph outlines |
| Sorted iteration, predecessor/successor, or range queries required | `BTreeMap` | Keep only where sorted behavior is actually used |
| Exact pointer/key lookup with a strict owner lifetime | `HashMap` plus explicit insert/remove lifecycle | fontdone charmap metadata registry |
| FIFO/LRU eviction order | `VecDeque` plus a lookup map, with byte accounting | resize coefficient cache |
| Fixed, format-bounded local window | Stack array or compact `Vec` sized from the format bound | SIMD order-statistic scratch |

Rust's collection guide recommends `Vec` for resizable/sequential arrays, `HashMap` for arbitrary maps and caches, and `BTreeMap` where sorted/range operations are needed. Its capacity guidance is to reserve for an exact size or reasonable upper bound, not merely because the input is large. See the [Rust standard collections guide](https://doc.rust-lang.org/std/collections/).

For every cache, track both entry count and retained bytes. Include spare capacity where possible, evict or decline an insertion when the budget would be exceeded, and tie entries to an owner/request lifetime. A per-thread or per-face cap multiplies by the number of threads/faces and should be described as such.

### 2. Centralize scheduling decisions by work class

Use a small helper over checked sizes and a rough cost estimate, rather than `if pixels >= ...` copied into many kernels. A minimal shape is:

```rust
struct WorkEstimate {
    input_bytes: usize,
    output_bytes: usize,
    passes: usize,
    taps_per_output: usize,
}

enum WorkClass {
    Bytewise,
    Neighborhood,
    Resample,
}
```

The helper should return a serial/parallel plan and tile grain. `Bytewise` work is close to bytes moved; neighborhood and resampling work must account for repeated taps/passes. Keep overrides only when measured workload families differ. Avoid building a runtime autotuner or dozens of per-op policies for a handful of crossover classes.

Rayon's slice APIs already support fixed-size parallel chunks, which fits the existing disjoint output tiles; choose grain independently from the serial/parallel crossover. See [Rayon's `ParallelSlice`](https://docs.rs/rayon/latest/rayon/slice/trait.ParallelSlice.html).

### 3. Bound retained and temporary memory in bytes

For CPU/GPU buffers and caches, write down peak and retained byte formulas before choosing a cap. Use checked multiplication and a fallible reserve path when allocation size is derived from untrusted input. For GPU caches, keep one aggregate idle-cache budget and report active/in-flight resources separately. For caches, count retained capacity rather than only logical length.

The GPU device limit is not a portable fixed pixel count. Derive dispatch and binding plans from the adapter's limits and the operation's element size; wgpu exposes limits including maximum workgroups per dispatch dimension. See [wgpu `Limits`](https://docs.rs/wgpu/latest/wgpu/struct.Limits.html).

### 4. Benchmark only a real crossover and stop when evidence stops

For each optimization cutoff, compare the complete public operation just below, at, and above the candidate threshold; include representative formats, channels, dimensions, and input entropy. Record allocations/temporary bytes as well as latency. Require exact-output parity for every measured workload. Criterion provides statistical benchmarking rather than relying on one timing; see [Criterion](https://docs.rs/criterion/latest/criterion/).

If a special path does not repeatedly win, delete it. The existing `getchannel` record demonstrates why: a plausible parallel split lost to scheduling and memory contention, so the code retained the simpler sequential path. The specialized SIMD transpose-cache branches need this same evidence standard.

## Priority recommendations

1. **Fix fontdone charmap metadata ownership and removal.** This is the only confirmed unbounded stale registry found in the focused cross-project inventory.
2. **Avoid deep sequence clones in image-slash-star.** Add a shared or borrowed sequence access path and account retained bytes.
3. **Introduce a shared CPU/SIMD scheduling helper in `pillow-rs`.** Start with the duplicated pointwise/bytewise thresholds; do not force neighborhood and resampling operations onto the same numeric cutoff.
4. **Review the narrow SIMD transpose row-cache paths.** Keep them only with evidence for each admitted geometry; otherwise delete the specialization.
5. **Aggregate GPU retained-cache budgets.** The current independent limits can retain substantially more memory than any one ceiling suggests.
6. **Keep cancellation intervals typed and centralized.** Reduce boilerplate and naming duplication while preserving work units and cancellation behavior.
7. **Preserve the good bounded structures.** Keep the `getcolors` dense/sparse hybrid, the 8 MiB resize coefficient budget, bounded WebP matching tables, and fixed filter-window scratch arrays.

## Research sources

- [Rust standard collections guide](https://doc.rust-lang.org/std/collections/) — choosing `Vec`, `HashMap`, `BTreeMap`, and capacity management.
- [Rust `HashMap` documentation](https://doc.rust-lang.org/std/collections/struct.HashMap.html) — map capacity, reservation, and lookup APIs.
- [Rayon `ParallelSlice` documentation](https://docs.rs/rayon/latest/rayon/slice/trait.ParallelSlice.html) — fixed chunked parallel slice processing.
- [Criterion documentation](https://docs.rs/criterion/latest/criterion/index.html) — statistical microbenchmarking.
- [wgpu `Limits` documentation](https://docs.rs/wgpu/latest/wgpu/struct.Limits.html) — adapter-provided dispatch and device resource limits.
