# Kata work item: one exact planner for every image operation

**Status:** proposed. This document is the complete plan and source audit for
the Kata work item. It describes work to do; it does not claim implementation,
parity, or performance completion.

## Goal

Plan and execute every existing `PipelineOp` through one shared semantic
execution path. A one-operation request must use the same planner, descriptor,
backend kernel, and memory rules as a long pipeline. CPU and SIMD should share
the host plan. GPU should lower that same semantic plan into a device-specific
plan because uploads, dispatches, device buffers, synchronization, and readback
have different costs.

Keep the existing optimized operation kernels. The plan decides where and how
to call them; it must not create a second copy of an algorithm just for planned
pipelines. Every operation must remain callable alone and must also be
representable in a multi-operation plan. Fusion is optional and legal only
when the fused result has the same Pillow-visible behavior as executing each
operation in sequence.

The work is complete only when exact Pillow parity remains intact and the
performance goals below are met per operation and mode. A better aggregate
score cannot hide an operation regression.

### Quick version

- Every operation becomes a node in the same plan, even when it runs alone.
- CPU and SIMD share the host plan; GPU translates it into device stages.
- The plan reuses existing kernels and combines operations only when the
  combined result is exactly the same.
- Buffers are reused after their last reader; otherwise the plan keeps a safe
  copy and records the cost.
- A faster result counts only after exact Pillow parity and proof that the
  requested backend actually ran.

## Non-negotiable rules

1. **Pillow parity first.** Pixels, mode, palette and metadata behavior,
   dimensions, exceptions, and ordering effects must match the selected Pillow
   contract. Do not weaken assertions or drop failing cases to accept a speedup.
2. **No benchmark-only fast paths.** Do not specialize for a fixture identity,
   benchmark name, or one known image size. Dispatch may use real properties
   such as mode, stride, dimensions, vector width, device limits, and ownership,
   but the branch must be correct and beneficial beyond the benchmark that
   exposed it.
3. **Keep native modes.** Do not convert all input to RGBA to share a kernel.
   A mode change is explicit plan state and a barrier unless an exact combined
   kernel proves otherwise.
4. **One kernel implementation per backend.** Standalone and planned calls
   reach the same registered CPU, SIMD, or GPU implementation. Existing public
   functions may remain as thin one-node-plan entry points during migration;
   they must not maintain a second pixel algorithm.
5. **Reuse only when ownership proves it safe.** A writable buffer may be
   reused when it is exclusively owned, its prior value has no remaining
   consumer, and its layout and capacity meet the next node's contract. A
   shared branch, caller-owned source, or secondary input stays immutable.
6. **Make fallback truthful.** A CPU operation between SIMD segments is a CPU
   segment; a host result carried through a GPU copy is not a GPU operation.
   Receipts must identify actual execution, copies, allocations, transfers,
   and fallback reasons.
7. **Keep backend boundaries.** Serial CPU, Parallel CPU, SIMD, and GPU remain
   separately measured profiles. Rayon is for the opt-in Parallel CPU profile;
   GPU dispatch never uses Rayon. GPU batching remains explicit and keeps its
   resource leases through completion.

## Source review and current implementation

The review followed the public enum through its key mapping, backend
registration, operation support checks, executors, and existing roadmap:

- [`PipelineOp`](../pillow-rs/src/pipeline.rs) defines 84 operations.
- [`registry.rs`](../pillow-rs/src/compute/registry.rs) maps them to 81 keys.
  `ResizeBoxed` shares `Resize`, `BoxBlurXY` shares `BoxBlur`, and
  `EqualizeMasked` shares `Equalize`. Keep these aliases visible in planning
  metadata; a shared key does not mean the variants have identical parameters
  or contracts.
- The registration table connects every key to a CPU implementation and, when
  the `gpu` feature is built, a shader entry. It also attaches a SIMD adapter to
  every key. This proves registration, not support for every mode or proof that
  a particular invocation ran on that backend.
- CPU is the universal materialized fallback. Its implementations are grouped
  in the `pool_cpu/ops` modules listed in the operation ledger below.
- SIMD has a contextual gate in
  [`adapters.rs`](../pillow-rs/src/compute/pool_simd/ops/adapters.rs): it checks
  the current image, logical mode, data layout, dimensions, parameters, and
  secondary operands. Automatic host routing in
  [`compute/mod.rs`](../pillow-rs/src/compute/mod.rs) splits at conservative
  shape/mode boundaries. It avoids Python-level materialization, but it still
  produces host intermediates at operation or segment boundaries.
- [`pool_simd/mod.rs`](../pillow-rs/src/compute/pool_simd/mod.rs) already has
  targeted point-LUT fusion, transpose composition, selected two-op kernels,
  and exclusive-owned-buffer in-place paths. Those exact fast paths must be
  retained and represented as legal planner lowerings, not replaced by a
  generic slow implementation.
- The GPU executor already lowers pointwise chains, transpose chains, crop
  borders, `Multiply -> Screen`, and `AlphaComposite -> Mirror`; it also
  expands geometry wrappers to their existing resize/placement plans. The
  plan must retain these rules and the specialized packed filter/color paths.
- [`pool_cpu/mod.rs`](../pillow-rs/src/compute/pool_cpu/mod.rs) has targeted
  batching/fusion for draw and pixel writes, point LUTs, transpose, and selected
  multi-image chains. It currently dispatches a batch incrementally rather
  than consuming the same typed plan as SIMD and GPU.
- [`pool_gpu/mod.rs`](../pillow-rs/src/compute/pool_gpu/mod.rs) lowers and
  validates GPU work, while [`pool_gpu/stream.rs`](../pillow-rs/src/compute/pool_gpu/stream.rs)
  has a separate bounded stage plan, auxiliary inputs, and resource accounting.
  The GPU has ping-pong buffers and reusable resources. Its operation registry
  and broad safety defaults are not, by themselves, proof that a public op
  completed natively; image-aware preflight and the final execution receipt
  decide that. Strict GPU results that use exact host semantics must remain
  marked as fallback.
- [`PIPELINE_ROADMAP.md`](PIPELINE_ROADMAP.md) already tracks related work in
  FIL-03, 12–23, 26–31, 34, 37–56, 61–63. This plan groups those items into one
  architecture and delivery sequence; it does not mark any existing item
  complete.

The operation inventory is complete for the current 84 variants. The module
and capability findings above are source-based. They do not establish exhaustive
mode-by-mode kernel proof or current performance: the worktree already contains
uncommitted code, benchmark, and documentation changes. The published
per-operation matrix is useful for ranking, but it is not a fresh benchmark of
this dirty tree and it does not prove native execution unless its row has an
eligible parity/backend receipt.

### Current fast-path gaps this plan addresses

- Backend selection and shape/mode segmentation happen in separate places;
  the GPU has its own planner. There is no one typed plan that records
  semantics, backend eligibility, intermediate layouts, aliasing, fusion
  legality, and buffer lifetimes for every operation.
- The SIMD executor has a single current image plus specific fusion and
  in-place rules. This is useful reuse, but it is not a general liveness plan
  across all operations or branches.
- The CPU executor has valuable special batches, but the registration
  signatures mostly return new `DynamicImage` values. General host scratch
  lifetime planning and destination-buffer kernels are therefore still
  required for broad reuse.
- The GPU stage plan has device-specific resource accounting, but its lowering
  rules are not derived from the same semantic operation descriptor used by
  CPU/SIMD. Keep this device plan; make it a backend lowering of shared
  semantics rather than trying to force GPU buffer rules into the host plan.
- A registration, a supported result from an operation-only query, a real
  kernel, and a no-fallback completed execution are different states. The new
  plan and receipts must keep those states distinct.

## Target architecture

```text
Existing lazy Image graph / PipelineOp list
                    |
                    v
        Shared semantic planner (one op is a one-node plan)
        - operation contract and parameters
        - input/output mode, layout, dimensions
        - reads, writes, side inputs, and last-use information
        - exact fusion rules and hard barriers
        - backend eligibility and fallback reason
                    |
          +---------+----------+
          |                    |
          v                    v
   Host plan             GPU lowering plan
   CPU / SIMD             device stages, buffers,
   serial / parallel      uploads, dispatches, fences,
   scratch lifetimes      readback, terminal sink
          |                    |
          +---------+----------+
                    v
      Existing backend operation kernels and receipts
```

### Shared semantic planner

Add one internal typed operation contract derived from the existing
`PipelineOp`, not a second public operation enum. For each variant it must
describe or compute:

- input count and whether it reads a secondary image or mask;
- mode and native-layout constraints;
- output dimensions, mode, and pixel layout;
- whether the node is pointwise, a reduction, neighborhood, geometry,
  copy-like, multi-image, ordered mutation, or random/stateful;
- read-only, writable, and alias rules;
- temporary/scratch requirements and valid reuse layouts;
- parameter and contextual capability checks for each backend;
- exact fusion families and barriers;
- a conservative work/traffic estimate used to compare a fusion with separate
  stages.

This metadata must have one canonical mapping for all 84 variants. Add a
completeness check so a new `PipelineOp` cannot silently lack a planner
contract, CPU function, or declared backend capability. Keep existing
operation-specific validation and kernels as the semantic source of truth;
the descriptor must call or reuse them rather than reimplementing them.

The planner should keep original public operations visible for diagnostics and
per-operation benchmarking, even when it lowers a run into fewer kernels.
Lowering may add internal nodes, but receipts must map each internal dispatch
back to the public operation(s) it implements.

### Adapter and module boundaries

Keep the top-level adapter files as thin dispatch facades. They should route an
operation family and execution context to the existing family adapter, not
accumulate per-operation pixel algorithms, duplicate mode checks, or repeat
backend capability policy. Put reusable kernels and family-specific admission
rules in focused modules such as geometry, filters, color, pointwise/binary,
draw, and terminal operations.

Use one canonical operation-contract registry for operation identity,
parameters, and cross-backend semantics. Backend adapters should consume that
contract and add only backend-specific eligibility and lowering. Prefer a
shared family handler or a declarative registration entry over parallel
handwritten operation-by-operation switches. Keep explicit per-variant data
only where the contract differs, and make the completeness check prove every
variant is registered once. Do not introduce an adapter trait or another
dispatch layer unless it removes duplicated policy without adding a measurable
hot-path cost.

### CPU and SIMD host plan

CPU and SIMD consume the same semantic nodes and host buffer-lifetime plan.
Each node selects an existing backend kernel through the common dispatcher.
The executor can fuse legal nodes or split at barriers without changing the
single-operation entry point. Serial CPU and opt-in Parallel CPU share the
same plan and operation semantics; their scheduling and measurements remain
separate. SIMD uses its real vector adapters, vector blocks, and scalar tails;
it must not be used as a label for Rayon work.

During migration, change a kernel to write into a leased destination buffer
only when that unlocks reuse or fusion. The standalone path allocates its
destination and invokes the same kernel body. The planned path leases that
destination from the arena. This avoids separate `execute_single` and
`execute_plan` algorithms.

### GPU lowering

The shared planner provides semantic facts, dependencies, and legal fusion
contracts. A GPU lowerer then chooses GPU-specific stages, workgroups,
pipelines, bind groups, parameter storage, auxiliary-image storage, queue
submissions, fences, and readback. Preserve explicit `GpuBatchExecutor`
admission, identity-keyed results, byte bounds, and resource leases. Keep
intermediate image data resident on device while the plan remains GPU-eligible.

An unsupported node splits to an explicit exact host segment only in an
automatic policy that permits fallback. A strict GPU request reports a
capability/fallback outcome honestly and preserves exact public semantics. Do
not route a GPU stage through Rayon or call a host-computed result “GPU
accelerated.”

### Buffer and memory plan

Build last-use information before execution. A buffer can be recycled only
after its final reader and only when its owner is exclusive and its concrete
layout/capacity can satisfy the next writer. Keep distinct pools for host and
device memory. The host scratch arena is bounded and mode/layout-aware; the GPU
continues using bounded device pools and leases.

The objective is to remove avoidable full-frame intermediates, not to promise
zero allocation for every chain. A materialized result needs storage; a
shape-changing operation may need a new destination; branched or caller-shared
images cannot be overwritten safely. Where possible, the last node should
write directly to the requested output/sink so no extra terminal copy is
needed.

Track two different events explicitly:

- **Backend fallback:** an operation planned for SIMD/GPU runs on CPU.
- **Buffer fallback:** an operation cannot reuse a live buffer and needs a new
  full-frame allocation/copy.

They have different causes and must not be collapsed into one receipt field.

## Fusion rules

The planner should choose a fused path only when the full semantic contract is
proven. Otherwise it executes the original sequence, with safe buffer reuse
between nodes where possible.

| Operation class | Planning rule |
| --- | --- |
| Pointwise transforms and LUTs | Compose mappings or run one vector/GPU pass when each intermediate's byte rounding, channel handling, and mode semantics are preserved. Do not materialize a full image between legal point ops. |
| Global statistics and histograms | Keep the reduction/control pass explicit; share a histogram/LUT only for nodes that consume the same exact input and mask. Apply a resulting LUT in a following fused point pass. |
| Neighborhood filters | Plan halo/tile or rolling-row scratch; preserve Pillow edge extension, coefficients, accumulation, and rounding. Combine adjacent neighborhoods only with an operation-specific equivalence proof. |
| Geometry and resampling | Compose exact coordinate transforms and cancel exact identities. Do not collapse sequential resizes or a resize plus crop merely because the algebra looks equivalent; intermediate resampling/rounding can change pixels. |
| Copy-like geometry | Use source views/offsets as planner metadata when the next kernel accepts them; otherwise write one correctly sized output. Compose orientations and offsets when exact. |
| Binary/multi-image operations | Keep every input as an explicit read dependency; reuse a materialized secondary input across consumers; fuse same-layout arithmetic when exact. Do not mutate an input that has another reader. |
| Ordered draw and write operations | Batch compatible commands into one mutable canvas while preserving call order, clipping, blend rules, and the existing rasterizer's behavior. |
| Mode-changing operations | End the current layout segment and update mode/layout metadata. Cross the barrier only through a kernel that explicitly supports both sides. |
| Random or stateful effects | Preserve RNG stream, seed, call order, and side effects. Do not reorder or merge random operations unless exact output equivalence is demonstrated. |
| Terminal operations | Allow direct output/sink writes and avoid an otherwise unnecessary materialize/copy/readback. |

## Operation-by-operation implementation ledger

**Backend notation for every row:** CPU has a registered core implementation.
SIMD has an adapter registered for the operation's shared dispatch key; the
contextual image/mode/parameter check still decides whether that invocation is
native. GPU has a shader registration for the shared key when the `gpu`
feature is enabled; only a completed receipt with the requested backend and no
fallback proves that invocation ran natively. These statements apply to every
row below; they are intentionally not inferred from a shader filename.

| `PipelineOp` | Current CPU owner | Planned treatment and required change |
| --- | --- | --- |
| `Resize` | `pool_cpu/ops/resize.rs` | Resampling node with explicit source/output layouts and coefficient/scratch lifetimes. Reuse exact horizontal/vertical kernels; do not fuse two resizes unless the intermediate rounding contract is preserved. |
| `ResizeBoxed` | `pool_cpu/ops/resize.rs` | Keep public box bounds and float32 decisions in its contract; lower to the existing resize machinery only after exact bound conversion. Reuse scratch with `Resize`. |
| `Crop` | `pool_cpu/ops/crop.rs` | Copy/view node with a source rectangle. Let compatible following kernels consume the rectangle directly; otherwise allocate one exact crop result. Treat terminal ownership separately. |
| `Rotate` | `pool_cpu/ops/rotate.rs` | Lower exact right-angle cases to the existing orientation path and arbitrary angles to transform/resample stages; preserve fill, center, expansion, and rounding rules. |
| `Transpose` | `pool_cpu/ops/transpose.rs` | Keep current D4 composition/cancellation. Make composed orientation a plan lowering and reuse its destination buffer rules. |
| `Thumbnail` | `pool_cpu/ops/thumbnail.rs` | Represent resize/reduce decisions in one semantic node. Avoid an intermediate only where the existing Pillow rounding and intermediate-size behavior are unchanged. |
| `Reduce` | `pool_cpu/ops/reduce.rs` | Shape-changing reduction node. Carry factors, partial edge blocks, alpha behavior, and exact fixed-point semantics to all backends. |
| `Contain` | `pool_cpu/ops/imageops.rs` | Geometry wrapper lowered to the existing checked size/resize path. Preserve aspect-ratio and dimension rounding; do not add a second resize algorithm. |
| `Cover` | `pool_cpu/ops/imageops.rs` | Geometry wrapper plus exact crop. Plan the crop offset with resize so an unnecessary full-size intermediate can be avoided only when output pixels remain exact. |
| `Fit` | `pool_cpu/ops/imageops.rs` | Keep fractional crop/centering/bleed in the descriptor; lower to existing resize/crop kernels only after exact coordinate proof. |
| `Pad` | `pool_cpu/ops/imageops.rs` | Plan resize and placement/fill as one output-writing stage where exact; allocate the final canvas once. Preserve fill mode and anchor semantics. |
| `Scale` | `pool_cpu/ops/imageops.rs` | Compute ties-to-even dimensions once in the semantic plan, then reuse the existing resize kernel. No separate planned scale implementation. |
| `Expand` | `pool_cpu/ops/imageops.rs` | Track new dimensions and fill placement; use one output allocation and fuse the placement into a compatible geometry writer. |
| `CropBorder` | `pool_cpu/ops/imageops.rs` | Treat as a checked copy/view rectangle; preserve Pillow errors for invalid borders before backend selection. Fuse with a following compatible read when possible. |
| `Transform` | `pool_cpu/ops/effects.rs` | Keep affine/perspective/quad/mesh method, fill, pixel-center, and palette behavior explicit. Use the current exact sampler and GPU admission proof; this is a hard fusion boundary without a proven coordinate composition. |
| `Filter3x3` | `pool_cpu/ops/filter/filter3x3.rs` | Neighborhood node with kernel/scale/offset and exact boundary/rounding policy. Reuse the rolling/tiled SIMD and proven GPU paths; capability must match parameter proof. |
| `Filter5x5` | `pool_cpu/ops/filter/filter5x5.rs` | Same contract as `Filter3x3` with a larger halo and scratch footprint. Tile by output region without changing accumulation order. |
| `GaussianBlur` | `pool_cpu/ops/filter/blur.rs` | Represent exact separable passes and their intermediate lifetimes. Reuse current rolling CPU/SIMD/GPU components; avoid allocating a full frame per pass when a compatible scratch slot is free. |
| `BoxBlur` | `pool_cpu/ops/filter/blur.rs` | Same neighborhood plan, including edge and radius behavior. Lower to existing horizontal/vertical code only when the discrete radius contract matches. |
| `BoxBlurXY` | `pool_cpu/ops/filter/blur.rs` | Store independent radii and pass count; schedule the existing separable kernels as one operation with leased scratch. Preserve exact pass order. |
| `MedianFilter` | `pool_cpu/ops/filter/rank.rs` | Order-statistic node. Keep size/mode-specific kernels and current SIMD/GPU specializations; reuse row windows/tiles instead of rescanning and reallocating. |
| `MaxFilter` | `pool_cpu/ops/filter/rank.rs` | Neighborhood reduction node. Share window/tile machinery with min/max only when semantics match; retain mode and border behavior. |
| `MinFilter` | `pool_cpu/ops/filter/rank.rs` | Same as max with exact min semantics; consider paired min/max only if both outputs are requested and share the same input. |
| `RankFilter` | `pool_cpu/ops/filter/rank.rs` | Order-statistic node with rank as a required parameter. Share selection/window infrastructure without changing tie, border, or rank behavior. |
| `Autocontrast` | `pool_cpu/ops/imageops.rs` | Global histogram/range-control plus LUT application. Keep mask/cutoff and per-band rules; fuse the remap with adjacent point ops after its LUT is known. |
| `Equalize` | `pool_cpu/ops/imageops.rs` | Histogram plus LUT node. Reuse current CPU/SIMD/GPU histogram improvements; compute only required statistics and write the final remap directly when legal. |
| `EqualizeMasked` | `pool_cpu/ops/imageops.rs` | Preserve the mask as a first-class dependency, not a validation-only value. Reuse the masked histogram/LUT path and permit result reuse only after all mask readers finish. |
| `Invert` | `pool_cpu/ops/imageops.rs` | Pointwise node; include in exact LUT/kernel composition and owned-buffer in-place execution. Retain the current SIMD vector path. |
| `Flip` | `pool_cpu/ops/imageops.rs` | Copy-like vertical orientation. Fuse with a compatible output traversal or compose with transpose/rotate; do not overwrite a shared source. |
| `Mirror` | `pool_cpu/ops/imageops.rs` | Copy-like horizontal orientation. Keep current alpha-composite/mirror opportunities and row-layout requirements in the planner. |
| `Posterize` | `pool_cpu/ops/imageops.rs` | Pointwise byte mapping. Compose with adjacent legal point maps and reuse an exclusive destination; preserve invalid direct-parameter error behavior. |
| `Solarize` | `pool_cpu/ops/imageops.rs` | Pointwise threshold mapping. Fuse into LUT/vector pass and preserve equality behavior at the threshold. |
| `Grayscale` | `pool_cpu/ops/imageops.rs` | Mode-changing color transform. Plan source/destination channel layouts explicitly and fuse only when downstream accepts its exact output mode. |
| `Colorize` | `pool_cpu/ops/imageops.rs` | L-to-color LUT transform. Treat palette/fill points and resulting mode as metadata; share LUT preparation across backends without duplicating pixel logic. |
| `Convert` | `pool_cpu/ops/color.rs` | Explicit mode barrier. Carry matrix/dither and source/target mode in the node; preserve native formats and reuse conversion kernels only for matching contracts. |
| `ConvertLab` | `pool_cpu/ops/color.rs` | Color-space conversion barrier with its exact LUT/LCMS-derived inputs. Share prepared immutable tables; preserve channel and output-mode semantics. |
| `RemapPalette` | `pool_cpu/ops/color.rs` | Index-byte point map plus palette metadata update. Plan sample mapping and metadata together; reuse the current in-place SIMD path only for exclusive storage. |
| `ExtractBand` | `pool_cpu/ops/color.rs` | Mode/channel-changing gather. Fuse channel extraction into a compatible consumer or write one output plane; never expand to RGBA just to extract. |
| `Add` | `pool_cpu/ops/chops.rs` | Binary pointwise node with a second image dependency and affine parameters. Fuse chains when layouts match and use last-use analysis before overwriting the primary input. |
| `Subtract` | `pool_cpu/ops/chops.rs` | Same binary plan as Add, retaining subtract-specific rounding/clamping. Reuse the affine/vector path only when parameter proof passes. |
| `Multiply` | `pool_cpu/ops/chops.rs` | Binary pointwise node. Preserve the existing Multiply → Screen fusion; store secondary-image lifetime and avoid duplicate materialization. |
| `Screen` | `pool_cpu/ops/chops.rs` | Binary pointwise node. Fuse with Multiply when the current exact contract applies; otherwise dispatch the existing standalone kernel. |
| `Darker` | `pool_cpu/ops/chops.rs` | Binary comparison/select node. Share same-layout binary traversal with Lighter without changing tie behavior or mode handling. |
| `Lighter` | `pool_cpu/ops/chops.rs` | Binary comparison/select node, paired with Darker only if both outputs are demanded. Keep source inputs read-only when they have further consumers. |
| `Difference` | `pool_cpu/ops/chops.rs` | Binary pointwise absolute difference. Schedule alongside other same-layout binary ops; preserve channel/mode validation. |
| `Overlay` | `pool_cpu/ops/chops.rs` | Binary pointwise transform. Reuse existing LUT/fixed-point backend kernels and table resources; no extra full-frame LUT output. |
| `HardLight` | `pool_cpu/ops/chops.rs` | Binary pointwise transform sharing table preparation with overlay where exact; preserve operand-order semantics. |
| `SoftLight` | `pool_cpu/ops/chops.rs` | Binary pointwise transform with exact table/formula semantics. Keep its current optimized formula and parameter preparation. |
| `AddModulo` | `pool_cpu/ops/chops.rs` | Binary modular arithmetic. Use the shared binary traversal; do not silently substitute saturating Add. |
| `SubtractModulo` | `pool_cpu/ops/chops.rs` | Binary modular arithmetic with its own wrap behavior. Reuse the binary scheduler, not Add's semantics. |
| `LogicalAnd` | `pool_cpu/ops/chops.rs` | Binary logical node. Keep sample-domain rules and use the common binary layout/alias contract. |
| `LogicalOr` | `pool_cpu/ops/chops.rs` | Binary logical node. Share traversal only; preserve mode validation and output bytes. |
| `LogicalXor` | `pool_cpu/ops/chops.rs` | Binary logical node. Share traversal only; retain exact xor and tail handling. |
| `Constant` | `pool_cpu/ops/chops.rs` | Unary fill node. Fuse fill into an output-producing neighbor or sink when exact; avoid allocating a separate constant full frame when it is only a temporary operand. |
| `Offset` | `pool_cpu/ops/chops.rs` | Wrapped relocation node. Carry offset and wrapping semantics as view/index metadata when a consumer supports it; otherwise one output pass. |
| `Duplicate` | `pool_cpu/ops/chops.rs` | Explicit copy/ownership node. Elide only when API identity and branch semantics permit; otherwise copy exactly once and report it. |
| `InvertChops` | `pool_cpu/ops/chops.rs` | Pointwise invert variant. Reuse the current SIMD kernel and pointwise fusion while preserving its distinct operation identity in receipts. |
| `Brightness` | `pool_cpu/ops/enhance.rs` | Pointwise blend with black. Compose exact byte mapping and reuse exclusive host buffers; retain current factor/rounding behavior. |
| `Contrast` | `pool_cpu/ops/enhance.rs` | Global midpoint/control plus pointwise blend. Keep reduction dependency separate from remap and share the scalar control result only for identical input. |
| `ColorSaturation` | `pool_cpu/ops/enhance.rs` | Color point transform with mode-aware luma/control. Track channel semantics and reuse exact vector/LUT paths where supported. |
| `Sharpness` | `pool_cpu/ops/enhance.rs` | Neighborhood smooth plus blend. Plan required source preservation and scratch; use existing mode-specific SIMD/GPU kernels and exact integer blend. |
| `EffectSpread` | `pool_cpu/ops/effects.rs` | Randomized spatial gather. Preserve RNG state/order and exact mapping; keep generated map as a leased auxiliary input if multiple backend stages consume it. |
| `Paste` | `pool_cpu/ops/effects.rs` | Ordered multi-image canvas mutation. Preserve mask, offset, crop, and alpha rules; reuse the destination only when exclusive, as in the current SIMD path. |
| `AlphaComposite` | `pool_cpu/ops/effects.rs` | Two-image alpha node. Carry source/destination offsets and premultiplication/rounding contract; retain current composite→mirror opportunities and deduplicate source materialization. |
| `Merge` | `pool_cpu/ops/effects.rs` | Multi-plane mode construction. Keep each plane as a live dependency; allocate the final native mode once and do not narrow unsupported typed/palette modes. |
| `BlendModule` | `pool_cpu/ops/effects.rs` | Two-image weighted pointwise blend. Carry alpha and both image layouts; fuse with a following point node only when intermediate rounding matches. |
| `CompositeModule` | `pool_cpu/ops/effects.rs` | Three-input image/mask node. Track all three lifetimes and mask mode; retain current GPU multi-input context checks. |
| `Eval` | `pool_cpu/ops/effects.rs` | LUT point transform. Compose adjacent LUTs and reuse the current SIMD/GPU table preparation; keep one output traversal. |
| `EffectNoise` | `pool_cpu/ops/effects.rs` | Randomized point transform. Preserve the RNG sequence and seed behavior; do not reorder, fold, or regenerate random values during planning. |
| `Color3DLut` | `pool_cpu/ops/effects.rs` | Table-backed color point transform. Prepare/share the immutable LUT once, track source/target modes, and keep interpolation/rounding identical. |
| `PutPixel` | `pool_cpu/ops/effects.rs` | Ordered single-pixel canvas write. Batch consecutive writes into one canvas and keep validation before mutation; retain the current PutPixel→Thumbnail optimization where exact. |
| `PutData` | `pool_cpu/ops/effects.rs` | Full image-data replacement and layout boundary. Treat replacement bytes as the output source; avoid copying them through an obsolete prior image. |
| `PutAlpha` | `pool_cpu/ops/effects.rs` | Mode/channel-changing write. Plan alpha promotion plus pixel fill in one output pass; preserve P/PA/CMYK reinterpretation rules. |
| `PutAlphaData` | `pool_cpu/ops/effects.rs` | Multi-image alpha write. Track mask lifetime and channel layout; combine promotion and mask copy in one exact output-writing kernel. |
| `DrawLine` | `pool_cpu/ops/draw.rs` | Ordered raster command. Keep the canonical Pillow-compatible rasterizer; batch with other draw commands on one mutable canvas and preserve RGB alpha blending. |
| `DrawRectangle` | `pool_cpu/ops/draw.rs` | Ordered canvas command. Batch fill/outline into the shared canvas while retaining edge/width and degenerate-rectangle behavior. |
| `DrawRoundedRect` | `pool_cpu/ops/draw.rs` | Ordered canvas command with radius. Share the canvas, not a different scan converter; preserve radius clipping and outline semantics. |
| `DrawEllipse` | `pool_cpu/ops/draw.rs` | Ordered canvas command. Reuse ellipse span generation and avoid per-command full-image copies. |
| `DrawCircle` | `pool_cpu/ops/draw.rs` | Ordered canvas command. Lower through current ellipse/circle spans while preserving inclusive bounds and clipping. |
| `DrawPolygon` | `pool_cpu/ops/draw.rs` | Ordered canvas command with point list. Keep point storage alive for the plan and preserve Pillow fill/raster order; no repeated canvas materialization. |
| `DrawArc` | `pool_cpu/ops/draw.rs` | Ordered canvas command with angle normalization. Reuse current arc geometry/control and share the canvas buffer. |
| `DrawChord` | `pool_cpu/ops/draw.rs` | Ordered canvas command. Preserve normalized angles and fill/outline order in the batch. |
| `DrawPieslice` | `pool_cpu/ops/draw.rs` | Ordered canvas command. Preserve center/arc/outline behavior and write through the common mutable canvas. |
| `DrawPoint` | `pool_cpu/ops/draw.rs` | Ordered point/sequence command. Batch coordinates with other writes only where order and duplicate-point behavior are unchanged. |

## Delivery plan

### K0 — Lock the full inventory and truth model

- Keep the 84-operation ledger above synchronized with `PipelineOp` and the
  81 dispatch keys.
- Add a completeness check for enum → key → CPU implementation → SIMD adapter →
  GPU registration/capability metadata. Shared keys are explicitly tested as
  aliases, not omitted.
- Define receipt states separately: registered, parameter-valid, image/mode
  eligible, lowered to a real backend kernel, completed, and fell back.
- Map each `PipelineOp` to its public API and canonical benchmark workloads.
  Record unmapped APIs and unbenchmarked variants as gaps; do not treat the
  current 330-path public API census or its 205 `pipeline-op` workloads as
  proof that every core variant/mode/backend is covered.
- **Done when:** every enum variant appears exactly once in the ledger and
  descriptor map, alias mappings are explicit, and the benchmark/report tool
  cannot label shader registration or a fallback as native execution.

### K1 — Add the shared operation contract and one-node plan

- Add typed input/output layout, side-input, mutability/alias, shape/mode,
  scratch, and fusion metadata for all 84 operations.
- Add a semantic `ExecutionPlan` that references the existing `PipelineOp`s;
  initially each op may lower to one node with no fusion.
- Route materialized pipelines and standalone operations through the same
  plan entry point. Keep existing API behavior and reuse the current registry
  implementation behind the node dispatcher.
- Keep operation-level validation/error behavior before any backend writes.
- **Done when:** one-op plan output and error behavior match the existing
  standalone route for every operation and current CPU/SIMD/GPU operation
  implementations remain the called kernels.

### K2 — Share the host plan across serial CPU, Parallel CPU, and SIMD

- Implement a host scheduler over the shared plan. CPU and SIMD use the same
  planned nodes, dependencies, boundaries, and host buffer-lifetime decisions.
- Preserve the current automatic SIMD contextual gate and CPU segment routing;
  express its boundary decisions in the plan and receipts.
- Keep Parallel CPU as its own opt-in scheduler/profile using the same semantic
  plan. Do not turn SIMD into a thread-pool profile.
- Keep current exact CPU/SIMD special fusions and SIMD vector/scalar-tail
  handling as lowering rules.
- **Done when:** a one-node plan and an N-node plan call the same backend
  adapters and cannot drift in algorithms or capability checks.

### K3 — Add host liveness and reusable scratch

- Compute last use for primary and secondary images before executing the
  segment.
- Add bounded reusable host buffers keyed by native layout, capacity, and
  alignment. Reuse them only after exclusive last use.
- Migrate high-traffic kernels to `execute_into`/view-style destinations one
  family at a time. Standalone execution allocates a destination and calls
  the same kernel; a plan leases its destination.
- Preserve shared lazy branches and caller-owned inputs. Report allocation
  count/bytes, copies, peak live bytes, and reuse hits separately.
- **Done when:** repeated eligible point/draw/write chains avoid avoidable
  full-frame buffers, while alias/branch tests show source images are unchanged.

### K4 — Add exact host fusion by operation family

Implement and measure in this order; after each family, preserve its parity
and performance checkpoint before moving on:

1. Point/LUT: invert, posterize, solarize, brightness, `Eval`, palette remap,
   and compatible color maps.
2. Copy/orientation/simple geometry: crop, mirror, flip, transpose, offset,
   duplicate, expand, crop-border, and exact right-angle rotate.
3. Binary/multi-image and mutations: Chops, paste, alpha composite, blend,
   composite, merge, put-pixel/data/alpha.
4. Reductions and neighborhoods: autocontrast/equalize, filters, blurs,
   sharpness, and resize/thumbnail/reduce.
5. Mode/color changes, transforms, random effects, and ordered drawing.

Do not make an operation wait for fusion to use a plan: all operations from K1
already execute as one-node or unfused multi-node plans. This order only
controls optimization work.

### K5 — Lower the shared plan into the GPU device plan

- Replace parallel semantic decisions with GPU lowering from shared operation
  contracts; keep GPU-specific stage and resource planning.
- Retain device-resident intermediates, ping-pong buffers, bounded buffer and
  parameter arenas, auxiliary image reuse, exact per-node mode/shape tracking,
  and direct terminal sinks.
- Model transfer, queue submission, synchronization, readback, device-loss,
  and strict-versus-automatic fallback explicitly.
- Keep GPU capability proof contextual and reconcile it with actual completed
  receipts before accepting an optimization.
- **Done when:** eligible pipelines upload once and read back once at the
  terminal boundary, intermediate stages remain on GPU, and any host fallback
  is explicit and excluded from native-GPU speed claims.

### K6 — Turn parity and performance into closure gates

- Add/retain input-only cases for modes, parameter boundaries, odd sizes,
  vector tails, empty/tiny images, neighborhood borders, aliases/branches,
  masks, invalid inputs, and chained operations.
- Compare every candidate against live Pillow with exact image and metadata
  observations on the matched workload. Keep backend-locked parity lanes and
  receipts; no fixture-specific runtime behavior.
- Benchmark both isolated operation kernels and complete call/materialize
  pipelines. Report allocations, copies, host/device bytes, dispatches,
  queue/synchronization cost, fallbacks, and sustained throughput separately.
- Use paired workloads on matching runners, multiple repetitions, varied
  distributions, and full declared mode/parameter/size coverage. Fixed-size
  results may diagnose a bottleneck but cannot close a broad operation.
- Keep Parallel CPU separately reported with its feature config and Pillow
  comparison. Do not merge it into CPU or SIMD results.
- **Done when:** correctness is exact, no measured per-operation regression is
  hidden by an aggregate, receipts prove the claimed backend, and the project
  targets below are met over their declared applicability.

### K7 — Guard rollout and keep the roadmaps aligned

- Land in observable increments: operation coverage, one-node plan, host plan,
  buffer leases, family fusion, then GPU lowering.
- Preserve current per-operation parity thresholds and benchmark workflows.
- Update the existing FIL statuses only when the matching code and evidence
  are accepted; retain completed items and don't duplicate them as new “done”
  tasks.
- **Done when:** the existing roadmap and reports derive status from the same
  operation inventory and evidence, and unsupported/unmeasured work remains
  visible.

## Performance acceptance

The hard project targets in `CLAUDE.md` govern this work:

- **Serial CPU:** no operation/mode workload slower than Pillow.
- **SIMD:** at least 5× faster than Pillow for every applicable operation/mode
  workload. Intermediate 2× checkpoints can rank work, but do not close the
  5× requirement.
- **GPU:** latency matches or beats SIMD for the matched workload and sustained
  throughput is higher under a changing-input concurrent workload.
- **Parallel CPU:** report separately against Pillow with opt-in feature
  provenance; do not count it toward CPU, SIMD, or GPU goals.

`docs/PERFORMANCE_CAMPAIGN.md` currently describes 2× as an active SIMD gate
while the controlling project guide requires 5×. Resolve that documentation
and reporting mismatch before marking the full SIMD objective complete. Keep
the 2× value as an interim prioritization checkpoint only if the maintainers
retain it; it is not final acceptance for this work item.

For every performance claim, compare the exact same operation, mode, input,
parameters, output boundary, and runner. A row is eligible only when exact
parity passes, the requested backend actually completes without fallback, and
the receipt matches the source/build used for timing. Report each operation and
mode separately; do not accept a faster average if any required row regresses.
GPU single-request latency does not prove sustained throughput. A smaller
number of allocations is not a speedup unless the full matched benchmark also
improves.

## Risks and safeguards

| Risk | Safeguard |
| --- | --- |
| Fusion changes rounding because Pillow materializes intermediate values | Default to unfused nodes. Enable each fusion only after input-only parity cases prove exact values and metadata. |
| Reuse overwrites a branch or secondary input | Compute graph last-use and exclusive ownership; otherwise keep the input immutable and allocate. |
| Unified planning adds overhead to tiny one-op calls | Benchmark the one-node path separately. Keep metadata/static planning cheap and avoid dynamic cost search for a single node. |
| Broad operation metadata becomes a second semantic implementation | Descriptors report constraints and call current validation/kernels; pixel math remains in existing per-backend code. |
| CPU/SIMD abstraction hides scalar tails or CPU fallback | Record vector blocks, tails, per-node actual backend, and fallback reason. Keep true SIMD and Parallel CPU separate. |
| GPU registration is counted as GPU work despite host completion | Require terminal completion receipts with zero fallback and GPU byte/dispatch evidence. |
| GPU and host planner decisions drift | Share semantic contracts and alias/fusion legality; retain a device-specific lowering and test both against the same parity cases. |
| Fixed benchmark dimensions get special paths | Allow branches only for hardware/layout/parameter conditions with broad matrix evidence; forbid fixture IDs and single-size completion claims. |

## Existing roadmap crosswalk

This is an umbrella plan for already listed work, not a request to close or
recreate those items:

- **Inventory, contracts, and planning:** FIL-12, FIL-13, FIL-18, FIL-19,
  FIL-20, FIL-22, FIL-63, FIL-64.
- **Host ownership and intermediates:** FIL-03, FIL-07, FIL-09, FIL-14,
  FIL-15, FIL-16, FIL-17, FIL-21, FIL-36.
- **Operation families:** FIL-23, FIL-24, FIL-25, FIL-26, FIL-27, FIL-29,
  FIL-30, FIL-31, FIL-32, FIL-33, FIL-34, FIL-35.
- **SIMD/CPU planning:** FIL-37, FIL-38, FIL-39, FIL-40, FIL-41, FIL-42,
  FIL-43, FIL-44.
- **GPU planning and device lifetime:** FIL-45, FIL-46, FIL-47, FIL-48,
  FIL-49, FIL-50, FIL-51, FIL-52, FIL-53, FIL-54, FIL-55, FIL-56, FIL-61.
- **Evidence and regression gates:** FIL-05, FIL-06, FIL-62.

No FIL status changes are implied by this plan. A status changes only when its
own stated scope has implementation, parity, and measurement evidence.
