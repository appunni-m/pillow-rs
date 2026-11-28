# CPU mode and architecture refactor plan

Status: active. The current source and evidence ledger is in
[CPU refactor inventory](CPU_REFACTOR_INVENTORY.md). Stage 5 source review
remains incomplete as of 2026-11-28. The exact repository file scanner finds
39 first-party Rust files at or above 800 lines, including eleven under
`pool_cpu`, and none over 1,000 (the largest is 993 lines); their source and
size review is tracked by owner in the inventory. Clippy-counted function-body
limits remain unverified. The current source pass moved Merge preparation into
its operation helper and split the architecture-policy and conversion-contract
dispatch functions; the Composite family entry is now 979 lines. These source
changes do not yet have their focused parity and benchmark evidence. On frozen
PostgreSQL snapshot `62db779f4eb483741937f4a6888bb113f97946dda69d816f7dd889a570043397`,
the 179,882-case Default and Neon corpus closures pass, the 970-case
architecture-selector closure passes on 13 selectors, and the 1,091-case
Filter closure passes on 14 platform/selector lanes. These runs do not close
the complete support-derived operation × Mode × Architecture denominator.
Reconcile that denominator with the generated input index, then verify all
required architecture routes and fallbacks. Native x86/AVX-512F execution,
Stage 5 source and size review, and the CPU benchmark gate remain pending.
Public `Image.transform` output, mode/filter/fill semantics,
callback/fault order, palette/metadata changes, and architecture/fallback
receipts remain frozen-parity work. Its built-in apply and MESH paths use the
prepared mode/architecture strategies. The separate low-level ImagingCore
region Transform is an internal boundary; assess it through whichever public
caller uses it, with no independent parity or benchmark contract. The vector
affine nearest fallback now treats non-finite mapped coordinates as invalid
before integer conversion, matching Default fill and validity behavior. The
input generator now adds an affine NaN nearest case for all 20 Pillow modes;
the generated, frozen corpus and Pillow comparison remain pending. Compare the
public result directly; internal implementation details do not define parity.
The Transform public-call source pass also found and corrected dropped
temporary P palettes from tuple fill colors when no nonempty source palette is
present; source/fill palette metadata parity remains pending.

2026-11-28 Filter architecture parity closure: the complete frozen 1,091-case
Filter selection passed on 14 platform/selector lanes (15,274 passes total,
zero mismatches, not-run cases, or infrastructure errors). The lanes include
ARM Default/AArch64/Neon/Wasm-selector and x86 Default/x86_64/SSE2/SSE3/SSSE3/
AVX/AVX2/AVX+FMA/AVX2+FMA/AVX-512F. Strict result validation and attestation
hash checks passed before each passing raw report was removed; the plan,
case IDs, compact summaries, and attestations remain in
`build/migration-parity/cpu-architecture/i32-filter-current-20261128/`.
One earlier SSE3 attempt failed during Postgres temporary-file hydration while
the VM filesystem was read-only; it emitted no final parity summary, is kept as
an infrastructure attempt, and is excluded from counts. The completed SSE3
retry passed. The Wasm selector was only a native-ARM fallback check, and the
AVX-512F selector was only a Haswell-QEMU fallback check; neither proves
execution on its named hardware. The bundle summary is
`filter-architecture-closure.summary.json` (run-plan SHA-256
`ed3e34e38608cb3041ebea090b2f95ddbe51d2a80f521116721d2a71ff5bc68f`). This
closes the current Filter parity slice, not the operation × Mode × Architecture
denominator or the complete CPU acceptance gate.

2026-11-28 F32 vector Resize verification: architecture-specific filter
resolution is now injected into SSE2 and non-FMA AVX vector paths, including
boxed execution. The 492-case F32 Resize/Thumbnail/pipeline closure passed on
ARM Default, AArch64, and Neon, and on x86 AVX. The broader 970-case
all-family closure passed on x86 SSE2 and AVX. A final 12-case closure reused
the frozen tall-F32 inputs (2x203 to 5x3, six filters, full and fractional
boxes) and passed on CPU Default, Neon, SSE2, and AVX after restoring the
vertical-first route. Total for that focused closure: 48/48 with no failed,
not-run, or infrastructure-error cases; the Neon execution receipts select the
vector strategy. Strict summaries, case hashes, and attestations are in
`build/migration-parity/cpu-architecture/{arm64-f32-vector-filter-resolver-20261128,x86-avx-f32-vector-filter-resolver-20261128,x86-sse2-avx-all-family-closure-20261128,f32-tall-two-axis-order-20261128}`. The combined function-marker bundle is
`f32-vector-closure-20261128` in `f32-tall-two-axis-order-20261128/f32-vector-closure.summary.json`.
The AVX build used `+avx,-fma` under Haswell QEMU; it does not establish native
x86 timing. AVX-512F direct execution remains unavailable under this host's
QEMU, and benchmark comparison remains pending. This focused verification does
not close Stage 5 source/size review or the full CPU completion gate.
The 2026-11-24 architecture-lowering sweep also found shared
`wide::f64x8::mul_add` in typed I;16 and F32 Resize. Their support tables now
separate compile-target AVX, AVX/FMA, and SSE2 strategies so the injected
architecture matches the selected arithmetic path. The adapters reuse the
mode's shared sample kernel. Runtime output, architecture receipts, target
build combinations, and latency remain pending. AVX-512F is now an injected
architecture detected at the routing boundary, with native 16-output F32
and I32 Reduce strategies behind complete-block guards. It also selects exact
F32 and I32 nearest Resize strategies for unboxed requests: a 16-lane gather
copies complete sample blocks and preserves output tails with scalar byte
copies. The Resize guard excludes identity, boxed, filtered, narrow-output,
and unrepresentable gather-index requests. Dedicated full-block, tail, and
narrow-width fallback parity outputs plus material nearest workloads are
declared in the input generator but have not been generated or run. These
kernels use function-level target-feature dispatch; crate-wide `+avx512f`
builds are rejected because
pinned `wide` changes shared vector representations and could emit AVX-512
instructions outside a selected strategy. Map the remaining active `wide`
paths to exact mode/architecture support, then verify outputs, receipts, and
latency on native AVX-512 hardware. I32 Reduce preserves signed wrapping
quartet accumulation and ROUND_UP; I-mode complete-block, fallback, wrapping,
and benchmark inputs are declared but have not been generated or run. Native
byte ImageOps Invert now also declares AVX-512F strategies for One, L, LA, P,
PA, RGB, RGBA, and CMYK, using 64-byte dword XOR blocks with scalar row tails
and selecting them only when each row contains a full vector block; trailing
bytes after the checked image prefix are preserved. The input generator now
defines below-block, full-block, and scalar-tail Pillow-comparable cases for L
and RGB; inputs remain ungenerated. Mode One boundary coverage and target-only
AVX boundary outputs for the `NO_PILLOW_SUPPORT` modes remain TODO. Per-mode
parity, hardware route receipts, and latency remain pending.
ImageChops Invert now has exact-mode AVX-512F strategies for all 14 Pillow
byte modes, I/F, the four I;16 storage modes, and the five internal typed
high-depth modes. Byte carriers use mode-owned adapters over the native-byte
leaf; typed u16/f32 samples use a separate 64-byte XOR leaf. Both retain exact
mode identity, hidden lanes, guarded selection, and the declared lower-strategy
or Default fallback. Boundary and material cases cover all 25 modes; the 11
CoreOnly modes use recorded expected outputs. Inputs remain ungenerated, and
parity, route receipts, native-hardware builds, and latency remain pending. F32 and I32 rank filters now have exact-mode AVX-512F
strategies for Min/Max/Median/Rank. Their guarded sixteen-lane gather and sort
paths preserve native samples; narrow rows, size-one windows, and oversized
gather indices use the declared fallback. F32 additionally falls back for
NaNs and mixed signed zeros. The generator defines 3x3 and 9x9 windows,
narrow-row fallbacks, F32 signed-zero fallback, and material noise workloads
for both modes. Inputs remain ungenerated; native route receipts, Pillow
parity, and latency are pending. Track remaining real AVX-512F opportunities
in `pillow-rs#fvsp`; byte strategies requiring AVX-512BW or VBMI do not count
as AVX-512F strategies. The
Transpose now also has a shared AVX-512F raw-dword gather path for RGBA, RGBa,
RGBX, CMYK, I, and F. It accepts only axis-swapping methods on images at least
16 pixels in both dimensions and within signed 32-bit gather indices; other
methods and shapes use their existing routes. The input generator defines
17x19 block/tail parity cases for Rotate90, Rotate270, Transpose, and
Transverse, 15x17 guard fallbacks, and 1024x768 workloads for all six modes.
Inputs remain ungenerated; parity and latency are pending. RGBA radius-seven
GaussianBlur now has a sixteen-pixel AVX-512F middle-block strategy using the
existing exact packed-dword recurrence. Both image dimensions must be at least
32 so both transposed passes can enter the kernel; smaller images keep their
declared lower architecture or Default route. Existing 1024x768 material and
noise workflows cover it, but AVX-512F parity and benchmark receipts are
pending.
LA and RGBA AlphaComposite now have exact AVX-512F strategies. The mode
adapters process 16 RGBA or 32 LA pixels per packed-dword block, retain Pillow's
fixed-point coefficient floor with integer correction around the F32 division
estimate, and use the existing scalar pixel routine for tails. Full-block guards
retain lower-strategy fallback. LA hidden physical lanes still use the shared
lane-preservation path. RGBA AlphaComposite+Mirror stays unfused on AVX-512F
until a matching fused kernel exists. Boundary cases and noisy 1024x768 inputs
plus named workloads are declared; inputs, parity, route receipts, hardware
builds, and latency remain pending.

F32 filtered Resize now has an eight-output AVX-512F path for unboxed,
horizontal-first requests. It keeps the generic path's separate-product prefix
and fused remainder, processes partial output blocks with AVX-512 masks, and
falls back for widths below eight, boxed or vertical-first requests, non-finite
samples, and unsupported coefficient sequences. I32 Thumbnail now vectorizes
its streamed 2x2 source reduction in 16-output blocks and keeps the existing
scalar resize ring and scalar reducer tail.
Native P-index Posterize also declares the shared AVX-512F byte leaf for the
Rust-only index contract. Their targeted cases and workloads are declared but
have not been generated or run; build, parity, route receipts, and latency
remain pending.

## AVX-512F suitability matrix

This source review covers the CPU families with existing x86 AVX/AVX2 paths.
An AVX-512F row is admitted only when an exact-mode leaf uses AVX-512F
instructions and preserves the selected operation's arithmetic and storage
contract. A wider register alone does not qualify when byte packing, mode
conversion, or hidden `wide` lowering would decide the implementation.

| Operation family | Exact AVX-512F strategies | Suitability decision / remaining work |
| --- | --- | --- |
| ImageOps Invert | One, L, LA, P, PA, RGB, RGBA, CMYK | Implemented with guarded 64-byte XOR blocks and native tails for the exact Rust child modes. Public wrapper mode and palette routing remain owned by the Python facade. |
| ImageChops Invert | One, L, LA, La, P, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, LAB, HSV; I, F, I;16, I;16L, I;16B, I;16N, ImageLumaA16, ImageRgb16, ImageRgba16, ImageRgb32F, ImageRgba32F | Implemented with per-mode AVX-512F adapters. Byte samples use guarded 64-byte dword XOR blocks; typed u16/f32 samples use guarded raw-bit XOR blocks. All 25 modes have boundary and named material definitions; typed CoreOnly modes use recorded expected outputs. Runtime parity, route receipts, native-hardware builds, and latency remain pending. |
| ImageOps Posterize | L, RGB, P indices | L and RGB use a repeated dword mask on complete 64-byte blocks and scalar tails. P declares the same leaf for the CoreOnly `NO_PILLOW_SUPPORT` index contract. A thin test-api entry now calls the direct Rust API; target-output boundary cases and 1024x768 material cases are defined for L/P/RGB. Inputs and workloads are not generated yet, and route receipts and latency remain pending. Public Python ImageOps transforms palette entries, so keep the direct Rust contract separate from Pillow parity. |
| Reduce | F32, I32, RGBA, RGBa, RGBX, CMYK | Implemented for complete 16-output 2x2 rows; other modes retain declared lower routes. |
| Resize | F32, I32, RGBA, RGBa, RGBX, CMYK | Nearest is implemented for unboxed requests. F32 filtered Resize has an eight-output horizontal-first path with masked tails; it preserves the separate-product prefix and FMA remainder, with Default handling requests that fail the guard. Native-byte filtered Resize has no declared AVX-512F strategy: the current exact path gathers byte samples per lane and uses either widened i64 accumulation or a guarded eight-lane i32 path. AVX-512F can widen the arithmetic lanes, but it supplies no byte gather, and mode-specific deinterleave plus source staging may cost more than it saves. Keep this as a benchmark-gated candidate; do not claim suitability from register width alone. |
| Transpose family | RGBA, RGBa, RGBX, CMYK, I, F; Rotate90, Rotate270, Transpose, Transverse | Implemented with guarded raw-dword gathers. Arbitrary-angle Rotate remains separate. |
| Filter | F32/I32 Min, Max, Median, Rank; I32 Filter3x3/5x5; RGBA radius-seven GaussianBlur | Implemented with exact mode guards. Native-byte extrema and rank remain without AVX-512F strategies: their current paths operate on byte lanes, while an F-only dword design needs promotion and narrowing and does not widen the existing 16-output rank block. AVX-512BW would allow 64 byte lanes; an AVX-512F conversion path needs a whole-call performance case before replacing an existing leaf. The radius-seven blur strategy is RGBA-only; extending it to other byte modes requires separate channel unpack/pack paths and a measured gain. |
| Analysis | I32 GetExtrema | Implemented with signed 16-lane dword min/max, a full-block guard, and scalar tails. F32 keeps ordered Default comparisons to preserve NaN and signed-zero outcomes; byte extrema need AVX-512BW byte lanes. I32 GetBBox retains its early-exit row scan because a dword mask path has no source-level win across sparse and dense images. |
| Composite Blend | L, LA, La, RGB, RGBA, RGBa, RGBX, CMYK, HSV, YCbCr | Implemented with 64-byte dword blocks, four byte-lane FMA passes, and scalar tails. |
| Enhance ColorSaturation | LA, RGB, RGBA, RGBX, CMYK | Implemented with sixteen byte-to-dword lanes and the existing float32 FMA formula. L is intentionally excluded because single-band saturation is the identity; an AVX-512 leaf would only allocate and rewrite identical bytes. |
| Color conversion and Grayscale | Existing RGB/RGBA/CMYK conversion routes | No F-only leaf identified: current paths depend on byte deinterleave, shuffle, and repacking. AVX-512BW/VBMI or a separately measured algorithm would be required. |
| Color3DLut | RGB, RGBA, CMYK, HSV, LAB, YCbCr | No clear F-only throughput win: table lookup and irregular byte layout dominate the current vector path. |
| Draw | RGB | Current 24-bit span writer depends on byte-vector layout; an F-only replacement would need lane gathering and scalar repacking. |
| Enhance Brightness, Contrast, Sharpness | Existing byte modes | Current hot paths use byte shuffles/saturating operations or neighborhood packing. Do not label them AVX-512F without a measured dword-lane design. |
| Composite AlphaComposite | LA, RGBA | Implemented with packed-dword AVX-512F blocks, scalar tails, exact fixed-point coefficient correction, full-block selection guards, and LA hidden-lane preservation. Parity, route receipts, and latency remain pending. |
| Composite Merge | LA, RGBA, CMYK | Implemented with sixteen-pixel AVX-512F blocks. RGBA/CMYK widen to dwords and store packed dwords; LA widens and narrows to packed 16-bit pixels. L Merge is a one-band copy and has no useful interleave work; RGB still needs three-byte repacking and remains under review. |
| Other analysis | RGB GetProjection; GetColors and Histogram | RGB projection reads packed 24-bit samples; no F-only deinterleave path is identified. GetColors and Histogram need mode-specific accumulation/scatter semantics; wider registers alone do not improve their update pattern. |
| Effects Mandelbrot | L | Current vector path carries eight f64 lanes and scalar active-lane bookkeeping; AVX-512F would not increase its lane count. No wider strategy is justified by source alone. |
| Thumbnail | I32 streamed path and nested geometry operations | I32 now reuses the exact AVX-512F 2x2 quartet reducer in the streamed source stage when the injected context also supplies AVX-FMA for the unchanged resize ring. It keeps pass order and scalar tail behavior. Other mode and nested geometry kernels remain under review. |
| Rotate and Transform | Exact mode-specific filtered paths | No F-only leaf is selected yet. Nearest paths need coordinate gathers plus per-pixel validity and fill handling; filtered paths also need ordered multi-sample accumulation and exact channel packing. A wider gather is suitable only if the complete public call has a measured gain with those semantics intact. |

TODO(`pillow-rs#fvsp`): generate and run the defined Posterize direct-Rust
target-output cases and 1024x768 workloads, then verify architecture selection,
fallback, palette retention, and latency. Native-byte filtered Resize remains
benchmark-gated because AVX-512F byte gathering needs a data-staging/packing
design. Native-byte Filter remains a candidate only if dword promotion beats
the existing byte-lane paths; AVX-512BW is the direct byte-lane alternative.
Arbitrary Rotate/Transform still need a measured gather/scatter and fill
strategy, and RGB Merge still needs a measured three-byte packing strategy.
For each candidate, add a mode/AVX-512F strategy with a guard and fallback only
when the full public-call path improves, or retain the recorded technical
blocker. F32 filtered Resize now
declares 7/8/9-output boundary cases and a
material workload; I32 Thumbnail declares below-block, exact-block, and tail
cases plus a material workload. GetExtrema I32 now has 15-sample fallback, exact
16-sample block, and 17-sample tail parity definitions plus an isolated
1024x768 I benchmark definition. Then freeze the full AVX-512F parity cases and
named workloads, regenerate the input corpus, and collect output, route-receipt,
native-hardware build, and latency evidence. Do not enable crate-wide
`+avx512f`; pinned `wide` may lower shared vectors outside an injected strategy.

The
canonical 25-mode
`Mode`/`DynamicImage` mapping and lazy
header-mode lookup have also had a source crosscheck. The Quantize exact-mode source routing and
recent P-output construction follow-up also had a focused source review; its
remaining palette-alpha, output/fault, receipt, and latency cases are tracked
in the inventory. PutAlpha/PutAlphaData's wrapper and ExtractBand conversion
routes now have a source pass; P→PA and PA alpha writes retain attached palette
metadata independently of sample-layout routing, with output and fault parity
still pending. P Equalize's P-to-RGB route, histogram reducer/LUT, and new
owned-buffer application path were compared with the archive; parity and
latency evidence for the in-place change remain pending. Autocontrast's
L/RGB strategies, CoreOnly native-P route, histogram/cutoff logic, and
Default/vector kernels have also had a source pass; output, fallback receipts,
and latency remain pending. Posterize and Solarize's exact L/RGB dispatch,
parameter guards, byte transforms, and CoreOnly P-index routes have had a
source pass; their output, fallback, and latency receipts remain pending.
The direct Rust mode contract is now resolved: Posterize supports native
L/RGB samples and core-only native P indices; other exact modes are explicitly
rejected. The legacy RGB-conversion fallback is retired and is not pending
work. Any future Rust-only extension must add a native exact-mode strategy and
`NO_PILLOW_SUPPORT` declaration. Output, rejection order, fallback receipts,
and latency still need parity evidence.
Solarize's per-leaf review now covers the Pillow LUT callback ordering,
palette alpha handling, exact L/P/RGB strategies, threshold arithmetic, and
Default/vector paths. Its direct Rust mode contract matches Posterize's:
native L/RGB and core-only P indices are supported; all other exact modes are
explicitly rejected. The legacy RGB-conversion fallback is retired. Output,
rejection order, receipt, and latency evidence remains pending.
Equalize now also has a caller-to-leaf source pass: L/RGB/P selection,
operation-defined P-to-RGB output, mask handling, histogram/LUT construction,
and Default/SIMD application were checked against the wrapper and archive.
Its direct Rust mode contract is resolved: L/RGB are native routes, P uses
the declared palette-to-RGB operation, and every other exact mode is
explicitly rejected. The legacy convenience conversion fallback is retired.
Runtime output, rejection order, receipt, and latency evidence is still
pending.
Colorize now has a caller-to-leaf source pass for Pillow's ImageOps wrapper
and the direct Rust L-to-RGB strategy. The pass corrected endpoint-before-
midpoint comparison order and the wrapper support table's non-L rejection
kind to ValueError. Its Default/vector outputs, subclass Convert results,
route receipts, and latency still require focused evidence.
The Colorize architecture slice also reconciles its SSE2/NEON/Wasm compile
guards, exact-L support declaration, injected-context selection checks, and
matching execution arms; shared `wide` code is not evidence of a particular
emitted ISA sequence. The remaining cross-family source sweep and compiler-
backed reachability audit are still open.
The cross-family sweep found one unguarded shared Paste NEON module containing
AArch64 intrinsics; its module declaration now uses the same `aarch64` plus
`neon` gate as its only callers. Filter I32's AVX/FMA instruction route now
has a source crosswalk from the x86 module gate through context capabilities,
the exact support guard, and the prepared execution arm. Cross-target
compilation remains pending. Transpose RGBA's AVX2 module, target-feature
leaves, support guard, injected-capability checks, and both execution arms also
reconcile. A read-only sweep accounted for all 26 maintained CPU files with
explicit intrinsic imports or inline assembly; this source inventory does not
replace cross-target compilation or runtime receipts.
Composite RGB Merge's SSSE3/NEON specialized guards, generic per-architecture
fallbacks, module gates, and owner dispatch also reconcile at source level.
Resize I32's NEON guard, AArch64+NEON module, context-bound entry, checked
intrinsic blocks, and same-mode wide block fallback also reconcile.
RGB Grayscale's AVX2/NEON support requirements, target gates, and exact-mode
prepared execution arms also reconcile.
YCbCr Grayscale's x86 SSSE3/SSE2 split and AArch64 NEON module gates also
match their support entries and prepared execution routes.
RGBA GaussianBlur's AVX2/NEON radius-seven entries are limited to their
compiled kernel targets; its separate native-byte entries route other cases,
including 32-bit x86 AVX2, to the shared portable-wide fallback.
LA Filter3x3's AArch64+NEON entry reaches its native two-channel binomial
kernel only for the exact coefficient, scale, offset, and width route; other
requests remain on the mode-owned generic vector path.
Paste's direct L-mask NEON rows now reconcile with each exact-mode guard,
module gate, owner dispatch, and the RGB/HSV shared kernel gate.
Composite Blend's AVX/FMA and NEON entries reconcile with their x86 and
AArch64 module gates, exact-mode tables, injected selection, and typed owners.
Rotate's public fast paths, affine construction, alpha/subclass routing, typed
preparation, and injected mode/architecture handoff now have a direct-caller
source pass. The Python wrapper now materializes queued Rotate and Transpose
results before returning, matching Pillow's public eager output boundary.
Allocation, execution-fault timing, and selected/fallback receipts remain
unverified.
EffectSpread and EffectNoise all-mode Default leaves and their shared RNG/sample
helpers and declared SSE2/NEON/Wasm SIMD128 leaves have now been compared with
the archived implementations; their output, receipt, and latency evidence
remains pending.
ImageOps Invert's Python callback ordering, P palette route, direct Rust
support split, prepared mode/architecture routing, and native point-LUT fusion
have also had a source pass. The fused identity path now zeros omitted physical
lanes like Pillow's newly allocated point result; raw-byte follow-on parity and
the Default-versus-vector malformed-suffix contract remain pending.
Eval's archived SIMD admission set was crosschecked against the nine active
exact-mode strategies; its empty-input and exact-native-byte guards match the
old adapter, and its portable LUT plus RGB NEON/affine kernels retain the same
mapping. Prepared Eval zeros omitted lanes for both identity and mapped
outputs. No missing mode/architecture path was found; parity, target receipts,
and performance remain pending.
Direct ImageOps-to-registry routing was source-reviewed (2026-11-24): each
queued ImageOps variant and its typed aliases resolve to the matching prepared
CPU entry, appended pipeline nodes retain injected context, and bounded RGB
Grayscale prepares and executes that same registry plan with its selection.
High-threshold and host-comparison Solarize inputs deliberately use the
declared exact-mode Eval LUT route; ordinary byte thresholds use Solarize.
The native Pad route had dropped the scoped context when it constructed a
padded wrapper directly; the binding now routes the pending result and the core
retains the original backend/context across a synthesized empty source. Output,
fault, selection/fallback receipts, and latency remain pending.
ChopsAdd/Subtract's L/Default requests, typed byte loop, arithmetic, and Pillow
input pair contract have had a focused archive/source comparison. The RGB/
Default typed three-byte route and fourth physical-lane sidecar flow have had a
source comparison against Pillow's four-byte RGB storage and Chops row loop.
Their selected vector paths have also been compared with the archived
SSE2/NEON/Wasm SIMD128 helper and input guards; La and LAB remain Default-only.
ChopsMultiply and ChopsScreen's integer arithmetic, divide-by-255 vector
kernel, mode/architecture routes, and equal-dimension strategy guard have also
had a source pass. Their shared hidden-lane mapping and the prepared
Multiply-then-Screen fusion were compared with the archived paths. The
Darker, Lighter, and Difference's byte formulas, same-band mode groups,
equal-dimension vector dispatch, and hidden-lane mapping have now also been
compared with Pillow and the archived helpers. Overlay, HardLight, and
SoftLight's LUT and integer-vector formulas have had the same source pass.
Modulo and logical byte formulas have now also been compared with Pillow and
the archived helpers; logical strategies remain limited to Mode::One for both
operands. Constant and Duplicate now have a focused source pass; Invert and
Offset mode/architecture maps are already recorded in the inventory. Chops
output/fault parity, receipts, hidden-lane follow-on cases, and latency remain
pending. This does not close CPU Stage 5 source review.
The 2026-11-24 review also corrected Mandelbrot's empty-output contract and
reviewed the typed request-contract dispatcher and semantic assessments; see
the inventory for the source-only findings. The materialized request guards
now have source passes for core, Rotate, Resize, Draw, and mutation predicates;
Rotate geometry guards remain split into their own module to keep files under
the size limit. Resize family
classification, prepared context and architecture selection, and representative
byte, endian-specific u16, typed CoreOnly, I32, and F32 routes have a
caller-to-leaf source crosscheck. PutAlpha/PutAlphaData, ImagingCore SetMode,
PutBand/FillBand, ordinary PutPixel single/batch routes, and the RGB
PutPixel+Thumbnail fusion also have caller-to-kernel source passes. Remaining
per-leaf archive/reachability and mutation caller/kernel review outside those
slices, plus all runtime evidence, remain pending. The
later GPU stage will use the injected CPU Architecture for host-semantic and
preflight fallbacks when that context is available.
PutAlpha/PutAlphaData now have a source-only mode-owner and kernel crosscheck:
accepted and rejected modes, output identities, One-mask normalization,
premultiplied inputs, indexed palette retention, typed CoreOnly output, and
Default/vector mask routes were reconciled. Their runtime parity, receipts,
and latency remain pending.
PutData now has a caller-to-kernel source pass across the Python sequence
adapter, payload/mode matrix, injected architecture selection, byte prefix
leaves, hidden-lane restoration, and native CoreOnly writes. Its callback,
fault, palette, output, receipt, and latency contracts remain pending CPU
parity.
Posterize now has a per-leaf source pass through its public Python wrapper,
callback adapter, P palette route, direct Rust support matrix, exact-mode
selection, Default/vector kernels, hidden-lane policy, and point-LUT fusion.
The composer now fuses only valid `1..=8` bit requests, leaving invalid input
for the normal request contract. The uncovered legacy mode-matrix difference
remains tracked; no runtime validation ran.
Remaining per-leaf review is tracked by owner there. Stage 6
structural, static, exact-size, and compilation results from 2026-11-13 are
historical and must be rerun after source review closes.
The frozen CPU parity matrix and performance gates remain pending. Do not begin
Parallel CPU until the CPU implementation passes its quality, parity, and
performance gates.
The follow-up x86 module-gate scan now reconciles Composite RGB SSSE3,
Thumbnail/Resize I32 AVX-FMA, YCbCr/RGB Grayscale SSSE3/AVX2, RGBA Transpose
AVX2/X86_64, and CMYK Convert SSSE3 module gates with their support predicates
and execution arms. RGBA Filter's radius-seven AVX2 leaf is x86_64-only while
its separate generic AVX2 blur strategy admits x86 and x86_64. The x86
grayscale strategies also have separate x86 wide modules; the remaining
x86_64-only implementation leaves are declared x86_64-only. No additional
source gate mismatch was found. Cross-target compilation remains pending and
this does not close Stage 5.

2026-11-23 Convert binding size follow-up: extracted
`convert_output_with_metadata` from the PyO3 Convert entry point to keep both
functions below the 100-line body limit. The extraction preserves wrapper
result construction order, warnings, transparency deltas, and the captured
execution route. Source-reviewed only; no validation command ran.

2026-11-23 Reduce source crosscheck: traced all 25 mode owners and the
Default/vector support tables through prepared execution. LA and RGBA fuse
Pillow's premultiply/reduce/unpremultiply route in their exact native mode
strategies. Runtime outputs, faults, architecture/fallback receipts, and
latency remain pending; no validation command ran.

2026-11-23 Enhance source review: traced Brightness, ColorSaturation,
Contrast, and Sharpness from Python enhancer callbacks and typed `Image`
methods through `PipelineOp`, registry preparation, the injected
`ExecutionContext`, exact `DynamicImage` mode arms, and mode-owned execution.
The source crosscheck found no mismatch between operation support, prepared
mode mapping, architecture availability, and execute arms. Brightness keeps
Default for its 22 accepted modes and exposes Sse2/Neon/Wasm SIMD128 for its
seven declared vector modes. ColorSaturation declares six exact native modes,
five of which have AvxFma/Neon strategies, plus its YCbCr/LAB/HSV conversion
compositions and explicit rejection routes. Contrast has nine prepared modes:
six native byte modes with Sse2/Neon/Wasm SIMD128 routes, Default-only YCbCr
and HSV, and LAB's four context-selected children. Sharpness accepts 22 modes,
retains the One/P/PA rejections, and maps six byte modes to seven architecture
variants. Its AVX/AVX2 names share five architecture folders because the
Avx/AvxFma and Avx2/Avx2Fma variants use the same mode-local wrappers.

Across Enhance, all 58 architecture leaf files delegate to the shared
`color_saturation`, `contrast`, or `sharpness_vector` helpers; Brightness's
three declared vector architectures call its shared `vector` helper. The
architecture leaf scan found no direct `target_feature`, `core::arch`,
`std::arch`, or unsafe intrinsic code. Parent architecture modules are
unconditional; support declarations apply build-feature and guard checks
before the selected wrappers are reached. This is a source-routing result,
not proof of compiler lowering or executed hardware instructions. Exact output
and fault parity, selected/fallback receipts, cross-target builds, and latency
remain pending. Refreshed source VERIFY markers on the reviewed family glue,
support builders, Contrast route, and all 58 architecture leaves. No build,
formatter, checkstyle, lint, test, parity, benchmark, or coverage ran.

Latest source-only continuation (2026-11-23): the separate mode-preflight
matrix now covers ImageEnhance Brightness/Contrast/Sharpness and the composed
Image.composite and Image.alpha_composite routes. Pillow's public composite
constructors expose different mode and materialization boundaries: ImageOps
composition defers to Paste, static AlphaComposite loads both operands before
its typed kernel, and the in-place method performs host geometry/crops before
nested AlphaComposite and Paste. The recorded recommendation is operation-owned
Mode preflight at each established public boundary, separate from pixel
eagerness. The direct typed composite API now accepts the destination-selected
ExecutionContext and binds it through its conversion and CompositeModule
children. Source and callback/fault review only; parity and latency remain
pending. The direct typed Composite route now validates declared mode support
before queueing, after any required source conversion; concrete architecture
selection remains deferred to prepared pipeline materialization. Pixel work
stays lazy. The Python static AlphaComposite facade now calls public
`im1.load()` then `im2.load()` before the typed core; the Rust entry mirrors
that order for direct callers, performs mode/size admission afterward, and
materializes successful output. Its output and fault parity remain pending.
Brightness now retains Pillow's same-mode zero base and optional uppercase-
alpha snapshot during construction, then blends it with the current source.
The shared Python Blend boundary loads operands in order and materializes the
Rust result before returning. Sharpness now calls public `filter(SMOOTH)` at
construction and retains its completed result plus optional alpha snapshot;
the Filter adapter materializes each callback result before wrapping it or
invoking a later per-band callback. Contrast now follows Pillow's public
convert-to-L, `ImageStat.Stat`, base allocation, optional convert-back, and
uppercase-alpha callback sequence. `ImageStat.Stat` obtains samples through
the public histogram method while Rust computes the summary; a base
Convert/`_new`/`load` route materializes before construction returns, while
virtual results retain their own boundary. The private direct-stat PyO3 hook
was removed because the public adapter now owns the histogram callback. Rust
statistics group only complete 256-bin bands, matching Pillow's handling of a
trailing incomplete histogram segment.
Source callback review is complete; output, fault, and mode/architecture
parity remain pending. The high-level Python Paste base-conversion boundary is
source-reviewed: the facade calls exact base-image `Image.convert` under the
active destination context and loads the returned image before destination
loading or mask truthiness. Virtual overrides provide their actual returned
core: a deferred Rust pipeline is materialized when the wrapper captures that
core, while a returned unopened encoded image preserves Pillow's missing-core
assertion before destination load. Non-core source validation remains after
destination and active-mask loading. Mask `.im` is read after its load; staged
native parsing then resolves box coordinates, validates the mask core type, and
coerces a non-core source. Direct ImagingCore Paste keeps the source raster's
Mode and reaches the declared destination/source pair contract without
applying high-level wrapper conversion. Core color strings are only resolved
by the high-level wrapper; direct core Paste delegates them to Pillow-style
numeric `getink` parsing. These are source findings; output, fault, route, and
architecture/fallback receipts remain pending.

The direct Rust route handoff is now source-reviewed. `Image::paste`,
`paste_at`, and `paste_with_input` capture the destination's requested Backend
and CPU `ExecutionContext` before any eager source Convert. The same route
drives the queued Paste; route-less mode-only source preparation rejects an
unresolved image source, while already-resolved Image and non-image adapter
paths retain their existing boundaries. This is operation-specific route
injection, not a shared output-Mode preflight abstraction. Output, fault-order,
fallback receipt, and latency parity remain pending under `pillow-rs#3ptb`.
The broader cross-operation Mode-preflight decision remains separate and open
under `pillow-rs#5e8f`. No validation command ran.

Cross-operation source follow-through (2026-11-23): Crop preserves exact Mode
and can return a queued non-full result after source loading and host extent
checks; retain this lazy graph unless output, metadata, source-snapshot, fault,
or strategy-receipt parity requires materialization before `_new`. Blend loads
both operands in public order, binds the first operand's context to the result,
and materializes before `_new`; its direct Rust graph accepts explicit context
before materialization. Filter takes a single callback's actual returned Mode,
and multiband custom filtering wraps each sequential callback result before
Merge uses the wrapper's current Mode. Source review only; all parity, fault,
route-receipt, and performance evidence remains pending.

Convert P-output callback correction (2026-11-23): the Python adapter now
receives whether the selected P route is adaptive quantization. It follows
Pillow 13.0's different exception scopes for adaptive and ordinary palette
allocation, preserves the ordinary route's exact full-palette warning rule,
and stores the RGB transparency tuple directly when an ordinary P output has
no attached palette. This is source-reviewed; callback output/fault, palette
mutation, and warning parity remain pending under `pillow-rs#3ptb`.

P/PA palette property follow-through (2026-11-23): the public wrapper now
reports `None` when its Rust image has no attached palette and retains explicit
wrapper palettes or attached empty palettes. Convert's P-output transparency
callback therefore follows Pillow's palette-presence branch. Verify lazy,
empty, and dirty palette behavior in the frozen CPU parity matrix.

Filter callback execution review found two eager ImagingCore routes that
previously selected their Architecture before the Python adapter could attach
the active route: `expand` and P-mode `rankfilter`. Both adapters now pass the
callback's captured Backend request and CPU `ExecutionContext` into core
strategy selection. Scalar rankfilter remains a queued graph and receives the
same context before materialization. Selection/fallback receipts and pixel,
fault, and latency parity remain pending under `pillow-rs#3ptb`.

The architecture source crosswalk now also traces RGB8 Grayscale's declared
Default/SSE2/SSSE3/AVX2/NEON/Wasm strategies and the Composite AVX/FMA, NEON,
and RGB Merge SSSE3 intrinsic leaves from support predicates through
mode-specific dispatch. These are source findings only; cross-target builds,
runtime receipts, parity, and latency remain gated.

2026-11-23 Filter architecture source review: Filter3x3/Filter5x5 byte modes
share the declared architecture tables and mode leaves; native-byte blur and
rank strategies share typed helpers; I32 convolution and F32 rank expose their
separate AVX2/FMA and portable-wide paths. RGBA's radius-seven fast path now
dispatches by its selected `FilterRgbaGaussianBlurRadiusSeven` guard. This
prevents a 32-bit x86 AVX2 build, which only admits the generic blur strategy,
from entering the x86_64-only specialized leaf. Cross-target build and
selected/fallback receipt evidence, output parity, and latency remain pending
under `pillow-rs#3ptb`.

2026-11-23 RemapPalette staged-input follow-through: the PyO3 adapter
interleaves each `dest_map` item with Pillow's source-palette slice and inverse
index assignment, preserving Python iteration, slicing, arithmetic, and list
index faults in order. It validates the inverse-map bytearray before copying
and retains the selected palette object so its ordinary public conversion
faults remain at the final `putpalette` boundary. The Python route switches the
copied core to P, constructs and assigns the wrapper palette object, calls its
`tobytes()` protocol, attaches the typed palette in Rust using Pillow's
`RGB;L`/`RGBA;L` raw layout, invokes virtual `convert("L")`, then invokes
final public `putpalette` and returns that converted wrapper. `self.copy()`
already invokes the virtual copy and `_new` boundary. The mapping palette and
raw-layout selection are generated in `pillow-rs` from validated inverse
indices and the typed palette model. Source review only; output, fault,
architecture/fallback receipts, and latency remain pending. No build,
formatter, checkstyle, lint, test, parity, benchmark, or coverage command ran.

RemapPalette CMYK output is a source-level parity question: Pillow keeps a
three-byte entry stride when its implicit palette mode is CMYK, then installs
the intermediate table with the four-plane `CMYK;L` raw layout. That creates a
192-entry intermediate palette from the 768-byte repeated map. The typed Rust
table emits four planes for all 256 indices, as required by the typed CMYK
layout. This may affect outputs for indices at or above 192. RGB, RGBA, and
CMYK mapping palettes now match their `;L` decoders' planar input layout. Keep
the full-range CMYK behavior pending frozen CPU parity evidence; if public
outputs differ, discuss whether Pillow's result is erroneous before choosing a
deviation. Sources: [Pillow 13.0 `Image.remap_palette`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L2053-L2103), [palette sizing](https://github.com/python-pillow/Pillow/blob/13.0.0/src/_imaging.c#L1939-L1966), and [the CMYK;L unpacker](https://github.com/python-pillow/Pillow/blob/13.0.0/src/libImaging/Unpack.c#L1645-L1651). Tracked in `pillow-rs#3ptb`.

Correction from the pinned Pillow 13.0 source: the intermediate map palette is
installed directly through `ImagingCore.putpalette` after assigning the copy's
mode and palette object. There is no virtual Python `putpalette` at this
intermediate boundary. The virtual callback sequence is `self.copy()` (and
its `_new`), copied-image `convert("L")` (and its result `_new`), then the
converted wrapper's final public `putpalette(selected_palette, palette_mode)`.
Earlier audit text claiming an intermediate virtual `putpalette` was
incorrect and has been corrected. Source: [Pillow 13.0
`Image.remap_palette`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L2036-L2115).

2026-11-23 RemapPalette public-wrapper source closure: compared the maintained
Python and PyO3 path with Pillow 13.0 from the wrapper mode check through the
late transparency update. Implicit P loads and snapshots its actual palette
before map iteration; L synthesizes the RGB grayscale table before iteration;
explicit palettes resolve `len(source_palette) > 768` before iteration. Each
map element's multiplication, slice, palette-byte addition, and inverse-list
assignment remain interleaved. The inverse-map bytearray conversion stays
before `copy()`. The copy and `_new` callbacks, P-mode staging, palette
constructor and `tobytes()`, direct typed intermediate attachment, virtual
`convert("L")`, final public `putpalette`, and late `dest_map.index()` all
remain in Pillow order. Public wrapper source review is complete; callback
mutation, output/fault, mode/architecture receipts, and latency still require
the frozen CPU evidence. The CMYK three-byte map stride versus four-plane
`CMYK;L` output remains an unresolved parity question; do not change that
contract before observing the pinned oracle. Source:
[Pillow 13.0 `Image.remap_palette`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L2036-L2115).

2026-11-23 source-size correction: the follow-up workspace scan found
`pillow-rs-py/src/image/operations.rs` at 1,025 lines after Convert adapter
growth. RemapPalette's PyO3 plan and methods now live in the existing
`image/palette.rs`, while Convert's PyO3 plan and methods live in the existing
`image/convert.rs`. The modules are now 700, 408, and 771 lines respectively;
the moved method bodies were preserved. A repeat physical-line scan covered
all 5,541 Rust files in the three workspace members and found none above 1,000
lines. Clippy-counted function sizes and the rest of source review remain
open. No build or validation ran.

2026-11-23 Convert source review: the typed Convert planner resolves source
and target `Mode` before pixel execution, while the PyO3 adapter stages only
the host values consumed by the selected branch. Python retains the public
load boundary, callback-visible matrix and palette checks, transparency
metadata delta, and source P-alpha mutation timing; execution carries the
selected `ExecutionContext` into conversion and transparent-alpha strategies.
Matrix transparency remains a host callback path after successful pixel
conversion. This is source review only: Convert pixel, fault-order, fallback
receipt, and latency parity remain pending. No validation command ran.

2026-11-23 Convert lazy/eager and backend handoff follow-through: Pillow's
public Python `Image.convert` loads the source before mode and argument
preflight. The typed plan then selects an output `Mode` before conversion
pixels execute. Same-mode copy preserves the loaded source; eligible ordinary
mode conversions can return a queued `PipelineOp::Convert`; target `One`,
matrix conversion, quantization, and transparent-alpha conversion materialize
through their eager executors. Python `_new` preserves lazy output graphs unless
the pending wrapper hook requires a load. Direct Rust and ImagingCore callers
have separate materialization boundaries.

The adapter captures `requested_backend`, `pinned_backend`, and
`ExecutionContext`. `execute_convert_input_plan_with_route` now composes a
`CpuExecutionRoute` for matrix and ordinary conversions; route-aware
quantization carries the request into its eager receipt and the context into
prepared mode/architecture selection. Since public Python Convert loads its
source before planning, eager matrix and quantize routes select their strategy
from an already materialized source; backend pinning must not move or repeat
that load. Transparent-alpha conversion receives the full request, pin, and
context. Lazy results continue through the Python `route_pending_image`
boundary, which applies a pinned backend and captured context to the pending
result graph. Verify eager request/context receipts and lazy pin/context
selection and fallback receipts separately. This applies the operation-owned
route recommendation without moving preflight or pixel work across
Pillow-visible boundaries. Source review only; output, callback order, fault
order, selection/fallback receipts, and latency remain pending. No validation
command ran.

2026-11-24 ImagingCore Convert destination-order correction: low-level
`convert_into` first resolves an accepted source/target route, then validates
the existing output mode and dimensions, and only then writes converted pixel
rows. The Rust route now uses the same declared Convert support to reject
unsupported pairs without materializing pixels, compares source and destination
dimensions before conversion, and commits a locally prepared destination only
after conversion succeeds. P/PA palette callback and malformed-palette fault
ordering, all mode-pair outputs, and selection/fallback receipts remain parity
TODOs; this source correction does not close those paths. Source: Pillow 13.0
[`ImagingConvert`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/libImaging/Convert.c#L1481-L1547),
[`ImagingNew2Dirty`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/libImaging/Storage.c#L706-L724),
and the palette special routes in [`Convert.c`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/libImaging/Convert.c#L990-L1103).

2026-11-23 indexed-pipeline metadata source review: queued palette-preserving
operations carry RGB bytes, alpha bytes, and typed `PaletteData` independently;
attached empty palettes remain distinguishable from absent palettes. Output
mode planning uses cached/header identity and the operation's support contract,
leaving unresolved transitions deferred to materialization. Prepared execution
receives the retained palette model, and loading restores it to the resulting
wrapper. This is source review only; indexed outputs, fault order, backend and
architecture receipts, and latency remain pending. No validation command ran.

2026-11-23 palette mutation source review: Python `putpalette` validates the
source mode, constructs and installs its wrapper palette, stages L/LA→P/PA,
then loads only when the existing core was materialized or raw decoding is
required. Dirty-palette realization attaches typed palette data before load
for materialized images and deferred Rust pipelines. For unopened encoded
sources, Pillow mode P commits from `load_prepare` before decoder execution;
mode PA commits at the final base load after decoding. The wrapper preserves
that predecoder/postdecoder split. The unloaded-source query follows the
palette-only empty metadata layer created by `putpalette`, but stops at a
pipeline with queued pixel operations. Allocation/mmap fault precedence and
wrapper-mode overrides at `load_prepare` remain parity TODOs. Raw LA/PA alpha
commits before transparency mutation. The PyO3 update boundary retains the
mutated Rust image on errors, preserving partial state. Source review only;
partial-state, callback/fault, mode/output, and route parity remain pending. No
validation command ran.

Current manual size scan (2026-11-23): the first follow-up scan found
`pillow-rs-py/src/image/operations.rs` at 1,025 physical lines. Moving the
RemapPalette and Convert adapters into their existing operation modules leaves
`operations.rs` at 700, `image/palette.rs` at 408, and `image/convert.rs` at
771. A repeat scan of all 5,541 Rust files in the workspace found none above
1,000; `pillow-rs/src/lib.rs` is exactly 1,000. The at-least-800-line files
still require file-by-file review. This scan does not prove Clippy-counted
function size compliance.

Latest source-only continuation (2026-11-22): RemapPalette now rebuilds its
palette from the selected source model's raw bytes and runs those bytes through
the shared palette decoder. This covers the observed CMYK remap layout as well
as RGB/RGBA and explicit palettes. Source and error-order review is recorded in
the inventory; parity, route receipts, and latency remain pending. No
validation command ran.

The same review now tracks palette attachment separately from palette bytes:
an absent P palette and an explicitly attached empty table both contain zero
bytes but expose different core state. `PalettedData` and palette-preserving
pipeline paths retain that distinction so palette lookup, mode queries, and
RemapPalette admission can report the correct result. Source-reviewed only;
parity, route receipts, and latency remain pending. No validation ran.

RemapPalette mode admission and its 256-entry destination-map limit now come
from the exact L/P support declarations. The public wrapper and prepared route
consume that contract, and the prepared route no longer silently truncates an
oversized map. Source-reviewed only; parity, error-order receipts, and latency
remain pending. No validation ran.

2026-11-23 RemapPalette public-boundary source follow-up against Pillow 13.0:
the implicit P-palette route calls `load()` before palette access; explicit
palette length is read and compared with 768 to choose RGB versus RGBA before
`dest_map` iteration; Pillow does not compare it with the attached palette's
length. Each map item is enumerated, sliced from `source_palette`, then assigned
to the fixed 256-entry inverse map. `copy()` materializes pixels after mapping;
the copied core receives the intermediate mapping palette directly, then
virtual conversion to L, public final palette installation, and transparency
remapping run in order. The maintained adapter snapshots the implicit P
palette or L grayscale palette before map callbacks, captures explicit palette
length/model before consuming `dest_map`, and interleaves each source slice
with its inverse-map assignment. It preserves `self.copy()` and its `_new`,
then routes through virtual `convert("L")` and final public `putpalette`.
Callback mutation and fault ordering remain subject to parity. Core L/P mode
selection, palette decoding, and native remap strategy are source-reviewed;
public output, callback/fault, architecture/fallback receipts, and latency
remain pending. No runtime validation ran.
Source: [Pillow 13.0 `Image.remap_palette`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L2036-L2115).


Draw now retains native CMYK palette entries when a Python `ImagePalette` adds
a color, and the direct Rust Draw tuple strategy resolves four CMYK components
as color channels rather than RGBA alpha. Source-reviewed only; Draw pixel and
palette parity, callback/fault order, route receipts, and latency remain
pending. No validation ran.

2026-11-22 typed palette-model continuation: Rust now retains palette identity
as `PaletteModel` through Convert's default P target, RemapPalette, Composite,
and indexed pixel-access decisions. `Image::palette_mode()` remains the
Pillow-facing spelling projection; the core no longer converts that known
model back from a string to choose these paths. Source review only; palette
output, error/callback order, route receipts, and latency remain pending. No
build, formatter, checkstyle, lint, test, parity, benchmark, or coverage ran.

2026-11-22 source-size continuation: moved indexed-palette pixel methods into
the existing `pixel_access/palette.rs` module and moved PyO3 palette methods
into the existing `image/palette.rs` module. This keeps the files organized by
responsibility without adding folders or changing their operation behavior.
The post-move maintained Rust source scan found no file over 1,000 physical
lines; the largest is `pillow-rs/src/lib.rs` at 999 lines. This is a line-count
scan, not function-size proof or quality-gate evidence. Palette output,
callback/fault order, and parity remain pending; no validation command ran.

The original CPU and SIMD trees remain preserved under `pool_cpu/legacy` as
inert references. Keep them as read-only comparison material; active code must
not import or execute from that archive. The current work continues on
`develop`, from baseline `3f9dfd4036caa061eabcf2255c9f34a745df9c7e`.
Issues: `pillow-rs#3ptb`; related Draw parity: `pillow-rs#brg0`.

## Outcome and scope

Transform CPU execution completely into the mode and architecture structure
defined by [Implementation structure](IMPLEMENTATION_STRUCTURE.md). That guide
is the sole placement authority; this plan defines work order and acceptance.

The maintained implementation is Rust-only. Do not add C or C++ source or
implement image operations in those languages. Comments may describe Pillow's
C implementation when that behavior is the compatibility reference; such
references do not authorize a C implementation or dependency.

Keep Python and `pillow-rs-py` as thin host boundaries. `pillow-rs` owns
operation rules and decisions, geometry, palette allocation, metadata
transitions, composition, and pixel algorithms. Host code may convert Python
objects into typed values and invoke Python callbacks, protocols, or overridable
public methods when their order is observable; it then delegates the resulting
image work to Rust. Any remaining host-side operation algorithm is migration
debt and must be listed in the source inventory with the callback/fault reason
that prevents a direct move.

Mode is carried by the flat Pillow-owned `raster::DynamicImage` enum. Execution
context carries Backend and Architecture from routing to implementation.
Scalar/default and existing SIMD kernels become implementations of the CPU
backend. Parallel CPU is a separate subsequent stage and begins only after the
entire CPU implementation compiles and passes its quality and parity gates.

Each operation exposes its typed support contract at the operation entry point.
It composes a mode strategy with an architecture strategy using the injected
context. Support reporting, request admission, and executable preparation use
the same declarations, as defined in Section 3 of the structure guide. Folder
placement isolates these strategies; explicit contracts establish support.

Ownership and operation-level visibility are required acceptance criteria,
not optional follow-up work. Use Section 2.1 of the structure guide to keep
Pillow mode identity separate from codec metadata. From an operation's entry
point, a caller must be able to inspect every mode's declared strategies,
input/output constraints, architecture requirements, and rejection or fallback
reasons without inspecting implementation folders. Generate that view from
the same declarations used to prepare execution; do not maintain a second
support matrix by hand.

The ownership decision is explicit: `pillow-rs` owns the operation-facing
`raster::Mode`. Rust callers use it directly, and Python translates
its strings to it before delegation. Review the support-visibility acceptance
contract in Section 3.3 of the structure guide through each operation's entry
point, including deferred guards for facts unavailable before materialization.

Pillow defines the observable compatibility contract: raster outputs, mode
transitions, public errors, and their ordering for inputs in the parity matrix.
Its internal algorithms, storage layout, dispatch, intermediate objects, and
implementation quirks are reference evidence, not architectural requirements.
Implement the contract with Rust-native mode and architecture strategies; an
algorithm, arithmetic sequence, allocation strategy, or fusion path may differ
when the externally visible result and fault contract remain correct. Prefer
the implementation that is simpler to reason about and faster without giving
up exact parity. Keep operation-defined mode conversions explicit, and never
convert an image to another mode merely to reuse an implementation.

Core support may intentionally exceed Pillow: add a declared native-mode
strategy where useful, mark it beside its implementation with
`// NO_PILLOW_SUPPORT: <reason>`, and report it separately from Pillow parity.
Pillow-specific internal behavior should be reproduced only when it changes an
observable output or fault covered by the compatibility contract.

Parity classification separates missing implementation from wrong behavior.
When Pillow completes a step but the target raises `NotImplementedError` at
that step, and every earlier observed step matches, classify the case as
`not_run` (pending), retain the target error in the receipt, and do not count
it as a mismatch or a pass. Target-only expected-output cases that reach an
unimplemented route are pending the same way. The run remains incomplete while
such cases are selected. Different outputs, wrong public errors, and
rejections that contradict a valid supported route remain parity mismatches;
this rule does not weaken comparison of implemented behavior or fault
contracts.

### Per-function verification markers

Place a short `VERIFY` comment immediately before every maintained function
that receives a source review, parity correction, or benchmark measurement.
Put it above the function's attached attributes or decorators so changes to
those are covered by the same marker. Record independent evidence rather than
one ambiguous “verified” tag:

```text
VERIFY: source=reviewed@YYYY-MM-DD|pending; parity=pass@<run-or-bundle-id>|fail@<run-or-bundle-id>|pending|n/a; benchmark=pass@<run-id/workload>|fail@<run-id/workload>|pending|n/a
```

The marker applies to the function implementation and its behavior-affecting
signature or attributes as they exist immediately below it. Update the marker
in the same change whenever any of those change: reset all three fields to
`pending`, then restore each status only after collecting that evidence for the
changed function and affected path. A source review records its date. Parity
and benchmark statuses record focused run/workload IDs and whether the result
passed or failed. For one function verified across several architecture runs,
use a bundle ID whose summary enumerates each run and profile. A parity fix remains `pending` until the correcting change
passes the relevant parity path. An aggregate result does not verify a
function unless its selected path is known. Use `n/a` only when that evidence
type cannot apply. Do not claim evidence that has not run. Functions without
a marker have no recorded verification status.

Implementation starts from this baseline with Stage 0. The user has authorized
the ordered stages below. Do not repeat the reset or discard work created after
the baseline. Keep `pillow-rs-bad` as reference.

## Current pending matrix

The current shared input matrix has 38 unique sizes and 115 deterministic
size/pattern pairs, using varied, checkerboard, boundary, impulse, and seeded
noise data where assigned. It covers all 25 exact `Mode` identities only where
an operation declares support. The four long-axis sizes (2048×1, 2049×1,
1×2048, and 1×2049) use varied, checkerboard, boundary, and seeded-noise
inputs. Direct PutData inputs cover all 20 Pillow modes and five CoreOnly
typed-sample modes; the CoreOnly set has 595 target-output cases, including 575
shared matrix points, and exercises native u16/f32 writes, tuple arities, and
that scale and offset are ignored for component tuples with non-identity
arguments.
`Image.fromarray` Resize inputs cover eight inferred mode/dtype layouts and
both Pillow 13 MKS filters. The encoded `Image.open` matrix has
1,035 cases across nine modes and 115 input points; it exercises the codec
boundary, not a compute backend. The public Rust-backed `Image.fromarrow`
constructor has been added after the last generated-input checkpoint; its
parity denominator is not yet frozen. Add public cases for each supported
Pillow mode, scalar/packed/fixed-list/interleaved schema layouts, offsets,
hidden physical lanes, P/PA palette state, the `readonly` transition, and
schema/size/buffer rejection. This constructor does not select a pixel
execution architecture, so its matrix is a public input-contract lane rather
than a CPU kernel architecture cross-product.

The pre-GetData snapshot contained 176,721 parity cases across 4,730 indexed
parity documents and 103,945 benchmark workloads. The latest GetData matrix
adds 1,150 target-only cases and 1,150 indexed benchmark workloads. The current
manifest indexes 5,110 parity documents and 105,095 benchmark workloads; the
176,721-case total is historical and is not the current case count. The index
includes the regular-polygon mode requirements and 66 F32 rank workflows for
NaN payloads, mixed signed zero, and vector-compatible infinities over 3×3,
5×5, 9×9, and 11×11 windows. All 66 passed for each of the nine CPU
architecture selectors (594
executions). On this arm64 host, Neon executed the vector path for 16 eligible
cases and selected Default for the NaN, mixed-zero, and size-11 cases; x86
selectors selected Default with `unavailable_hardware`, so native x86
execution remains unverified. A frozen focused selection of 192
regular-polygon cases also passed under each of nine CPU architecture
selectors (1,728 passes total). One standard 16×16 RGB workload was measured
for Default and NEON: Default was 8.3% faster than Pillow, while NEON was 4.8%
slower, within the stated 1–5% allowance. These focused results do not replace
the full matrix. This supersedes the previous
171,666-case/100,610-workload snapshot documented in older inventory entries.
The shared matrix still uses 38 unique sizes and 115 deterministic
size/pattern pairs; CPU selection replays each eligible case under the nine
configured Architecture selectors. The added typed Reduce contract contributes
2,070 target-only outputs across nine operation-specific `NO_PILLOW_SUPPORT`
modes, 115 size/pattern points, and two factor shapes (`2×2`, `2×3`). Its u16
and f32 results retain native sample types; GPU workloads remain excluded
where no typed admission exists.

The matrix generator and its manifest/index, asset, profile, and workload-ID
checks completed in the latest committed checkpoint. `make quality` passed on
the 2026-11-12 checkpoint and included its 4,730 indexed parity documents.
The current regular-polygon specification and input generation completed
successfully, and focused parity/benchmark results are summarized above.
Push-time checks on 2026-11-12 passed: `make quality`, `make docs-lint`,
`make fmt clippy`, and the
implementation-structure/repository-map audit. Clippy reported existing
diagnostics but exited successfully. Full CPU parity and benchmark campaigns
remain gated on Stage 5 source closure. No coverage was collected.

The 2026-11-13 GetData checkpoint passed its focused 20-case CPU-default parity
run, `make fmt`, `make clippy`, `make docs-lint`, `make checkstyle`, and the
large-file and diff checks. Its attempted `make quality` run did not finish:
the target started full parity-input regeneration across the 13 GB local input
tree and was interrupted. Do not count that invocation as a quality pass. The
full frozen CPU parity and CPU benchmark campaigns remain gated on Stage 5
source closure; no coverage was collected.

2026-11-13 push checkpoint for the F32 Reduce helper placement: the structure
audit and full structure inventory passed with zero unclassified paths;
`repo-map-update` and `repo-map-check` passed; `make fmt`, `make docs-lint`,
and `SCCACHE_NO_DAEMON=1 make clippy` passed. Clippy reported existing warning
diagnostics. `make build-parity-cpu-architecture` passed. Pillow parity case
`PIL.Image.Image.reduce.mode.f.matrix-size-1280x720` passed 1/1 under CPU NEON;
its execution receipt records `selected_architecture=neon`. This case exercises
the guarded NEON 2x2 implementation. The shared portable vector helper was not
executed on this ARM host; x86 and Wasm runtime paths remain unverified. No
benchmark or coverage ran. Stage 5 and the complete CPU parity campaign remain
open.

2026-11-13 CPU path correction: moved the portable F32 Reduce vector helper
from `reduce/f32/kernels/vector.rs` to `reduce/f32/helpers/vector.rs` and
updated the AVX, SSE2, NEON, and Wasm SIMD128 leaves to call that mode-local
helper. The arithmetic, guard, and fallback are unchanged; function VERIFY
markers record the source review, with parity and benchmark still pending. The
empty Chops LAB kernel directory was removed. The four remaining `kernels`
groups are beneath shared helper scopes, outside the exact family/mode/arch
path. No build, formatting, lint, test, parity, benchmark, or coverage ran;
Stage 5 remains open.

The generated public-contract page is stale: it records manifest SHA-256
`3e36271fae8f7f25405a2ff961ea338992b7f3b80679c4b8460ed2c8ef56bc4a`, while the
current manifest hashes to
`aea5f40d617175d88bc18ab2515bac729e44b6c097024fc9b41724bb18ae6445`. Parity,
benchmark, coverage, and target-output evidence pages also contain older
status data. TODO(pillow-rs#3ptb): after Stage 5 closes, regenerate the public
contract and refresh evidence views from the current frozen input index; keep
the older unindexed CoreOnly shards and duplicate benchmark documents
preserved until their differing assets and outputs are reviewed.

The Rust source inventory still has 113 OperationId values × 25 exact modes
(2,825 source-mode outcomes); mapping that denominator to public requirements
and composed routes remains open. CPU source closure remains open, so parity
and benchmark campaigns remain gated until source review is complete.

2026-11-13 GetData wrapper source correction: Python `Image.getdata()` and
`get_flattened_data()` now use the Rust-backed image's `bands` property,
which reads `raster::Mode::band_count()`, instead of inferring tuple width
from the Pillow mode string. `_image_data_bands()` remains only at the
explicit `ImagingCore(bytes, mode, size)` boundary, where raw bytes have no
Rust image identity. Source review confirmed GetData materializes before
reading `bands`, and the internal high-depth GetData strategies return one
byte per logical band for their CoreOnly projection. The target-output input
generator now defines both GetData and flattened-data cases across the existing
CoreOnly size/pattern matrix, plus benchmark rows for those exact mode routes.
Expected pixel bytes use a compact packed contract with mode, size, and band
metadata; the runner compares every projected byte against the public
GetData result. The target-only matrix generator completed and added 1,150
tracked benchmark rows (five modes × two methods × 115 size/pattern points).
The focused CPU-default run passed 20 varied/boundary cases at 1×1 across all
five modes and both methods (`migration-parity-dee7a1fa052b47138d0e3c735ad78c01`,
manifest SHA-256
`aea5f40d617175d88bc18ab2515bac729e44b6c097024fc9b41724bb18ae6445`). Full CPU
architecture receipts and performance measurements remain pending Stage 5
closure; full source-to-operation closure remains open.

Baseline: `3f9dfd4036caa061eabcf2255c9f34a745df9c7e`; continue from the current
`develop` HEAD and preserve all later WIP commits. Earlier CPU family and
direct-call source reviews covered operation/Mode/Architecture contracts,
prepared routes, and fallback paths. The clean run on `ba4fc8e9c` is historical:
the full 17,616-case selection ran under all nine CPU selectors, with 17,615
passing and one external I32-to-PNG encoder failure per selector. The twenty
recorded Rust-only outputs and errors passed in that run. x86 selectors fell
back on arm64, so native x86 execution remains unproven. The historical Rust
coverage campaign completed 27 plans and 17,377 cases with no plan failures;
active `pool_cpu` source line coverage was 39.0%. This broad figure identifies
unexercised code but does not prove complete operation × Mode × Architecture
behavior.

The prior generated CPU fixture snapshot included typed-u16 Chops Offset
expected-output contracts. That 2026-11-10 snapshot contained 25,003 cases:
the CPU profile selected 25,002 (24,750 Pillow-parity cases and 252 recorded
expected-output cases, excluding one GPU-only fault contract). The ImageChops
input contained 10,070 cases, including 204 expected-output cases. Its case
IDs have SHA-256
`a2005623bb7f9bcf243c030cc5e12ff4e028858b423427a892247e1fc96cf619`.

Historical Pillow 12.2 CPU parity and execution receipts ran for that snapshot on 2026-11-10
under Default, Neon, SSE2, SSE3, SSSE3, AVX, AVX2, AVX+FMA, and AVX2+FMA. Each
selector executed all 25,002 cases: 25,001 passed, including all 252 recorded
expected-output cases, and one Pillow-parity case failed. The sole failure is
`PIL.Image.Image.save.nuanced.i-png`: Pillow successfully writes the I-mode
image, while the current image-slash-star PNG encoder returns
`PNG cannot encode this image mode`. There were zero infrastructure errors.
This is tracked as `image-slash-star#9h5w`. Reports and execution receipts are
under `build/migration-parity/cpu-architecture/`. The x86 selectors used their
declared fallback on this ARM64 host, so native x86 execution remains
unproven.

Crop mode matrix update (2026-11-10): Crop has manifest requirements for all
25 exact raster identities: 20 Pillow modes and five `NO_PILLOW_SUPPORT`
internal modes. Each Pillow mode has a distinct nonzero 2x2 `Image.frombytes`
workflow that crops the right column. This replaces the zero-filled 16x16
inputs for ten pre-existing direct mode cases; nine previously missing modes
and a dedicated RGB mode case complete the 20-case Pillow matrix (RGB was
previously covered only incidentally by the default workflow). The five
internal modes have fixed typed-sample expected outputs for the same crop.
This initial Pillow 12.2 revision contained 25,018 cases; its CPU selection
contained 25,017 cases and is superseded by the Pillow 13.0 corpus described
below. The input check passed for the 12.2 revision. None of its changed Crop
inputs had run on that revision, and its receipts cannot be reused after the
oracle and input changes.

Pillow oracle upgrade (2026-11-10): the active parity oracle is Pillow
13.0.0. Pillow 12.2.0's I;16 `Offset` implementation wrote one byte per pixel
into two-byte storage, leaving half of each output row uninitialized; this is
a Pillow bug, not a stable output contract. Pillow 13.0 fixes it, so the
I;16/I;16L/I;16B/I;16N Offset workflows now compare directly with the live
13.0 oracle and use native u16 wrapping semantics. The old recorded-output
exception was removed. Pillow 13 also removed `Image.get_child_images`; its
retained pillow-rs API is now explicitly target-only with recorded empty-list
outputs. The 13.0 generator and input validation pass. The current corpus has
25,022 cases; CPU selects 25,021 (24,790 Pillow comparisons and 231 recorded
outputs), with case-ID SHA-256
`42905f5f4dab5716889c6d227bf86b599398208700fec23ea3e955300e9678f9`. A focused default-CPU run passed all 20 selected I;16-family Offset cases
(run `migration-parity-a666e1ff6f744ffabf568ec61af9c7ca`). This selection was
refreshed against Pillow 13.0.0 on the current `develop` checkout: all 20 cases
passed under each of the nine CPU architecture selectors (180/180, zero
infrastructure errors). Receipts are under
`build/migration-parity/pillow13-i16-offset-refresh/`; the input SHA-256 is
`8252392cd4a619347f3fb784a279d18b57d0cd51731531870116dd4b00092440`. On this
ARM64 host the x86 selectors use their declared fallbacks, so native x86
execution remains unverified. The active 25,021-case CPU corpus then ran once
on default CPU: 24,718 passed, 303 failed, and zero
infrastructure errors (run `migration-parity-d122153930244ee5b72d0cd5722c7196`).
Its per-operation failure list is in the inventory; it does not cover the full
mode/architecture matrix. Pillow 12.2 receipts are historical and incompatible
with the active oracle. See `pillow-rs#3ptb`.

Current full-corpus rerun (2026-11-10): after the Pad, Resize, and Thumbnail
fixes, commit `147add214ff8d09e0126fc1a5e5fa0f64b6556ec` executed the same
25,021 CPU/default cases against Pillow 13.0.0: 24,916 passed, 105 failed, and
there were zero infrastructure errors. All 231 recorded expected outputs
passed; Pillow comparisons passed 24,685/24,790. Run
`migration-parity-1f1b8167163a424bbafdf84b592cba20`; the report and execution
sidecar are under
`build/migration-parity/pillow13-cpu-post-pad-resize-thumbnail/`. The sidecar
classifies 16,002 pipeline-applicable cases: 15,935 complete, 66 missing a
pipeline receipt, and one partial. This remains default-architecture evidence,
not the full strategy matrix. All Pad, Resize, and Thumbnail cases pass in
this run.

Rounded-rectangle correction (2026-11-10): the Pillow adapter now groups flat
coordinates into pairs before unpacking, preserving Pillow's malformed-input
error. Rust computes the initial diameter as `min(width, height, 2 * radius)`
before rounding coordinates, matching Pillow's behavior for zero-height boxes.
All 31 frozen rounded-rectangle cases pass against Pillow 13.0.0 under all nine
CPU architecture selectors (279/279, zero failures and infrastructure
errors). Receipts are under
`build/migration-parity/pillow13-rounded-rectangle-cpu-architectures/`; the
input SHA-256 is
`b2caf6f94976c15261372f0f7121bc4a82cfe34e4a7a6925feda156c49e0999b`. This
ARM64 run exercises Default and Neon; native x86 execution remains unverified.
The latest full-corpus report still predates this correction, so its 15
rounded-rectangle failures are historical. No benchmark or coverage ran.

Image.frombytes buffer-boundary correction (2026-11-10): the latest full CPU
report contains five direct `Image.Image.frombytes` cases where a tuple was
accepted by Python's `bytes()` conversion, then misreported by Rust as a short
buffer. The wrapper now passes the original object to the PyO3 boundary; that
adapter accepts bytes/bytearray without an extra conversion, otherwise asks
for a buffer view, and preserves Pillow's bytes-like `TypeError` rather than
iterating a sequence. Empty instance images return before inspecting decoder
or data, and empty class inputs ignore both as Pillow does. The instance
facade now reads dimensions before `self.mode`, so an empty lazy image returns
without a mode lookup, matching Pillow's instance method. These changes are
source-only and unverified; the old five-case failures remain the last runtime
evidence. The Rust boundary now checks Pillow's list/tuple type, length, and
nonnegative dimensions before parsing mode, then performs C-int conversion.
`RawDecoderPlan` now carries typed target Mode, validated raw layout, stride,
and orientation. The thin Python facade forwards variadic arguments for both
class and instance calls, including Pillow's single-tuple argument form. Core
supports packed 1/2/4-bit rows, typed 16/32/64-bit scalar conversion, 16-bit
color samples, channel extraction, line-interleaved LA/PA input, and the
mode-specific color transforms, including YCC/YCCA to RGB/RGBA. The raw layout
never changes output Mode. The plan is prepared before buffer extraction,
while stride and payload checks occur after the host obtains a contiguous
buffer. Source-only and unverified. Rust also accepts packed `L;1` as an
explicit extension: Pillow 13 declares that raw-mode identifier but has no
matching `Unpack.c` decoder entry. Static source crosswalk (2026-11-10):
compared every Pillow 13 table row, including native-endian conditional rows,
against the maintained mode/rawmode dispatch in `layout.rs` and
`layout/rgb_alpha.rs`. The declared Pillow pairs have a corresponding exact
mode strategy; `L;1` remains an explicit target-only extension. This checks
table membership, not each transform's arithmetic. PhotoYCC transform review
(2026-11-11): compared the maintained implementation with Pillow 13
`UnpackYCC.c`. All five 256-entry lookup tables match byte-for-byte. RGB output
uses the same lookup indices, channel sums, and 0–255 clamp; YCC sets the
fourth physical lane to 255. YCCA divides each premultiplied input lane by
alpha using integer truncation (zero alpha maps the three color lanes to zero),
applies the same PhotoYCC transform, and preserves alpha. The Rust route has
the same arithmetic and lane behavior. This is source review only; it does
not replace the pending parity matrix. TODO(pillow-rs#3ptb): review the
remaining supported mode/rawmode strategies in parity, then freeze the
combined class/instance rawmode, stride, orientation, tuple-flattening,
buffer, and empty-input matrix. The `L;1` `NO_PILLOW_SUPPORT` expected-output
cases already exist in the indexed corpus; verify their CPU architecture
receipts with the rest of the frozen matrix. A full source comparison of the
current `RawDecoderTransform` variants is now complete: packed L/P bit and
plane layouts, channel and high-byte extraction, 15/16-bit packed color,
native 12-bit samples, typed scalar conversion, byte order, channel order,
premultiplied alpha, CMYK, LAB, and PhotoYCC match their Pillow 13 decoder
formulas. This closes formula review only, not rawmode parity or wrapper/fault
verification.
No build or parity evidence is fresh for this change.
Pillow 13.0 contract references: [raw decoder argument parser](https://github.com/python-pillow/Pillow/blob/13.0.0/src/decode.c#L3117-L3158), [raw row stride/orientation](https://github.com/python-pillow/Pillow/blob/13.0.0/src/libImaging/RawDecode.c#L25-L88), and [mode/rawmode pairs](https://github.com/python-pillow/Pillow/blob/13.0.0/src/libImaging/Unpack.c#L1456-L1738).
Arithmetic references: [packed and typed unpackers](https://github.com/python-pillow/Pillow/blob/13.0.0/src/libImaging/Unpack.c#L248-L1451), [mode/rawmode dispatch](https://github.com/python-pillow/Pillow/blob/13.0.0/src/libImaging/Unpack.c#L1456-L1738), [CMYK conversion](https://github.com/python-pillow/Pillow/blob/13.0.0/src/libImaging/Convert.c#L495-L507), and [PhotoYCC](https://github.com/python-pillow/Pillow/blob/13.0.0/src/libImaging/UnpackYCC.c#L16-L157).

Focused GetBBox correction (2026-11-10): `I;16`, `I;16L`, `I;16B`, and `I;16N`
must scan typed u16 samples as logical pixels. Replacing the byte-prefix scan
with the existing native typed-sample scan passes all eight affected cases
(run `migration-parity-5f4e4f80ca094dc29f79030cc10bac94`, case-ID SHA-256
`b00da5c775ce081e39e80d425c2185db977b81af0ccf2dce87df770f8d258601`). The
full corpus has not been rerun after this correction; the latest 105-failure
report therefore still includes these eight now-passing cases.

Focused masked-LA Histogram correction (2026-11-10): Pillow 13.0 counts the
stored luma and alpha samples when an `LA` image is histogrammed through an
`L` mask. The LA strategy now reads both typed samples directly instead of
substituting its first band for alpha. All eight previously failing cases and
one existing masked-LA guard pass (9/9; run
`migration-parity-7d5185a8599a4170911c0cdd6138b503`, case-ID SHA-256
`4cedb507a406e1967ced5129ec03690b4c294f070a1ef274f2fd0452f54dfe01`). The
full corpus has not been rerun after either focused fix.

Pad follow-up (2026-11-10): after the full-corpus baseline above, the maintained
Pad route was updated for Pillow 13 zero-axis containment, P-mode fill palettes,
and native typed color-name handling. All 292 active parity cases that call
`ImageOps.pad` pass on default CPU against Pillow 13.0.0, with zero
infrastructure errors and no backend or strategy fallback. Run
`migration-parity-cc84997a45574896b5b91850db98c7ff`; its execution sidecar is
under `build/migration-parity/pillow13-pad-all/`. The run uses the same manifest
SHA-256 above and a dirty target based on `a8ba7b37e`. The 303 full-corpus
failures are a pre-Pad-fix baseline. The later full-corpus run at `147add214`
passes Pad, Resize, and Thumbnail; its 105 remaining mismatches are listed in
the inventory. The focused GetBBox correction passes its eight I;16-family
cases.

Live operation support census (2026-11-10): the current support API reports
113 operation identities × 25 Modes = 2,825 source-mode outcomes: 1,406
Pillow-supported, 460 CoreOnly-supported, and 959 rejected. This is a source
denominator, not parity evidence; target-mode pairs, secondary inputs, guards,
callbacks, metadata, faults, and execution receipts add cases.

The updated selection is still not the complete operation × 25-Mode ×
Architecture matrix required by Stage 7. The prior corpus also has one shared
external I-to-PNG encoder failure per selector. Continue expanding the
support-declared outcomes and relevant secondary-mode combinations, then
freeze and execute the full matrix; after the codec fix, rerun the affected
save case in the final build. Native x86 execution and CPU performance remain
pending. See the append-only checkpoint notes below for operation-specific
evidence.
The public support declarations classify 113 operation identities across 25
source modes; parameterized targets, secondary inputs, and route guards add
cases beyond that source-mode denominator. `pool_parallel` remains future work
and must wait until CPU acceptance. See the append-only checkpoint notes below
for historical evidence and limitations.

Lazy indexed mode-planning correction (2026-11-08): `known_pipeline_op_mode`
uses accepted P/PA output contracts for fixed or explicit target transitions,
and now resolves accepted `Preserve` routes after palette expansion from
wrapper metadata: PA expands to RGBA; P expands to RGB or RGBA when a palette
is attached, according to its alpha table. Missing-palette P routes, rejected
routes, and other unresolved support contracts retain materialization so
admission and fault order are not guessed. This source change is uncompiled
and unvalidated; remaining lazy-mode transitions and CPU gates are still open.

Thin-host ImageOps source update (2026-11-05): `pillow-rs` now owns the
callback-driven geometry algorithms for custom `contain`/`cover` sizes,
`fit` crop-box arithmetic, custom `scale` validation/dimensions, and custom
`crop` border expansion/box arithmetic plus custom `expand` border/dimension
arithmetic. Python keeps input unpacking,
native-route eligibility checks, Python protocol callbacks, and public
`copy`/`crop`/`resize` dispatch. Follow-up callback strategies moved Expand
fill/palette/canvas/paste orchestration and Pad's custom fallback composition
into core. The native Pad route now also sequences geometry preflight, filter
admission, palette realization, mode-specific fill preparation, centering,
and the final native result in core; PyO3 reports host eligibility facts and
adapts Python protocol calls and wrapper construction. The Python facade no
longer defines standalone Pad route predicates. Remaining host wrapper calls
are individual public-method boundaries rather than ImageOps algorithms.
This source work remains unverified; no build, format, checkstyle, lint, test,
parity, benchmark, or coverage ran.
Operation registry census (2026-11-05): all 103 `OperationId` variants have
support dispatch and active-source references. `PipelineOp::operation_id()`
maps 75 IDs; the other 28 belong to direct or eager paths and need their
caller-level context, exact-mode selection, and receipt review. This census
does not establish support completeness or execution correctness. No checks or
tests ran.
Direct/eager caller audit checkpoint (2026-11-05): source review also traced
ImageOps Invert's wrapper contract to its exact-mode Invert child, the three
eager generators through routed context and retained selection receipts, and
the production PutPixel batch run through `pool_cpu/batch_execution.rs`.
Synchronous `putdata` I;16 byte writes, value-at writes, and scalar `putpixel`
routes select against their captured mode/context and pass that selection to
the writer. This is a caller trace, not a complete 28-operation audit or
runtime evidence; the remaining direct/eager callers, validation order, and
receipt contracts remain open. No build, formatter, checkstyle, lint, test,
parity, benchmark, or coverage ran.
Follow-up source audit (2026-11-05): the caller trace now covers all 28 direct
or eager identities listed in the inventory, including analysis, channel and
data access, Split, Quantize, point operations, filters, generators, and
mutation. Kernel-backed paths prepare the declared exact-mode selection from
the routed context and pass it into execution; identity/copy paths and
composed wrappers are explicitly called out where they skip kernel selection
or defer a child. This closes only the source-wiring census. Output, fault
order, callback order, and route/receipt parity remain open, including Split's
lazy child cases and parity for `GetColors` negative-limit validation and
materialization before its no-scan `None` result. Source review confirmed that
the negative path validates the exact-mode support contract and materializes
the image without preparing a color-count scan. No build, formatter,
checkstyle, lint, test, parity, benchmark, or coverage ran.

2026-11-22 ImageChops input-order source recheck: the public Python facade
loads the first and second images in that order before invoking the private
PyImage operation, and `Image.load()` materializes the underlying Rust image.
The PyO3 helper's result load therefore follows already-loaded inputs on the
public ImageChops path; no extra input loads were added at that boundary. The
remaining `chops.rs` TODO is limited to direct Rust lazy constructors, whose
mode-validation versus secondary decode-error order is distinct from the
Python facade contract. Output/fault parity and selected/fallback receipts
remain pending; no validation command ran.

2026-11-22 GetColors visible-mode source recheck: Pillow loads the image, then
tests the overridable `self.mode` against `("1", "L", "P")` to choose between
its histogram wrapper and low-level `ImagingCore.getcolors`. The Python facade
performs that same branch and passes its boolean result. Rust selects the pixel
algorithm from the actual `DynamicImage` identity, so the branch bool is the
complete host carrier; no second visible-mode enum or string needs to enter
core. `maxcolors` remains a Python object on the histogram route so Pillow's
post-count comparison is preserved. Output, mode-override fault, and route
receipts remain pending; no validation command ran. Reference: [Pillow 13.0
Image.getcolors](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L1375-L1396).

2026-11-22 Equalize builtin-mutation scope decision: replacing Python builtins
before or after import, including C-level replacements with forged metadata,
is outside the supported compatibility contract. Do not add builtin identity
guards, mutation-specific fallback logic, or parity cases. The generic
histogram callback path remains for non-native histogram values; its output,
fault order, and route receipts remain pending under the existing
`pillow-rs#3ptb` TODO. Earlier notes requesting builtin-replacement parity or
identity proofs are superseded. No validation ran.

2026-11-22 builtin-mutation scope applied to AutoContrast: the native LUT
admission path no longer checks the live identities of Python builtins.
Explicit `pillow_rs.imageops` module-global overrides remain detectable and
retain their host-callback route; builtin mutation is outside the contract.
The ordinary native path and module-global fallback still need output, fault,
and route-receipt parity under `pillow-rs#3ptb`. No validation ran.

2026-11-22 ImageFilter callback identity correction: `Image.filter` now passes
the live `PyImage` handle to both `ImageFilter.Filter` subclasses and
duck-typed filters. The prior route-bound clone could hide callback mutations
from `Image.im`; only context-aware ImagingCore methods resolve the
already-captured route from the active wrapper scope. Built-in filters that take explicit
execution context retain that override. Callback output/fault order and
selected/fallback receipts remain TODO under `pillow-rs#3ptb`; no validation
ran.

`PyImage.getpalette(*args)` now supplies the direct ImagingCore palette read for
custom filters. Rust packs the attached palette into the requested raw mode;
the adapter returns the packed bytes and raises Pillow's no-palette error.
Output bytes, rawmode faults, and callback behavior remain in CPU parity.
Autocontrast source correction (2026-11-06): the current core callback path
sequences preserve-tone conversion, histogram, channel LUT construction,
post-LUT mode admission, and public `point` dispatch. The Python adapter keeps
the required cutoff numeric protocols, while Rust owns channel iteration and
LUT construction. The typed Rust API computes histogram/LUT work before
rejecting an unsupported mode. This supersedes the earlier source note that
described early mode admission and f64 cutoff loss. At this checkpoint custom
histogram channel objects and callback parity were pending; the later callback-
preservation correction and focused verification below address the narrowing
defect. Full callback fault-order and architecture receipt coverage remains open.
Current source status supersedes older checkpoint notes below: Convert has
direct source-to-LAB target rows for all 25 exact source modes. Each
nonidentity source owner writes the final LAB raster from native samples using
the shared LAB kernel; LAB-to-LAB remains its exact-mode identity path. The
`La` route is an explicitly declared Rust-only extension and is marked
`NO_PILLOW_SUPPORT`, so it is excluded from Pillow-parity claims. The Rust core
owns multiline layout, bounds, rendering, and the `ImageText.Text`
`get_length`, `_split`, `get_bbox`, and `wrap` state machines. Draw uses the
same core line-placement implementation after the Python facade resolves inks
and palette allocation. `Text.wrap` also uses the core state machine for custom
font callbacks, custom numeric protocols, string/bytes subclasses, and
path-backed scaling; PyO3 adapts those host calls while Python constructs and
mutates the public `Text` and remainder wrappers. These host adapters preserve
Python protocols and call order; they do not contain a second wrapping
algorithm. The public contract remains Pillow-facing: accepted argument kinds
and signatures, return types and shapes, output values, errors, and observable
mutation/fault order must match. Text output, fault, callback, and strategy
selection parity is pending the CPU gate.
Python strings containing lone surrogates use the shared, doc-hidden Rust
`ImageFontTextInput::TextCodepoints(Vec<u32>)` transport variant across the
existing Pillow-facing `FreeTypeFont.getlength`/`getbbox`/`getmask`,
`ImageDraw.text`/`textbbox`/`textlength`, and `ImageText.Text.wrap` paths,
including multiline layout. Pillow's FreeType fallback reads Unicode by
codepoint and its RAQM path copies Unicode to UCS-4; `ImageText.Text.wrap`
keeps and slices the original Python string before measuring candidate lines.
The carrier preserves that input across all shared consumers.
Python method signatures, accepted `str`/`bytes` arguments, and result shapes
stay unchanged. This applies to FreeType-backed paths; bitmap `ImageFont` uses
its distinct Latin-1 byte-text contract and must retain its own conversion and
encoding errors. Nine input-only parity stimuli cover lone-surrogate handling
across font, Draw, and wrap entry points, byte-text wrapping, and scaled font
mutation; generated inputs and parity have not been run. Reference: [Pillow
12.2 `_imagingft.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imagingft.c#L380-L464).
Pillow 12.2 `ImageText.Text` construction and `Text.wrap` are now declared in
the parity-inventory source; manifest and input generation remain pending. The
workflow records constructor state, both the returned remainder and the
receiver's public state after the mutating call, including `text` and font
selection. The string-surrogate and bytes wrap cases exercise those outcomes;
callback and fault ordering still need the CPU parity pass
(TODO(pillow-rs#3ptb)). The Rust codepoint carrier does not change the
Python-facing API. Reference: [`ImageText.py` wrapping and slicing](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageText.py#L19-L87).
Pillow's public implementations are in
[`ImageFont.py`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageFont.py),
[`ImageDraw.py`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py),
and [`ImageText.py`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageText.py).
The wrapper and native paths use the same Rust wrapping state machine.
One-mode matrix conversions now use the exact-mode `ConvertMatrix` strategy
directly. Its native source iterator expands each bilevel sample to 0/255 before
applying the matrix; the wrapper no longer allocates an intermediate L raster
or changes the selection identity from One to L. For three-band sources with
transparency, the wrapper now recomputes the copied transparency key only after
matrix pixels and `_new` complete, preserving Python sequence and numeric
protocol order. Matrix pixel and transparency output, fault ordering, and
selection-receipt parity remain pending.
Font and Draw option modes now use typed `raster::Mode` inside Rust. Python
mode spellings are translated at the PyO3 boundary; adapters format the
selected font-mask mode only when calling the host font methods. This removes
string-based raster modes from those core font and Draw strategies without
changing their rendering algorithms. Output, fault-order, and Draw route parity
remain pending.
Transform MESH now allocates one mode-tagged output before iterating Python
records, computes QUAD coefficients in Python, validates each record's filter,
loads the current source, then writes the record before requesting the next.
The selected CPU architecture is captured once and each record prepares its
exact-mode Default fallback. Byte, scalar I/F, and native I;16 writers reuse
the same destination; alpha regions and omitted physical lanes are updated per
record. A source-mode change between records returns Pillow's `image has wrong
mode` contract; output/fault/callback/selection parity and full source review
remain pending. No validation has run for this work.
Non-MESH built-in methods now allocate an exact-mode destination and capture
their execution context in `_transform_prepare`, before Python method-data
callbacks. The selected mode/architecture strategy writes directly into that
storage for byte modes, I, F, and each I;16 byte order; it does not create a
second full output raster. MESH starts from the same prepared destination.
The non-MESH method dispatch now evaluates user-defined method comparisons
once while shaping method data; a redundant pre-validation comparison chain
was removed. Callback, palette, output, fault, selection, and parity review
remain pending; no validation has run.
Transform receipt source correction (2026-11-05): the non-MESH direct-destination
path now records `cpu` for a selected Default strategy and `vector` for a
selected architecture kernel. MESH keeps its `cpu` receipt because its support
guard restricts it to Default. This corrects telemetry classification without
changing strategy selection or pixel execution. Source-only; output and receipt
parity remain pending, and no build, formatter, lint, test, parity, or benchmark
ran.
ImageOps Colorize now delegates its full LUT strategy to `colorize/ops.rs`:
core performs typed L-mode admission, ordered point comparisons, channel-list
construction, range iteration, per-component Python arithmetic callbacks, and
Convert→Point orchestration. PyO3 adapts the host sequence, indexing, range,
length, and arithmetic protocols without narrowing point values or colors;
string parsing uses the core color parser. The obsolete binding LUT shortcut and
range-length API were removed. Direct typed Rust Colorize remains available.
Output, fault-order, mode, and architecture parity remain pending; source only.
`ImageOps.scale` keeps factor-one `Image.copy()`, non-positive checks, and the
public `Image.resize()` dispatch in Python. Typed factors on the base Image
route now use Rust's geometry preflight for ties-to-even rounding and
NaN/overflow outcomes; custom numeric protocols or overridden properties keep
the existing Python callback path. Scale callback, output, and
mode/architecture-selection parity remains open.
Ordinary typed `ImageOps.contain`, `cover`, `fit`, and `crop` inputs now compute
their dimensions or crop box in `pillow-rs`, then call public
`Image.resize()`/`Image.crop()` so wrapper dispatch and metadata behavior stay
observable. Python retains arithmetic fallbacks for custom numeric protocols,
subclasses, and overridden properties or methods. Callback, fault, output,
and selection parity remains open for both paths. Ordinary typed
`ImageOps.pad` inputs, including P/PA, now use the typed Rust Pad orchestration
for fill resolution, palette propagation, and canvas composition. The binding
keeps Pillow's order by preflighting geometry and filter, invoking dirty
palette realization, snapshotting the current raster after callbacks, and
then adapting fill and centering inputs. Python's `contain`/`Image.new`/`paste`
path remains only for custom inputs and overridden wrapper methods. The
remaining custom-protocol arithmetic and callback paths are listed migration
debt; keep algorithms in `pillow-rs` whenever the required Python protocol can
be represented as a typed input.
For exact FreeType `ImageText.Text`, `_split`, `get_bbox`, `get_length`, and
`wrap` now call core font/layout APIs shared with Draw. Python keeps public
wrapper mutation and variant-object installation on that path. Custom font
callbacks, unusual Python protocols, and path-backed scaling still use the
compatibility wrap loop; move its decisions into core while keeping Python
calls only as a host adapter for callbacks and file access.
`ImageOps.deform` now calls `getmesh` before passing the selected resample mode
to public `Image.transform`, matching Pillow's callback and filter-validation
order; the redundant prevalidation binding is removed. Deform output/fault and
selection parity remains open.
`ImageOps.exif_transpose` now loads before EXIF inspection and updates the
wrapper `info` mapping only when orientation changes. It carries the core's
updated EXIF bytes into existing `exif` and raw-profile entries. Rust now owns
the two XMP orientation-marker rewrites over Unicode codepoints, byte buffers,
and byte tuples; PyO3 adapts those host values and preserves Pillow's
`re.sub` callback route when it is overridden or a value is not bytes-like.
Python retains only the `info` mapping membership, lookup, and assignment
sequence. The first `in_place` truth test now follows the EXIF orientation
read; the wrapper retains the original value for the later metadata-target and
return tests. Unchanged orientations defer copying to public `Image.copy()`,
and the EXIF/raw-profile metadata branch follows Pillow's `if`/`elif`. Exact
`Exif.tobytes()` serialization and XMP output/fault/callback parity remain
open; this source change is unverified.
The direct-instruction reachability reviews for Paste, Color Convert, and
Composite found those instructions behind architecture and target-feature
gates. The reviewed operation support declarations constrain selection by
mode, architecture, and operation guard; executable strategies consume the
prepared selection and retain same-mode Default fallbacks. The broader audit of
instruction-free architecture wrappers and other CPU families remains open.
The maintained-source scan found 27 direct architecture-instruction files in
nine operation families; their support/dispatch paths have now been traced.
This does not close the broader module-declaration or prepared-selection audit.
Targeted LumaA8 leaf tracing found three different module patterns: Transpose
gates its architecture modules at the mode boundary; Paste declares portable
architecture wrappers unconditionally but matches each selected architecture
to a concrete execution arm; Enhance also exposes wrappers while its support
declarations gate build availability and its selected vector routes share the
wide kernels. Therefore parent `mod.rs` declarations alone do not prove which
instruction leaves are reachable or dead. The remaining all-family review must
reconcile module reachability, support/build requirements, and execution arms;
the CPU ops facade retains this actionable source TODO. Architecture/fallback
receipts remain unverified.
2026-11-23 context handoff source trace: the unpinned `CpuPool` entry resolves
the CPU context once, while its context-aware entries forward the supplied
value to batch preparation. `execute_prepared_on_backend` passes that prepared
context and palette through the CPU/SIMD-compatible route; lazy materialization
uses the pipeline's injected context when preparing the operation. A targeted
active-source scan found no `ExecutionContext::new` or host architecture
detection inside CPU operations, and no active `pool_cpu` reference to legacy,
`pool_simd`, or Rayon. This traces handoff only; it does not prove strategy
availability, selected/fallback receipts, or output behavior. The palette-
target lazy Convert wrapper route is CPU-owned even if the pipeline router
selected GPU; `PreparedExecution` now retains an injected CPU context separately
from the GPU context so that this path honors its caller's Architecture. When
no CPU context was injected, its existing CPU Default fallback remains. Exact
mode output and selected/fallback receipts remain pending.

2026-11-23 nested context source audit: compared every `PipelineOp` image-
valued field with recursive backend locking and `ExecutionContext` injection.
Both walkers cover secondary operands, composite and paste masks, alpha
composite sources, Merge bands, and optional histogram masks. `PutAlphaData`
carries an already-materialized `DynamicImage`, not a lazy `Image`, so it has
no child pipeline to bind. This closes the source-level child-field crosswalk;
nested selected/fallback receipts and results remain unverified.

2026-11-23 architecture-module source inventory: the active CPU tree has 832
source-bearing architecture module directories declared across 242 mode-level
`mod.rs` files. A refreshed source scan finds 282 declarations with local
target `cfg` attributes and 550 unconditional declarations; this supersedes
the earlier 228/604 split. Every source-bearing directory has a matching
parent module declaration. The 66 architecture leaves with no direct
reference in their mode folder are reached through the shared ImageOps
inversion and Transpose strategy macros; those macro paths were traced to
their explicit mode support and execution arms. The sibling-file scan finds
explicit Architecture references for the other 766 paths. The 15 ImageOps
no-direct-reference leaves are instantiated by `declare_invert_native_mode!`;
the 51 Transpose no-direct-reference leaves are instantiated by the 13
native-byte and four native-I;16 mode declarations. ImageOps uses the same
Invert macro in other modes with separate direct architecture references.
These shared macros define the mode support, target guards, prepared
architecture variants, and execution matches.

Among the direct-reference folders, Put has 57 paths across 16 modes. Their
mode-level files pass architecture and module identifiers into shared
strategy macros. Those macros generate per-operation support entries and
prepared execution arms for native writes and related routes; a literal
`sse2::`-style path is not present in the mode file. This explains the
architecture folders whose dispatch is macro-bound, but does not close the
per-operation support/guard audit.

A static cfg-implication scan checked parent-module availability for the 57
Put macro paths and 75 ImageOps-Invert/Transpose paths (24 plus 51). Across
the modeled x86, x86_64, aarch64, and wasm32 target-feature combinations,
every strategy availability cfg implies that its parent module is included.
This does not prove instruction-level feature safety, compiler reachability,
or runtime execution.

Color has a separate declaration layout: Convert architecture support and
dispatch for RGB8, RGBA8, and RGBX8 live in each mode's `convert.rs`, rather
than `ops.rs`. All 15 source-bearing mode/architecture paths (three modes by
five architectures) have a module declaration, a matching Convert support
entry, and a concrete execution arm. Eight remaining architecture-named
directories contain no Rust source and therefore declare no modules. A
static cfg implication scan evaluated 66 Convert availability predicates
across these 15 paths; every support-enabled target includes its parent mode
module. This checks module availability, not instruction-level feature safety.
A read-only module-path walk checked 4,836 maintained CPU `mod` declarations
and found no unresolved conventional source paths. This is source evidence,
not compiler reachability proof. Continue the family-by-family
support/build/selection/arm reconciliation before closing the TODO in
`pool_cpu/ops/mod.rs`.
Recent WIP source audits include complete Transpose mode/architecture
declarations, ImageOps Invert mode scope, and the ImageOps point/histogram
strategies. Invert now declares Pillow's P and high-depth rejection contracts
in operation support, records Rust-only modes as CoreOnly, and leaves
output/fault parity pending. Posterize/Solarize/Autocontrast's P paths are
CoreOnly, while Equalize's P-to-RGB transition is Pillow-defined. The
Expand also has an exhaustive 25-mode strategy map with mode-owned fill rules.
The instruction-free wrapper and all-family prepared-selection audits remain open.
Draw's shared mode contract now reports the five internal high-depth identities
as rejected, matching their absent Bitmap/Text strategy folders and the
executor's existing `unsupported image mode` result. Direct Draw output and
fault-order parity remains pending.
Recent source-only checkpoints thread `StrategySelection` through Rotate and
Transform hidden-lane helpers; native-byte Filter extrema, rank, and blur
helpers; LA/RGBA AlphaComposite architecture binding; the UnsharpMask
mutable-byte handoff; and mode-aware `DynamicImage::put_pixel` writes. This
source update also remaps retained physical lanes through
`DynamicImage::copy_within`. `GenericImage::copy_from` is explicitly a logical
pixel copy: its source trait exposes no mode or physical-lane provenance, and
destination-only state follows `put_pixel`; mode-aware raw-lane transfer stays
with operation strategies such as Paste. The mutable-byte accessor documents
its sidecar invalidation contract. A static source scan finds no maintained
Rust imports, module declarations, or `include!` paths reaching
`pool_cpu/legacy`; the archive remains reference-only. A separate static scan
finds no runtime helper parameters carrying a bare `Architecture`; remaining
raw parameters are the const Merge support declaration builder and Analysis's
internal missing-strategy error formatter. Convert's physical-lane review
identified missing fourth-byte propagation for RGB-family-to-YCbCr/HSV routes.
Exact RGB, RGBA, RGBa, RGBX, and YCbCr owners now attach those lanes after
their selected kernels. The next source pass added YCbCr/HSV-to-RGB lane
transfer and LA alpha transfer into RGB/HSV results; LA-to-YCbCr keeps its
opaque synthesized lane. This preserves
observable follow-on pixel behavior without coupling the logical pixel kernels
to Pillow's storage representation. P/PA conversions to RGB, YCbCr, and HSV,
and RGBX/RGBa conversions to RGB, were source-reviewed and use the canonical
opaque byte. Newly produced LA/La/PA conversions use canonical duplicated
sample/index lanes, while same-mode identity retains existing sidecars. The
compact-lane source review confirms CMYK-to-RGB, YCbCr, and HSV synthesize the
canonical opaque X value; no CMYK source lane is copied. Convert-to-LAB now
uses the exact source-mode Convert strategy for every declared source/target
pair. Each supported source strategy maps its native samples directly into the
shared LAB lookup kernel and writes final LAB pixels without building
intermediate L, RGB, or RGBA rasters. LAB has no architecture-specific
Convert target guards today, so an architecture request resolves through the
mode's Default strategy; exact selection receipts remain part of parity. The
same-mode LAB path remains an identity copy, and the existing `La` rejection
remains explicit. `ConvertLab` stays available as
a separate operation for its other callers; Convert no longer composes it as a
child operation. LAB-to-RGB/RGBA/RGBX has a native serial transform and
preserves the auxiliary lane. The declared Rust-only LAB-source routes still
write L, LA, premultiplied, bilevel, palette, I;16, CMYK, YCbCr, HSV, I, and F
outputs directly from generated samples. PA-to-La and PA-to-RGBa use the
RGB-base opaque-alpha result in direct palette-sample loops; PA-to-RGBX uses
Pillow's direct palette route and retains PA alpha. PA declares RGBX and has
no CoreOnly targets. Exact outputs, source × target selection, metadata,
fault-order, and CoreOnly parity remain pending. Official source references
and the remaining Convert mode/target matrix are in the
[CPU refactor inventory](CPU_REFACTOR_INVENTORY.md).

Convert routing clarification (2026-11-04): the exact source-mode support row
and requested target select the Rust strategy directly. Pillow base-mode retry
behavior is used only to derive expected outputs and Pillow support; ordinary
Convert strategies consume the original source raster and write the final
target without materializing an intermediate L, RGB, or RGBA image. This
includes LAB, whose source-specific sample mapping now runs inside each
declared mode strategy before the shared color lookup kernel. A Default
architecture fallback means only that a requested architecture has no declared
kernel. Output, fault, palette, and selection parity remain pending under
`pillow-rs#3ptb`.

Reduce's top-level dispatch has now been source-reviewed across all 25 modes.
Seven byte modes and Pillow `F` declare vector routes; the remaining modes use
their typed Default owners. Prepared execution retains the selected mode and
applies mode-specific physical-lane behavior after reduction. The current
source comparisons cover byte Default geometry/rounding, I32 Default, Pillow
`F` Default and vector arithmetic, typed-u16 storage, and byte-architecture
selection and arithmetic. The archived byte kernel also routes a row narrower
than eight pixels through a partial vector block; zero-width/height outputs
are rejected.
The maintained wide kernel now follows that loop shape instead of routing the
entire narrow row to scalar code. Preserve partial-lane admission and exact
architecture receipts in the parity matrix; this source edit is unvalidated.
Pillow's indexed-mode contract is also source-traced: P and One copy a
factor-one request when its box covers the complete image and crop when the box
is a subregion. Non-identity factors reach `ImagingReduce`, which rejects both
modes. Their typed owners declare identity-factor contracts only;
`reduce_public` copies full-image identity requests and crops factor-one
subregions. Every mode's Default/output identity, hidden lanes, caller/fault
ordering, outputs, and runtime
architecture/fallback receipts remain open; see the Reduce checkpoints below.

The Expand Python boundary now resolves named fills against the initial mode,
snapshots an existing palette's mode before `getpalette` loads the source,
allocates 3/4-component colors through a fresh palette without passing the
source image, and attaches its final data for both P and PA modes. It captures
a target image after palette callbacks, validates size before non-string fill
conversion, and pastes the current source wrapper into the captured-mode
canvas. The exact-mode Rust Expand strategies still perform canvas filling.
Expand and Pad now share the exact-mode host fill adapter. For P-mode RGB(A)
integer lists and tuples it invokes the package's `ImagePalette.getcolor` and
passes the chosen index and temporary palette to Rust; Expand retains that
palette on its materialized canvas. Non-palette tuple arity and component
conversion now use the captured mode's band count and native integer widths.
Dirty/raw palette faults, metadata, allocation order, CoreOnly fill cases, and
runtime parity remain under source review.
For P-mode Pad, the host bridge now routes three/four-component all-integer
lists and tuples through the package's `ImagePalette.getcolor`, carrying the
selected index and temporary palette bytes into the native canvas strategy.
P-mode strings first call the package's `ImageColor.getcolor`, and its result
then follows that same palette route. This preserves iterable and
integer-subclass callbacks before centering conversion. Remaining P values
follow the one-band native fill contract: only an integer or one-element
integer tuple is accepted after the palette route. The eager `Image.new`
allocation/fill-fault/centering order remains TODO, as does runtime parity.
These source changes are uncompiled.
Chops, Pad, Draw, Thumbnail, Quantize, Resize, geometry sidecars, Filter, and
Composite context propagation remain uncompiled and runtime-unverified. The
audit is not a completion proof: operation support declarations, ownership,
source behavior, algorithm coverage, inert-archive review, and function-size
limits remain open. A current physical-line scan shows the largest maintained
Rust files below 1,000 lines: `raster/buffer.rs` 998, `image/materialization.rs`
994, `compute/pool_cpu/ops/analysis/ops.rs` 994, `pipeline.rs` 990, and
`pillow-rs-py/src/image/operations.rs` 985. This is not the required
Clippy-counted function audit. Refresh the complete source and function
inventories only after source closure.

The last focused library compilation passed for the earlier Resize
selection-context refactor with
`RUSTFLAGS='--cap-lints allow' cargo check -p pillow-rs --lib
--no-default-features --features image-codecs-all`. It confirms compilation
only; lint caps suppressed the denied-diagnostic backlog. It predates the
Chops, Pad, Draw, Thumbnail, Quantize, Rotate/Transform, Filter, and Composite
selection-context checkpoints. Current lane-preservation edits have only
received static source review. No formatter, checkstyle, lint, tests, parity,
benchmark, or coverage has run for this source update. This matrix tracks
pending work; it is not support or runtime verification evidence.
Detailed mappings and source TODOs remain in [CPU refactor
inventory](CPU_REFACTOR_INVENTORY.md). Refresh this matrix at each completed
checkpoint.

Header-mode source update (2026-11-02): image-slash-star PR
[#157](https://github.com/appunni-m/image-slash-star/pull/157), commit
`eee65cbdad44594a09e526b11313c7eb4428732a`, adds a codec-owned `Mode` selected
during header inspection and exposes the cached identity from `ImageInfo`,
`EncodedImage`, and `EncodedImageView` without decoding pixels. The Pillow
dependency is pinned to that commit at version `0.1.3` while the PR awaits its
later merge. Its resolved JPEG SIMD dependency also requires `wide` 1.7.1, so
the workspace lockfile is updated accordingly. `pillow-rs` keeps its separate
operation-facing `raster::Mode`: `Image::Bytes` translates the cached codec
mode once at open time, retains the required mode while lazy, and applies it to
Pillow's `DynamicImage` only when pixels are decoded. This preserves Pillow
ownership and does not materialize pixels to discover mode. Premultiplied LA
and big-endian TIFF I;16 retain their exact header identity; associated-LA TIFF
remains marked `NO_PILLOW_SUPPORT` because Pillow 12.2 has no matching TIFF
open-table entry. The Pillow adapter preserves the `I;16L`, `I;16B`, and
`I;16N` identities on the normalized u16 carrier without changing samples;
current header selection emits native `I;16` or TIFF `I;16B`. The Pillow
adapter now has an explicit, header-Mode-keyed relabel from the RGBA carrier to
`ImageRgbPremultiplied8`; it does not inspect codec source metadata. The same
Pillow decode boundary now re-tags only layout-compatible RGB triplets (YCbCr,
LAB, HSV), four-byte RGBX, palette-alpha LA carriers, and normalized I;16
aliases without changing pixel samples. Header-mode versus decoded-raster
parity, including deferred decode failures, remains TODO
under `pillow-rs#3ptb`. The pinned codec's TIFF decoder unassociates RGBA for
contiguous and compressed input, but preserves premultiplied samples for raw
separate uncompressed input. Its current `Mode::from_header` maps both routes
to straight RGBA, so Pillow cannot identify the latter transfer semantics yet.
The codec header selection must distinguish the decoded transfer mode before
Pillow can preserve that case as `ImageRgbPremultiplied8`; do not infer it from
`SourceDescriptor` inside the Pillow adapter.

Encoding remains a separate boundary TODO. `raster_to_decoded` now preserves
the codec's exact transfer modes for bilevel `1`, scalar `I`, and scalar `F`,
and serializes native u16 carriers in the codec's little-endian transfer
layout. Mode-specific transfer gaps—including PA per-pixel alpha,
premultiplied alpha, RGBX, YCbCr, LAB, HSV, format options, and explicit
support/rejection behavior—are tracked in the append-only
`image-slash-star#9h5w` task list; implementation is tracked in
`image-slash-star#nype`. The codec owns format-specific layouts and encoding
behavior; Pillow translates its
raster `Mode` at the boundary and verifies Pillow-visible results and faults.
Append newly discovered mode/format cases to the task list. Until its PA
transfer contract is implemented, the current generic Pillow adapter maps the
two-byte PA carrier to LA and omits palette metadata. The pixel alpha byte is
retained, but the index is treated as luma and the encoded output cannot retain
PA identity. Keep codec transfer layouts distinct from Pillow's mode identity.

Mode-tag source audit (2026-11-03): the maintained Rust source has four direct
`DynamicImage::with_mode` call sites: compatible-carrier assignment in
`Image::from_dynamic_with_mode`, generated-image identity assignment,
`frombytes` boundary tagging, and Resize's exact-source-mode restoration. Each
call uses the requested or source `Mode` explicitly; `with_mode` checks carrier
compatibility and does not infer identity from buffer type. No active caller
uses `with_mode` to substitute a different algorithm or convert samples. The
operation constructors found by the caller scan pass their declared target
mode into `from_dynamic_with_mode`. Source review only; output and rejection
parity remain in the frozen CPU gate.

PutPixel mode-source update (2026-11-03): the public scalar `Image::putpixel_mode`
and numeric `Image::putpixel_mode_scalar` methods no longer accept a redundant
`Mode` argument. They resolve identity from the Pillow-owned `Image` and keep
the resolved value in private helpers for exact-mode input expansion and CPU
strategy selection; the Python binding remains a separate host adapter and
continues to obtain mode from the image wrapper. Source-only; scalar and mode
error-order parity remains pending.

Direct GetPixel source continuation (2026-11-03): the family support
classifier now links to exact-mode `GET_PIXEL_SUPPORT` declarations in all 25
analysis mode `ops.rs` files. Each mode owner validates its selected Default
strategy and calls its native-carrier implementation in
`<mode>/default/get_pixel_impl.rs`; family orchestration dispatches from the
authoritative `DynamicImage` variant. Shared helpers read matching carrier
types directly, with LAB and scalar I/F behavior remaining distinct. No vector
strategy is declared for a one-pixel read; all architecture requests resolve
through the declared Default implementation. The fixed-RGBA8 Rust/JavaScript
accessor has per-mode CoreOnly result declarations and uses the shared
per-pixel projection without an intermediate image. Public coordinate checks
and the loaded RGB `None` fast-path behavior remain at the Image wrapper.
Output shape, error order, selected/fallback receipts, and CPU parity remain
pending; this source change is unvalidated.

GetPixel source follow-up (2026-11-03): rechecked the `Mode::ALL` list against
both family classifiers, all per-mode support declarations, every Default
owner, and the exhaustive `DynamicImage` execution match; the source mapping
covers 25/25 identities. The loaded RGB binding shortcut checks its exact
mode before coordinates and then uses the same RGB mode declaration and
Default owner. Removed a redundant second mode check from that shortcut.
This closes only the GetPixel source-placement review; values, coordinate and
error ordering, fallback receipts, and mode-by-architecture parity remain
pending.

Analysis helper module-path correction (2026-11-03): the WIP Clippy compile
output exposed four prepared Analysis helpers resolving support constants via
`analysis::helpers::ops`, a nonexistent sibling. Corrected those imports to the
parent `analysis::ops` module. This is source-only and has not been compiled;
the overall CPU source review and validation gates remain open.

Draw SIMD module-boundary correction (2026-11-03): the same WIP compiler output
showed Draw's SIMD kernel re-exports widening `pub(super)` items across
multiple parents, plus shape kernels importing clipping helpers from the Draw
family root where they are not exported. Kept admission and layout imports
local to the SIMD subtree, kept the span writer private to its kernel parent,
restricted re-exported kernels to the SIMD subtree, and resolved clipping and
I;16 text helpers through their owning helper modules. Algorithms are
unchanged; this source correction is uncompiled, and Draw source review remains
open.

Support and compile-diagnostic source corrections (2026-11-03): kept the
Blend/Composite vector contracts in the architecture-selection layer and made
their unused `InputContract` forms fail if misdeclared as public input
validation; updated
the exhaustive native pixel-write request guards for the new typed request
facts; and made ChannelAccess reject Draw-only wrapper guards it cannot
resolve. Also removed a Rotate row aliasing conflict by caching its pixel
count before mutable iteration, and corrected target-specific unused bindings
and an unreachable palette comparison arm exposed by the failed WIP Clippy
attempt. No algorithm changes were intended. These edits are uncompiled; the
cross-cutting source audit and CPU gates remain open.

Convert source audit update (2026-11-02): I;16-to-I;16 alias targets preserve
the native u16 samples when selecting among `I;16`, `I;16L`, `I;16B`, and
`I;16N`; byte order remains an image-boundary concern. The previous
cross-identity route clipped samples through 8-bit luma. CoreOnly source/target
alias parity, including values above 255 and mode-specific byte export, remains
TODO in the implementation. The `I`→`I;16N` target is also CoreOnly: Pillow
12.2 has no converter for that pair; the native extension preserves the clamped
u16 value like the other I;16 targets, with input-range parity still TODO.
CoreOnly scalar F32-to-I;16 now clamps numeric samples to the u16 range and
truncates directly. LA16-to-I;16 preserves its native luma sample; RGB16 and
RGBA16 compute luma from u16 channels; RGB32F and RGBA32F compute normalized
float luma and map it directly to u16. These routes do not create an 8-bit
intermediate. Each is marked `NO_PILLOW_SUPPORT`; focused extension parity
remains a source-owner TODO.

Palette-target Convert preparation correction (2026-11-03): the generic
Convert registry entry rejected cross-mode P/PA targets during preparation,
before Image's lazy and eager wrapper routes could consume the prepared plan's
palette-bearing output. Moved the raster-only rejection into
`PreparedConvert::execute`, before pixel work. Image wrapper paths use
`execute_convert_output` and retain palette/alpha sidecars; direct raster-only
execution still returns the existing `NotImplementedError`. This fixes the
source-level route mismatch only. Palette output, metadata, and exact-mode
parity remain pending. Push-time `docs-lint` passed; `cargo fmt --check` reports
workspace-wide formatting drift and Clippy cannot compile the unfinished CPU
tree. No behavior tests, parity, or benchmark ran.

Compiler-diagnostic source corrections (2026-11-03): widened implementation
visibility to the owning family/mode boundary for Geometry, Draw, Eval, and
Filter re-exports;
classified Paste as ineligible for unrelated pixel-span guards; aligned the
PutAlphaData image/mask borrow lifetime; documented and locally permitted the
NEON token construction under statically NEON-enabled targets; and removed
unused imports and locals identified in the prior Clippy output. This is
source-only cleanup from the previous failed compile; no follow-up build or
quality check has run, and CPU source closure remains open.

Histogram source review (2026-11-03): Pillow 12.2's masked multiband `Histo.c` reads physical lane 1 into the second result bank, while the unmasked two-band branch reads lane 3. LA, La, and PA raw inputs use `unpackLA`, which initializes omitted lanes by repeating the first sample; ordinary two-component inks do the same. The maintained typed Histogram owner reads retained lane 1 when available and falls back to the canonical repeated first sample when no independent-lane provenance exists. Source review confirms retained lanes are attached by new/frombytes/decode construction, packed PutData, and same-pixel-size mode-mismatched Paste. This repository has no Arrow image-import API, so `ImagingNewArrow` is not a reachable producer here. The remaining cross-operation risk is any producer that loses sidecar provenance before Histogram; track that with the hidden-lane producer audit and parity. Histogram parity remains pending. Sources: [Histo.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Histo.c#L721-L830), [Unpack.c mode dispatch](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Unpack.c#L1493-L1511), [`unpackLA`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Unpack.c#L2924-L2957), [`_imaging.c::getink`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L534-L645), [Paste.c raw pixelsize path](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Paste.c#L1982-L2017).

PutData La/LAB source completion (2026-11-03): Pillow's `_imaging.c::getink`
normalizes multiband tuple and packed values from the core band's count, and
`Storage.c` records La and LAB as two- and three-band four-byte physical
pixels. The typed PutData normalization already retains those physical lanes
beside the compact 2-/3-byte Rust carriers; this source pass connects their
callback-preserving `PutDataValueAt` contract to the existing native mode
writers instead of rejecting the exact La and LAB identities. Bulk Default and
architecture owners already use the matching two-/three-channel strategies.
TODO(pillow-rs#3ptb): verify packed and component callbacks plus bulk partial,
complete, and empty writes with raw-lane follow-on parity after CPU source
closure. This source change is unvalidated. Sources: [`_imaging.c::getink`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L534-L645)
and [`Storage.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Storage.c#L1848-L1910).

Serial CPU isolation audit (2026-11-03): the maintained `pool_cpu` and shared
`ops/pil_resize` source contain no Rayon calls, parallel iterators, or
`cfg(feature = "parallel")` execution branches. The preserved legacy archive
still contains the old scheduling implementations but remains outside the
active module graph. Keep that scheduling work for `pool_parallel` after CPU
source, quality, parity, and performance gates. Read-only source scan; no
validation ran.

CPU leaf context/mode source scan (2026-11-03): maintained
`pool_cpu/ops` and `batch_execution.rs` have no `ExecutionContext`
construction, global backend reads, runtime architecture detection, or
Pillow-string-to-Mode parsing outside tests. Remaining `pillow_name()` uses
format error text or identify Pillow versus CoreOnly support; they do not
select an implementation. The active CPU tree also has no Rayon or
parallel-feature execution references outside its inert archive. This bounded
textual audit does not prove end-to-end context injection or runtime selection;
direct host terminals, wrapper routes, declarations, and receipts remain open
in the workstream matrix.

ImageOps physical-lane review (2026-11-03): Pillow 12.2 `Point.c` writes only mode-defined bytes for two- and three-band pixels, and its ordinary `ImagingNew` output allocation is zero initialized. The maintained Invert, Posterize, Solarize, Autocontrast, Equalize, and Colorize owners now attach zero values for omitted physical lanes after their selected Default/SIMD kernel, including the Equalize identity-LUT result. Expand now carries retained source lanes into its copied region and fills the expanded region with the exact mode-specific `Image.new` lane values; the Default and selected SIMD paths use the same prepared composition boundary. These sidecar updates leave pixel kernels unchanged. Raw-byte follow-on Paste parity remains pending. Sources: [Pillow 12.2 Point.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Point.c), [Pillow 12.2 Storage.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Storage.c).

Chops Invert source review (2026-11-03): Pillow `Negative.c::ImagingNegative` inverts every physical byte. The typed Default strategy already inverts every logical sample; it now also inverts retained omitted bytes for compact LA/La/PA and RGB-family carriers. The selected SIMD strategy applies the same sidecar transform after its native-byte kernel. Chops Invert now records the inverted RGB-family pad byte when its source uses the canonical Paste fallback. Binary Chops applies each operation's byte arithmetic to retained physical lanes for Default, selected SIMD, fused Multiply+Screen, and owned in-place batch paths. When an input lacks retained lane state, Chops derives that operand from the same canonical per-mode fallback used by Paste, then applies the selected byte operation. Raw-byte follow-on parity remains pending; no validation has run. Sources: [`Negative.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Negative.c) and [`Chops.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Chops.c).

Contrast enhancer base source review (2026-11-03): the direct constant-base path in `contrast_degenerate()` previously cleared omitted physical lanes when it mutated a clone's compact carrier. It now restores the omitted lanes from the selected `ContrastBase` values, matching the converted constant base for LA, RGB, YCbCr, and HSV while retaining the source uppercase A byte where applicable. This keeps later physical-byte operations consistent with the base image exposed by `ImageEnhance.Contrast`; parity through follow-on Paste remains TODO(pillow-rs#3ptb). No runtime validation has run. Sources: [Pillow 12.2 `Convert.c` L/LA/RGB/HSV conversions](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L2420-L2479) and [L/YCbCr conversion](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L3439-L3455).

Mutable byte-view source audit (2026-11-03): the maintained Rust tree has four callers of `DynamicImage::as_bytes_mut()`: the Contrast degenerate base, owned in-place Invert, PutDataValueAt, and UnsharpMask. Each compact-carrier path restores or composes omitted lanes at its owner: Contrast derives the constant base lanes, Invert restores point-output zeros or inverts retained Chops lanes, PutDataValueAt handles compact carriers through its lane-aware writer, and UnsharpMask finalizes filtered/blended lanes after mutating visible bytes. This closes the callsite inventory only; output and downstream Paste parity remain pending. No build, format, lint, test, parity, or benchmark ran.

Owned in-place inversion source follow-up (2026-11-03): the prepared batch route now retains the operation distinction after the native visible-byte kernel. `Invert` restores the zero omitted lanes produced by ImageOps point allocation; `InvertChops` transforms retained physical bytes with the same inversion. The sidecar mapping is in place and does not allocate a second lane buffer. Raw-byte follow-on parity remains TODO(pillow-rs#3ptb); no validation has run.

Paste raw-storage audit (2026-11-03): Pillow's Python wrapper normally converts a mismatched source, except it skips conversion when the destination is `RGB` and the source is `LA`, `RGBA`, or `RGBa`; its source code also trusts a subclass's virtual `convert()` result without checking the returned mode. The RGB fast path has a documented performance rationale: Pillow's changelog says premultiplied `RGBa` paste avoids multiplication/shift work and is typically twice as fast as ordinary RGBA paste, and records removing an extra conversion for `RGBA`/`RGBa` pasted onto `RGB`. The C `ImagingPaste` primitive checks dimensions and physical `pixelsize`, then copies or blends every physical lane without checking mode identities. Pillow 12.3.0 source and the user's local reproduction establish this as observable implementation behavior, not a documented guarantee: a subclass override may return a different mode and the native paste path proceeds when physical pixel sizes match. Base-image conversion remains before backend strategy selection except for that RGB shortcut; a resolved override result is passed through as returned. The 1-byte and 4-byte source strategies now map physical layouts and retain omitted lanes on compact Rust carriers for Paste/Composite; PutPixel, synchronous and bulk PutData, and LA-family PutAlpha/PutAlphaData preserve their affected lanes; mode-owned Draw Bitmap/Text writers and Default/SIMD geometry spans preserve their affected lanes; crop and transpose-family outputs now remap retained lanes with their pixels. Binary and fused Chops compute omitted bytes from canonical per-mode fallbacks when an input lacks a sidecar, across Default, SIMD, and in-place batch routes. Blend applies Pillow's raw-byte float32 interpolation to retained lanes and resolves a missing endpoint through the canonical per-mode fallback. Brightness applies its zero-image Blend formula to retained lanes in the prepared Default/SIMD wrapper, resolving absent RGB-family input state from the canonical padding fallback. Prepared Color Saturation and direct Contrast LUT now preserve compact hidden lanes. Sharpness maps omitted physical lanes through Pillow's copied-border/zero-interior filter behavior and final float32 Blend for LA, La, RGB, YCbCr, LAB, and HSV. When input sidecars are absent, it derives canonical source lanes, including per-pixel LA-family luma, and stores output sidecars only when they differ from canonical output fallbacks. EffectSpread now carries omitted lanes through the exact Default relocation map and through the selected vector route's identical random destination decisions without advancing the process RNG a second time. Resize and ResizeBoxed now preserve omitted physical lanes for identity and exact-crop copies and normalize resampled lanes by mode: direct 2-/3-band paths clear unused bytes, while filtered LA reconstructs lanes 1 and 2 from final luma. Reduce preserves lanes on identity copies and deterministically chooses zero omitted bytes after actual reduction. Pillow allocates Reduce output with `ImagingNewDirty` and leaves those omitted bytes undefined; do not claim exact follow-on raw-byte parity for that storage state. Prepared Thumbnail and sparse RGB PutPixel/Thumbnail fusion use the same Rust policy. ImageOps point strategies record zeroed omitted lanes and Expand composes retained source and exact fill lanes at its prepared mode/architecture boundary; raw-byte follow-on parity remains pending. Transform now composes omitted physical lanes at its prepared mode/architecture boundary, with runtime parity pending; prepared Pad composes Resize or canonical source lanes through each output path, including fill-border bytes; Rotate remaps lanes for identity/discrete requests and composes affine nearest/filter lanes at the prepared architecture boundary; absent source sidecars use canonical input lanes so RGB fill padding is retained, while LA-family lanes follow output luma. Runtime parity remains pending. Direct fused Resize-to-Pad owners are L/RGBX and have no omitted sidecar; compact modes use prepared Resize followed by Pad lane composition. Filter3x3/Filter5x5 derive canonical input lanes when sidecars are absent, then apply Pillow's observed border-copy/zero-interior behavior; BoxBlur filters all physical bytes, UnsharpMask applies the same blend to filtered lanes, and native rank/ModeFilter extensions process omitted bytes independently. Pillow's source shows these writes but does not document why Filter.c zeros omitted lanes. Filter runtime parity remains pending. Same-size Pad borders now compose source lanes from retained sidecars or canonical mode fallbacks, then retain the exact configured fill lanes. The typed I;16 path now accepts same-pixel-size source aliases and translates stored byte order at the Paste boundary; exhaustive byte-order, mask, and clipping parity remains TODO(pillow-rs#3ptb). In the RGBa-to-RGB shortcut, source samples stay premultiplied and are copied/blended as stored; `Convert.c::rgba2rgb_` only applies when `convert("RGB")` is actually called. Changes are source-only and unvalidated. Sources: [`Image.paste`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1756-L1764), [`Storage.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Storage.c#L1848-L1910), [`Paste.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Paste.c#L270-L329), [`Convert.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L2990-L3020), and [Pillow 1.0b2 performance change](https://github.com/python-pillow/Pillow/blob/12.2.0/CHANGES.rst#L2845-L2849).

Pillow paste override compatibility rule (2026-11-03): the user reproduced Pillow 12.3.0 with a subclass `convert()` that returns the original image. Same-pixel-size results RGBA→CMYK, RGB→RGBA, and L→P succeed with raw internal bytes; RGBA→L raises `ValueError: images do not match`. Treat this as undocumented, version-pinned compatibility behavior, not a promise that differently named modes are generally interchangeable. The Python wrapper calls conversion as Pillow does, retains the returned mode without a second mode check, and lets Paste reject mismatched physical pixel sizes. Unmasked Paste copies physical bytes; masked Paste remains a separate per-lane blend contract. User instruction: do not contact or open issues in external repositories. The parity input protocol and builder now express an instance `convert()` override returning the same image, with cases for RGBA→CMYK, RGB→RGBA, L→P, RGBA→L, and masked LA→RGBA lane blending. The shared parity oracle remains pinned to Pillow 12.2.0; the user independently reproduced the behavior on Pillow 12.3.0. No runtime verification has run. This does not change the flat `Mode`/`DynamicImage` architecture.

I;16 Paste alias source audit (2026-11-03): traced all four exact source and destination aliases through their mode-owned declarations and the shared typed-u16 plan/kernel. Unmasked writes translate source stored byte order into the preserved destination mode; the architecture route now performs cross-endian swaps in eight-lane u16 blocks and records those vector blocks. Masked writes select the byte at the clipped source/destination offset; binary masks copy full samples, ordinary masks blend one storage byte, and premultiplied RGBa masks retain the PREBLEND zero-mask source contribution. Mask layouts and extents are checked before pixel execution. The source comparison found no discrepancy; all byte-order pairs, mask modes, clipping cases, same-pixel-size override behavior, and route receipts remain parity TODOs. No validation has run.

Raw-lane producer audit (2026-11-03): Pillow physical lanes are producer state, not derivable from `Mode` alone. The LA unpacker writes `[L, L, L, A]`; RGB unpack paths write a fourth padding lane of 255. `Image.new` with no explicit color starts with zero fill. A three-component RGB tuple gets a 255 fourth lane; four-component tuples and packed integers retain the value produced by `getink`. Two-component LA tuples repeat L into the omitted lanes, while packed integers retain their raw component lanes. Raw same-pixel-size Paste can then replace every physical lane. The Pillow-owned `ImageBuffer` now retains those omitted bytes in an optional compact payload; encoded decode and `frombytes` attach decoder lanes, `Image.new` distinguishes omitted color from an explicit fill, and raster crop/flip/rotate/transpose remap them. Paste and Composite carry those lanes through raw copy and masked blend. Masked LA Histogram reads retained physical lane 1 and uses the historical reconstruction only when no lane payload exists. Mode now owns the Pillow physical pixel size, byte-carrier pixel width, and omitted-lane offsets, while ImageBuffer retains producer-provided lane values. Mode-owned PutPixel Default, ordered batch, and all declared SIMD single-write paths update the hidden payload for compact LA/La/PA/RGB/YCbCr/LAB/HSV carriers. Callback-synchronous `PutDataValueAt` and bulk `PutData` preserve exact omitted lanes for LA/La/PA packed and component values and RGB/YCbCr/LAB/HSV packed, three-component, and four-component values; bulk restoration runs after the selected CPU mode/architecture kernel. Constant and image-backed PutAlpha/PutAlphaData preserve untouched hidden lanes for existing LA-family images across Default and SIMD paths; L-to-LA output uses Pillow's repeated-luma fallback. Mode-owned Draw Bitmap/Text writers and Default/SIMD geometry span writers retain omitted lanes for compact LA/La/PA/RGB/YCbCr/LAB/HSV carriers; crop and transpose-family outputs remap retained lanes with their pixels. Chops Invert and binary/fused Chops transform retained bytes when sidecars are present. Invert records the changed RGB-family fallback byte; binary/fused Chops resolve absent input sidecars through the canonical per-mode fallback shared with Paste. EffectSpread now reuses the selected relocation decisions for omitted lanes in Default and SIMD without consuming another process RNG sequence. Resize/ResizeBoxed now compose omitted lanes at the prepared boundary using identity/crop remaps or mode-specific resample output. Reduce preserves lanes on identity copies and deterministically chooses zero omitted bytes after actual reduction. Pillow allocates Reduce output with `ImagingNewDirty` and leaves those omitted bytes undefined; do not claim exact follow-on raw-byte parity for that storage state. Prepared Thumbnail and sparse RGB PutPixel/Thumbnail fusion use the same Rust policy. Affine Rotate now remaps identity/discrete outputs and composes affine nearest/filter fill lanes at its prepared architecture boundary; ImageOps point results initialize omitted lanes to zero and Expand composes source/fill lanes at its prepared mode/architecture boundary. Transform also composes omitted physical lanes at its prepared mode/architecture boundary, with runtime parity pending; prepared Pad composes Resize or canonical source lanes through each output path, including fill-border bytes; direct fused Resize-to-Pad owners are L/RGBX and have no omitted sidecar, while compact modes use prepared Resize followed by Pad lane composition. Filter follows the source-reviewed physical-lane behavior documented above. Raw-byte follow-on parity remains pending for these source-reviewed paths. Compact-carrier GPU PutData remains declined until its result strategy preserves these lanes. The generic ImageBuffer mutable APIs continue to invalidate hidden lanes because they have no Mode context. The flat `DynamicImage` tag remains the only Mode identity; this is payload storage, not a second selector. See Section 2.2 of [Implementation structure](IMPLEMENTATION_STRUCTURE.md). Sources: [`unpackLA`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Unpack.c#L2924-L2937), [`unpackRGBL`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Unpack.c#L3306-L3317), [`_imaging.c::getink` and fill](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L534-L645). This is an unverified source implementation; no validation has run.

Histogram logical-pixel bound correction (2026-11-03): the Default typed scans now iterate only `ImageBuffer::pixels()` within width × height, matching Pillow Histo.c and the archived byte-mode row traversal when a Rust carrier contains trailing storage. CoreOnly u16/f32 and scalar I/F scans remain native and allocation-free. Source-only; no validation ran. Source: [Histo.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Histo.c#L721-L830).
GetExtremaBytes logical-pixel bound correction (2026-11-03): all 25 mode-owned reducers now receive only the native samples in width × height, matching image extent and ignoring trailing carrier storage without staging or conversion. Source-only; no validation ran.

Chops lane preservation (2026-11-03): typed Default and architecture Offset
kernels now remap existing omitted physical lanes with the same wrapped pixel
coordinates used for logical samples. Default and selected SIMD Invert also
transform omitted bytes, matching Pillow's all-physical-byte Negative kernel.
Binary Chops lane arithmetic and ordered batches remain source TODOs; no build,
parity, or benchmark has run.

Color3DLut lane preservation (2026-11-03): Pillow `ColorLUT.c` copies input
physical byte 3 unchanged when the table has three output channels. CPU Default
and selected architecture strategies now retain that byte when the target mode
is RGB, taking it from the source sidecar for compact RGB/YCbCr/LAB/HSV and from
the visible fourth channel for RGBA/CMYK. Source comparison is complete; the
full source/target-mode × Architecture parity matrix remains TODO. No build,
parity, or benchmark has run. Source:
[`ColorLUT.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/ColorLUT.c#L738-L850).

Eval lane output source review (2026-11-03): Pillow 12.2 `Point.c` writes only
the declared one-, two-, three-, or four-band byte lanes into a newly allocated
clean image. For compact LA/La/PA and RGB/YCbCr/LAB/HSV storage, omitted lanes in
the output therefore remain zero; they are not transformed from input padding.
`PreparedEval` now attaches that zero lane payload after Default or architecture
execution. Mode/architecture lane parity remains pending; no checks have run.
Sources: [`Point.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Point.c#L26-L88), [`Storage.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Storage.c#L42-L53), and [`memory_get_block`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Storage.c#L332-L365).

Color Saturation hidden-lane follow-up (2026-11-03): Pillow 12.2 `ImageEnhance.Color` uses `LA` as the intermediate for uppercase-alpha modes; for an existing LA source the degenerate image is the source itself, so retained omitted lanes pass through unchanged. RGB uses an L-to-RGB degenerate whose omitted padding byte is 255, and Blend applies the float32 interpolation to that physical byte. The prepared mode-aware Color Saturation wrapper now retains LA sidecars and maps RGB sidecars with the same `mul_add` expression for Default and selected vector execution; factor zero produces a constant 255 lane. Absent canonical lane state remains implicit because LA keeps its input as the degenerate image and RGB's input and grayscale-base padding lanes are both 255. Direct Contrast LUT lanes now use the prepared ContrastBase. Sharpness now models the Pillow filter's hidden-lane boundary/interior output before applying its final blend. The user supplied the rationale for the Paste override behavior; it is an undocumented implementation detail with local compatibility inputs pending. Source-only; no build, formatter, checkstyle, lint, test, parity, or benchmark ran. Sources: [ImageEnhance.py](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageEnhance.py), [Blend.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Blend.c), [Unpack.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Unpack.c).

Pillow cross-version Paste source review (2026-11-03): source review found the Python wrapper trusts custom `convert()` results while native Paste checks physical pixel size instead of mode identity. The user's local Pillow 12.3.0 reproduction confirms the listed same-size successes and width-mismatch error. This is an implementation detail; no external maintainer question or issue will be raised. The generated cases use the repository's shared Pillow 12.2.0 oracle; the user's Pillow 12.3.0 reproduction remains separately documented evidence. Verify masked behavior as its own case.

Contrast hidden-lane follow-up (2026-11-03): each direct exact-mode Contrast strategy now retains its already-computed `ContrastBase` and factor through prepared execution, avoiding a second source scan. The Contrast helper applies the same float32 `mul_add` byte formula to omitted lanes after Default or selected SIMD execution. LA maps its two hidden luma copies to the measured mean; RGB, YCbCr, and HSV map their padding lane to the mode-specific base value of 255. Factor zero yields known base lanes and factor one preserves the source. With absent sidecars, each supported mode's canonical lane is a fixed point of its mapping: LA lanes follow output luma, while RGB, YCbCr, and HSV padding stays 255. LAB continues through its composed Blend child, which already carries sidecars. Sharpness mode owners now preserve compact sidecars using Pillow's copied filter borders, zeroed interiors, and final Blend. Source-only; no build, formatter, checkstyle, lint, test, parity, or benchmark ran. Source: [ImageEnhance.py](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageEnhance.py).

UnsharpMask lane handoff correction (2026-11-03): the logical-byte blend uses
`DynamicImage::as_bytes_mut`, whose mode-blind mutable escape intentionally
invalidates omitted physical lanes. The prepared UnsharpMask owner now saves
the already-filtered GaussianBlur lane payload, restores it after blending the
logical carrier, then applies the same UnsharpMask formula to those lanes.
Source review only; runtime mode/architecture parity remains pending.

Mutable native-byte access audit (2026-11-03): the maintained Rust source has
four `DynamicImage::as_bytes_mut` call sites. PutData routes compact carriers
through lane-aware writers; batch Invert snapshots and restores or transforms
the sidecar; ContrastDegenerate rebuilds the exact output lane payload; and
UnsharpMask now preserves the GaussianBlur lane output across its blend. The
accessor docs now state that acquiring its generic mutable view invalidates
omitted lanes and that preserving callers must snapshot and restore or
recompute them. This does not close the broader audit of direct `ImageBuffer`
mutable APIs or downstream raw-byte parity.

Mode-aware generic pixel write correction (2026-11-03):
`DynamicImage::put_pixel` now writes the logical samples in compact LA/La/PA
and RGB/YCbCr/LAB/HSV carriers without invalidating their retained physical
lane payload. The lower-level mode-blind `ImageBuffer` mutators invalidate
sidecars. Source review confirms the generic mutable escape paths and the
`DynamicImage`-owned exception. Downstream raw-byte outcomes remain pending
CPU parity. `DynamicImage::copy_within` now remaps retained lanes from the
original source coordinates alongside the logical overlap-safe copy.
`GenericImage::copy_from` is a logical-pixel operation:
its source trait exposes no mode or physical-lane provenance, so omitted
destination lanes follow `put_pixel` and are not copied from the source.
Mode-aware physical-lane transfer remains owned by operations such as Paste.
This resolves the earlier `copy_from` contract TODO. The generic mutable
escape source audit is complete; downstream raw-byte outcomes remain pending
CPU parity. No validation has run.

EffectSpread physical-lane follow-up (2026-11-03): the Default strategy remaps hidden lanes through its existing Pillow relocation map. Selected SSE2, NEON, and Wasm SIMD128 strategies preserve per-pixel hidden lanes using a cloned starting RNG state and the same destination decisions as the active-byte kernel; the process RNG still advances only once. Constant sidecars remain compact, and absent producer state stays absent. No build, formatter, checkstyle, lint, test, parity, or benchmark ran.

Paste wrapper validation follow-up (2026-11-03): the final staged call now
checks the resolved source, mask, and region against Paste's existing typed
input declaration before queuing the lazy operation. Incompatible override
results therefore fail at the paste call with the declared mismatch fault;
prepared execution repeats the same declaration after materialization. This
is source-only and remains unverified by parity.

ImageOps LUT wrapper source correction (2026-11-07): the shared Pillow 12.2
`_lut` wrapper rejects mode P with `NotImplementedError("mode P support coming
soon")` before calling `Image.point`. `ImageOpsPosterize` and
`ImageOpsSolarize` now declare that wrapper rejection and expose only their
L/RGB `Eval` child route. The separate Rust `Posterize` and `Solarize`
operations retain their `NO_PILLOW_SUPPORT` native palette-index strategies,
so the Rust extension does not change Python's ImageOps behavior. Pixel and
fault-order parity remain pending. Reference: [Pillow 12.2 ImageOps.py
`_lut`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L601-L620).

### Ordered completion gates

| Order | Gate | Status | Remaining work | Unblocks |
| ---: | --- | --- | --- | --- |
| 1 | CPU family implementation and source review | Incomplete; exact-mode and per-leaf algorithm review remains open | Mode owners and injected Default/Architecture routes are mapped, and the current manual caller-route audit found no unreviewed caller. Continue owner-by-owner archive and mode-leaf review, including guards and the exact kernel each prepared strategy reaches. Preserve unresolved output and fault cases for the frozen CPU parity matrix. | Gate 2 |
| 2 | Cross-cutting source closure | Incomplete; source TODOs and runtime evidence remain | Mode ownership, palette transitions, and injected Backend/Architecture context are source-mapped across direct, lazy, eager, fused, nested, fallback, and terminal routes. Close remaining owner-specific source questions in the inventory, then verify support-derived output identity, callback/fault order, and actual selected/fallback receipts with the frozen matrix. | Gate 3 |
| 3 | Archive, dead-code, and size audit | Source maps exist; compiler reachability, dead-code, and exact size proof remain pending | The baseline declarations and Rust files are cross-referenced to maintained owners or retirement, and static scans find no active legacy imports or Rayon scheduling in maintained CPU. Cross-family architecture-module target reachability remains in source closure. Compiler-backed module reachability, maintained dead-code review, Clippy-counted function length, and the final 1,000-line audit belong after Stage 5 closes. Earlier `make quality` results do not cover this WIP. | Gate 4 |
| 4 | Static quality and compilation | Historical pass on 2026-11-13; stale after later source changes | The earlier structure audit, `make quality`, `make build-parity`, and `make build-parity-cpu-architecture` passed at that checkpoint. Rerun the prescribed structural, size, static, formatting, lint, and compilation gates after current Stage 5 source review closes. Native x86 kernel execution remains unavailable on this Apple Silicon host. | Gate 5 |
| 5 | CPU parity | Full frozen CPU matrix pending; current results are historical or focused | The 177,881-case CPU/Default campaign recorded 173,118 passed, 4,526 failed, and 237 not_run. Failed-only retries and later focused corrections resolved many cases; the latest saved 42-ID retry passed 41/42, with the remaining I-to-PNG error-message difference owned by image-slash-star. Those receipts predate the current source WIP and do not replace the frozen matrix. After Gate 4, rerun the frozen CPU denominator and affected architecture/fallback guards without changing assertions. | Gate 6 |
| 6 | CPU performance | Not started; blocked by Gate 5. Workload IDs are reconciled; baseline timings are not. | The `3f9dfd403` input set has 1,032 workload IDs (SHA-256 `639e358082276c924343618d5a3b09c6945de012c1a80ac5a8ee2121ccb84`). The current set has 1,033 IDs (SHA-256 `e134adf04711dfd5bfdd638ef4ca3c580b725b6f0832aff591dc15e07c5000b1`): 1,031 shared definitions are unchanged; Pillow 13 removed `pil-image-image.get-child-images.standard`, and `pil-imagetext-text.wrap.standard` plus `pil-imagetext.text.standard` were added. Collect compatible baseline timings after Gate 5; then report each operation, mode, and architecture separately, with at most one confirmation repeat for questionable pairs. | CPU complete; Gate 7 |
| 7 | Parallel CPU | Waiting for CPU completion | Move parallel scheduling into `pool_parallel`, then review and verify it as a separate backend and evidence profile. | Later backend work |

2026-11-25 CPU execution-context handoff source crosscheck: the unpinned
`CpuPool` entry resolves policy once at the backend boundary; context-aware
CPU entries forward the supplied context unchanged. Prepared CPU operations
receive `PreparedExecution.context`; serial batch steps forward it through
owned-special, ordinary prepared, and fused-route entry points. An injected CPU
context is retained beside a selected GPU context for CPU-owned wrapper work.
The palette-producing Convert route reuses that retained context. This closes
only the reviewed CPU handoff slice; public output, selection/fallback
receipts, and latency remain pending, and GPU fallback propagation remains
assigned to the later GPU stage.

Gate 3 source crosswalk update (2026-11-09): Section 8 now maps all 313 CPU and
569 SIMD declarations to maintained owners or intentional retirement. The
maintained direct-caller and Cargo feature source audit is recorded in the
inventory. Source TODOs were reviewed: active CPU items are parity/receipt
evidence, future `pool_parallel` scheduling, or architecture opportunities to
assess after measurement. Gate 3 remains open for compiler-backed module/target
reachability, dead-code, and function-size evidence.

Rust-only raw-mode input slice (2026-11-09): `PIL.Image.frombytes` has twenty
recorded target-result cases for `ImageLumaA16`, `ImageRgb16`, `ImageRgba16`,
`ImageRgb32F`, and `ImageRgba32F`: one successful one-pixel sample, one short-
buffer `ValueError`, and accepted zero-width and zero-height rasters for each
mode. They enter through the ordinary thin Python facade and are explicitly
`NO_PILLOW_SUPPORT`; expected outputs and errors stay separate from Pillow
comparisons and injected faults. On clean commit `2773be01b`, the normal CPU
profile passed all twenty cases (run
`migration-parity-929d200d41e24d748fef68c0235026dd`); the same cases passed
under Default, Neon, SSE2, SSE3, SSSE3, AVX, AVX2, AVX+FMA, and AVX2+FMA
(180/180, zero infrastructure errors). These are construction contracts and
do not exercise architecture-specific pixel strategies. Native x86 execution
remains unavailable on this ARM host.

The historical CPU selection was 17,616 cases (17,596 Pillow-parity cases
plus twenty recorded target results), case-ID SHA-256
`e4e8ade188ad7965f44a70b36f438b61a6a2d54188771c15e43373710b85b0f6`. That
selection passed except for the external I32-to-PNG encoder gap described
above. The regenerated 25,002-case selection later ran against Pillow 12.2; it is historical and incompatible with the active Pillow 13.0 oracle. Focused Rust coverage on
clean commit `2773be01b`
executed one plan and passed all twenty cases (run
`migration-coverage-eef1a17440da4c2c9a3bb2b1889f280f`). `internal_samples.rs`
measured 49/56 lines, 7/8 branches, 8/10 functions, and 96/112 regions. The one uncovered branch is
the private decoder's unsupported-mode return; the current public `frombytes`
routes send only these five modes to that decoder. The defensive
`buffer_error` is also unobserved because exact byte-count and dimension checks
make its `ImageBuffer::from_raw` failure impossible for valid inputs. TODO
(`pillow-rs#3ptb`): recheck these paths if the decoder gains another caller; do
not add a test-only Rust entry or claim them covered. The coverage receipt was
not ingested into the aggregate collector. The generated target-output page
records the clean normal-CPU result separately from Pillow parity totals. The
coverage receipt is
`build/migration-parity/core-only-mode-frombytes-20261109-clean-2773be01b/coverage-result-rust.json`.

Color3DLUT input source follow-up (2026-11-09): the Python adapter now accepts
the low-level sequence protocol Pillow accepts, checks the exact table length
before numeric conversion, snapshots non-list/tuple sequences, and preserves
the live list/tuple conversion order. The Rust LUT algorithm is unchanged.
Custom sequence callback/fault parity and runtime performance are still pending;
this source review does not close the Color3DLUT family or Gate 3.

“In progress” means source work exists but the gate has not met its exit
condition. “Partial” means focused evidence exists while the full gate remains
open. “Not started” and “blocked” mean no fresh evidence is claimed.
These are work-status rows, not a manually maintained operation-support
matrix; each operation's support view must come from its own declarations.

### CPU parity evidence versus acceptance coverage

| Coverage area | Existing evidence | Required for CPU acceptance |
| --- | --- | --- |
| Input corpus | The generated index lists 5,110 parity documents, 177,882 indexed cases, and 105,095 benchmark workloads (manifest SHA-256 `aea5f40d617175d88bc18ab2515bac729e44b6c097024fc9b41724bb18ae6445`). Recent PostgreSQL parity uses frozen input snapshot `62db779f4eb483741937f4a6888bb113f97946dda69d816f7dd889a570043397`; the Default and Neon full-corpus evidence selects 179,882 cases. The index count and frozen-campaign count differ and have not been reconciled into one current support-derived denominator. | Freeze one current corpus and case-ID denominator from the indexed inputs and the operation support declarations; record its manifest, snapshot, and selector plans. |
| Recorded differences | The CPU/Default closure reports 179,882 unique verified passes: 178,912 unchanged baseline passes, 17 formerly failing cases replayed successfully, and 953 affected guards passed. CPU/Neon also passed the 179,882-case selection. Thirteen declared selectors passed a separate 970-case architecture closure (12,610 passes). The latest Filter closure adds 1,091 cases × 14 platform/selector lanes (15,274 passes). These closures report zero remaining mismatches in their selected sets; none establishes a complete current operation × Mode × Architecture denominator. | Reconcile the frozen failure and guard IDs with the current manifest, then execute every case required by the support-derived denominator. Preserve comparisons and public fault contracts. Keep image-slash-star encoding work outside pillow-rs. |
| Mode identity stimulus | Current inputs include all 20 Pillow spellings and five internal high-depth Mode identities. All 231 recorded target outputs passed on the default CPU profile. This does not prove every operation/mode route or any non-default architecture. | Finish the support-derived denominator for every operation and mode, then collect output and exact identity evidence on every CPU Architecture selector. |
| Mode strategies | The current corpus includes selected accepted and rejected mode routes, but does not contain one case for every `OperationId` × `Mode` outcome. The full operation/mode support cross-product remains unverified. | For every operation, exercise each exact mode’s declared output or rejection, including metadata, fault order, native CoreOnly outputs, and relevant target-mode transitions. |
| Architecture strategies | The full 179,882-case corpus is verified on Default and Neon. Focused closures cover 970 cases × 13 selectors, 1,091 Filter cases × 14 platform/selector lanes, and selected F32 Resize/Thumbnail paths. x86 parity ran under QEMU on the ARM host; `wasm_simd128` and AVX-512F lanes explicitly exercised fallback only. | Run the frozen complete CPU selection for every required architecture selector, with actual selected/fallback receipts; run x86 and AVX-512F specializations on matching hardware before claiming native execution. |
| Edges and composition | The existing corpus contains selected edge, callback, and multi-image cases; it does not prove all operation-specific boundaries or mode relations. | Include relevant empty/small dimensions, complete/partial vector blocks, tails, multi-image mode pairs, lazy/eager paths, and public error ordering for every declared route. |

There is no defensible operation × Mode × Architecture coverage percentage yet.
The support declarations enumerate the required combinations, but the parity
corpus is not generated from that denominator. The 2026-11-10 matrix run proves
outcomes only for its frozen 25,002 case IDs; it does not close missing support
rows or represent the current 5,110-document input index. Keep its report and
receipts as historical full-run evidence, not as proof of complete strategy
coverage. Historical focused counts must not be added to the current
denominator.
The broad 2026-11-07 WIP serial-CPU report
([`cpu-wip-parity.json`](../build/migration-parity/cpu-wip-parity.json))
selected 689 cases and failed 10, all in ImageOps Equalize. Eight had a
workflow-status mismatch: Pillow
reported `not_run` while the target completed
(`PIL.ImageOps.equalize.mode.la`, `PIL.ImageOps.equalize.mode.rgba`,
`PIL.ImageOps.equalize.nuanced.alpha-la`,
`PIL.ImageOps.equalize.nuanced.alpha-rgba`,
`PIL.ImageOps.equalize.nuanced.simd-alpha-la`,
`PIL.ImageOps.equalize.nuanced.simd-alpha-rgba`,
`PIL.ImageOps.equalize.nuanced.unsupported-mode-cmyk`, and
`PIL.ImageOps.equalize.nuanced.unsupported-mode-one`). Two had an error-class
mismatch: Pillow raised `OSError` while the target raised `ValueError`
(`PIL.ImageOps.equalize.nuanced.unsupported-mode-f` and
`PIL.ImageOps.equalize.nuanced.unsupported-mode-i`). The run had zero
infrastructure errors and no pixel-value differences. The saved
[`cpu-wip-equalize-failures-20261107.json`](../build/migration-parity/cpu-wip-equalize-failures-20261107.json)
follow-up passed these ten cases 10/10. A later WIP rerun selected the same
689-case cohort and passed 689/689 (run
`migration-parity-343d0ca427754575a5effbabdf63a9bd`); it selected zero fault
contracts. The rerun used dirty revision `02284322`, so it supersedes the ten
Equalize failures for that saved cohort but does not verify later source edits
or the current worktree. The six
failures in `build/migration-parity/cpu-wip-status-cases-20261107.json` are an
earlier status mismatch set: extension cases ran only on the target, while the
I-to-PNG save case ran only on Pillow. A rebuilt-target 27-case status/codec
guard later passed 26/27; its sole remaining failure is I-to-PNG. Treat all
these artifacts as scoped evidence, not the full CPU acceptance run.

The generated parity status page and its `c961cb80ace865f0ca092c3648bb86c1417638402efb7480f872a6085b027cee`
manifest hash describe an earlier generated manifest. They report twenty
expected outputs passing 20/20 on a clean CPU profile and are historical only.
The historical Pillow 12.2 25,002-case selection ran on all nine selectors on 2026-11-10, but
its receipts are incompatible with Pillow 13.0. The Pillow 13.0 corpus ran at
clean `a8ba7b37e`: 24,718 passed and 303 failed. A later current-source Pad
selection passed 292/292; the full corpus has not been rerun after that fix.
The CPU acceptance matrix is still incomplete across operation × Mode ×
Architecture.

### Operation-family source work

| Family / area | Current state | Next pending work | Exit condition |
| --- | --- | --- | --- |
| Chops | Source review complete; runtime unverified | Include all 19 operations, exact-mode owner routes, architecture selection/fallback, P/PA palette retention, lazy output identities, compatibility routing, and receipts in CPU parity. Resolve every mismatch without changing the frozen cases. | Pass the frozen operation × exact Mode × Architecture parity and receipt cases, including fault order, direct/in-place/fused outputs, hidden-lane follow-on Paste, and same-mode Default fallback. |
| Composite | Source review complete; runtime unverified | Run CPU parity for CompositeModule, Merge, and BlendModule across accepted/rejected exact modes and source/mask pairs. Verify wrapper error order, exact architecture selection/fallback receipts, blocks, tails, empty paths, and output identity without changing the frozen cases. | Every accepted/rejected exact mode and source/mask pair, wrapper error order, prepared caller, and selected architecture/fallback receipt meets the frozen public output, mode, fault-order, and selection-receipt contracts; archive comparisons remain diagnostic evidence. |
| Enhance | Source review complete for Color and stateful wrapper routes; runtime unverified | Brightness byte source review is complete: Default and vector LUT owners share Pillow's float32 blend expression, and binary shift shortcuts are selected after factor conversion. Eleven Pillow byte modes are declared; seven have vector architecture routes and four are Default-only. LA/RGBA preserve uppercase `A`, while La/RGBa and RGBX transform their lowercase/padding samples; I/F, I;16-family, and internal high-depth routes remain CoreOnly. Pending: per-mode CPU parity and selected/fallback receipts, including sample 10 × factor `0.09999999999999999` → 1 on Default and each declared vector route. Contrast exposes 25 routed outcomes, including PA's alpha-band child path; CPU parity and architecture/fallback receipts remain pending. ColorSaturation exposes 25 route entries at its operation boundary: six prepared routes, three successful composed routes, nine Convert+Blend rejections, La and RGBa conversion/guard errors, and five internal-mode conversion errors. Each route records its exact L/LA intermediate; `Image::color_degenerate` reads it from the same declaration used by direct Rust routing. Python Color's direct typed shortcut now requires the base getbands/mode/convert/load/_new methods and no pending transpose-info load; overrides use the virtual convert(intermediate).convert(mode) sequence, and an unmodified base second conversion plus base load hook materialize its queued result before construction returns. Contrast now mirrors Pillow's Convert/Stat/new/alpha callback sequence, while Contrast, Brightness, and Sharpness still require frozen output/fault and constructor-boundary parity. The native Color base route reads the intermediate Mode from ColorSaturation support. Override output/fault and constructor-boundary parity remain pending under pillow-rs#3ptb. Blend carries the original source's backend pin when the eager base has none; host/direct parity and receipts remain pending. Pending: freeze enhancement parity, including metadata behavior and exact architecture receipts. Sharpness source review is recorded complete. | Every enhancement route has a verified source map, exact output/rejection contract, and injected context; support reporting agrees with public and prepared routes, and Pillow-defined parity passes without changing cases. |
| Paste | Source review complete; runtime unvalidated | The Rust mode/architecture strategies are source-reviewed. The Python facade handles Image-as-mask shorthand, None/default positioning, and len/endpoint expansion before source work. It loads and mode-converts Image sources or resolves color strings, then loads and realizes the destination, evaluates mask truthiness, and loads/realizes a truthy mask; final box arity/coordinate parsing precedes deferred raw numeric color coercion. Box preparation rejects a two-coordinate/default-position paste early when neither the source nor effective mask is an image. Image-as-mask shorthand is normalized into the ordinary mask path in Python, so its size survives two-coordinate expansion even when final truthiness disables the mask. Built-in tuple/list provenance is retained: lists are rejected without inspecting elements; exact-mode component parsing keeps integer, wide-integer, float, and invalid-type distinctions; correctly shaped byte-mode tuples apply `__index__` in field order after arity validation; `F` color parsing is mode-selected and uses float extraction, while non-`F` scalar classification preserves Pillow's integer-only path. The non-tuple scalar diagnostics now distinguish single-band and multi-band byte modes. The Python facade follows Pillow's box order: Image-as-mask handling, `None` to `(0, 0)`, `len(box)` before source work, two-coordinate source/mask size lookup plus Python endpoint addition, and deferred coordinate indexing at final Paste. After source, destination, and truthy-mask loads, the staged final call follows Pillow's `(iiii)` parser: the nested `(iiii)` parser rejects bytes and non-sequences, checks `PySequence_Size`, then fetches each item with `PySequence_GetItem` and converts it to C-int before fetching the next item; strings and bytearrays follow the sequence path. Deferred numeric colors resolve only after all box coordinates. Source review confirms `load()` realizes dirty P/PA palettes at the same source → destination → active-mask boundaries as Pillow. Source review now shows resolved image sources are snapshotted before destination and mask loading, matching Pillow's captured `source.im`; Python subclasses invoke virtual `convert()` at the same point while base-image conversion remains in the typed Rust Paste strategy. Tracing the two pathways shows that the already-resolved Python subclass result is stored and queued unchanged; `paste_prepared_source` does not call `resolve_image_source`. The ordinary base-image path converts differing modes in Rust before backend admission, except Pillow's `LA`/`RGBA`/`RGBa`-to-`RGB` fast path; the Python facade passes a source subclass's overridden `convert()` result through unchanged. CPU Paste now admits same-width 1-byte and 4-byte layouts on that resolved-source path and reconstructs omitted physical lanes for compact Rust carriers. This follows Pillow's optimization: its C paste primitive checks pixel size, then copies or blends every physical lane. `RGBa`-to-`RGB` remains premultiplied in this route; no unpremultiplication occurs because Pillow skipped `convert("RGB")`. The typed 2-byte I;16 alias layout now admits all four exact aliases and preserves destination Mode while translating byte order; the current source audit now accounts for byte-order translation, mask-byte selection, and clipped coordinates; runtime parity for those inputs remains TODO(pillow-rs#3ptb). `DynamicImage::put_pixel` now preserves retained compact-mode sidecars for its mode-aware logical writes. Generic mode-blind `ImageBuffer` mutable escapes invalidate sidecars, while mode-aware `DynamicImage` writes update them; downstream Paste/Histogram outcomes remain TODO(pillow-rs#3ptb). The final bridge holds a Python handle while deferred parsing runs, acquires the mutable `PyImage` borrow afterward, and retains exclusive access through the core mutation; source/destination aliasing and callback-driven mode/palette mutation still need parity. Noninteger singleton tuples in LA/La/PA now preserve Pillow's CPython `SystemError` from the post-unwrapping `PyArg_ParseTuple` call. The unused eager low-level PyO3 `paste` bypass has been removed. Then run CPU parity for all 25 mode identities and each declared/available architecture. Cover normalized source and mask contracts, clipping, empty regions, P/PA metadata, all seven L-mask NEON strategies, generic vector fallbacks, cross-layout RGB and RGBa routes, instance `convert()` overrides returning the source (RGBA→CMYK, RGB→RGBA, and L→P raw-copy successes; RGBA→L pixel-size mismatch; masked LA→RGBA lane blending) against the active Pillow 12.2.0 oracle; retain the user's independent Pillow 12.3.0 reproduction as separate evidence, I;16 byte-lane behavior, output identities, and selected-implementation/fallback receipts. Include Paste's frozen performance workloads after full CPU parity passes. | Every mode declaration, caller, serial and architecture path, kernel formula, fallback, and output identity has source evidence; parity and performance evidence pass their frozen gates. |
| Grayscale / Convert-to-L | Source review complete; runtime unverified | The Grayscale contract covers all 25 identities, rejects `La`, and routes the 24 accepted modes through exact-mode prepared strategies with `L` output. All 25 top-level mode declarations now map to their mode-owner `SUPPORT`; only RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, and LA8 declare architecture kernels, while the other 17 identities use Default fallback. HSV, P/PA, and RGBa now compute grayscale from source samples without allocating an intermediate RGB(A) raster; palette lookup and alpha unpremultiplication live in shared CPU helpers. The HSV, palette, and RGBa leaves retain their prior per-sample conversion arithmetic, but parity and performance evidence for these changes is pending. The bounded RGB eager ImageOps path and lazy P/PA palette input reach that owner with injected CPU context; Convert reuses exact-mode luma leaves and declared vector routes. The nonstandard-source declarations for CMYK, YCbCr, HSV, P, PA, I, and F all include L and LA, so public Convert dispatch selects their native mode strategies. The unreachable generic luma/luma-alpha fallback and YCbCr projection helper are removed. The Rust LAB-to-L route is a deliberate extension: Pillow 12.2 raises `ValueError: conversion from LAB to RGB not supported`. It carries `NO_PILLOW_SUPPORT`; a target-only output and Architecture receipt contract remains TODO. | Include exact output identity, error ordering, architecture selection/fallback, and supported-mode parity in the frozen CPU matrix. |
| ExtractBand / GetChannel / Split | Source review complete; runtime unverified | `Mode::band_count()` owns native band counts for all 25 identities. `Image.getchannel` declares exact selector counts: `1` and `P` preserve their mode identity, P retains its palette, I/F reject before materialization/index validation, I;16* is a marked CoreOnly native identity extension, and internal high-depth multi-band outputs preserve u16/f32 samples. Other multi-band routes use the prepared `ExtractBand` child. The SIMD caller-owned `GetDataBandInto` path forwards the image-resolved Architecture context into its byte executor. `OperationSupport::routes()` now exposes per-mode wrapper choices: identity/palette copies, loaded-L shared-buffer identity (exact-sized bytes, CPU-only image route) with a `native-identity` receipt and no architecture selection before its child fallback, tiny-RGB eager CPU child before its lazy child with explicit Loaded-source, requested CPU/SIMD, and maximum-256-pixel guards, and per-band children for multiband Split. The loaded-L identity declaration also carries the Loaded, CPU-request, and one-byte-per-pixel guards consumed by the wrapper; this no-kernel route resolves no Architecture. `GetChannel` identities in `Loaded` and caches successful at call entry use shared-buffer `native-identity` receipts; sources first materialized during the call keep their existing materialization path. Single-band `Split` reports the shared-buffer path except when cached `Bytes` P materialization creates the palette-index copy, which reports `native-copy`; direct `Paletted` P also reports its native index copy. Neither path selects an Architecture; an explicit GPU request records that these host-native routes did not dispatch to GPU. Lazy identity routes preserve source graph receipts before terminal identity receipts; multiband ExtractBand remains a deferred child selected when used. Split reads the band count from shared materialized pixels without cloning the raster, then branches non-palette mode-changing pipelines from those pixels while retaining the original backend pin; P/PA continue through the palette-aware pipeline boundary. Verify the branch output, metadata, and selection receipts in CPU parity and the affected Split performance workload. Source routing, output identity, metadata, and receipt paths are reviewed. Terminal timing/resource summaries stay scoped to the eager wrapper; source graph paths and selections stay in ordered operation receipts. Freeze CPU parity across all 25 modes and Default plus each non-Default architecture request, including Split branch outputs, metadata, error ordering, fallback receipts, and the affected Split performance workload. | Every public and pipeline route reaches the declared mode strategy or preserved rejection, its exact per-mode wrapper/child path is visible at the operation entry point, and selected/fallback behavior is recorded accurately. |
| Put and Get | Source review complete; runtime unverified | Python `getdata`, `putalpha`, `putdata`, `putpixel`, `getpixel`, `getchannel`, `split`, `getextrema`, `getprojection`, and `getpalette` now call `load()` at Pillow's eager boundary; `getpixel` converts `xy` to a tuple only after loading, and its binding accepts signed C-int coordinates, maps negative indices from the right/bottom, then checks bounds. Band/index validation and Python input coercion happen only after loading. This matches Pillow 12.2's `getdata()`, `getpixel()`, `getchannel()`, and `_ensure_mutable()` ordering. PutAlpha/PutAlphaData wrapper mode changes, error promotion, retained Python Image.info synchronization, mask validation, and typed output planning are source-reviewed; scalar PutAlpha parses through the PyArg-compatible C-int adapter before core byte clipping, and built-in integer overflow promotes the destination before preserving OverflowError. `AttributeError`/`ValueError` from scalar conversion now invoke the host `Image.new("L", current_size, alpha)` fallback before the core receives its mask; other parser exceptions, including custom `__index__` TypeError, propagate unchanged. The catch scope follows Pillow's `fillband` boundary; callback/fault parity remains pending, including reentrant destination changes after Pillow captures the output band. PutData load-before-coercion, typed mode/value-kind handoff, length validation, bulk-versus-callback split, and I;16 routed writes are source-traced; PutData P/PA metadata and payload transitions are source-reviewed; PutPixel host coercion now runs without an active PyImage borrow: palette components are parsed before a short palette mutation, coordinate conversion runs without a borrow, and value coercion completes before the final write borrow. Reentrant mode/palette mutation, mode expansion, P/PA metadata, coordinate/error order, and selected/fallback receipts still need frozen CPU parity. ExtractBand/GetChannel/Split caller and receipt-source review is complete; exact mode/architecture, child-selection/fallback, and ordered source/child receipt parity remain pending, as does moving GPU terminal eligibility into its declared support contract. `GetData` contracts and direct band reads consume the same exact `Mode::band_count()` mapping. The loaded single-band byte view now consults the owning Image's resolved backend route before taking its CPU-only zero-copy shortcut. GetData byte-band SIMD eligibility reads the exact-mode ExtractBand architecture declarations through the injected CPU context; CPU Default fallback is source-traced. GPU terminal eligibility remains mode-gated in its wrapper until a GPU support declaration composes this CPU contract. Full-buffer, scalar-only band, and formatted I/F/I;16 reads use image-pinned/active backend policy to create a routed CPU Architecture context and publish selected/fallback receipts. Formatted GetPixel source review is complete across all 25 exact identities: every path matches the DynamicImage variant directly; CoreOnly ImageLumaA16, ImageRgb16, ImageRgba16, ImageRgb32F, and ImageRgba32F return native u16/f32 components, and byte modes avoid the generic RGBA projection. Typed values, bounds, and wrapper error order remain parity TODOs. Lazy source operation receipts are now retained before terminal GetData selection for full-buffer, scalar, formatted, and CPU caller-owned band reads; SIMD/GPU projection keeps its selected ExtractBand child receipt. Eager terminal timing/resource summaries cover only the host read; ordered lazy source operation paths and selections are retained in separate receipts. Callback-preserving PutData now captures the original backend request and CPU Architecture together in one reusable sequence context; synchronous I;16 byte writes update only the supplied row-prefix bytes in place and preserve the rest of each native sample. They use Python's reported sequence length, scale/offset clipping, and the exact mode's byte order, with no full-frame staging buffer. Numeric I/F and I;16 PutPixel writers use the same routed context through exact-mode selection and retain the native Default fallback when no specialized writer exists. PutAlpha support now matches Pillow-wrapper rejections, exposes native-width strategies as CoreOnly, admits those typed modes through the Rust wrapper, and no longer predicts YCbCr/HSV output modes in the lazy planner. `putalpha_data` validates destination-mode support before inspecting the mask. Pillow 12.2 promotes a supported destination before reading the image mask's mode or calling its load method. The Python facade now queues this typed promotion before those callbacks, then queues PutAlphaData; direct Rust success retains its single native operation, and validation errors retain the promoted mode. Invalid non-image values also leave the promoted Rust mode visible. The wrapper's staged promotion work is included in the pending PutAlpha latency comparison. The PyO3 boundary maps arbitrary-size Python integers to the core clamp range without narrowing overflow. The Python facade refreshes retained `Image.info` after success or error so mode-promotion metadata remains visible. Verify mutation, metadata, load-before-coercion, and error order in CPU parity. RGBa PutAlpha and PutAlphaData unpremultiply directly into the required RGBA output before replacing alpha, with no staging raster; source-alpha edge parity remains pending. Default CMYK PutAlpha and PutAlphaData now write RGBA directly from native CMYK samples through the shared rounded CMYK pixel formula, avoiding an intermediate RGB allocation; declared vector routes remain separate. This scalar owner change is source-only and parity remains pending. Run the frozen CPU parity matrix for input/error order, callback-visible mutations, palette metadata, all declared exact modes and architectures, child selection/fallback receipts, and direct versus deferred read/write paths. Keep GPU terminal eligibility in the later GPU stage. | Each direct, callback, and deferred mutation/read path has an exact-mode contract and preserves ordering and output identity. |
| Statistics and Analysis | Source review complete; runtime unverified | `GetExtrema`, Histogram, Entropy, GetColors, Stat, and multiband Split size logical bands from exact `Mode::band_count()`. Pillow-facing `GetExtrema` has an `OperationId`/support descriptor, injected CPU context, and exact-mode Default strategies. Its byte modes scan native samples; I/F and I;16 preserve typed scalar results; five internal multiband modes return native u16/f32 extrema marked `NO_PILLOW_SUPPORT`. The separate public Rust/JS byte-valued `getextrema()` now has a `GetExtremaBytes` support descriptor, injected CPU context, exhaustive mode classification, and a Default owner in every exact-mode folder. It directly scans native carriers and projects typed samples without creating an RGBA raster, while preserving the old byte API's raw I/F byte lanes and RGB32F opaque fourth lane. All 25 mode-owned byte-valued GetExtrema scans now bound sample slices to width × height, ignoring trailing carrier storage. Its per-mode parity remains TODO. Eager terminal timing/resource summaries intentionally cover only the terminal operation; ordered source and mask graph operation paths and selections are preserved separately in operation receipts. Full-call latency remains a benchmark-boundary measurement. Histogram and Entropy each expose a 25-mode support contract and 1/L mask rules; all Histogram scans dispatch to exact-mode Default owners, including direct native u16/f32 binning for CoreOnly modes. `HistogramExtrema` is declared as an operation input for exact I/F modes and the Python wrapper passes it to the native I/F scan; unmasked I/F calls preserve Pillow's float32 scale, truncating bin conversion, and out-of-range sample omission, while masked calls and non-I/F modes ignore the optional range. Entropy prepares its own operation selection and composes the Histogram child under the same CPU context. The existing optional Entropy extrema argument is preserved as a native I/F Rust extension marked `NO_PILLOW_SUPPORT`; Pillow's standard Entropy method remains mask-only. CoreOnly LA16/RGB16/RGBA16 and RGB32F/RGBA32F retain native 256-bin-per-band scans with no converted image or staging samples; RGB32F preserves its legacy four-band result by counting the synthesized opaque alpha directly; these internal multiband routes are marked `NO_PILLOW_SUPPORT`. Histogram, Entropy, Stat, and GetColors preserve source graph operation receipts; Histogram, Entropy, and masked Stat also preserve mask graph receipts in validation order, followed by terminal strategy selection. GetBBox, formatted GetExtrema, and byte-valued GetExtrema preserve source graph receipts before terminal selection. Eager terminal timing/resource summaries intentionally describe only the terminal CPU operation; ordered source/mask graph paths and selections remain in operation receipts. Quantize selects exact-mode L, P, RGB8, RGBA8, and all CoreOnly modes. Each accepted identity has a Default owner in its mode folder. L/P/RGB/RGBA read native buffers. CoreOnly modes pass native typed samples to statically dispatched quantizers without a converted DynamicImage or RGB/RGBA staging raster; RGBa unpremultiplies its RGB channels per sample before alpha-aware octree reduction, RGBX ignores its padding byte; CMYK reads its RGB-projection lanes, while YCbCr/LAB/HSV retain their three native lanes, matching the Rust DynamicImage projection without materializing it; I;16 preserves clipping, and internal u16/f32 modes use the raster FromPrimitive mapping fused into sample reads. Every CoreOnly owner is marked NO_PILLOW_SUPPORT. I quantization reads each little-endian i32 scalar and clamps it to byte grayscale; F reads each little-endian f32 scalar through the raster float-to-byte projection. MaxCoverage and median-cut k-means no longer stage converted pixel lists. Mode × Architecture parity, lazy receipts, and CoreOnly extension evidence remain pending. Source-ownership update (2026-11-08): GetChannel outputs/routes, GetData/GetDataBand/GetDataBandInto support categories and band counts, GetBBox alpha inputs, and GetProjection default/architecture support are declared beside exact-mode owners; family support functions aggregate all 25 identities. A static source map passes 25/25 declarations and aggregator arms. This is not compiler or runtime evidence. GetBBox has an operation-level contract and exact-mode Default owners under `pool_cpu/ops/analysis`; native scans replace RGBA conversion for all 25 identities, with CoreOnly u16/f32 scans marked `NO_PILLOW_SUPPORT`. Source comparison against Pillow's `GetBBox.c` confirms logical byte lanes, alpha-only selection, I/F raw-word predicates, and native-u16 I;16 predicates. The previous Rust I;16 byte-prefix scan missed nonzero high bytes and now uses typed u16 samples. Direct GetBBox receipts include selected architecture and fallback reason from the prepared selection; its operation receipt list retains lazy source graph work before terminal selection. The eager terminal timing/resource summary covers only the CPU scan; ordered source graph paths and selections remain in operation receipts. The formatted GetExtrema implementation has been source-reviewed; include it and GetBBox's 25-mode Default and all 12 non-Default Architecture outcomes in CPU parity, with every non-Default request selecting same-mode Default. Cover I;16 values with a nonzero high byte and alpha-only selection. Split and GetChannel preserve native u16/f32 samples for CoreOnly high-depth bands. Stat now has a 25-mode support contract, exact-mode Default owners, routed context, direct I/F scalar strategies, and native CoreOnly histogram composition without converted rasters. Its masked wrapper prepares the selected Stat operation and Histogram child under one routed context, executes the child through the exact-mode Stat owner, and reduces native u32 bins with the public f64 reducer arithmetic without a widened histogram allocation. Stat wrappers now preserve source and optional mask graph operation receipts before terminal selection. The eager terminal timing/resource summary covers only Stat; ordered source and mask graph paths and selections remain in operation receipts. GetColors has a 25-mode support contract, per-mode Default owners, native-width CoreOnly counting marked `NO_PILLOW_SUPPORT`, and injected context. Its wrapper now preserves lazy source graph operation receipts before terminal selection; terminal summary scope is defined above; expanded parity remains TODO. Mask/error order for Histogram, Entropy, masked Stat, and GetColors has been compared with `3f9dfd403`; image-mask validation remains dimension-before-mode, unsupported scalar I/F Histogram is rejected after mask validation, and I;16 GetColors rejection remains before materialization. Direct/deferred receipts and mode × architecture selection/fallback evidence remain pending. Native analysis readers have been source-audited; no converted staging raster remains in active analysis paths. Fused RGB/CMYK PutPixel→GetProjection terminals retain one prepared `StrategySelection` through execution and operation receipts, including its fallback reason. Default versus architecture execution follows that selection rather than the Backend compatibility label; RGB/CMYK kernels and the CMYK positive-write path consume it without selecting again. Source review complete; mode × architecture parity remains pending. | Analysis output shapes and values follow exact modes; special Pillow scalar behavior, CoreOnly extensions, and all callers are source-accounted. |
| Transpose | Source review complete; runtime unvalidated | Keep all transpose-specific CPU execution serial. The archived byte Flip adapter used native row copying without a separate vector kernel; I;16 retains its real vector Flip. Native-byte transpose accepts the archived logical prefix for nonempty input and exact empty storage; I;16 transpose uses the prefix only for nonempty input and falls back for empty input. Single operations, adjacent-transpose fusion, palette metadata, and GaussianBlur's nested RGBA transpose now have source-traced callers and context flow. Pending: frozen operation × mode × Architecture parity and receipts for empty/trailing storage, every method, same-mode fallback, and canceled runs (`native-identity`, no architecture selection). Keep parallel collection kernels for the later `pool_parallel` stage. | Every method, exact mode, architecture guard, storage boundary, and caller maps to archived behavior; CPU parity and receipts pass. |
| Transform | Source review complete; runtime unverified | Python and JavaScript fill coercion use exact `Mode`. Pillow's public `fill` parameter is unused for built-in transform method IDs; `fillcolor` controls outside pixels. Any empty Python MESH iterable, including custom iterators, now bypasses resample validation and source loading, returning the prepared fill with P palette state retained; dedicated public empty-MESH P/PA cases now compare output bytes and retained palette data, with parity still pending. The Python facade exposes `Transform`, `ImageTransformHandler`, and `SupportsGetData` names, dispatches handler objects with Pillow's LA/RGBA premultiplied-image behavior, and resolves `getdata()` providers before calling the typed Rust route. Built-in LA/RGBA non-nearest calls follow the public LA-to-La or RGBA-to-RGBa conversion, Transform, and conversion-back sequence. For dirty Python P/PA palettes, the wrapper queues the typed request then loads the source. P results receive the copied Python palette snapshot; PA results keep index/alpha samples but clear the source palette, matching Pillow's newly allocated destination. P's copied Python palette state is retained. P tuple/name fill now retains its temporary palette when no source palette entries exist. The binding stages exact mode, list/tuple size checks, and fill resolution before method-data extraction; the facade snapshots info and P palette state before data callbacks, performs Pillow's MESH/non-MESH size and method comparisons, applies AFFINE/PERSPECTIVE slicing and EXTENT unpack/arithmetic before resample validation, then validates filter membership (explicit `None` is unknown; omitted calls pass `0`) and converts numeric data; built-in `fill` stays unused and execution reads the current Rust image after callbacks. Pillow creates the destination before consuming transform data; for MESH, it computes one quad, validates the filter, loads source and destination, writes into the same output using the original source, then requests the next record. The Python MESH facade drives mode-owned byte, scalar I/F, and I;16 writers into one destination, updates alpha and hidden lanes for each record, and emits a selection receipt for each executed quad. No full destination is allocated per quad. Non-MESH built-in methods now allocate an exact-mode destination and capture execution context in `_transform_prepare`, before method-data callbacks, then write directly through the selected mode/architecture strategy for all 20 accepted modes, including I/F scalar and each typed I;16 path. The existing mode-owned sampling loops remain in place. QUAD slicing, custom numeric operator execution and coefficient order run in the Python facade before resample validation; post-load C-int/C-double extraction follows bbox → resample → coefficient order. Dirty-palette mutation, callback-driven source-mode mismatch, exact fault/output parity, and receipt parity remain TODO; source-level direct-output wiring is not yet runtime-verified. A later MESH record whose source mode changes after a callback returns Pillow's `image has wrong mode` contract; this remains parity-unverified. Typed Rust calls preflight method data before filter parsing, including EXTENT zero-size and first-record MESH shape faults. The prepared boundary composes omitted physical lanes for LA/La/PA and three-band modes after the selected mode/architecture kernel: nearest sampling follows the declared affine/projective sampler, filtered LA-family output repeats final luma, and filtered three-band output retains its fill lane. Source review accounts for all 20 accepted mode owners, five explicit CoreOnly rejections, declared architecture dispatch arms and files, and same-mode Default fallbacks. CPU parity must cover every mode/method/filter route, including the new all-20-mode affine-NaN nearest cases, custom-handler dispatch, SupportsGetData forwarding, LA/RGBA handler mode conversion, non-default palette retention for empty-mesh P, and requested-versus-actual architecture receipts. The public Pillow output defines behavior; the lower-level ImagingCore region API has no independent parity contract. | All accepted and rejected modes and caller paths are accounted for. |
| Draw | Source review complete; runtime unverified | Byte-mode Bitmap mask owners now read the RGBa mask alpha and apply Pillow's `PREBLEND`; L-mask coverage follows the alpha-bearing destination rule, and PA writes its native index/alpha pair directly. The RGB Bitmap fast path delegates to the same exact-mode Default owner, preserving that RGBa behavior. I;16* Bitmap follows Pillow's native byte loops: L blends both sample bytes, while 1/RGBA/RGBa step through the raw row byte prefix; RGBa uses alpha with PREBLEND. I;16* Text blends L coverage against both native sample bytes and keeps RGBA-mask raw row-prefix behavior; neither path projects the full raster through u8. Text dispatch specializes directly over FreeType's coverage or RGBA mask layout without an expanded intermediate mask. Pillow's ImagePath.Path input conversion and transform protocol are now represented by the Rust-backed `PIL.ImagePath.Path`; ImageDraw2 uses that shared path type for transformed coordinates. Source review now covers ImagePath general-sequence access, PyBUF_SIMPLE buffer reads, conversion faults, and mapping-guard behavior; callback mutation and Draw output/fault parity remain pending in the frozen CPU gate under `pillow-rs#3ptb`. CMYK and RGBA Text were source-compared with baseline `3f9dfd403`: their native per-channel blend, fill/mask behavior, clipping, stroke order, and output identities match by inspection; direct coverage reads replace the temporary RGBA mask expansion. Follow-up review corrected integer CMYK Text ink handling: its exact-mode owner now consumes all four packed bytes already resolved by Draw instead of zeroing the upper channels. Draw integer resolution now also preserves Pillow’s low-four-byte multi-band UINT8 packing and low-32-bit I storage after signed-long-long parsing. Pillow source confirms both contracts. It now maps two-band LA/La alpha from packed byte 3, three-band LAB from bytes 0–2, and four-band RGBa/RGBX from bytes 0–3. LA/La text shares Pillow’s zero-alpha rule. RGBA/RGBa L-mask text raises color coverage only for zero-alpha destinations; RGBA-mask text follows Pillow’s `paste` blend, selected from the mask layout rather than stroke-pass state. RGBX blends its fourth byte. Singleton tuples now follow Pillow getink unwrapping before mode resolution, including packed multiband inks and the one-value F path; unsupported single-band tuple arities keep mode-specific errors. Wide-integer conversion, tuple-component C-int overflow, and exact mode output still need frozen CPU parity. The frozen `pillow-rs#brg0` CMYK Text and MultilineText inputs use `fill=None`. Source review confirms the wrapper resolves that to `(255, 255, 255, 255)` for the CMYK context, and the exact-mode owner applies the same four-channel coverage blend as the archived CMYK carrier path. No source-level default-fill discrepancy is identified. Keep these frozen calls as focused parity cases; exact output and boundary behavior remain runtime-unverified. Pillow 12.2 `ImageDraw._getink` resolves P-mode tuples through `ImagePalette.getcolor` before ink validation; lists reach ink validation without palette allocation. The Python Draw facade resolves attached-P tuple and string inks through its Python-owned ImagePalette.getcolor before passing a typed index to Rust; this preserves Python ImagePalette.colors lookup, normalization, and mutation before raw-byte conversion errors. Direct JS/Rust tuple input now resolves all arities with a Draw-owned key cache; the core keeps only complete entries; Python retains trailing bytes in its public ImagePalette buffer. Successful palette-byte changes use a metadata-only Rust replacement. The Python/Rust palette boundary now retains dirty bytes in the cached Python `ImagePalette` and sends Rust only the first 256 complete entries needed while drawing. `_realize_dirty_palette` installs the full raw palette at Pillow load/access boundaries, so oversized palette errors are deferred until realization. Dirty LA raw-palette realization now commits native LA alpha even when `info["transparency"]` is absent; when present, it applies Pillow integer/table transparency into the Rust alpha sidecar and preserves the Python info mapping. It rebases metadata after raw-palette installation and before transparency coercion so setter errors retain Pillow’s mutation order. PA raw-palette construction is explicitly rejected when Pillow has no RGB/RGBA unpacker for the supplied raw mode; this does not restrict PA as a raster mode. Dirty P/PA resize realizes after filter/gap and identity checks. Rotate now prepares and validates its typed request once, realizes the dirty palette, then queues against the updated palette metadata. Thumbnail now preflights geometry, no-op, and same-size output for every public mode before Python filter coercion. Resize-reaching paths normalize once; dirty P/PA palettes are realized only after that validation. Paste now stages Image-source loading/conversion or color-string resolution, destination realization, and mask loading in Pillow order, with raw numeric sources and box coordinates resolved at the final call; The direct low-level PyO3 `paste` bypass was removed after a repository-wide caller search found no users; the staged prepare/apply methods are the sole Python binding route. Singleton-tuple `SystemError` diagnostics are source-corrected. Remaining parity TODOs cover custom/reentrant sequence hooks, unusual subclass and source/destination alias mutations, and exact output/fault contracts. Save now injects the wrapper load/palette realization after path/format discovery and before encoding; file-object extension handling and complete writer/path fault order remain TODO. Finish Transform source review. Verify Paste malformed-length/error-order and thumbnail dirty-palette cases in CPU parity. JavaScript numeric arrays and direct Rust callers use the core resolver as tuple-like P Draw colors at every arity. Python arbitrary-size integers retain exact sign/magnitude keys; wide integral JavaScript numbers use the same canonical representation, including equality with numerically equal floats. The Draw-owned key cache preserves `ImagePalette.colors` lookup and key insertion before byte type/range errors; allocation failure still precedes that insertion. Valid nonstandard-arity colors update complete core palette entries, while Python retains partial tails in ImagePalette.palette; Rust stores only complete entries used by indexed pixels. Python Draw mutates its cached `ImagePalette` before passing the resolved index and installs only representable Rust palette entries; `_sync` replaces the raster handle without overwriting the Python-owned byte buffer, preserving malformed and dirty bytes across later draws until realization. The core Draw context starts with native compatibility-metadata reservations, and Python refreshes them from the current mutable `Image.info` before P color resolution; allocation advances past reserved indices but appends palette bytes at the current byte length, matching Pillow 12.2; full-palette allocation chooses an unused index in the established reverse scan order. Lazy inputs keep earlier operations behind a metadata-only layer instead of materializing pixels. The lower-level typed DrawColorInput retains tuple/list provenance and all-arity P tuple resolution; the Python facade handles attached-P tuples through its ImagePalette, while lists remain outside palette allocation. The core cache preserves numeric key equality and key insertion before byte-conversion errors. Allocation availability is checked first; RGB/RGBA alpha normalization happens before lookup, and mixed numeric tuples retain component kinds for ordered type/range errors. Direct Rust/JavaScript paths expose the normalized core palette, so raw tails are not part of that raster state. Source review of JavaScript numeric-array tuple routing and core P palette lookup/allocation ordering is complete; verify Python tuple/list, alpha, reserved-index, cache mutation, malformed raw-tail, and byte-conversion error behavior in the frozen CPU parity matrix. The shared multiline text layout now follows Pillow 12.2 `ImageText.Text._split`: mode-aware font metrics, horizontal and top-to-bottom line placement, its single-line early path, alignment validation, Unicode-safe anchor indexing, and centered/right/justified offsets. CMYK and stroked RGBA continue through exact-mode raster owners. The centered-anchor, explicit-font-size, three-line-spacing, CMYK, and stroked-RGBA `pillow-rs#brg0` cases remain runtime-unverified until the CPU source-completion gate. PA raw-palette input remains explicitly rejected when Pillow has no RGB/RGBA unpacker for its raw mode. The public Python Bitmap facade loads the mask before resolving fill; direct PyO3 Bitmap calls now invoke the existing Rust image load method before fill parsing, which preserves its GIL-release behavior. The direct PyO3 ImageDraw constructor also loads the destination before parsing an optional mode, matching the public facade and Pillow error order. Text and MultilineText now prepare the default font and validate embedded-color mode before fill resolution, and resolve stroke_fill only when stroke_width is truthy; direct PyO3 calls follow the same order. Source audit now accounts for every explicit load in the Draw facade and binding: destination at construction, Bitmap mask before fill parsing, color-font mask then coverage before native paste, and floodfill's image before pixel access. No further load-triggering Draw wrapper path remains unidentified; verify Bitmap load-before-fill ordering and malformed-arity, over-capacity, Python tuple/list, reserved-index, palette mutation, I;16* byte-order, existing ImageText.Text/custom-font argument behavior, and direct Text error order in parity; Draw support now links the Paste child. The current fast-path predicate is: nonempty mask, an explicit non-CPU backend request, mask `1` for destination contexts `1`/`P`/`L`/`LA`/`RGB`/`RGBA`/`CMYK`/`HSV`, or mask `L` for `L`/`RGB`/`CMYK`/`HSV`; RGBA drawing context over RGB is also eligible for RGB storage. Draw's top-level support now exposes the Paste child in each exact mode's `OperationModeRoute`, with raster/context/mask triples, explicit GPU/SIMD backend selection, and nonempty-mask guards; the wrapper evaluates those same guards before queueing the child. Source inspection confirms the lazy graph records the child as Paste; Draw fast-route parity and receipt evidence remain pending. | Every Draw operation's entry contract, source mode, destination mode, architecture route, metadata behavior, and preserved error order map to native-mode implementations; CPU parity and selection receipts pass. |
| ImageOps | CPU routes implemented; wrapper ownership source-reviewed; runtime unverified | Posterize, Solarize, Autocontrast, and direct Rust Equalize/Colorize requests validate through their operation declarations. Rust's callback-driven Equalize wrapper owns the public composition: support selects P→RGB, core invokes public Histogram, builds the per-band LUT while Python callbacks preserve host collection/numeric protocols, validates the current exact mode after callbacks, then invokes public Point. Colorize delegates typed mode admission, ordered point checks, LUT range iteration and component arithmetic orchestration, then public Convert→Point calls to its Rust operation strategy; PyO3 adapts Python sequence, indexing, range, length, and arithmetic protocols. Autocontrast now carries cutoff pairs, ignored bins, and preserve_tone through its typed request and implements native L/RGB CPU histogram and LUT behavior; preserve-tone reads RGB-to-L statistics while retaining RGB output. The unmasked GPU L/RGB path lowers this LUT to Eval, while masked nonzero cutoffs, ignored bins, and preserve-tone remain TODOs for GPU work. For other rejected modes, the typed Rust API computes the source (or preserve-tone L) histogram and validates LUT parameters before returning the declared mode error; accepted Rust requests remain lazy; Rust also declares a CoreOnly P-index strategy, while the Python callback applies Pillow's final `_lut` mode checks using the wrapper's public `mode` after building the histogram LUT: P raises `NotImplementedError`, only L/RGB proceed, and RGB repeats a 256-entry LUT. The Python binding now preserves host histogram values through bin reads, total accumulation, cutoff arithmetic, edge trimming, and truthiness; Rust owns traversal and final LUT construction, while callbacks retain Python item/numeric/comparison protocols. The focused Autocontrast CPU parity and custom-value reproduction passed before the public-mode correction; those receipts are stale for the current source diff. Full mode/backend receipt coverage remains open. ImageOps Invert keeps its typed Rust Invert child separately. The Python facade runs Pillow's public-mode branch and `image.point(lut)` through a Rust callback orchestrator; `ImageOpsInvert` declares the Eval child for this wrapper path, while Point owns selected CPU/Architecture execution. The shared ImageOps Posterize/Solarize callback routes now reject P as Pillow `_lut` does; their separate Rust `Posterize`/`Solarize` operations retain `NO_PILLOW_SUPPORT` CoreOnly P-index strategies. Visible-mode errors and subclass point overrides are preserved. Output, fault-order, and receipt parity remain pending. CropBorder and Expand use exact-mode prepared declarations. Expand retains scalar/component values for I;16 and CoreOnly high-depth modes through a typed CPU pipeline route; its per-mode support contract declares P palette-index/color input and byte-carried fills for PA and other byte modes; the Python adapter resolves palette colors for P and PA when a source palette exists, while high-depth modes use native u16/f32 fills. The Python Expand adapter normalizes borders, resolves named fills against the initial mode, and runs P/PA palette callbacks before capturing the source raster used to derive the typed target mode. It validates dimensions against that captured target before building the filled canvas; if size comparisons replace the source wrapper, Paste receives the current wrapper while the canvas keeps the captured target mode. Palette callback mutation ordering is source-reviewed. The binding converts the captured `image.mode` spelling to the flat `Mode` enum and prepares the canvas from it, including subclass overrides. Dirty/raw palette faults, output metadata, mutation/fault parity, valid mode-override output parity, invalid-mode error ordering, and P/PA tuple-fill faults when a subclass suppresses `image.palette` remain pending. P palette resolution consumes `OperationId::Expand.support()` and its declared fill rule instead of repeating a mode check. Malformed/raw palette behavior, exact output metadata, and mutation/error-order parity remain pending. Source correction (2026-11-03): `PreparedExpandFill.mode` remains the Image.new target snapshot. The binding validates against that mode and materializes the filled target canvas before the Python facade calls `output.paste(image, (left, top))`. This preserves allocation-before-source-load order and delegates current-source conversion, same-pixelsize RGB behavior, and source loading to the existing Paste facade; the result is then materialized before returning, preserving eager paste faults. The direct Rust same-mode Expand API remains fused. Dirty/malformed palette behavior, output metadata, and mutation/fault parity remain unverified. I/F keeps its exact scalar bit-pattern encoded in the existing raw byte carrier. The direct byte-tuple route is unchanged and GPU declines the typed variant. CoreOnly Histogram bins now scan native samples; RGB32F preserves the legacy four-band output without conversion, and its result layout is exposed by the exact-mode support classifier. Draw P tuple/list provenance is preserved, and 3/4-component integer tuples preflight palette-slot availability before byte-range errors. Non-integer tuple values, other arities, allocation mutation order, and parity remain pending. Equalize's P-to-RGB transition is intentional because Pillow converts P to RGB before histogramming and returns RGB; retain it as an operation-defined mode transition, not a reuse cast. Source ownership is reviewed; exact public outputs, fault order, metadata, and strategy receipts still require the frozen CPU parity gate. |
| Convert, ConvertLab, RemapPalette | Convert/ConvertLab/RemapPalette source review complete; runtime unverified | CPU-owned binary, matrix, I;16-target, palette-wrapper, scalar I/F→RGBX, premultiplied, and RGBX compositions pass the image-resolved Backend/Architecture context into eager child strategy execution; CPU-owned fallbacks retain the original backend receipt. Lazy source materializes through its own pinned/active route before wrapper children run. Convert declares LAB and palette wrapper routes plus exact-mode premultiplied/RGBX outcomes in its OperationSupport; supported premultiplied pairs now write directly from their mode-owned samples without an RGB intermediate, and prepared Convert applies declared wrapper rejections before strategy assessment. Lazy Convert-to-P/PA uses a prepared CPU strategy, preserves an injected CPU Architecture when the wrapper-owned route falls back from GPU, and caches generated palette, alpha, and wrapper metadata beside its pixel materialization; copy, load, palette access, metadata queries, and palette-preserving follow-on pipelines consume that sidecar without forcing early materialization. Both image-facing cross-mode P/PA callers use `execute_convert_output`; `Image::push_op` keeps palette conversion in its own lazy node, and the generic raster-only prepared path rejects that output before pixel work. CPU parity and architecture/fallback receipts remain pending. Same-mode Convert now makes the target guard ineligible for architecture kernels and selects the mode's Default identity path. RGB→La/RGBa, LA→La, and RGBA→RGBa are Pillow-supported pairs. PA, RGBA→La, high-depth, CMYK, YCbCr, HSV, LAB, and RGBX premultiplied targets are CoreOnly direct mode strategies marked NO_PILLOW_SUPPORT. The Convert source/target/architecture comparison for all 25 source modes is recorded in the inventory. ConvertLab source review confirms the exact RGB8 prepared contract, archived CLUT interpolation and byte packing, explicit Convert normalization, and rejection of other direct input identities; no architecture-specific kernel is declared. RemapPalette source review confirms the public L/P contract, inverse index mapping, fixed P result, palette-alpha wrapper metadata, empty-raster fallback, declared architecture routes, and Pillow 13 wrapper callback/load/fault ordering; its redundant unreferenced Default wrappers were removed in favor of the shared native helper. The CMYK three-byte intermediate stride versus four-plane `CMYK;L` high-index output remains a frozen-parity decision. Pillow 12.2 pair scopes are now explicit: I;16→L/RGB/I/F, I;16L and I;16B→L/I/F, and I;16N→L, with identity copies; other declared pairs are CoreOnly. Pending: verify Convert, ConvertLab, and RemapPalette outputs and selection/fallback receipts, including RemapPalette CMYK indices around the intermediate palette limit; cover CoreOnly conversion pairs across exact modes and Architecture requests. | Every conversion route has an explicit source/target identity and exact strategy or rejection. |
| Resize, Reduce, Thumbnail, Crop, Geometry | Source review complete; runtime unvalidated | Resize/ResizeBoxed retain exhaustive operation-level support facades, while each of the 25 exact-mode `ops.rs` owners declares its own ordinary and boxed `ModeSupport`; the family classifier only aggregates those declarations. Shared byte and I;16 architecture-guard construction lives in `resize/helpers/support.rs`. Both operations still dispatch all 25 modes through prepared CPU strategies. Public and direct prepared Resize validation order and float32 box narrowing match the baseline source. Exact `One` and `ImagePalette8` filter policy now resolves to nearest in the exact-mode Resize owner before direct or nested child selection; contain, cover, fit, scale, and pad reach this same prepared route. Boxed tall-image Resize now composes vertical then horizontal prepared child strategies for Pillow modes; LA/RGBA become their explicit premultiplied Mode for the two passes and return to their original mode once. F32 keeps its specialized tall path. CoreOnly typed modes retain their existing native path. I;16 Reduce scans typed u16 samples and returns each exact source identity without an 8-bit L intermediate. The native u16/f32 reducers widen row coordinates before flattening; Reduce facade/parser and operation support routing are source-reviewed: the load/error order and P factor-one box path, C-int overflow and custom `__index__` handling, list/tuple factor normalization, Python sequence box acceptance, length-before-item conversion, ordered integer conversion, and load-before-parse behavior are mapped. A static source map confirms exhaustive support, prepare, selection, and execute aggregation across all 25 exact-mode owners; this does not verify their algorithms or architecture kernels. Source comparison confirms I32 Default preserves the archived little-endian scalar carrier, wrapping quartet/pair sums for interior blocks, per-sample f64 accumulation on partial edges, and final `round_up`; archived native-byte Reduce admission has no I32 specialization. The F32 eight-lane 2×2 kernel uses the archived float quartet addition order and f64 scaling, with the same shape guard selecting the exact-mode Default fallback outside that tile. The initial Default native-byte comparison matches the legacy fixed-point block/edge divisors and straight-alpha premultiply, average, and unpremultiply flow; the RGB sparse route retains its size threshold, density cutoff, scan grouping, block sums, and rounding. The later Reduce source-closure entry below completes the per-mode Default and byte architecture-kernel comparisons. Sequence callback/fault receipts and mode × architecture parity remain pending. The exact-mode One and P owners declare identity-factor constraints only; `reduce_public` copies full-image factor-one requests and crops factor-one sub-boxes, while non-identity requests are rejected after box validation with Pillow's wrong-mode ValueError. No Rust-only One reduction extension is declared. Crop source review now covers signed/out-of-bounds normalization, lazy operation routing, injected context through CPU registry preparation, exact-mode Default/architecture selection and fallback, native output identity, CropBorder composition, and empty/error handling; parity remains pending. Thumbnail caller and architecture source review covers public sizing, filter normalization, materialization/error order, exact-mode selection, guarded I32 AVX/FMA streaming with Default fallback, and same-context Reduce/Resize children. The host boundary now keeps an opaque `ThumbnailPlan` across `draft`, filter, and reducing-gap coercion; it floors/unpacks public bounds, performs Pillow's draft multiplication/int conversion, carries returned draft boxes through boxed Resize, and parses custom filter/gap values outside a `PyImage` borrow. Execution uses the preflight dimensions instead of recomputing against potentially changed image dimensions. Source review is complete: geometry stays preflighted, while filter/gap callbacks run before the current image is routed; if a callback resizes the wrapper to the selected target after the resize branch begins, the facade assigns a current-mode identity resize, preserving Pillow's core replacement and prior `getdata` backing-store identity. Verify callback-driven mode/storage changes and `getdata` identity in CPU parity. The Python facade preflights no-op, geometry, and same-size output for every mode from known dimensions before filter coercion; a request that does not reach resize returns without decoding or parsing the filter. Resize-reaching calls normalize the filter, validate reducing_gap, then realize a dirty P/PA palette. The gap now travels in the prepared Thumbnail operation into the CPU mode strategy: Pillow default 2.0 selects the existing reduction path, while None disables it; mode-specific alpha and nearest policies remain selected by the exact-mode strategy. Custom gaps decline direct GPU reduction and retain host control. Mode × architecture, reducing_gap, and dirty-palette parity remain pending. Direct typed Pad APIs declare `PadFillRule` per exact mode beside `PadParameters`: byte-carried, P palette index/color, native u16, or native f32. Their Rust wrapper reads that declaration before constructing the typed fill payload; Python ImageOps.pad follows the host composition described below. Mixed numeric tuples retain component kinds and follow the existing mode conversion; in Pillow 12.2, P uses its temporary palette path only for all-integer tuples of length three or four, while mixed tuples reach the single-band core fill error path. Invalid P tuple arities are rejected before component coercion, and a one-integer tuple uses scalar extraction so oversized values preserve the overflow fault. Valid integer tuple alpha is checked before RGB byte conversion and the temporary P fill uses index zero. Image.new now classifies scalar and tuple fills by exact Mode, routes strings through ImageColor, performs P RGB(A) temporary-palette allocation in the Python facade, and applies Pillow's `_check_size` order before native C-int size parsing. Custom numeric, wide-tuple, P palette, and size-callback output/fault parity remains unvalidated. Dirty/raw palette behavior, allocation, and output metadata remain parity TODOs. Oversized integer tuples and lists are retained for mode-aware resolution. P RGB(A) integer tuple fills now validate byte ranges and preserve Pillow’s non-opaque-alpha error order. P color strings use unclamped RGB parsing and discard alpha as ImageColor.getcolor does. P name and RGB(A) tuple/list fills now retain Image.new's temporary RGB palette when no source palette is attached; a source palette carried by the P image remains authoritative. Python ImageOps.pad composes the existing `contain`, `Image.new`, palette, and `paste` wrapper operations in Pillow order, including canvas construction before centering access. Its unused Pad-specific PyO3 bindings and centering adapters are removed. Direct Rust and JavaScript callers retain the typed core Pad operation and CPU Default/SIMD strategies; output, callback, and mode-by-architecture parity remain pending. The shared `ops/pil_resize` coefficient and sampling helpers remain in production; unreferenced, undeclared full-operation wrapper files were removed after the active module and call-site audit. Rayon scheduling belongs to the later `pool_parallel` owner. | Nested and direct operations use prepared child strategies with the same context and preserve exact empty/crop behavior; CPU execution stays serial regardless of the separate Parallel CPU build. |
| Color3DLut and Eval | Source review complete; direct point receipts source-updated | Verify operation × exact Mode × target Mode × Architecture behavior, point error order, and selected/fallback receipts in frozen CPU parity. | All source paths and operation contracts align; then include them in frozen parity. |
| Effects | Source-reviewed; runtime unvalidated | Run CPU parity for all declared mode and architecture routes. Cover generator output modes, Spread/Noise RNG order and identity fast paths, selected implementation receipts, unavailable-architecture fallback, and eager/lazy caller behavior. | The archived source map is complete; every declared mode and architecture route, output identity, RNG transition, and receipt matches on CPU parity. |
| Rotate | Source review complete; runtime unvalidated | The wrapper's validation order and exact-mode force-nearest rules have been compared with the archive. The fillcolor binding now uses Transform's mode-aware fill adapter, including ordinary P-mode RGB(A) list/tuple handling and temporary fill-palette propagation. Fill parsing now rejects non-P lists, unwraps singleton tuples, and enforces long-long/C-int tuple bounds in the host adapter, while P RGB(A) tuples/lists retain wide integer palette handling. The Python facade now builds reverse-affine coefficients and the expanded output size with Pillow's repeated point reads, Python arithmetic, and expansion bounds before the typed Rust adapter parses filter/fill values. Rust carries those values through the same mode/architecture Rotate strategy, and dirty-palette and ordinary calls share the path. Overridden transform methods use public dispatch, and LA/RGBA non-nearest calls preserve overridden convert composition. Output-size container/arity/nonnegative validation precedes fill work; named colors and all-integer P RGB(A) sequences resolve before signed C-int narrowing, while other fill coercion follows it. The fixed six-value affine tuple is parsed after load through six ordered `PyFloat_AsDouble` conversions, matching Pillow's native parser; coefficient callback mutation, remaining custom fill faults/output, and runtime mode/architecture receipts remain parity-pending. Shared geometry source comparison is complete: coefficient rounding/order, center and translation composition, bounds, expanded-canvas dimensions, and shift match the archived CPU route. The 25 exact-mode owners are mapped to native Default and eligible architecture kernels: `1`/`P` force nearest; `LA`/`RGBA` use alpha-aware filtering; premultiplied, indexed-alpha, padding, and color-space modes keep their stored-channel behavior; `I`/`F` use typed scalar interpolation; the four I;16 identities remain typed-u16 Default-only because the archived byte SIMD coordinate and fill contract differs; internal high-depth identities use native samples and are marked `NO_PILLOW_SUPPORT`. Architecture strategies retain identity and exact right-angle paths for eligible nonempty inputs; host-computed affine requests bypass the zero-angle clone shortcut so translation and output geometry are applied. Empty-source requests select same-mode Default so its fill, geometry, and rejection behavior remain authoritative. Compact LA/La/PA affine nearest paths sample omitted bytes with the selected nearest geometry, filtered output repeats final luma into omitted bytes, and RGB-family filtered output retains the configured physical padding fill; source behavior is reviewed, runtime parity remains pending. The routed context reaches prepared execution across ordinary, palette-safe, and cached-prefix paths; each owner returns its exact `DynamicImage` identity. The Python dirty-palette route validates once before palette realization and checks the prepared plan's exact mode before enqueueing. Ordinary and prepared Rust/Python routes converge on the same lazy pipeline; prepared execution and batch telemetry record the selected strategy and fallback reason before running the leaf. CPU parity remains pending for dirty-palette mutation/error order, empty outputs, per-mode output identity, and runtime architecture receipts. | Every supported mode and same-mode fallback is source-accounted and then parity-verified. |
| Filter | Source review complete for the declared CPU operations and callback adapter surface; runtime remains unverified | Run frozen CPU parity for all filter operations, parameters, exact input modes, architecture routes and same-mode fallbacks. Cover Filter3x3/5x5 native kernels, small/no-interior copies, I32 uniform identities and AVX/FMA binomial routing, blur and radius specializations, ModeFilter palette/native-channel behavior through exact-mode support and Default leaves sharing the native-byte kernel, UnsharpMask's nested blur context, rank edge/empty behavior, architecture receipts, and public wrapper error/coercion order. Max/Min/Median/Mode/Rank PyO3 arguments preserve the Python object, use Pillow's numeric defaults, and pass through the shared C-int parser; rank-family size preflight matches Pillow's float32 workspace overflow bound. Explicit-None, integer-boundary, window-limit error-order, and coercion parity remain pending. Their CPU-only eager wrappers retain the requested backend while injecting CPU Architecture, including the SIMD alias when GPU is preferred but has no strategy. Kernel source support includes the 13 admitted byte modes (including `One`), `I`, and exact `I;16*` owners with native-u16 Default kernels; P, F, and internal multiband high-depth stay rejected. F32 Default mirrors Pillow 12.2's in-place comparison/partition selection in safe Rust; NaNs and mixed signed-zero images fall back from vector order to this path, while total-order-compatible samples may use the bounded vector strategy. Pillow's edge-padding-plus-crop rank semantics match nonempty clamped gathers; empty-image rank windows of size 3 or larger return a safe empty result, marked `NO_PILLOW_SUPPORT` because Pillow 12.2 reaches an edge read. Source review of Pillow's built-in callback bodies confirms their core calls: `expand` then `rankfilter` for rank filters, `modefilter` for ModeFilter, and `color_lut_3d(mode, BILINEAR, channels, size, table)` for Color3DLUT. This package's built-ins call equivalent typed Rust methods directly. PyImage exposes the callback-visible `mode`, `size`, `bands`, `readonly`, sequence, pixel/data/analysis/band-read methods, plus Rust-backed `expand`, `rankfilter`, `modefilter`, and `color_lut_3d`. PixelAccess retains the exact loaded Rust image and uses dedicated low-level read/write/delete adapters. Source review covers tuple-only coordinates, Pillow's __int__ conversion, negative indexing, bounds order, direct mode-native writes, and deletion as a checked no-op; output, mutation, callback, fault, and view-identity parity remain pending. Float coordinates truncate toward zero within i32; infinities and finite overflow saturate to the i32 bounds, and NaN maps to zero. These inputs are defined by this Rust adapter because Pillow's C cast has no portable result; freeze the extension in CPU parity. `putband` and `fillband` now have source-reviewed native CPU strategies; callback output, fault, and receipt parity remain pending. `setmode` has a source-reviewed native CPU strategy; output, fault, fallback receipt, and RGB-to-RGBX padding-lane parity remain pending. Exclude raw `ptr` because it is outside the behavior contract. The private Color3DLUT adapter now accepts Pillow’s exact-length contiguous one-dimensional float16/float32/float64 buffers and retains its sequence fallback. Color3DLUT output, fault, and receipt parity remain open. No validation has run for this family. | All filter operations, parameters, input modes, and architecture/build guards are source-reviewed; public output, fault, callback, and runtime parity gates pass. |

PutAlpha callback-order source update (2026-11-04): callback-capable inputs now
see the destination's Pillow alpha-mode promotion before scalar coercion or
image-wrapper inspection. Exact built-in integers and exact facade `Image`
masks retain direct routes so valid masks do not stage a separate promotion
raster. Python `Image` subclasses still pass through the callback-capable path;
the facade now evaluates their `mode`, `load()`, and post-load `mode` in
Pillow's order. Subclass-returned mode/carrier combinations, wide integer and
custom `__index__` conversion/error behavior remain pending. The exact-Image
mask fast path still needs a fault-order review when `alpha.load()` can re-enter
and observe destination promotion. Output, fault, reentrant mutation,
metadata, and strategy-receipt parity remain pending.
Source: [Pillow 12.2 `Image.putalpha`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1880-L1929).

GetPixel C-int source follow-up (2026-11-04): after tuple conversion and
arity validation, x and y now use the shared C-int parser in left-to-right
order, preserving Pillow's signed range and overflow message without a
PyImage borrow. Negative-index normalization and bounds checks remain after
both conversions; exact fault/output parity remains pending. Source:
[Pillow 12.2 `Image.getpixel`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1612-L1625).

Filter source review has an explicit wrapper order: the Python facade loads
before filtering; the PyO3 layer checks `_apply` before reading the filter name,
then performs named mode preflight before invoking `_apply`. Pillow 12.2's
built-in callback methods are source-mapped: rank filters call `expand` then
`rankfilter`, ModeFilter calls `modefilter`, Color3DLUT calls
`color_lut_3d(mode, BILINEAR, channels, size, table)`, and the other built-ins
use `filter`, `gaussian_blur`, `box_blur`, `unsharp_mask`, or `copy`. PyImage
provides Rust-backed implementations for these methods, including native
edge-replicating expansion and cropped rank filtering. Its callback-visible
mode, dimensions, bands, readonly state, sequence, pixel/data/analysis, and band
mutation surfaces have source crosswalks. The low-level Color3DLUT table
adapter accepts exact-length contiguous one-dimensional `e`, `f`, and `d`
buffers, then falls back to the Pillow sequence contract; mode, filter,
channel, and dimension checks precede table parsing. RankFilter and
Color3DLUT private callback routes remain distinct from their public classes
where Pillow exposes different shape or channel behavior. Source gaps are
closed for this surface; output, metadata, callback/fault order, exact
operation/architecture receipts, parity, and performance remain in the CPU
gate. Exclude the raw `ptr` capsule as implementation-specific. Source
references: [Pillow 12.2 `ImageFilter.py`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageFilter.py#L1445-L1457), [rank](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageFilter.py#L1537-L1547), [mode](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageFilter.py#L1626-L1628), [Color3DLUT](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageFilter.py#L2318-L2333), and [`_imaging.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L772-L931).

ModeFilter's active native-byte helper was compared with the implementation at
`3f9dfd403`: it keeps the same `size / 2` neighborhood, skips out-of-bounds
samples, builds one histogram per channel, chooses the lower value on ties via
strict `>`, and retains the center sample unless a value occurs more than twice.
P input is filtered as palette indices and the wrapper retains its palette and
alpha table. The refactored caller now routes through the injected CPU
architecture and records the prepared selection. Mode, palette, and fallback
parity remain pending; this does not close the broader Filter source review.

The shared BoxBlur/BoxBlurXY/GaussianBlur source comparison is complete. The
exact-mode support and Default/vector routes, specialized radius-one/radius-
seven kernels, identity proofs, architecture guards, and same-mode fallback
are crosswalked against the archived CPU/SIMD adapters in the inventory.
Output, selection/fallback receipts, and latency remain pending the frozen CPU
matrix; the active source owners carry those TODOs.
The source comparison found that the maintained vector blur's zero-radius and
small-uniform exits copied with `to_vec()` without publishing the archived
`native-copy` path or its vector/tail counts. They now use one checked,
mode-preserving vector-copy helper; RGBA's radius-seven identity proof uses
the same helper after recording its proof scan. This receipt correction is
source-reviewed but not runtime-validated.

RGBA GaussianBlur's radius-seven specialization has now been source-compared
with the preserved CPU and SIMD routes from `3f9dfd403`. Default keeps Pillow's
three horizontal passes followed by three vertical passes, Q24 weighting, and
replicated-edge windows. The AVX2 middle-block kernel keeps the archived
eight-pixel prefix recurrence and uses transpose-based vertical passes; NEON
keeps the same recurrence and uses row-major vertical blocks at 64x64 or larger,
with transpose-based passes below that threshold. Architecture selection now
admits those exact kernels only for finite, nonzero sigma whose derived radius
is seven with nonzero fractional weight; other declared architectures use the
native-byte fallback, and unsupported host/build architectures fall back via
the prepared selection. The large-uniform identity proof remains before the
specialized kernel. No source-level algorithm discrepancy was found. Focused
RGBA GaussianBlur output and selection-receipt parity remain pending. The
separate filter-wrapper name/mode/error-order and private ImagingCore review
remain open at the host boundary.

The L radius-one BoxBlur kernel has now been compared with the archived L SIMD
kernel: edge replication, the exact reciprocal horizontal rounding, Q24
vertical rounding, three-row buffering, and vector-block receipts match. The
L GaussianBlur radius-one kernel also retains the archived three rounded
horizontal and vertical passes, f32-derived box weights, edge swizzles, and
pass receipts. A missing RGBA radius-one strategy was found in the archived
adapter comparison: BoxBlur, BoxBlurXY, and GaussianBlur had used direct RGBA
vector passes whenever both axis windows truncated to radius one. That exact
mode strategy now lives in `rgba8/helpers/radius_one_blur.rs`; it preserves
horizontal/vertical pass order, scalar borders, vector blocks, scalar tails,
and the small-uniform native-copy route. Its output and selection receipts
remain pending CPU parity. The LA GaussianBlur radius-one strategy also matches
the archived `[L, A]` horizontal and vertical fixed-point passes, including the
three byte-rounding boundaries and direct full-vector loads. Generic native-
byte blur and exact-mode admission/fallback are source-crosswalked in the
inventory; their output and selected/fallback receipts remain parity work.

The L rank-window Default and architecture paths have also been compared with
the archived implementations. Default keeps clamped edge coordinates, the
3x3 and min/max special cases, small-window selection, and the larger-window
histogram route. The architecture path keeps 16-lane gathering, the dedicated
second-minimum and median-of-nine 3x3 selectors, sorting through 5x5, and
byte-domain rank selection through 15x15. Exact-mode L output and prepared
architecture admission match by source inspection. The archive records a
measured Apple ARM64 regression for 9x9 RankFilter(rank=40) versus serial CPU;
retain it as a performance TODO for a later algorithm-focused change. Output,
edge, and fallback-receipt parity remain pending.

The native-byte Filter3x3 Default and architecture paths have now been
source-compared with the archived CPU and SIMD kernels. Default preserves
Pillow's bottom-to-top coefficient rows, f32 normalization and accumulation,
rounding bias, clipping, and untouched border bytes. The architecture helper
keeps the same eight-lane arithmetic order and per-mode native channels; its
RGB byte stream and LA binomial specialization retain their archived layouts
and rounding. Mode leaves reconstruct their exact raster variants. This byte
Filter3x3 comparison does not include the typed I32 kernel or Filter5x5 routes,
which are reviewed below. Byte-mode output and architecture-receipt parity
remain pending.

The typed I32 Filter5x5 Default, AVX/FMA, and AVX2/FMA routes have now been
source-compared with the archived CPU/SIMD implementations. Default retains
the exact ImageI32 carrier, scalar f32 row order, copied borders, and guarded
bounded-binomial path. AVX/FMA reuses that scalar ring only for the exact
normalized binomial request. AVX2/FMA retains the eight-lane arithmetic,
gathered final block, and copied-border behavior. The comparison found that
the maintained AVX2 leaf had lost the archived uniform-identity shortcut and
minimum 5x5 admission; both are restored through its identity proof and
strategy guard. I32 Filter5x5 output and selected/fallback receipts remain
pending in the frozen parity matrix; the byte-mode Filter5x5 review is recorded
below. The AVX/FMA route is admitted only by the prepared mode-support selector:
its build declaration is x86/x86_64-gated, `ExecutionContext` requires detected
AVX and FMA support, and its guard accepts only the exact normalized binomial
kernel with four-byte pixels and nonempty dimensions. Shared execution first
checks its uniform-identity and bounded-binomial shortcuts, then uses the
scalar FMA ring for the remaining eligible input. An `Avx2Fma` request may use
this declared lower `AvxFma` strategy when the AVX2 route is unavailable and
the binomial guard is satisfied; if no eligible declared architecture route
survives admission, the same-mode Default remains selected. This dispatch
trace is source-reviewed; parity and fallback receipts remain pending.

The typed I32 Filter3x3 Default and AVX2/FMA routes have also been
source-compared with the archived implementations. Default preserves the
scalar f32 row order and exact cross-smooth specialization. AVX2/FMA preserves
the four-lane arithmetic order, scalar tail, and copied border; its support
guard requires exact four-byte storage, finite nonzero scale, finite kernel
coefficients, and nonempty dimensions. It does not require a complete
four-lane interior block: for a nonempty image whose width or height is at
most two, the prepared executor copies the native identity; otherwise the
kernel may run only its scalar tail when the interior is narrower than four
pixels. A prepared uniform-image proof uses the matching Default or AVX2 scan
before execution, and a successful proof can return the original shared image
buffer. Output parity remains pending. Freeze architecture, no-vector-block,
identity-shortcut, and fallback receipts, then compare latency for the small
shapes before deciding whether the current AVX2 admission guard should remain.

The remaining native-byte Filter5x5 Default and architecture routes have now
been source-compared with the archived implementations. The shared vector
kernel retains the eight-lane channel or complete-pixel layouts, accumulation
order, clipping, and copied two-pixel border. Exact mode leaves reconstruct
their original variants; the bounded uniform identity shortcut keeps its
archived admission range. L retains its uniform and binomial kernels, LA its
single-outlier and binomial kernels including the NEON leaf, and RGB its
large-uniform scan and BLUR-ring kernel. Unsupported kernel parameters and
shapes resolve through Default. Byte output and selected/fallback receipts are
still pending CPU parity.

The native-byte MaxFilter/MinFilter architecture paths have now been compared
with the archived SIMD adapter. Both retain exact byte-layout admission,
positive odd windows, clamped horizontal/vertical 16-lane extrema passes,
bounded uniform/all-zero identity copies, and exact-mode output reconstruction.
The maintained prepared guard matches the archived nonempty shape contract.
The native-byte MedianFilter/RankFilter architecture path has also been
compared: the 15x15 bound, rank-range checks, 16-lane gathering, LA
interleaved selector, RGB 3x3 median network, sort-through-5x5 path, and
byte-domain selection through 15x15 are retained. The RGB gather again shares
one clamped-coordinate walk across its three columns. L remains owned by its
separate exact-mode rank strategy. Output and architecture/fallback receipt
parity remain pending; the archived Apple ARM64 9x9 rank-40 regression remains
a later algorithm-focused performance item.

ModeFilter has been compared with Pillow 12.2's `ModeFilter.c`. The exact
one-band byte strategies use the same clipped neighborhood, ascending-value
tie rule, `count > 2` threshold, and original-center fallback; the public
wrapper's even-size normalization preserves Pillow's integer-halved radius,
and the P wrapper preserves its palette and alpha metadata. Multiband byte
strategies remain explicit `NO_PILLOW_SUPPORT` Rust extensions. The mode,
palette, public argument-order, and selected/fallback receipt cases remain
pending CPU parity.

UnsharpMask's source comparison against `3f9dfd403` now covers the raw-byte
blend, signed truncation at 150%, RGB-only architecture specialization, and
GaussianBlur composition. Its backend/CPU-architecture route is resolved
before pixel materialization, matching the archived caller order, while the
materialized `DynamicImage` remains the strategy mode authority. The blend and
nested blur are prepared with the same injected context. Runtime mode,
architecture, receipt, and error-order parity remains pending.

ImageOps.expand now carries signed left/top/right/bottom margins through the
typed pipeline and uses native exact-mode Default kernels; SIMD admits only
symmetric nonnegative margins. P-mode tuple, valid RGB(A) list, and named fills
resolve through a palette copy and attach the resulting palette metadata to
the output. The
Python adapter now preserves built-in mixed and arbitrary-width numeric tuple
components as typed values for P-mode Expand, using the same exact integer
sign/magnitude conversion as Draw. The P resolver compares those values before
byte conversion, retaining palette allocation and conversion-error order for
three- and four-component tuples and integer lists; oversized integer lists are
retained in their own variant so non-P list errors are unchanged. Remaining
custom numeric coercion and mixed/wide tuple/list parity are pending. The Python adapter normalizes border
input before realizing pending dirty P/PA
palette metadata for Rust. Automatic GPU routing can fall through to CPU for
unsupported shapes, while explicit GPU requests remain strict. Source parity
remains pending for margin forms, negative cropping, malformed/raw palette
behavior, allocation metadata, output-size overflow, and backend fallback. No
checks have run for this source continuation.

Equalize native-histogram fast-path source audit (2026-11-07): the shortcut
now requires each captured `len`, `range`, `sum`, and `list` object to have its
expected built-in type and builtins module/name, as well as matching the
wrapper's import-time snapshot. This prevents ordinary replacements installed
before the wrapper module import from being mistaken for the original
builtins. Builtin replacement is outside the supported compatibility contract;
the guard's behavior under replacements is not a required parity target. No
build or parity ran after this correction.

Reduce straight-alpha source audit (2026-11-07): Pillow 12.2 converts LA and
RGBA to La and RGBa, respectively, reduces that premultiplied image, then
converts back. The exact-mode Rust reducer fuses the same premultiply, block
average, and unpremultiply steps for straight-alpha LA/RGBA, while La/RGBa and
non-alpha layouts average their stored samples directly. No intermediate mode
conversion is needed. Verify zero- and low-alpha rounding in CPU parity after
source closure. Source: [Pillow 12.2 `Image.reduce`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2262-L2269).

### Direct analysis and adjacent ImageOps migration matrix

This matrix tracks source migration only. Runtime parity and benchmark columns
remain pending for every row; `Architecture` coverage means `Default` plus all
12 non-Default requests (`Aarch64`, `X86_64`, `Neon`, `WasmSimd128`, `Sse2`,
`Sse3`, `Ssse3`, `Avx`, `AvxFma`, `Avx2`, `Avx2Fma`, and `Avx512F`), with
same-mode `Default` fallback where no exact kernel exists. The configured CPU
selector list now includes each of these requests; unsupported host/build
combinations must prove the declared fallback and reason.

| Operation / public route | Mode contract and CPU owner | Native sample behavior | Context / caller state | Pending |
| --- | --- | --- | --- | --- |
| Formatted GetExtrema | `OperationId`, exhaustive 25-identity classification, and exact-mode Default owners exist; 22 identities are accepted and three I;16 endian identities retain the Pillow rejection. | Byte, I32, F32, I16, and internal u16/f32 multiband strategies reduce native samples; internal results are marked `NO_PILLOW_SUPPORT`. | Prepared CPU selection reaches the formatted Rust and host formatters. | CPU mode × architecture parity and receipts. |
| Byte-valued `Image::getextrema()` | `GetExtremaBytes` has exhaustive exact-mode support, injected context, and Default owners for all 25 identities; every typed scan is bounded to the declared width × height. | Direct native-carrier scans preserve the byte API result contract without an RGBA raster; typed samples are projected only at the result boundary. | The operation receipt list preserves lazy source graph work and terminal selection; summary timing/resources cover only the CPU scan; source graph paths and selections stay in ordered operation receipts. | Freeze operation × mode × architecture parity for the exact byte-result contract and fallback receipts. |
| GetBBox | Exhaustive 25-identity contract and one Default owner per exact mode exist. | Native byte, I/F raw-word, I;16 u16, and internal high-depth scans. | The operation receipt list preserves lazy source graph work and terminal selection; summary timing/resources cover only the CPU scan; source graph paths and selections stay in ordered operation receipts. | CPU mode × architecture parity, terminal selection receipts, alpha-only and high-byte guards. |
| GetProjection | Exhaustive exact-mode support and prepared CPU strategies exist; RGB may use its declared GPU route. | Each Default/architecture strategy scans the native carrier. | Direct and materialized-pipeline routes now carry one Backend/Architecture policy snapshot; unsupported GPU routes use the selected CPU architecture, including active SIMD fallback. | Verify direct, GPU-fallback, and lazy terminal receipts plus operation × mode × architecture parity. |
| Histogram | `OperationId`, exhaustive 25-mode support, and declared 1/L mask rules exist; all 25 exact-mode Default owners dispatch from the prepared CPU selection. All owners scan only the width × height logical pixels from native typed samples. For masked LA/La/PA, Pillow Histo.c reads physical lane 1 into the second bank; the typed owner reads retained lane 1 and falls back to the canonical repeated first sample when provenance is absent. New/frombytes/decode construction, Merge interleaving, packed PutData, same-pixel-size mode-mismatched Paste, LA AlphaComposite, and Arrow `fromarrow` retain or update physical lanes in HiddenPixelLanes. The new Arrow importer is source-reviewed but has no frozen parity coverage yet; exact-mode producer provenance is still a runtime check. Other operation producers still need an audit for dropped sidecar lanes before Histogram; parity remains pending. The same mode classifier exposes Histogram result bands and bins; RGB32F declares four bands to preserve its synthesized opaque compatibility band. The `HistogramExtrema` input contract names only I/F; Python parses it only for unmasked I/F calls, whose native scan uses Pillow-compatible float32 scaling, truncating bin conversion, and omission of out-of-range bins. I's full-range signed overflow and F's non-finite/out-of-i32-range float-to-int cases are Rust-defined extensions marked `NO_PILLOW_SUPPORT`; their extreme boundaries remain frozen parity TODOs. | Pillow I/F and I;16 retain their special sample rules. CoreOnly modes scan native samples directly into 256 bins per native band: u16 computes `(sample + 128) / 257`; f32 computes `round(clamp(sample, 0, 1) * 255)`, with NaN mapped to the upper endpoint. These calculations produce bin indices directly from each u16/f32 sample; neither samples nor images are converted or staged. | Direct wrapper injects routed CPU/architecture context and records materialized-call selection; Entropy composes this prepared child under the same context. | Complete operation × mode × architecture parity and terminal selection receipts; separately verify CoreOnly extension thresholds (u16 bin edges and f32 clamp, NaN, and rounding boundaries) without treating them as Pillow modes. |
| Entropy | `OperationId`, exhaustive 25-mode support, and 1/L mask rules exist; the prepared Entropy owner composes Histogram. Its pre-existing optional extrema parameter remains a `NO_PILLOW_SUPPORT` Rust extension for unmasked I/F samples. | All mode semantics come from the matching exact-mode Histogram owner, including native u16/f32 bins for CoreOnly modes. | Injected CPU context selects Entropy and its Histogram child. Its operation receipt list preserves source graph, optional mask graph in validation order, and terminal selection; timing/resource summaries cover only terminal Entropy. | Complete operation × mode × architecture parity and terminal pipeline receipts. |
| GetColors | Exhaustive 25-mode `OperationId`/support contract; one Default owner per accepted exact mode. I;16* support rejection follows materialization and signed C-int/table-limit parsing to preserve Pillow's public order. | Public Python `Image.getcolors` passes whether its visible mode selects the `1`/`L`/`P` histogram route. Core uses exact-mode GetColors when raster identity agrees and Histogram's first 256 bins when it does not; PyO3 compares `len(result) > maxcolors` against the original object. Other visible modes use the core low-level GetColors route after native signed C-int-style parsing; core owns table-limit and wrong-mode error ordering. Typed Rust GetColors retains per-mode counting, including native `1`/`L`/`P`; byte multiband modes count raw channels in Pillow slot order; I/F retain scalar hash behavior; CoreOnly u16/f32 modes count native samples without conversion and return typed tuples. | Direct Rust GetColors preserves lazy source graph receipts and terminal selection. Core's route method uses GetColors selection for matching raster identities and Histogram selection for a visible-mode override; terminal summary timing/resources cover only that scan; source graph paths and selections stay in ordered receipts. | Freeze CPU-only GetColors parity for all 25 modes × declared Architecture requests, including output shape/order, special-mode rich-comparison callbacks, other-mode C-int coercion, rejection, and fallback receipts. |
| Stat | Exhaustive 25-mode `OperationId`/support contract and one Default owner per exact mode; non-scalar modes compose the Histogram child. | I/F preserve their specialized scalar histogram formulas; byte, I;16, and CoreOnly modes reduce prepared 256-bin Histogram output, with CoreOnly bins scanned from native samples and no converted image. I full-range overflow and F undefined float-to-integer cases use explicit Rust behavior and are marked `NO_PILLOW_SUPPORT`; their boundaries remain unverified. | Direct and masked wrappers inject routed CPU/Architecture context; the masked path executes its Histogram child through the selected exact-mode Stat owner using direct u32-bin reduction that preserves the public f64 arithmetic without a widened histogram allocation. Both preserve source and optional mask graph operation receipts before the terminal selection; terminal timing/resource summaries cover only the Stat operation; source and mask graph paths and selections stay in ordered operation receipts. | Freeze all 25 modes × declared Architecture parity, masked error order, and terminal selection receipts, including scalar I/F extension boundaries. |
| Split | Exhaustive 25-mode `OperationId` contract composes the exact-mode `ExtractBand` child. | P preserves its indexed palette; single-band modes preserve exact identity; multiband children extract native bands, including CoreOnly u16/f32 samples. | The wrapper carries a mode-validated lazy graph; each child selects backend and architecture when materialized. | Verify palette and output metadata, child selection/fallback receipts, and all exact-mode outcomes in CPU parity. |
| ImageOps / Quantize | `OperationId` and exhaustive 25-mode source contract select the fixed P output under injected CPU/Architecture context; all modes have Default, with no architecture kernels declared. | L, P, RGB8, RGBA8, and all 21 CoreOnly modes use exact-mode Default owners under their mode folders. The operation contract lists method-restricted alpha modes and their allowed methods; LA/La/PA, RGBA, RGBa, LA16, RGBA16, and RGBA32F select alpha-aware Default sources, while RGBX uses RGB-only quantization and ignores padding. The facade preserves Pillow's visible-mode RGBA method default and rejection before palette callbacks. When wrapper `mode` differs from DynamicImage, the resolved method default and visible admission identity are passed separately; source strategy dispatch remains keyed by the actual DynamicImage mode. The support guard checks the declared visible source set, actual carrier set, and supplied palette entry count independently, including the `empty palette` rejection. Each CoreOnly owner reads its native storage through a statically dispatched sample source, without constructing a converted DynamicImage or RGB/RGBA staging raster. LA/La/PA and typed alpha modes read native color/alpha samples through an alpha-aware octree source; PA resolves palette colors and combines per-pixel/palette alpha without a staged image. RGBa unpremultiplies each native sample before alpha-aware octree reduction; RGBX uses the native RGB-only quantizer and ignores padding; I;16 keeps its clipping rule; internal u16/f32 modes use the raster FromPrimitive mapping fused into sample reads. Every CoreOnly owner is marked `NO_PILLOW_SUPPORT`. Shared palette algorithms live under `pool_cpu/helpers/quantization`; MaxCoverage and median-cut k-means consume native sources without staging converted pixel lists. | Direct wrapper prepares from the materialized DynamicImage mode, preserves requested Backend, and records exact-mode selection. Lazy source graph receipts are restored before the terminal Quantize selection; terminal summary timing/resources cover Quantize alone; source graph paths and selections stay in ordered operation receipts. Palette-image, empty-palette, method-restricted-mode, and allowed-method constraints are in the operation input contract. | Source review compared the moved palette reducers with the pre-refactor files: median cut, MaxCoverage, octree, and fixed-palette mapping preserve their existing algorithm routes while consuming statically dispatched sample sources; MaxCoverage retains its single-color palette behavior. The exact-mode Default routes and RGB/RGBA palette output projections are source-reviewed. The Python facade resolves palette truthiness, calls truthy palettes' `load()` and `mode` callbacks in Pillow order, and parses `dither` only on the supplied-palette path. The PyO3 adapter accepts this binding's typed carrier directly and reads palette mode/data from a foreign ImagingCore-compatible carrier into a Rust-owned P metadata carrier; image quantization remains in the exact-mode Rust strategy. Falsey palettes use ordinary quantization; non-image palette carriers reject explicitly. Missing palette pointers now use a generated grayscale/WEB table selected from the actual DynamicImage identity; attached empty tables retain the declared `empty palette` rejection. Their output/fault and fallback-receipt parity remains pending. Freeze CPU mode × architecture parity, selected/fallback receipts, palette alpha and empty-image cases, including wrapper/source-mode disagreement and CoreOnly extensions separately. Runtime verification remains pending. |

Parallel isolation: the maintained shared `ops/pil_resize` compatibility path
now uses serial row loops, the RGB tiled transpose, and the serial typed kernels
regardless of Cargo features. Rayon calls and threshold plumbing were removed
from this path. Rehome the parallel scheduler into the later `pool_parallel`
owner with injected execution context; the separate backend remains pending.
The CMYK helpers in `color/grayscale` and `color/conversion` also remain serial,
and `ops/convert/nonstandard.rs` no longer changes eager receipt behavior based
on the global `parallel` feature. Resolve remaining eager receipts from injected
Backend/Architecture context.

Direct-operation gap: Histogram, Entropy, GetColors, Stat, Split, and Quantize
now expose operation descriptors. Histogram scans use exact-mode owners,
Entropy composes that prepared child, GetColors has native exact-mode counting
owners, and Split declares its `ExtractBand` child. Stat has exact-mode owners
and native high-depth histogram composition; its masked wrapper prepares the
Histogram child under Stat's routed context, executes it through the exact-mode
Stat owner, and reduces native u32 bins without a widened histogram allocation.
It records Stat selection for materialized inputs. Quantize has an exhaustive
source-mode contract, a fixed P output, injected context, shared palette
kernels under `pool_cpu`, and ordered source-graph receipts restored before
the terminal selection. Parity, receipt verification, and remaining caller/error-
order review remain open in the matrix; terminal timing/resource scope is defined
above, so source ownership alone does not close rows.

GetColors load/error ordering (2026-11-06): direct and signed-limit entry
points now materialize the image before consulting `GetColors` support, using
the materialized `DynamicImage::mode()`. Pillow's public method calls `load()`
before choosing its histogram or low-level counting route, so decoder and
pipeline faults precede mode rejection even when a negative limit will return
`None`. Output and error-order parity remain pending in the frozen CPU matrix.

GetColors public argument handling (2026-11-06): after load, the Python
facade checks its visible `mode`. For visible `1`, `L`, and `P`, the binding
uses the exact-mode GetColors owner when the raster identity agrees, otherwise
it composes the native Histogram route; it compares result length to the
original Python object. Other visible modes use GetColors and native signed
C-int-style extraction. If the visible mode selects low-level GetColors for
an underlying `1`, `L`, or `P` raster, the binding parses the signed C int,
preserves Pillow's no-table-size MemoryError boundary, then reports the
low-level wrong-mode error. Rejected I;16* modes follow the same table-limit
error order before their declared mode rejection. Rich-comparison callbacks,
coercion faults, and output parity remain pending in the frozen CPU matrix.
Rich-comparison callbacks, coercion faults, and output parity remain pending
in the frozen CPU matrix.

### Cross-cutting source work and evidence gates

| Workstream | Status | Pending work / exit condition |
| --- | --- | --- |
| Mode ownership and boundaries | CPU source review complete; parity pending | `DynamicImage` and `Mode` declare the same 25 exact identities; CPU construction and operation APIs use typed `Mode`, while materialized execution reads the `DynamicImage` variant. Decode/encode translation stays in Pillow-owned codec adapters, lazy output transitions use operation support declarations, and indexed palette state retains the exact P/PA raster identity. Python and JavaScript parse public raster-mode strings at binding boundaries. `ImageColor.getcolor` uses a distinct typed `ColorOutputMode` for Pillow `ImageMode` descriptors such as `I;32` and `I;16S`; these values are not coerced to raster `Mode`. The later GPU compatibility path still carries Pillow spellings in its pending execution metadata and remains a separate backend-stage TODO. CPU source review is complete; frozen CPU parity and receipt verification remain pending. |
| Backend and Architecture context | CPU routing source review complete; owner audit and runtime receipts pending | The shared selector checks the requested Architecture before permitted lower-architecture declarations, preserves `MissingSpecialization` for lower implementations, and carries the selected guard in `StrategySelection` for execution. CPU-only direct Analysis, Quantize, point/eval, eager Filter, Draw, GetData, GetProjection, callback PutData/PutPixel, and the CPU-owned binary/matrix/I;16/palette Convert routes now use the shared image-pinned/active Backend plus CPU Architecture resolver (advisory where the old wrapper swallowed backend-state errors); GPU preference can therefore retain an active SIMD architecture for CPU fallback, while an explicit GPU pin stays CPU Default. Eager receipts treat SIMD as an architecture request: a selected SIMD strategy clears the provisional host-fallback note, while Default/lower-architecture selection records the strategy fallback; GPU-to-CPU stays a backend fallback. Per-operation telemetry now exposes optional typed strategy fallback reasons separately from pipeline-level backend fallback (`None` means selection is unavailable; an empty list means a recorded selection had no fallback), and prepared CPU batch, direct eager, composed, fused, and vector PutPixel batch selection paths record the actual `StrategySelection`. The owned batch Invert path keeps its exact byte-mode eligibility and derives its kernel band count from `Mode::band_count()`. Fused RGB/CMYK GetProjection terminal execution and receipts now consume the prepared selection directly; RGB PutPixel→Thumbnail fusion admission also passes the single Image-resolved Backend/Architecture snapshot into its support guard. Source audit (2026-11-05): CPU direct terminal pixel kernels pass the routed `ExecutionContext` into prepared selection; lazy operations select through registry preparation; identity/copy routes are declared mode paths with no pixel kernel; empty DrawText exits before selection. Static scans found no active `pool_cpu` imports of `legacy` or `pool_simd`, no Rayon use, and no production `ExecutionContext::new` or `Architecture::detected_host` inside CPU strategy code (host context resolution is centralized in `for_routed_backend`; explicit constructors are in tests). The remaining context-source TODO is GPU GetData terminal eligibility, deferred to the GPU stage. Audit owner declarations and verify exact, lower-architecture, build/hardware-unavailable, and guard-fallback receipts in parity. |
| Operation support declarations | Declaration model and caller source audit complete; runtime evidence pending | Each operation exposes a `ModeSupport` outcome for all 25 exact modes, including input/output rules, Default strategy, architecture/build requirements, request guards, and rejection or fallback reasons. `OperationId::support()` maps all 113 identities to their family-owned declaration; `OperationSupport::modes/routes()` enumerate `Mode::ALL` through the same classifier. `PipelineOp::operation_id()` maps 75 identities and `PipelineOp::support()` delegates to that declaration; prepared plans expose the contract for their selected operation. `BoxBlur` and `BoxBlurXY` retain distinct identities and inputs while sharing an executor entry. `Equalize` and `EqualizeMasked` intentionally share one identity with optional-mask facts. Pad and Expand typed/positioned variants share semantic operation declarations and exact-mode fill rules. The source audit enumerated all 28 direct/eager identities and traced their support owners and caller use; kernel-backed paths select declared exact-mode strategies, while identity, copy, wrapper, and composed paths document their selection boundaries. `registry::cpu_supports()` remains a coarse executor-registration query, followed by exact mode assessment during preparation. Convert's source/target declarations include direct LAB rows for every mode, including CoreOnly `La` and internal modes; each streams into only the final LAB raster. This closes source caller-to-declaration reconciliation only; output, fault order, callbacks, route receipts, CPU parity, and performance remain pending. |
| Archive, dead code, and source size | Archive integrity, family map, and declaration/caller disposition complete; compiler reachability, dead-code, and size proof pending | All 98 archived CPU files and 182 archived SIMD files match baseline `3f9dfd403`; all 21 baseline operation-family exports have maintained CPU counterparts, with `analysis` added. The 87 current `PipelineOp` variants map to 75 operation identities, and Section 8 maps all 313 CPU and 569 SIMD declarations to owners or intentional retirement. Active-source scans find no legacy imports, Rayon scheduling, or implementation bodies in maintained CPU `mod.rs` files. Current direct family roots contain only `mod.rs` and `ops.rs`; the largest active CPU files are Composite `ops.rs` (976 lines), ImageOps `ops.rs` (953), Enhance `ops.rs` (896), `batch_execution.rs` (866), Put `ops.rs` (853), Filter `ops.rs` (845), and Analysis `ops.rs` (812). The Pad mode-owner move reduced Geometry `ops.rs` to 767 lines. A fresh 2026-11-23 first-party Rust physical-line scan finds no maintained file above 1,000 lines; `pillow-rs/src/lib.rs` is exactly 1,000, and 15 modified Rust files are in the 800–1,000-line review band. This does not prove the Clippy-counted function-size limit. Finish compiler-backed target/module reachability and identify any dead maintained routes; then prove the 100-line function limit and repeat the file-size audit after Stage 5 source closure. Historical quality passes do not cover current WIP. |
| Static quality and compilation | The current 2026-11-12 WIP passed `make build-parity`, `make fmt clippy`, `make docs-lint`, and `make quality`; the generated repository map was refreshed and verified. Clippy exited successfully with the repository’s existing warning diagnostics. The broad CPU gate remains pending until Stage 5 closes and the remaining source edits are complete. AArch64 and x86_64 Apple target checks are historical. The current crosswalk maps 217 public operations, 174 native Python calls, 87 pipeline variants, and all counted direct PyO3 routes; four GPU shader candidates remain for GPU review. | Rerun the final static/compile gate after Stage 5 closure. Keep the GPU asset candidates for later review and retain the unavailable native x86 execution limitation. No unit-test suites for this migration. |
| Full CPU parity | The latest full 25,021-case Pillow 13.0 CPU/default run at `147add214` passed 24,916 and failed 105, including all 231 recorded outputs; infrastructure errors were zero. It predates focused GetBBox, Histogram, and Convert fixes, so its per-operation counts are stale for those groups and no current aggregate count is claimed. Since that run, typed-u16 GetBBox passed 8/8, masked-LA Histogram passed 9/9, and the Convert correction plus matrix fault contract passed 17/17 under Default and 17/17 under requested NEON (same frozen IDs, Pillow 13.0, zero infrastructure errors). The NEON receipts show native vector execution for I;16N→I/F/L; I→I;16N still selects Default due MissingSpecialization. Full corpus report and focused artifacts are in `build/migration-parity/`. | Fix all remaining mismatches; finish operation × mode cases, secondary-mode relations, target-only outputs, boundaries, and error ordering. Then run all CPU architecture selectors; keep the codec failure tracked externally. No CPU fault contract without a distinct deterministic public outcome. |
| CPU performance | Not started; parity gate remains open. Workload IDs are reconciled, but compatible baseline timing evidence is not. | The `3f9dfd403` input set has 1,032 IDs (SHA-256 `639e358082276c924343618d5a3b09c6945de012c1a80ac5a8ee2121ccb84`); the current set has 1,033 (SHA-256 `e134adf04711dfd5bfdd638ef4ca3c580b725b6f0832aff591dc15e07c5000b1`). There are 1,031 unchanged shared definitions; Pillow 13 removed `pil-image-image.get-child-images.standard`, and two ImageText workloads were added. Collect compatible baseline timings after parity passes; then report each operation/mode/architecture separately, with at most one confirmation repeat for questionable pairs. |
| Parallel CPU and later backends | Waiting on CPU completion | Start `pool_parallel` only after CPU source, quality, parity, and performance gates pass. Keep scheduling and evidence separate; GPU work follows later. GPU decision (2026-11-24): GPU host-semantic and preflight CPU fallbacks use the CPU Architecture from injected context when available; verify the selected/fallback receipts during the GPU stage. |

Prepared batch CPU source review is complete; output and receipt parity remains
pending. `execute_batch` validates the injected CPU context, advances through
`execute_batch_step`, and publishes the final resource receipt. Generic registry
preparation retains the exact current `DynamicImage` mode and passes palette
metadata only while the raster is P/PA. Fused Draw runs use the family batch
contract; AlphaComposite+Mirror checks zero offsets, RGBA inputs, matching
dimensions, and common selected architecture; Multiply+Screen requires the same
operand source, matching native layouts, and one architecture selected by both
operation contracts. Point LUT fusion is limited to L/LA/RGB/RGBA and sends
non-identity maps through the prepared Eval strategy; identity maps perform the
required copy without claiming a selected kernel. Transpose runs preserve the
packed-storage identity guard and use the prepared composed strategy otherwise.
PutPixel batches use their exact-mode batch contract, while the RGB
PutPixel+Thumbnail route binds sparse writes to the declared Resize or
ResizeBoxed child. Owned Invert and Chops Add/Sub retain the selected mode and
architecture, using in-place execution only after the strategy's guard admits
it and the prepared allocating route otherwise. The archived batch behavior and
these current caller/guard paths have been compared by source inspection. No
build or runtime verification has been run.

Mode-string boundary source audit (2026-11-01): active `pool_cpu` operation,
helper, and batch APIs pass typed `Mode` and `DynamicImage`; they do not accept
mode strings or call `Mode::from_pillow_name()` to choose an implementation.
Remaining `pillow_name()` calls format errors or diagnostics. The only
`from_pillow_name()` occurrence in `pool_cpu` is a test helper. GPU compatibility
metadata remains a separately tracked backend-stage TODO.

Current Chops working-tree changes are source-only and unvalidated. They retain
the exact-byte binary guards and fusion facts and selected architecture
telemetry. Binary Chops treats LAB as an ordinary Pillow-compatible three-band
mode: RGB, YCbCr, LAB, and HSV operands use their native byte lanes, and the
primary mode determines the output identity. The typed three-band helper
accepts those secondary variants without image conversion. Matching same-width
La and LAB secondary layouts are eligible for the primary mode's declared
architecture strategy where one exists. Every Chops mode's Default Offset owner now selects its exact
`DynamicImage` variant and calls a typed buffer helper; the generic
`DynamicImage::offset_with_mode` route was removed from raster transforms.
Accelerated byte and I;16 Offset owners pass typed buffers to shared kernels,
which return the same buffer type without mode matching or output reconstruction.
The checked-u32 guards, endian rules, palette retention, context flow,
fallbacks, and selection/path receipts for Offset have been source-reviewed.
The I;16 architecture guard requires eight samples per row and selects
same-mode Default for narrower non-empty rows. Source review has
compared the shared scalar and vector Add/Sub, Multiply/Screen, bytewise, LUT,
and logical formulas to the archived CPU/SIMD code; no arithmetic discrepancy
was found. The typed direct wrapper preserves the baseline channel-layout
groups. LAB uses Pillow parity rather than a CoreOnly classification, and its
former chroma-bias adjustment was removed because it changed the raw-byte
result. Prepared
Multiply→Screen retains its archived exact modes and same-architecture
requirement.
The public Rust and Python caller paths, typed validation order, operation
registry, lazy output-mode planning, palette retention, compatibility routing,
owned Add/Sub, in-place Add/Sub, Invert, and Multiply→Screen fallback paths are
now source-reviewed against the baseline and preserved CPU/SIMD callers. The
review found no formula or error-order discrepancy. This source review does
not establish runtime parity; all 19 Chops operations remain in the frozen CPU
parity and receipt matrix. The shared binary and vector kernels consume typed buffers for
both operands, and each exact-primary-mode owner accepts secondary variants
only from the declared native channel-layout group before dispatch. The
in-place Add/Sub route uses the same typed boundary and preserves those
cross-mode pairs. Multiply→Screen
fusion's shared arithmetic also consumes typed buffers; its exact-mode owners
wrap the result. Default Invert has one typed owner for each of the 25 modes,
and its shared sample helper complements u8/u16 samples or the stored f32 bit
pattern without selecting or reconstructing a mode. Constant and Duplicate
remain mode-neutral shared Default behavior.

## Stage 0: freeze the inventory and execution contract

Perform source inspection only; do not run builds or validation campaigns.

1. Confirm the active checkout, current Git status, baseline, user changes,
   attached worktrees, and recovery refs. Preserve unrelated work and refs.
2. Use Kata to maintain `pillow-rs#3ptb` and record the revised scope. Earlier
   evidence from the abandoned implementation does not verify the new source.
3. Inventory the baseline CPU tree: 98 tracked files and its 84 `PipelineOp`
   variants. The current enum has 87 variants, including `ExpandTyped`,
   `PadPositioned`, and `PadTyped`; review those additions alongside the
   baseline crosswalk. Record every
   source-to-target move, exported function, caller, mode, kernel, and helper
   in `docs/CPU_REFACTOR_INVENTORY.md`; the baseline file paths, visible
   function declarations, mode candidates, cross-pool references, instruction
   sites, and verification denominator are frozen there. Confirm each family's
   exact mode outcomes and function destinations from the archived source
   before writing its maintained implementation.
   The source enum and existing public manifest remain the authorities.
4. Include direct/eager CPU entry points, constructors, decode adapters,
   materialization, lazy graphs, mutation, scalar samples, palette operations,
   and public calls that do not become a `PipelineOp`.
5. Inventory the existing SIMD kernels and their exact feature requirements.
   Include SIMD work outside `pool_simd`, such as specialized transpose or
   resize paths. Identify mixed serial/Rayon helpers and preserve both original
   implementations for their eventual owners.
6. Record source-owned image modes and storage representations, output-mode
   changes, secondary-image/mask contracts, defaults, and unsupported cases.
   Every affected mode/operation combination needs an explicit destination or
   preserved error outcome. Record fallback as a destination, not as a gap.
   For each operation, classify all 25 identities and their input combinations,
   architecture requirements, build availability, eligibility guards, and
   fallback/rejection reasons before declaring its strategy support.
7. Identify source locations requiring registry, pipeline, binding, telemetry,
   feature, Makefile, source-map, or documentation wiring changes. GPU changes
   are limited to compatibility required by shared types and CPU fallback.
8. Freeze the complete CPU-applicable parity denominator from the existing
   manifest: IDs, count, hash, oracle, source/build identity, target features,
   materialization contract, and output locations. Record platform and
   non-CPU-only contract exclusions explicitly. No sampling or surface filters.
9. Freeze existing CPU benchmark workload IDs and compatible baseline evidence.
   Do not run a baseline campaign during implementation. If suitable evidence
   is absent, record that limitation and measure the preserved baseline only
   after the CPU validation gate.

Deliverable: an exhaustive baseline source/path inventory and a fixed
verification scope, tracked with the issue. Record each family's exact
mode/function route and behavior as part of its source review before writing
that family's maintained implementation. Do not replace this inventory with
a claim that a few representative operations cover the entire backend.

## Stage 1: preserve legacy and establish the approved tree

1. Preserve the existing CPU source in its inert legacy location, preserving
   file contents and relative paths. Record baseline provenance for each file.
   Completed before rebuilding: 98 CPU files are under `pool_cpu/legacy/` and
   182 SIMD files are under `pool_cpu/legacy/pool_simd/`.
2. Prepare the maintained tree using the guide's family, mode, and architecture
   mapping. Create simple module exports and orchestration boundaries without
   importing legacy or copying the old family layout back into active code.
3. Preserve existing SIMD source in the specified legacy namespace before
   folding it into CPU. Completed; trace every retained or retired
   implementation against this archive.
4. Record the actual directory tree alongside its inventory. Confirm that mode
   and architecture levels exist and that `mod.rs` files remain simple.
5. Keep the architecture unchanged while filling in implementations. A need to
   alter the agreed design must be discussed before replacing the tree.

Deliverable: preserved reference source and the correct maintained module tree.
The temporary migration may be uncompilable; no compiler or lint runs here.

## Stage 2: make DynamicImage authoritative for mode

1. Expand the existing raster enum into distinct mode-bearing variants while
   preserving its typed pixel buffers and existing high-depth representations.
   Use the guide's exact 25 mode identities and folder mapping. Give the
   payload-free `raster::Mode` selector the same variant identities, and read
   the materialized mode directly from the `DynamicImage` variant tag.
   Both types belong to pillow-rs. Keep image-slash-star codec types and mode
   descriptors independent, following the guide's ownership boundary. Raster
   `ColorType` may describe a storage layout but must never determine `Mode`;
   translate codec color/layout metadata only inside Pillow-owned I/O adapters.
   All backends consume this same Mode type; do not create backend-local enums.
   Representability in Mode does not imply support by any particular operation.
2. Give construction, decoding, raw-byte import, and conversion explicit output
   variants. Validate required sample shape, palette, and alpha invariants at
   those boundaries while retaining existing pixel-value behavior.
3. Preserve identity through clone, crop, transpose, resize, and every other
   mode-preserving result. Audit mode-changing operations and select their
   resulting variants explicitly; do not blindly inherit the source mode.
4. Keep lazy graph mode information consistent with materialized output.
   Resolve planned transitions when the graph changes. Python parses Pillow
   strings before calling typed Rust operation APIs; bindings and explicit
   I/O adapters own string parsing/formatting. Keep core mode parameters and
   strategy declarations typed, including internal high-depth identities.
5. Reconcile the `ColorMode` compatibility alias, decoded metadata,
   `explicit_mode`, and pipeline caches. Retain genuinely distinct parameter
   roles, but remove
   duplicated image identity and channel-based inference from active execution.
6. Update binary inputs, masks, merge bands, pasted images, and fused outputs so
   their own exact modes travel with them. Preserve palette and source metadata
   through the appropriate Pillow wrapper dependencies.
7. Adapt callers and exports across core and bindings as required to compile
   against the new type. Map decoded codec metadata once in the Pillow-owned
   decode adapter and translate back at the encoding boundary. Preserve
   palette, sample, alpha, byte-order, conversion, and error behavior without
   adding Pillow execution metadata to image-slash-star types.

Deliverable: mode is determined once at admission or a mode-changing operation;
maintained CPU code does not recover it from strings or storage layout.

Committed source-only progress (`0cb132aac`, 2026-11-01): `Image::Pipeline`
now has one `PipelineModeCache` instead of a separate `explicit_mode` field and lazy cache.
The cache distinguishes a graph-declared output mode from a mode inferred
by metadata planning, so materialization keeps consuming declared execution
context while mode queries can use the planned result. This boundary has not
been compiled or validated, and the remaining Stage 2 work is still open.

Committed source-only progress (`40ef86a05`, 2026-11-01): CPU P and PA
ImageOps Invert now retain their indexed samples and attached palette metadata
through lazy materialization. Their support declarations mark these Rust-only
routes CoreOnly because the Python Pillow adapter rejects them. The change has
not been compiled or parity-validated.

Source-only CPU support progress (`2026-11-01`): every `PipelineOp` now maps
exhaustively to an `OperationId` and exposes the same `OperationSupport`
descriptor used by preparation; matrix conversions select the matrix contract.
The support and strategy types plus `ExecutionContext` and `Architecture` are
re-exported from the crate root for typed Rust inspection. CPU indexed
materialization checks the first operation's declared P/PA mode before generic
palette expansion, covering direct `Image::push_op` calls. A source-only
crosswalk matched the baseline's 84 `PipelineOp` variants with no unmapped
variants or unknown `OperationId`s. The current enum has 87 variants, and its
exhaustive `operation_id()` match covers the three added variants (`PadTyped`
and `PadPositioned` map to `Pad`; `ExpandTyped` maps to `Expand`). These
changes remain uncompiled and unvalidated; full
support-contract review, caller reconciliation, CPU source review, and all
planned validation stages remain open.

Source-only CPU Blend admission progress (`2026-11-01`): the Rust module-level
`blend` wrapper now validates its source/secondary mode pair and dimensions
through the same `BlendModule` support declaration used by prepared execution.
It checks the mode pair before querying dimensions, preserving the prior
rejection order, and the indexed materializer rejects direct P/PA requests from
that same declaration before palette expansion. The duplicated mode-pair
matrix was removed. This source change has not been compiled or validated.

Source-only CPU ImageOps admission progress (`2026-11-01`): the Autocontrast,
Equalize, Posterize, and Solarize wrappers now use their operation contracts
for exact-mode admission. Posterize/Solarize internal high-depth rejection
messages now match the existing public `LA`/`RGB`/`RGBA` aliases. Source review
confirmed the wrappers retain their prior mask-before-mode order for masked
Autocontrast/Equalize and keep P Posterize/Solarize as CoreOnly native-index
routes. Direct/fusion caller review and all validation remain open.

Source-only CPU Merge admission progress (`2026-11-01`): the direct typed Rust
`merge` wrapper now derives supported target identities, band counts, allowed
band identities, and error messages from `OperationId::Merge.support()`. It
still validates target mode and arity before reading bands, each band mode
before checking sizes, and rejects the same first failing input. Python and
JavaScript continue converting mode strings to `Mode` before calling core.
This change is uncompiled and unvalidated; composite-family caller review
remains open.

Source-only CPU Composite admission progress (`2026-11-02`): the direct typed
Rust `composite` wrapper now reads mask identities, luma-versus-alpha selection,
and dimension rejection from the `CompositeModule` input declaration used by
prepared admission. It retains the prior size-before-mask validation order,
source conversion, and destination palette handling. The change is uncompiled
and unvalidated; composite-family caller review remains open.

Source-only CPU Chops admission progress (`2026-11-02`): each direct byte-binary
and mode-1 logical wrapper now validates its mode pair through that operation's
`OperationId` support declaration. The duplicate byte-width mode classifier is
removed. Binary wrappers still resolve both identities before rejecting;
logical wrappers still reject an unsupported first mode before reading the
second. Overlap clipping and deferred execution are unchanged. This change is
uncompiled and unvalidated; source review is complete, with runtime parity and
receipt evidence pending.

Source-only Chops unary caller review (`2026-11-02`): Constant, Offset,
Duplicate, and Invert wrappers intentionally keep queuing their typed
`PipelineOp`s without eager mode reads. `Image::push_op` preserves their exact
mode transitions, and the shared registered prepare route maps each variant to
its own operation contract and prepared strategy. Python and JavaScript
delegate to these typed Rust functions. This preserves their lazy error
boundary. Chops mode-owner and architecture-kernel comparison is complete at
source level; runtime parity and receipt evidence remain pending.

## Stage 3: inject Backend and Architecture

1. Define immutable execution context and the centralized selection policy.
   CPU with Default architecture uses the serial default implementation.
2. Respect existing explicit/backend-auto-routing scenarios. Resolve architecture
   from guaranteed compilation features or the one-time portable capability
   policy described in the guide. Do not repeatedly detect it in operations.
3. Thread the context and exact `Mode` through the backend interface, registry
   function types, eager execution, lazy materialization, pipeline batches,
   admission, fusion, nested operations, and fallback calls. Audit every call
   edge, not just the top-level pool method.
4. Eliminate fresh default contexts inside helpers and context-free escape
   routes. Geometry helpers calling resize, thumbnail calling reduce/resize,
   filters calling shared kernels, and multi-image operations keep the supplied
   selection or an explicit validated fallback derived from it.
5. Replace independent SIMD registry and pool selection with CPU architecture
   selection. Adapt legacy public SIMD requests at admission if compatibility
   requires it. Keep actual backend, requested architecture, selected
   implementation, and fallback reason observable.
   Source-only status (2026-10-31): the active SIMD pool, adapter registry,
   segment planner, and pool-based admission were removed. Legacy `Backend::Simd`
   requests now select the CPU pool with the detected host architecture, and
   telemetry reports CPU as the executor. This routing change remains
   unverified until the complete CPU source review and validation stages.
6. Remove feature-dependent Rayon execution from maintained CPU paths. Enabling
   the parallel feature must not change CPU into a parallel executor.
7. Add the shared operation-support contract and registry delegation described
   in the guide. Expose the complete contract from each operation's identity
   and `PipelineOp`, including direct operations outside the graph. Separate
   build availability, request assessment, and actual prepared selection.
8. Route admission and execution through a prepared strategy decision. Inject
   validated inputs and dependencies into the plan; retain requested/selected
   backend, architecture, implementation identity, and fallback reason. Keep
   unresolved lazy guards deferred until they can actually be evaluated.

Deliverable: every maintained CPU execution route receives its selection from
the injected context, and every image supplies its exact mode.

Source-only receipt update (2026-11-01): per-operation telemetry now exposes
optional typed `strategy_fallbacks` separately from pipeline-level backend fallback (`None` means selection is unavailable; an empty list means a recorded selection had no fallback).
Prepared batch, direct eager, composed, fused, and vector PutPixel batch paths
record the actual `StrategySelection`. The fused RGB/CMYK GetProjection
terminal now also consumes its prepared selection directly and records it with
its fallback reason. Other host terminal receipts still need equivalent
operation-owned selection plumbing and retain source TODOs. The strategy
selector now continues through permitted lower architectures when the requested
architecture is unavailable on the captured host, while checking each
candidate's own architecture and
additional requirements. This dispatch correction is source-only. No checks
have run.

## Stage 4: implement every CPU family and fold SIMD

Complete every family assigned by the structure guide, including all 87
current pipeline variants and the direct CPU entry points found in Stage 0.

For each family:

1. Read the preserved implementation and all callers. Record mode-specific
   outputs, mode transitions, rejection behavior, and measured hot paths.
   Treat internal Pillow mechanics as evidence for observable cases, not as
   requirements to copy.
2. Implement each serial strategy in its mode's Default owner. Split mixed
   mode branches into their correct implementations. Arithmetic, rounding,
   clipping, border handling, and sample layout may be reorganized when the
   exact public output and fault contract remains satisfied.
3. Place vector kernels in the corresponding architecture owners. Keep their
   architecture requirements, scalar tails, dimensions, alignment, and
   precision guards accurate. Do not add empty architecture implementations.
4. Define each operation's explicit support declaration at its entry point.
   Compose its mode and architecture strategies with small exhaustive
   orchestration and concrete prepared plans. The same linked declarations
   drive support queries, eligibility, and executable selection; do not add a
   separate support table that can disagree with dispatch. Use context to
   select an eligible specialization or the same mode's Default strategy.
5. Place real shared helpers according to the guide. Reuse identical algorithms
   with direct calls and injected dependencies; preserve specialized hot loops.
   Do not duplicate kernels merely to populate folders. Do not cast, convert,
   or retag an image to another mode merely to reuse its implementation; each
   mode strategy processes its native representation. Only conversions defined
   by the operation's Pillow semantics may change mode, and the support/output
   contract must state that transition.
6. Preserve output mode, palettes, alpha conventions, and metadata. Unsupported
   modes retain their existing public exception behavior rather than silently
   converting to a supported mode. Additional core-only mode support is
   allowed when it has a native representation and explicit strategy; mark
   each implementation `NO_PILLOW_SUPPORT` and keep it outside Pillow parity.
7. Rewire every caller and remove superseded active functions and exports.
   Retire maintained `pool_simd` code only after every implementation and
   public compatibility route has an accounted-for CPU destination.
8. Update the inventory and mark source review complete. Keep arch-specific
   reachability findings explicit. Record strategy identities and guards for
   every operation independently, including siblings in the same family.
   Do not execute tests for individual families.

Deliverable: the entire CPU backend is written in the required structure.
There are no placeholders, runtime legacy dependencies, unclassified operation
paths, or hidden serial/parallel alternatives inside CPU code.

## Stage 5: finish all source work before validation

Complete a source review of the entire migration before running any compiler,
formatter, lint, test, parity, benchmark, or coverage command.

### Mode and output preflight audit (`pillow-rs#5e8f`)

Cross-operation decision from the 2026-11-23 source trace: do not add one
generic preflight orchestrator. Each operation owns its metadata preflight and
runs it at that operation's Pillow-visible point in the call sequence. A
preflight may use exact source `Mode`, typed parameters, the operation's
existing support declaration, and the injected `ExecutionContext` to resolve
output identity and a candidate strategy without decoding pixels. Shared
support declarations and strategy types remain reusable; callback, load, and
mutation ordering stays operation-specific. A preflight must not invoke Python
callbacks, cross a required load boundary, or resolve guards that depend on
pixels or palette state. This decision does not require every operation to
become lazy or eager.

Mode storage follows the implementation-structure ownership rule: a
materialized `raster::DynamicImage` carries identity in its exact enum variant
and must not gain a second mutable mode field. An unopened encoded `Image`
stores the Pillow-owned `Mode` translated once from its header. A lazy operation
result has no `DynamicImage` yet, so its operation must declare and cache a
statically known output `Mode` when it queues the pipeline; a route whose output
depends on unresolved runtime state keeps the existing deferred/materialization
path. This is where mode preflight matters: it lets metadata queries and later
operations use the selected output identity without decoding pixels. It does
not decide whether pixel work is lazy or eager; that remains an operation
contract, as the table below shows.

| Operation | Preflight facts | Pixel or state boundary | Current source decision |
| --- | --- | --- | --- |
| Convert | Exact source `Mode`, parsed target mode, matrix truthiness, transparency presence, and the declared source/target route. | The public Python method calls `load()` before mode parsing and plan construction. Many ordinary mode-only routes queue a typed Convert pipeline; matrix, palette, target-One, scalar-I/F-to-RGBX, and most nontrivial I;16 routes execute their pixel strategy during the call. Some same-carrier I;16 conversions still queue. | Convert is mixed by source/target strategy: source loading is eager, while output pixels may be deferred. Preserve the wrapper load-before-argument order; a metadata plan must not make the whole API eager or skip that load. |
| Paste | Destination, post-conversion source, and mask `Mode`, plus the declared Paste input contract. Its output mode is statically known from the destination's exact `Mode` through `OutputModeRule::Preserve`; no pixel-dependent output-mode calculation is needed. This is distinct from prepared strategy admission, which consumes the destination/source/mask facts and injected `ExecutionContext` after the source conversion or override boundary. Keep that selection at its current admission point unless separate fault-order evidence justifies moving it earlier. | High-level `Image.paste` resolves box shape, loads the source, compares wrapper-visible modes (including Pillow's RGB exception), and calls virtual `convert()` when needed. The unmodified base Convert result is explicitly materialized before preparing the source, loading the destination through `_ensure_mutable()`, checking mask truthiness, or loading an active mask. A Convert override runs before destination loading; Paste reads its returned wrapper's `.im` at source capture. Direct ImagingCore Paste has no high-level source-wrapper conversion call; it captures the supplied source mode and lets the declared mode-pair contract validate it. | Keep high-level source conversion at its callback point and capture the returned image's exact typed `Mode`. Do not calculate a new output mode: high-level Paste preserves the destination. Base-image conversion runs under the active destination context and materializes before destination/mask boundaries. For a virtual Convert result, read the returned wrapper's `.im` before destination loading. If the result is an unopened encoded source without a core, raise `AssertionError`; if it is a deferred Rust operation graph, materialize it with the destination `ExecutionContext` before destination and mask loading. Defer non-Rust core type failures until after destination and active-mask loading. Direct ImagingCore Paste preserves the supplied image mode instead of applying wrapper conversion, then admits the source through the declared pair contract. Direct Rust `Image::paste`, `paste_at`, and `paste_with_input` attach the destination Backend/Architecture context to source Convert and Paste; a mode-only source Convert may stay queued until the combined pipeline is materialized. Keep this distinct from the Python adapter's explicit base-Convert load. After applying its prepared Paste, the public wrapper executes the destination mutation with the same context before returning, matching Pillow's `_ensure_mutable()` then core Paste sequence. Direct Rust `Image::paste`, `paste_at`, and `paste_with_input` remain reusable lazy graph APIs. Route-less mode-only source preparation rejects unresolved image sources. Pixel, fault-order, mutation-isolation, and architecture receipts remain pending. |
| `Image.putalpha` | Exact source `Mode` plus scalar or mask `Mode`; the declared PutAlpha/PutAlphaData contract selects whether identity is preserved or promoted to an alpha-bearing mode. | Pillow loads the destination first, performs mode promotion before mask-mode/load callbacks, then applies the final alpha mutation before returning. | Keep the promotion at its existing callback boundary and materialize it before later host callbacks; finish the final typed write before returning or re-raising a later host error. Preserve partial promotion on faults. Direct Rust graph APIs remain lazy. Output, fault, metadata, and architecture receipts remain pending. Source: [Pillow 13.0 `Image.putalpha`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L1882-L1931). |
| `Image.putdata` | The exact source `Mode` declares accepted scalar/component shapes; sequence length, scale/offset, and per-item Python protocols remain wrapper inputs. | Pillow loads before sequence validation/coercion. The Rust callback-sensitive path already writes each accepted prefix synchronously; its callback-free bulk path queues one typed operation. The public wrapper now executes that operation before returning. | Preserve source-load-before-input behavior and ordered per-item coercion. Flush only after the bulk call succeeds; on callback errors keep already completed prefix writes visible. Direct Rust PutData graph APIs remain lazy. Pixel, fault, and architecture receipts remain pending. Source: [Pillow 13.0 `Image.putdata`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L1932-L1953). |
| `Image.putpixel` and `PixelAccess.__setitem__` | The current exact `Mode` selects scalar/component coercion, palette behavior, and the typed PutPixel strategy; coordinates are normalized at the existing host boundary. | Pillow calls `_ensure_mutable()` before coordinate/value work and changes the pixel before returning. The Rust public Image wrapper and returned PixelAccess now materialize the queued write with their captured context before completing the setter. | Keep palette allocation and Python coordinate/value callbacks in their current order, then execute the exact-mode PutPixel with the injected context. Direct ImagingCore entry points have no separate parity lane. Output, partial side effects, error order, and route receipts remain pending. Sources: [Pillow 13.0 `Image.putpixel`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L1997-L2035) and [`Image.load`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L3666-L3718). |
| `Image.blend` | The first operand's exact `Mode` is the output identity; the pair contract also needs the second operand's `Mode`, both sizes, and the typed factor. | Pillow loads operand 1, then operand 2, validates their mode/size pair, computes pixels, and returns materialized output. | The Python facade loads in that order, calls the typed Rust Blend contract, injects the selected context into its pipeline, and materializes the result before `_new` copies first-operand metadata. Do not preflight mode/size faults ahead of either load. Output, fault-order, and route receipts remain pending. Source: [Pillow 13.0 `Image.blend`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L3439-L3457). |
| `Image.filter` | The actual returned filter carrier supplies output `Mode`; built-in filter strategy and custom callback result are distinct routes. Per-band filters can change later source observations through callbacks. | Pillow loads the source before filter argument construction/validation. A single or multiband filter completes before wrapping the result; for per-band filtering, each band is extracted, filtered, and completed before the next callback, then public Merge completes the combined result. | Do not infer final output Mode from the source before invoking a custom filter. The adapter now materializes each valid returned Rust image at the callback boundary; per-band results finish before the next callback, and Merge completes before wrapping. Preserve invalid-return handling and the active context. Output, callback/fault, and route receipts remain pending. Source: [Pillow 13.0 `Image.filter`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L1325-L1353). |
| `putpalette` | The wrapper validates its Pillow-visible mode before constructing `ImagePalette.raw`; the typed core derives the P/PA target from the current raster `Mode`. Raw palette model and layout are attached after host palette construction. | Pillow changes L/LA to P/PA before later palette failures. It calls `load()` for a pending raw palette, or when an existing core image's mode differs after the wrapper transition. A still-lazy L/LA image with a native palette stays lazy; an already materialized L/LA image crosses the load boundary. For unopened encoded sources, P commits a dirty palette in `load_prepare` before decoder execution, while PA commits at the final base load after decoding. | Keep the initial mode check and `ImagePalette` object work at the host boundary to preserve callback and error order. The typed mutation stages output Mode and reports whether prior materialization requires `load()`; the wrapper combines that with `palette.rawmode` after palette metadata updates. `_load_with_execution_context` preserves P/PA predecoder/postdecoder order for unopened sources and commits before resolving deferred Rust pipelines. Allocation/mmap fault precedence and wrapper-mode overrides at `load_prepare` remain parity TODOs. Rust direct APIs retain their own exact-Mode validation. |
| `ImageOps.expand` | The first wrapper-visible `image.mode` read is evaluated as `_color(fill, image.mode)` before `_color` checks the fill type; string fills use that value. Palette copying and `palette.getcolor` follow. After the wrapper constructor is captured, Pillow reads `image.mode` again for `Image.new`; that second value is the canvas Mode and selects the typed fill contract. | The Python callback path allocates and materializes the fill canvas during the call, then applies the copied palette, copies transparency metadata, and calls public `paste`. It captures the active source route before the fill strategy materializes. Nested public Paste keeps its source/destination loading and strategy boundary, then executes the canvas mutation before returning. Direct Rust Expand APIs still queue their pipeline result. | Keep preflight operation-owned after host color and palette callbacks. Prepare native fill against the second captured Mode; inject the active pinned backend and Architecture context before eager canvas materialization. Do not seed the canvas from the first read or the underlying source raster Mode. Preserve wrapper-visible mode changes, palette callbacks, metadata, and Paste order; public Paste carries its route through the nested mutation and executes it before returning. Output, callback/fault, palette/transparency, and mode-by-architecture receipts remain pending. Source: [Pillow 13.0 `ImageOps.py`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/ImageOps.py) and [`Image.new`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py). |
| `ImageOps.crop` | Border normalization and `image.size` arithmetic form a host crop box before the wrapper invokes virtual `image.crop`. Ordinary Crop preserves exact source Mode; an overridden public crop method owns its result wrapper and identity. | Pillow's `Image.crop` copies an exact full-image box before reversed-coordinate checks; otherwise it checks reversed coordinates, loads the source, applies Python `round`/`int`, checks the output extent, and crops immediately. The Rust wrapper preserves those host boundaries and loads the source, but non-full core Crop queues a typed Crop operation; the full-box path goes through `copy()` and materializes. | Do not compute a new Mode for ordinary Crop. Declare the typed route as Mode-preserving and select its exact mode/architecture strategy from the prepared Crop contract when the queued result is materialized. Keep public virtual `crop`, full-box copy, source load, numeric conversion, and exception order host-visible. Treat the Rust deferred non-full result as an explicit lazy-boundary difference requiring output/fault review. Output, mode-by-architecture, and materialization receipts remain pending. Sources: [Pillow 13.0 `ImageOps.py`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/ImageOps.py) and [`Image.crop`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py). |
| `ImageOps.pad` | The native route preflights source `Mode`, geometry, filter, and exact-mode fill; `PadFillPreflight` binds the typed mode to its native fill/palette strategy. | `contain` runs first. A no-padding result returns before fill and centering work; otherwise canvas construction and palette copying precede centering arithmetic and Paste. On the host fallback, the result is the `contain` result when no canvas is needed; otherwise `Image.new(image.mode, ...)` reads the wrapper-visible mode at canvas construction. | Use this as the existing pattern: precompute geometry/mode-dependent fill in Rust, but invoke it only after the no-padding decision and preserve canvas/palette/centering/Paste order. Native output retains typed source mode; the callback fallback's output identity follows host-visible `contain`/`image.mode` behavior and must not be inferred from the native preflight mode. Keep custom wrapper dispatch host-visible. |
| `ImageEnhance.Color` | Pillow first calls wrapper-visible `getbands()` to choose `L` or `LA`, then reads wrapper-visible `mode`. The typed `ColorSaturation` declaration already exposes the intermediate Mode for the ordinary base-image route. | If the selected intermediate string equals `image.mode`, Pillow aliases `degenerate` to the original object. Otherwise it calls virtual `image.convert(intermediate).convert(image.mode)` during construction; those calls can be overridden and can fail or mutate in sequence. The native shortcut uses typed support only when wrapper methods are the base implementations and no transpose-info load is pending. Every base `Image.convert` loads its source before route validation and completes before the next conversion call. | Keep host getbands/mode reads and same-object aliasing. Use the direct typed Rust route for an unmodified base Image; for overrides, call virtual convert(intermediate).convert(mode) in that order. The native path loads the prior operand/intermediate before returning any synchronous first- or second-hop Convert admission error, while valid queued conversions retain their existing construction-time materialization boundary. The explicit `RGBa` reverse-conversion error materializes the first `L` result before returning. Output/fault and constructor-boundary parity remain open. Sources: [Pillow 13.0 `ImageEnhance.Color`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/ImageEnhance.py#L47-L54) and [`Image.convert`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L937-L983). |
| `ImageEnhance.Brightness` | Pillow reads `image.mode` and `image.size` to construct a black base during enhancer construction, then checks `image.getbands()` and, for uppercase `A`, snapshots `getchannel("A")` through `putalpha`. Output Mode follows that base. | The Python adapter now creates the retained base before `getbands`; the alpha path loads the source through `getchannel`. `enhance()` uses the shared eager Blend adapter, loading the base then current source, validating the pair, and materializing output. | Keep preflight at construction and preserve mode/size, band, and alpha callback order. At `enhance`, perform mode/size admission only after Blend's ordered loads; use the declared Blend strategy under the injected context. Read current source pixels while retaining the uppercase-alpha snapshot. Output, fault, callback, aliasing, and mode/architecture receipts remain pending. Sources: [Pillow 13.0 `ImageEnhance.Brightness`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/ImageEnhance.py#L84-L89) and [`Image.blend`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L3439-L3457). |
| `ImageEnhance.Contrast` | Pillow conditionally calls virtual `convert("L")` based on a live mode comparison, computes a rounded grayscale mean from `ImageStat.Stat` (whose histogram call loads the converted image), creates an L base, compares the returned base's live mode with the original wrapper's mode and may call virtual `convert` again, then checks bands and copies uppercase `A` through `getchannel`/`putalpha`. The ordinary final base preserves the original mode. | The Python adapter follows that sequence. `ImageStat.Stat` calls public `histogram()` and delegates the summary to Rust. The exact base Convert/`_new`/`load` route materializes its result before construction returns; virtual routes retain their returned boundary. The direct Rust constructor first materializes the routed source before selecting or rejecting its conversion path, so source decode/pending-pipeline faults precede a `La -> L` conversion rejection. Public `Image.blend` later loads both operands and executes eagerly. | Use each actual conversion result, comparison, statistic, `Image.new`, and alpha callback as the source of subsequent facts; never plan through a virtual conversion as if its requested mode were guaranteed. Keep the Blend load/validation boundary explicit and admit its pair only after ordered loads. Output, callback/fault, and mode/architecture receipts remain pending. Sources: [Pillow 13.0 `ImageEnhance.py`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/ImageEnhance.py#L57-L74), [`ImageStat.Stat`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/ImageStat.py#L31-L57), [`Image.histogram`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L1639-L1668), [`Image.py` getchannel](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L2567-L2585), and [`Image.blend`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L3439-L3457). |
| `ImageEnhance.Sharpness` | Pillow calls virtual `filter(SMOOTH)` during construction; that returned wrapper is the retained degenerate and supplies Blend's first Mode. It then checks the original wrapper's bands and may snapshot its alpha with `getchannel("A")`/`putalpha`. | The Python adapter now calls `Image.filter(SMOOTH)` during construction and retains its completed result; optional `getchannel` reads the current source and `putalpha` snapshots uppercase alpha. `enhance()` uses the shared eager Blend adapter with ordered degenerate/source loads. | Let public Filter complete before resolving the returned degenerate's Mode; preserve the filtered and optional alpha snapshots. Blend's exact-mode admission follows ordered operand loads. Do not reject P before Filter's source load or recompute the retained filter result after source mutation. Output, fault, callback, mutation, aliasing, and mode/architecture receipts remain pending. Sources: [Pillow 13.0 `ImageEnhance.Sharpness`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/ImageEnhance.py#L99-L103), [`Image.filter`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L1325-L1348), [`Image.py` getchannel](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L2567-L2585), and [`Image.blend`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L3439-L3457). |
| `PIL.Image.composite(image1, image2, mask)` | The output Mode comes from `image2.copy()`. The composite wrapper has no independent source-mode selector; nested public Paste compares the actual source and destination wrappers, handles conversion, and admits the mask. | The wrapper calls public `image2.copy()` first, which loads the destination, then public Paste resolves/loads the source and active mask and mutates the copy during the call. The maintained Python route preserves those calls. The direct typed Rust API uses a declared CompositeModule pipeline. | Do not preflight a new output Mode at the composite wrapper. Keep output identity and the caller context anchored to the destination copy, then let nested Paste own source/mask mode strategy and callbacks. The direct typed route accepts an explicit ExecutionContext, binds it to conversion and CompositeModule children, and validates destination Mode through the same CompositeModule declaration after source conversion and before queueing. Concrete architecture selection remains deferred until prepared materialization. Output, callback/fault, palette, and mode/architecture receipts remain pending. Source: [Pillow 13.0 `Image.composite`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L3460-L3473). |
| `PIL.Image.alpha_composite(im1, im2)` | `im1` supplies output Mode. AlphaComposite requires the same exact supported mode for both operands (`LA` or `RGBA`) and equal dimensions; no mode conversion is requested. | Pillow calls `im1.load()` then `im2.load()` before the core operation and constructs an output wrapper from `im1`. The Python facade calls public `im1.load()` then `im2.load()` before the typed core, preserving overrides; the Rust static API independently loads routed inputs in that order for direct callers, performs mode/size admission afterward, and materializes successful output. | Use the AlphaComposite declaration for exact mode/architecture selection after the ordered input loads, then materialize a successful result before returning. Do not add a color-conversion preflight. Keep eager output behavior separate from Mode selection. Output, error-order, mode/architecture, and latency receipts remain pending. Source: [Pillow 13.0 `Image.alpha_composite`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L3425-L3437). |
| `Image.alpha_composite(self, im, dest, source)` | The in-place destination's Mode is preserved. Source/destination crop boxes are computed from host sequences and dimensions; the actual cropped wrappers become the inputs to nested AlphaComposite. | Pillow validates sequence types and lengths, computes geometry, calls public Crop for source/destination regions as needed, executes static AlphaComposite, then calls public Paste to mutate the destination. The maintained facade keeps those callbacks and the Rust context route. Separately, direct Rust `Image::alpha_composite` validates exact modes and dimensions from metadata before queuing its lazy pipeline; those admission faults precede pixel materialization. | Do not plan the nested strategy from the uncropped input before host geometry and public Crop. Let the resulting pair pass through AlphaComposite's exact declaration, then let Paste own the destination mutation contract; there is no independent output Mode planner. Track the direct Rust queue-time admission and deferred execution as its own contract, distinct from Pillow's static wrapper loading both inputs before checks. Callback, fault, output, route, and mode/architecture receipts remain pending. Source: [Pillow 13.0 instance `Image.alpha_composite`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L1777-L1825). |
| `Image.point` / `Image.eval` | Exact source `Mode`, callable-versus-sequence input, and the optional output-mode argument. Callable scalar transforms use their affine route before consulting `mode`; byte-LUT output mode is known only after the host callback, the `mode != "F"` comparison, rounding, and typed mode parsing. | The wrapper calls `load()` before handler dispatch or LUT work. `ImagePointHandler.point(image)` returns its host result directly. Ordinary byte LUTs queue `Eval`; `mode="F"`, L/P→1, I→L, and scalar callable routes materialize through their typed operation. | Keep the eager source-load boundary. Construct output-mode facts only at the existing typed dispatch point, after host callback and mode-comparison order. Use the declared Point route for its output Mode and strategy; retain lazy Eval for same-mode byte LUTs and eager execution for mode-changing routes. Do not precompute across ImagePointHandler or move LUT callbacks. Output, fault, mode/architecture, and latency receipts remain pending. Source: [Pillow 13.0 `Image.point`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L1828-L1881). |
| `ImageOps.fit` | `min(size)` decides the empty-target branch using the host's current `min`; positive targets use source size, target dimensions, bleed, and centering to compute a crop box. On the native base-image route, the nested Resize preserves exact source `Mode`; host overrides can return another identity. | Pillow checks `min(size) == 0`, unpacks and clamps centering, clamps bleed, reads source size and computes crop geometry, then returns `Image.new(image.mode, size)` for zero live height or calls virtual `image.resize(size, method, box=crop)`. The adapter preserves host comparisons and arithmetic for non-native inputs. The native geometry callback snapshots the core image and reads stored dimensions without decoding pixels. | Fit has no independent output-mode planner. Its empty result reads wrapper-visible `image.mode`; its ordinary route delegates to `resize`, which owns filter validation, loading, mode-specific behavior, and exact Mode/Architecture selection. Direct typed Fit declares `Preserve` with nested `ResizeBoxed`; prepared execution computes geometry before selecting that child. Do not move selection ahead of host callbacks or the virtual Resize call. Output, callback/fault, lazy-boundary, and selection receipts remain pending. Source: [Pillow 13.0 `ImageOps.fit`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/ImageOps.py#L2717-L2863). |
| `ImageOps.contain` / `cover` | The wrappers read current `height`, then `width` and target-size values as needed for aspect ratios. Contain also checks `min(computed_size)` after geometry; the original wrapper `mode` is read only if a new empty image is constructed. | Zero source height and zero target height return through `Image.new(image.mode, ...)` before public resize. Contain has a later zero-axis result branch; ordinary results call virtual `image.resize`. Cover's rounded dimensions reach the public Resize boundary, where its own filter, dimension, mode, and architecture rules apply. | Neither wrapper needs a separate mode/architecture choice on the ordinary path. Empty-result identity comes from wrapper-visible `mode`; normal output belongs to virtual Resize. The direct typed contracts preserve Mode and prepare geometry before selecting the declared same-mode Resize child. Keep host dimension callbacks and empty-result branches ahead of filter parsing where Pillow places them. Output, callback/fault, lazy-boundary, and strategy receipts remain pending. Sources: [Pillow 13.0 `ImageOps.contain` and `cover`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/ImageOps.py). |
| `ImageOps.scale` | The public comparison `factor == 1` is evaluated first; a true result calls `image.copy()` before factor rejection or filter access. Otherwise `factor <= 0`, width/height properties, host multiplication, and built-in `round` determine the Resize size. | A factor of one returns the current public copy result. Nonpositive factors raise before dimension work. Other factors compute dimensions, then call virtual `image.resize(size, resample)`; neither branch reads `image.mode` directly. The native fast path uses metadata snapshots only for admitted base wrapper and built-in factor types; custom values retain host comparison/arithmetic. | Scale has no separate wrapper mode planner. Copy identity is determined by public `copy`; ordinary identity and architecture selection belong to public Resize. The direct typed Scale contract carries Preserve and composes its selected mode/architecture with the prepared Resize child after geometry and parameter validation. Do not move mode/strategy selection ahead of `factor == 1`, host arithmetic, or public Resize. Output, callback/fault, lazy-boundary, and selection receipts remain pending. Source: [Pillow 13.0 `ImageOps.scale`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/ImageOps.py). |
| `ImageOps.deform` | ImageOps snapshots `image.size`, invokes the user `deformer.getmesh(image)`, converts the resample argument, then calls public `image.transform`. That callback may mutate or replace the source before Transform reads its current mode. For LA/RGBA with a non-nearest filter, Pillow applies its premultiplied-mode Transform route and converts back; built-in MESH otherwise preserves the mode captured when the Transform destination is prepared. Custom Transform handlers own their returned wrapper. | Built-in Transform allocates its destination before iterating MESH records. Each record validates the filter and loads/executes the current source during the call; an empty mesh has no per-record load. The prepared Transform strategy captures output Mode and context before record iteration and checks subsequent source identity/mode at record boundaries. | Do not preflight Mode in ImageOps.deform before getmesh. Keep size as the earlier snapshot, then let Image.transform resolve mode and the mode/architecture strategy after the callback, from the same Transform support declaration. Preserve the premultiplied LA/RGBA path, destination allocation, per-record load/filter order, and custom handler route. Reentrant callback, fault, output, and mode-by-architecture receipts remain pending. Sources: [Pillow 13.0 `ImageOps.py`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/ImageOps.py) and [`Image.transform`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py). |
| `ImageOps.equalize` | The initial wrapper mode comparison determines whether the operation requests `convert("RGB")`; the Equalize support declaration gives P that fixed requested output. After conversion and histogram/LUT callbacks, the current wrapper mode determines `_lut` acceptance and the declared L or RGB Point route. | Preserve the sequence: read mode, optionally call virtual `convert`, call public `histogram(mask)`, construct the LUT, then perform `_lut`'s separate live mode checks in order before public `point(lut)`. The histogram and conversion callbacks may return or leave a wrapper whose mode differs from the requested RGB or changes before `_lut`; `point` may also be overridden. | Preflight the P-to-RGB request from the shared support declaration at the conversion boundary, but do not treat it as the returned image's mode or precompute final output mode before callbacks. Re-read wrapper mode at each `_lut` check and validate typed L/RGB support immediately before `point`. The final wrapper result identity belongs to public `point`; keep its execution/materialization boundary operation-owned. Output, callback/fault, and route receipts remain pending. Sources: [Pillow 13.0 `ImageOps.equalize`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/ImageOps.py#L2622-L2674) and [`_lut`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/ImageOps.py#L1863-L1914). |
| `ImageOps.colorize` | The wrapper requires Pillow mode L using the `!=` comparison protocol; the shared declaration gives its normal route a fixed RGB output. Color strings are resolved as RGB and the LUT is three channel, so color values do not choose a different target mode. | Preserve the mode comparison and failure-only `str(image.mode)` conversion, then color-point validation, color resolution/LUT construction, virtual `convert("RGB")`, and `_lut` checks against the returned wrapper before public `point`. The Convert override can return another mode: P is rejected as NotImplemented, L/RGB are accepted by `_lut`, and other modes raise its mode error. | Declare expected output Mode RGB from the exact-L operation contract before pixel execution, while retaining the actual post-callback wrapper for `_lut` admission and nested Eval selection. Do not assume virtual Convert returned RGB or bypass it because the support declaration names RGB. Public Point determines the final wrapper result. Output, fault, callback, and selection receipts remain pending. Sources: [Pillow 13.0 `ImageOps.colorize`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/ImageOps.py#L2146-L2269) and [`_lut`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/ImageOps.py#L1863-L1914). |
| `ImageOps.invert`, `posterize`, `solarize` | These wrappers build their LUT before the shared mode route. Invert first tests `mode == "1"` and routes that case to public `point`; Posterize evaluates `1 <= bits <= 8` and raises its ValueError before building the table; Solarize evaluates the threshold comparison for every LUT entry. | For the ordinary route, `_lut(..., palette=True)` reads live mode after LUT construction. P copies the image and transforms its palette through public palette methods; L/RGB use public `point` (RGB repeats a 256-entry table); unsupported modes fail there. Invert's `1` route skips `_lut` and calls Point directly. | Keep exact mode selection at the wrapper's existing reads, after the operation-specific LUT or validation work. The Posterize Rust orchestrator invokes the injected chained-comparison callback before `8 - bits` or LUT work, preserving Python rich comparisons and ValueError ordering. The P route changes palette entries while preserving indexed pixels and returns the copied wrapper from `putpalette`; L/RGB return the public Point result. Do not use an earlier typed Mode to skip host mode checks, palette calls, or their fault order. Direct typed APIs may select from their exact Mode while preserving their separately declared child contract. Output, callback/fault, lazy-boundary, and selection receipts remain pending. Source: [Pillow 13.0 `ImageOps.py`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/ImageOps.py). |
| `ImageOps.autocontrast` | `preserve_tone` truthiness chooses whether to call virtual `convert("L")`; it does not choose the result mode. The histogram image may be the original wrapper or the actual Convert result. | Pillow evaluates `preserve_tone`, calls `histogram(mask)` on the selected wrapper, builds the LUT (including cutoff and ignore callbacks), then `_lut` reads the original wrapper's live mode: P rejection, L/RGB admission, RGB LUT repetition, then public `point`. | Do not preselect a result strategy from the source mode before histogram/LUT callbacks. Preserve the optional virtual Convert and histogram call order, then resolve the current wrapper mode at `_lut` admission and validate the exact L/RGB support before Point. The Point callback owns final wrapper identity. Direct typed core calls derive exact Mode before queueing, but rejected modes still compute required histogram/LUT work before returning the declared rejection. Output, callback/fault, lazy-boundary, and selection receipts remain pending. Sources: [Pillow 13.0 `ImageOps.autocontrast`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/ImageOps.py#L1918-L2087) and [`_lut`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/ImageOps.py#L1863-L1914). |
| `Image.remap_palette` | Exact L/P source Mode and fixed P result come from the operation declaration; indexed color model and wrapper transparency are separate metadata facts. | Pillow checks Mode; for P without `source_palette`, it loads before reading the attached palette. For L without an explicit palette it prepares the grayscale palette before map iteration. For an explicit palette it evaluates `len(source_palette) > 768` before map iteration to choose RGB versus RGBA; it does not compare against the attached palette length. It then enumerates `dest_map`, slices source bytes per item, and assigns the inverse-map slot. `copy()` materializes after mapping; ImagingCore receives the intermediate palette directly, virtual `convert("L")` follows, then public final `putpalette` and transparency update. | Keep mode/support preflight operation-owned. The staged adapter loads P and snapshots its implicit palette or L's synthesized grayscale palette before map callbacks, captures explicit palette length/model before map iteration, and interleaves each source slice with the same map item's inverse-slot assignment. It creates a typed model-specific mapping palette in core, invokes virtual `self.copy()` (including its `_new` path), assigns the wrapper palette object, calls its `tobytes()` protocol, and installs the resulting bytes through typed core state using the matching `RGB;L`/`RGBA;L`/`CMYK;L` layout. It then invokes virtual `convert("L")` and final public `putpalette`. Callback-driven wrapper/core mode changes remain subject to output/fault parity. Output, callback/fault, route, and latency receipts remain pending. Source: [Pillow 13.0 `Image.remap_palette`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L2036-L2115). |

### Paste override lazy-result fault follow-through

Pillow 13.0 loads the original Paste source, invokes a mode-mismatch Convert,
then reads the returned wrapper's `im` property before `_ensure_mutable()` loads
the destination or mask processing begins. Its base `im` accessor raises
`AssertionError` when the core is absent. A Rust `Bytes` handle with no cached
pixels represents that unopened-image state and must raise at this boundary.
A deferred Rust operation graph is different: Pillow's corresponding
operation has already produced a core, so Paste materializes the graph under
the destination `ExecutionContext` before destination or mask loading. Cached
decode and pipeline errors therefore retain their source-side precedence. A
non-Rust result from an overridden `im` property is held until destination and
active-mask loading, where typed source admission fails. This lets Rust
operations remain deferred until a public caller needs their pixels without
confusing a queued result with an unopened image. Freeze the lazy encoded,
deferred Convert result, and destination/mask fault-precedence cases in the CPU
parity matrix; no runtime validation has run. Sources: [Pillow 13.0
`Image.paste`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L1919-L1938)
and [`Image.im`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L644-L648).
Sources: [Pillow 13.0 `Image.paste`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L1919-L1938)
and [`Image.im`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L644-L648).

### Crop return-boundary source follow-through

The Python `Image.crop` source load, coordinate conversion, and decompression
bomb check are complete before the typed crop call. The typed operation
preserves the exact source Mode and validates normalized extents before pixel
execution. Pillow returns the cropped core before constructing its result
wrapper. The Python boundary now materializes the queued typed crop immediately
before `_new`, so the returned image is a snapshot and allocation/execution
faults occur during the public call. Direct Rust Crop remains lazy. Output,
metadata, source-mutation, fault-order, architecture/fallback receipt, and
latency checks remain parity work; do not change the Crop algorithm or direct
Rust graph based on this wrapper-boundary correction alone.

The source path for the decompression-bomb policy is complete: the Python
facade warns above the mutable `PIL.Image.MAX_IMAGE_PIXELS`, raises above twice
the limit, and passes any representable fatal threshold to Rust. Direct Rust
crop calls retain their fixed safety threshold. Python warning-filter
escalation, mutable-limit boundaries, C-int overflow precedence, and lazy
decode ordering remain parity/fault-contract checks under `pillow-rs#3ptb`.

### Resize and Reduce return-boundary source follow-through

Pillow loads the source and completes Resize and Reduce before constructing
their returned wrappers. The Python facade now materializes typed results with
the active execution context immediately before `_new`; both reducing-gap
paths also materialize the final core Resize. Direct Rust operations remain
lazy. Mode outputs, callback and fault order, metadata, architecture/fallback
receipts, and latency remain pending the frozen CPU gates. Sources: [Pillow
13.0 `Image.resize` and `Image.reduce`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L2136-L2270).

### Point, GetChannel, and Split return-boundary source follow-through

The public Python wrapper now materializes typed Point, GetChannel, and each
Split band with the active execution context before constructing wrappers.
Point-handler callbacks retain their own result behavior, and direct Rust
operations remain lazy. Exact-mode output and rejection behavior, callback and
fault order, metadata, architecture/fallback receipts, and latency remain
pending. Sources: [Pillow 13.0 `Image.point`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L1828-L1881)
and [`split`/`getchannel`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L2549-L2585).

### EffectSpread return-boundary source follow-through

The Python `Image.effect_spread` wrapper now loads the source, executes the
queued Rust result with the active execution context, and returns only after
the public call completes. Direct Rust execution remains lazy. This preserves
Pillow's decode/fault boundary and when the process RNG is consumed. Output,
RNG order, mode/architecture receipts, and latency remain pending. Source:
[Pillow 13.0 `Image.effect_spread`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L2865-L2872).

### ImageChops return-boundary source follow-through

The shared public Python unary/binary Chops adapters now load their inputs in
Pillow order, execute each queued Rust operation with the active context, and
materialize its output before `_new`. Blend and Composite already use eager
routes. Direct Rust Chops graphs remain lazy. Mode outputs, faults, metadata,
architecture/fallback receipts, and latency remain pending. Source:
[Pillow 13.0 `ImageChops.py`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/ImageChops.py).

### ImageOps EXIF transpose return-boundary source follow-through

The Python ImageOps wrapper now materializes the queued typed transpose with
the active context before `_new`, or materializes the replaced in-place core
before reading and rewriting EXIF/XMP metadata. Unchanged-orientation copy
remains eager. Direct Rust Transpose remains lazy; subclass callback behavior,
metadata/fault order, mode/architecture receipts, and latency remain pending.
Source: [Pillow 13.0 `ImageOps.exif_transpose`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/ImageOps.py#L690-L745).

### Filter callback source follow-through

`Image.filter` loads its source before resolving the filter argument, then
invokes the actual filter callback. A one-band or multiband result takes its
Mode from that callback's returned Rust image; Rust image results are
materialized before the callback boundary returns. For a multiband custom
filter, the wrapper extracts and filters each band in sequence, wraps each
callback result immediately, then invokes public Merge using the wrapper's
current Mode. Therefore the source Mode is not a valid prediction of every
callback result, and a requested filter identity must not override the actual
returned carrier. This route remains operation-owned: preserve callback and
load order, select each typed child from its actual Mode, and let Merge own the
final output contract. Source review confirms the actual returned Mode reaches
`_new` and Merge, and the base wrapper now raises missing-property faults at
the Pillow `_new` boundary. Callback-created operation routing is source-
reviewed for methods currently exposed by PyImage: `Image.filter` keeps an
active wrapper scope, binding calls resolve that scope when no explicit context
is supplied, and returned lazy images are bound to its pinned backend and
Architecture before the callback boundary materializes them. `PyImage.transform`
and `PyImage.convert_into` have Rust-backed internal routes. Public
`Image.transform` uses `_transform_apply` and `_transform_mesh_record`; its
public output, mode, fill, callback/fault order, and selection/fallback receipts
remain required parity work. The direct low-level `PyImage.transform` mutates
an existing core and is not the route used by public `Image.transform`, so it
has no separate parity contract. `PyImage.convert_into` output, mode-pair
fault-order, palette, aliasing, and route parity remain TODOs. PyImage and
public Image Arrow C-data methods are still absent. Returned-mode output,
invalid-return faults, callback mutation, and route receipts also remain
parity TODOs.

The base wrapper reads `mode` and `size` from non-Rust callback results before
constructing the derived wrapper, preserving Pillow's `_new` failure boundary
for invalid callback returns. Each callback result is wrapped before the next
band callback, and module-level Merge reads the live wrapper Mode after those
callbacks. These source paths are reviewed; exact output, invalid-return
faults, callback mutation, and route receipts remain parity TODOs.

At that callback-surface review snapshot, Pillow's Arrow C-data methods on
PyImage and public Image were still absent. The 2026-11-25 Arrow follow-through
below records the Rust-backed export and public `fromarrow` source paths.
Pillow 13.0's
`ImagingCore.transform` mutates the destination core from a separate source
core inside `(x0, y0, x1, y1)`, with method, matrix data, and optional filter
and fill. Its C boundary accepts affine, perspective, and quad methods and
reads six or eight doubles from the data sequence. This is distinct from the
existing high-level `Image.transform`, which returns a new image for the full
output extent. The separate low-level PyImage route uses the prepared mode-owned
strategy and carries sample validity into the clipped destination copy: when
`fill` is false, invalid samples and pixels outside the region remain
untouched; when true, invalid samples in the region receive zero fill. Public
`Image.transform` parity is checked through its own prepared apply/MESH route;
the low-level method is an internal detail and is not a separate parity target.
The internal route's partial destination boxes still compute the full output to
preserve absolute coordinates; this is an internal optimization concern outside
the public parity contract. Pillow 13.0 also
exposes `ImagingCore.convert_into`
in `_imaging.c`; it takes a destination core and converts the source into that
existing destination using the destination's exact mode, a null palette, and
dithering disabled. The new binding uses Rust's mode-owned Convert strategy
and applies its raster to the destination in place. Base `Image.py` does not
call it, but Filter callbacks receive the core carrier, so its output,
destination palette update, aliasing, and fault contracts remain an actionable
parity TODO.
Sources: [Pillow 13.0 Image.filter and Image.transform](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py),
[`_imaging.c` transform method](https://github.com/python-pillow/Pillow/blob/13.0.0/src/_imaging.c#L2253-L2312),
[`_imaging.c` convert_into method](https://github.com/python-pillow/Pillow/blob/13.0.0/src/_imaging.c#L1018-L1035),
and [`_imaging.c` Arrow support](https://github.com/python-pillow/Pillow/blob/13.0.0/src/_imaging.c#L215-L317).

### Decision recommendation

Keep output-Mode preflight operation-owned, and keep it separate from pixel
materialization. A generic cross-operation planner would not have enough
information to preserve the distinct callback, conversion, mutation, and load
boundaries in this table. Each typed operation should prepare its declared
input and output Modes, backend, architecture, and concrete strategy at the
operation's existing decision point. Support reporting, admission, and prepared
execution must consume that same declaration; nested operations receive the
prepared context instead of re-inferring mode or architecture.

Expand allocation-order decision (2026-11-23): retain `CheckedDims`-first
validation before fill preparation and canvas allocation. This preserves the
configured Rust pixel-limit policy; Pillow's allocation-first fault precedence
for an allocation failure before ordinary fill conversion is an intentional
difference. It does not change Expand's callback order, output Mode selection,
or eager wrapper materialization boundary.

The preparation point is determined by the public contract. Use facts already
available from exact typed inputs or image headers when they are stable. When a
public callback, virtual conversion, wrapper mode read, palette mutation, or
validation can affect the actual operand, preserve that step and prepare from
its actual result afterward. A requested conversion target is not proof that
an overridden `convert()` returned that mode. Preserve-mode operations still
declare their output identity explicitly, even when no output-mode calculation
is needed.

Preflight does not imply eager execution. Queue pixel work when the operation's
public load, callback, mutation, and fault order permits it; materialize at the
existing observable boundary otherwise. For example, Convert keeps its source
load before target parsing and may queue an eligible conversion afterward.
Paste keeps source loading and virtual conversion first, captures the returned
source's exact Mode, loads the destination before mask truthiness, then loads
an active mask. Its output Mode remains the destination Mode; the prepared
multi-input strategy uses the post-callback source Mode, destination Mode, and
optional mask Mode. The queued context is recursively injected into nested
source and mask graphs before Paste prepares them. For an exact base-image
Convert, the Python facade calls public `Image.convert` under the active
destination `ExecutionContext` and loads its returned image at that call point,
before destination loading and mask truthiness. This pins the source snapshot
and exposes conversion faults before later callback effects. A virtual Convert
override remains host-owned; capture its actual returned wrapper and do not
assume it honored the requested target mode or force it through the base load
path. Read its core at Pillow's source-capture point. Preserve
`AssertionError` for an unopened encoded image with no core; materialize a
queued Rust operation result under the destination context, since Pillow's
corresponding operation has completed before returning. No preflight may move
host callbacks or loads.

This is the recommendation for `pillow-rs#5e8f`, not a closed design task. The
matrix still needs review for affected operation-family coverage, callback and
fault order, and unresolved lazy/eager boundaries before the task is closed.
Keep path-specific source TODOs in the owning modules and the inventory. Do not
add a parallel mode-only planner that can reorder Convert loading, Paste source
conversion or mask handling, palette mutation, or enhancer construction-time
callbacks. The Color wrapper route described above remains source-only and has
not been validated.

Recommended staged implementation plan:

1. **Convert and preserving-mode operations:** make the operation entry point
   capture its typed source Mode, target/output Mode rule, `ExecutionContext`,
   and declared strategy after the existing load and host-argument boundary.
   Queue work only for routes whose public contract permits deferred pixels.
2. **Paste and other multi-input operations:** after public source conversion,
   prepare from the actual returned source plus destination and active-mask
   Modes. Use the destination Mode as output identity, and retain source,
   destination, and mask loading and mutation order. Keep compatibility
   callbacks in the host facade; pass the resolved context and typed inputs to
   the same support declaration used by prepared execution.
3. **Composed operations and enhancers:** apply the same operation-owned
   handoff only after observable conversions, palette changes, geometry,
   filter, statistics, or custom callbacks have produced the actual operands.
   Keep each operation's documented eager/lazy boundary independent, then add
   affected mode-by-architecture output, callback/fault, and route-receipt
   cases to the frozen CPU parity matrix.

- Account for every legacy export, pipeline variant, direct call, mode,
  specialization, and caller edge in the inventory.
- Numeric I;16 PutData must preserve Pillow's low-byte and high-byte
  `CLIP8` behavior while writing the native typed sample directly. The active
  helper now reproduces both clipped lanes for values representable by C `int`;
  retain a source TODO for NaN, infinite, and out-of-range C casts and include
  I;16 numeric bounds in the frozen CPU parity matrix.
- I;16 bytes PutData must mutate only the raw row-prefix bytes Pillow copies,
  preserving all untouched source sample bytes. Use the Python sequence length,
  clip scaled bytes at the same point, and select lane order from exact Mode;
  do not materialize a replacement frame. Keep undefined bytes-subclass
  overreads and scaled C casts outside the finite integer range as source TODOs;
  freeze partial/full-row parity cases.
- Any implementation route left for later must have an actionable `TODO` in
  its owning maintained source module and a matching inventory entry. Name the
  operation or call edge and the next concrete migration step; a broad plan
  statement alone does not count. Keep these TODOs until the route is migrated
  and reviewed, and do not treat their presence as completion evidence.
- Confirm exact mode identity and output transitions, including `1`, palette
  modes, premultiplied alpha, LAB, numeric samples, byte order, and high depth.
- Confirm pillow-rs owns Mode and its mode-bearing DynamicImage, with codec
  translation confined to the decode/encode boundaries. Python parses strings
  before typed core operation calls; image-slash-star retains its own types.
  Inspect operation arguments, support descriptors, and prepared strategies
  together: all use the same Pillow-owned enum, with no string or codec-type
  translation inside compute.
- Review Python facades and PyO3 adapters for duplicated image-operation logic.
  Move mode-specific rules, geometry, LUT/histogram calculations, palette
  allocation, metadata transitions, and image composition into typed
  `pillow-rs` APIs. Keep host bindings to Python protocol conversion, required
  callback/override invocation, wrapper-state updates, and delegation. Record
  each callback-sensitive sequence beside its core entry point and in the
  inventory; a Python implementation branch needs a specific host-boundary
  reason. Preserve callback and fault order while consolidating the logic.
- Keep an indexed image's raster `Mode::ImagePalette8`/`Mode::ImagePaletteAlpha8`
  separate from its palette color model. Add Pillow-owned typed palette data
  that retains RGB, RGBA, or CMYK colors and alpha through loaded, lazy, and
  pipeline states. `image-slash-star::ImagePalette` remains an encoded
  RGB-plus-alpha type; translate it only at the Pillow-owned codec boundary.
  Palette-consuming operations must receive the typed palette model and use
  native mode-specific strategies. Preserve Python `ImagePalette.getcolor`
  callback and histogram order at the binding boundary, then pass its resolved
  native key to Rust. This is required for P-mode CMYK conversion, palette
  transparency, and round-trips; RGB projection is not a valid substitute.
  For `Image.resize(reducing_gap=...)`, freeze parity cases for large finite
  factors and safe-box coordinates around the host C-long/C-int boundaries,
  plus short/long box sequences, custom numeric protocols, and reducer result
  objects exposing Pillow's `im.resize` and `_new` protocol.
- Confirm Backend and Architecture flow from routing through nested operations.
  Default is the fallback for every supported mode missing a specialization.
- Inspect every operation's top-level support contract and its executable
  bindings. All 25 modes have explicit outcomes; multi-input constraints,
  output identity, architecture guards, and fallback reasons are visible.
  Include rejected modes without implementation folders, and distinguish
  declared support, request eligibility, actual selection, and verification.
  Admission and execution consume the same prepared choice. Confirm support
  views are derived from registrations rather than directory inspection or
  an independent boolean registry. Keep evidence status distinct from support.
- Confirm maintained CPU execution contains no Rayon scheduling, parallel
  flags, repeated CPU detection, or separate SIMD executor.
- Inspect the actual tree against the guide. Keep orchestration out of
  `mod.rs` and retain ordinary named implementation files in architecture owners.
- Finish compatibility edits, path-classification rules, source crosswalks,
  maps, and documentation. Do not add, move, or run unit tests for this migration;
  validate operation behavior and strategy selection through the expanded
  parity matrix. Preserve existing parity inputs, expected outputs, and
  assertions while adding the operation × exact Mode × Architecture cases.
- Review file and function size by reading/splitting source. Target 1,000
  physical lines per Rust file and 100 Clippy-counted body lines per function.
  The exact automated function-size proof belongs to the next stage.
- Remove dead maintained code and temporary scaffolding after accounting for
  all feature and architecture configurations. Leave the reference archive inert.

2026-11-23 textual archive-isolation review: searched maintained Rust sources
and repository code for active module/import/include paths into `pool_cpu/legacy`
or `legacy_2`; none were found. The source tree contains both preserved archive
directories and archive-related checker paths, but this textual search is not a
compiler reachability proof. Reconfirm isolation with the planned compilation
and implementation-structure gates after source work is complete.

When all items above are complete, record source review as complete while
keeping compilation, parity, and performance explicitly unproved. Proceed
directly to validation without an extra approval request.

## Stage 6: compile and run static quality gates

Only now run the existing Makefile gates. Fix failures without weakening rules,
adding blanket exclusions, suppressing warnings, or inflating size baselines.

1. Refresh the source map and ownership inventories using the maintained
   Makefile targets. Ensure new paths and the inert archive are classified
   accurately. Preserve checks for every maintained file.
2. Run `make quality`, the consolidated static gate containing formatting,
   checkstyle/Clippy, documentation, and repository checks. Do not separately
   repeat the same successful gate through its `lint` alias.
3. Run the structure and crosswalk contract checks affected by their changes.
   Do not add, move, or run unit-test suites for this refactor, including
   source-tool `*-test` targets. Use the structural inventory generators and
   audits for placement/crosswalk integrity. Prove operation behavior and
   strategy routing through the parity matrix, including preserved errors,
   exact mode identity, support rejection, architecture eligibility, and
   Default fallback.
4. Build the CPU parity target with `make build-parity`. Keep Rayon opt-in and
   disabled for this CPU build. This target avoids installing the replacement
   `PIL` over the live Pillow oracle.
5. Check feature/target configurations affected by the enum and context changes.
   Record which architectures can be compiled and executed on available hosts.
   Do not claim an unavailable architecture was runtime-verified.

Useful maintained inventory targets are:

```sh
make implementation-structure-audit implementation-structure-inventory
make repo-map-update repo-map-check operation-source-crosswalk
```

After a repair, rerun failed or directly affected checks. Do not rerun successful
unaffected campaigns. Coverage collection is outside this migration campaign.

## Stage 7: run the entire CPU parity suite

Run the complete fixed CPU-applicable denominator after Stage 6 succeeds.
The existing full-suite invocation is:

```sh
MIGRATION_TARGET_BACKEND=cpu \
MIGRATION_PARITY_EXECUTION_OUTPUT=build/migration-parity/cpu-refactor.execution.json \
make migration-parity-test \
  MIGRATION_PARITY_OUTPUT=build/migration-parity/cpu-refactor.parity.json
```

The command selects the full manifest because no case, surface, or limit filter
is supplied. Confirm the frozen selection and CPU profile identities in the
result. Keep oracle and target processes isolated. The current full selection
was executed with the architecture matrix below; this remains available for a
future Default-only rerun.

The architecture-aware CPU runner now accepts `MIGRATION_TARGET_ARCHITECTURE`
and injects it through `ExecutionContext` using a test-api-only hook. Mode
implementations do not read runner environment or infer architecture. The
execution sidecar records the requested selector in target backend state and
each operation's selected architecture and fallback receipt. Fixtures and
source oracle cases remain backend-neutral.

Run all non-WASM CPU selectors with:

```sh
MIGRATION_CPU_ARCH_PARITY_DIR=build/migration-parity/cpu-architecture/final-1935a89ac \
make migration-parity-test-cpu-architectures
```

This first builds the test-only selector hook, then runs the complete CPU case
selection once for each of `default`, `neon`, `sse2`, `sse3`, `ssse3`, `avx`,
`avx2`, `avx_fma`, and `avx2_fma`. The requested selector and execution sidecar
are written to separate per-architecture artifacts. Do not include
`wasm_simd128` in this CPU stage. Each specialization available on the host
must execute; unavailable or ineligible routes must report the documented
Default fallback or rejection and its reason.

Historical full pre-extension matrix result (2026-11-09): each selector executed 17,596 Pillow-parity cases,
passed 17,595, and reported zero infrastructure errors. The only failure is
`PIL.Image.Image.save.nuanced.i-png`, where Pillow saves an I32 image as PNG
and image-slash-star rejects the mode during encoding. All nine execution
sidecars report 13,111 complete pipeline cases, 4,485 not applicable, no
indeterminate/missing/partial receipts, and no incomplete terminal receipts.
On this Apple Silicon host, x86 selectors fall back after
`UnavailableHardware`; native x86 kernel execution is unverified. Results are
under `build/migration-parity/cpu-architecture/final-1935a89ac/`. The Make
target returned nonzero when disk exhaustion interrupted compression of the
last report after every report passed schema validation; the final report was
saved in gzip format and passed `gzip -t` afterward. The CPU parity gate stays
open because of the codec mismatch and incomplete operation × Mode outcome
coverage.

Current corpus result (2026-11-10): each of the nine selectors executed the
frozen 25,002-case corpus; 25,001 passed and the same external I-to-PNG encoder
case failed, with zero infrastructure errors. All 252 recorded Rust-only
expected outputs passed on each selector. The receipts establish completion
for this corpus only. They do not establish every `OperationSupport` × exact
Mode × Architecture outcome or every relevant secondary-mode relation, which
must be added before Stage 7 is complete. The complete receipts and corpus hash
are recorded in the current pending matrix above.

Expand parity coverage to the full operation-by-mode-by-architecture
cross-product: exercise every operation request against all 25 exact `Mode`
identities and every CPU `Architecture` selector, including Default. Execute
each specialization that is available on the host; for unavailable or
ineligible routes, assert the documented Default fallback or rejection and
its reason. Supported routes must prove output and exact-mode identity;
declared rejections must prove their existing error contract. Cases should
include the operation's relevant complete and partial blocks, vector tails,
empty dimensions, and mode-specific edge semantics. For operations with
secondary image modes, include the mode combinations that affect support or
dispatch. Cross-compilation is not a substitute for execution on another
architecture.

The Resize portion must include full-source box identity, integer-aligned
crop, fractional boxes, each unchanged-axis pass-elision case, F32 crops with
NaN/Inf samples, filtered versus nearest straight-alpha crops, reducing-gap
exceptions, and native I;16 byte-order/empty-shape cases. These are parity
inputs across the complete Resize mode-by-architecture matrix, not unit tests.

Success requires all selected CPU cases to pass, zero implementation failures,
zero infrastructure errors, and complete applicable execution evidence. Report
legitimate non-CPU-only contracts and platform exclusions separately. Do not
shrink the denominator, hide failures as skips, or accept output parity alone
as proof that a requested specialization ran.

Do not run GPU, WASM, the old independent SIMD backend, or Parallel CPU parity
in this stage. SIMD kernels are exercised as CPU architecture implementations.
Fix implementation failures; rerun failures and directly affected guards. Reuse
passing results only when their input, oracle, build, producer, and executed
path identities still match. If a repair changes all CPU paths, broader
revalidation is justified and its reason must be recorded.

## Stage 8: CPU performance checkpoint and handoff

Preserve the earlier performance requirement after parity passes: compare the
frozen CPU workload IDs with compatible pre-refactor evidence under quiet
conditions. Keep scalar CPU and architecture-specific CPU results identifiable.
Do not mix in Parallel CPU or GPU results, reduce samples, change workloads,
or hide a per-operation regression in an aggregate.

Use the maintained benchmark runner with explicit CPU-only backend selection;
the default benchmark target includes other backends and must not be run
unfiltered. Discover the current runner's selectors from source/help before
execution rather than inventing flags. Accept at most a 5% slowdown per
comparable operation/mode pair, with 1% or less the preferred outcome.
Allow at most one confirmation repeat of questionable pairs and record its
reason. Earlier repeats from a different implementation do not become fresh
evidence; do not exceed the user's repetition allowance without discussion.

Record a CPU completion report containing:

- The actual maintained tree and source-to-target inventory, with no missing
  pipeline or direct CPU paths.
- The mode-bearing enum, mode-transition review, context call edges,
  specialization owners, and explicit Default fallback paths.
- A top-level per-operation support view derived from the typed strategy
  declarations, including rejected combinations, build availability, guards,
  implementation identities, and selection/fallback receipts.
- Quality, compilation, focused parity, full CPU parity, execution-receipt,
  and benchmark results bound to source/build identities.
- All unresolved issues, unavailable architectures, and validation limitations.
  CPU implementation, compilation, quality, or parity failures block handoff.
  Performance failures must be resolved or explicitly accepted before calling
  the CPU stage complete.

Only after the CPU gate passes may work begin on `pool_parallel`. Give it its
own backend context and scheduling, reuse CPU implementations by composition,
and complete its code before its own static checks and separate Parallel CPU
parity campaign. CPU evidence remains a fixed checkpoint unless a subsequent
change actually affects a CPU path. GPU migration follows as separate work.

Keep Kata issues open until their exact work is complete and verified. Old
SIMD/GPU findings in `pillow-rs#brg0` are not closed by CPU parity. Commit only
completed, verified work at the agreed checkpoint; do not mix unfinished
experiments into that commit or force-push history as part of this goal.

## Goal prompt

Run the following to continue the current `pillow-rs` work. Starting the goal
authorizes its implementation and ordered validation stages; it does not
authorize another reset, changing the agreed structure, or reducing the parity matrix.

```text
/goal In /Users/lazytrot/work/pillow-rs, continue docs/CPU_REFACTOR_PLAN.md
under the strict docs/IMPLEMENTATION_STRUCTURE.md contract. Preserve current
work and the baseline 3f9dfd403 backups; do not reset or repeat completed
archive moves. Use pool_cpu/legacy as reference only, with no active imports.

Finish CPU first. pillow-rs owns the flat 25-identity raster::DynamicImage
and matching payload-free Mode selector. DynamicImage::mode() reads the enum
variant; do not store an independently mutable mode/storage pair or infer
mode from buffers. Keep image-slash-star codec types independent; translate
metadata in Pillow-owned decode/encode adapters. Rust mode inputs use the
Pillow-owned enum; Python parses strings before typed Rust operation calls.
Codec pixel-format descriptors must not become a second mode authority.
Use that same enum in operation arguments, support descriptors, and prepared
strategies; keep string parsing and codec translation outside compute.
All backends use this same Mode type, with no backend-local copies. Its 25
identities describe representable rasters; each operation explicitly declares
which it accepts or rejects.

Expose each operation's complete typed support contract at its entry point.
Make it accessible through its OperationId and PipelineOp where applicable,
including direct operations outside PipelineOp. Folder presence is not proof
of support.
Explicitly classify all 25 modes and relevant multi-input combinations,
outputs, architecture requirements, build availability, request guards,
and rejection or fallback reasons. Compose a Mode Strategy with an
Architecture Strategy using concrete types and closed enums. The same linked
declarations must drive support reporting, admission, and prepared execution.
Include rejected identities in the top-level view even without mode folders;
keep declarations, request eligibility, actual selection, and evidence distinct.
Make requested and actual implementation selections inspectable.
Allow support enumeration without materializing pixels; expose unresolved
content requirements as deferred guards, and consume the prepared selection
without independently selecting again during execution.

Inject Backend and Architecture end to end; resolve capabilities centrally
once, with no leaf hardware probes or mode inference. Fold SIMD into CPU at
pool_cpu/ops/<family>/<mode>/<arch>/<files>, retain same-mode serial Default
fallbacks, keep mod.rs limited to declarations/exports, and keep Rayon out of
CPU. Reuse proven shared helpers by composition without duplicating kernels.
Preserve public raster outputs and samples, mode transitions, errors, and
fault ordering. Archived and Pillow internals are reference evidence, not
design requirements. Choose Rust-native algorithms, representations,
arithmetic, and fusion for performance when the public contract remains
exact. Keep specialized hot paths where they help; preserve fixtures,
assertions, and benchmark contracts.

Finish all CPU code and source review, including the 100-line function and
1,000-line file limits, before running any build, formatter, checkstyle,
lint, test, parity, or benchmark. Then run the planned static/compilation
gates, entire frozen CPU parity suite, and CPU performance comparison with
the documented 1-5% allowance and at most one confirmation repeat. Run no
coverage. Prove CPU completion before proceeding to pool_parallel and its
separate context, scheduling, and parity; other backends follow afterward.

Use kata to track tasks under pillow-rs#3ptb and related pillow-rs#brg0.
Keep status and remaining work current; close only verified completed work
and commit completed, verified checkpoints. Discuss necessary design changes
or genuine blockers before deviating; continue routine work autonomously.
```

### CoreOnly histogram bin contract

CoreOnly LA16/RGB16/RGBA16 and RGB32F/RGBA32F histograms keep 256 bins per
channel. Each typed sample is mapped directly to the bin produced by the
established 8-bit projection: u16 uses `(sample + 128) / 257`, and f32 uses the
same clamp, NaN, and rounding rules as `FromPrimitive<f32> for u8`. This is the
performance-first CoreOnly extension contract: scan native typed samples
directly and compute each bin from its source value in place. The f32 owner
uses the same fused `mul_add(255.0, 0.0)` and rounding as the projection helper
so edge samples keep the archived bin. Do not convert the
image mode or sample buffer, allocate an 8-bit/projected raster, or stage
converted samples to reuse the byte-mode path. Masked and unmasked loops are
separate so the unmasked hot path has no per-pixel mask branch. Freeze extension
parity for u16 bin edges and f32 clamp, NaN, and rounding boundaries, including
masked and unmasked scans. These modes carry `NO_PILLOW_SUPPORT` at their mode
owners.

### ImageOps.pad Python composition

Python `ImageOps.pad` now follows Pillow's wrapper composition. It calls
`contain` first and returns that image immediately when it already matches the
target size. Otherwise it creates the canvas with
`Image.new(image.mode, size, color)`, transfers an attached palette from the
contained image, evaluates only the centering coordinate for the padded axis
with Python's `min`, `max`, `round`, and multiplication behavior, then calls
`paste` at the resulting position. This leaves canvas fill conversion and
allocation, palette handling, centering callbacks, and source paste behavior
in their existing wrapper and mode-specific operation owners.

The Python facade no longer uses the Pad-specific PyO3 entrypoints or
centering adapters; those unused bindings were removed. Direct Rust and
JavaScript APIs still use the typed core Pad operation and its CPU Default/SIMD
strategies. Source review only: exact output, callback and mutation order,
metadata, and mode-by-architecture receipts remain in the frozen CPU parity
gate. Source: [Pillow 12.2 ImageOps.pad](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L305-L345).


### Save load and palette boundary

Python `Image.save` injects its virtual `Image.load` method into the Rust save
orchestrator. Save resolves the host path and, when the format comes from a
path extension, resolves that format before invoking the loader. A file-like
input also checks its path-like `.name` for extension-based format selection
without replacing the writer object. A truthy explicit format bypasses
extension selection; its `.upper()` callback runs after the injected load. The
callback runs without an active `PyImage` borrow, so `Image.load` can realize a
dirty Python-owned P/PA palette and update the same Rust image before encoding
reads it. The Rust binding now opens path destinations before compression and
encoder-option conversion, preserving public path-error precedence. Runtime
path cleanup now removes a newly created destination after any post-open save
failure, ignores a permission error from that removal, and preserves an
existing destination's truncation behavior. Runtime parity remains TODO for
custom format and `.name` callbacks, file-like writer callbacks and partial
writes, encoder-failure side effects, route receipts, and encoded bytes. The
current codec API returns a complete byte buffer, so file-like writers still
receive the encoded result at the terminal write boundary. Source: [Pillow 13.0
Image.save](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L2376-L2496).


### Transform dirty-palette boundary

For built-in Transform requests, the Python wrapper queues the typed core
operation first so its synchronous parameter, mode, fill, and method checks
finish before source loading. A dirty P/PA source is then loaded. P output keeps
the Python palette snapshot copied before source realization; the same snapshot
is attached to the core output when needed. PA output keeps its native
index/alpha samples but drops the source palette, matching Pillow's fresh PA
destination. Verify both palette contracts and pixel outputs in CPU parity.
Temporary P fill palettes, reentrant source replacement/info mutation, and
complete callback/error order remain TODO. Source: [Pillow 12.2
Image.transform](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2674-L2838).

### PutData I;16 bytes-subclass boundary

Pillow 12.2 `_putdata` calls `PyObject_Length` for the entry count, then its
bytes fast path reads that many bytes directly from `PyBytes_AS_STRING` while
copying a width-sized prefix per row. A bytes subclass whose reported length
exceeds its physical storage can trigger an out-of-bounds read in Pillow, so
that malformed-input behavior is undefined. The Rust writer zero-fills
missing entries inside the declared row prefix and marks this safe extension
`NO_PILLOW_SUPPORT` at its mode-owned helper. Scaled bytes use Pillow's
`(int)(byte * scale + offset)` conversion; NaN, infinity, and values outside C
`int` are undefined in Pillow and are deterministic Rust extensions marked
`NO_PILLOW_SUPPORT` at the writer. Numeric I;16 values use the same C cast and
the same explicit Rust-only boundary. Freeze both deterministic extension
contracts in the CPU parity extension lane, and keep ordinary partial and
complete byte inputs in Pillow CPU parity. Source: [Pillow 12.2.0
`_imaging.c::_putdata`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L1496-L1589).

### ImageOps.expand staged Python boundary

The Python facade copies Pillow's two- and four-value border unpacking and
reads `image.size` while computing `left + width + right` /
`top + height + bottom`. It captures the initial `image.mode` for
`ImageColor.getcolor`. When a palette exists, it captures that palette's mode,
reads its bytes through `image.getpalette` (after the non-materializing
`image.palette` lookup), and constructs a fresh Python palette. Three- and
four-component tuple fills are resolved by that palette's `getcolor` with the
source image argument; the allocator may inspect reserved transparency entries
or image histogram data when choosing a palette slot. This route applies to P
and PA.
After palette callbacks the facade captures the output wrapper class before
evaluating the second `image.mode` argument, matching Python's callee-before-
arguments call order. It captures that Pillow spelling before size comparisons.
Negative-size comparisons run in Python; the PyO3 boundary then extracts
C-int dimensions and translates the captured spelling to the flat `Mode` enum,
then validates exact-mode `ExpandedDimensions`. The prepared fill and canvas
use that typed target, including when a subclass returns a different valid
mode. The wrapper instance is created around the materialized Rust canvas.
Non-string fill validation follows those size checks. The Rust
`PreparedExpandFill` is
built against the captured target, while Paste uses the current source wrapper
after size comparisons, preserving Pillow's Image.new-then-paste composition
when callbacks replace or change the source. The canvas uses existing
exact-mode Expand strategies. The callback route captures the active pinned
backend and `ExecutionContext` after host size and fill preflight, then applies
them to the queued canvas fill before `materialized_branch()` evaluates it. This
keeps mode strategy selection in Rust and ensures the eagerly allocated canvas
uses the same Architecture policy as the enclosing Python operation. If a
palette snapshot exists, its data is attached through the wrapper's
`putpalette` path before the current source is pasted. The public Python
ImageOps path therefore materializes its `Image.new` canvas during the call;
direct Rust `expand_with_margins` and `expand_with_margins_and_input` remain
lazy pipeline APIs and materialize only when their result is consumed.

The operation support declaration and the prepared `DynamicImage` dispatcher
both enumerate all 25 exact `Mode` variants. Every mode has a declared Default
leaf; the native-byte modes additionally declare their eligible Sse2, Neon, or
Wasm SIMD128 route, composed through the shared native-byte Expand kernel.
High-depth identities use typed u16/f32 fills without mode conversion; the
five internal LA16/RGB16/RGBA16/RGB32F/RGBA32F owners declare CoreOnly support
with nearby `NO_PILLOW_SUPPORT` comments. The prepared selection carries the
actual Architecture and fallback reason into execution. This is source review
only: pixel parity, physical-lane output, exact architecture receipts, and
latency remain pending.

Remaining source review covers dirty/raw and malformed palette faults, output
metadata, and CoreOnly typed fill behavior. The allocation/fill ordering
decision is settled: Rust validates `CheckedDims` and converts ordinary fill
components before allocating pixels. This intentionally differs from Pillow
13 only if allocation itself would fail before a fill conversion error, and
preserves the configured Rust pixel-limit policy. Runtime mode/output/fault
and mode-by-architecture receipt parity remains pending. Sources: [Pillow 13.0
`Image.new`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L2922-L2980)
and [`_imaging.fill`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/_imaging.c#L679-L710).

Both the wrapper preflight and CPU Expand allocation use the declared exact-mode
`ExpandedDimensions` byte count and allow valid zero-sized results. Source
review confirms that the packed mode, native u16, and native f32 loops handle
empty output without staging or converting samples. Palette callbacks run before
the mode property is captured; Python size comparisons run after that; then the
PyO3 boundary resolves the typed target mode and Paste receives the current
source wrapper. Remaining source gaps are dirty/raw and malformed palette
faults, output metadata, CoreOnly typed fill contracts, mode-override output
parity, invalid-mode error ordering, and P/PA tuple-fill faults when a subclass
suppresses `image.palette`. The settled CheckedDims-first allocation/fill order
is the declared Rust contract. No runtime validation has run for this boundary.

Expand callback source audit (2026-11-04): the initial fill mode, optional
palette-mode snapshot, Python palette fill callback, later `image.mode` read,
negative size checks, typed target-mode conversion, prepared Rust fill, and
current-source Paste are separate ordered stages. P and PA now use the same palette-presence route, matching Pillow's
fresh palette construction and its no-image-argument `getcolor` call. The
remaining source TODO covers dirty/raw and malformed palette faults, metadata,
and CoreOnly fill contracts; parity and allocation-fault checks remain pending.

### Filter blur source review update

BoxBlur, BoxBlurXY, and GaussianBlur source comparison against the archived
implementation is complete. The generic native-byte recurrence now retains the
archived no-fraction vector block as well as rolling sample order, Q24 rounding,
horizontal-before-vertical pass ordering, and serial transposition. Exact-mode
owners retain the L, LA, RGB, RGBA, and remaining byte-mode specializations;
declared architecture selection and same-mode Default fallback remain driven by
the prepared filter context. The maintained serial transpose matches the
archived non-parallel loop; parallel scheduling stays outside this CPU path.
Output bytes and selected/fallback receipts still need the frozen CPU parity
matrix. No validation has run for this source-only update.

The RGB radius-one BoxBlur and GaussianBlur architecture strategies have now
also been compared with the archived kernels. BoxBlur retains its three-row
ring and one-row shortcut; GaussianBlur retains its f32-derived weights and six
separately rounded passes. `BoxBlurXY` uses the specialized strategy only for
one pass with radius one on each axis. The L, LA, RGB, and RGBA radius-one
source comparisons and the generic native-byte recurrence comparison are now
recorded. The exact-mode admission and same-mode Default fallback boundary are
source-reviewed: the six native byte identities are L, LA, RGB, RGBA, RGBa,
and RGBX, with their declared 1/2/3/4-channel strategies. The operation support
lookup starts from `DynamicImage::mode()`, prepared selection is context-bound,
and the executor rechecks the exact mode, operation, backend, requested
architecture, and checked byte layout before running the selected route. The
strategy table tries the requested architecture and then permitted declared
lower architectures; when none is usable it retains the exact mode's Default
leaf. Output and selected/fallback receipts remain for the frozen CPU parity
phase. The direct/eager Python Filter caller crosswalk is recorded in the
inventory; private `ImageFilter`/`ImagingCore` duck-typed compatibility routes
remain a source TODO. No build, tests, parity, benchmark, or quality gate has
run for this source update.

The maintained portable-wide architecture recurrence now receives the
physical channel count as a const generic from each exact-mode owner. Its
accumulator and sample-lane storage are sized to that mode, and row/block/
transpose helpers no longer take a runtime channel argument. The Pillow mode
still controls the exact output variant. This keeps the shared identical
arithmetic while specializing the vector loop before execution; it changes no
blur formula. The SIMD declaration-to-owner crosswalk records all 30 archived
BoxBlur/GaussianBlur declarations in Section 8. Output, fault, selection and
fallback receipts, and latency remain unverified pending the CPU gates.

Filter3x3 and Filter5x5 architecture source comparison found a missed
no-interior route: the archived SIMD adapter accepts nonempty native-byte
images when the kernel footprint covers no interior pixels and returns a
`native-copy` result. The maintained architecture guards now admit those
positive-size, checked-layout requests when scale is finite and nonzero and
kernel coefficients are finite, and each exact-mode architecture leaf uses
the shared vector-copy helper and retains its exact carrier. Zero-sized inputs
remain ineligible. The maintained Default and vector byte kernels match the
archived 3x3/5x5 row/tap order, rounding, bounds, channel-lane grouping, and
copied borders by inspection, including the RGB 3x3 byte stream and LA integer
specialization. The L uniform/binomial, LA binomial/single-outlier, and RGB
uniform/BLUR-ring 5x5 routes also match their archived kernel formulas and
receipts by source inspection. Per-mode support and fallback assertions remain
unverified; broader operation-family caller coverage remains open. I32's
architecture uniform-identity shortcuts now retain the archived
`scalar-control` receipt, and nonempty no-interior requests retain the
architecture-selected `native-copy` route across byte and I32 carriers.
The small-uniform byte Filter3x3 identity branch now returns the shared
vector-copy output instead of allocating that output and discarding a second
copy made only for its receipt.
Public Filter3x3/Filter5x5 callers are source-traced: Pillow's Python facade
loads before filter validation, custom Kernel validates mode before kernel
shape at apply time, lazy Image operations carry the source backend and exact
output mode, and direct indexed materialization validates support before
palette expansion. UnsharpMask and ModeFilter remain their separate eager
prepared routes. Exact runtime error order, output, selected/fallback
receipts, and mode × architecture parity remain pending. Broader direct/eager
filter-family coverage remains open. No build, formatter, lint, test, parity,
or benchmark has run for this source update.

Blend physical-lane follow-up (2026-11-03): Pillow `Blend.c::ImagingBlend` interpolates every physical byte in each row, and its alpha-zero/one shortcuts copy the corresponding complete image. The shared Default/SIMD Blend boundary now applies the same float32 sample formula to retained sidecars and resolves a missing endpoint through the canonical per-mode fallback; its alpha-zero/one shortcuts preserve the selected image's lane state. The direct LAB Contrast blend helper follows the same rule. This covers BlendModule and the direct LAB Contrast blend helper. The prepared Brightness wrapper now maps its zero-based physical lane blend in Default/SIMD, resolving absent RGB-family source lanes from the canonical 255 padding fallback; LA-family hidden luma copies remain implicit because they follow the output luma. Prepared Color Saturation and direct Contrast LUT now preserve compact hidden lanes. Sharpness models its filter border/interior sidecar output and final blend in the six compact-carrier strategies, including absent-sidecar fallback synthesis for one- and two-lane modes. Source-only; no validation has run. Source: [Pillow 12.2.0 Blend.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Blend.c).

Brightness physical-lane follow-up (2026-11-03): Pillow `ImageEnhance.Brightness` constructs a zero-filled degenerate image in the source mode, restores a declared alpha band from the source, then calls `Image.blend`. The prepared CPU Brightness wrapper now applies that zero-based float32 Blend formula to retained omitted lanes after the exact Default or SIMD mode strategy; factor zero produces known zero lanes, while factor one preserves the source sidecar. For absent sidecars, RGB-family modes compute the canonical 255 padding lane's zero-based blend; LA-family hidden luma copies remain implicit because they follow the output luma. Source-only; no validation has run. Sources: [Pillow 12.2.0 ImageEnhance.py](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageEnhance.py#L76-L89) and [Blend.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Blend.c).

Draw Bitmap source follow-up (2026-11-03): the public Bitmap entry point now reads accepted mask modes and rejection text from the operation's declared `DrawInputs` contract, retaining early mask validation before image-size lookup. Supported direct strategies still resolve before mode-specific execution, while declared Rejected modes preserve the existing direct Rust order by materializing the mask before returning the unsupported-mode error. The RGB/CMYK native fast paths remain behind Default selection, and the prepared strategy is passed to a smaller raster/output helper. Python already loads the mask before core entry. Public fast-route/error ordering, selected/fallback receipts, and mode-by-architecture behavior remain open for the frozen CPU parity phase. No build, formatter, checkstyle, lint, test, parity, or benchmark has run.

Draw output metadata source correction (2026-11-03): eager Draw output wrapping now retains the input's source format, `Image.info`, EXIF, and palette sidecars for generic Text/Bitmap output, RGB/CMYK Bitmap fast paths, PA, and explicit RGB restoration. This replaces the previous metadata-clearing `Image::from_dynamic` calls; the Python facade reads `format` and native `info` from the Rust image after mutation. Source-only; verify metadata and exact raster outcomes across the frozen Draw mode × architecture cases. No build, formatter, checkstyle, lint, test, parity, or benchmark has run.

Draw exact-mode propagation correction (2026-11-03): removed `effective_mode()`'s silent RGBA fallback when both the explicit drawing context and cached source Mode were absent. It now returns a `Result`, uses the cached enum for normal images, and propagates `Image::mode_identity()` failure on the unresolved path. Color resolution, Bitmap, Text, and default-shape-ink call sites propagate that result while preserving the lazy constructor. This removes a guessed mode from Draw's input path; error order and mode × architecture parity remain pending. No build, formatter, checkstyle, lint, test, parity, or benchmark has run.

Draw image-clone mode error propagation (2026-11-03): `image_clone()` skips mode lookup when the original mode cannot require RGB restoration, and propagates `Image::mode_identity()` errors for the explicit RGBA-over-RGB restoration path. This removes the last swallowed mode lookup found in Draw. Source-only; output/error parity remains pending. No build, formatter, checkstyle, lint, test, parity, or benchmark has run.

Draw Bitmap mask source audit (2026-11-03): the PA exact-mode owner applies the Pillow L-mask rule independently to its index and alpha samples: nonzero L coverage is promoted to full index coverage only when destination alpha is zero, while alpha uses the original coverage. RGBa masks read their alpha byte and use PREBLEND for destination bytes, including zero-alpha mask samples. These paths are present in `BitmapMask`, the PA owner, the four-band writer, and RGB's native fast path; source comparison against Pillow 12.2 `Paste.c` found no discrepancy. This closes the two previously listed Bitmap source-review items; output, error order, metadata, and mode × architecture parity remain pending. No build, formatter, checkstyle, lint, test, parity, or benchmark has run. Source: [Pillow 12.2.0 Paste.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Paste.c).

Custom-font Draw source follow-up (2026-11-03): `ImageFont.ImagingCore` wraps a Pillow-owned image and has no `load()` method. The Python Draw facade now unwraps that carrier and hands the native mask image to the existing mode-specific Rust Bitmap operation; zero-sized masks remain no-ops, and mask mode follows the effective draw mode (`1` for `1`/`P`/`I`/`F`, otherwise `L`). This fixes the current package's `getmask` handoff without routing mask pixels through a Python loop or converting their mode. `ImageText.Text` line state, arbitrary `getmask2` options/offsets, and RGBA embedded-color masks remain TODO at the wrapper owner. Source-only; no validation has run.

Paste contract clarification (2026-11-03): Pillow's C pixel-size checks and raw copy/blend mechanism are diagnostic implementation details, not requirements for Rust. The compatibility target is the externally visible pixel output, exact mode and metadata, and fault behavior/order. Keep the Rust implementation native to its mode strategies, but do not reproduce Pillow's internal storage or copying mechanism solely for structural similarity.

CPU structure and context scan (2026-11-03): a targeted scan of maintained `pool_cpu` found no active imports of `pool_cpu::legacy`, no Rayon APIs or iterators, and no production construction of a default/detected execution context. The remaining `ExecutionContext::new`/`Architecture::detected_host` matches are under operation test modules; production matches inspect the injected `context.backend()`. `pool_parallel` appears only in CPU comments documenting deferred scheduling. This is source-scan evidence only; it does not verify selection, fallback, or output behavior, which remains for the CPU parity gate.

PutData mode-selection audit (2026-11-03): each exact source-mode operation declares the accepted payload `Mode` values and its `ByPayload` output relation. The source `Mode` remains the output identity when payload arity matches; a mismatch selects the payload group’s explicitly declared canonical output mode. The arity value is fixed by the source mode strategy and does not reconstruct source identity from buffer shape or `ColorType`. The `ColorType` read in Transpose is likewise limited to validating the logical carrier extent for the identity-defer path; native mirror channel layout is selected from its exact `Mode`. Source review only; parity remains pending.

Custom-font Draw source continuation (2026-11-03; placement updated
2026-11-05): Draw now obtains positioned custom-font lines from the existing
`ImageText.Text._split` entry point, whose anchor, spacing, alignment, and
justification strategy is owned by `pillow-rs`. The Python facade retains
observable custom-font `getbbox`/`getlength` and `getmask2` calls, mask-mode
adaptation, and returned-offset application. Coverage masks continue through
exact-mode Draw Bitmap; RGBA masks set the requested ink alpha and compose
through Paste while preserving color channels. Native FreeType strings remain
on Rust Text. TODO(pillow-rs#3ptb): support arbitrary `getmask2` arguments,
implement custom-font `textbbox`/`textlength` paths, and cover callback fault
ordering plus RGB/RGBA mask composition in CPU parity. Source-only; no build,
formatting, lint, test, parity, or benchmark ran. Pillow 12.2 reference:
[ImageDraw text mask and fallback path](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L3190-L3310),
[ImageText multiline splitting](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageText.py#L1724-L1940),
[mode-specific ink resolution](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L3092-L3305).

Custom-font Draw binding correction (2026-11-03): RGBA masks now pass the underlying `_rust_image` objects to the PyO3 Paste bridge, matching its `PyImage` inputs instead of passing Python facade wrappers. The `getmask2` fallback is selected only when the method is absent, so an `AttributeError` raised by a custom font propagates through the existing callback path. TODO(pillow-rs#3ptb): custom `ImageText.Text` state, arbitrary `getmask2` arguments, custom metrics, and exact callback/fault and RGB/RGBA composition parity. Source review only; no validation ran.

Autocontrast option source migration (2026-11-03): the typed pipeline and exact L/RGB CPU strategies now carry cutoff pairs, ignored bins, and preserve_tone. Preserve-tone gathers native RGB-to-L statistics and applies one LUT to RGB without changing output mode. Unmasked native L/RGB GPU requests lower the CPU LUT to Eval; masked nonzero cutoffs, ignored bins, and preserve-tone remain a GPU TODO. The Python facade calls the source histogram (or preserve-tone L conversion followed by histogram) before processing ignore and cutoff; the mask load/truthiness boundary and cutoff arithmetic callbacks therefore follow Pillow's source → mask → histogram → ignore → cutoff order. Preserve this wrapper ordering during direct-route work. Source trace matches Pillow 12.2; output/fault parity remains TODO(pillow-rs#3ptb). Pillow 12.2 reference: [ImageOps.autocontrast](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L62-L152).

ImageOps support entrypoint relocation (2026-11-03): the family `imageops::ops` module owns the support descriptors for Autocontrast, Equalize, Expand, and Quantize, plus the Autocontrast, Equalize, and Expand pipeline front doors. Their mode classifiers and prepared mode/architecture strategies remain in the operation modules. The registry and `OperationId::support()` now resolve through the family entrypoint. This code and documentation change is uncompiled and unvalidated; the complete CPU review and verification gates remain pending.

ImageOps mode-strategy placement (2026-11-03): Posterize and Solarize now keep
their mode support declarations and prepared kernel dispatch in the exact
`luma8`, `palette8`, and `rgb8` `ops.rs` owners. Colorize's L support, prepared
strategy, and architecture dispatch live in `luma8/ops.rs`. Each owner receives
the already selected `StrategySelection` and matches only its supported
Default/SSE2/NEON/Wasm SIMD kernels. The family prepared enums route by
`DynamicImage` mode and retain shared hidden-lane composition. This removes the
repeated mode-by-architecture dispatch from the family entrypoint without
changing kernel algorithms. The family `ops.rs` is now 796 lines. This
source-only restructuring is not compiled or parity-validated; full CPU review
and verification gates remain pending.

Autocontrast mode-strategy placement (2026-11-03): moved its exact L/RGB input
guards, architecture declarations, and typed prepared kernel dispatch into the
`luma8/ops.rs` and `rgb8/ops.rs` owners. The family Autocontrast module now
classifies exact modes, builds the shared histogram-derived LUT, and routes to
the selected typed mode strategy; hidden-lane composition remains shared.
Histogram and LUT algorithms are unchanged. Wrapper coercion/fault ordering
still has the recorded TODO. Source-only; compile, parity, and benchmark gates
remain pending.

Autocontrast Python fault-order correction (2026-11-03): the Python facade now
computes the source or preserve-tone histogram before ignoring bins in each
channel and coercing `cutoff` per channel, then checks `_lut` output-mode
support and calls the current wrapper image's `point` method. Rust retains
histogram-to-LUT arithmetic and the point kernels; the wrapper does not
snapshot the source before effectful parameter callbacks. The typed Rust API now computes the public source or preserve-tone L
histogram and validates LUT parameters before returning a declared mode
rejection; accepted L/RGB calls remain lazy. The PyO3 boundary now evaluates
Pillow’s `n * cutoff // 100` and `int()` protocol per channel, then passes
bounded signed counts to the Rust LUT builder. Custom-cutoff callback, pixel,
and fault behavior remains unverified by parity. Source-only; compile, parity,
and benchmark gates remain pending.

ImageOps.expand border source correction (2026-11-03): `_expand_border` now
broadcasts tuple borders of lengths other than two or four to all four sides,
matching Pillow's tuple normalization before the existing Python arithmetic.
Effectful size comparisons, allocation timing, dirty/raw palettes, and source
mutation remain under the recorded Expand TODO. Source-only; no validation has
run.

Equalize mode-strategy placement (2026-11-03): moved L, RGB, and P support
contracts, input guards, architecture sets, and prepared kernel dispatch into
their respective mode `ops.rs` owners. Family Equalize retains exact mode
admission, shared mask/histogram orchestration, P palette expansion, and
hidden-lane composition. The native mode algorithms and P-to-RGB semantic
path are unchanged. This source-only move is not compiled or parity-validated.

Resize legacy-wrapper source cleanup (2026-11-03): `ops::pil_resize::resize` and the boxed wrapper were already gated to tests; source tracing found the old `integer_modes` reducers had no production consumers once the prepared CPU mode strategies were active. Gated that module to tests as well, retaining the shared low-level helpers used by the maintained CPU owners. Source-only; the full CPU source review and verification gates remain open.

Resize strategy-selection continuation (2026-11-03): Resize architecture guards now consume the retained `StrategySelection`, verify CPU/Resize/requested-architecture identity against `ExecutionContext`, and return the selected implementation architecture for local dispatch. The ImageI32 AVX/FMA and NEON kernels and ImageF32 AVX/FMA kernel now receive the selection instead of a bare `Architecture`; their arithmetic paths are unchanged. A static scan of 242 active Resize source files found no raw `Architecture` function parameters or old context-plus-architecture guard calls. Source-only; no build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.

Rotate/Transform sidecar selection-context follow-up (2026-11-03): prepared
Rotate and Transform now pass their retained `StrategySelection` into the
hidden physical-lane composition helpers. Those helpers validate operation,
exact mode, and CPU backend before selecting the architecture-specific nearest
lane sampler. Filtering and pixel kernels are unchanged. Static source review
found no bare `Architecture` parameters in the helpers or prepared call sites;
`git diff --check` passed. Source-only; no build, formatter, checkstyle, lint,
tests, parity, benchmark, or coverage ran.

Filter kernel selection-context follow-up (2026-11-03): native-byte extrema,
rank, and blur helpers now receive `StrategySelection` instead of a bare
architecture. Extrema and rank validate operation, exact mode, and CPU
backend; blur also validates the retained selection against the injected
`ExecutionContext` before its specialized or portable-wide path. All native-byte
mode owners pass the prepared selection. Algorithms are unchanged. Static
source scan found no `Architecture` parameters in active Filter helpers and
`git diff --check` passed. Source-only; no build, formatter, checkstyle, lint,
tests, parity, benchmark, or coverage ran.

Composite AlphaComposite selection binding (2026-11-03): RGBA architecture
binding now receives the retained `StrategySelection` and validates
AlphaComposite/RGBA/CPU identity before producing its local architecture
strategy; mirror fusion binds from that selection. LA preparation validates
the same operation/mode/backend identity before matching its selected
architecture. Static review of remaining raw `Architecture` parameters found
only the const Merge support-declaration builder and Analysis's internal error
formatter. Source-only; no build, formatter, checkstyle, lint, tests, parity,
benchmark, or coverage ran.


ImageOps.expand palette fill continuation (2026-11-04): when no source palette
callback resolves a P fill, the Rust strategy now reproduces `Image.new("P")`
for integer RGB(A) tuples and lists by creating the temporary RGB palette and
filling index zero. PA three- and four-component tuple/list fills now return
the matching mode-specific Pillow type errors. Other P component tuples keep
the single-band fill error, including invalid three/four-component tuple kinds
before component coercion. One/two-component custom tuple coercion, custom
palette callback outcomes, dirty/raw palette faults, palette metadata variants,
CoreOnly fill extensions, and pixel/fault parity remain pending. Source-only;
no build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.

Autocontrast native P strategy (2026-11-04): the Rust CPU operation now declares a CoreOnly P route that builds its histogram from native palette indices, applies the resulting byte LUT through Default or the selected vector architecture, returns exact P identity, and retains the attached palette. The Python facade keeps Pillow's existing P rejection. The new strategy and its palette propagation are source-only and remain uncompiled and unverified; no build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.

Eager GetChannel receipt continuation (2026-11-04): the tiny RGB direct child
route now restores the source graph's captured operation receipts after
clearing terminal telemetry and before recording its selected ExtractBand
strategy. The source graph and eager child therefore remain ordered and retain
their independently selected strategies. Other eager architecture routes that
can follow lazy materialization still need caller-by-caller receipt review.
Source-only; no compilation, formatting, lint, test, parity, benchmark, or
coverage ran.


- Raster mutable escape audit (2026-11-04): `pixels_mut`, pixel/row enumerators, `rows_mut`, `get_pixel_mut`, its checked form, `put_pixel`, `IndexMut`, and `DerefMut` all invalidate retained hidden lanes before exposing mutable samples. `Clone`/`clone_from` preserve data and lanes together; `from_raw` begins without lane provenance; consuming `into_raw` drops the sidecar with the buffer. The only non-invalidating pixel writer is crate-private and called by `DynamicImage::put_pixel`, which updates the lane sidecar from its exact Mode. Source audit complete; downstream Paste/Histogram output parity remains pending. No runtime checks ran.

Eager Convert receipt continuation (2026-11-04): operation-ready materialization
now has a receipt-capturing form, and the direct CPU strategy routes for One,
I;16 targets, palette targets, nonstandard compositions, scalar-to-RGBX, and
matrix conversion restore source graph receipts before recording the selected
Convert strategy. The bilevel matrix route keeps its existing native One-to-L
normalization and now carries the original image's injected Backend and
Architecture into the matrix child instead of rerouting through a temporary
Image wrapper. Other eager architecture callers still need source-receipt and
context propagation review. Source-only; no build, formatting, checkstyle,
lint, tests, parity, benchmark, or coverage ran.

Parameterized Filter eager receipts (2026-11-04): UnsharpMask and ModeFilter
now capture the source graph during operation-ready materialization and restore
its operation receipts before their prepared CPU strategy records selection.
UnsharpMask still resolves its CPU route before materializing; ModeFilter keeps
its palette snapshot and materializes before resolving the route, preserving
their prior ordering. The selected Backend/Architecture context reaches the
same specialized Filter owner and no pixel algorithm changed. Other eager
architecture callers remain under source review. No build, formatting,
checkstyle, lint, tests, parity, benchmark, or coverage ran.

Nested multi-image operation receipts (2026-11-04): binary Chops, fused
Multiply/Screen, and Paste now preserve the caller's active and completed
operation receipts while lazy secondary images materialize, then append each
secondary graph's receipts before the parent operation completes. Multiple
operands are captured separately so a later materialization cannot erase an
earlier graph's receipt list. Pixel algorithms and operation error order are
unchanged. Other multi-image CPU preparation paths remain under source review.
Source-only; no build, formatting, checkstyle, lint, tests, parity, benchmark,
or coverage ran.

Terminal pixel read and projection receipts (2026-11-04): `getpixel`,
`getpixel_formatted`, numeric `putpixel`, and the lazy `getprojection` route
now retain completed source-pipeline operation receipts when starting their
terminal CPU receipt. Loaded-only fast paths keep their existing behavior.

Draw nested receipts (2026-11-04): direct DrawBitmap and DrawText execution
now suspends its active terminal receipt while destination, mask, and output
images materialize, then restores the nested graph receipts in order before
finishing the Draw receipt. The compatible Bitmap-to-Paste route continues to
use Paste's own operation receipts. No drawing algorithm or public fault order
changed.

One-shot PutData receipt (2026-11-04): the I;16 raw-byte fast path captures
source graph receipts across its synchronous load and restores them before
recording its selected PutData strategy. Validation and load-before-selection
ordering remain unchanged. Callback-driven PutData now keeps one ordered
receipt capture in its sequence context, collecting source materialization,
per-item PutData, and callback-generated operation receipts before each
per-write telemetry reset. Dropping the context restores the aggregate in
order, including earlier completed writes if a later coercion fails. Pixel
writes remain synchronous before the next item is coerced. This source change
is uncompiled and unverified; TODO: review nested callback telemetry and
failure paths, then cover them in the frozen CPU parity matrix. No build,
formatting, checkstyle, lint, tests, parity, benchmark, or coverage ran.

Draw dispatch source review (2026-11-04): the operation entry declares support
per exact raster mode and binds the injected CPU/Architecture context once.
Geometry batches admit a vector span strategy only when every operation passes
the mode-specific eligibility guard; direct Bitmap and Text requests select
their exact-mode Default owner, and their executors validate the prepared mode,
backend, and declared implementation before applying pixels. The eager Bitmap
path keeps the compatible Paste route separate, then runs its native fast
routes before materializing the mask and destination for the typed owner. The
Text path obtains the font mask before strategy selection, returns early for an
empty mask, and materializes the destination only after selecting the typed
owner. This review confirms context and strategy propagation only. TODO:
finish semantic and callback/fault-order review for direct Text and Bitmap,
including palette mutation, unsupported CoreOnly modes, and eager native fast
routes; frozen CPU parity remains pending. No build, formatting, checkstyle,
lint, tests, parity, benchmark, or coverage ran.

PutAlphaData mask receipt continuation (2026-11-04): eager mask materialization
for exact mode and dimension validation now uses a nested receipt capture. The
mask graph's receipts are restored around the still-lazy destination
PutAlphaData operation, including when validation fails and Pillow's visible
opaque-alpha promotion is applied. Validation and mutation order are unchanged.
This is source-only; wrapper callback and receipt parity remain pending. No
build, formatting, checkstyle, lint, tests, parity, benchmark, or coverage ran.

Crop and Transpose dispatch source review (2026-11-04): Crop's entry classifies
all 25 modes; 21 select Default and the four I;16 identities compose Default
with guarded SSE2, NEON, or wasm SIMD128 copy strategies. Transpose's entry
classifies all 25 modes across Flip, Mirror, and Transpose; exact mode owners
compose their declarations with prepared Default and available vector
strategies, and the registry records the same prepared selections on execution.
The active `pool_cpu` tree has no Rayon or `parallel` feature dispatch; archived
parallel collection work remains reserved for the later `pool_parallel`
stage. This closes source dispatch review for these families only. Architecture
reachability and mode/architecture parity remain pending. No build, formatting,
checkstyle, lint, tests, parity, benchmark, or coverage ran.

Reduce dispatch source review (2026-11-04): the operation entry classifies all
25 exact modes and prepares the matching mode-owned strategy with the injected
execution context. Seven byte modes declare AVX2, SSE2, NEON, and wasm SIMD128
strategies; Pillow `F` declares AVX, SSE2, NEON, and wasm SIMD128; remaining
modes declare their Default owner. Prepared execution invokes the stored
selection, preserves the exact mode, and composes Pillow's physical-lane copy
or zeroing behavior after reduction. Core-only u16 and internal float modes
stay on native typed sample paths. The active CPU tree still has no Rayon or
`parallel` feature dispatch. This source pass does not establish arithmetic,
fault, palette-metadata, architecture-reachability, or mode/architecture
parity. Those checks remain pending; no build, formatting, checkstyle, lint,
tests, parity, benchmark, or coverage ran.

Analysis callback borrow staging (2026-11-04): Histogram and Entropy classify
the mask before borrowing the source, snapshot its mode for extrema parsing,
release that borrow while Python converts the extrema values, then borrow the
live image for execution. GetColors parses its optional integer limit before
borrowing. Invalid-mask delegation and mask/mode/extrema ordering remain
unchanged. Other effectful conversions and runtime output/fault parity remain
open. Source-only; no build, formatting, checkstyle, lint, tests, parity,
benchmark, or coverage ran.

Python callback borrow staging (2026-11-04): PutAlpha and palette setters now
coerce host inputs before borrowing the image. GetData extracts its optional
band, GetPixel extracts both tuple coordinates in order, and GetChannel parses
its selector before borrowing. These Python conversions can re-enter and
mutate the wrapper; each operation now borrows the live image only after they
finish. The core algorithms are unchanged. Other effectful PyO3 argument
conversions still need review, and output, callback, and fault-order parity
remain pending. No build, formatting, checkstyle, lint, tests, parity,
benchmark, or coverage ran.

Sequence callback borrow staging (2026-11-04): AlphaComposite snapshots its
source image, then parses destination and source sequences in order before
borrowing the destination for mutation. RemapPalette parses its destination
map and optional source palette before borrowing the live image. This lets
custom sequence methods re-enter the wrapper without an active PyO3 image
borrow and preserves the existing core operation order. Remaining effectful
PyO3 argument conversions and runtime output/fault parity are still open.
Source-only; no build, formatting, checkstyle, lint, tests, parity, benchmark,
or coverage ran.

Save/Seek callback borrow staging (2026-11-04): the initial Save binding
extracted format and compression arguments before PathLike conversion; the
Save-specific ordering is superseded by the load/palette boundary above. It
keeps the image borrow only for encoding and releases it before calling a file
object's `write` method. Seek parses its frame value before taking the mutable
image borrow. This keeps Python callbacks outside PyO3 image borrows while
preserving the reviewed argument order. Runtime output and fault-order parity
remain pending.
Source-only; no build, formatting, checkstyle, lint, tests, parity, benchmark,
or coverage ran.

Paste preparation borrow staging (2026-11-04): the box preparation bridge no
longer borrows its image while checking or retaining coordinates. Source
preparation captures destination mode before Python color coercion, then
reacquires the live destination for the existing mode-aware core preparation;
final Paste parsing already uses a handle and takes its mutable borrow after
callbacks. Paste output, reentrant mutation, and fault-order parity remain
pending. Source-only; no build, formatting, checkstyle, lint, tests, parity,
benchmark, or coverage ran.

Geometry input borrow staging (2026-11-04): Resize extracts size, box, and
reducing-gap values before parsing its resampling selector and borrowing the
image. Crop, Rotate, and Transpose likewise finish coordinate, angle, or method
conversion before borrowing. The resize-filter helper now holds only a Py
handle while parsing. Existing argument conversion order and core algorithms
are unchanged. Geometry output and callback/fault-order parity remain pending.
Source-only; no build, formatting, checkstyle, lint, tests, parity, benchmark,
or coverage ran.

Mode and filter input borrow staging (2026-11-04): Convert now extracts its
matrix and color count before mode/dither/palette parsing, then borrows the
current image. Reduce parses factor then box before borrowing. Gaussian/box
blur and max/min/median/mode/rank filters parse radii, sizes, and rank first.
Their core algorithms and selection paths are unchanged. Callback/output
parity remains pending. Source-only; no build, formatting, checkstyle, lint,
tests, parity, benchmark, or coverage ran.

Typed numeric input borrow staging (2026-11-04): KernelFilter, thumbnail
preflight/application, Quantize, UnsharpMask, Color3DLut, EffectSpread, and
enhancement operations now retain a Py handle through PyO3 numeric extraction
and borrow the image only for core access. ColorTransparency likewise parses
its Python value before borrowing. Core algorithms and defaults are unchanged.
Output and callback/fault-order parity remain pending. Source-only; no build,
formatting, checkstyle, lint, tests, parity, benchmark, or coverage ran.

Filter/Transform callback borrow staging (2026-11-04): Filter now performs
callable, attribute, and name callbacks before borrowing the image to validate
and snapshot it; `_apply` runs after that borrow is released. Transform parses
data, snapshots Mode for fill parsing, then reacquires the live image for
execution. Point now loads before callback invocation, captures the initial mode
for Pillow's callable strategy choice, releases the PyImage borrow while the
callable builds its table or affine transform, and reacquires the live image
for application. This preserves mutations made by the callback before point
execution. Output/fault parity remains pending. Source-only; no build,
formatting, checkstyle, lint, tests, parity, benchmark, or coverage ran.

Encoded byte argument borrow staging (2026-11-04): `tobytes_encoded` now
extracts encoder arguments before borrowing the image, then performs mode
lookup and encoding against the current image. This keeps custom sequence
conversion outside the PyImage borrow without changing encoder dispatch.
Output/fault parity remains pending. Source-only; no build, formatting,
checkstyle, lint, tests, parity, benchmark, or coverage ran.

Point callback borrow staging (2026-11-04): Pillow 12.2 loads the image before
calling a point LUT function, evaluates the callable before applying the LUT
to the image, and chooses its scalar-transform route from the initial mode.
The binding now follows that order, builds callback tables without a live
PyImage borrow, then executes against the current image so callback mutations
are visible. Non-callable table extraction also completes before the execution
borrow. Source: [Pillow 12.2.0 Image.point](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1826-L1879). Output and fault parity remain pending. Source-only; no build, formatting, checkstyle, lint, tests, parity, benchmark, or coverage ran.

GetPixel input-order review (2026-11-04): the binding now loads the image
before applying Python's `tuple(xy)` conversion, and performs that potentially
re-entrant conversion without a PyImage borrow. It accepts the iterable inputs
accepted by Pillow's wrapper, then extracts x and y in order before checking
coordinates and reading the pixel. Negative indices are normalized against
their respective dimensions. Remaining C-int boundary, fault-order, and
runtime parity checks are tracked in the existing GetPixel TODO. Source:
[Pillow 12.2.0 Image.getpixel](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1612-L1625). No build, formatting, checkstyle, lint, tests, parity, benchmark, or coverage ran.

Paste architecture reachability review (2026-11-04): the family’s direct
`core::arch::aarch64` intrinsics occur only in NEON row kernels declared under
`aarch64` plus `neon` cfgs. Architecture folders also contain portable wrapper
modules over the shared `wide` kernel; their operation support declarations
mark SSE2, NEON, and Wasm SIMD128 available only for matching target configs.
No architecture instruction is called through an ungated module in this
family. Remaining CPU families still need this audit. Source-only; no builds,
lint, tests, parity, or benchmarks ran.

Color Convert and Composite architecture reachability review (2026-11-04):
the direct intrinsic paths in Color Convert are the RGB8/NEON, RGBA8/NEON,
RGBA8/SSSE3, and CMYK8/NEON routes. Their mode owners declare the supported
target modes and required architecture; `ops.rs` dispatches by the prepared
selection and applies the target-mode guards. Target-feature leaves are called
only by the corresponding selected wrappers, with mode-specific Default
fallbacks retained. Composite's direct AVX/FMA and NEON Blend blocks are
entered only by `execute_selected` for modes in the native-vector support
declaration, whose architecture requirements match `ExecutionContext` feature
checks. RGB Merge's SSSE3 and NEON interleavers likewise require their
mode-specific Merge declaration and guard; other architecture folders use
portable `wide` wrappers. No direct instruction path outside these declarations
was found in the reviewed families. This is static source review only; the
broader instruction-free wrapper audit remains open. No build, formatting,
checkstyle, lint, test, parity, benchmark, or coverage ran.

Direct CPU instruction reachability pass (2026-11-04): all 27 maintained Rust
files found by the `core::arch`, `std::arch`, and target-feature scan were
traced across Color Convert, Composite, Eval, Filter, Grayscale, Paste, Reduce,
Resize, and Transpose. Direct instruction leaves are reached through the
operation's mode/architecture support and prepared dispatch, with exact request
guards where required; mode strategies retain their declared Default fallback.
The I32 Filter AVX/FMA inline-assembly path is likewise restricted to the
prepared `Architecture::AvxFma` selection. This source pass does not audit all
instruction-free architecture wrapper declarations or every family boundary
for prepared-selection propagation; the existing `pool_cpu/ops/mod.rs` TODO
remains open. No build, formatting, checkstyle, lint, test, parity, benchmark,
or coverage ran.

Transpose strategy declaration audit (2026-11-04): the family-level
`declare_modes!` classification covers all 25 exact `Mode` variants. Thirteen
native-byte modes and four I;16 identities use their corresponding prepared
SSE2/NEON/Wasm strategies; seven typed modes are explicitly Default-only, and
RGBA keeps its explicit AVX2 transpose strategy alongside native-byte routes.
The shared declaration macros build the public `OperationSupport`, select
from the injected `ExecutionContext` and request facts, and store the concrete
mode/architecture variant with its `StrategySelection`. The mode-module cfgs
match the declared architecture modules. Default and architecture owners read
the exact mode's native carrier without changing mode to reuse an algorithm.
This closes the Transpose mode/architecture declaration review only; output,
storage-boundary, fallback, and receipt parity remain pending in its existing
TODO. Source-only; no build, formatting, checkstyle, lint, test, parity,
benchmark, or coverage ran.

Quantize method and load-order source audit (2026-11-04): Pillow 12.2 loads the
source before method validation. RGBA accepts methods 2/3 only; other values
receive its RGBA-specific `ValueError`. With a supplied palette, L/RGB skip
color-count, method, and k-means checks after palette/source-mode validation.
Without a palette, negative k-means is rejected before the core quantizer;
nonempty input validates colors before methods, method 3 reports the missing
dependency in this build, and other unknown values report
`ValueError("quantization error")`. Empty input returns empty P before
color/method validation and has an empty palette. The support contract and
mode dispatch now preserve these routes. The Python facade loads the source,
resolves the RGBA method default and restriction, then evaluates palette
truthiness, calls `load()`, and reads `mode` in Pillow's order. Falsey palette
objects take the ordinary
quantization path, and `dither` is only parsed for the supplied-palette path.
External `im` carriers and subclass `mode` overrides that disagree with the
wrapped Rust image remain a code TODO. All 21 CoreOnly mode owners now carry
an adjacent `NO_PILLOW_SUPPORT` note; Pillow's quantizer only admits L, P, RGB,
and RGBA. Source references: [Pillow 12.2
Image.quantize](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1176-L1250),
[Pillow 12.2 quantizer dispatch](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Quant.c#L1588-L1792),
[empty-image binding path](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L1650-L1663).
Source-only; compile, mode × architecture output/fault parity, and lazy-load
fault ordering remain pending. No quality gates or tests ran.

ImageOps Invert mode-scope source review (2026-11-04): Pillow 12.2 routes mode
`1` through `point` and other modes through `_lut`; `_lut` supports `L` and
`RGB`, raises `NotImplementedError` for `P`, and raises `OSError` for other
unsupported Pillow modes ([source](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L1735-L1756),
[invert route](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L2683-L2697)).
The Rust ImageOps support contract now owns `P`'s declared `NotImplementedError`
and the Pillow unsupported high-depth `OSError` routes, instead of duplicating
the `P` rejection in the public operation wrapper. Rust-only inversion for
`La`, `RGBa`, `RGBX`, `YCbCr`, `LAB`, and `HSV` remains available and is now
declared `CoreOnly` with `NO_PILLOW_SUPPORT` at each mode strategy. The P-mode
native child remains a core extension while the ImageOps wrapper rejects it.
ImageOps output and fault parity remain pending; source-only, with no build,
formatting, checkstyle, lint, test, parity, benchmark, or coverage run.

ImageOps point and histogram-strategy audit (2026-11-04): Posterize and
Solarize declare exact L/RGB strategies plus an explicitly CoreOnly P-index
extension; all other modes reject through their operation support. Autocontrast
has the same Pillow L/RGB and CoreOnly P split. Equalize's P route is a Pillow
defined transition: it expands the palette to RGB, computes the RGB histogram,
and returns RGB, reflected by `OutputModeRule::Fixed(RGB)`. Colorize accepts
only L and returns RGB; its exhaustive classifier retains Pillow's assertion
fault for all other modes. Each selected architecture is retained by the
prepared mode owner and dispatched through that exact mode's architecture
module. The palette-to-RGB step for Equalize is semantic output behavior, not
conversion for kernel reuse. Source checks against [Pillow 12.2 ImageOps
Equalize and Colorize](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L1936-L2105)
agree with these routes. Output, fault-order, and per-architecture parity
remain pending; source-only, with no build, formatting, checkstyle, lint, test,
parity, benchmark, or coverage run.

ImageOps Expand mode/architecture source audit (2026-11-04): the family
declaration maps every one of the 25 `Mode` and `DynamicImage` identities to a
mode-owned strategy and forwards `ExecutionContext` to that owner. Byte modes
declare Default plus selected SSE2/NEON/Wasm paths; typed modes declare their
Default strategy and exact native fill rule. The separate typed-fill entry
admits a mode only when its support contract declares `NativeU16` or
`NativeF32`, then dispatches that mode's typed strategy. Internal high-depth
extensions use explicit `NO_PILLOW_SUPPORT` annotations. The support output
preserves source mode. Source-only; output, fill, and fallback parity remain
pending, and no build, formatting, checkstyle, lint, test, parity, benchmark,
or coverage ran.

Draw custom-font callback correction (2026-11-04): Pillow 12.2 wraps the
`getmask2` lookup, call, and offset use in an `AttributeError` fallback, then
tries the full `getmask` signature and falls back to `getmask(text)` only on
`TypeError`. The Python facade now preserves that callback/fault order and
forwards variadic arguments to custom mask methods. Native Rust FreeType
requests carry argument-presence flags into the font options; BASIC rendering
intentionally ignores their values. At this source checkpoint the Python
package did not yet expose `ImageText.Text`, and custom-font metrics were not
implemented. Later source continuations below add the public Text path and
custom-font metrics. Arbitrary `getmask2` arguments, callback faults, and
RGB/RGBA composition remain for frozen CPU parity. Source-only; no build,
formatting, checkstyle, lint, test, parity, benchmark, or coverage ran. Source:
[Pillow 12.2 ImageDraw text](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L3220-L3285).

Draw implicit Bitmap ink correction (2026-11-04): Pillow initializes its
default drawing ink with `draw_ink(-1)`, except that `I` and `F` use `1`.
The mode-specific ink conversion makes omitted fill zero for scalar byte
modes, white for packed multiband modes, one for `I`/`F`, and 65535 for
`I;16`. Bitmap previously supplied white bytes for every mode. It now uses the
shared mode-resolved Draw ink rule already used by Text, while retaining mask
validation before default-ink selection on the no-fill Rust path. Frozen CPU
mode-by-architecture parity remains pending. Source: [Pillow 12.2
ImageDraw initialization and Bitmap](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L2288-L2341),
[`_imaging.c::getink`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L3090-L3305).

Draw P-mode Bitmap ink correction (2026-11-04): Pillow converts string ink
through `ImageColor.getcolor(..., "P")` before consulting the palette. That
produces an RGB tuple; an attached palette maps it to an index, while no
palette leaves a three-component value that single-band `draw_ink` rejects
with `TypeError`. Rust Bitmap previously treated this case as a successful
no-op, and direct Rust string input retained the parsed alpha for palette
allocation. The Python wrapper now parses before the palette check, Rust drops
the string alpha for P palette allocation, and the no-palette route follows
the regular single-band arity error. Source-only; focused CPU output/fault
parity remains pending. Sources: [Pillow 12.2 ImageDraw `_getink` and Bitmap](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L2395-L2482),
[ImageColor.getcolor](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageColor.py#L1017-L1078),
[`_imaging.c::getink`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L3090-L3305).

CMYK Convert fallback reachability review (2026-11-04): Pillow 12.2 gives
CMYK the RGB base mode. `Image.convert` retries an unsupported direct target
through that base; RGB can reach CMYK's declared bilevel, luma/alpha, palette,
I;16, premultiplied, YCbCr, I, and F targets. Palette construction accepts the
normalized RGB carrier. Pillow's LAB path normalizes CMYK through RGBA and
applies the sRGB-to-LAB ImageCms transform; the maintained Rust Convert path
now computes the same result directly from native CMYK samples using its
source-mode matrix. The CMYK mode strategy no longer labels those
Pillow-supported target pairs CoreOnly, and its palette path comment records
the Pillow base-mode behavior. This changes support classification only;
target output, palette metadata, dither, fault, and architecture parity remain
TODOs for the frozen CPU matrix. Sources: [Pillow
12.2 ImageMode.py](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageMode.py#L45-L52),
[Image.convert](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1118-L1151),
[Convert.c RGB conversion table](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L1402-L1487),
[Convert.c palette conversion](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L1093-L1106).
Source-only; no builds, formatting, checkstyle, lint, tests, parity,
benchmark, or coverage ran.

HSV/YCbCr Convert fallback reachability review (2026-11-04): Pillow 12.2
assigns both sources the RGB base mode. Its wrapper retries unsupported direct
conversions through RGB, and Pillow's RGB strategy supports every target
currently declared by these two source-mode owners, including palette and
premultiplied outputs. Their target declarations no longer mark those pairs
CoreOnly; palette comments now record the wrapper normalization. Pillow also
supports RGBX via the same base fallback, and both mode owners now declare and
execute that target with X=255. Existing target output, alpha-lane, palette,
dither, fault, and architecture parity remains pending. Sources: [Pillow 12.2
ImageMode.py](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageMode.py#L48-L52),
[Image.convert](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1118-L1151),
[Convert.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L1402-L1487).
Source-only; no builds, formatting, checkstyle, lint, tests, parity,
benchmark, or coverage ran.

Scalar Convert fallback correction (2026-11-04): this supersedes the earlier
inventory notes that treated F32→I;16* and I→I;16N as Rust-only extensions.
Pillow 12.2 assigns `I` and
`F` the `L` base mode, and `Image.convert` retries a failed direct target after
normalizing through that base. The `F`→`I;16*` strategies now fuse Pillow's
`F`→`L` byte clamp/truncation with the `L`→u16 widening. `I`→`I;16N` likewise
clips through the `I`→`L` byte result before widening; its direct `I;16`,
`I;16L`, and `I;16B` strategies retain their full u16 clamp. All declared
scalar targets for these two modes are therefore Pillow-reachable and no
longer classified CoreOnly. Rust writes directly into the requested native
carrier without constructing an intermediate image. Non-finite F behavior,
all fallback target outputs, palette metadata, dither, faults, and architecture
receipts remain TODOs under `pillow-rs#3ptb`; no validation has run. Sources:
[ImageMode.py base modes](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageMode.py#L37-L80),
[Image.convert fallback](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1139-L1151),
[Convert.c F-to-L and I-to-L](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L572-L584),
[Convert.c F-to-L](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L650-L664),
[Convert.c I;16 converters](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L723-L747),
[Convert.c I;16N table](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L1462-L1482).

One/LA Convert fallback correction (2026-11-04): the One and LA mode support
contracts no longer classify their missing direct `Convert.c` target pairs as
CoreOnly. Pillow assigns both modes the L base and retries unsupported direct
targets through L. One's native path normalizes its expanded samples to 0/255;
LA's paths consume only its luma lane for bilevel, palette, and typed outputs.
Those fused paths preserve the wrapper's L intermediate results for LA, P/PA,
and I;16* targets. Palette metadata and target mode remain attached by the
existing wrapper result path. Exact sample, dither, metadata, fault, and
architecture parity remains TODO under `pillow-rs#3ptb`; no validation ran.
Sources: [ImageMode.py base modes](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageMode.py#L37-L57),
[Image.convert fallback](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1139-L1151),
[Convert.c One/L/LA routes](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L1373-L1401),
[Convert.c L-to-I;16 routes](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L1463-L1482),
[Convert.c grayscale palette route](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L1093-L1148).

RGBA Convert fallback correction (2026-11-04): Pillow's RGBA base is RGB.
RGBA→La and RGBA→I;16* are therefore reached by dropping to RGB, then applying
the corresponding RGB converter. The RGBA→La kernel now emits the RGB-derived
luma with opaque alpha, matching `rgb2la`; it no longer carries the source
alpha through this wrapper route. The existing native-luma I;16* outputs match
the RGB converter's luma bytes and target byte order, so these targets are no
longer CoreOnly. Exact output, fault, and architecture parity remains TODO
under `pillow-rs#3ptb`; no validation ran. Sources: [ImageMode.py RGBA base](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageMode.py#L45-L57),
[Image.convert fallback](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1139-L1151),
[Convert.c RGB-to-La and I;16](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L222-L254),
[Convert.c RGBA and RGB routes](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L391-L410),
[Convert.c mode converter table](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L1411-L1443).

RGBX Convert fallback correction (2026-11-04): the RGBX source strategy no
longer marks I;16*, RGBA, `La`, or `RGBa` as CoreOnly. Pillow assigns RGBX the
RGB base and retries these targets through RGB. The existing fused mode-owned
outputs match those routes: I;16* widens RGB luma; RGBA and RGBa use opaque
alpha; `La` uses RGB-derived luma and opaque alpha. Removed stale comments that
described these routes as Pillow-unsupported because the direct converter table
omits them. Exact output/fault/architecture parity remains TODO under
`pillow-rs#3ptb`; no validation ran. Sources: [ImageMode.py RGBX base](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageMode.py#L45-L57),
[Image.convert fallback](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1139-L1151),
[Convert.c RGBX and RGB target routes](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L1411-L1452),
[Convert.c RGB-to-La](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L222-L228),
[Convert.c opaque alpha expansion](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L380-L388).

RGBa Convert direct target-matrix correction (2026-11-04): the exact target
row remains visible in Convert's `RGB_PREMULTIPLIED_TARGETS`, and the RGBa
source owner now classifies every declared target as Pillow-supported. Pillow's
RGB base path is only the output oracle; Rust computes each result directly
from the native RGBa carrier without allocating an intermediate RGB image.
Straight-color routes unpremultiply each sample as it enters the selected
target kernel: One, L, LA, La, I;16*, RGB, RGBA, RGBX, CMYK, YCbCr, HSV, I, and
F. P/PA remain a separate direct palette strategy: Pillow's `topalette` reads
the stored RGBa color lanes and writes opaque PA alpha. The Default and declared
SSE2/NEON/Wasm kernels now use the same source-mode arithmetic. RGBa-to-YCbCr
and HSV leave the output's omitted X lane at the opaque RGB result; LAB
uses direct source-mode Convert entries and no intermediate image or
`ConvertLab` child. TODO(pillow-rs#3ptb):
verify every target's exact samples, palette metadata, dither, hidden lanes,
faults, and declared architecture paths after the CPU source pass is complete.
Sources: [ImageMode.py bases](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageMode.py#L37-L57),
[Image.convert wrapper](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1139-L1151),
[Convert.c palette strategy](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L1093-L1277),
[Convert.c RGBa direct converters](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L2949-L3022),
[Convert.c RGBa converter table](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L1411-L1452).

I;16 Convert direct-matrix update (2026-11-04): the top-level I;16 target row
now includes RGBX, and the four exact source strategies implement the declared
targets directly. The kernels do not materialize an intermediate L image:
typed samples feed the L, LA, RGB-family, color-space, I/F, palette, and bilevel
outputs in their final loops. The bilevel error-diffusion kernel consumes the
clipped native sample iterator directly; grayscale P/PA writes clipped palette
indices and opaque PA alpha. The SIMD matrix now declares RGBX alongside L,
LA, RGB, RGBA, CMYK, I, and F for SSE2, NEON, and Wasm SIMD128. Cross-alias
I;16 targets preserve full u16 samples as Rust extensions; I;16N→I and
I;16N→F also remain CoreOnly extensions. TODO(pillow-rs#3ptb): verify all
I;16 source × target × architecture samples, palette metadata, dither, faults,
and selection receipts after source review is complete. Sources: [ImageMode.py
I;16 base mode](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageMode.py#L62-L80),
[Image.convert wrapper](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1139-L1151),
[Convert.c I;16 functions](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L720-L830),
[Convert.c I;16 target table](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L1462-L1486),
[Convert.c palette and bilevel paths](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L1101-L1148).

Draw falsey-font source correction (2026-11-04): Pillow 12.2 first uses the
`font is None` branch to apply `font_size`; `ImageText.Text` then uses
`font or ImageFont.load_default()`. The facade now preserves both steps: an
omitted font uses `_getfont(font_size)`, while a supplied falsey font uses the
plain default font without applying `font_size`. Later source continuations
route metrics and drawing through `ImageText.Text`; exact Draw output and
callback/fault parity remain pending. Sources: [ImageDraw.text](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L3150-L3164)
and [ImageText.Text](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageText.py#L1332-L1408).
No tests or quality checks ran.

Draw Python text input ordering (2026-11-04, initial binding split): PyO3 Draw
entries defer text interpretation until after embedded-color validation and,
for drawing, fill/stroke ink resolution. Unsupported inputs follow Pillow's
delayed `.split()`/FreeType error path. Later `ImageText.Text` and custom-font
metric continuations below supersede this checkpoint's note that custom
callbacks were unsupported. Exact text, embedded-color, ink, feature,
font-callback, output, and fault parity remains pending. Source-only; no build,
formatting, checkstyle, lint, tests, parity, benchmark, or coverage ran.
Sources:
[Pillow 12.2 ImageDraw.text](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L3150-L3205),
[ImageDraw text metrics](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L3405-L3524),
and [ImageText.Text](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageText.py#L1332-L1757).

RemapPalette placement review (2026-11-04): L and P selected their serial
strategy by calling the shared scalar byte kernel directly from each mode's
`ops.rs`, leaving the required mode-owned `default` path absent. Added one
Default entry point under each exact mode; both delegate to the unchanged
family-level byte kernel, while mode selection and architecture routing remain
in their existing `ops.rs` files. TODOs remain at both mode owners for native
output, metadata, empty-input, and selection-receipt parity. The broader
architecture-module and prepared-selection source audits remain open. No build,
formatter, lint, test, parity, or benchmark ran.

Mode-directory source audit (2026-11-04): every nonempty direct architecture
directory under `pool_cpu/ops/<family>/<mode>` has a declaration in that mode's
`mod.rs` or `ops.rs`. The only mode owners without populated `default` folders
are five explicitly rejected internal high-depth Draw modes and eight
Thumbnail modes whose prepared execution composes same-mode Reduce and Resize
strategies. No undeclared architecture implementation or missing serial leaf
was found in this inventory. The family-wide source audit for prepared
selection flow and operation support declarations remains open. No build,
formatter, lint, test, parity, or benchmark ran.

Draw binding input split (2026-11-04): moved PyO3 Draw argument classification
into `pillow-rs-py/src/draw/input.rs`, separating host-value parsing from the
ImageDraw method bindings. This brings `pillow-rs-py/src/draw.rs` below the
1,000-line source limit without changing parsing behavior. No build, formatter,
checkstyle, lint, test, parity, or benchmark ran.

Custom-font metric source closure (2026-11-04): the Python Draw facade now sends
custom-font `textbbox`, `multiline_textbbox`, and `textlength` through that
font's `getbbox`/`getlength` callbacks. Bounding boxes reuse the existing
mode-aware line layout, including anchor, spacing, alignment, justification,
stroke width, and font-mask mode. Newline selection now follows the input's
string-versus-bytes contract for both line splitting and justification. This
supersedes earlier source notes listing custom-font metrics as unimplemented;
custom callback/fault ordering and exact results remain unverified until the
CPU parity gate. Direct source-to-LAB Convert entries were also rechecked:
all supported source modes write LAB through their mode-owned Default strategy,
LAB identity stays mode-owned, and La retains its declared rejection. Neither
source review ran builds, formatters, checkstyle, lint, tests, parity,
benchmarks, or coverage.


ImageOps wrapper composition source audit (2026-11-04): `grayscale` now
delegates to the public `Image.convert("L")` boundary, preserving target-mode
translation and conversion-time transparency metadata. `flip` and `mirror` now
compose through `Image.transpose`, reusing the existing mode/architecture
route and wrapper metadata behavior. For `P`, `equalize` now uses
`Image.convert("RGB")` before its Rust Equalize strategy, so dirty Python-owned
palette edits and conversion metadata are applied before palette expansion.
The Python facade then calls `image.histogram(mask)` before the typed Rust LUT
builder and applies the result through public `image.point(lut)`. This keeps
the eager histogram boundary and routes the computed counts without rescanning
the raster. At this 2026-11-04 snapshot, arbitrary subclass histogram return
objects were still a TODO and the adapter narrowed values to `Vec<u32>`; that
gap was resolved by the callback strategy recorded in the 2026-11-05 Equalize
histogram-protocol entry below. Other direct ImageOps strategies remain on
their mode-owned Rust paths; Posterize/Solarize P-index execution is an explicit
`NO_PILLOW_SUPPORT` Rust extension. Source review only; no build, formatter,
checkstyle, lint, tests, parity, benchmark, or coverage ran. Source:
[Pillow 12.2 ImageOps](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L1735-L1757)
and [ImageOps actions](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L2408-L2460).

ImageOps Colorize host-order follow-up (2026-11-04): the Python wrapper now
checks `image.mode == "L"` and Pillow's two-/three-color point ordering before
calling the typed Rust strategy, so invalid mode and point errors precede color
parsing. TODO(pillow-rs#3ptb): custom point objects whose comparisons succeed
but whose `__index__` fails still reach PyO3 narrowing before Rust color
parsing; preserve that late coercion order in a host input adapter. No build,
formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran. Source:
[Pillow 12.2 ImageOps.colorize](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L1992-L2010).

Direct Convert-to-LAB matrix source recheck (2026-11-04): all 23 supported
non-LAB/non-La source modes have a mode-owned Default path that maps native
samples into the shared RGB-sample LAB kernel and allocates only the final LAB
output. RGBa unpremultiplies at its source owner; palette modes read palette
colors; CMYK/YCbCr/HSV apply their source color transform; One and L/LA use
expanded luma; Pillow I/F use their clipped byte samples; typed internal modes
use their explicit native-to-byte projection. LAB identity remains a separate
mode-owned path, and La has no LAB target. No architecture guard declares a
LAB-specific vector route, so requested architectures resolve to each source
mode's Default implementation. Target output/fault/fallback-receipt parity is
still TODO(pillow-rs#3ptb). Read-only source review; no checks ran.

ImageOps Equalize eager-composition update (2026-11-04): the Python facade now
calls `Image.histogram(mask)` before building Pillow's per-band Equalize LUT,
then applies it through `Image.point`. The P-to-RGB public conversion remains
first. Unsupported modes are rejected after histogram and LUT preparation,
matching the `_lut` boundary. This closes the facade's deferred-histogram source
TODO; direct Rust `imageops_equalize*` APIs retain their separate lazy pipeline
contract. Mode/architecture selection, outputs, errors, and callback parity
remain pending the CPU parity gate. Source-only; no validation ran. Sources:
[Pillow 12.2 Equalize](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L2408-L2460)
and [Pillow 12.2 `_lut`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L1735-L1756).

ImageOps Colorize composition update (2026-11-04): Python now performs the
Pillow sequence in the wrapper: assert L mode and point ordering, parse color
strings, build the two- or three-color channel LUT while leaving point values
uncoerced until their actual range/arithmetic use, convert through
`Image.convert("RGB")`, then call `Image.point`. This removes premature PyO3
u8 narrowing and retains Python sequence/index callbacks in LUT construction.
The direct Rust Colorize operation remains a typed lazy API. Exact output,
callback, fault, and mode/architecture parity remain pending. Source-only; no
validation ran. Source: [Pillow 12.2 ImageOps.colorize](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L1992-L2105).

ImageOps host-algorithm boundary correction (2026-11-04): the final Equalize
composition keeps histogram collection in the Python facade, but delegates LUT
arithmetic and Equalize mode admission to a hidden typed Rust entrypoint before
calling public `Image.point`. Colorize keeps the Rust Colorize strategy and
core LUT implementation; its PyO3 boundary now accepts point objects without
narrowing and converts them only after parsing black/white/mid colors. This
supersedes the previous wording that placed LUT algorithms in Python. Arbitrary
custom Histogram return objects and adversarial point arithmetic callback
interleaving remain TODOs. No validation ran.


Draw ImageText public API follow-up (2026-11-04): added `PIL.ImageText.Text`
and exposed its text, font, mode, spacing, direction, features, language,
embedded-color, and stroke state through the public module. `Draw.text` and
`Draw.multiline_text` unpack the state and use the existing Rust FreeType
renderer or Python custom-font callback path. Existing `_split` overrides run
after ink resolution, and their positioned lines reach those same renderers.
`Text` also provides font-mode selection, length, multiline splitting,
bounding-box operations, word wrapping, height limits, and FreeType grow/shrink
scaling. The wrap state machine preserves byte versus string text, explicit
newlines, unbreakable-word errors, partial remainder state, and font mutation
order. Source review only; TODO(pillow-rs#3ptb): verify wrapping, bounds,
remainder text, font size changes, and fault order in the frozen CPU matrix. No
validation ran. Reference: [Pillow 12.2
ImageText.Text](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageText.py#L1332-L1757).

ImageFont text-input source correction (2026-11-04): the FreeType text adapter
applies Python `len(text)` in the public facade before PyO3 converts optional
arguments, enforcing Pillow's 1,000,000-character ceiling. The Rust adapter
then retains `str`, `bytes`, or invalid-type identity for core dispatch rather
than stringifying or rejecting invalid objects early. Option-bearing ImageFont
BASIC paths receive feature presence only and do not iterate a non-None feature
object; core option validation precedes the invalid-text TypeError. Draw
`textlength` now constructs this Python-owned value before any PyO3 call, so
newline membership and font callbacks run before typed option conversion; its
obsolete direct PyO3 shortcut was removed. TODO(pillow-rs#3ptb): verify custom
newline `__contains__`, text `__len__`, font `getlength`, embedded-color,
output, and fault ordering in the frozen CPU matrix. Source-only; no build,
formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran. Sources: [Pillow
12.2 ImageFont guard](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageFont.py#L2273-L2278), [ImageText length ordering](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageText.py#L1697-L1722), and [FreeType layout validation](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imagingft.c#L380-L416).

Draw Text source correction (2026-11-04): plain `ImageDraw.text` inputs now
construct the same `ImageText.Text` value as Pillow and use its `_split` path
for all Rust-font and custom-font calls. This restores ordinary newline
handling and ensures per-line anchors and fractional positions reach the Rust
font mask through `start`. Direct Text output, fault order, and selection
parity—including the open historical cases in `pillow-rs#brg0`—remain TODO.
Source-only; no build, formatter, checkstyle, lint, tests, parity, benchmark,
or coverage ran. Sources: [Pillow 12.2 ImageDraw.text](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L3103-L3218) and [ImageText.Text._split](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageText.py#L1724-L1940).

Draw multiline delegation source correction (2026-11-04): `multiline_text`
now delegates every input to the shared `text` method, matching Pillow's
wrapper and ensuring plain strings and `ImageText.Text` use the same splitting,
alignment, fractional positioning, font selection, and rendering paths. Exact
output and fault parity remain TODO under `pillow-rs#3ptb` and `pillow-rs#brg0`.
Source-only; no validation ran. Source: [Pillow 12.2 ImageDraw.multiline_text](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L3328-L3403).

Transform P-fill palette source correction (2026-11-04): the core now carries
an RGB palette created by tuple or named-color `fillcolor` through the lazy P
result when no source palette entries exist. A populated source palette keeps
precedence, and empty-mesh output uses the same palette metadata path. Python
no longer installs a synthetic empty source palette over the generated fill
palette. Reentrant source/info changes and full fill/data callback ordering
remain TODO under `pillow-rs#3ptb`; output and metadata parity are unverified.
Source-only; no validation ran. Source: [Pillow 12.2 Image.new and Image.transform](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2926-L2968) and [Image.transform](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2674-L2758).

Transform binding-order source correction (2026-11-04): omitted method data
now raises `ValueError("missing method data")` before fill parsing, and a
present fillcolor is converted before Python data-sequence extraction. This
matches the wrapper's early missing-data check and `Image.new` fill work before
mesh iteration. Method coercion, geometry-vs-filter validation order, and
reentrant metadata snapshots remain TODO. Source-only; no validation ran.

Transform premultiplication-order correction (2026-11-04): the Python wrapper
now routes LA/RGBA through La/RGBa for non-nearest resampling before dispatching
an `ImageTransformHandler`, then converts the handler result back to the
original mode. This matches Pillow's public outcome for custom handlers as well
as built-in transforms. Exact output, callback/fault order, and mode/architecture
parity remain TODO under `pillow-rs#3ptb`; the Transform input/data callback and
metadata snapshot audit is still open. Source-only; no build, formatting,
checkstyle, lint, tests, parity, benchmark, or coverage ran. Source:
[Pillow 12.2 Image.transform](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2728-L2757).

Transform missing-data facade correction (2026-11-04): after an optional
`getdata()` callback and after custom-handler dispatch, the Python facade now
raises `ValueError("missing method data")` before entering PyO3. This keeps the
missing-data fault ahead of typed `size`/`method` extraction on the public
route. Custom-handler inputs continue to ignore `data`. Full callback order,
metadata snapshots, output/fault results, and mode/architecture receipts remain
TODO. Source-only; no validation ran.

Transform callback-order staging (2026-11-05): the PyO3 bridge now separates
fill/size preparation, host method/data coercion, and execution. The facade
snapshots public `info` and P palette state after fill preparation but before
method-data iteration, then reads the current Rust image for execution so a
data callback that replaces the wrapper image is observed. Fill and size are
prepared first; the facade then performs Pillow's size/method comparisons,
applies method-specific shaping, checks resample membership and unusable-filter
diagnostics, converts numeric data, and then narrows the method value. The staged plan
carries the exact source Mode captured before fill callbacks. Pillow 12.2 constructs the output before consuming method data. Non-mesh
methods perform their method-specific slice, unpack, or arithmetic before
resample validation, then load source and destination before the raster call;
mesh iterates and executes each quad in sequence. AFFINE/PERSPECTIVE slicing and EXTENT unpack/arithmetic now happen before
resample validation; numeric extraction still happens before image loads, and
QUAD/MESH remain generic paths. The Python bridge now allocates an exact-mode
output and snapshots its Rust palette state before method-data callbacks; P
output construction copies the source palette at that same stage. Remaining
TODOs are post-load numeric conversion, exact custom-sequence callbacks, QUAD
coefficient semantics, per-quad MESH execution/fault interleaving, overridden
mode/palette properties, callback-driven palette/mode mutation, output/fault
parity, and mode/architecture receipts under `pillow-rs#3ptb`. Source-only; no
build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.

Convert LAB rejection clarification (2026-11-04): the current Pillow wrapper's
LAB path normalizes non-RGB-family sources to RGBA. `La` cannot complete that
public conversion and reports `conversion from La to L not supported` through
the conversion fallback, so it remains an explicit rejected source/target row;
all admitted rows retain direct native-sample-to-LAB strategies. This records
the observed output/fault contract, not a requirement to reproduce Pillow's
intermediate conversion. Sources: [Pillow 12.2 Image.convert](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1118-L1151)
and [Pillow 12.2 Convert.c routes](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L1368-L1402).

PutAlpha exact-mask load-state source correction (2026-11-04): the exact
Python `Image` mask path now checks PutAlphaData destination support before
loading the mask, calls `alpha.load()` for accepted `1`/`L` masks so the actual
wrapper retains its materialized/info synchronization, and promotes the
destination if that load fails. A mask that aliases the destination promotes
before its changed mode is validated. Invalid distinct-mask modes still enter
the core PutAlphaData path, which applies the declared destination promotion
before returning its mask error. Successful distinct-mask calls still queue
one PutAlphaData operation. TODO: verify aliasing, mask-load failure,
invalid-mode promotion, metadata, and selection receipts in the frozen CPU
parity matrix. Source-only; no validation ran.

Rotate non-fast-path point validation correction (2026-11-04; superseded):
the initial binding validated ordered `__getitem__` calls but converted each
point item only once. The affine handoff below supersedes that snapshot route.
Exact fill output and mode/architecture parity remain pending. Source-only; no
validation ran. Source: [Pillow 12.2 Image.rotate](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2302-L2374).

Rotate fill source correction (2026-11-04): the binding now accepts tuple
components only for tuple inputs, rejects non-Python lists for non-Palette
modes, and unwraps one-element tuples as Pillow's `getink` does. Multi-band
integer tuples validate the first C `long long` and subsequent C `int` lanes
before core fill resolution; wrong tuple arity remains mode-specific. Python
scalar integers preserve the long-long overflow fault, and scalar floats in
integer modes retain the mode-specific type error. Custom `__index__` callback/error parity and exact fill output remain TODO. Source-only;
no validation ran. Sources: [Pillow 12.2 Image.transform](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2674-L2758), [Image.new](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2926-L2968), and [`getink`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L534-L649).

Rotate Python affine handoff (2026-11-04): the facade now computes the six
reverse-affine coefficients, expanded dimensions, and expansion translation
before Rust parses resampling or fill inputs. It preserves Pillow's repeated
center/translate indexing and Python arithmetic callbacks, then carries the
result through the existing Rotate pipeline and exact-mode CPU strategies.
Dirty-palette preparation shares this path. GPU admission rejects this new
host-computed payload pending `pillow-rs#3ftb`. Exact matrix
coercion, fill faults/output, callback mutation, and mode/architecture receipts
remain TODO under `pillow-rs#3ptb`. Source-only; no build, formatter, checkstyle,
lint, tests, parity, benchmark, or coverage ran. Source: [Pillow 12.2
Image.rotate](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2302-L2374).

Rotate prepared-stage correction (2026-11-04): ordinary and dirty-palette
requests now use the same prepared-plan handoff. The binding captures Mode and
output size, converts and resolves fill, then converts/validates resampling.
The facade loads the source next; a second binding step narrows and attaches
the affine values after that load. This preserves the public
`Image.transform` fill-before-filter and filter/load/matrix fault order while
keeping execution on the selected exact-mode CPU Rotate strategy. LA/RGBA
non-nearest requests preserve Transform's earlier resample comparison and
source-load boundary. GPU admission continues to reject the host-computed
affine payload until `pillow-rs#3ftb` implements it. Native output versus
Pillow's premultiplied result, overridden `convert()` behavior, fill/resample
faults, reentrant callbacks, size/matrix faults, and mode/architecture receipts
remain open under `pillow-rs#3ptb`.
Source-only; no validation ran. Source: [Pillow 12.2
Image.transform](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2741-L2758).


Rotate virtual-method source follow-up (2026-11-04): after building the affine
request, the facade delegates to an overridden `transform` method unchanged.
For LA/RGBA non-nearest requests, an overridden `convert` follows the public
`convert(premultiplied) → transform → convert(original mode)` composition; the
base Image route continues through its native exact-mode Rotate strategy. This
preserves subclass-visible calls without adding mode conversion to ordinary
images. Output/fault, callback, and selected-route parity remain TODO under
`pillow-rs#3ptb`. Source-only; no build, formatter, checkstyle, lint, test,
parity, benchmark, or coverage ran. Source: [Pillow 12.2
Image.rotate](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2302-L2374)
and [Image.transform](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2728-L2758).


Rotate host-affine identity correction (2026-11-04): native byte, scalar, u16,
and numeric Default routes now use the shared identity contract, which rejects
a host-supplied affine matrix from the zero-angle clone shortcut. A 0° request
with translation or explicit output geometry therefore reaches the selected
mode kernel and consumes its affine payload. SIMD routes already used the same
contract. Output and architecture-receipt parity remain pending. Source-only; no
build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.

Rotate output-size/fill ordering correction (2026-11-05): the prepared PyO3
route validates size container, arity, and negative dimensions first. It
classifies P sequences before native size conversion; only all-integer RGB(A)
sequences resolve as temporary palette colors there. Other fills parse after
signed C-int conversion, matching Pillow's `Image.new` wrapper and `_fill`
boundary. Dimensions no longer accept the wider `u32` range. Output allocation
and exact fault/output parity remain pending under `pillow-rs#3ptb`. Source-only;
no build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.
Sources: [Pillow 12.2
`Image.new`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2909-L2968)
and [`_fill`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L655-L686).


Transform typed-input preflight correction (2026-11-05): `transform_prepared_public` now checks typed AFFINE, EXTENT, PERSPECTIVE, QUAD, and MESH data before parsing the resample filter. EXTENT validates its four-value unpack and zero output dimensions; PERSPECTIVE and QUAD retain their method-specific short-data errors; MESH validates its first typed box/quad record before filter parsing and bypasses filter parsing for recognized empty mesh payloads. The image execution path remains unchanged. Python custom-sequence iteration, later per-quad fault/execution order, output allocation timing, and parity/selection receipts remain TODO. Source-only; no build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.


Transform generic MESH iterator staging (2026-11-05): the Python binding now obtains the MESH outer iterator once, returns the prepared empty-mesh result without checking resample when it is exhausted, unpacks and converts the first record before resample validation, then consumes remaining records afterward. This accepts generators and empty custom iterables and preserves dictionary-key iteration instead of rejecting mappings by type. Later records are still converted before source loading and raster execution; their callbacks/faults do not yet interleave with each completed quad. Custom inner sequences and numeric conversion after loads remain TODO. Source-only; no build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.


Transform MESH receipt and C-conversion order follow-up (2026-11-05): each
executed MESH record now records its exact mode/architecture selection in a
short telemetry scope, so Python iterator and numeric callbacks run outside
that scope. After source loading, the facade converts the box coordinates,
resample integer, and coefficient doubles in Pillow's parser order. Source
review confirms destination clipping retains the transform's clipped local
origin. Callback-driven source-mode mismatch, exact callback/fault/output
parity, and selection-receipt parity remain TODO under `pillow-rs#3ptb`. No
validation ran. Sources: [Pillow 12.2 Image.transform](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2744-L2838),
[`__imaging._transform`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L1970-L2026), and
[Geometry generic transform](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Geometry.c#L720-L779).


Rotate matrix source follow-up (2026-11-05): the Python helper returns a built-in
six-tuple. The binding converts its values after source loading, in field order,
using PyO3's `f64` extraction (`PyFloat_AsDouble`), matching Pillow's fixed-size
`_imaging.getlist` path. The callback-sensitive source replacement/mutation
contract and output/fault receipts remain under `pillow-rs#3ptb` for parity.
Source-only; no build, formatting, checkstyle, lint, tests, parity, benchmark, or
coverage ran. Sources: [Pillow 12.2 `Image.__transformer`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2760-L2838)
and [`_imaging.getlist` / `_transform`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L386-L449).


Backend/Architecture routing audit (2026-11-05): direct CPU terminal kernels use
the Image-resolved context for `prepare_*` selection; lazy graph operations use
registry preparation; identity/copy routes have no pixel kernel; empty DrawText
returns before strategy selection. Active CPU strategy scans found no imports of
`pool_cpu::legacy` or `pool_simd`, no Rayon calls, and no production context
construction. Runtime receipt coverage and the full owner-declaration audit remain
open. GPU GetData terminal eligibility remains an explicitly deferred GPU task.
No build, formatting, checkstyle, lint, test, parity, benchmark, or coverage ran.

Convert matrix-to-LAB source route (2026-11-05): the 12-coefficient matrix
operation now declares LAB for each exact source-mode strategy and feeds the
matrix-transformed RGB samples directly into the LAB kernel without allocating
an intermediate RGB raster. The RGB source's support declaration classifies
LAB as CoreOnly. The One source keeps its existing L normalization
for Pillow-supported matrix targets and bypasses it for this Rust-only LAB
extension. A four-coefficient LAB matrix is rejected with a target-specific
error. Output/fault and selected Default/fallback receipt parity, plus the new
extension's contract, remain TODO under `pillow-rs#3ptb`. Source-only; no build,
formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.

Draw constructor mode-input ordering (2026-11-05): the Rust binding now accepts
the raw mode object and loads the image before attempting string extraction or
mode lookup; invalid type/name inputs return the existing `mode mismatch`
ValueError. The Python facade now distinguishes `None` from other falsey mode
values rather than truth-testing them before the native constructor. Callback,
fault, and direct-binding parity remain TODO under `pillow-rs#3ptb`. Source-only;
no build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.
Source: [Pillow 12.2 ImageDraw constructor](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L2289-L2307).

Draw text-bounds stroke ordering (2026-11-05): `textbbox` and
`multiline_textbbox` now resolve the default/custom font and validate embedded
color before testing `stroke_width` for truthiness. Falsey values become the
default zero width; truthy values remain unconverted until the font call,
matching Pillow's `ImageText.Text` route. Custom fonts receive the original
truthy width. Callback/fault/output parity for unusual stroke objects and
custom fonts remains TODO under `pillow-rs#3ptb`. Source-only; no build,
formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran. Sources:
[Pillow 12.2 ImageDraw](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L3464-L3593)
and [ImageText](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageText.py#L1405-L1455).

Native-float ImageOps fill adapter (2026-11-05): RGB32F/RGBA32F geometry fills
now use a mode-specific host parser. Integer scalars preserve the i64 payload
when possible and use Python's float conversion only when they exceed it;
numeric tuples convert each component in order through PyO3's f64 extraction,
and conversion failures propagate instead of being discarded as `Invalid`.
Lists remain rejected for these modes. The existing native-float Expand/Pad
strategies receive the resolved values without byte projection. Source-only;
no build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.

ImageOps.pad source-order review (2026-11-05): the public wrapper now matches
Pillow's sequence: `contain`, the no-padding early return, canvas construction
through `Image.new`, palette copy, centering access only for the padded axis,
then paste. This confirms the wrapper orchestration and does not close
allocation-before-fill fault order inside `Image.new`; custom callback,
output/fault, and operation-selection receipts remain in CPU parity. Source-only;
no build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage
ran. Source: [Pillow 12.2 ImageOps.contain/pad](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L2107-L2285).

Draw Bitmap coordinate adapter (2026-11-05): the PyO3 binding now defers
coordinate parsing until after bitmap loading and fill resolution, then accepts
flat pairs and one nested list/tuple pair. It retains numeric conversion errors
and the one-point count contract. Sequence-backed `Path` objects use that same
path. The source follow-up below adds Pillow's float-buffer and integer
narrowing routes. Exact callback and error parity remains pending. Source-only;
no build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage
ran. Sources: [Pillow 12.2
ImageDraw.bitmap](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L2465-L2482)
and [Pillow path flattening](https://github.com/python-pillow/Pillow/blob/12.2.0/src/path.c#L1632-L1914).

Draw Bitmap float-buffer and integer conversion follow-up (2026-11-05):
coordinate input now first requests Pillow's simple buffer view and reads one
native `float32` pair; a failed buffer request is cleared before sequence
fallback, and incomplete trailing bytes are ignored when counting pairs.
Sequence integers now use the platform C `long` range and narrow through
`float32`; nested tuple components retain their separate double conversion.
The repository has no native `PyPath` binding; incoming path objects use their
public sequence protocol, which exposes the same flattened coordinate pairs.
Only Pillow's private path-copy shortcut is absent. TODO(pillow-rs#3ptb):
verify output, fault, callback, and selected-route parity for buffers, integer
limits, and path-like inputs. Source-only; no build, formatter, checkstyle,
lint, tests, parity, benchmark, or coverage ran. Source: [Pillow 12.2 path
flattening](https://github.com/python-pillow/Pillow/blob/12.2.0/src/path.c#L1632-L1914)
and [Path sequence methods](https://github.com/python-pillow/Pillow/blob/12.2.0/src/path.c#L2497-L2529).

Draw Bitmap validation order follow-up (2026-11-05): the binding now validates
and resolves the mode-specific fill after loading the mask but before parsing
`xy`, then enters the core mask and strategy path. This preserves Pillow's
ink-before-coordinate-before-native-mask order for invalid fill/coordinate
pairs. Exact callback and output/fault receipts remain TODO. Source-only; no
validation ran.

Draw text-bounds coordinate deferral (2026-11-05): the PyO3 `textbbox` and
`multiline_textbbox` bindings now accept raw `xy` and extract the integer pair
after embedded-color validation and text input classification. This keeps an
invalid embedded-color mode ahead of a malformed coordinate, matching the
Python wrapper's ImageText construction and `embed_color()` order. Direction,
anchor, feature, alignment, fractional-coordinate, and exact callback/error
parity remain pending. Source-only; no validation ran. Sources: [Pillow 12.2
ImageDraw.textbbox](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L3464-L3524)
and [ImageText.Text](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageText.py#L1332-L1455).

Draw ink and eager Bitmap route source review (2026-11-05): color strings now
resolve against the selected drawing context mode, including the explicit RGBA
context over an RGB image. Text resolves fill and conditional stroke ink before
`Text._split` calls font metrics, then passes normalized native numeric inks to
per-line execution and synchronizes palette mutation if layout or rendering
fails. Public and direct PyO3 Bitmap routes load the mask, resolve fill, parse
coordinates, and then enter core mask validation and prepared exact-mode
selection; RGB/CMYK eager routes remain behind that selection. This closes the
source-order review only. Text/Bitmap pixels, callbacks, faults, metadata, and
selected-route receipts remain for the frozen CPU parity gate. No build,
formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran. Sources:
[Pillow 12.2 ImageDraw](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L2395-L2439)
and [ImageColor.getcolor](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageColor.py#L1017-L1078).

Operation support declaration trace (2026-11-05): a source-only scan found 103
`OperationId` variants, all routed by its exhaustive support match. The
`PipelineOp::operation_id()` match maps 75 distinct identities; the 28 direct
or eager identities are `PointScalar`, `PointFloat`, `ImageOpsInvert`,
`UnsharpMask`, `ModeFilter`, `LinearGradient`, `RadialGradient`,
`EffectMandelbrot`, `PutPixelBatch`, `PutPixelValueAt`, `PutDataL16Bytes`,
`PutDataValueAt`, `GetBBox`, `Histogram`, `Entropy`, `GetColors`, `Quantize`,
`Stat`, `GetExtrema`, `GetExtremaBytes`, `Split`, `GetProjection`, `GetData`,
`GetDataBand`, `GetDataBandInto`, `GetPixel`, `GetPixelRgba`, and `GetChannel`.
`PipelineOp::support()` delegates through that identity; prepared plans expose
the support declaration selected by their concrete `StrategySelection`.
`OperationSupport::modes()` and `routes()` enumerate `Mode::ALL`, and the
resulting `ModeSupport` carries exact output, input, Default, and architecture
declarations. This proves descriptor coverage and the support API's complete
mode enumeration, not that every direct/eager caller consumes its declaration
on all paths. That caller-level source audit remains pending under
`pillow-rs#3ptb`; no build, formatter, lint, test, parity, benchmark, or
coverage ran.

Operation support census follow-up (2026-11-06): the module-level FloodFill
operation adds a 104th `OperationId` and a 29th direct/eager identity;
`PipelineOp::operation_id()` remains at 75 because FloodFill is not a pipeline
node. The November 5 census above remains a dated snapshot. Source-only; no
verification has run.

ImageText thin-host correction (2026-11-05): all Unicode/byte `Text.wrap`
paths now use the `pillow-rs` wrapping state machine, including custom font
metrics, custom numeric comparisons, string/bytes subclasses, path-backed
font scaling, and shrink/grow choices. Python retains wrapper preflight,
scaling tuple unpacking, host protocol calls through PyO3 callbacks, and
mutation of the public `Text`/remainder wrappers. It no longer implements
tokenization, line formation, height placement, or scaling loops. Custom-font
`_split` placement/alignment and `get_bbox` aggregation still execute in
Python and are explicitly tracked as migration debt under `pillow-rs#3ptb`.
The shared `ImageFontTextInput` now preserves lone surrogates as Unicode
codepoints across ordinary font, Draw, multiline layout, and Text.wrap routes;
Python reconstructs the original string only at host callback boundaries.
Output and fault parity remain pending. No build, formatter, checkstyle, lint,
tests, parity, benchmark, or coverage ran.

ImageText bounds aggregation correction (2026-11-05): custom-font and
overridden `_split` results now flow through a Rust-owned bounds traversal.
PyO3 advances the host iterator and adapts `_get_bbox`, coordinate addition,
and Python `min`/`max` comparisons; the Python facade no longer performs the
bounds loop. Custom `_split` placement/alignment remain explicit migration
debt; lone-surrogate output and fault parity remain pending. Source review
only; no build, format, checkstyle, lint, test, parity, benchmark, or coverage
ran.

ImageText placement correction (2026-11-05): the custom-font `_split`
placement state machine now runs in `pillow-rs` too. The core owns line
selection, early return, anchor and direction branches, line spacing,
horizontal and top-to-bottom placement, alignment, justification, and the
order of numeric operations. PyO3 adapts Python sequence iteration, font
metrics, string and numeric protocols, and construction of the existing
`_Line` value. Bounds offset addition now also runs in core; its adapter calls
`_get_bbox`, retrieves host coordinates, and applies Python arithmetic and
comparisons when requested by the core traversal. The Python `_split` method
is now a thin fast-path selector and core-call adapter. Source-only review;
parity and callback/fault order are pending. Python wrapper preflight and
optimization predicates remain migration debt.
No build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage
ran.

ImageOps Expand orchestration follow-up (2026-11-05): `pillow-rs` now owns the
callback-driven order from custom border geometry through fill parsing,
palette copying and tuple color resolution, the second mode read, size checks,
typed mode selection, native fill preparation, palette resolution, canvas
creation, palette propagation, public `paste`, and `load`. Python and PyO3
adapt host values and preserve when property/method lookup, tuple construction,
and the output wrapper constructor occur. Pure Rust validation, fill
preparation, and canvas phases release the Python runtime lock. The Python
facade delegates directly; the PyO3 adapter moved to
`pillow-rs-py/src/image_ops/expand.rs` to keep source files below the size cap.
Source review only. Expand output, callback/fault order, and mode/architecture
receipts remain pending; no build, formatter, checkstyle, lint, tests, parity,
benchmark, or coverage ran. Remaining ImageOps composition debt includes the
custom Pad fallback, and the facade's native-versus-callback route predicates
remain migration debt.

ImageOps Pad fallback follow-up (2026-11-05): the core now owns the
custom-protocol fallback sequence: call the current public `contain` function,
return its result when its size matches, otherwise create the output with
`Image.new`, propagate a present palette, choose the horizontal or vertical
centering branch, compute its offset through host numeric protocols, and call
public `paste`. PyO3 resolves Python globals and preserves property, operator,
and method evaluation order; the Python facade delegates after the existing
native-route admission. The fallback composition and centering algorithm no
longer live in Python. Source review only. Route-selection predicates and
Pad output/fault/selection parity remain open; no build, formatter, checkstyle,
lint, tests, parity, benchmark, or coverage ran.

ImageOps native-size route selection follow-up (2026-11-05): the common
`_native_size_pair` eligibility sequence now runs in `pillow-rs` through an
ordered host-fact callback contract. Core short-circuits instance eligibility,
default attribute access, base size/width/height descriptors, exact tuple
shape, exact integer types, and u32 range before returning native dimensions.
The Python helper now delegates to that selector. The remaining facade route
selection is not complete: Pad-specific eligibility checks and the final
native-versus-callback branch, plus Scale and Crop fast-path predicates, still
need core-owned dispatch. No build, formatter, checkstyle, lint, tests, parity,
benchmark, or coverage ran.

ImageOps Contain/Cover route consolidation (2026-11-05): both operations now
enter one Rust-owned native-versus-callback strategy selector. The core
short-circuits host capability checks, runs the exact-image geometry path when
eligible, otherwise performs custom Python arithmetic through the existing
callback strategy, and returns the selected size. The Python facade no longer
branches between geometry implementations and still dispatches through public
`Image.resize`. Fit, Pad-specific admission, Scale, and Crop route predicates
remain source migration debt. This is source-only; no build, formatter,
checkstyle, lint, tests, parity, benchmark, or coverage ran.

ImageOps Crop route consolidation (2026-11-05): the core now owns the ordered
native-versus-callback decision for Crop. PyO3 reports host facts one at a time
so the selector preserves short-circuit order for the exact base Image type,
base size/crop methods, instance overrides, and the exact integer border range.
The native geometry result still flows through public `Image.crop`; custom
borders still use Python protocol callbacks. Source review also found and
corrected missing Crop callback adapters and redundant `?` operators around
Rust-only callback phases. Scale, Fit, and Pad-specific route selection remain
unmigrated. No build, formatter, checkstyle, lint, tests, parity, benchmark,
or coverage ran.

ImageOps Fit route consolidation (2026-11-05): the core now selects the native
geometry or custom-protocol strategy after the shared native-size admission.
The Python facade delegates the box calculation and retains the public
`Image.resize` call. PyO3 adapts the typed numeric host path and the custom
centering unpack protocol. Fit's callback and route strategies now live in
their own `geometry/fit_callbacks.rs` module to keep shared geometry callbacks
focused and leave file-size headroom. Scale and Pad-specific route selection
remain. This is source-only; no build, formatter, checkstyle, lint, tests,
parity, benchmark, or coverage ran.

ImageOps Scale route consolidation (2026-11-05): the core now handles the
factor-one copy path, native-versus-callback geometry choice, and final public
resize dispatch. PyO3 adapts host comparison, capability, numeric conversion,
rounding, and wrapper-method calls. Scale callbacks and route selection now
live in `geometry/scale_callbacks.rs`. The Python facade delegates directly;
Pad-specific admission and composition remain the ImageOps route debt. This is
source-only; no build, formatter, checkstyle, lint, tests, parity, benchmark,
or coverage ran.

ImageOps Pad route consolidation (2026-11-05): the core now combines native
size admission with Pad-specific host facts and selects the typed Pad or
custom-protocol fallback strategy. The facade delegates through one core
entry point. PyO3 adapts exact-type and wrapper-state checks, runs the existing
typed Pad preflight, realizes dirty P/PA palette state at the prior point,
adapts fill and centering, and preserves public result-wrapper construction.
The custom fallback retains its existing public `contain`, `Image.new`,
palette, centering, and `paste` callback order. Source review only; output,
fault-order, and mode/architecture receipt parity remain pending. No build,
formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.

ImageDraw custom-font text-bounds migration (2026-11-05): Python no longer
calculates custom-font line positions or aggregates line bounds. Its facade
delegates to a PyDraw callback adapter; `pillow-rs` runs the shared line-layout
and bounds strategies using the typed drawing-context Mode. PyO3 adapts the
custom font's `getbbox`/`getlength` methods, host sequence access, numeric
operators, and comparisons. The adapter preserves the custom Draw path's
ordering by measuring line spacing before indexing `xy`, then recomputing its
bilevel-first font mask mode before per-line bbox calls. Source review only;
custom-font output across Draw modes and callback/fault order remain pending
under `pillow-rs#3ptb`. No build, formatter, checkstyle, lint, tests, parity,
benchmark, or coverage ran.

ImageDraw float-text-fill migration (2026-11-05): F-mode font-mask ink now
uses the typed drawing-context Mode in `pillow-rs` to reinterpret the packed
little-endian i32 carrier as f32. Python no longer branches on the Pillow mode
string or packs/unpacks the value with `struct`; non-F modes retain the integer
ink. Source review only; verify F ink bits and Draw output in CPU parity. No
build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.

ImageDraw custom-font mask-mode selection (2026-11-05): stroke and fill mask
modes now come from pillow-rs using the typed drawing-context Mode. The bbox
path checks bilevel modes before embedded-color truthiness; the mask-rendering
path checks embedded color first. Python no longer repeats Pillow mode string
checks and still adapts custom font methods. Source review only; freeze all
Draw-mode mask outputs in CPU parity. No build,
formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.

ImageDraw direct-text route selection (2026-11-05): the ordered fast-path
predicate now runs through a core selector using the typed drawing-context
Mode. PyO3 adapts Pillow's module imports, spacing read, exact-type checks,
numeric bounds, and host values in short-circuit order. Python no longer owns
the duplicate `_can_delegate_text_layout` predicate. Exact float coordinates
remain eligible, including values whose later native conversion can fail, as
in the prior route check. Ink and palette callbacks still happen before this
selector and before `ImageText._split`. Source review only; route and callback
fault parity remain pending. No build, formatter, checkstyle, lint, tests,
parity, benchmark, or coverage ran.

ImageDraw custom-font mask strategy (2026-11-05): `pillow-rs` now owns the
ordered `getmask2` → AttributeError fallback → `getmask` → TypeError fallback
sequence, offset application, empty-mask check, image-or-bytes adaptation
strategy, and RGBA-versus-bitmap composition choice. PyO3 adapts Python numeric
conversion, font calls, error classification, mask/image methods, and the
existing ImageDraw wrapper callbacks. The Rust-owned line strategy now calls
this adapter after route selection. Ink and palette resolution still precede
`ImageText._split`. Source review only; per-line route selection and iteration,
Draw-mode output, and callback/fault parity remain pending. No build,
formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.

ImageDraw split-line routing and iteration (2026-11-05): the core callback
strategy now owns the ordered direct-route choice, `_split` invocation,
post-split font capability checks, per-line iteration, custom stroke/fill
selection, and final fallback route. Python retains ink resolution and
palette allocation before entering the strategy. PyO3 adapts host iteration,
font attributes, Python conversions, and calls into the existing mode-aware
Draw paths. Source review only; direct/custom/fallback output, callback and
fault order, and Draw-mode parity remain pending. No build, formatter,
checkstyle, lint, tests, parity, benchmark, or coverage ran.

ImageDraw custom bitmap callback boundary (2026-11-05): source review found
that the custom-font mask adapter calls `owner.bitmap` while
`PyDraw._draw_text_lines` still holds a mutable borrow of the same PyDraw.
That re-entry can fail at the PyO3 borrow boundary before Pillow's bitmap
load/fill/palette/synchronization behavior runs. Split the custom bitmap action
across the binding boundary so Python overrides still run between line steps;
preserve the current per-line callback and synchronization order. This source
path is explicitly pending under `pillow-rs#3ptb`; no runtime validation ran.

ImageOps Autocontrast final routing (2026-11-05): typed `Mode` from the current
wrapped raster now selects Pillow's P rejection and L/RGB acceptance after
histogram/LUT work. Rust also owns RGB's single-channel LUT replication and
the final order before calling public `Image.point`. PyO3 adapts host `len`,
sequence multiplication, unsupported-mode formatting, and `point`; Python no
longer branches on mode. The per-channel histogram slicing and ignore/cutoff
iteration remain a source TODO in `imageops.py`. Source-only; mode, output,
fault, and callback parity remain pending. No build, formatter, checkstyle,
lint, tests, parity, benchmark, or coverage ran.

ImageOps Autocontrast channel orchestration (2026-11-05): core now owns output
LUT creation, range-driven channel iteration, host slicing, per-channel ignore
iteration and assignment, cutoff truthiness and tuple normalization, channel
LUT requests, and LUT extension. PyO3 adapts Python's dynamic names, iterator,
slice, sequence, and numeric operations; the existing Rust cutoff arithmetic
remains the LUT implementation. Python retains the source histogram and
preserve-tone conversion calls, then delegates channel processing and final
mode routing. This removes the facade's channel loop and its source TODO.
Source-only; callback/fault order, exact output, mode, and architecture results
remain unverified. No build, formatter, checkstyle, lint, tests, parity,
benchmark, or coverage ran.

ImageDraw custom-font bitmap borrow correction (2026-11-05): `_draw_text_lines`
now takes an owned `Py<PyDraw>` handle and borrows it only for the mode read or
individual direct Rust draw calls. The custom-mask adapter retains that handle
without holding a `Draw` borrow; RGBA composition borrows only around its
native operation, while bitmap composition calls the dynamic public
`owner.bitmap` method with no active PyDraw borrow. This preserves subclass
dispatch and the bitmap method's per-call load, palette, mutation, and sync
sequence while allowing the callback to re-enter PyDraw safely. The source TODO
is removed. Source review only; Draw output and callback/fault order remain
unverified. No build, formatter, checkstyle, lint, tests, parity, benchmark,
or coverage ran.

ImageOps Equalize mode routing (2026-11-05): the facade no longer checks the
Pillow string `image.mode`. Core reads the stored raster Mode and uses
Equalize's declared fixed-RGB output rule to invoke public `Image.convert("RGB")`
before histogram collection for P. Other modes still collect their public
histogram before the LUT binding reports mode errors. PyO3 adapts histogram,
the existing native LUT entry point, and public `Image.point`; the algorithm
is unchanged at this snapshot. Its statement that the binding narrowed
histogram objects to `Vec<u32>` is superseded by the next Equalize
histogram-protocol entry, which records the generic host callbacks and guarded
native fast path. Source-only; output, mode-transition, and fault-order results
remain unverified. No build, formatter, checkstyle, lint, tests, parity,
benchmark, or coverage ran.

ImageOps Colorize routing and point validation (2026-11-05): Python no longer
checks `image.mode` or implements the point-range assertion chains. Core uses
the typed Colorize support contract first, then invokes host rich comparisons
in Pillow's short-circuit order, requests the existing color LUT, and calls
public `Image.convert("RGB")` and `Image.point`. PyO3 retains arbitrary color
and numeric protocols; the Rust LUT implementation is unchanged. Source-only;
error order, output, and Convert/Point strategy receipts remain unverified.
No build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.

ImageOps Autocontrast source selection (2026-11-05): the facade now delegates
the full wrapper flow to one core strategy. Rust evaluates host truthiness for
`preserve_tone`, selects whether to call public `convert("L")`, then sequences
public histogram collection, per-channel LUT construction, typed mode
admission, and public `Image.point`. The adapter retains the Python protocols
and overridable method calls. Source-only; callback/fault order, exact output,
mode, and architecture results remain unverified. No build, formatter,
checkstyle, lint, tests, parity, benchmark, or coverage ran.

ImageOps Equalize histogram protocol (2026-11-05): core now owns per-channel
iteration, nonzero filtering, step calculation, cumulative LUT construction,
and typed mode admission after the public histogram callback. The adapter
retains Python slicing, iteration, truthiness, `len`/`sum`/`range`/`list`,
arithmetic, and indexing protocols for arbitrary histogram objects. Exact
lists of exact integers use the native u32 LUT kernel when each channel's total
fits u32; the Rust binding does not extract arbitrary histograms to `Vec<u32>`.
This path does not inspect or emulate replaced builtins, which are outside the
supported compatibility contract. Source-only; callback/fault order, output,
mode, and architecture results remain unverified. No build, formatter,
checkstyle, lint, tests, parity, benchmark, or coverage ran.

2026-11-21 Equalize protocol source review: compared the active callback state
machine with Pillow 13.0.0 `ImageOps.equalize`. Pillow builds each channel's
nonzero list by iterating the sliced channel, then uses host `len`, `sum`,
`range`, list extension, and indexed reads while constructing the LUT; the
generic adapter path preserves those operations through callbacks. The native
`Vec<u32>` path requires exact integer lists and per-channel totals fitting
u32. Builtin replacement, including replacements that spoof builtin metadata,
is outside the supported compatibility contract; the implementation does not
inspect builtin identities or add replacement-specific fallbacks. Wrapper
output, callback/fault order, selected/fallback receipts, and latency remain
pending the frozen CPU gates. Source review only; no build,
formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran. Source:
[Pillow 13.0 ImageOps.equalize](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/ImageOps.py#L2622-L2674).

ImageOps Pad native-route orchestration (2026-11-05): the core route now owns
the ordering from admitted native size through contain geometry, filter
validation, dirty palette realization, fresh-raster capture, typed fill
preparation, conditional centering adaptation, and mode-aware Pad execution.
The PyO3 adapter supplies Rust snapshots and individual host operations, then
constructs the contained or padded wrapper result. This removes the grouped
native preflight algorithm from the binding; native-route host eligibility
facts remain at the boundary. Source review only; output, callback/fault order,
and architecture receipts remain unverified. No build, formatter, checkstyle,
lint, tests, parity, benchmark, or coverage ran.


ImageOps Colorize callback strategy update (2026-11-05): `colorize/ops.rs`
now owns mode admission, point-comparison order, color-channel list creation,
range traversal, interpolation sequencing, final LUT assembly, and public
Convert→Point orchestration. PyO3 implements only Python value, range,
sequence, indexing, and arithmetic operations. The narrowed range-length
binding and its arbitrary-color-sequence limitation were removed; the unused
range-length LUT API was deleted. Source review only. Colorize output, error
ordering, and mode/architecture receipts remain pending. No build, formatter,
checkstyle, lint, tests, parity, benchmark, or coverage ran.

ImageFont public text behavior (2026-11-05; source-confirmed 2026-11-06):
Pillow 12.2's BASIC FreeType path reads each Python Unicode codepoint and passes
it directly to FreeType; the RAQM path copies Python Unicode to UCS4 and passes
that to `raqm_set_text`. Neither path UTF-8 encodes the text first. Pillow's
ImageDraw passes its `str` lines to FreeType, and `ImageText.Text.wrap` measures
those same Python strings through font bounding boxes. Therefore unpaired
surrogates must be preserved as codepoints across the shared
`ImageFontTextInput` FreeType, Draw, multiline, and wrap paths. The Python API
continues to take and return its existing `str` / `bytes` values. The classic
bitmap PILfont is a separate public path with Latin-1 byte-table semantics via
`PilFontTextInput`; it retains its own encoding error contract and does not use
the FreeType carrier. The borrowed Rust `&str` multiline-bounds route uses the
same layout without copying the full input or each line. Runtime output,
callback order, and fault parity remain unverified. No build, formatter,
checkstyle, lint, tests, parity, benchmark, or coverage ran. References:
[Pillow 12.2.0 ImageFont](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageFont.py),
[ImageDraw](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py),
[ImageText](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageText.py),
and [FreeType layout](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imagingft.c).

LA AlphaComposite lane provenance (2026-11-05): Pillow 12.2 reinterprets LA
pixels through its four-byte RGBA pixel structure while compositing. The LA
CPU strategy now propagates both omitted color lanes with the same fixed-point
formula after the selected Default or SIMD kernel; canonical repeated-luma
lanes remain implicit to avoid an output sidecar. This closes one producer gap
for later Histogram and raw-byte operations. Source-only; the producer audit
and parity remain pending. No build, formatter, checkstyle, lint, tests,
parity, benchmark, or coverage ran. Reference: [Pillow 12.2.0
AlphaComposite.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/AlphaComposite.c#L394-L493).

Merge physical-lane provenance (2026-11-05): Pillow 12.2 `ImagingMerge`
writes zero into omitted lanes for two-band LA/La/PA and three-band RGB-family
outputs. The shared CPU Merge interleaver now attaches that constant zero
payload for generic interleaving; the dedicated RGB Default, SSSE3, and NEON
paths use the same attachment helper after their optimized interleaves. This keeps later masked
Histogram and raw-byte operations aligned with Merge's public result. Other
producer paths and parity remain pending. No build, formatter, checkstyle,
lint, tests, parity, benchmark, or coverage ran. Reference: [Pillow 12.2.0
Bands.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Bands.c#L1209-L1334).

GetData execution source audit (2026-11-06): `GetData`, `GetDataBand`, and
`GetDataBandInto` declare the 25 exact mode contracts and caller-validated
band counts; each materialized request verifies that its prepared selection
matches the `DynamicImage` mode before dispatching to that mode's owner. The
caller-owned output path is a serial Default strategy. The legacy `simd`
request is translated by the routed context to CPU plus its selected host
architecture; loaded-image accelerated band reads prepare the declared
`ExtractBand` child, and unsupported requests fall through to the declared
`GetDataBandInto` Default strategy. GPU eligibility remains a separate manual
predicate to move into GPU support in its later stage. This closes the source
trace for these CPU entry points only; output, fault, buffer, and receipt
parity remain pending. No build, formatter, checkstyle, lint, tests, parity,
benchmark, or coverage ran.

Effects execution source audit (2026-11-06): EffectSpread and EffectNoise
support dispatch is exhaustive across the 25 `DynamicImage` variants, and
their prepared enums retain each exact mode owner's selection through
execution. LinearGradient and RadialGradient expose the five supported output
modes at their operation contracts; Mandelbrot declares its fixed-L result
and guards its architecture selection from typed extent/quality inputs.
Pipeline operations use the same prepared mode/architecture path. Eager
generators resolve the compatibility `simd` request once into CPU plus the
detected architecture, and report CPU as the executing backend. This traces
source routing and receipt ownership only; generator pixels, RNG state, fault
order, and architecture receipts remain parity work. No build, formatter,
checkstyle, lint, tests, parity, benchmark, or coverage ran.

ImageDraw public signature correction (2026-11-06): compared the maintained
Python facade against Pillow 12.2 `ImageDraw.py`. `Draw.textbbox` and
`Draw.textlength` now expose Pillow's explicit positional parameters and
keyword-only `font_size`, so positional arguments are accepted in the same
slots and unknown keywords are rejected instead of silently ignored. `Draw.text`
keeps Pillow's `*args, **kwargs` surface; `font_size` is read from `kwargs` for
default-font selection and remains in the mask-call kwargs as in Pillow. This
is source alignment only; public-call, callback, exception, and signature
inspection parity remain unverified. No build, formatter, checkstyle, lint,
tests, parity, benchmark, or coverage ran.

ImageDraw font dispatch and mutable font mode correction (2026-11-06): Pillow
12.2 calls `ImageText._split`, then invokes the font's public `getmask2` method
for each line, falling back to public `getmask` only when `getmask2` raises
`AttributeError`; a full `getmask` call falls back to `getmask(text)` only on
`TypeError`. The Draw wrapper no longer probes mask support with `hasattr` or
uses `_rust_font` to bypass those methods after splitting. Its whole-call Rust
fast path now requires the unmodified built-in Text and FreeType methods, plus
the mode-derived default `fontmode`; otherwise callbacks preserve the public
Python method route. `Draw.fontmode` is initialized and its assigned value is
passed through mask selection, and `Draw.font` is honored by `getfont`.
`textbbox` now creates `ImageText.Text` and calls its public `get_bbox`, leaving
fast-path selection and callback fallback with that strategy. `Draw.mode` is
carried as public state and used when creating Text and resolving color names.
The Python Draw object now carries Pillow's mutable `ink`, `fill`, and `palette`
attributes. Its `_getink` follows Pillow's default outline/fill selection and
packs explicit colors once; the Rust binding decodes packed inks into the
mode-specific channel carrier before strategy execution. Palette tuple lookup
uses the Draw object's current palette. The facade exposes Pillow's
`ImageDraw` class and `Draw` factory, including the virtual `im.getdraw(mode)`
route. Four-corner rounded rectangles use a core-owned ordered shape program.
The Rust preparation strategy now owns coordinate comparisons, radius
multiplication, ordered rounding, full-axis clipping, early-route selection,
and radius flooring. PyO3 invokes the requested Python numeric protocols and
returns their original values without narrowing. Pillow corner reads for a
curve group complete before selected arcs or pieslices draw; each primitive is
synced before the next request. Rust now owns outline border formulas and the
width-enabled decision, while the facade supplies custom numeric and corner
truth results and dispatches the public `ellipse`/`rectangle` overrides.
Border corner adjustments use Python's in-place numeric protocol for `+=` and
`-=` so host numeric objects retain their operator behavior. Sequence unpacking
also remains at the Python boundary. The draw-step adapter now parses arc
angles, color, and signed width before converting generated bounds, matching
Pillow's native argument parser before `PyPath_Flatten`; pieslice angles and
color likewise precede bound conversion. TODO `pillow-rs#3ptb`: ordinary
bounds compare as doubles before narrowing, matching Pillow's
`_draw_rectangle`; review non-finite and out-of-range C double-to-int casts,
whose result is not portable, then complete rounded-rectangle output,
callback, mutation, and fault parity. Include malformed generated bounds paired
with invalid arc angle/width values to lock down the corrected parser order.
Pillow
12.2 performs new indexed reads for fill curves, fill side rectangles, outline
curves, and outline borders; see [its rounded-rectangle
implementation](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L374-L502).
`ImagePath.Path` inputs and
ImageDraw2 transformation are implemented at source level; their
sequence, method-fault, buffer, mapping-mutation, and output parity remains
pending. `floodfill(image, xy, value, border=None, thresh=0)`
now has a typed FloodFill support descriptor and CPU Default strategy. Its
Python facade keeps Pillow's signature, load/assert/unpack order, while Rust
owns the four-neighbor traversal and calls the supplied PixelAccess protocol.
Traversal retains Pillow's previous-edge set behavior, performs membership
before negative-coordinate comparisons, and computes color differences over
`range(len(second))` with host indexing and arithmetic. Callback, output, and
fault parity remain pending. TODO(pillow-rs#3ptb): add focused CPU parity cases
for output, callback order, mutation, and fault outcomes. The facade
exposes Pillow's assignable `draw` context and `im`
core-image attributes. Palette color resolution does not read `image.info`
before Pillow's `ImagePalette.getcolor` path does. `getdraw` and the ImageDraw2
module are now exposed through the PIL namespace. Shape close callbacks run
before color conversion, and regular polygon geometry runs before overridable
`polygon()` dispatch. Public signatures, mutable-state behavior, callbacks,
output, and fault order still need focused CPU parity. No
build, formatter, checkstyle,
lint, tests, parity, benchmark, or coverage ran. Reference: [Pillow 12.2
ImageDraw constructor, `_getink`, and `Draw`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L2289-L2439).

Public font text input preservation (2026-11-06): Python continues to expose
Pillow's `str` and `bytes` inputs; `ImageFontTextInput` is the typed Rust
adapter contract between the binding and core. Python strings containing lone
surrogates are retained as Unicode codepoints and use the same core input path
as ordinary strings for FreeType metrics, mask rendering, Draw, multiline
layout, and `Text.wrap`.
Classic bitmap PILfont retains its separate byte-oriented contract. Public
method signatures, accepted `str`/`bytes` arguments, outputs, exceptions,
callback order, and mutable state remain the compatibility contract; this
internal representation adds no Python-facing input type. Pillow's FreeType
fallback uses `PyUnicode_READ_CHAR`, while RAQM builds a UCS-4 copy; these
source details are diagnostic evidence, not required implementation details.
Source review only; output and fault parity remain pending. No build, formatter,
checkstyle, lint, tests, parity, benchmark, or coverage ran. References:
[Pillow 12.2 `_imagingft.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imagingft.c#L380-L464) and [ImageText.py](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageText.py#L1207-L1310).

Transposed font delegation (2026-11-06): `TransposedFont.getmask` now calls
the wrapped font's public `getmask(text, mode, *args, **kwargs)` and transposes
that returned mask, preserving caller arguments and custom font overrides as
Pillow does. The Python wrapper no longer routes `FreeTypeFont` subclasses
around their public method. Source-only; callback and output parity remain
pending. No build, formatter, checkstyle, lint, tests, parity, benchmark, or
coverage ran.

Draw text API consolidation (2026-11-06): removed the unused lower-level
`_core.ImageDraw.text` and `multiline_text` methods and the separate direct
single-line helper. Repository call-site review shows the public Python
facade routes through `_draw_text_lines`; its built-in multiline optimization
remains behind the prepared route checks, while custom calls retain public
`ImageText.Text` and font callbacks. This removes an alternate internal entry
that skipped the public text wrapper. Source-only; CPU output, fault, callback,
and receipt parity remain pending. No build, formatter, checkstyle, lint,
tests, parity, benchmark, or coverage ran.

GetChannel selector source correction (2026-11-06): Pillow loads the image,
uses Python `getbands().index(channel)` for every `str` instance, then passes
other values to the native C-int parser. The facade now keeps string lookup at
that Python protocol boundary, preserving overridden `getbands()` and
`str.__eq__` behavior; PyO3 uses the shared C-int conversion without
swallowing custom `__index__` errors. Input-only parity cases were added for
custom `ValueError` and `AttributeError` from `__index__`. String-subclass
comparison, exact native parser errors, image state, and selection-receipt
parity remain pending. Source-only; no build, formatter, checkstyle, lint,
tests, parity, benchmark, or coverage ran. Reference: [Pillow 12.2
`Image.getchannel`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2563-L2581).

Regular ImageDraw width source migration: `rectangle`, `ellipse`, `chord`, and
`pieslice` now stage fill and outline as separate native calls, compare an
active outline width after the fill has synchronized, and pass that width to a
signed C-int parser. Their Python signatures are unchanged. `circle` builds
the Pillow ellipse box and dispatches through overridable `self.ellipse`.
`polygon` now stages fill, one-pixel outline, and wide outline separately.
The wide path preserves Pillow's `width != 0`, `width == 1`, intermediate-mask
coordinate parse, Python `width * 2 - 1`, and signed C-int conversion order.
Rust stores that transformed width in `DrawPolygon` and builds the binary mask
inside the selected serial or SIMD strategy, without converting the image or
materializing a second Pillow image. `regular_polygon` still delegates to
`polygon`. Pillow's `_draw_polygon` binding rejects fewer than two vertices
with `TypeError` before calling `ImagingDrawPolygon`; the lower-level
degenerate one-point outline is not reachable through this public call. The
existing parser retains that minimum. Source-only; verify output, mutation,
callback, and fault ordering for every affected width path in the CPU gate,
including the minimum-vertex error. References: [Pillow 12.2
`ImageDraw.py`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L2484-L2558), [polygon](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L2766-L2805), and [rectangle](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L2830-L2854).

FreeTypeFont public mask dispatch (2026-11-06): `FreeTypeFont.getmask` now
returns `self.getmask2(...)[0]`, matching Pillow's public call chain so
subclass overrides of `getmask2` remain observable. The Python signature is
unchanged. Source-only; callback, return-value, and error parity remain
pending. No build, formatter, checkstyle, lint, tests, parity, benchmark, or
coverage ran. Reference: [Pillow 12.2 `FreeTypeFont.getmask`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageFont.py#L2854-L2991).

Draw bounding-box input source continuation (2026-11-06): rectangle, ellipse,
arc, chord, pieslice, and the normalized rounded-rectangle route now use the
shared PyPath-style host flattener. Core retains four `f64` coordinates until
it checks `x1 >= x0` and `y1 >= y0`, then converts to integer raster
coordinates. This preserves float ordering that was previously lost by
host-side integer extraction. Path/buffer/sequence output and callback/fault
parity remain pending. The common C double-to-int conversion is undefined for
non-finite and out-of-range values; that output edge remains TODO under
`pillow-rs#3ptb`. No build, formatter, checkstyle, lint, tests, parity,
benchmark, or coverage ran. Reference: [Pillow 12.2 `_draw_rectangle`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L3233-L3280) and [`PyPath_Flatten`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/path.c#L148-L252).

Draw coordinate subclass correction (2026-11-06): the shared PyPath adapter
now recognizes Python `float` and `int` subclasses before its numeric fallback,
matching Pillow's `PyFloat_Check`/`PyLong_Check` route. Integer subclasses retain
the C-long then float32 narrowing used by Pillow. Public Python signatures and
accepted input types are unchanged. Subclass conversion callbacks, overflow,
output, and fault parity remain pending; no build, formatter, checkstyle, lint,
tests, parity, benchmark, or coverage ran. Reference: [Pillow 12.2
`PyPath_Flatten`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/path.c#L103-L146).

ImageDraw.line source continuation (2026-11-06): the public signature remains
`(xy, fill=None, width=0, joint=None)`. The binding now parses the signed C-int
width before flattening the base coordinates. The facade syncs the base line
before comparing the original `joint` and `width`, and groups the original
coordinates only after those checks. Rust provides shared joint geometry; the
facade calls public `self.pieslice` and `self.line` methods with the original fill, preserving
subclass overrides, palette re-resolution, and their order. Bounds and width
operations use the original Python values at Pillow's call points. Angle and
gap-offset arithmetic also runs in Python against the original coordinates;
Rust selects the shared flipped-arc strategy from those angles. Freeze parity
cases for custom numeric coordinates, overridden `pieslice`/`line` methods,
and repeated P-mode fill resolution. The public output, callback, mutation,
and fault contract remains unverified in CPU parity. Source-only; no build,
formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.
Reference: [Pillow 12.2
`ImageDraw.line`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L2560-L2700).

ImagePath assignment fault-order correction (2026-11-06): Pillow's `Path`
sequence setter checks the original index bounds before parsing the assigned
coordinate tuple and does not normalize negative assignment indices. Its
deletion slot performs the same bounds check before raising the deletion
TypeError. Assignment and deletion use the sequence index conversion rather
than the custom `__getitem__` type check. The Rust-backed `Path` adapter now
preserves those ordering and index rules. Output and fault parity remain
pending alongside buffer construction and map-callback mutation;
no build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage
ran. Reference: [Pillow 12.2 `path.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/path.c#L429-L449)
and [sequence slots](https://github.com/python-pillow/Pillow/blob/12.2.0/src/path.c#L550-L605).

ImagePath empty-pointer identity source correction (2026-11-06): Pillow's Path
allocator reserves coordinate storage for an empty path because its public
`id` exposes that storage address. Rust now retains a one-point allocation for
empty paths so `id` does not expose a dangling Vec sentinel. Pointer values are
process-local; parity remains pending. No build, formatter, checkstyle, lint,
tests, parity, benchmark, or coverage ran. Reference: [Pillow 12.2
`path.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/path.c#L48-L60)
and [`id`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/path.c#L544-L548).

Rounded-rectangle preparation moved to core (2026-11-06): Rust now sequences
the coordinate comparisons, `radius * 2`, four `round()` calls, `all(corners)`,
sequential axis clipping, early-route decisions, `any(corners)`, and floor-half
radius conversion. A typed callback contract invokes Python numeric protocols
without moving formula ownership back to the facade. Rust also constructs
outline border bounds and applies corner offsets, requesting Python arithmetic
and corner-index truth values in Pillow's order. The core program now waits
until fill drawing completes before requesting the `width != 0` callback, then
appends outline steps only when enabled. The facade passes host values and
executes requested primitives; C-int width conversion and low-level fault
timing remain source gaps under `pillow-rs#3ptb`. No build, formatter,
checkstyle, lint, tests, parity, benchmark, or coverage ran.

Rounded-rectangle width adapter cleanup (2026-11-06): removed the redundant
`_effective_outline_width` normalization and pass the original width through.
For active outlines it already returned the original value; for inactive
outlines the core suppresses the outline. This preserves negative widths for
the primitive path. The rounded-rectangle geometry-ownership and parity gaps
remain open.

Rounded-rectangle raw numeric retention correction (2026-11-06): preparation
and draw programming now retain host numeric objects instead of extracting
coordinates, diameter, or radius to `f64`/`i32` before geometry steps. The Rust
program requests Python operations as each Pillow expression is reached: it
builds each curve group before corner reads, waits for fill primitives before
constructing later fill geometry, and does not ask whether outline width is
nonzero until fill work completes. The program is split into
`draw/rounded_rectangle.rs` and `draw/rounded_rectangle_program.rs`; both stay
below 1,000 lines. The public Python signature is unchanged. Native primitive
coordinate conversion, output, callback, mutation, and fault parity remain
unverified and tracked by `pillow-rs#3ptb`. Source-only; no build, formatter,
checkstyle, lint, tests, parity, benchmark, or coverage ran. Reference:
[Pillow 12.2 rounded_rectangle](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L374-L502).

Pillow text input contract clarification (2026-11-06): the shared internal
codepoint carrier remains applicable to every public Python API that accepts
Unicode text, including ImageFont, ImageDraw, and ImageText.Text/wrap. Their
Python signatures remain `str`/`bytes`; the internal carrier preserves Python
codepoints and does not promise that every font/layout route renders a glyph
for a surrogate. Each route must match Pillow's own result or error. Pillow's
FreeType BASIC path reads Unicode codepoints and its RAQM path copies them to
UCS-4. No public Python API was widened. Text output and fault parity remain
pending. References: [Pillow 12.2 ImageFont](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageFont.py),
[ImageDraw](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L503-L757),
[ImageText](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageText.py#L89-L129),
and [FreeType input paths](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imagingft.c#L235-L279).

Rust root size correction (2026-11-06): the current source inventory found
`pillow-rs/src/lib.rs` at 1,047 lines, superseding the earlier scan that
reported no over-limit files. Its test-only fault controls and backend-support
summary now live in `pillow-rs/src/test_api.rs`; feature-gated crate-root
re-exports preserve the existing Rust paths. `lib.rs` is now 887 lines and
`test_api.rs` is 168 lines. The root public API inventory still needs its full
source review; no compiler or validation command ran.

ImagePath public parser correction (2026-11-06): Pillow's native argument
parser now handles `Path.transform`, `compact`, `tolist`, and `getbbox`, keeping
sequence-valued transform matrices and method argument errors aligned with the
public API. General sequence items use `PySequence_GetItem`. Pair assignment
converts and commits x before converting y, preserving an earlier x write if y
conversion fails. Non-tuple assignment/map results reproduce Pillow's
`PyArg_ParseTuple` `SystemError`. Remaining source TODOs cover PyBUF_SIMPLE
acquisition and the pending-exception `SystemError` from failed numeric
conversion in Path construction; map mutation and all output/fault behavior
still require the frozen CPU parity pass. No build, formatter, checkstyle,
lint, tests, parity, benchmark, or coverage ran. References: [Pillow 12.2
`path.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/path.c#L103-L252)
and [method parser slots](https://github.com/python-pillow/Pillow/blob/12.2.0/src/path.c#L289-L478).

ImagePath construction and mutation source continuation (2026-11-06): path
construction now reserves the final point storage before sequence callbacks
and builds points in one allocation, preserving Pillow's allocation order and
avoiding an intermediate flat-coordinate allocation. The sequence adapter
uses direct list/tuple item access for those built-ins and `PySequence_GetItem`
for general sequences. Pair assignment writes x before converting y, and the
mapping guard follows Pillow's single mutable mapping flag, including its
nested-map reset behavior. `Path(-1)` and failed reservations raise an empty
`MemoryError`, matching Pillow's `PyErr_NoMemory` surface. Remaining
ImagePath TODOs are failed numeric-conversion exceptions in construction;
map, callback, output, and fault parity remain in the CPU gate. No build,
formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.

ImagePath buffer adapter consolidation (2026-11-06): ImagePath and ImageDraw
now share one RAII `PyBUF_SIMPLE` float32-pair reader. It clears failed buffer
acquisition before sequence fallback, reads complete native-endian float32
pairs directly from the live contiguous view, and releases the exporter view
on success and every error path. This removes ImagePath's intermediate
`memoryview.cast().tobytes()` copy and removes the duplicated Draw buffer
reader. The public Python constructor and drawing signatures and accepted
input types are unchanged. Failed numeric-conversion exception behavior in
Path construction and all buffer/output/fault parity remain pending. Source
review only; no build, formatter, checkstyle, lint, tests, parity, benchmark,
or coverage ran.

ImagePath numeric-conversion fault correction (2026-11-06): scalar and nested
list coordinates now call the matching CPython float and C-long conversion
functions directly, retaining their sentinel values and pending exceptions
while the path sequence continues. The `Path` constructor returns its built
object with the conversion exception still pending, allowing CPython's public
call boundary to produce Pillow's observed `SystemError` and conversion error
context. Tuple coordinates continue to use the separate `PyArg_ParseTuple`
path and its `ValueError` mapping. The public signature and accepted input
types are unchanged. Callback, output, and fault parity remain in the frozen
CPU gate. This closes the remaining ImagePath source TODOs from the previous
entries. Source review only; no build, formatter, checkstyle, lint, tests,
parity, benchmark, or coverage ran. Reference: [Pillow 12.2 `path.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/path.c#L1632-L1714).

Image.transform palette snapshot correction (2026-11-06): P-mode transform
now uses Pillow's `if self.palette` check and calls `copy()` for every truthy
palette before transform callbacks. This preserves observable truthiness and
copy behavior for custom palette subclasses and empty palette objects instead
of inferring whether a copy is needed from internal fields. Public signatures,
accepted inputs, output mode, and mutation/fault ordering remain the contract.
The inspectable `transform` signature now carries Pillow's annotations and
`Resampling.NEAREST` enum default instead of a plain integer default.
Transform no longer calls `load()` or palette `getdata()` after raster
execution; the returned image retains the exact result of the pre-callback
palette `copy()` call, including a custom `None` result, until its normal load
path. This preserves Pillow's callback count and reentrant palette timing.
Remaining custom-property and reentrant palette behavior, output, callback,
and fault parity are pending. Source review only; no build, formatter,
checkstyle, lint, tests, parity, benchmark, or coverage ran. Reference:
[Pillow 12.2 `Image.transform`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2674-L2758).

Autocontrast WIP correction (2026-11-06): removed the final host-side LUT algorithm
and the `Vec<u32>` narrowing of ImageOps histogram channels. Rust now coordinates
Pillow's sum, cutoff, trim, and edge-scan sequence through callbacks that retain
host values for Python item, numeric, comparison, and truthiness behavior. The
shared byte LUT helper supplies the final mapping. This is source-only until
focused callback parity and compilation run.

Autocontrast WIP verification (2026-11-06): the release Python binding compile
and `make build-parity` passed. The frozen 188-case Autocontrast CPU selection
passed 188/188 with zero infrastructure errors. A separate custom histogram
reproduction matched Pillow 12.2.0 LUT hashes for truthiness-only bin objects
and custom numeric bin objects with cutoff 17.0. The Autocontrast narrowing WIP
is resolved; full CPU source, fault-order, and architecture receipt gates remain
open. Report: `build/migration-parity/autocontrast-wip-20261106.json`.

CPU parity WIP refresh (2026-11-07; failure classification corrected
2026-11-07): the full CPU report selected and executed 17,436 cases; 17,380
passed, 56 failed, and there were no infrastructure errors. Reviewing the
saved result shows 16 I;16 Convert cases where both sides returned images but
image/byte outputs differed; these are parity mismatches, not mode extensions.
Another 39 failures are target-only status differences: 38 P-mode
Posterize/Solarize cases and native I;16 PutAlpha. One I-to-PNG save ran only
on Pillow and remains the image-slash-star codec gap tracked by
`image-slash-star#9h5w` and `#nype`. The full report is
`build/migration-parity/cpu-wip-postfix-20261107.json`; it used manifest
SHA-256 `6dcb1bcf4b8f6122302c726cf4a9995b7da8a7c1ec901d19e28a2b42d87963cc`
and a dirty target at revision `05a88d9b92ff8c2352dad4f22995d714437f4ee9`.
This is historical evidence and does not establish current-source behavior.
The parity command was nonzero; the full CPU gate remains open.

The refreshed run includes the previously failing Thumbnail zero-size fault
order and P-mode affine fill-list conversion; both now match Pillow. It also
includes a new `PA` BILINEAR Thumbnail case after source inspection showed
Pillow filters PA's raw index/alpha lanes. The native PA Thumbnail adapter now
keeps those samples indexed through its typed Resize strategy instead of
expanding to RGBA. The frozen 26-case guard set, including this new case,
passed 26/26 with zero infrastructure errors in
`build/migration-parity/cpu-pa-thumbnail-guards-20261107.json`. Further PA
filters, reducing-gap boundaries, all other operation × mode × architecture
coverage, full source review, and performance gates remain open.

Text API parity refresh (2026-11-07): the full CPU report also passes all 19
selected ImageText/Text.wrap, FreeType text, and Draw lone-surrogate cases.
This verifies the shared codepoint transport on those public inputs, including
the original string and mutated remainder observations. It does not close
custom font callback or arbitrary Python subclass protocol coverage; retain
those as explicit source and parity TODOs.

PA Thumbnail matrix continuation (2026-11-07): the canonical input corpus now
covers filtered PA Thumbnail with BOX, BILINEAR, HAMMING, BICUBIC, and LANCZOS,
plus reducing gaps `None`, `1.0`, the default `2.0`, and `4.0`; the existing
PA NEAREST case remains the guard for Pillow's nearest path. The frozen CPU
selection passed 9/9 with zero infrastructure errors. Report:
`build/migration-parity/cpu-pa-thumbnail-matrix-20261107.json`. Manifest
SHA-256: `6dcb1bcf4b8f6122302c726cf4a9995b7da8a7c1ec901d19e28a2b42d87963cc`;
parity input SHA-256: `11172f9efaa6cd0d298a8da7440d0347ca8f34dd9d7c795de6a67904a09ea2ac`;
selected-ID SHA-256: `12d45e8993476aff336d2917c8f064b155bad4a66d12bed124e875937fcbb5ff`.
The remaining PA work is architecture-strategy parity and fallback-selection
receipts, alongside the complete operation × mode × architecture matrix.

Text.wrap callback continuation (2026-11-07): added input-only parity cases
for an instance-overridden `FreeTypeFont.getbbox`, including a deterministic
failure on the third callback. Both matched Pillow: 2/2, zero infrastructure
errors. Report: `build/migration-parity/cpu-text-wrap-callbacks-20261107.json`.
Manifest SHA-256: `6dcb1bcf4b8f6122302c726cf4a9995b7da8a7c1ec901d19e28a2b42d87963cc`;
ImageText parity input SHA-256:
`820e9d7bbb91357fe59d2689e54f39a0904adc1480959199de22552ba969bae9`;
selected-ID SHA-256: `6e6fd71c2634bb1ca2ce7391afeebf5ddc0bbeac66a490e0ea7bfc72cea33bd5`.
This closes the instance-level font metric callback gap for these inputs;
Text subclass overrides and the complete operation × mode × architecture
matrix remain open.

Current CPU corpus evidence reconciliation (2026-11-07; failure
classification corrected 2026-11-07): the previous full 17,436-case CPU run
plus focused evidence for the 9 then-new cases covers all 17,445 unique parity
inputs in that corpus. Combined evidence is 17,389 matches and 56 differences:
the same 16 I;16 Convert output mismatches, 39 target-only status differences,
and one external codec difference described above. This was evidence reuse,
not a second full run, and it does not verify current source.

Color3DLut mode declaration consolidation (2026-11-07): one closed
`Color3DLutModeStrategy` now maps the six supported source modes and supplies
both root `ModeSupport` and prepared mode routing. The pipeline passes its
already-resolved `OperationInputs::mode()` through the source-mode consistency
check into that strategy, removing a second `DynamicImage::mode()` inference
and the duplicate six-mode dispatch. Rejection text and child architecture
selection are unchanged. Source-reviewed only; Color3DLut parity and
architecture receipts remain in the final CPU matrix.

CPU WIP compile cleanup (2026-11-07): the binding library now passes
`cargo check --manifest-path pillow-rs-py/Cargo.toml --lib --locked` with zero
Rust warnings. The check exposed visibility mismatches in exact-mode filter
and Expand strategies plus undocumented public enum fields; the internal
visibilities and field documentation now match their actual consumers.

Text subclass callback continuation (2026-11-07): added two input-only parity
cases for a `Text` subclass overriding `_get_bbox`, including a deterministic
third-call `ValueError`. Both matched Pillow: 2/2, zero infrastructure errors.
Report: `build/migration-parity/cpu-text-subclass-callbacks-20261107.json`;
run ID `migration-parity-56a4996ef0dc4b63881425a872e6a54f`; manifest SHA-256
`6dcb1bcf4b8f6122302c726cf4a9995b7da8a7c1ec901d19e28a2b42d87963cc`; input
SHA-256 `190515a6feee56da56101df0d2aca8d797ae13f6e9672196a7e5f90677f2a236`.
TODO(pillow-rs#3ptb): add focused parity cases for `Text` subclass `_split` and
`_get_fontmode` overrides, including multiline-only invocation and callback
fault ordering. Source audit (2026-11-07): non-direct Draw calls dispatch the
host's `_split` method, and the built-in splitter routes multiline font-mode
selection through `_get_fontmode`; direct routing rejects subclass and
instance overrides. The remaining gap is parity evidence for those routes.

Color3DLut routing follow-up (2026-11-07): the materialized smoke workflow
passed 1/1 with zero infrastructure errors after the shared mode strategy
change. Report: `build/migration-parity/cpu-color3dlut-wip-20261107.json`;
run ID `migration-parity-e47b137307d445b78199cfd1e3193d59`.

Current CPU input reconciliation (2026-11-07; failure classification
corrected 2026-11-07): the prior full run plus focused evidence covers all
17,447 CPU parity IDs in the generated 17,448-case corpus (the remaining case
is GPU-only). Combined evidence is 17,391 matches and the same 56 differences:
16 I;16 Convert output mismatches, 39 target-only status differences, and one
external I-to-PNG codec difference. A later focused 1,523-case report passed
all 16 I;16 cases on revision `2357323`; current Convert owners changed in
commit `0228432`, so their present result is unknown. The extension results
are not Pillow parity failures when declared in the Rust-only support lane.
This reuses prior full-run evidence and focused cases; it is not a new
full-corpus run and does not verify current source.

Text subclass hook parity follow-up (2026-11-07): the shared input-only hook
protocol now covers subclass overrides of `_get_bbox`, `_split`, and
`_get_fontmode`. All four focused cases, including `_get_bbox` and
`_get_fontmode` callback failures and custom `_split` placement, passed 4/4
against Pillow with zero infrastructure errors. Report:
`build/migration-parity/cpu-text-subclass-hooks-20261107.json`; run ID
`migration-parity-7eb9a7a99e184ff99fc192fa1e829758`; manifest SHA-256
`6dcb1bcf4b8f6122302c726cf4a9995b7da8a7c1ec901d19e28a2b42d87963cc`;
ImageText input SHA-256
`3b17a037fba073a818b57ab549309ba2a157a181a144c4815c582a411e2545f2`.
The case matrix verifies output and fault ordering; broader callback mutation
and reentrancy behavior remains part of the CPU contract audit.

CPU input reconciliation correction (2026-11-07): after adding the two new
Text subclass hook cases, the generated corpus contains 17,450 total cases,
including one GPU-only fault contract, and therefore 17,449 CPU parity IDs.
The previous full 17,436-case run plus focused evidence for all 13 later CPU
IDs covers this denominator. The four-case Text subclass run above overlaps
the earlier `_get_bbox` cases and verifies the two newly added hook cases;
the combined classification remains 17,391 matches, 56 declared Rust-only
extension or external I-to-PNG differences, and zero infrastructure errors.
No new full-corpus run was performed.

Draw mode parity follow-up (2026-11-07): the existing full CPU report confirms
all six historical `pillow-rs#brg0` byte cases pass, including CMYK `text` and
`multiline_text`, centered anchors, explicit font size, three-line spacing,
and stroked RGBA text. Added focused native-mode inputs for RGBX fourth-byte
blending, RGBa channel/alpha blending, and La alpha fill; these passed 3/3 with
zero infrastructure errors. Report:
`build/migration-parity/cpu-draw-native-alpha-modes-20261107.json`; run ID
`migration-parity-1b5fe0d6187d4e69a503fe3f5536fa36`; manifest SHA-256
`6dcb1bcf4b8f6122302c726cf4a9995b7da8a7c1ec901d19e28a2b42d87963cc`;
ImageDraw input SHA-256
`3af4db06618f2a8e04e19fcda9a2d6c341043278ee516dd346e93a0ddf1b3d9c`. The
historical cases are in full report
`migration-parity-0667a97f67d84cbaaeaf420f4c2830d7`. Their verified behavior
TODO comments were removed. `pillow-rs#brg0` remains open for findings outside
these CPU Draw cases.

CPU corpus reconciliation correction (2026-11-07; failure classification
corrected 2026-11-07): these Draw cases bring the generated corpus to 17,453
total cases, including one GPU-only fault contract, and therefore 17,452 CPU
parity IDs. The earlier full CPU run and focused evidence cover those IDs:
17,396 matched and the same 56 differences remained (16 I;16 Convert output
mismatches, 39 target-only status differences, and one external I-to-PNG
encoder difference). This reuses unchanged case evidence; no full-corpus
rerun was performed and the results do not verify current source.

Draw font metric callback parity (2026-11-07): generalized the existing
input-only font metric override to cover both `getbbox` and `getlength`. The
two `Text.wrap` `getbbox` cases and two new `Draw.textlength` `getlength`
cases, including a first-call failure, passed 4/4 with zero infrastructure
errors. Report:
`build/migration-parity/cpu-text-metric-callbacks-20261107.json`; run ID
`migration-parity-766024f90d5948a18b267ff889a028f9`; manifest SHA-256
`6dcb1bcf4b8f6122302c726cf4a9995b7da8a7c1ec901d19e28a2b42d87963cc`;
Draw input SHA-256
`c8bd6d143944995cad87d26f4095fc09c3efd5395c7bd805c81c0f310d4ed1df`;
ImageText input SHA-256
`48e2ef38cadda2e8705ecab57598c4572c1d15f99dbc93b23e60ac862ba68c9b`.
Removed the covered `Draw.textlength` callback TODO.

CPU input reconciliation correction (2026-11-07): the corpus then contained
17,455 total parity cases, with one GPU-only fault contract and 17,454 CPU
IDs. The prior full CPU run plus focused reports covered those IDs: 17,398
passed, with the same 56 differences (16 I;16 Convert output mismatches, 39
target-only status differences, and one external I-to-PNG encoder difference)
and zero infrastructure errors. The later focused 1,523-case run passed the
16 I;16 cases before the Convert-dispatch commit changed their owners. No
full-corpus rerun was performed; these results do not verify current source.

Draw ink and palette evidence correction (2026-11-07): regenerated inputs
contain 17,542 cases (17,541 CPU cases and one GPU-only fault contract). The
current ImageDraw parity input SHA-256 is
`c0a213d85e7398fbc9963156a4aa5ef1b6b5d8f03f1b1884a7b4b63e744d1912`; the
manifest SHA-256 remains
`6dcb1bcf4b8f6122302c726cf4a9995b7da8a7c1ec901d19e28a2b42d87963cc`.
Because the Draw input hash changed, older full-corpus results are not counted
as current full-corpus evidence. The focused report
`build/migration-parity/cpu-draw-mode-matrix-current-20261107.json`, run ID
`migration-parity-367d74caafcf40b7b6ecb5c9ffd0dd30`, passed 125/125 CPU cases
with zero infrastructure errors. It includes the 110-case Draw mask/mode and
callback cohort plus 15 ink-boundary and F-mode text cases. Coverage includes
all four Bitmap mask modes across 20 public Pillow destination modes, signed
long-long and tuple C-int boundaries on both signs, singleton tuple ink,
wide F-mode integers, F-mode text ink bits, and P-mode `getpalette()` retention
after negative-origin text. The long-long and C-int overflow messages now
match the observed CPython fault contract. The P-mode palette mutation/error
ordering case remains TODO, as do full Draw architecture and route receipts.

ImageOps.expand I;16 fill follow-up (2026-11-07): added 8 CPU parity cases for
`I;16`, `I;16L`, `I;16B`, and `I;16N`, covering `70000` and `-1` fills. All
8/8 passed with zero infrastructure errors in
`build/migration-parity/cpu-expand-i16-fill-20261107.json`, run ID
`migration-parity-31ae7821fbd74b8f9c2d6746d7a612a0`; manifest SHA-256
`6dcb1bcf4b8f6122302c726cf4a9995b7da8a7c1ec901d19e28a2b42d87963cc`; input
SHA-256 `5c1a852787ea3df6cbae22cd2919d655fcd0ed71d20bf6c80e6a631a0231b529`.
This verifies low-16-bit fill wrapping for all four byte-order variants and
removes that portion of the source TODO. CoreOnly typed component fills and
architecture fallback receipts remain open. The regenerated corpus now has
17,550 cases, including one GPU-only contract and 17,549 CPU cases; a full
parity reconciliation is still deferred until the source pass is complete.

Image I→I;16N conversion follow-up (2026-11-07): three explicit source values
(`-1`, `300`, and `128`) passed 3/3 with zero infrastructure errors in
`build/migration-parity/cpu-convert-i32-to-i16n-20261107.json`, run ID
`migration-parity-eed28d98fd164476833f0c76da2c403f`; manifest SHA-256
`6dcb1bcf4b8f6122302c726cf4a9995b7da8a7c1ec901d19e28a2b42d87963cc`; input
SHA-256 `86c10651329ce7d31dc25b5a2c62c125dce5c34dcca45daac269b082dc619e3c`.
This verifies clipping below zero and above the L range plus an in-range
sample, with I;16N native byte output, and removes that source TODO. The
regenerated corpus now contains 17,553 cases: 17,552 CPU cases and one
GPU-only contract. Full CPU reconciliation remains pending until the source
pass is complete.

Brightness float32 boundary follow-up (2026-11-07): sample `10` with factor
`0.09999999999999999` passed 1/1 with zero infrastructure errors in
`build/migration-parity/cpu-brightness-float32-boundary-20261107.json`, run ID
`migration-parity-d1f1a1556a8f414490228c92d56b1b16`; manifest SHA-256
`6dcb1bcf4b8f6122302c726cf4a9995b7da8a7c1ec901d19e28a2b42d87963cc`; input
SHA-256 `b90da1fcb44395093deb16895eb97ff5bee2db9fe51f35ace88471c1c95d1b84`.
The Pillow-compatible float32 rounding result is pinned; the remaining
Brightness TODO covers complete mode and architecture route/fallback receipts.
The current corpus has 17,554 total cases, including one GPU-only contract and
17,553 CPU cases. Full CPU reconciliation remains deferred until the source
pass is complete.


ModeFilter public support correction (2026-11-07): Pillow's C `ModeFilter` kernel accepts one-band UINT8 storage, but public `Image.filter` splits a multiband image into L bands, filters each band, and merges the results. Therefore all fourteen Pillow byte modes `1`, `L`, `LA`, `La`, `P`, `PA`, `RGB`, `RGBA`, `RGBa`, `RGBX`, `CMYK`, `YCbCr`, `LAB`, and `HSV` are supported by the public filter route. The exact-mode native byte-channel owners fuse the independent per-band passes without changing the algorithm. `P` filters palette indices and retains its palette; the multiband route retains its exact output mode. The earlier CoreOnly and `NO_PILLOW_SUPPORT` classification for multiband ModeFilter was incorrect and has been removed. Added input-only coverage for all fourteen modes alongside P palette and L info cases; generated input, selected/fallback receipts, and parity remain pending until CPU source completion. Sources: [Pillow 12.2.0 Image.filter](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1330-L1353), [Pillow 12.2.0 ImageFilter.ModeFilter](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageFilter.py#L1607-L1628), and [Pillow 12.2.0 ModeFilter.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/ModeFilter.c#L378-L382).

ModeFilter metadata (2026-11-07): Pillow's `Image.filter` preserves `info` through `_new` for both the direct single-band path and the multiband split/merge path. Direct P filtering copies its source palette. Multiband PA filtering merges from an L-band wrapper with no palette, so Pillow attaches a default empty PA palette; it does not preserve the input PA palette. The Python facade copies `info` through `Image._new`/`_copy_info_state_from`; CPU P mode owns its retained palette, while the PA merge output has no attached palette. Added input-only L info-copy and PA attached-palette cases that observe the result record and bytes. Neither input has been generated or run; CPU source completion still gates parity. References: [Pillow 12.2.0 Image._new](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L629-L642), [Pillow 12.2.0 Image.filter and merge](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1330-L1353), [Rust Image.filter facade](../pillow-rs-py/python/pillow_rs/image.py), and [ModeFilter metadata parity cases](../scripts/build_migration_parity_inputs.py).

Draw Text ink-order source correction (2026-11-07): the Python facade now
resolves fill ink before reading `ImageText.Text` stroke state. It preserves
`direction`, `features`, `language`, and `embedded_color` from the public
`ImageDraw.text` arguments, as Pillow does, instead of substituting values from
a supplied Text object. Custom `_split` line objects remain host-owned until
callbacks read `x`, `y`, `text`, and `anchor` at their call sites; custom-font
callbacks read the current font and stroke width at mask invocation. Position
conversion follows `int(x)`, `int(y)`, then `math.modf(x)` and `math.modf(y)`,
and the selected font-mask mode precedes position conversion. Direct routing
short-circuits non-native text before preparing route state, defers ImageFont
import and spacing access until native candidate checks, and requires the exact
built-in ImageDraw owner before reading its mutable `fontmode`. Direct-route
coordinates now fall back when truncation is non-finite or outside C-int range,
so Rust's saturating float casts cannot replace Pillow's coordinate fault path.
PyFont/Option<String> and coordinate error text/order remain parity-pending;
outputs, metadata, and route receipts are also pending parity. No validation
ran. Sources: [Pillow 12.2.0 ImageDraw.text](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L3150-L3205), [ImageText._split](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageText.py#L1724-L1940).

Draw RGBA mask adapter order (2026-11-07): the custom-mask bridge now reads
`coverage.size` twice, builds `(x, y, x + width, y + height)`, and converts all
four box coordinates before resolving and type-checking the coverage carrier,
then the source carrier. This follows Pillow's argument evaluation and native
paste binding, which parses the box and optional typed mask before checking the
source carrier. This is a source correction only; coordinate error messages,
outputs, metadata, and route receipts remain parity-pending. No validation ran.
References: [Pillow 12.2.0 ImageDraw.text](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L3287-L3305) and [Pillow 12.2.0
`_imaging.c::_paste`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L1450-L1485).

P-mode Draw palette inputs (2026-11-07): the successful new tuple-color draw
observes pixels and palette, and the valid tuple color followed by an invalid
text anchor records error ordering. The float-tuple case now records the
`ImagePalette.getcolor` byte-conversion error, then makes an independent public
`point` call on the existing Draw object with the same tuple and observes the
image bytes and palette. The parity runner's opt-in continuation executes only
steps whose binding dependencies do not include a failed or skipped step; the
contract validator requires the primary error and later observations to be
independent. This makes the palette-cache side effect observable through
public behavior rather than inspecting private state. The inputs and runner
changes are source-only; generation and parity remain deferred until the CPU
source pass is complete. No validation ran.

ModeFilter source audit (2026-11-07): the operation-level support query
projects `mode_filter_route`, and prepared execution uses that same route for
all fourteen Pillow byte modes. Each mode owner checks its exact
`DynamicImage` variant and invokes the native-byte channel strategy; unsupported
requested architectures retain the same-mode Default implementation with a
`missing_specialization` fallback receipt. The Python `Image.filter` wrapper
loads before dispatch and `_new` copies `info`; the P result retains its
palette, while PA does not inherit the source palette after the multiband
merge route. Output, metadata, and architecture receipt parity remain pending
for the CPU gate. No validation ran.

Legacy archive source audit (2026-11-07): repository-wide active-source search
outside `pool_cpu/legacy` found no `pool_cpu::legacy` import, `pool_cpu/legacy`
path, `mod legacy`, or `legacy::` reference. The only active-source match for
the word `legacy` is the documented public `simd` alias in compute context.
This is source-search evidence only; the final build gate remains necessary
to verify the compiled module graph. No files in the archive were modified.

Pad full-width Resize source review (2026-11-07): the archived SIMD Pad's
full-width convolution branch is admitted only for non-nearest L and RGBX.
The maintained Pad operation composes its exact-mode Resize selection and
writes those outputs into the final canvas through the L and RGBX
`resize_into_canvas` adapters. `PreparedResize::execute_into_pad_window`
dispatches the selected Default/SSE2/NEON/Wasm SIMD128 implementation under
the retained mode and architecture. Fill bytes match the mode-owned Pad
strategies. This closes that source-migration item; the remaining Pad source
audit and all runtime output/error/receipt parity remain pending. No validation
ran.

Convert owner-layout source audit (2026-11-07): each of the 25 exact source
modes has a `default/convert_impl.rs`, and every architecture named in that
mode's `ops.rs` has a matching architecture folder. This confirms folder and
declaration alignment only; archived source/target/guard reconciliation,
unreachable-path accounting, and caller review remain open. No validation ran.

Convert RGB8 source reconciliation (2026-11-07): the archived native SIMD
preflight admits RGB-to-L, LA, RGB, RGBA, CMYK, YCbCr, HSV, I, and F byte
routes, with SSE2/NEON/Wasm SIMD128 generic paths and the separate
SSSE3/AVX2 luma specialization. The maintained RGB8 declaration preserves
those exact targets and architecture routes. P/PA palette conversion, LAB,
and the four I;16 output identities remain Default-only, matching their
separate archived paths or lack of a SIMD route. The new RGB-to-La and RGB-to-
RGBa vector routes append opaque alpha and are explicit additional strategies;
their output parity remains unverified. No validation ran.

Convert RGBa source/build-dispatch review (2026-11-07): the exact target
contract has 20 identities. Archived native-byte layout and alpha routes are
accounted for by the active exact-mode Default owners; the architecture table
declares SSE2/NEON/Wasm SIMD128 for generic native RGBa conversions and
nonempty unpremultiplication routes, plus SSE2/SSSE3/AVX2/NEON/Wasm for
RGBa-to-L through the Grayscale kernels. The source's same-mode copy,
palette, LAB, and I;16 routes remain Default-only. Each availability predicate
matches the corresponding cfg-gated execution arm, and every declared
architecture has a mode-owned implementation folder. RGBa-to-L's nonempty
vector route is an added strategy; the archived SIMD Convert adapter rejected
that pair and its empty input selects same-mode Default. Exact output and
fallback parity remain pending. No build or validation ran.

Convert CMYK source/build-dispatch review (2026-11-07): the CMYK owner declares
20 output identities and routes same-mode, RGB-derived colors, binary output,
palette output, and typed I/F outputs through its exact-mode Default strategy
and dedicated wrapper helpers. The archived SIMD adapter had one CMYK-source
special case: CMYK-to-RGB. Its generic native-byte layout accepts only L, LA,
RGB, RGBA, RGBa, and RGBX source identities, so it did not provide additional
CMYK-source SIMD targets. The maintained declaration guards SSE2, SSSE3, NEON,
and Wasm SIMD128 implementations to RGB output; the prepared dispatcher has
matching target arms and the architecture folders exist. The shared vector
helper preserves the archived CMYK channel formula and handles a scalar-sized
tail; Default uses the same `cmyk_pixel_to_rgb` arithmetic. Output, faults,
palette metadata, empty-input selection receipts, and CPU parity remain open.
No build or validation ran.

Convert Luma8 source/build-dispatch review (2026-11-07): the exact L source
contract declares 18 output identities. Its prepared identity path preserves
same-mode L samples; the Default owner composes LAB from L samples, uses the
grayscale palette path for P/PA, and delegates One to the shared binary target
strategy. The archived SIMD adapter's generic native-byte matrix covers L,
LA, RGB, RGBA, CMYK,
YCbCr, HSV, I, and F; its separate L→I;16 kernel is carried by four typed
I;16 target identities. The maintained strategy matrix has 36 guarded
entries: SSE2, NEON, and Wasm SIMD128 for twelve targets (LA, RGB, RGBA,
CMYK, YCbCr, HSV, I, F, and four I;16 identities). Each architecture
dispatcher enters the matching exact-mode helper, whose target match covers
those same twelve modes. The same-mode L request now uses the prepared Default
identity path: its three unreachable vector declarations and both
mode-local identity branches were removed because support assessment excludes
same-mode kernel selection.
One, P, PA, RGBX, and LAB also remain Default-only. The archive had no generic
SIMD route for RGBX or LAB, and its palette/binary targets were handled outside
this adapter. Output, dither and palette faults, metadata, empty-input
receipts, same-mode behavior, and CPU parity remain open. No build or
validation ran.

Convert LumaA8 source/build-dispatch review (2026-11-07): the exact LA source
contract declares 19 output identities. Its same-mode result comes from the
prepared identity path. The archived generic native-byte vector matrix covers
LA-to-L, RGB, RGBA, CMYK, YCbCr, HSV, I, and F; archived alpha-special paths
cover nonempty LA-to-La and LA-to-RGBX. The maintained declaration has 30
architecture entries: SSE2, NEON, and Wasm SIMD128 for ten targets (L, La,
RGB, RGBA, RGBX, CMYK, YCbCr, HSV, I, and F), with matching dispatch arms and
architecture folders. Its vector helper uses the source luma lane for L/RGB,
the dedicated expansion, typed, color, and premultiplication helpers for the
other targets. RGBA/RGBX carry alpha in their output storage; the post-dispatch
compatibility helper retains LA alpha in compact RGB/HSV hidden lanes, while
YCbCr uses Pillow's opaque padding behavior. One, P, PA, LAB, and four I;16
targets have no
architecture entry. The same-mode LA vector declaration and both mode-local
identity branches were removed because prepared identity returns before mode
dispatch and same-mode architecture guards are ineligible. Empty alpha
transforms preserve their nonempty fallback guard; other empty routes remain
source-reviewed. Output bytes, hidden lanes, faults, palette metadata,
architecture receipts, and CPU parity remain open. No build or validation
ran.

Convert One source/build-dispatch review (2026-11-07): the exact bilevel source
contract declares 18 output identities. Same-mode One uses prepared identity;
the Default owner expands nonzero source samples to 255 and feeds its declared
L/LA/RGB/RGBA/RGBX/CMYK/YCbCr/HSV/I/F, LAB, palette, and I;16 targets through
native sample helpers. The archived SIMD adapter admits the One byte carrier
only for I;16 targets, using the special `1` source normalization; it rejects
One for the generic layout SIMD matrix. The maintained catalog therefore has
12 entries: SSE2, NEON, and Wasm SIMD128 for the four I;16 identities, with
matching target dispatch and architecture folders. The I;16 helper expands
every nonzero byte to 255 before widening to u16, including the archived
zero-work vector receipt. All other nonidentity targets remain Default-only;
P/PA use the palette-aware output boundary. Output/fault/palette metadata,
empty-input receipts, architecture fallback receipts, and CPU parity remain
open. No build or validation ran.

Convert P source/build-dispatch review (2026-11-07): the exact palette-index
contract declares 14 identities: One, L, LA, P, PA, RGB, RGBX, RGBA, CMYK,
YCbCr, HSV, LAB, I, and F. Pillow's `frompalette` admits the listed direct
palette conversions, while Python's LAB branch promotes P through RGBA before
the profile transform; P's Pillow mode base is P, so unsupported native
`frompalette` targets do not gain routes through generic base-mode fallback.
The active P owner requires palette metadata, has no architecture strategies,
and the archived SIMD generic-layout preflight also rejects P's one-byte
carrier. Same-mode P retains its wrapper palette; P→PA retains the palette and
expands per-index alpha; RGBX uses the palette alpha byte as its fourth output
byte, while RGB/YCbCr/HSV use opaque padding. P→LAB now retains the palette
alpha in LAB's hidden fourth byte, matching the Python RGBA intermediate. The
Default owner handles other color/scalar targets directly from indexed palette
samples, and target One uses the P-specific binary strategy: it compares the
unrounded `299R + 587G + 114B` palette sum with `128000`, without an RGB raster
or dithering. High-depth u16,
La, and RGBa destinations are not in the Pillow P contract. Source references:
[Pillow Convert.c palette routes](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L879-L1067),
[Image.convert LAB route](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1118-L1134),
and [Pillow P mode base](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageMode.py#L42-L57).
Palette faults, output bytes, metadata, hidden lanes, empty-input selection
receipts, and CPU parity remain open. No build or validation ran.

Convert PA source/build-dispatch review (2026-11-07): the exact palette-index
and per-pixel-alpha source contract declares 20 output identities: One, L, LA,
P, PA, La, RGBa, I;16 variants, RGB, RGBA, RGBX, LAB, CMYK, YCbCr, HSV, I,
and F. PA requires palette metadata and has no declared architecture strategy;
the archived SIMD byte-layout preflight accepts only L/LA/RGB/RGBA carriers,
so PA's two-byte index/alpha carrier falls back to CPU. Its direct One path
reads the palette color by index, ignores per-pixel alpha, applies the same
unrounded weighted threshold as P, and bypasses dithering. The Default strategy
keeps indexed data native for its declared targets; the LAB path restores the
omitted physical lane from per-pixel alpha. Pillow's `PA` mode has RGB base
mode, so base-mode fallback is part of the target-routing review; output
bytes, alpha/padding lanes, high-depth route reachability, palette/wrapper
metadata, faults, empty-input receipts, and CPU parity remain open. Sources:
[Pillow palette luma formulas](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L39-L41),
[Pillow P/PA-to-One kernels](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L3723-L3748),
[Pillow mode base table](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageMode.py#L407-L438),
and [Image.convert base-mode fallback](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1118-L1151).
No build or validation ran.


Convert RGBA source/build-dispatch review (2026-11-07): the exact straight-RGBA contract declares 20 targets: One, L, LA, P, PA, La, I;16 variants, RGB, RGBA, LAB, RGBa, RGBX, CMYK, YCbCr, HSV, I, and F. Same-mode RGBA returns from the prepared identity path before mode execution. The three unreachable SSE2, NEON, and Wasm RGBA-to-RGBA declarations, their vector copy arm, and the Default clone arm were removed; the architecture catalog now has 33 declared entries subject to their availability guards. Its target coverage is RGB under SSE2/SSSE3/NEON/Wasm; L under AVX2/SSSE3/SSE2/NEON/Wasm; nonempty RGBX and RGBa under SSE2/NEON/Wasm; and LA, CMYK, YCbCr, HSV, I, and F under SSE2/NEON/Wasm. One, P/PA palette output, four I;16 targets, and LAB remain Default-only. Prepared palette targets synthesize their output metadata, while direct RGBA-to-LAB reads RGB lanes. Target bytes, alpha/padding lanes, palette metadata, architecture receipts/fallbacks, empty-input behavior, faults, and CPU parity remain open. Source-only; no build or validation ran.


Convert RGBX source/build-dispatch review (2026-11-07): the exact RGBX contract declares 20 targets: One, L, LA, P, PA, I;16 variants, RGB, RGBA, RGBX, La, RGBa, CMYK, YCbCr, HSV, LAB, I, and F. Same-mode RGBX returns through prepared identity and has no architecture declaration. The 36 declared architecture entries map L to AVX2/SSSE3/SSE2/NEON/Wasm; RGB to SSSE3/SSE2/NEON/Wasm; and LA, RGBA, La, RGBa, CMYK, YCbCr, HSV, I, and F to SSE2/NEON/Wasm. The active target dispatcher and architecture folders cover those entries. One, palette output, I;16 targets, LAB, and same-mode RGBX remain Default-only. LAB preserves the source X lane in its hidden physical byte. RGBX-to-La and RGBX-to-RGBa are explicit Rust-only extensions with adjacent `NO_PILLOW_SUPPORT` comments. Exact lane/output parity, palette metadata, faults, empty-input and fallback receipts, and CPU parity remain open. Source-only; no build or validation ran.


Convert La source/build-dispatch review (2026-11-07): the exact La contract declares three targets: same-mode La, LA, and LAB. Same-mode La returns through prepared identity; LAB is the direct native-sample Rust extension and is marked `NO_PILLOW_SUPPORT`. The only architecture strategy is La-to-LA unpremultiplication, declared for SSE2, NEON, and Wasm SIMD128 with a nonempty guard; the Default owner handles the same conversion and empty inputs. The archived SIMD Convert alpha adapter contains the matching exact `La`→`LA` unpremultiply route, while generic native-byte layout does not admit La. Unsupported targets retain the declared `conversion from La to L not supported` rejection. Removed the unreachable binary-target wrapper and the Default same-mode clone arm. Output/fault/empty-input receipts and parity remain open. Source-only; no build or validation ran.


Convert I32 source/build-dispatch review (2026-11-07): I32's exact source
contract declares 18 scalar targets. Same-mode identity is handled before
mode dispatch; One and P/PA have their own prepared target paths. Other targets
use the native signed sample in the I32 Default owner, including direct I32→F
conversion and direct LAB output. No I32-specific source architecture kernel
exists in the archive; the generic physical four-byte path is not a typed
I32 strategy. Removed the unreachable same-mode clone. Preserve the current
algorithms until exact sample, fault, output, metadata, and empty-input parity
receipts are collected. Architecture specialization remains a TODO only if a
measured, semantics-preserving I32 kernel is justified. No build or validation
ran.

Convert F32 source/build-dispatch review (2026-11-07): F32's exact source
contract declares 18 scalar targets, with same-mode identity handled before
mode dispatch and separate One and palette target paths. Its Default owner
reads native float samples and emits target pixels directly; F→I16 follows the
L-derived byte clipping rule. No F32-specific source architecture kernel
exists in the archive; the generic physical four-byte path is not a typed F32
strategy. Removed the unreachable same-mode clone. Non-finite and out-of-range
F→I behavior remains an explicit TODO, along with exact output, fault, metadata,
and empty-input parity receipts. Add an architecture strategy only after its
float semantics are exact and a distinct kernel is warranted. No build or
validation ran.


Convert YCbCr source/build-dispatch review (2026-11-07): the source contract
declares 20 Pillow targets and no CoreOnly targets. Same-mode identity exits
before the mode owner; the Default path derives outputs directly from native
Y/Cb/Cr samples and preserves the compact hidden lane for RGB-family outputs.
The archived SIMD YCbCr kernel is target-side (RGB/luma→YCbCr); its generic
three-byte path cannot treat YCbCr as RGB. Removed the unreachable same-mode
clone. TODO: collect exact output, fault, metadata, and empty-input receipts,
then evaluate a YCbCr-source vector kernel only if its arithmetic can match the
Default path. No build or validation ran.


Convert HSV source/build-dispatch review (2026-11-07): the source contract
declares 20 Pillow targets and no CoreOnly targets. Prepared identity handles
HSV→HSV; the Default path converts directly from native H/S/V samples and
preserves compact hidden lanes in RGB-family outputs. The archived SIMD HSV
kernel is target-side RGB/luma→HSV, while its generic three-byte path cannot
interpret H/S/V correctly as RGB. Removed the unreachable same-mode clone.
TODO: collect exact output, fault, metadata, and empty-input receipts, then
evaluate a source-side HSV vector strategy only if it preserves exact arithmetic.
No build or validation ran.


Convert LAB source/build-dispatch review (2026-11-07): LAB declares 20
targets. Same-mode identity and RGB/RGBA/RGBX are Pillow-facing; the other 16
targets are explicitly CoreOnly and marked `NO_PILLOW_SUPPORT`. The Default
strategy uses the native LAB→sRGB LUT path and writes each target from its
mode-specific output strategy. The archive has no LAB-source architecture
kernel; generic three-byte conversion would interpret LAB as RGB. Removed the
unreachable same-mode clone. TODO: complete ImageCms output/error and CoreOnly
output/fault parity, then evaluate architecture acceleration only if exact
RGB-family bytes can be preserved. No build or validation ran.


Convert I16-family source/build-dispatch review (2026-11-07): I;16, I;16L,
I;16B, and I;16N remain four separate exact source identities. Each declares
the same 18 targets and no CoreOnly targets, and each passes its own Mode into
its Default or architecture strategy. The shared typed-u16 helper declares
SSE2, NEON, and Wasm SIMD128 strategies for eight targets (L, LA, RGB, RGBA,
RGBX, CMYK, I, F), processing eight samples per vector block. One and palette
targets, LAB, YCbCr, HSV, and cross-alias I;16 outputs remain Default-only.
Prepared identity preserves same-mode samples; cross-alias I;16 conversion
clips to 255, with the I;16N-to-I/F byte-range rule applied in both Default
and vector paths. Removed the unreachable same-mode clone branch from the
shared alias conversion. TODO: complete every exact source/target/architecture
output, fault, palette, receipt, fallback, and empty-input parity case. No
build or validation ran.


Convert CoreOnly high-depth source/build-dispatch review (2026-11-07): the
LA16, RGB16, RGBA16, RGB32F, and RGBA32F source owners each declare 22 target
identities and mark the source operation CoreOnly with `NO_PILLOW_SUPPORT`.
Prepared identity preserves same-mode samples. Mode-owned Default code passes
the native typed carrier to monomorphized helpers; matching grayscale owners
handle L. No architecture specialization is declared. The One strategy is
mode-bound and streams the same RGB8 projection plus truncating Pillow luma
formula directly into threshold/dither, removing the intermediate RGB raster
without changing those formulas. Palette metadata stays at the wrapper output
boundary. Different internal high-depth target identities now have direct
CoreOnly typed conversion routes. TODO: complete target-only output/fault/
metadata and selection receipts, then consider typed architecture kernels
after parity and measurement. Float limits,
non-finite values, alpha edges, and palette/dither behavior remain open. No
build or validation ran.


Convert 25-mode contract/dispatch audit closure (2026-11-07): each of the 25
exact source identities now has a reviewed target contract, Default owner,
declared architecture-to-folder/dispatch mapping, and prepared same-mode
identity decision. The inventory no longer lists the other 21 mode rows as
pending. This closes only the per-mode contract/dispatch inventory. Kernel
formula/packing cases still marked in detailed notes, prepared registry and
direct/eager caller review, and output/fault/metadata/selection/fallback and
empty-input receipts remain open before the frozen CPU parity gate. No build
or validation ran.


Draw direct-text route source audit (2026-11-07): compared the route selector
and PyO3 conversions with Pillow 12.2 `ImageDraw.text`. The native route is
limited to exact built-in text values and `ImageText.Text`, an exact Pillow mode
string matching the destination, exact built-in `FreeTypeFont` methods backed
by a native `PyFont`, integer spacing/stroke values within native ranges, exact
option types, no direction/features/language, a two-item built-in coordinate
tuple within the native range, no extra arguments, and the built-in drawing
context/font mode. These guards make the later `PyFont` cast and optional
string extraction type-safe; coordinates outside the route use `_split` and
the Python callback path. Fill and stroke ink resolution remains before route
selection, and the public wrapper synchronizes in `finally`. TODO: verify
direct-route output, fault ordering, and selection receipts in the frozen CPU
parity gate. No build or validation ran. Pillow source:
[ImageDraw.text](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L3150-L3326).


ImageOps Colorize source correction (2026-11-07): Pillow's `_color` helper
imports the current public `PIL.ImageColor` module and calls
`getcolor(color, "RGB")` for each string endpoint. The PyO3 callback now makes
that same public call instead of parsing the string directly in Rust, so a
replacement module/function and its errors remain observable. Point checks
still precede color resolution, and the Rust strategy retains LUT traversal
and arithmetic orchestration. The initial `image.mode == "L"` assertion is
also evaluated through the wrapper before point validation; the binding no
longer pre-reads `_rust_image.mode`, so subclasses and compatible wrapper
objects retain Pillow's admission behavior before public Convert/Point calls.
TODO: verify mode-override comparisons, resolver overrides, color callback
fault order, output, and exact mode/architecture receipts in the frozen CPU
parity gate. No build or validation ran. Pillow source:
[ImageOps._color and colorize](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L1726-L1734), [colorize](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L1936-L2105).


ImageOps Equalize source correction (2026-11-07): Pillow's first Equalize
branch reads the wrapper's current `image.mode` and compares it with `"P"`
before calling `convert("RGB")`. The Rust callback strategy now performs that
same comparison before histogram collection, then consults the declared P
output rule before conversion. The binding no longer reads `_rust_image.mode`
before the public wrapper property. After histogram and LUT callbacks, core
delegates final mode handling directly to the public `image.point` method and
does not re-read the DynamicImage mode. Output, wrapper mutation, mask-fault,
and selection parity remain TODO. No build or validation ran. Pillow source:
[ImageOps.equalize](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L2408-L2460).


ImageOps visible-mode audit TODO (2026-11-07): Invert, Posterize, Solarize,
and Equalize use Pillow's `_lut` mode checks, while Invert also special-cases
visible mode `1`. Invert, Posterize, and Solarize now follow public wrapper-mode
checks and call the wrapper's public `point`; Equalize now delegates post-LUT
mode handling to that same public method. Their callback-order, output, fault,
mode-override, and Point receipt cases remain open in parity. Colorize and
Equalize's initial public mode comparisons are source-corrected. Autocontrast
follows public `_lut` checks after its callbacks; its updated output, error,
and receipt parity remains open. Pillow source:
[ImageOps._lut and Invert/Posterize/Solarize](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L1735-L1756), [Invert/Posterize/Solarize](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L2683-L2757), [Equalize](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L2427-L2460). No build or validation ran.


ImageOps Autocontrast public-mode correction (2026-11-07): after histogram and LUT construction, the callback strategy now follows Pillow 12.2 `_lut` ordering: compare the current public `image.mode` with `P`, then test membership in `(L, RGB)`, then compare with `RGB` before reading LUT length. P keeps its `NotImplementedError`; unsupported modes format the current public mode in Pillow's `OSError`; RGB repeats a 256-entry LUT; L/RGB call the public `point`. The DynamicImage mode no longer controls this wrapper decision, while `OperationId::Autocontrast` still validates the selected L/RGB strategy. The old focused receipts predate this correction and must be regenerated in the frozen parity gate. Source: [Pillow 12.2 ImageOps._lut and autocontrast](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L1735-L1756), [autocontrast](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L1761-L1934). Source review only; no formatter, build, lint, tests, parity, benchmark, or coverage ran.


ImageOps Invert wrapper-mode source correction (2026-11-07): Python's ImageOps.invert now passes the image wrapper to a Rust callback orchestrator. It constructs `list(range(255, -1, -1))` through the public module namespace, evaluates visible `mode == "1"` before the shared `_lut` checks, preserves the P NotImplementedError and unsupported-mode OSError, repeats the 256-entry LUT for visible RGB, and calls public `image.point`. The `ImageOpsInvert` support descriptor declares Eval for the Python wrapper path; the direct Rust `imageops_invert` API retains its typed Invert child. Python Point owns backend/architecture execution and can honor subclass overrides. Output, callback faults, and Point selection receipts are source-only and remain parity-pending. Pillow 12.2 source: [ImageOps.invert and _lut](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L1735-L1756), [invert](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L2683-L2697). No validation ran.


ImageOps Posterize/Solarize wrapper-mode source correction (2026-11-07): both
Python wrappers now pass the public image and original parameter to Rust-owned
callback orchestration. `pillow-rs` owns the 256-entry LUT loops and the shared
`_lut` mode route; PyO3 invokes Python's `range`, arithmetic, iteration, and
comparison protocols at each source-defined step. The core also owns RGB table
repetition and support admission before calling public `image.point`.
Wrapper-mode decisions no longer come from the wrapped DynamicImage, and
public `point` overrides remain visible. The declared P
route is an explicit `NO_PILLOW_SUPPORT` native-index extension; direct Rust
Posterize/Solarize APIs retain their mode-specific operation strategies.
Output, callback/fault ordering, mode overrides, and Eval selection receipts
remain parity-pending. Pillow 12.2 source: [ImageOps._lut and
Posterize/Solarize](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L1735-L1756), [operations](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L2712-L2757). No validation ran.


ImageOps Posterize LUT construction order correction (2026-11-07): after
evaluating the mask expression, the Rust callback flow now creates the LUT
list before resolving and iterating `range(256)`, matching the list
comprehension's evaluation order. This preserves observable allocation and
monkeypatched-range faults. Callback, output, mode-override, and Eval receipt
parity remain pending; no validation ran. Pillow 12.2 source:
[ImageOps.posterize](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L2712-L2730).

ImageOps Autocontrast RGB LUT expansion source correction (2026-11-07): the
Python callback now follows shared `_lut`'s `lut + lut + lut` sequence instead
of multiplying the LUT by three. Pillow builds an exact list, so pixel values
match either way, but the source-defined allocation and sequence-operation
order is preserved. Output, fault, and receipt parity remain pending; no
validation ran. Pillow 12.2 source:
[ImageOps._lut](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L1735-L1756).

ImageOps Colorize support-route correction (2026-11-07): full Colorize wrapper
support and its post-Convert `_lut` stage now have separate declarations under
`pool_cpu/ops/imageops/colorize`. The wrapper route declares L → Convert(RGB)
→ ColorizeLut and RGB output; the callback checks the converted wrapper's
current public mode before Point, preserving subclass `convert()` behavior.
The final LUT route admits L/RGB, declares Pillow's P NotImplementedError, and
records OSError for other modes. Direct Rust Colorize remains its own typed
operation. Its PyO3 adapter reuses the shared ImageOps callback object for
mode checks, LUT expansion, and public Point. Subclass mode overrides, output
metadata, faults, and Eval selection/fallback receipts remain TODO. No
validation ran.

ImageOps Colorize Pillow 13 error-contract correction (2026-11-10): rejected
modes now return Pillow's `ValueError("mode must be L, not <mode>")`, and
invalid black/white or three-color point ordering returns Pillow's exact
`ValueError` text. The adapter fetches and stringifies `image.mode` only after
the initial mode comparison rejects the image. The full 45-case Colorize
selection passed against Pillow 13.0.0; see the current evidence entry in
`CPU_REFACTOR_INVENTORY.md`. This focused result does not replace full CPU
parity or architecture receipt requirements.

Draw Bitmap clipped-region continuation (2026-11-07): every byte-mode
Default Bitmap writer now traverses only the `BitmapRegion::clipped`
intersection and maps the destination origin once, then advances within the
proven in-bounds region. This removes per-sample signed coordinate checks while
retaining each exact-mode blend loop and its mask order. I;16 byte-lane and RGB
specialized Bitmap loops use the same origin mapping. Direct `i32::MIN`/
`i32::MAX` origins are safe by source arithmetic: clipping promotes the signed
origin and unsigned extents to `i64`, and bounds each mask range so every
destination coordinate is within the image. The nonempty region's first
destination coordinate is checked once; subsequent in-region increments stay
below the image dimensions. I;16 and P text paths also clip first and add in
`i64` per sample. Extreme-origin output and fault parity remain TODO; no
validation ran.

Unit-test cleanup audit (2026-11-07): the active Rust tree has no `#[test]` or
`#[cfg(test)]` unit-test blocks, and the active repository has no JavaScript
unit-test suite. The Python files named `scripts/test_*` are isolated Pillow
parity diagnostics (some use `unittest` only as a runner), so they remain with
the parity suite. `pillow-rs/src/test_api.rs` exposes deterministic controls
and support receipts for migration parity; it is not a unit-test suite. The
benchmark harnesses remain. No unit-test files were removed, and
`pool_cpu/legacy` was not changed.

PutAlpha La source correction (2026-11-07): Pillow 12.2 cannot change `La` to
`LA` in place, so `Image.putalpha` converts first; the La→LA converter
unpremultiplies luma and retains converted physical lanes before scalar or mask
alpha replacement. Active PutAlpha and PutAlphaData now implement that result
directly and admit La. The Pillow 12.2 `1`→`LA` conversion remains unsupported,
so its rejection stays. CPU parity for La output, mode transition, fault order,
and architecture receipts remains TODO under `pillow-rs#3ptb`; no validation
ran. See the detailed source links and implementation note in
`docs/CPU_REFACTOR_INVENTORY.md`.

PutAlpha LAB rejection now reports Pillow's converter-derived
`conversion from LAB to RGBA not supported` ValueError. The same source review
confirmed that YCbCr and HSV target RGBA errors, and checked the remaining
rejected mode routes against Pillow 12.2's mode bases and converter table.
No validation ran; details and references are in
`docs/CPU_REFACTOR_INVENTORY.md`.

RGBa PutAlpha arithmetic source review (2026-11-07): Pillow 12.2's
`rgba2rgbA` converter preserves each color component unchanged when the source
alpha is 0 or 255; otherwise it computes integer-floor
`255 * component / alpha` and clips to a byte. `unpremultiply_byte` matches
that formula, and both constant-alpha and mask PutAlpha use it before replacing
the alpha band. Edge-value output and architecture-receipt parity remain TODO in
`pillow-rs#3ptb`. Source: [Pillow 12.2 `Convert.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L426-L442). No validation ran.

ImageOps `_lut` mode-property source correction (2026-11-07): Pillow 12.2
reads the current public `image.mode` separately for `_lut`'s P check, tuple
membership check, RGB check, and unsupported-mode error. Equalize now follows
those separate property reads after histogram/LUT construction, and raises
`NotImplementedError` when a palette conversion override still returns P. The
focused 48-case Equalize parity evidence predates this source correction and
must not be treated as current evidence. The matching behavior for
Autocontrast, Invert, Posterize, and Solarize remains callback-driven; tuple
membership adapters now fetch the left-hand mode value before creating the
tuple. No build or parity ran after this correction; full output, callback/fault
ordering, and strategy receipt verification remain pending. Pillow source:
[ImageOps._lut](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L47-L58),
[ImageOps.equalize](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L417-L444).

ImageOps mode callback helper consolidation (2026-11-07): the PyO3 binding
shares mode equality, L/RGB membership, and unsupported-mode error helpers
across the ImageOps LUT, Autocontrast, and Equalize adapters. Every callback
still retrieves the visible `image.mode` at its own source-defined step; the
adapter does not cache a mode value across `_lut` checks. This removes
duplicated boundary code while keeping operation support and typed strategy
selection in `pillow-rs`. No build or validation ran.

ImageOps unsupported-mode formatting correction (2026-11-07): the shared
PyO3 error helper now formats the public mode value with an empty format spec
through the fixed string template `"not supported for mode {}"`, rather than
calling `str(mode)`. This follows Pillow's f-string `__format__` behavior and
propagates custom formatting errors before constructing the `OSError`; each
operation still fetches `image.mode` at its own callback point. This is
source-only. No build or parity ran, so custom `__format__` output/fault and
wrapper callback receipts remain pending.

ImageOps Colorize string detection correction (2026-11-07): Pillow's `_color`
uses module-global `isinstance(color, str)` before importing `ImageColor` and
calling `getcolor`. The PyO3 callback now resolves `isinstance` and `str`
through the ImageOps module/builtins lookup helper, calls it, and truth-tests
the result before resolving a string color. Direct type inspection no longer
skips those observable protocols. Source-only; lookup, truthiness, resolver,
and downstream output/fault parity remain pending. Source: [Pillow 12.2
ImageOps._color](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L1726-L1734).

ImageOps type-check truthiness correction (2026-11-07): Expand's fill-string
and tuple checks, the shared border-tuple check, and Scale's native-image
eligibility now truth-test the result of the module/builtin `isinstance` call.
This matches their Python `if isinstance(...)` use even when a replaced global
returns a non-boolean truthy value; strict PyO3 `bool` extraction had rejected
that result. Source-only; callback faults and route/output parity remain
pending.

ImageOps Expand public color resolver correction (2026-11-07): string fills
now resolve `PIL.ImageColor` at the callback point and call its current
`getcolor(fill, mode)`, matching Pillow's shared `_color` helper instead of
importing the internal color module directly. This preserves a replaced public
module or `getcolor` callable and its faults. Source-only; callback, palette,
output, and error-order parity remain pending. Source: [Pillow 12.2
ImageOps._color and expand](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L1726-L1734), [expand](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L2462-L2514).

ImageOps Expand geometry callback cleanup (2026-11-07): removed an adapter
callback that constructed a four-item `(width, height, left, top)` tuple and
immediately discarded it. Pillow constructs the size and paste-position tuples
at their actual public-call boundaries; the extra tuple added a spurious
allocation/fault point. Source-only; callback and allocation-fault parity
remain pending.

ImageOps Expand palette-fill length ordering correction (2026-11-07): the
P/PA tuple-fill condition now evaluates `len(fill) == 3` and then, only when
false, performs a second `len(fill) == 4` callback. Each step resolves the
current module/builtin `len`, uses host equality, and truth-tests its result,
matching Pillow's short-circuit expression. This preserves monkeypatched
length/comparison behavior that the previous single `PySequence_Size` call
skipped. Source-only; callback, palette mutation, and output/fault parity
remain pending. Source: [Pillow 12.2 ImageOps.expand](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L2493-L2502).

ImageOps Expand load-order correction (2026-11-07): removed an explicit
`output.load()` after public `output.paste()`. Pillow 12.2 returns immediately
after `out.paste(image, (left, top))`; this wrapper's public `paste` already
loads the destination. The extra call repeated load-time callbacks and could
observe a second overridden `load` result/fault. Callback-order and output
parity remain pending. Source: [Pillow 12.2 ImageOps.expand](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L2485-L2514).

ImageOps Posterize callback-order correction (2026-11-07): the Rust-owned LUT
orchestration now obtains the `range(256)` iterator before allocating the
result list, matching Python's list-comprehension evaluation order. This keeps
module-replaced `range` side effects and faults ahead of result-list allocation;
Solarize still allocates its explicit empty list before resolving `range`, as
its source loop requires. Source-only; callback/fault parity remains pending.

PutPixel reentrant palette source review (2026-11-07): Pillow resolves a P/PA
palette color before looking up `self.im` for the final core write. The current
facade evaluates `_rust_image.putpixel_mode` first, so that binding retains the
pre-callback PyImage if palette component conversion reentrantly replaces the
wrapper's core. Add a staged boundary that resolves the write target after
palette callbacks and before coordinate parsing, then verify mode, pixel, and
fault outcomes in the frozen CPU parity matrix. A TODO is recorded at the
facade call site; no implementation or validation change was made.

PutPixel palette target capture correction (2026-11-07): the Python facade now
performs Pillow's P/PA tuple-color checks and calls the current public
`ImagePalette.getcolor` before evaluating `_rust_image.putpixel_mode`. The Rust
call therefore captures the wrapper's current core after palette callbacks
and before coordinate parsing. Palette state remains dirty in the Python
ImagePalette until the existing load boundary realizes it; the adapter no
longer overwrites that state from an earlier Rust core after write errors.
Source-only; callback, mode, pixel, fault, metadata, and selection-receipt
parity remain pending. No build or validation ran.

Equalize fast-path guard investigation (2026-11-07, superseded): an earlier
version attempted to recognize replaced `range`, `list`, `len`, and `sum`.
Builtin mutation is outside the supported compatibility contract, so those
identity checks were later removed. No replacement-specific behavior or
parity is required. No validation ran. The investigation referenced
CPython's `Py_TPFLAGS_HEAPTYPE` in [`object.h`](https://github.com/python/cpython/blob/3.12/Include/object.h).

Scoped CPU WIP verification (2026-11-08): after the Equalize callback edits,
`make build-parity` exposed and then cleared two binding compile blockers: a
missing `Error` associated type on `PythonAutocontrastCallbacks` and a stale
`PyStringMethods` import. A fresh build succeeded. The frozen 689-case CPU WIP
selection in `build/migration-parity/cpu-wip-case-ids.txt` then passed 689/689
with zero infrastructure errors (`migration-parity-343d0ca427754575a5effbabdf63a9bd`);
this includes all 10 Equalize differences from the earlier 689-case run. The
48-case Equalize selection also passed 48/48 on the same current build. These
are scoped WIP results, not the complete 17,541-case CPU denominator or the
mode-by-architecture receipt matrix. Source completion, full CPU parity,
quality gates, and performance acceptance remain open. No coverage ran.

Rejected PutAlpha mode cleanup (2026-11-08): the six source modes rejected by
the shared PutAlpha contract (`One`, LAB, YCbCr, HSV, I, and F) now expose only
their 25-mode support entries. Their unreachable Default kernels were removed
from the maintained tree, and the prepared alpha enums now contain only the
19 executable modes. PutAlphaData still constructs and validates the mask
strategy before returning the declared source-mode rejection. No pixel
algorithm or error contract changed. This is source-only; rejection ordering,
all supported mode outputs, and architecture/fallback receipts still require
the planned CPU parity matrix. No validation ran after this source edit.


Resize support ownership correction (2026-11-08): `Resize` and `ResizeBoxed`
now keep each exact mode's support contract beside that mode's concrete
architecture dispatch. Their operation entry point exhaustively aggregates the
25 mode contracts and retains the existing Default fallback declarations.
Common byte and I;16 architecture requirement construction is in
`resize/helpers/support.rs`; I32, F32, and CoreOnly high-depth owners declare
their exact strategies locally. The support modes, guards, target requirements,
and Default availability were moved without changing the accepted matrix or
pixel algorithms. A source-only mapping audit found both operation contracts
for all 25 identities. This change is uncompiled and unvalidated; Resize
caller, output, fault, and architecture-receipt review remains open.

Draw support ownership correction (2026-11-08): the 16 byte/I32/F32 mode owners
now declare their own Default/SSE2/NEON/Wasm SIMD128 strategies, the four I;16
owners declare Default-only support, and the five internal high-depth owners
declare the existing unsupported-mode rejection beside `NO_PILLOW_SUPPORT`.
`draw/ops.rs` retains exhaustive mode dispatch and the shared operation input
contract; common architecture guards and support constructors live in
`draw/helpers/support.rs`. A static source audit mapped all 25 exact modes and
found no remaining family-level mode support tables. The support modes and
guards were preserved by inspection. This is uncompiled and unvalidated;
Draw source closure and frozen output, fault, and architecture-receipt parity
remain open.

CropBorder support ownership correction (2026-11-08): all 25 exact-mode
ImageOps owners now declare the CropBorder contract. The four I;16 owners keep
their existing SSE2/NEON/Wasm SIMD128 copy strategies beside the mode; the
other 21 owners declare the same Default-only support as before. The family
file retains the shared input contract, exhaustive aggregation, and prepared
Crop composition. Common serial contract construction uses the existing
ImageOps helper. The mode set, guards, and prepared Crop child were preserved
by inspection. This is uncompiled and unvalidated; output, fault, empty-image,
and architecture-receipt parity remain open.

Current I;16 Convert failure rerun (2026-11-08): `make build-parity` first
exposed six rejected-mode macro invocations using a semicolon unsupported by
the macro matcher and an unused `InputContract` import in Draw support. The
macro matcher and import were corrected; the release extension then built
successfully. The 16 frozen I;16 Convert cases from the historical full-run
failure set passed 16/16 on the current working source with zero
infrastructure errors, CPU profile `python-cpu`, report
`build/migration-parity/cpu-convert-i16-historical-failures-current-20261108.json`,
run ID `migration-parity-73ab2d4feee0443b86c718ef2391e6a4`, and manifest
SHA-256 `6dcb1bcf4b8f6122302c726cf4a9995b7da8a7c1ec901d19e28a2b42d87963cc`.
This completes the source TODO to rerun the 16 active mode-audit IDs after the
luma16 clipping change. It does not prove the historical payloads because the
current input corpus differs from the historical full report. The current
17,541-case CPU corpus and operation × exact Mode × Architecture receipt
matrix remain unverified. No coverage or benchmark ran; full source and static
gates remain open.

Current historical-status guard rerun (2026-11-08): a 27-case CPU cohort
selected the currently active Posterize/Solarize guard IDs plus the I-to-PNG
save case. The 26 operation guards passed; the save case failed because the
target codec reports `unsupported Png: encode: PNG cannot encode this image
mode`. Report `build/migration-parity/cpu-current-status-codec-guards-20261108.json`,
run ID `migration-parity-e7905ff4cc01447c9ea15f602bf57ca8`, input hashes
`ff4a74d88dc28bc1c5ee39c21c9a781eef66245b33dd0f17709888cb62101568` and
`94594526a048d451f577e0a429410a38008aab8a50a0f7f208b4b0cd507786d3`.
Thirteen IDs from the historical status cohort are absent from current inputs;
this cohort does not replay those exact old inputs. No source behavior changed
after the run; no coverage or benchmark ran.

Reduce byte-architecture selection source trace (2026-11-08): the seven
mode-owned byte strategies (`L`, `LA`, `RGB`, `RGBA`, `CMYK`, `YCbCr`, and
`HSV`) bind AVX2 to the dedicated `kernels/avx2.rs` implementation and bind
SSE2, NEON, and Wasm SIMD128 to the shared `kernels/wide.rs` lane kernel. The
AVX2 arithmetic is inside `#[target_feature(enable = "avx2")]` functions; the
prepared strategy is selected through `ModeSupport`, which checks injected
architecture policy, host capabilities, build availability, requirements, and
the Reduce guard before dispatch. The earlier source-level concern that an
AVX2 request could accidentally execute the portable-wide path was not
confirmed. This traces declarations and source dispatch only; compiler codegen,
outputs, fault behavior, and selected/fallback receipts remain unverified. The
AVX2/wide algorithms still need their completed legacy comparison before CPU
source review can close. No validation ran.

Reduce typed-u16 source trace (2026-11-08): the four Pillow-spelled I;16
identities and internal LumaA16/RGB16/RGBA16 owners pass their typed `u16`
buffers to `helpers/native_samples.rs` and map the result back to the same
DynamicImage variant. The path does not reinterpret bytes or convert modes.
Each output sample uses a `u128` block sum and half-up integer division; the
four Pillow-spelled I;16 strategies are explicitly `NO_PILLOW_SUPPORT`
extensions, as native typed Reduce is outside Pillow's I;16 Reduce contract.
This confirms the source storage and output-identity path only; output and
boundary behavior, exact support/fallback receipts, and the full Reduce source
review remain open. No validation ran.

Superseded: Reduce One extension discussion (2026-11-08): the proposed byte
average is not part of the current One strategy. Pillow rejects nonidentity
mode `1` reduction; the maintained reducer therefore preserves the fast
factor-one copy/crop contract and rejects nonidentity factors. No threshold or
byte-average conversion is needed to represent a successful Pillow outcome.
Pillow-visible output and rejection parity still need their frozen CPU cases;
no validation ran. Pillow's public wrapper preserves the factor-one copy and
delegates nonidentity reduction to its core; that core rejects mode `1` before
running a reducer ([wrapper](https://raw.githubusercontent.com/python-pillow/Pillow/12.2.0/src/PIL/Image.py#L2238-L2269),
[core](https://raw.githubusercontent.com/python-pillow/Pillow/12.2.0/src/libImaging/Reduce.c#L1415-L1426)).

Reduce F32 source comparison (2026-11-08): the exact Pillow `F` Default path
in `helpers/scalar32.rs` preserves the archived interior f32 pair/quartet
addition order before f64 accumulation, uses f64 per-sample accumulation on
partial edge blocks, and stores the reciprocal-scaled result as little-endian
f32. AVX, SSE2, and Wasm wrappers use the shared eight-lane 2x2 kernel only
when factors are 2×2, dimensions are even, and output width is divisible by
eight; otherwise they call the same-mode Default reducer. NEON uses two
four-lane structure-load groups while retaining the same quartet order and
guard. The architecture/kernel source comparison is complete for this exact
F path; output, NaN/edge, compilation, and strategy/fallback receipts remain
unverified. No validation ran.

PutAlpha CMYK architecture source comparison (2026-11-08): the archived SIMD
block and maintained mode-bound shared kernel match in four-pixel grouping,
channel gathers, MULDIV255 rounding, RGB/alpha interleave, and scalar tails.
`ImageCmyk8` binds the guarded route to SSE2, NEON, and Wasm SIMD128 strategies
and keeps the same-mode Default fallback. Mode-1 mask normalization is fused
into vector and tail writes, matching Pillow for valid bilevel samples; the
existing noncanonical-One sample TODO remains. This closes the CMYK SIMD source
comparison only. Runtime output, callback/fault order, receipts, and latency
remain pending; no CMYK parity was run.

PutAlpha declaration census (2026-11-08): the family classifier covers all 25
canonical Mode identities exactly once, with 19 accepted strategies and six
explicit rejections; every exact-mode `ops.rs` is present and exposes both
PutAlpha and PutAlphaData declarations. This is a source-coverage census only;
mode semantics, faults, metadata, and architecture/fallback execution remain
subject to source review and the frozen CPU parity matrix.

PutAlpha architecture inventory (2026-11-08): constant alpha declares SSE2,
NEON, and Wasm SIMD128 for `L`, `P`, `RGB`, `RGBA`, `RGBX`, and `CMYK`; mask
alpha declares those architectures for `L`, `LA`, `P`, `PA`, `RGB`, `RGBX`,
and `CMYK`. The remaining accepted identities are Default-only for the
respective route. All 39 declared leaves have linked mode-local wrappers and
bind a kernel. This closes only declaration-to-file source mapping; runtime
selection, fallback receipts, output parity, and latency remain pending.

PutAlpha RGBa arithmetic source comparison (2026-11-08): the archived
RGBa-to-RGBA conversion and maintained `unpremultiply_byte` agree for zero
alpha and for floor-and-clamp `color * 255 / alpha`; exact-mode Default owners
fuse the conversion and alpha write into the final RGBA allocation. These
operations declare no architecture kernel, so non-Default architecture
requests use the declared Default fallback. Alpha-edge output, mask, fault,
receipt, and latency checks remain pending; no validation ran.

PutAlpha La source comparison (2026-11-08): Pillow's `la2lA` writes the
unpremultiplied luma into the repeated physical LA lanes before `putalpha`
replaces alpha. The maintained Default scalar and mask owners fuse those
steps, use the same zero/255 and floor/clamp arithmetic, and synthesize the
same repeated luma lanes without an intermediate frame. The mode-1 mask path
normalizes valid bilevel samples to 0/255. These alpha strategies are
Default-only; edge output, faults, receipts, and latency remain pending. No
validation ran.

PutAlpha rejected-mode and admission source closure (2026-11-08): Pillow 12.2
maps `1`, `I`, and `F` to base `L`, so `Image.putalpha` requests `LA`; it maps
`YCbCr`, `LAB`, and `HSV` to base `RGB`, so it requests `RGBA`. Its
`ImagingCore.setmode` only relabels the RGB/RGBA/RGBX family, and the remaining
in-place converter only handles `1`↔`L`. The conversion table has no `1`, `I`,
or `F`→`LA`, `YCbCr` or `HSV`→`RGBA` route, and no LAB converter. The six
exact-mode Rust declarations therefore match Pillow's rejected source set.
Support reporting and rejected preparation derive the same per-mode
`ModeSupport`. The CPU `PutAlphaData` preparer now validates that destination
declaration before constructing its mask strategy, so raw pipeline preparation
preserves destination-before-mask rejection order as the public Image wrapper
does. This closes source review of rejected-mode classification and that
admission boundary only; output, exact errors, mutation/callback order,
selection receipts, and runtime parity remain pending. No build, formatter,
checkstyle, lint, test, parity, benchmark, or coverage ran. Sources: [Pillow
12.2 `ImageMode.py`](https://raw.githubusercontent.com/python-pillow/Pillow/12.2.0/src/PIL/ImageMode.py#L37-L58), [`Image.putalpha`](https://raw.githubusercontent.com/python-pillow/Pillow/12.2.0/src/PIL/Image.py#L1890-L1929), [`_imaging.c::im_setmode`](https://raw.githubusercontent.com/python-pillow/Pillow/12.2.0/src/_imaging.c#L1924-L1966), and [`Convert.c`](https://raw.githubusercontent.com/python-pillow/Pillow/12.2.0/src/libImaging/Convert.c#L1372-L1462), [`ImagingConvertInPlace`](https://raw.githubusercontent.com/python-pillow/Pillow/12.2.0/src/libImaging/Convert.c#L1641-L1661).

PutAlpha P/PA palette metadata source trace (2026-11-08): Pillow's mode table
defines `P` with base mode `P`, so `Image.putalpha` promotes it to `PA`; the
wrapper retains its existing palette while the image core changes samples.
The maintained planner marks P/PA PutAlpha as palette-safe, declares P→PA and
PA→PA output identities, and carries both palette RGB entries and palette
alpha through the lazy result and materialization metadata. No source mismatch
was found. The four active P/PA PutAlpha cases passed in the prior 689-case
dirty WIP run, but those workflows observe call status or `tobytes()` only;
they do not assert the output mode, attached palette, palette alpha, or a PA
mask input. Those output/metadata/fault/receipt dimensions remain in the CPU
parity TODO; this source trace is not current-snapshot parity evidence. No
validation ran. Sources: [Pillow 12.2 `ImageMode.py`](https://raw.githubusercontent.com/python-pillow/Pillow/12.2.0/src/PIL/ImageMode.py#L37-L58), [`Image.putalpha`](https://raw.githubusercontent.com/python-pillow/Pillow/12.2.0/src/PIL/Image.py#L1880-L1929), [`Image._new`](https://raw.githubusercontent.com/python-pillow/Pillow/12.2.0/src/PIL/Image.py#L629-L642), and [`Convert.c::frompalette`](https://raw.githubusercontent.com/python-pillow/Pillow/12.2.0/src/libImaging/Convert.c#L1028-L1088).


PutPixel P/PA palette ownership review (2026-11-08): the public Python path
calls `ImagePalette.getcolor()` before coordinate parsing, preserving Pillow's
palette callback, allocation-error, and palette-cache ordering. The default
allocation algorithm currently lives in Python `imagepalette.py` while
`pillow-rs::Image::prepare_putpixel_palette` has a separate native allocator;
the public Python path bypasses the Rust allocator after resolving the color.
Move the default allocation strategy and pixel-index choice into core while
retaining host calls for overridden `getcolor` methods and Python-value
conversion. Preserve palette mutation on allocation failures as well as
success. This remains implementation debt beside the TODO in `image.py` and
requires focused mode, callback, error-order, and output parity before it can
be considered complete. No validation ran. References: [Pillow 12.2
`Image.putpixel`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2017-L2037),
[`ImagePalette.getcolor`](https://raw.githubusercontent.com/python-pillow/Pillow/12.2.0/src/PIL/ImagePalette.py#L106-L169),
and the maintained [`ImagePalette` facade](../pillow-rs-py/python/pillow_rs/imagepalette.py)
plus [`Image.putpixel`](../pillow-rs-py/python/pillow_rs/image.py).

PutPixel default palette slot selection follow-up (2026-11-08): added a shared
core selector for the base `ImagePalette` path. The Python facade still calls
`histogram()` at Pillow's public boundary and owns palette bytes, cache updates,
dirty state, and exception causes; exact built-in inputs delegate only the
slot choice to Rust. Subclasses and values with observable Python comparison
or iteration behavior keep the compatibility fallback. Core PutPixel and
`ImagePalette` now share the same next-slot and highest-unused helpers. This is
source-only; P/PA callbacks, reentrant mutation, metadata, error ordering, and
fallback parity remain open. No build or validation ran.

Reduce premultiplied and indexed-mode source review (2026-11-08): `La` and
`RGBa` are separate exact-mode owners with Default-only strategies. Their
reducers pass the stored two- and four-byte samples to the native byte kernel
with premultiplication disabled, so each stored lane is averaged directly and
the result retains the same exact mode. This matches the archived
`reduce_alpha_layout` exclusions for `La` and `RGBa`; it does not run the
straight-alpha `LA`/`RGBA` premultiply-and-restore path. The P owner admits only
identity factors, returns the same-mode copy for `(1, 1)`, and rejects a real
reduction; this follows Pillow's wrapper identity route and `ImagingReduce`
mode rejection. PA remains a two-sample mode and its Default reducer averages
the palette index and alpha independently, preserving PA identity. The source
paths and exact-mode selections agree with the archive and Pillow's Reduce
implementation. This closes only these mode source comparisons; One's
CoreOnly output normalization, remaining mode source comparisons, caller and
fault order, execution/fallback receipts, parity, and latency remain open. No
build, formatter, checkstyle, lint, test, parity, benchmark, or coverage ran.
Sources: [Pillow 12.2 `Image.reduce`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2238-L2269)
and [`Reduce.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Reduce.c#L48-L84).

Reduce non-alpha byte-mode source review (2026-11-08): `CMYK` and `RGBX` use
four stored byte samples, and their Default owners invoke the native
four-channel reducer without alpha interpretation. `CMYK` also binds its
declared AVX2 and portable-wide architecture routes to the four-channel
non-premultiplied kernel; `RGBX` remains Default-only, matching the archived
SIMD admission map. `YCbCr`, `HSV`, and `LAB` use three native samples and the
three-channel non-premultiplied reducer without color conversion. The first
two retain their archived AVX2 and portable-wide declarations; `LAB` remains
Default-only. Pillow's bands-three Reduce loop averages lanes 0–2 and writes
zero to physical lane 3. The compact Rust RGB/YCbCr/LAB/HSV carriers preserve
that documented lane policy by retaining source lanes on identity and setting
omitted lanes to zero after actual reduction. Pillow allocates those output
lanes dirty, so this deterministic Rust choice does not prove follow-on raw
byte parity for the omitted lane. These exact-mode owners and architecture
leaves match the source maps; remaining mode comparisons, generated
instructions, output/fault parity, receipts, and latency remain open. No
build, formatter, checkstyle, lint, test, parity, benchmark, or coverage ran.
Source: [Pillow 12.2 `Reduce.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Reduce.c#L85-L176).

Reduce CoreOnly multiband high-depth source review (2026-11-08): the five
internal identities `LumaA16`, `RGB16`, `RGBA16`, `RGB32F`, and `RGBA32F` each
declare `cpu_core_only` support with a Default strategy and exact-mode output
identity. The four u16 storage modes and three internal u16 multiband modes use
the typed `reduce_u16` implementation directly; every channel accumulates in
`u128`, and each partial or full block uses half-up integer division by its
actual pixel count. The RGB32F/RGBA32F identities use typed `reduce_f32`,
accumulating native channel samples in f64 and storing f32 results without
changing layout or converting images. Mode-specific owners return the same
DynamicImage variant, and none advertises an architecture kernel. This source
review confirms implementation and support mapping for the multiband CoreOnly
modes; exact-I;16 parity extensions, One's output contract, overflow/NaN
boundaries, and all runtime receipts remain open. No build, formatter,
checkstyle, lint, test, parity, benchmark, or coverage ran.

Reduce One support correction (2026-11-08): supersedes the earlier proposal
for a CoreOnly byte-average extension. The parity-first contract keeps the
Pillow-observable copy for full-image factor `(1, 1)` requests and crops
factor-one sub-boxes; nonidentity Reduce rejects `One` with
`ValueError("image has wrong mode")`, matching Pillow's `ImagingReduce` mode
rejection. The exact-mode owner declares an identity-factor
`ModeSupport::cpu` contract, and its Default leaf returns the same-mode copy
for the valid full-image identity input. It does not emit non-bilevel gray
bytes tagged as `One`. This closes the output-contract design TODO without
inventing an extension.
Source-only; copy/crop/rejection output ordering and selection receipts remain
unverified. No build, formatter, checkstyle, lint, test, parity, benchmark, or
coverage ran. Sources:
[Pillow 12.2 `Image.reduce`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2238-L2269)
and [`ImagingReduce`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Reduce.c#L1415-L1426).

Geometry ratio-comparison correction (2026-11-08): Pillow 12.2 `ImageOps.contain`
and `cover` compare `im_ratio != dest_ratio` exactly before rounding the
adjusted axis. `pad` delegates its sizing to `contain`. The maintained CPU
geometry helpers had treated ratios within `1e-10` as equal, which can skip a
rounded-axis update for large dimensions. Contain, Cover, and Pad now use the
same exact f64 equality check as Pillow. Source correction only; tie-rounding,
zero-sized input, callback/error order, output, and receipts remain for the
frozen CPU parity matrix. No build, formatter, checkstyle, lint, test, parity,
benchmark, or coverage ran. Source: [Pillow 12.2 `ImageOps.py`](https://raw.githubusercontent.com/python-pillow/Pillow/12.2.0/src/PIL/ImageOps.py#L249-L303).

PutPixel palette allocator ownership correction (2026-11-08): the default
`ImagePalette` slot path now delegates both next-slot selection and
full-palette highest-unused selection to the same `pillow-rs` core function
used by native `Image::putpixel`. The Python facade still performs public
color lookup and host `image.histogram()` calls in Pillow order, and owns the
palette bytes, color cache, dirty flag, and Python exception cause. Subclasses
or values with observable custom comparison/iteration behavior retain the
compatibility path. Removed the per-image PyO3 forwarding method; one internal
module function exposes the shared core selector. The source-level ownership
gap is resolved; callback/reentrancy, reserved metadata, failure mutation,
fallback/output parity, and latency remain unverified. No build, formatter,
checkstyle, lint, test, parity, benchmark, or coverage ran.

Pad prepared-path zero-height correction (2026-11-08): the public ImageOps
wrapper rejects zero source/target heights during its contain preflight, but
callers can append `PipelineOp::Pad` directly and reach the CPU strategy
without that adapter. The exact-mode Pad dimension builder now returns the
same `ZeroDivisionError("division by zero")` before floating-point ratio
calculation for zero source or target height. Prepared Pad now computes this
geometry before support/strategy admission as well, preserving Pillow's
contain-before-resize/fill order for direct Rust pipeline inputs. Output/fault
order and direct Rust receipts remain in the frozen CPU parity work. No build,
formatter, checkstyle, lint, test, parity, benchmark, or coverage ran.

Prepared Contain/Cover/Fit/Scale ordering correction (2026-11-08): direct
`PipelineOp` preparation now computes geometry before selecting mode and
architecture strategies, matching the public ImageOps wrappers' eager
geometry checks. Fit also applies the facade's invalid-bleed and out-of-range
centering normalization to direct typed operations. Scale reuses the facade's
ties-to-even dimension preflight, checks integer overflow and zero-sized
results before strategy admission, and keeps identity and all-empty copies.
This is source-only; direct pipeline error/output and architecture-receipt
parity remain pending. No build, formatter, checkstyle, lint, test, parity,
benchmark, or coverage ran.

Reduce 25-mode source closure (2026-11-08): the per-mode Default comparisons
now cover all 25 exact identities: shared native-byte reducers and RGB sparse
paths, native I/F scalar carriers, typed-u16 paths, and internal multiband f32
paths. The seven declared byte architecture families and F32's guarded vector
routes are mapped to their kernel leaves and Default fallbacks. One and P keep
their identity-only contracts; omitted physical lanes retain the source on
identity and use the documented Rust zero policy after reduction. Source
review is complete for Reduce; output/fault, CoreOnly extension, hidden-lane,
architecture-receipt, and performance evidence remains gated on the frozen
CPU campaign. No build, formatter, checkstyle, lint, test, parity, benchmark,
or coverage ran.

Resize boxed and caller source review (2026-11-08): compared the public
`Image.resize` flow with the pinned Pillow 12.2 implementation. The maintained
typed facade keeps filter/reducing-gap validation and the same-size full-box
copy boundary ahead of execution, normalizes `One`/`P` to nearest, skips the
reducing-gap path for filtered straight `LA`/`RGBA`, and preserves the
vertical-then-horizontal pass order for tall boxed inputs. Direct
`PipelineOp::Resize`/`ResizeBoxed` preparation, the CPU registry route, and
nested Resize children from Geometry, Pad, and Thumbnail were traced to their
injected context and exact-mode support declarations. Boxed coordinates are
narrowed to float32 and validated before strategy selection; exact crop and
full-source shortcuts use that same boundary, while filtered straight-alpha
crops retain premultiply/resample/unpremultiply work. Tall `LA`/`RGBA` stages
carry `La`/`RGBa` mode identities between the two prepared child plans. The
architecture guards leave tall, invalid-shape, and out-of-range coefficient
requests on the matching mode's Default route. This closes the remaining CPU
Resize caller/boxed-order source review; the GPU compatibility helper remains
for the GPU stage. Output/fault, actual selection receipts, and performance
remain pending. No build, formatter, checkstyle, lint, test, parity, benchmark,
or coverage ran. Reference: [Pillow 12.2 `Image.resize`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2170-L2237).

Source review continuation (2026-11-08): Reduce `One` follows Pillow's public
identity/crop/error outcomes; no averaging or threshold kernel is warranted.
The shared `ReduceFactors` contract rejects zero factors before architecture
strategy selection for both support assessment and execution admission.
Prepared Contain, Cover, and Pad now share the public ImageOps dimension,
rounding, and zero-output helpers. These source changes remain unvalidated;
frozen CPU parity, architecture receipts, and performance are still required.

PutAlphaData source-review continuation (2026-11-08): noncanonical `One` mask
samples are normalized to bilevel `L` alpha values in the typed Default and
SIMD paths, including vector blocks and tails. This is source-closed; the
frozen parity cases, output/fault ordering, receipts, and performance remain
pending.

Reduce One dispatch correction (2026-11-08): the shared identity-only support
contract owns non-identity rejection before architecture selection; the One
Default leaf now carries no factors and only clones its valid identity input.
This removes a duplicate guard from the kernel path. Runtime parity and
selection receipts remain pending.

PutData callback writer source closure (2026-11-08): `PutDataValueAt` now uses
the injected selection's exact mode together with its matching `DynamicImage`
variant to dispatch to that mode's `default/put_data_impl.rs`. The byte and
I;16 paths are separate at the mode leaf; shared helpers only copy normalized
bytes or update compact-carrier hidden lanes. The support declaration remains
the admission owner, and each value is still written before Python coerces the
next sequence item. This closes the PutDataValueAt source-dispatch review for
its 16 byte-addressable modes and four I;16 identities. Callback-visible
partial writes, coercion faults, raw-lane outputs, mode receipts, and latency
remain pending in frozen CPU parity/performance evidence. Source-only; no build,
formatter, checkstyle, lint, test, parity, benchmark, or coverage ran.
Reference: [Pillow 12.2 `_imaging.c::_putdata`](https://raw.githubusercontent.com/python-pillow/Pillow/12.2.0/src/_imaging.c#L1496-L1645).

Synchronous PutPixel and I;16 byte-writer dispatch source closure (2026-11-08):
`PutPixelValueAt` and `PutDataL16Bytes` now match the injected exact mode with
the corresponding `DynamicImage` variant before calling each mode's Default
leaf. The support inventory admits six `PutPixelValueAt` modes (I, F, and the
four I;16 identities) and four I;16 byte-write modes. Pillow's `_putpixel`
checks coordinates before native ink conversion; the Python palette route
resolves palette colors before coordinate conversion, preserving mutation
ordering. Pillow's I;16 bytes path copies raw row prefixes, while scaled bytes
clip per byte and unmodified bytes remain intact. Rust keeps typed u16 storage
and updates only the addressed byte lanes. Source review only; output, partial
mutation/fault order, receipt, and performance parity remain pending. No build,
formatter, checkstyle, lint, test, parity, benchmark, or coverage ran.
References: [Pillow 12.2 `_imaging.c::_putpixel`](https://raw.githubusercontent.com/python-pillow/Pillow/12.2.0/src/_imaging.c#L1772-L1805), [Pillow 12.2 `_imaging.c::_putdata`](https://raw.githubusercontent.com/python-pillow/Pillow/12.2.0/src/_imaging.c#L1496-L1589), and [Pillow `Image.putpixel`](https://raw.githubusercontent.com/python-pillow/Pillow/12.2.0/src/PIL/Image.py#L1996-L2032).

Canonical DynamicImage match source (2026-11-08): `Mode` and `DynamicImage`
already derive their variants from `for_each_raster_identity!`, while
`dynamic_map!` repeated all 25 variants manually. Rewired that generic raster
dispatch macro to generate its arms from the same canonical list and widened
the list macro's argument capture to one token tree for pattern-bearing
callbacks. This removes a parallel identity list; no behavior change is
intended. Source-only and uncompiled; no formatter, checkstyle, lint, test,
parity, benchmark, or coverage ran.

ImageOps wrapper-ownership source review (2026-11-08): the Python
`pillow_rs.imageops` facade now contains public-call composition, host protocol
adaptation, and metadata updates; geometry, mode admission, palette decisions,
color/LUT calculations, and pixel transforms are owned by `pillow-rs`. The
remaining `deformer.getmesh`, custom numeric/sequence/comparison protocols,
and EXIF/XMP dictionary mutations stay at the host boundary because they
invoke Python behavior. This closes CPU source ownership review for the facade;
the ImageOps row now awaits frozen parity and selection receipts. Masked or
specialized Autocontrast GPU routes remain later GPU work. No validation ran.

Geometry and Pad archive comparison (2026-11-08): Contain, Cover, Fit, and
Scale now use shared core sizing functions and compose a prepared exact-mode
Resize/ResizeBoxed child with the injected context; their error and empty
dimension checks occur before support admission as required by the public
wrapper order. The old SIMD adapter existed for Pad, not for those four
geometry wrappers. Pad's exact-mode architecture owners retain the archived
native-byte fill/copy routes and use the prepared Resize child for resampled
content; architecture requests without a matching specialized path retain
the declared Default child fallback. This closes the remaining source
comparison for geometry placement and dispatch; public outputs, error order,
selection receipts, and performance remain pending. No validation ran.

PutAlpha staged wrapper source review (2026-11-08): the Python facade loads
before mutable-mode preparation. Exact image masks validate destination
support, promote before mask-mode access/load, then call the same typed mask
writer. Subclass and custom scalar values promote before callbacks or C-int
coercion; overflow preserves the promoted destination, while only the
documented AttributeError/ValueError fallback constructs an L image and calls
public PutAlpha again. One masks normalize within the native output loop, with
no intermediate mode conversion. Source-only; callback, error-order, metadata,
sample, and selection-receipt parity remain pending. Reference: [Pillow 12.2
`Image.putalpha`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1880-L1929).

Put/Get and Reduce One source follow-up (2026-11-08): the focused Put/Get TODO
scan found parity/receipt work, Rust-only extension behavior, and later GPU
eligibility work, but no missing CPU kernel in the reviewed entries. Pillow
`Image.reduce` copies a full-image factor-one request and crops factor-one
sub-boxes. Only non-identity One/P requests reach `ImagingReduce`, which rejects
those modes. The CPU One and P owners declare identity-factor constraints;
`reduce_public` crops factor-one sub-boxes and rejects non-identity requests
after factor and box validation.
Factor/box validation order, wider Put/Get source review, parity, receipts,
and performance remain open. No validation ran.

Quantize supplied-palette source follow-up (2026-11-08): wrapper-visible mode
controls Pillow's admission checks, while the exact `DynamicImage` mode
selects the CPU mapper. L/RGB/RGBA/RGBa/RGBX native carriers map their stored
RGB lanes directly to the supplied palette without converting or staging a
second raster. The Python adapter accepts this binding's `PyImage` carrier;
adapting foreign `image-slash-star` ImagingCore values remains TODO. Palette
callbacks/copy behavior, wrapper overrides, errors, samples, strategy receipts,
and performance remain pending frozen parity. Source-only; no build,
formatter, checkstyle, lint, test, parity, benchmark, or coverage ran.

Serial CPU scheduling source audit (2026-11-08): searched the maintained
`pool_cpu` tree for Rayon imports, `par_iter`, and Cargo `parallel` cfgs.
Matches are ownership comments and deferred `pool_parallel` TODOs; no active
CPU code schedules Rayon work. This is a source search only and does not
validate feature builds or runtime selection. No validation ran.

Lazy encoded-header mode source trace (2026-11-08): `Image::open_bytes_with_formats`
constructs `image_slash_star::EncodedImage`, reads its cached `raster_mode`,
and translates that codec identity at `image/codec_adapter.rs`. It stores the
result in `Image::Bytes::header_mode`; `Image::mode_identity()` returns that
identity before its materialization fallback. Opening/querying mode therefore
does not build a `DynamicImage` or decode pixel payloads. This verifies the
current source boundary only; TIFF associated-alpha semantics and remaining
codec transfer mappings stay tracked as codec work. No validation ran.

CPU operation-root structure audit (2026-11-08): `pool_cpu/ops/` contains only
its `mod.rs`; each immediate family has both `mod.rs` and `ops.rs`. The
additional family-level support modules currently present (`imageops` operation
contracts, Eval's scalar leaf, and Thumbnail's mode strategy) are also listed
in the structure guide's family example. This checks only the operation/family
roots; mode/architecture placement and source-size/function limits remain in
the full source audit. No validation ran.

Filter protocol source update (2026-11-08): the Python Image adapter now follows
Pillow's callable construction, public `filter` method protocol,
`MultibandFilter` marker, and per-band split/merge behavior. Built-in classes
implement the same protocol while all pixel operations and merge work remain
Rust-owned; the Rust binding's `filter` method matches Pillow's low-level
kernel convolution call shape. Built-in `filterargs` metadata is exposed,
canonical kernels retain their exact-mode Rust strategy, overridden arguments
use the native convolution entry point, and Kernel construction preserves
Pillow's scale-before-length check order. The generic `_apply` callback path is
removed. `PyImage` now exposes Rust-backed `expand`, `rankfilter`, `modefilter`,
and `color_lut_3d` spellings for Pillow's duck-typed filter callbacks; `expand`
uses edge replication. This checkpoint preceded the later callback-visible
utility and sequence crosswalk. Callback fault order, hidden physical-lane
outcomes, exact operation/architecture receipts, CPU parity, and performance
remain pending. No build, formatter,
checkstyle, lint, test, parity, benchmark, or coverage ran.

Merge lazy-output source correction (2026-11-08): accepted and rejected Merge
routes now declare `ByTargetMode` in `MERGE_SUPPORT`. The lazy planner selects
the support row by the requested result mode, which may differ from the first
band's source mode, then resolves the output through that declared rule. This
removes the standalone target-result override while preserving Merge's
rejected-route lazy mode and palette-source behavior. Output/fault timing and
pipeline parity remain pending; source-only, no validation ran.

Filter caller source crosswalk (2026-11-08): the Python `Image.filter` facade
loads before argument validation, constructs callable filter classes, and
calls the filter object's public `filter` method. Built-in kernel filters use
the named Rust operation when their name and `filterargs` retain the class
defaults; overridden arguments enter `PyImage.filter` and core
`Image::kernel_filter`. Both paths create typed Filter3x3/Filter5x5 pipeline
operations. GaussianBlur, BoxBlur, rank filters, UnsharpMask, and ModeFilter
map through their PyImage methods to lazy CPU pipeline operations or the
prepared eager CPU route; `prepare_pipeline` derives the exact mode from
`OperationInputs`, selects through the injected `ExecutionContext`, and
executes the selected mode owner. Non-multiband custom filters receive split
band handles and are merged through the public mode. The caller/source wiring
crosswalk is complete. Pillow 12.2's bundled filters use the low-level methods
`filter`, `expand`, `rankfilter`, `modefilter`, `copy`, `gaussian_blur`,
`box_blur`, `unsharp_mask`, and `color_lut_3d`. The `PyImage` handle now exposes
these spellings through Rust-backed operations, including edge-replicating
`expand` and cropped `rankfilter`; it does not add a second pixel algorithm.
TODO(pillow-rs#3ptb): freeze callback output/fault cases. The callback-visible
`readonly`, utility, and sequence source crosswalks were closed in later
follow-ups. Hidden physical lanes, exact operation/architecture receipts,
parity, and performance remain open. No build, formatter, checkstyle, lint, test, parity,
benchmark, or coverage ran. Source: [Pillow 12.2 `ImageFilter.py`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageFilter.py#L1434-L1660).

Mode `1` Filter source check (2026-11-08): Pillow 12.2 `ImagingFilter` rejects
float32 and special multiband inputs, then allocates its output in the source
mode and calls the 3x3 or 5x5 byte convolution. It has no bilevel threshold or
conversion step for mode `1`; the exact-mode One strategy therefore keeps the
native byte kernel and same-mode result, including noncanonical byte samples.
Source contract only; output, faults, architecture/fallback receipts, and
performance remain pending CPU parity, including explicit noncanonical One
samples. Reference: [Pillow 12.2 `Filter.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Filter.c#L1986-L2037).

Paste clipping source correction (2026-11-08): aligned prepared and direct
Paste admission with Pillow 12.2 `ImagingPaste` and `ImagingFill2`: source/mask
sizes and image-source physical pixel size are checked before clipping; an
empty destination intersection returns before mask-mode validation. Solid
color coercion remains before clipping. Source-only; add empty-intersection
invalid-mask and nonempty-invalid-mask parity cases before freezing the CPU
denominator. Output/fault order, architecture receipts, and performance remain
unverified. No build, formatter, checkstyle, lint, test, parity, benchmark, or
coverage ran. References: [Pillow 12.2 `Paste.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Paste.c#L256-L329),
[masked fill](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Paste.c#L561-L629), and
[`_imaging.c` dispatch](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L1369-L1400).

I;16 Paste byte-order correction (2026-11-08): Paste now treats `I;16L` as
little-endian on every target, matching the existing byte-construction and
data-access mode mappings; `I;16B` stays big-endian and `I;16`/`I;16N` follow
native order. The previous helper misclassified `I;16L` on big-endian hosts.
The source inconsistency is fixed; cross-alias, masked, clipped, big-endian,
receipt, and performance parity remain pending. No build, formatter,
checkstyle, lint, test, parity, benchmark, or coverage ran.

Filter callback method source follow-up (2026-11-08): the Rust-backed Image
handle now exposes Pillow's `modefilter` spelling and the exact
`color_lut_3d(mode, filter, channels, size, table)` adapter shape. The latter
parses the requested target into core `Mode`, requires Pillow's BILINEAR code,
and delegates table preparation and execution to the existing typed Rust
Color3DLUT operation; the Python built-in now invokes this same adapter. The
public Color3DLUT constructor still admits only three/four output channels.
The private ImagingCore adapter now preserves its separate validation order:
it resolves mode, checks BILINEAR, accepts channel counts 1–4 and dimensions
2–65, then converts the table argument. A correctly sized one/two-channel
table reaches the same `table_channels could be 3 or 4` ValueError as Pillow;
malformed table lengths fail first. The Python table argument is retained as a
Python object until after those checks. Source comparison only; parity remains
pending for table buffers, extraction faults, source/target mode combinations,
outputs, metadata, and selection receipts. `PyImage.expand`
now uses a native typed edge-replication strategy for every Pillow mode, while
the five internal high-depth modes are explicit `NO_PILLOW_SUPPORT`
extensions. `PyImage.rankfilter` returns the cropped native result; P uses a
dedicated `ImagingCorePaletteRankFilter` operation support and exact-mode
owner, reusing the native byte order-statistic kernel and preserving palette
metadata. Public `ImageFilter.RankFilter` retains its existing same-size,
mode-restricted route. TODO(pillow-rs#3ptb): verify callback output,
metadata, fault order, empty/negative expand cases, operation/fallback
receipts, and full-call latency. Finish the Color3DLUT one/two-channel fault
cases and verify callback-visible ImagingCore utilities in parity. The
Color3DLUT one/two-channel validation order and contiguous float-buffer input
path are source-implemented; runtime fault/output parity remains pending.
Source-only; no build, formatting,
checkstyle, lint, test, parity, benchmark, or coverage ran.

Filter callback sequence and readonly source closure (2026-11-08): `PyImage`
now exposes Pillow ImagingCore's flattened pixel count and row-major indexing,
including negative indices and pixel formatting, plus `readonly == 0` and the
`getpixel` spelling. The Python constructors copy buffer/array data into
Rust-owned storage, and `Image.filter` loads the source before invoking its
callback. Pillow's raw C `ptr` capsule remains outside the behavior contract.
Other callback-visible utility spellings and behavior, especially mutation,
remain source-audit work, including `putpixel`, `putdata`, `histogram`,
`entropy`, `getcolors`, `getextrema`, and band access; output, error, and
reentrant mutation behavior remain unverified. No build, formatter,
checkstyle, lint, test, parity, benchmark, or
coverage ran. References: [Pillow 12.2 Image.filter](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1330-L1353) and [ImagingCore attributes and sequence slots](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L3563-L3647).
References: [Pillow 12.2
ModeFilter](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageFilter.py#L1626-L1628), [Color3DLUT](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageFilter.py#L2318-L2333), and [the native parser](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L3782-L3839).

Filter callback utility source follow-up (2026-11-08, updated 2026-11-09): `PyImage` now exposes
the ImagingCore spellings `putpixel`, `putdata`, `histogram`, `entropy`,
`getcolors`, `getextrema`, and `getband`, backed by existing mode-native Rust
pixel, data, analysis, and channel operations. Direct ImagingCore `putpixel`
skips the public P/PA palette-color allocation path. Pillow's C callback
`putband`, `fillband`, and `setmode` now have source-reviewed native CPU strategies; output, fault, and fallback receipt parity remain unverified. PixelAccess mapping source review is recorded in the later source follow-up; output and fault parity remain pending. No build, formatter, checkstyle, lint, test,
parity, benchmark, or coverage ran. The added spellings and mutation outcomes
remain uncompiled and unverified. References: [Pillow 12.2 ImagingCore
methods](https://raw.githubusercontent.com/python-pillow/Pillow/12.2.0/src/_imaging.c#L3478-L3528), [pixel writes](https://raw.githubusercontent.com/python-pillow/Pillow/12.2.0/src/_imaging.c#L1773-L1805), and [band access/mutation](https://raw.githubusercontent.com/python-pillow/Pillow/12.2.0/src/_imaging.c#L2261-L2299).

Filter callback analysis argument correction (2026-11-08): the private
`histogram(extrema=None, mask=None)` and `entropy(extrema=None, mask=None)`
adapters now require a callback `PyImage` for a supplied mask and convert its
native Rust image directly. For `I`/`F`, the extrema tuple is coerced after
mask-type validation even when a valid mask makes the range irrelevant to the
scan, matching Pillow's observable argument faults. The Rust core then uses
its existing exact-mode histogram/entropy implementation. Source-only; no
build, formatting, checkstyle, lint, test, parity, benchmark, or coverage ran.
Reference: [Pillow 12.2 `_histogram`/`_entropy` parsing](https://raw.githubusercontent.com/python-pillow/Pillow/12.2.0/src/_imaging.c#L1219-L1348).

Multiband Filter callback-order correction (2026-11-08): the wrapper now gets
each band immediately before invoking its callback, wraps that callback's
result immediately, and merges using the first wrapped band's metadata state.
This preserves Pillow's behavior when a callback mutates the source between
band calls, returns an invalid result, or changes metadata. Pillow 12.2
`Image.filter` reads `self.im.getband(c)` inside the comprehension, wraps each
result with `self._new`, and passes those images to `merge`; `merge` creates
the output with `bands[0]._new`. Source-only; reentrant output/fault/metadata
parity and callback selection receipts remain pending. No build, formatter,
checkstyle, lint, test, parity, benchmark, or coverage ran. References:
[Pillow 12.2 `Image.filter`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1443-L1468),
[module-level `merge`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L3788-L3812).

Unit-test removal inventory (2026-11-08): a maintained-source scan finds no
Rust `#[test]` attributes outside `pool_cpu/legacy`. The five maintained
`scripts/test_*.py` / `scripts/*_test.py` files found by the filename scan are
input-driven Pillow parity diagnostics, not ordinary unit tests, and remain in
place under the user's parity-only test policy. This is a source inventory,
not a test run; no validation command ran.

Merge orchestration consolidation (2026-11-09): multiband `Image.filter` now
delegates its already wrapped callback results to the same module-level merge
adapter used by `Image.merge`. The adapter wraps the merged core image using
the first filtered band's metadata state, retaining Pillow's callback timing
while removing a duplicate Python orchestration path. This does not change
merge validation or eager-load behavior; those remain part of the pending
callback contract review. Source-only; no validation ran.

Merge first-band mode correction (2026-11-09): Pillow checks `mode` for bands
after the first, then its native byte merge accepts any one-band byte carrier
as band zero. The active `MergeBandModeRule` now admits `One`, `L`, and `P` in
that first position while requiring `L` for following bands. Default and
architecture guards use the same set, so a `One` first band can reach an
eligible interleave strategy. Single-band output contracts remain exact-mode
because Pillow's copy path rejects mismatched source/output modes. Review of
other first-band storage types and frozen output/fault parity remain TODO under
`pillow-rs#3ptb`. Source-only; no validation ran. References: [Pillow 12.2
`Image.merge`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L3788-L3812),
[`ImagingMerge`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Bands.c#L231-L299),
and [single-band `ImagingCopy2`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Copy.c#L46-L53).

Default RGB Merge source follow-up (2026-11-09): the chunked byte-band
interleave now accepts `One`, `L`, or `P` as its first band when the remaining
bands are `L`. This keeps the added same-byte modes on the specialized Default
path without conversion; output and latency still require the frozen CPU gates.
Source-only; no validation ran.

Merge callback-order source follow-up (2026-11-09): the Python adapter now gets
arity, later-band mode, and rejection text from the typed core Merge contract;
it checks `len`, then each later band's `mode` and `size`, calls `load` on all
bands, and delegates raster work back to the core. This replaces the earlier
pending wrapper-validation/eager-load item, but callback, fault, and output
parity remain unverified. No validation ran.

PixelAccess source follow-up (2026-11-09): `Image.load()` now gives its pixel
view the exact Rust image handle it loaded. Reads and writes use dedicated
ImagingCore adapters: tuple-only coordinate parsing, low-level color handling,
and bounds behavior, without the public `Image.putpixel` palette-color path.
Deletion validates the key and otherwise follows Pillow's no-op behavior;
Draw floodfill receives the captured handle. Source review is complete for the
defined coordinate and callback contracts. Output, mutation, identity, fault,
and parity evidence remain pending. Float coordinates truncate toward zero
within i32; infinities and finite overflow saturate at the i32 bounds, and
NaN maps to zero. This deterministic Rust-adapter behavior covers inputs where
Pillow's C cast has no portable result; freeze those edge outcomes in CPU
parity. Source-only; no validation ran.

ImagingCore band-mutation source follow-up (2026-11-09): `putband` and
`fillband` now have exact-mode CPU support declarations, Default execution, and
core `Image` entry points. The implementation validates a one-band source and
band index in Pillow order, copies bytes only for compatible layouts, clips
fill values to `u8`, and preserves hidden physical lanes while editing LA/RGB
compact carriers. Python exposes these adapters to filter callbacks. Their
callback output, error, and fallback-receipt parity remains pending. The
`setmode` strategy preserves four-byte storage across RGB, RGBA, and RGBX
reinterpretations; its output, fault, fallback-receipt, and padding-lane parity
remain pending. PixelAccess has dedicated mapping adapters; its output,
mutation, callback, fault, and view-identity parity remain pending, with
Float coordinates truncate toward zero within i32; infinities and finite overflow saturate to the i32 bounds, and NaN maps to zero. These inputs are defined by this Rust adapter because Pillow's C cast has no portable result; freeze the extension in CPU parity. No build, formatter, checkstyle, lint, test, parity, benchmark,
or coverage ran.

Typed font mode boundary follow-up (2026-11-09): the JavaScript option adapter
now translates its external mode string to `raster::Mode`, matching Python and
the typed Rust font API. Empty strings retain the default-layout path, and
unknown nonempty names retain non-bilevel, non-color behavior. This closes the
remaining core font-option string caller found by source audit. Output,
fault-order, and route parity remain pending; no validation ran.

x86 architecture compile follow-up (2026-11-09): `cargo check -p pillow-rs
--target x86_64-apple-darwin --features test-api` now passes. The cross-target
check exposed filter module-path errors, architecture-only unused items, and
missing narrow unsafe-lint annotations around already-guarded SSSE3/AVX2 call
boundaries; these were corrected without changing the workspace-level unsafe
deny. The native AArch64 `cargo check -p pillow-rs --features test-api` also
passes after the corrections. This proves both target builds compile, not x86
runtime parity or execution receipts; x86 execution and the full CPU parity,
receipt, and performance gates remain pending.

Mode One explicit full-box Reduce parity (2026-11-09): added the ordinary
public-input case `one-identity-factor-explicit-full-box` to distinguish an
explicit full-image box from the factor-one default-box and sub-box routes.
Input regeneration and schema validation pass. The case passes against Pillow
12.2 under both Default and requested Neon CPU architectures (1/1 each). Its
Reduce path is a copy and records no Reduce kernel; Neon telemetry in the case
comes from setup's `PutPixel`, not the Reduce call. No averaging or threshold
work is needed. A second One case combines a non-identity factor and negative
box offset; Pillow and Rust both report `ValueError: box offset can't be
negative` before the unsupported-mode result under Default and Neon (1/1 each).
The One-specific output and tested error-order contracts are covered; the full
CPU performance gate remains pending.

Refreshed CPU architecture parity matrix (2026-11-09): from the designated
`/Users/lazytrot/work/pillow-rs` checkout on `develop` at dirty revision
`739a07403af989b6103cb320a2521bd595ad35f0`,
`env -u MIGRATION_PARITY_CASE_IDS -u MIGRATION_PARITY_CASE_ID make
migration-parity-test-cpu-architectures` built the release test API and ran the
refreshed 17,594-case CPU selection against Pillow 12.2.0 under Default, Neon,
SSE2, SSE3, SSSE3, AVX, AVX2, AVX+FMA, and AVX2+FMA. Every lane executed 17,594,
passed 17,593, and reported one parity failure with zero infrastructure
errors. Focused one-case reruns under all nine selectors confirm that failure
is `PIL.Image.Image.save.nuanced.i-png`: Pillow completes the save and the
target reports `OSError: unsupported Png: encode: PNG cannot encode this image
mode`. This is the existing image-slash-star encoder issue
`image-slash-star#9h5w`; do not add encoding logic to pillow-rs. The full
Default report is `migration-parity-3be892f9951d486081ad5f1c6c218171`; the
focused Default report is `migration-parity-12e20238a20c45c48814c5015122d3b6`.

The initial full-matrix execution sidecars classified four cases as
indeterminate and one as partial. The classifier was corrected after tracing
the cases: the two clipped solid RGB Pastes return before queuing pixel work;
the two EXIF workflows copy images with no orientation; and terminal
observations now compare operation names with the string-valued observation
set. A focused replay of these five IDs under all nine selectors passes 5/5
per lane with zero infrastructure errors. The two Paste cases and two
EXIF-transpose cases now classify as `not_applicable`; the DrawPoint case has
a terminal-complete receipt. The full aggregate sidecar was not rerun after
this classifier-only change, so these focused sidecars supersede its five old
classifications; regenerate the aggregate at the final frozen CPU gate.

The x86 selectors ran on an ARM host and recorded Default fallback after
`UnavailableHardware`; this does not verify native x86 SIMD execution. Five
focused One Reduce cases pass under Default and requested Neon; the refreshed
full campaign includes them and shows no One-specific parity failure. The
identity path is copy/crop and dispatches no Reduce kernel, so averaging or
thresholding would add work without changing Pillow-visible output. No
dedicated One latency measurement was made. The whole CPU benchmark gate,
final aggregate receipt refresh, native x86 runtime evidence, and codec-owned
parity case remain pending; no coverage was run.

Complete CPU matrix and Rust coverage refresh (2026-11-09): on clean
`develop` commit `ba4fc8e9c4832ad0c360eeb30a88ed89a926451a`, the complete
17,616-case selection ran under Default, Neon, SSE2, SSE3, SSSE3, AVX, AVX2,
AVX+FMA, and AVX2+FMA. Every selector executed 17,616 cases: 17,595 of 17,596
Pillow parity cases passed, all twenty recorded target-output/error cases
passed, and the sole failure was
`PIL.Image.Image.save.nuanced.i-png`. Pillow completed the save; the target
returned `OSError: unsupported Png: encode: PNG cannot encode this image
mode`. This is the image-slash-star encoder gap `image-slash-star#9h5w`, outside
the Rust image-operation scope. There were zero infrastructure errors. The
selection ID SHA-256 is
`e4e8ade188ad7965f44a70b36f438b61a6a2d54188771c15e43373710b85b0f6`; reports
and execution sidecars are under
`build/migration-parity/cpu-architecture/final-2773be01b/`. x86 selectors
recorded `UnavailableHardware` and used Default on this arm64 host; native x86
kernels were not exercised. The manifest selected zero CPU fault contracts,
so ordinary CPU error/fault-order cases still need coverage.

The matching Rust coverage run completed 27/27 plans, 17,377 cases passed,
zero failed, and zero infrastructure errors. Its LLVM report covers 70,624 of
182,663 Rust lines overall (38.7%); the active `pool_cpu` entries cover 41,078
of 105,284 lines (39.0%) and 3,641 of 10,623 branches (34.3%). Several large
vector/helper files show zero hits in this run, including resize's bounded
native-byte vector helper, Enhance sharpness, Chops, Draw SIMD shapes, Paste,
Convert native RGB, and Filter rank/blur kernels. These are candidates for
mode/architecture parity review; zero coverage alone does not distinguish an
uncovered supported route from compile-time or hardware unavailability. No
operation × Mode × Architecture matrix is claimed complete. The benchmark,
native x86 runtime, and public fault-contract gates remain open.

Brightness mode matrix checkpoint (2026-11-09): all 25 `raster::Mode` identities now have explicit Brightness outcomes through the public Python facade: ten Pillow parity modes, three Pillow rejection modes (`1`, `P`, `PA`), and twelve recorded target-output modes (LAB plus eleven CoreOnly high-depth/internal modes). The target-only expectations cover factors 0, 1, and 2 using native sample encodings; LAB is explicitly marked `NO_PILLOW_SUPPORT`. The 25 workflows pass under Default and each of nine architecture selectors (225/225); added La, RGBa, and RGBX non-identity cases pass 3/3 Default and 27/27 across selectors. Default focused Rust coverage passes 292/292, including full line/branch coverage for all 22 mode-specific Brightness owners. Receipts select Default directly, report `missing_specialization` for Neon, and `unavailable_hardware` for x86 selectors on arm64. Native architecture Brightness execution is not established by these fallback runs. The Neon coverage attempt is unavailable through the current coverage instrumentation build because it omits the test-api architecture setter. Remaining CPU acceptance is unchanged: operation × Mode × Architecture evidence, native x86 runtime, full CPU parity, ordinary error/fault cases, external I32-to-PNG codec completion, and the benchmark comparison.


Chops Invert matrix checkpoint (2026-11-09): explicit requirements now cover
all 25 Mode identities. Fourteen Pillow byte modes pass public Python parity;
eleven CoreOnly numeric/high-depth modes pass recorded raw-byte expected
outputs. The 38-workflow selection passes under Default and all nine CPU
architecture selectors (342/342). Receipts show Default direct selection,
Neon `missing_specialization`, and x86 `unavailable_hardware` on arm64; Invert
has no declared architecture specialization. Focused Rust coverage passes
61/61. This does not prove SIMD execution or complete the broader Chops family
matrix. Binary Chops, Offset/fusion paths, native x86 runtime, complete CPU
parity, ordinary error/fault cases, the external I32-to-PNG gap, and performance
remain pending.

Offset mode-matrix WIP (2026-11-09; pre-resolution diagnostic): the expanded
selection had 61 cases: all 20 Pillow modes, nine narrow-row cases, eight empty
dimension cases for the four Pillow 16-bit modes, and five CoreOnly expected
outputs. The initial 53-case matrix passed 49 cases under each of the nine
selectors, including all five CoreOnly expectations; the same four mismatches
were the large `I;16`, `I;16L`, `I;16B`, and `I;16N` cases. The eight empty
dimension cases passed under all selectors (72/72). These byte-tail outputs,
SIMD block counts, and scalar-tail byte counts describe the retired
byte-oriented kernel, not the typed-u16 implementation now in the working
tree. Five isolated CPython 3.12 /
Pillow 12.2 runs of the same `I;16` input produced five different output-tail
hashes. Pillow leaves the second half of each 16-bit storage row unwritten; its
stable first-width-byte data matches Rust, whose output deterministically
zero-fills that tail. The Neon receipt executed the native I;16 kernel (17
vector blocks and 255 scalar-tail bytes per 31×17 case), and each 3×3 narrow
I;16 alias used two vector blocks and two scalar-tail bytes. X86 selectors fell
back as unavailable hardware on arm64. After the recorded-output decision,
the five CoreOnly typed output cases were refreshed under all nine selectors
and pass 45/45. Neon reports `missing_specialization` for all five and runs
Default; no native high-depth Offset architecture kernel is claimed. For empty
16-bit inputs, Neon selected
its strategy and returned `native-copy` with zero vector blocks; Default ran
directly. A Rust operation-coverage run previously
executed 172/172 cases with zero failures, but its build did not expose the
architecture selector, so its SIMD helper files remain uninstrumented.
Offset I;16 contract decision (2026-11-09): Pillow's public Offset contract
wraps pixels. Pillow 12.2's implementation treats its two-byte I;16* rows as a
one-byte image in the shared loop, which visits only `width` bytes and leaves
the rest unwritten. Pillow 13.0's release notes explicitly record this as a
bug fix, and its implementation copies complete pixels. Rust therefore wraps
typed `u16` samples. The four Pillow modes keep mode-specific expected-output
contracts while the repository's Pillow 12.2 oracle remains pinned; this is
not a `NO_PILLOW_SUPPORT` classification. The Default path now rotates typed
pixels, and the SIMD path rotates `u16` vectors. Fixture generation now records
four shapes for each mode (odd-width wide, narrow, zero-width, and zero-height)
plus five existing CoreOnly outputs: 21 target-output cases total. No Pillow
12.2 parity cases remain for these four modes. The regenerated fixture snapshot
has 25,003 cases overall; its ImageChops input has 156 Offset Pillow-parity
cases plus 21 Offset expected outputs. No parity run has checked the new
source and fixtures.
Manual source review of the typed-sample kernel and expected-output generator
is complete; compiler and runtime evidence are still absent. The Default and
SIMD owners preserve whole `u16` samples. The row-wise SIMD guard requires
`width >= 8`; the 3×3 narrow expected-output case keeps total storage above
16 bytes so it also covers the narrow-row fallback. Source inspection of the locked `wide` 1.7.1
implementation confirms `u16x8` maps to SSE2, Wasm SIMD128, or NEON when the
corresponding target feature is enabled; the strategy declarations use the
same target-feature gates. This establishes the source-level mapping, not a
runtime selection receipt or parity result.
TODO(pillow-rs#3ptb): finish review of all current CPU WIP before any gates,
then run the static/compile gates and frozen Offset output/error/receipt cases
across Default and every declared architecture. Keep the Pillow 12.2 mismatch
as a version-pinned historical bug; do not restore unstable unwritten-byte
parity. Pillow 13.0 fixes this behavior upstream.

Chops three-band raw-byte correction (2026-11-09): the Pillow 12.2 Python
wrappers pass the second image to the native operation without requiring equal
mode names. `Chops.c` admits equal band counts and evaluates each stored byte;
its output keeps the primary image mode. The Rust support declaration already
groups RGB, YCbCr, LAB, and HSV as compatible three-band inputs. The Default
strategy accounts for LAB's biased chroma representation when LAB is the
secondary image. Current three-band vector kernels do not, so architecture
guards keep those accepted pairs on Default until the vector implementation
handles LAB natively. This changes implementation selection, not the public
mode-pair contract. Hidden physical lanes remain computed by the outer
prepared plan. Focused architecture verification now passes; see the
2026-11-10 Chops fallback checkpoint below.

Rust-only successful-mode verification decision (2026-11-09): when Pillow
cannot produce a declared Rust `Mode`, verify the public Python-to-Rust path
with a fixed `expected-output` contract and the normal execution receipt. Keep
the wrapper as input conversion and delegation; do not derive expected output
in Python or substitute a Rust unit test. This is the contract for the five
CoreOnly high-depth modes and any future successful `NO_PILLOW_SUPPORT` mode.

Binary Chops mode-matrix checkpoint (2026-11-09): all twelve byte-binary
operations declare and exercise all 25 `raster::Mode` identities: 14 Pillow
byte modes, six Pillow numeric modes rejected by the operation, and five
internal high-depth modes rejected by the operation. LAB was initially
misclassified as a Rust-only extension. A Pillow 12.2 oracle accepts LAB and
same-sized cross-mode three-byte pairs for all twelve operations, so LAB now
uses the ordinary CPU support declaration and a native mode-specific byte
mapping. Each operation has 32 live Pillow workflows: 20 same-mode cases and
the twelve ordered mixed-mode pairs among RGB, YCbCr, LAB, and HSV. Five
target-only expected-error contracts cover the internal high-depth modes.
Across Default, Neon, SSE2, SSE3, SSSE3, AVX, AVX2, AVX+FMA, and AVX2+FMA, all
3,996 selected outcomes pass with zero infrastructure errors. Execution
receipts show RGB with LAB as its secondary input selects Default under Neon
(`ineligible_request`); LAB has no declared architecture specialization and
uses Default. The seven x86 selectors report
`unavailable_hardware` on this arm64 host, so native x86 execution remains
unverified. `La` has no archived architecture-specific Chops strategy, so its
Neon fallback is declared; assess a native strategy later against the CPU
performance target. Remaining: cover same-pixel-size pairs among `1`/`L`/`P`
and `LA`/`La`/`PA`, dimension mismatches, parameter variants, and fused paths
for the byte-binary family. Logical Chops, Constant, Offset, Duplicate, Invert,
native x86 execution, full CPU parity, fault contracts, and the benchmark gate
remain pending. The frozen selected case IDs have SHA-256
`1b79ce61fae476ef7aad6e93a62d5fde4feb2acbc180e476dcfcba5db3b08dfa`.
Reports and execution receipts are under
`build/migration-parity/cpu-architecture/chops-binary-mode-matrix-cross-lab-20261109/`.

Binary Chops exhaustive pair-matrix expansion (2026-11-10): the source builder
now declares every ordered pair from the 20 Pillow mode spellings (14 accepted
byte modes and six rejected numeric modes) for each of the twelve operations.
It adds all three unequal-size layouts for every compatible ordered pair among
the accepted modes, retains Add/Subtract parameter variants, and records each
of the five CoreOnly modes in both operand positions for every binary
operation. This closes the builder-side mode-pair and secondary-input gaps
identified in the prior checkpoint. Fixture generation now includes this
matrix: the corpus contains 25,003 total cases, with 24,762 Pillow-parity cases,
240 expected outputs, and one GPU-only fault contract. The ImageChops input
contains 10,070 cases, including 192 expected outputs. These generated IDs are
not yet verified by determinism/manifest checks or runtime parity. The updated
guard keeps LAB-secondary three-band pairs on Default until the vector kernels
implement the same chroma mapping. The pre-guard full CPU run found 36 LAB
cross-mode mismatches under Neon, in addition to the existing I32-to-PNG codec
failure. After the guard and release rebuild, the 36 frozen case IDs pass on
all nine architecture selectors (324/324), with zero infrastructure errors.
Neon execution receipts show Default selected with `ineligible_request` for all
36 cases. Reports and receipts are under
`build/migration-parity/cpu-architecture/chops-lab-fallback-final/`. These
focused results do not replace the pending full CPU parity gate. No coverage
ran.

Binary Chops pair-relation checkpoint (2026-11-09): the input builder now
enumerates every distinct ordered pair among the fourteen Pillow byte modes
for all twelve operations. Same-band pairs are active; cross-band pairs are
expected to reject with `images do not match`. This adds the `1`/`L`/`P`,
`LA`/`La`/`PA`, and four-band relations alongside the existing three-band
relations. For each operation, it adds three unequal-dimension layouts for
every primary mode to cover width/height clipping, row strides, and
architecture fallback. Add and Subtract each receive two non-default
scale/offset cases per mode. Generation produced 20,993 parity cases overall;
the focused Chops selection contains 3,045 cases per CPU selector (27,405
outcomes across nine selectors), including the pre-existing Add L/RGB mismatch
case and 60 target-only rejection contracts. Its sorted case-ID SHA-256 is
`5e26cc5872e174590f8ff4ea87c02f6dca5e8b4ea3f3ddef58e86234feed8485`.
`make migration-parity-fixtures` regenerated the inputs, and
`make migration-parity-inputs-check` passed deterministic input and manifest
contract checks. All nine CPU selectors passed all 3,045 cases (27,405/27,405
outcomes) with zero infrastructure errors. Neon receipts show 504 native
architecture selections and 656 Default selections across 1,160 applicable
operation receipts; 492 fall back for `ineligible_request` and 164 for
`missing_specialization`. Each x86 selector records 1,160
`unavailable_hardware` fallbacks on this arm64 host, so native x86 execution
remains unverified. No coverage was run. The prior 444-case result applies
only to its separate recorded case-ID hash. Pillow 12.2's public Chops wrappers
pass both images to the operation; equal band counts and clipping each extent
to the smaller input inform the matrix, with observed outputs remaining the
acceptance criterion. Sources:
[ImageChops.py](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageChops.py),
[Chops.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Chops.c).
Parity and execution receipts are under
`build/migration-parity/cpu-architecture/<architecture>.parity.json.gz` and
`<architecture>.execution.json.gz`.
Fused Multiply/Screen parity, other Chops families, native x86 execution, full
CPU fault and parity matrices, and the benchmark gate remain pending. Logical
Chops mode coverage is recorded below.

Logical Chops mode-matrix checkpoint (2026-11-09): the manifest now exposes
all 20 Pillow mode spellings for `logical_and`, `logical_or`, and `logical_xor`;
`1` is accepted and the other 19 are checked as invalid in both operand
positions. The five internal high-depth modes receive recorded target error
contracts for both positions, run through the public Python facade. The
wrapper only constructs typed inputs and delegates; expected outcomes are
fixed in the contract generator. Fixture generation produced 21,191 parity
cases, and `make migration-parity-inputs-check` passed deterministic input and
manifest validation. The frozen focused selection is 147 cases per selector:
117 Pillow comparisons and 30 target-only error expectations. Its sorted
case-ID SHA-256 is
`bab72cc3b20845eff3c131cb83e39587bd3ec156a7df1246381634f6fa4b0c59`. All nine
serial CPU selectors passed (1,323/1,323 outcomes) with zero infrastructure
errors. Default selected Default for the three accepted `One` workflows;
Neon selected its native vector strategy; the seven x86 selectors fell back
to Default with `unavailable_hardware` on this arm64 host. Rejected-mode
inputs exit during support validation and therefore have no execution-strategy
receipt. No coverage was run. Artifacts are under
`build/migration-parity/cpu-architecture/chops-logical-mode-matrix-20261109/`.

Duplicate mode-matrix preparation (2026-11-09): source review confirms that
`ChopsDuplicate` declares mode-preserving support for all 25 `raster::Mode`
identities and routes through each exact mode owner before the typed clone.
There is no architecture specialization; requested SIMD architectures resolve
to the serial Default strategy. The manifest and input generator now include
all 20 Pillow mode spellings plus five `NO_PILLOW_SUPPORT` internal-mode output
contracts. Fixture generation produced 20 new Pillow mode cases and five
recorded target outputs. The generated ImageChops input currently contains 55
Duplicate cases total. This is fixture preparation only: no parity or coverage
campaign has run for this matrix, and no execution-receipt result is claimed.

Offset I;16 diagnosis (2026-11-09): five isolated Pillow 12.2 runs produced
different unwritten tails for the same input. Pillow 12.2's `image8` alias
indexes bytes, but its loop bound is pixels. Pillow 13.0 release notes confirm
that Offset was fixed for 16-bit images. See the contract-decision checkpoint
above and TODO(pillow-rs#3ptb) for remaining verification.

Constant mode-matrix preparation (2026-11-09): the existing exact-mode
declarations accept each of the 20 Pillow source modes and all five internal
source modes, with a fixed `L` output. The manifest and input generator now
include 20 mode-specific Pillow comparisons and five recorded CoreOnly output
contracts. Generation confirms 54 total Constant inputs, including the 20 new
mode cases and five target-only outputs. No parity or coverage campaign has
run for Constant; output and architecture receipts remain unverified.

LAB Chops regression checkpoint (2026-11-10): Pillow 12.2 exposes LAB `a`/`b`
samples with a +128 byte bias, while its native Chops operations work on the
signed-byte storage. The LAB Default strategy now maps those two channels into
and out of that representation around the native byte operation; three-band
mixed-mode pairs decode a LAB secondary operand as well. A focused run of the
324 LAB Chops cases that failed in the earlier full CPU snapshot now passes
against the pinned Pillow 12.2 oracle, including all recorded operations,
mixed operands, and dimension variants. Build and focused parity passed; no
unit tests ran. This is affected-case evidence only: the earlier 25,002-case
full CPU snapshot still records 334 failures and has not been rerun after this
fix. At this checkpoint, nine `I;16` Offset cases still used the 12.2 oracle;
the following checkpoint records their resolution. The existing I32-to-PNG
encoder gap is tracked under `image-slash-star#9h5w`. Native x86 execution and
the final full CPU gate remain pending.

Historical Pillow 12.2 I;16 Offset expected-output checkpoint (2026-11-10): Pillow 12.2's byte-indexed
loop leaves half of each 16-bit output row unwritten; Pillow 13.0 fixes the
operation. The typed-u16 wrapping contract remains in force. The twelve
generated `simd-cov` cases for I;16L/I;16B/I;16N now use recorded target outputs
and retain their case IDs and coverage selection. The omitted-yoffset case
uses the public default of `xoffset`. Fixture generation, deterministic input
checks, and manifest validation pass. Across nine CPU architecture selectors,
all twelve target contracts pass on each selector (108/108). Run IDs are
`migration-parity-9afe0efb5fe44f8c8038b05ecf8af1b8` (Default),
`migration-parity-f2e00de77b4b4c90b85a017c9fc70a85` (Neon),
`migration-parity-744cb70cf6b24e60916f473501696901` (SSE2),
`migration-parity-73146a13265b4c5a80d1c17f649e29ff` (SSE3),
`migration-parity-8ee601e5ec32412d8000415d00cbc0f9` (SSSE3),
`migration-parity-1a380164f0064db09570fed5810dac79` (AVX),
`migration-parity-a23ed168e290414d8a198aecf4c787fd` (AVX2),
`migration-parity-8c5d1aa59f1f41609dc5dbb9113b6d49` (AVX+FMA), and
`migration-parity-bc900c18da3b4570bf1124cef0bd6e8a` (AVX2+FMA). Reports are
under `build/migration-parity/cpu-architecture/i16-offset-target-final/`.
This interim target-output contract was removed when the active oracle moved
to Pillow 13.0, which fixes I;16 Offset. The full 12.2 snapshot still includes
the single I32-to-PNG encoder failure tracked by
`image-slash-star#9h5w`, and native x86 execution remains unproven.

Thumbnail vertical-first SIMD correction (2026-11-10): Pillow 13.0 parity
initially exposed three Neon differences in tall, reducing-gap L/RGB
thumbnails. The straight-alpha `LA`/`RGBA` path now passes `reducing_gap` to
the prepared ResizeBoxed child, matching Pillow's resize route without a
separate Reduce stage. The shared native-byte vector executor now transposes
vertical-first requests, reuses the selected architecture kernel in Pillow's
pass order, and restores the output layout; sparse row facts are recomputed
from the transposed pixels when supplied. The frozen 188-case Thumbnail
selection passed under all nine CPU architecture selectors (1,692/1,692,
zero failures and infrastructure errors). Neon passed all cases; this ARM64
host cannot establish native x86 execution. Selected case-ID SHA-256:
`6c56b6059ab0f2c65367b1ac1ef16f6d3899794ac7bfea9ae3fc52800c946e6f`;
manifest SHA-256: `6143e61a3ca1d6b31c430ed43c6dd04f599a0a0447bf3879e7bbdfa916846a8c`.
The direct three-case Neon rerun and the full final matrix artifacts are under
`build/migration-parity/pillow13-thumbnail-neon-vertical-first.*` and
`build/migration-parity/pillow13-thumbnail-cpu-architectures-checked-channels/`.
No coverage or benchmark ran. Full CPU operation/mode/architecture parity and
the CPU performance gate remain pending.

PhotoYCC rawmode correction (2026-11-10): Pillow 13 uses separate PhotoYCC
lookup tables for `YCC;P` and unpremultiplies the color bytes before decoding
`YCCA;P`; the previous Rust path incorrectly reused the YCbCr transform and
skipped YCCA unpremultiplication. Core now uses a Rust fixed-table PhotoYCC
transform for both inputs, and the parity corpus contains dedicated zero,
partial, and opaque alpha inputs for YCCA plus varied YCC inputs. The input
generator expands both rawmodes across 89 size/pattern cases each, with a
matching benchmark workflow for every expanded case. The generated indexed
corpus now has 97,320 parity cases and 43,366 benchmark workloads. No parity or
benchmark execution has run for this correction. The `L;1` target-output
contract is generated as a `NO_PILLOW_SUPPORT` extension for every declared
size/pattern point. TODO(pillow-rs#3ptb): verify these cases across the CPU
architecture matrix, finish the remaining mode/rawmode transform coverage,
and add the selected GPU architecture/device attribution axis.

Target-only matrix input indexing correction (2026-11-11): size-specific
expected-output splitting now applies only to `ImageChops`, which owns the
large recorded matrix outputs. Other surfaces retain their expected-output
matrix cases in their normal matrix document. This restores the 18 large
`L;1` rawmode cases that the previous splitter removed without indexing; all
87 generated `L;1` size/pattern cases are now present in the indexed corpus.
The input corpus was regenerated, but no parity or benchmark execution ran.

Historical 31-size snapshot, superseded by the current indexed input snapshot
above (2026-11-11): the shared matrix used 31
sizes: 1×1, 1×17, 17×1, 2×2, 3×5, 5×7, 8×8, 15×17, 16×16, 17×19, 31×33,
32×32, 33×25, 48×33, 63×65, 64×64, 65×47, 127×129, 128×128, 129×65,
255×257, 256×192, 320×240, 512×384, 640×480, 1024×768, 1280×720, 1600×900,
1920×1080, 1080×1920, and 1921×1080.
Eligible literal-source workflows are resized down or up, covering tiny
neighborhoods, one-row/column cases, odd shapes, vector-width tails, VGA, and
Full HD in both orientations, including an odd-width near-Full-HD row. Opened,
array-backed, and buffer-backed workflows remain separate. Size
points use deterministic varied, checkerboard, boundary, origin-impulse, and
seeded-noise samples where declared; `Image.new` uses varied/boundary fill
values because it creates a uniform image. The 1600×900 and three Full-HD size
points use one varied pattern to bound material-size inputs and recorded
outputs. They expand through the same declared operation, mode, and backend
support. The generator creates 75,562 matrix variants before workflow alias
consolidation; the indexed matrix contains 74,792 unique cases across all 25
mode identities. The three new sizes add 2,496 indexed parity cases. It skips
8,188 source-workflow/size/pattern combinations that do not have a safe
literal-source route. Matrix parity inputs are separately indexed per surface so every
JSON document stays under the 90 MiB contract. The three new large-image
expected-output cohorts are split by size and internal mode. The active
indexed corpus contains 99,816 parity cases across 81 parity documents.

The five CoreOnly high-depth modes have 460 target-only `ImageChops.duplicate`
cases with recorded native u16/f32 outputs: every one of the 31 sizes has its
declared input patterns. Large expected-output cases use eight size-specific
documents and 15 additional size/mode shards so each document stays under 90
MiB; the largest is 84.42 MiB. The indexed benchmark corpus contains 44,830
workloads: 43,797 mode/size/pattern workflows (43,247 with Pillow comparisons
and 550 target-only workflows: 460 CoreOnly Duplicate cases and 90 `L;1`
rawmode extensions) plus 1,033 existing non-matrix workflows. Each new size
adds 488 benchmark workflows. Every matrix benchmark
points to its matching parity input and declares Pillow plus CPU, SIMD, and GPU
subjects when Pillow comparison applies. CPU architecture targets replay the
same selected inputs under all nine selectors; architecture is an execution
dimension and does not duplicate case data. No parity or benchmark campaign
ran for this expansion because Stage 5 source closure is still pending. GPU
remains one WGPU kernel profile; the physical adapter selection and replay
axis are described in the follow-up below.

TODO(pillow-rs#3ptb): run the CPU architecture benchmark matrix after source
closure and verify that each result records the requested selector plus each
operation's actual kernel or fallback receipt. Keep the compatibility `simd`
profile separate from these CPU Architecture runs. No architecture-specific
benchmark evidence is claimed yet.

Completed by the CoreOnly output matrix expansion recorded below: the five
internal modes now have size/pattern outputs for Brightness, Invert, Offset,
Crop, Duplicate, and Resize. Execution remains pending until CPU source closure.

GPU adapter architecture follow-up (2026-11-11): added native WGPU adapter
inventory, explicit per-process adapter-index selection, and parity/benchmark
Make targets that replay the unchanged shared inputs once per enumerated
physical adapter. The parity execution receipt carries adapter name, vendor and
device IDs, device type, WGPU backend, driver, and selected index; benchmark
results carry the selected adapter identity and use a separate output per
adapter. `MIGRATION_GPU_ADAPTER_INDICES` can restrict the enumerated set. This
is source-only and has not been built or run. TODO(pillow-rs#3ptb): verify
adapter selection and receipt attribution after CPU source closure. This
selection axis describes physical WGPU devices; future CUDA or other GPU
kernel implementations remain separate backend/architecture work.

TODO(pillow-rs#3ptb): regenerate the parity, coverage, and benchmark evidence
pages from a fresh aggregate after source closure. The public-contract
specification page has been regenerated from the current indexed inputs; no
fresh parity or benchmark evidence is claimed.

Source-factory matrix boundary (2026-11-11): the 1,380 skipped attempts were
reviewed. They are not missing raster-operation matrix cells: `Image.open`,
`Image.fromarray`, and `Image.frombuffer` carry encoded-input or
array-interface contracts, while `ImageColor.getcolor` and the gradient
factories do not expose variable raster dimensions. Keep these workflows in
their source-specific contracts instead of changing their semantics to fit
the operation matrix.

The `Image.fromarray` source-factory matrix now feeds its results through
Resize across supported dtype/mode layouts and representative sizes; these
cases count as Resize execution, not constructor-only backend coverage.
Source-factory matrix review (2026-11-11): the indexed corpus contains 501
`Image.open`-backed workflows, including CPU operation calls; 96
`Image.fromarray` → Resize workflows across the existing dtype/mode and size
matrix; and three direct `Image.frombuffer` workflows. The current
`Image.frombuffer` facade delegates to `Image.frombytes`, so it has no separate
CPU mode or architecture executor. The shared operation matrix exercises the
same typed raster owners after native construction, while open/fromarray keep
their source-specific contracts. This closes the source-factory matrix review
for CPU execution placement; it does not claim those source workflows passed
parity or that their host/codec contracts are complete.

Resize target-only matrix and size-shard follow-up (2026-11-11): the public
Resize contract now declares all 20 Pillow modes plus five internal
`NO_PILLOW_SUPPORT` modes. The five internal modes have 460 recorded native
u16/f32 nearest-Resize output cases across the declared size/pattern schedule,
and 460 corresponding target-only benchmark workflows. The generator now
indexes large ordinary mode-size parity inputs by size for `PIL.Image.Image`
and `PIL.ImageChops`; expected-output size shards are emitted for both surfaces
when applicable. Regeneration produced 82,796 unique matrix parity cases
(107,830 total parity cases across 114 indexed documents), 51,709 matrix
benchmark workflows (50,699 Pillow-comparison and 1,010 target-only), and
52,742 indexed benchmark workloads overall. The five CoreOnly Duplicate modes
and five CoreOnly Resize modes each contribute 460 recorded-output workflows;
90 `L;1` rawmode extension workflows remain target-only. These workflows
declare CPU, SIMD, and GPU profiles; CPU selector replay remains the nine
configured architectures, while GPU remains one automatically selected WGPU
profile without a device/architecture axis. Manifest and inputs were
regenerated; no parity, benchmark, coverage, tests, or quality checks ran.
CPU source closure remains pending.

Pillow 13 Resize MKS source completion (2026-11-11): the typed filter parser
and coefficient strategy now include MKS2013 (code 6, support 2.5) and
MKS2021 (code 7, support 4.5), using the piecewise formulas in Pillow's
`Resample.c`. Coefficient-cache identities distinguish both filters. The
public Rotate and Transform parsers retain Pillow's narrower filter contract
and report MKS values as recognized resize-only filters. Direct typed
PipelineOp execution applies the same validation before CPU preparation, and
GPU admission rejects unsupported Rotate/Transform filter descriptors. The Python resize
facade validates resampling and reducing gap before defaulting the box and
tupling the size, checks the identity-copy path, and loads before PyO3 extracts
native box and gap values. The PyO3 adapter parses resampling first for direct
extension calls. Source-only; no build, formatter, checkstyle, lint, tests,
parity, benchmark, or coverage ran. The MKS parity inputs were added in the
matrix follow-up below; CoreOnly MKS expected outputs and execution evidence
remain pending.

Resize filter and mode matrix follow-up (2026-11-11): added public
`Resampling.MKS2013` and `Resampling.MKS2021` values (codes 6 and 7) to the
Python facade, and generated parity plus benchmark inputs for both filters
across all 20 Pillow Resize modes, the full 31-size schedule, and each declared
deterministic input pattern. Each filter contributes 2,982 indexed parity
cases and matching benchmark workloads. During the matrix audit, corrected the
RGBa requirement-slug mapping and ensured Resize mode requirements merged into
the default workflow still receive size/pattern expansion; RGB and RGBa are
now present in the matrix. The regenerated indexed corpus contains 90,528
unique mode-size cases and 115,562 parity cases across 114 indexed files, plus
59,349 mode-size benchmark workloads (58,339 Pillow-comparison and 1,010
target-only) and 60,394 benchmark workloads total. The largest indexed parity
document is 84.42 MiB, under the 90 MiB limit. No parity, benchmark, tests,
coverage, or quality checks ran. TODO(pillow-rs#3ptb): add independent MKS
expected outputs for the five CoreOnly Resize modes, then verify the expanded
inputs across CPU architecture selectors and GPU admission/output after CPU
source closure.

GPU adapter replay follow-up (2026-11-11): added explicit WGPU adapter
enumeration and selection by native inventory index, plus parity and benchmark
targets that replay the existing input corpus across each selected physical
adapter. The adapter inventory sidecar maps indexes to device identity, and
the parity/benchmark receipts carry the selected adapter details. This does
not duplicate input fixtures per architecture. Source-only; no build, parity,
benchmark, tests, coverage, or quality checks ran. TODO(pillow-rs#3ptb): verify
the adapter selection and receipt path after CPU source closure.

Rotate direct Rust entry follow-up (2026-11-12): `Image::rotate_with_input`
now selects Pillow's identity, half-turn, and eligible quarter-turn fast paths
before resolving fill or resampling inputs; discrete turns enqueue the existing
typed Transpose operation. Requests that reach Rotate validate its restricted
filter set even when the angle is a multiple of 360. The Python facade keeps
its matching host shortcut so subclass `copy`/`transpose` overrides stay
observable. Added a test-api-only binding and target-only expected-output
generation for all 25 exact modes and all direct shortcut branches. The
test-only observer reads native `DynamicImage` carrier bytes through the
existing unpacked-byte method. TODO in `ops/rotate.rs`: generate/freeze these
cases and run them under CPU Architecture selectors after Stage 5 closes.
Direct binding and case-helper markers record source review; manifest and
input-builder integration markers remain source-pending. Parity is pending and
the test hook has no production benchmark. No build,
formatting, lint, tests, parity, benchmark, or coverage ran. Source:
[Pillow 13.0 `Image.rotate`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L2303-L2375).

Maintained CPU boundary and large-file review (2026-11-11): the active
`pool_cpu` module graph has no `legacy` declaration, path import, or include;
the only Rayon/parallel-feature code found by the maintained CPU source scan is
inside the inert legacy archive. Normal routed batches call the context-aware
CPU entry and pass the router-selected `ExecutionContext`; the plain backend
trait entry resolves its context at that direct-call boundary. CPU capability
detection remains cached in `OnceLock`. Reviewed the five maintained CPU Rust
files in the 800-line review band: `imageops/ops.rs` (915), `enhance/ops.rs`
(896), `batch_execution.rs` (866), `filter/ops.rs` (845), and `put/ops.rs`
(840). Their largest manually scanned function spans are at most 100 physical
lines; this is not the Clippy-counted function-size proof. No split was
indicated by this review because the family `ops.rs` files hold support
composition and routing, while `batch_execution.rs` composes already-prepared
strategies. TODO(pillow-rs#3ptb): finish the complete operation/caller and
dead-code review, and prove function limits with the planned quality gate only
after all source review is closed. No build, formatter, checkstyle, lint, test,
parity, benchmark, or coverage ran.

FloodFill placement follow-up (2026-11-11): moved FloodFill support and
selection into the Draw family operation entry, and moved the unchanged
callback traversal into `pool_cpu/ops/draw/helpers/floodfill.rs`. Removed the
nested `draw/floodfill/{ops,default}` strategy tree and redirected the public
callback adapter to the family-owned support and helper. Added direct
FloodFill to the Draw ownership row in the structure guide. This is source-only;
compile, function-size quality proof, parity, and callback-order receipt review
remain pending with CPU source closure.

Initial 31-size source-workflow audit (2026-11-11): the generated input corpus
expanded eligible image-operation sources across that size schedule,
all 25 declared mode identities where supported, and deterministic varied,
checkerboard, boundary, impulse, and seeded-noise patterns. CPU selectors replay
the same inputs for nine architecture selectors; WGPU replay separately records
each selected adapter. The 1,380 skipped source-workflow/size/pattern attempts
come from source-factory cases rather than ordinary operation inputs: `open`
cases exercise its I/O-mode argument, `fromarray` cases carry fixed
array-interface shape/data contracts, and `ImageColor.getcolor` plus gradient
factories have no variable image-size input. Do not mutate these workflows to
force them into the raster-operation matrix. TODO(pillow-rs#3ptb): add separate
source-specific factory matrices only where a valid size/dtype/mode contract is
defined, then execute matrix parity and benchmark evidence after CPU source
closure. No corpus files were changed in this audit.

Family-helper placement follow-up (2026-11-11): moved Thumbnail's shared
mode-policy selector, Contrast wrapper routing, scalar Point preparation and
mode dispatch, ImagingCore set-mode orchestration, and PutData value-at
dispatch under their owning family `helpers/` modules. These moves preserve the
existing functions and call contracts. Pad's 25 mode trees now sit directly
under `geometry/<mode>/`, Pad support and preparation are composed from the
family `geometry/ops.rs`, and its reusable helpers are under
`geometry/helpers/`. At this checkpoint, the path audit still found `color/convert/` (241 strategy
and support files), ImageOps operation files at family root plus `colorize/`,
and `composite/ops/` operation submodules. Color Convert was flattened in the
follow-up below; the ImageOps and Composite findings remain open. The analysis
operation modules were flattened into `analysis/ops.rs` in the follow-up below.
TODO(pillow-rs#3ptb): merge the remaining outliers into the family
mode/architecture strategy tree and family support/orchestration modules,
preserving shared helpers and co-locating operations that share a mode. No
build, formatter, lint, tests, parity, benchmark, or coverage ran.

Analysis and geometry path review follow-up (2026-11-11): consolidated the
GetBBox and GetProjection support/prepare entry points into
`analysis/ops.rs`, removing the extra `analysis/ops/` source directory while
preserving the exported names and exact-mode dispatch. Corrected prepared
operation registration for Pad to use `geometry::ops::PreparedPad`; the old
`geometry::pad` module was deleted during the preceding move. Static path
searches now find no active references to the deleted Pad path and no active
imports from `pool_cpu/legacy`. This remains source-only and uncompiled. At this checkpoint, family path debt still included `color/convert/`, ImageOps
root operation files and `colorize/`, and `composite/ops/`. Color Convert was
flattened in the subsequent follow-up below; the ImageOps and Composite moves,
complete review, and quality-gate size proof remain pending.

Convert helper placement follow-up (2026-11-11): moved the 18-file shared
Convert helper group from `color/convert/helpers/` to
`color/helpers/convert/`, matching the family helper scope for code shared by
multiple Convert modes and CPU callers such as Paste. Updated active imports
and relative module paths; no active source reference to the removed helper
path remains. At this checkpoint, the 241 mode-strategy/support files still
under `color/convert/` remained to be flattened; the subsequent follow-up below
records that move. Source-only; no build, formatter, checkstyle, lint, tests,
parity, benchmark, or coverage ran.


Color Convert strategy flattening follow-up (2026-11-11): moved all 25 exact
mode owners directly under `color/<mode>/`, including the five modes that
already owned other color operations. Their Convert strategies now share the
exact mode owner and architecture folders. The Convert and ConvertMatrix
contracts, source-mode classification, and registry preparation entry points
now live in `color/ops.rs`; shared Convert code stays under
`color/helpers/convert/`. Updated Rust callers and removed the
`color/convert/` module tree. Static source scans find no remaining
`color::convert` imports, all 25 mode directories exist, and every declared
color module resolves to a source file. `color/ops.rs` is 506 physical lines.
This is source-only and uncompiled; mode strategy calls, support, wrapper
metadata, and architecture gates remain part of the pending CPU source and
caller review. No build, formatter, checkstyle, lint, tests, parity, benchmark,
or coverage ran.

ImageOps Autocontrast family-entry follow-up (2026-11-11): moved the exact-mode
support declaration, prepared strategy, and mode dispatch from the family-root
`autocontrast.rs` into `imageops/ops.rs`, then removed the sibling module. The
L, P, and RGB strategy routes and LUT preparation are unchanged. Static source
searches find no active import of the removed module. `imageops/ops.rs` is now
995 physical lines, leaving five lines below the file limit; resolve its
remaining family-root operation modules without growing past that limit.
Equalize, Expand, ExpandEdges, and Quantize placement and the Composite
operation modules remain pending. Colorize's redundant one-operation
`colorize/ops.rs` wrapper has since been flattened to `imageops/colorize.rs`;
its registry references now resolve directly to that family module. No build, formatter, checkstyle,
lint, tests, parity, benchmark, or coverage ran.

Quantize native-mode dispatch follow-up (2026-11-11): kept CoreOnly Quantize
on one `DynamicImage` match and moved its repeated exact-mode
forwarding into a local compile-time macro. Every native mode still calls
its existing mode `ops.rs` entry with the same raw native buffer and arguments;
the selected architecture remains owned by that mode strategy. This avoids an
additional runtime match and changes no quantization algorithm or fallback.
Source-only; no compiler, formatter, checkstyle, lint, tests, parity, benchmark,
or coverage ran. TODO(pillow-rs#3ptb): close the remaining ImageOps and
Composite source layout review, then complete the whole CPU code review before
running the gated quality and parity/benchmark campaigns.

Composite family entry consolidation follow-up (2026-11-11): CompositeModule's
support, prepared pipeline orchestration, and exhaustive mode dispatch now live
in `composite/ops.rs`; the active `composite/composite_module.rs` file and its
module declaration were removed. One 25-mode mapping generates both the
operation support lookup from each mode's `COMPOSITE_MODULE_MODE_SUPPORT` and
the exact-mode execution routing. The shared Composite input contract and its
wrapper size/mask checks live in
`composite/helpers/composite_module_support.rs`; the family entry re-exports
the wrapper checks, preserving their caller paths. The support macro is
declared in all 25 exact mode owners, including marked CoreOnly identities.
Static review confirms `composite/ops.rs` is 976 lines, the nested
`composite/ops/` directory is absent, and no active import names
`composite_module.rs`. This remains source-only; do not infer compilation,
parity, or performance from the path checks. Composite callers and exact-mode
behavior remain in the final CPU source review and frozen parity matrix.
No build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage
ran.

ImageOps Colorize module flattening follow-up (2026-11-11): moved the
Colorize wrapper support declarations out of the one-file
`imageops/colorize/{mod.rs,ops.rs}` wrapper and into the direct
`imageops/colorize.rs` family module. Updated its shared-helper path and the
operation registry now resolves both contracts through the ImageOps family
`ops.rs` entry, which re-exports them. Removed the empty wrapper directory.
Static searches find no remaining imports of `colorize::ops` or the removed
path. The L-to-RGB composition, `_lut` route, and support declarations are
unchanged. The family `ops.rs` is now 998 physical lines, two below the file
limit. This is source-only and uncompiled. TODO(pillow-rs#3ptb): finish the
remaining ImageOps family-entry layout and caller review, then prove CPU size
limits before the gated verification campaign.

ImagingCore Expand descriptor consolidation (2026-11-11): the direct callback
preparation now selects through `imageops/ops.rs::IMAGING_CORE_EXPAND_SUPPORT`,
the same descriptor exposed by the registry. Removed the duplicate local
`OperationSupport` declaration from `expand_edges.rs`; that module retains
mode classification, typed preparation, and exact-mode edge replication.
Static references show one descriptor feeding both support inspection and
preparation. Source-only; no compilation or quality checks ran.

ImageOps Colorize and Equalize family-entry consolidation (2026-11-11): moved
both operation support/preparation paths into `imageops/ops.rs`. Their public
`OperationSupport` descriptors now live beside the family entry; the shared
wrapper-route classifiers for Colorize, Invert, Posterize, and Solarize are in
`imageops/helpers/wrapper_routes.rs`. Equalize's support, typed prepared plan,
mask admission, and exact L/RGB/P preparation moved unchanged; its pixel
strategies remain with their exact mode owners. Removed the family-root
`colorize.rs` and `equalize.rs` modules and declarations. Static searches find
no active references to either removed module. `imageops/ops.rs` is 947
physical lines. This is source-only and uncompiled. No build, formatter,
checkstyle, lint, tests, parity, benchmark, or coverage ran.

CPU family-root module inventory (2026-11-11): CompositeModule was consolidated
into `composite/ops.rs` in the follow-up above. A static path scan now finds two
active ImageOps implementation modules outside family `mod.rs`/`ops.rs`:
`expand.rs` and `quantize.rs`. Consolidate their support/preparation at the
family entry and keep pixel algorithms in exact mode/architecture owners,
without adding another operation-folder layer or exceeding the 1,000-line
source limit. ImageOps `ops.rs` currently has 947 physical lines. TODO(pillow-rs#3ptb):
complete this layout and caller review, then finish CPU source/dead-code review
before quality, parity, or benchmark verification. Source-only; no builds,
formatting, lint, tests, parity, benchmark, or coverage ran.

CoreOnly parity and benchmark input expansion (2026-11-11): generated
size/pattern expected outputs for five internal modes across Duplicate, Resize,
Crop, Offset, and Invert (460 cases per operation), plus Brightness at zero,
identity, and double factors (1,380 cases). This adds 2,760 target-only cases
and matching benchmark workloads; with the existing 460 Resize, 460 Duplicate,
and 90 `L;1` cases, the target-only mode/size benchmark matrix now has 3,770
workloads. The complete indexed corpus now contains 93,288 mode/size parity
cases, 118,322 parity cases across 1,320 indexed parity documents, 62,109
mode/size benchmark workflows (58,339 Pillow-comparison and 3,770 target-only),
and 63,142 benchmark workloads overall. These inputs use CPU, SIMD, and GPU
profiles; the CPU architecture and GPU adapter runners replay the same cases.
No parity, benchmark, coverage, test, or quality run was made. TODO(pillow-rs#3ptb):
after CPU source closure, execute the frozen CPU architecture parity and
benchmark matrices, then GPU parity/benchmark and adapter replay as applicable.

ImagingCore Expand helper placement (2026-11-11): moved the all-mode prepared
edge-replication route from the ImageOps family root to
`imageops/helpers/expand_edges.rs`; the public `IMAGING_CORE_EXPAND_SUPPORT`
descriptor remains in `imageops/ops.rs` and is still the descriptor consumed by
preparation. Updated the `Image::imaging_core_expand` caller. The byte-copy
algorithm, all 25 mode classifications, Default execution, and error behavior
were left unchanged. Source-only; no build, formatting, checkstyle, lint, test,
parity, benchmark, or coverage ran. Seven active family-root modules remain in
the inventory above; CPU source review and verification gates are still open.

BlendModule family entry consolidation (2026-11-11): moved the exhaustive
support table, architecture declarations, prepared mode dispatch, and pipeline
adapter from `composite/blend.rs` into the Composite family `ops.rs`. Exact
mode implementations remain in their existing mode owners. Removed the sibling
module declaration and re-export; registry and public operation references now
resolve through the family entry. The family file is 480 lines, below the
1,000-line limit. Source-only; no build, formatting, checkstyle, lint, test,
parity, benchmark, or coverage ran. Six family-root modules remained at this
checkpoint.

Merge family entry consolidation (2026-11-11): moved Merge's support and
architecture declarations, mode dispatch, prepared route, and pipeline adapter
from the sibling `composite/merge.rs` into Composite `ops.rs`. Updated the
module declaration and active callers; exact mode implementations are
unchanged. Static searches find no active references to the removed sibling.
Composite `ops.rs` is 836 physical lines, under the 1,000-line cap. Five active
family-root modules remain: CompositeModule and ImageOps Colorize, Equalize,
Expand, and Quantize. Source-only; no build, formatter, checkstyle, lint, test,
parity, benchmark, or coverage ran.

Mode/size parity and benchmark input expansion (2026-11-11): added 257×193,
513×385, and 1025×769 odd dimensions above the 256, 512, and 1024 tiers, with
checkerboard and seeded-noise inputs on representative sizes. Regenerated the
indexed 34-size, 99 size/pattern-pair matrix across all 25 Mode identities.
The current index contains 100,493 `.matrix-size-*` parity cases (125,527 total
parity cases across 1,446 parity documents) and 66,892 matching mode/size
benchmark workloads (62,835 Pillow comparisons and 4,057 recorded-output
workloads; 67,925 benchmark workloads total). The added sizes contribute 7,205
parity cases and 4,783 benchmark workloads. Every matrix benchmark workload
includes CPU, SIMD, and GPU target profiles. CPU architecture and WGPU adapter
sweeps replay the same input definitions. Input generation used the pinned
Pillow 13.0.0 environment; no parity, benchmark, coverage, test, or quality run
was performed. TODO(pillow-rs#3ptb): finish CPU source closure before running
the frozen architecture/backend parity and benchmark matrices.

ImageOps and CPU family-root consolidation follow-up (2026-11-11): moved
Expand's exhaustive 25-mode preparation into `imageops/helpers/expand.rs` and
placed its mode-to-support map in `imageops/ops.rs`. Moved Quantize's prepared
mode routing into `imageops/helpers/quantize.rs`; its exact-mode source/default
method mapping and complete support/input contract now live in `imageops/ops.rs`.
The public CPU operation adapter still enters through that family file.
Autocontrast and Equalize preparation now reside with the other ImageOps
helpers. Removed the remaining family-root ImageOps implementation files.
Static path inventory finds that every direct family under `pool_cpu/ops/`
contains only `mod.rs` and `ops.rs`; ImageOps has no active import of the
removed sibling modules. The shared native-byte Posterize/Solarize exact-mode
classifier now lives in `imageops/helpers/point_support.rs`, leaving the
family-owned support declarations in `imageops/ops.rs`. The reviewed ImageOps
entry has 953 physical lines; all 39 functions in it and the two moved helper
functions have source-review VERIFY markers. `composite/ops.rs` has 976 lines.
The operation/mode caller review, Clippy-counted function-size proof, and
output/fault/metadata/receipt parity remain pending.
The Quantize and Expand parity TODOs stay open. Source-only; no build,
formatting, checkstyle, lint, tests, parity, benchmark, or coverage ran.

Current-tree source crosswalk review (2026-11-11): regenerated the static
operation/source graph from the dirty `develop` checkout into a temporary file
outside the repository. It maps all 216 manifest public operations and 87
pipeline variants, sees 125,527 declared parity cases and 67,925 benchmark
workloads, and reports no unresolved Rust module edges, operation-fixture
links, requirement references, or family-layout roles. Its six review findings
are two Convert registration misses caused by the parser not following the
`pub(crate) use` alias for `prepare_convert_pipeline` in `color/ops.rs`, plus
four GPU shader asset candidates reserved for the later GPU source review.
Direct inspection confirms the Convert alias delegates to the shared prepared
helper; these parser findings are not evidence of a missing implementation or
dead code. This inventory is source mapping only and does not prove runtime
selection or execution. No build, formatter, lint, tests, parity, benchmark,
or coverage ran.

Current-tree source crosswalk refresh (2026-11-12): regenerated the graph from
the current dirty `develop` checkout after the input-matrix expansion. It maps
all 216 public operations, all 87 pipeline variants, 171,666 declared parity
cases, and 100,610 benchmark workloads. The crosswalk reports zero unresolved
Rust module edges, Python-to-PyO3 native calls, direct PyO3-to-core calls,
operation-fixture links, requirement references, or benchmark-to-parity
references. Its six findings remain two parser misses for the Convert
`prepare_convert_pipeline` alias and four shader-asset candidates reserved for
the later GPU review. This is static mapping only: the report marks Rust
reachability, runtime selection, and execution unproven, and its 281 source
items without a discovered operation-graph route remain review candidates.
The generated report is `target/operation-source-crosswalk.json`; no build,
formatter, checkstyle, lint, test, parity, benchmark, or coverage ran.

Baseline archive hash audit (2026-11-12): compared every file under
`pool_cpu/legacy` with its original path at `3f9dfd4036caa061eabcf2255c9f34a745df9c7e`
using Git blob hashes. All 280 baseline Rust files match exactly: 98 CPU
files (96 under `ops/` plus the two CPU root files) and 182 SIMD files. No
baseline file is missing or differs. Five ignored `.DS_Store` metadata files
are present in archive directories; they are not baseline source, and were
left untouched. This verifies backup contents, not compiler reachability or
the correctness of maintained replacements. No build, formatter, checkstyle,
lint, test, parity, benchmark, or coverage ran.

Crosswalk Convert re-export follow-up (2026-11-12): the CPU registration
resolver now follows grouped `pub(crate) use` aliases to their concrete
function definitions. The regenerated graph connects the Convert registration
through `color/ops.rs::prepare_convert_pipeline` to
`color/helpers/convert/prepared.rs::prepare_pipeline`; the two earlier parser
findings are gone. Four GPU shader asset candidates remain for the later GPU
review. The generator reports 1,111 edges, zero unresolved module edges, and
zero unresolved host/core/fixture/requirement/workload references. Static
mapping still does not prove compiler reachability or runtime execution. No
tests, build, formatter, checkstyle, lint, parity, benchmark, or coverage ran.

PutData typed-u16 input matrix follow-up (2026-11-11): added 36 direct numeric
parity cases for `I;16`, `I;16L`, `I;16B`, and `I;16N` across five sizes from
1×1 through odd HD tails, with boundary, checkerboard, varied, and seeded-noise
samples. Added a matching benchmark workload for each case; each compares
Pillow, CPU, SIMD, and GPU subjects and uses the existing CPU architecture and
GPU adapter replay paths. The measured steps are the public `putdata` call and
receiver `tobytes` observation. These are input definitions only; no parity,
benchmark, quality, or coverage run occurred.

PutData mode/size input expansion (2026-11-11): expanded the manifest to declare
all 20 Pillow mode identities for PutData and added 144 matching parity cases
and benchmark workloads for the remaining non-I;16 input matrix. The new cases
cover 1×1, 17×19, 65×47, and 257×193, using varied, boundary, checkerboard,
impulse, and seeded-noise samples where assigned by size. Workloads measure the
public `putdata` call and receiver `tobytes` observation and select CPU, SIMD,
and GPU target profiles. CPU architecture and physical WGPU adapter sweeps
replay the same inputs. Normalized the existing 36 typed-u16 case and workload
identities to `i16`, `i16l`, `i16b`, and `i16n`, matching the manifest mode
requirements. This updates input definitions only; no parity, benchmark, test,
quality, or coverage run occurred.

CoreOnly PutData native-write extension (2026-11-11): implemented context-
selected default writers for LA16, RGB16, RGBA16, RGB32F, and RGBA32F. Integer
components clamp to native u16 samples; float components cast directly to
native f32. RGB accepts and ignores an optional fourth component; RGBA accepts
three components and supplies opaque alpha. Packed integers populate native
u16 lanes. Multiband scale and offset remain ignored. Python provides float
tuples as a typed value, and the JavaScript adapter accepts the additive value
kind. Added 575 target-only expected-output cases and matching benchmark
workflows across the declared size/pattern matrix. These inputs were indexed
and size-checked against Pillow 13.0.0; no Rust build, formatter, lint, test,
parity, benchmark, or coverage run occurred. TODO(pillow-rs#3ptb): verify these
public paths after CPU source closure and the frozen parity/benchmark gates.

CPU pre-gate source and input audit (2026-11-11): on the current `develop`
checkout, all 1,085 manifest requirements with `dimension: mode` and a parity
lane are referenced by at least one indexed parity case. This proves the
manifest requirement-to-input join only. The Rust support inventory still
contains 113 `OperationId` values × 25 exact modes (2,825 source-mode outcomes);
reconciling those outcomes with public requirements and composed wrapper paths
remains open. The operation/mode/architecture execution matrix is not proven
until the frozen CPU suite runs under each selector.

The maintained `pool_cpu` path inventory has no extra Rust files or nested
`ops/` directories directly under family roots. Seven maintained files are at
least 800 physical lines; all remain below 1,000, with `composite/ops.rs` the
largest at 976 and `imageops/ops.rs` at 953 after moving the shared native-byte
point classifier to `imageops/helpers/point_support.rs`. The Pad strategy
ownership move brought `geometry/ops.rs` below 800 lines. This is a manual
physical-line audit; Clippy-counted function limits and compiler-backed
reachability remain for Stage 6. No build, formatter, checkstyle, lint, test,
parity, benchmark, or coverage ran.

Mode/size backend matrix follow-up (2026-11-12): expanded and regenerated the
active input index to 171,666 parity cases across 4,046 documents and 100,610
benchmark workloads in 78 suites. The corpus contains 146,591
`.matrix-size-*` parity cases and 99,556 mode/size benchmark workflows. Every
eligible mode/size case now maps to an operation benchmark contract; the
generator reports zero missing benchmark requirements. Workflow subjects now
follow each operation's declared target profiles: 95,416 matrix workflows use
CPU, 805 use CPU and SIMD, and 3,335 use CPU, SIMD, and GPU. A profile audit
found no mismatches, correcting the earlier interim claim that every workflow
should use all three profiles. The shared matrix has 38 unique sizes and 115
size/pattern pairs, using varied, checkerboard, boundary, impulse, and seeded
noise inputs where assigned. It expands only operation/mode pairs declared by
their support contracts. GetBBox covers 25 Mode identities across all 115
inputs (2,875 workflows) and declares the CPU profile only. Restored the
1024×768 filled-F GetBBox case as an indexed parity case and benchmark. The
Makefile's nine CPU architecture selectors and selected physical GPU adapter
indices reuse the same inputs; architecture is chosen by the runner, not
encoded as duplicate fixtures. No parity, benchmark, test, Rust build, lint,
or coverage campaign ran. The input generator completed its manifest/index
and generated-asset checks. TODO(pillow-rs#3ptb): finish CPU source closure and
function-size review, then run the frozen CPU parity and benchmark matrix under
the declared architecture selectors. GPU profile and adapter execution remains
after CPU completion.

2026-11-12 Pad strategy ownership follow-up: moved the Default vertical-append
routes for Luma8, RGB8, RGBA8, CMYK8, HSV8, and RGBX8 into those exact mode
strategy modules. Moved the vector vertical-append routes for Luma8, RGB8,
RGBA8, CMYK8, and RGBX8, plus the fused Resize-to-canvas routes for Luma8 and
RGBX8, into their mode owners. geometry/ops.rs now sequences prepared Resize
and Pad, records the shared execution telemetry, and routes to these mode
strategies. The existing generic row-copy and vector kernels remain shared
helpers; the typed mode/architecture path selection is localized to the mode
owner. Manual source comparison confirms the previous route conditions and
kernel calls are preserved. geometry/ops.rs is now 767 physical lines; the
new mode strategy methods are each below 50 source lines. This is source review
only; the complete Clippy-counted function-size audit and compiler/runtime
reachability remain open. No build, formatter, checkstyle, lint, test, parity,
benchmark, or coverage ran.

2026-11-12 post-Pad source crosswalk refresh: regenerated
target/operation-source-crosswalk.json after moving Pad's mode-specific fast
paths. The graph still maps all 216 public operations, 87 pipeline variants,
171,666 parity cases, and 100,610 benchmark workloads with 1,111 edges and
zero unresolved Rust module edges, host/core calls, fixture links, requirement
references, or benchmark/parity references. Its 281 static Rust
unreachability candidates include all 280 files under pool_cpu/legacy and no
maintained pool_cpu source file. Four remaining source findings are GPU shader
asset candidates reserved for that later stage. This is static source mapping,
not compiler reachability or execution evidence. No build, formatter,
checkstyle, lint, test, parity, benchmark, or coverage ran.

2026-11-12 resize source-candidate review: inspected the sole static
unreachability candidate outside pool_cpu/legacy,
`pillow-rs/src/ops/pil_resize/vertical_passes/transpose.rs`. Git history shows
its former resize-intermediate transpose implementation was removed from this
file; the current file contained only a bare `#[inline]` attribute and no
module, include, or caller referenced it. Removed the inert source artifact.
The maintained CPU resize transpose helper remains under
`compute/pool_cpu/ops/resize/helpers`. The refreshed static crosswalk maps
5,489 Rust source files and leaves 280 unreached candidates, all under
pool_cpu/legacy; it retains 1,111 graph edges and four GPU shader findings.
This resolves the only non-archive Rust candidate but does not prove compiler
reachability of the remaining sources or close CPU source review. No build,
formatter, checkstyle, lint, test, parity, benchmark, or coverage ran.

2026-11-12 ImageOps family-entry review: manually reviewed the complete
`pool_cpu/ops/imageops/ops.rs` source. The family entry contains operation
support declarations, exhaustive mode-to-owner tables, typed preparation, and
prepared-child composition; its hot pixel loops remain in exact-mode
strategies and helpers. Expand and CropBorder have exhaustive 25-mode tables;
pointwise operations explicitly admit L, P, and RGB and classify every other
mode as rejected. CropBorder validates width before height and delegates
nonempty copying to the selected Crop strategy. This does not cover every
caller, wrapper callback/fault contract, or runtime selection receipt; those
remain TODOs in the source and parity plan. No build, formatter, checkstyle,
lint, test, parity, benchmark, or coverage ran.

2026-11-12 Composite family-entry review: manually reviewed the complete
`pool_cpu/ops/composite/ops.rs` source (976 physical lines). It contains
support/admission, input-fact preparation, context-selected architecture
declarations, and typed exact-mode strategy dispatch; no pixel loops are in the
family entry. CompositeModule and Merge each have exhaustive 25-mode routes;
Blend and AlphaComposite expose their narrower accepted-mode contracts and
reject the other identities. Function source spans are below 100 physical
lines; Clippy-counted size proof remains gated. Full Composite/Blend/Merge
mode/architecture output and receipt parity remains open. No build, formatter,
checkstyle, lint, test, parity, benchmark, or coverage ran.

2026-11-12 Enhance family-entry review: manually reviewed the complete
`pool_cpu/ops/enhance/ops.rs` source (896 physical lines). It owns exact-mode
support, wrapper composition descriptions, typed preparation, and exhaustive
prepared-strategy dispatch; no pixel loops are in the family entry. Brightness
and Sharpness explicitly classify all 25 identities, ColorSaturation declares
six native mode strategies plus its typed conversion/blend wrapper routes, and
Contrast routes through its mode-owned preparation. Unsupported modes retain
their declared rejection reasons. Caller fault ordering, receipts, and
operation/mode/architecture parity remain open; no build, formatter,
checkstyle, lint, test, parity, benchmark, or coverage ran.

2026-11-12 Filter family-entry review: manually reviewed the complete
`pool_cpu/ops/filter/ops.rs` source (845 physical lines). It owns exact-mode
support classification, typed filter requests, prepared context/architecture
selection, and UnsharpMask composition; no pixel loops are present. ModeFilter
routes its 14 byte identities to exact-mode owners, while standard palette
filtering and unsupported high-depth identities retain explicit rejections.
Gaussian/box blur architecture dispatch is limited to the declared exact modes;
F32 rank compatibility is delegated to the F32 owner. Wrapper loading/error
order, physical-lane cases, output/fault parity, and actual receipts remain
open. No build, formatter, checkstyle, lint, test, parity, benchmark, or
coverage ran.

2026-11-12 native CoreOnly PutData source checkpoint: the five internal
LA16/RGB16/RGBA16/RGB32F/RGBA32F modes now have mode-owned typed bulk writers.
They normalize each native sample directly, select the declared Default
strategy once, materialize/copy the image once, and preserve ordered prefix
writes if a later Rust value is invalid. Exact built-in Python component tuples
can use that bulk route; custom or malformed values retain callback-visible
per-item writes. I;16 remains on its existing separately ordered native path.
This change is source-reviewed only. No tests, build, formatting, lint, parity,
benchmark, or coverage ran.

The indexed input matrix covers 171,666 parity cases and 100,610 benchmark
workloads across 38 sizes and 115 size/pattern pairs. It declares CPU/SIMD/GPU
profiles, nine CPU architecture selectors, and physical GPU adapter replay over
the same varied inputs; selectors replay cases rather than multiplying case
IDs. These are matrix declarations, not execution evidence. Matrix source and
runner review remain part of the pre-validation CPU gate.

2026-11-12 Put family-entry review: manually reviewed the complete
`pool_cpu/ops/put/ops.rs` entrypoint after adding the typed CoreOnly bulk route.
The entrypoint composes support declarations, exhaustive mode classification,
typed prepared enums, context-selected Default strategies, and exact-mode
dispatch; the pixel loops remain in mode owners and helpers. The accepted and
rejected PutAlpha/PutAlphaData and PutPixel sets are explicit, and the five
CoreOnly PutData writers remain native typed outputs. Python destination-mode
validation precedes mask validation on PutAlphaData as required by its wrapper
contract. Callback order, full error/fault outputs, selection receipts, and
CPU parity remain open. No verification command ran.

2026-11-12 Analysis family-entry review: manually reviewed all of
`pool_cpu/ops/analysis/ops.rs` (812 physical lines). Exact-mode owners supply
the support classifications for pixel access, extrema, histogram/entropy,
GetColors, Stat, Split, GetChannel, GetData, GetBBox, and GetProjection. The
family entry composes support, mode maps, prepared selection, and typed
execution; materialized projection dispatch remains mode-specific. No pixel
loops are present in the family entry. Caller ordering, exact result shapes,
architectures/fallbacks, receipts, and CPU parity remain open; no verification
command ran.

2026-11-12 Color family-entry review: inspected the exact-mode Convert,
ConvertMatrix, ConvertLab, and RemapPalette support and preparation paths in
`pool_cpu/ops/color/ops.rs`. Source/target declarations, CoreOnly targets,
output-mode rules, architecture selection, and typed prepared dispatch are
visible at the family entry; pixel algorithms remain in mode owners/helpers.
The Convert target composition is shared with its prepared strategy. Source
and target output/fault parity, architecture receipts, and runtime selection
remain open. No verification command ran.

2026-11-12 Geometry family-entry review: inspected the complete
`pool_cpu/ops/geometry/ops.rs` (767 physical lines). Contain, Cover, Fit, and
Scale declare all 25 modes and compose the selected Resize child; geometry
validation precedes child admission as required by each wrapper. Pad declares
all 25 exact-mode owners, typed fill contracts, and a selected Resize child;
its direct resize-canvas and vertical-append paths are limited to their named
mode owners, with exact-mode Pad fallback for the rest. Hidden physical lanes
are preserved after padding. Wrapper callback order, outputs, and nested
selection/fallback receipts remain open. No verification command ran.

2026-11-12 Resize family-entry review: inspected the support facade,
`helpers/prepared.rs`, and `helpers/mode_dispatch.rs`. Resize and ResizeBoxed
classify all 25 modes through exact-mode owners; effective-filter policy,
ordinary and boxed requests, and straight-alpha reducing-gap admission are
explicit. Prepared plans bind source mode, Backend, Architecture, and child
selection before execution. Byte, I;16, and CoreOnly high-depth modes dispatch
to separate typed owner groups; boxed crop, tall-image, alpha, sparse-RGB, and
Pad-window paths retain their specialized routes. Full kernel/caller parity
and fallback receipts remain open. No verification command ran.

2026-11-12 Chops family-entry review: inspected all of
`pool_cpu/ops/chops/ops.rs` (749 physical lines). Binary-operation input
contracts group exact byte modes by channel width; each operation's support is
supplied by the corresponding mode owner. Unary Constant, Offset, Duplicate,
and Invert expose explicit mode outcomes, including marked CoreOnly
extensions. Preparation materializes secondary inputs before assessing their
mode/size/storage facts and retains the selected architecture; Multiply/Screen
fusion admits only exact shared-input modes with matching selections. Hidden
physical lanes are handled at the execution boundary. Public fault/output/
fusion receipts and full mode/architecture parity remain open. No verification
command ran.

2026-11-12 Draw family-entry review: inspected the full
`pool_cpu/ops/draw/ops.rs` (652 physical lines). Draw declares exact mode,
context, mask, palette, and input contracts; eligible Paste composition is
guarded by the declared child route. FloodFill exposes Pillow and marked
CoreOnly mode outcomes but selects CPU Default only. Bitmap/Text use the same
mode-owned routing contract, with explicit CoreOnly rejections. Geometry batches
bind declared Default/architecture strategies and validate exact mode/backend
selection. Text behavior, route receipts, i32 origin bounds, and callbacks
remain open for parity review. No verification command ran.

2026-11-12 Thumbnail family-entry review: inspected the complete
`pool_cpu/ops/thumbnail/ops.rs` (584 lines). Thumbnail declares composed Reduce,
Resize, and ResizeBoxed support and classifies exact source modes. Preparation
binds source mode and injected context, computes factors/filter policy, and
prepares concrete child selections; execution consumes those prepared children
or uses the typed I32 streaming owner. The specialized RGB PutPixel fusion is
restricted by exact mode, backend, size, factor, filter, and prepared child
selection. Filter/reducing-gap behavior, stream and child outputs, fusion
receipts, and full mode/architecture parity remain open. No verification
command ran.

2026-11-12 Thumbnail owner review: inspected the exact-mode policy owners,
mode strategy, I32 Default/AVX streaming owners and ring helper, F32 policy,
and RGB PutPixel fusion implementation. Mode 1 and P force nearest; LA/RGBA
route reducing-gap through boxed Resize, while La/PA use separate reduction
policy. I32 streaming is limited to even 2x2 reducing-gap cases and retains
architecture-bound FMA selection; F32 has a narrow unboxed-resize condition.
RGB fusion is guarded to large even RGB inputs and materializes only affected
2x2 blocks. Oracle output, partial-failure, fallback-receipt, and full
architecture performance checks remain open. Source review only.

2026-11-12 matrix-generator source review: inspected the shared mode/size axes,
mode-correct pixel and raw-byte pattern builders, case expansion, Image.open
asset creation, and benchmark workload construction. The axes cover 38 sizes,
115 assigned size/pattern pairs, and five deterministic data patterns; source
constructors are rewritten without converting image modes. Matrix parity cases
are reused as benchmark inputs. The Makefile replays these inputs across nine
CPU architecture selectors and across available GPU adapter indices; operation
support determines eligible target profiles. Target-only CoreOnly outputs stay
separate from Pillow comparisons. The existing indexed manifest summary records
171,666 parity cases and 100,610 unique workloads. This was source inspection;
no generator, validator, parity, benchmark, or coverage command ran.

2026-11-12 Effects family-entry review: inspected `pool_cpu/ops/effects/ops.rs`
and its registry routes. Gradient constructors and Mandelbrot have explicit
mode/output contracts; EffectSpread and EffectNoise classify all 25 Mode
identities through exact-mode owners and injected architecture strategies, with
serial Default selection retained as fallback. Mandelbrot is fixed to L, and
CoreOnly Noise dimension-carrier routes are marked `NO_PILLOW_SUPPORT`.
Generator/noise outputs, RNG receipts, wrapper order, and full mode/architecture
parity remain open. Source review only; no verification command ran.

2026-11-12 Eval family-entry review: inspected the complete
`pool_cpu/ops/eval/ops.rs` and direct point registration. Byte-LUT Eval admits
14 byte modes and explicitly rejects the other 11; float-LUT Point admits
One/L/P and returns F only after exact source-mode agreement. Mode owners bind
SIMD or serial Default strategies, and prepared execution initializes omitted
physical lanes. LUT validation, callback order, architecture receipts,
hidden-lane follow-on behavior, and complete mode/architecture parity remain
open. The archived Rayon row path is deferred to `pool_parallel`. Source review
only; no verification command ran.

2026-11-12 Paste family-entry review: inspected the complete
`pool_cpu/ops/paste/ops.rs`, shared source/mask contracts, representative mode
owners, and public staged Paste route. The facade materializes secondary
inputs, assesses exact destination mode and source/mask facts, then prepares
and dispatches a mode-owned strategy using its captured selection. All 25
destination modes are explicit; byte layouts declare physical aliases and
masks, while CoreOnly typed modes admit exact-mode sources. Paste preserves the
destination mode. Conversion and callback order, aliasing, clipping and
mismatch faults, hidden lanes, and mode/architecture output receipts remain
open. Source review only; no verification command ran.

2026-11-12 Grayscale and Color3DLut family-entry review: inspected both
complete facades. Grayscale declares all 25 mode outcomes, routes 24 exact
modes to owners, preserves the explicit La rejection, and fixes output to L;
palette modes receive palette facts during preparation. Color3DLut admits six
exact byte source modes, with YCbCr/LAB/HSV marked as CoreOnly native
extensions; it declares RGB/RGBA/CMYK as its target set and rejects other
sources explicitly. Prepared plans retain the exact source/target pair and
architecture selection. Output/fault contracts, palette details, fallback
receipts, and complete mode/architecture parity remain open. Source review
only; no verification command ran.

2026-11-12 Rotate and Transpose family-entry review: inspected both complete
facades and representative One, RGB8, and CoreOnly LA16 owners. Rotate binds
all 25 source identities to mode-owned contracts, retains the full request,
and finalizes hidden lanes using the selected implementation; I;16* remains
typed Default-only until a floating-geometry vector path is justified.
Transpose, Flip, and Mirror each declare all 25 outcomes and remap hidden lanes
after exact-mode execution. RGBA's AVX2 buffer route stays specialized.
Empty/trailing storage, cancellation receipts, wrapper fault order, and full
mode/architecture parity remain open. Source review only; no verification ran.

2026-11-12 Crop family-entry review: inspected the complete
`pool_cpu/ops/crop/ops.rs`, crop helpers, representative I;16 vector and
CoreOnly typed owners, public coordinate normalization, and registry
registration. Crop declares all 25 identities and routes each to same-mode
native output. I;16 variants have typed architecture row-copy paths; internal
high-depth carriers retain their sample types in Default. Out-of-bounds public
requests compose crop, canvas, and Paste while restoring the pinned backend.
Empty results, overflow/error order, trailing storage, CropBorder composition,
and complete mode/architecture receipts remain open. Source review only; no
verification ran.

2026-11-12 Point output-mode strategies: implemented Pillow's documented
`L/P -> 1` and `I -> L` routes as the `PointModeOutput` operation. The L/P
strategies use the injected CPU architecture context and the existing native
byte LUT kernels while constructing the declared One output; I uses native
signed samples, clamps LUT indices to `0..=65535`, and writes a typed L result.
Explicit-mode LUT validation uses the requested output mode's band count;
`I -> L` remains the 65,536-entry exception. Float output mode F always
requires 256 entries before the core rejects an unsupported source type.
The Rust API receives `Mode`; the Python adapter only translates the optional
Pillow mode name and normalizes its LUT values. Point results retain Pillow
`_new` wrapper metadata, with palettes retained only for P/PA outputs and the
encoded source format cleared. Source review is recorded on the changed
functions. Parity and benchmark markers remain pending, and Stage 5 source
closure still gates all validation. TODO(pillow-rs#3ptb): freeze and run
focused Point output-mode/metadata/error-order parity and public-call
benchmark cases after Stage 5.

2026-11-23 Point mode-output source follow-through: the direct Rust route now
materializes the source and validates its declared exact Mode before resolving
the CPU backend route. The mode-owned strategy matrix remains L/P→One on
Default/Sse2/Neon/Wasm SIMD128 and I→L on Default. The output Mode is checked
against the declared fixed transition before constructing the result Image.
Source review only; mode/LUT fault order, outputs, route receipts, and latency
remain pending under `pillow-rs#3ptb`.

2026-11-23 Point LUT length correction: Pillow 13's native parser chooses an
F-target table length of 256, an I-to-L table length of 65,536, and otherwise
256 entries per requested output band before source-mode kernel admission.
Rust's explicit-mode validator now uses target bands, and direct float-Point
validates the fixed F-target length before rejecting an unsupported source
mode. The documented L/P-to-One and I-to-L execution strategies are unchanged.
Source-only; exact invalid-length/error precedence, outputs, route receipts,
and latency remain pending in `pillow-rs#3ptb`.

2026-11-23 PointScalar source follow-through: direct Rust callable and affine
routes now materialize and validate the exact `DynamicImage` identity before
resolving backend policy or selecting support. The serial strategy preparer
receives the owned native raster, avoiding a second materialization. I32, F32,
native I16, and Rust-only I16L/I16B/I16N remain separate exact-mode branches;
no conversion path was introduced. Source review only; output, callback/decode
fault ordering, and route receipts remain pending under `pillow-rs#3ptb`.

2026-11-12 Transform family-entry review: inspected the complete
`pool_cpu/ops/transform/ops.rs` and shared request/architecture preparation.
Transform declares all 25 outcomes: 20 Pillow modes route to exact owners and
five internal high-depth multichannel modes retain explicit
`NotImplementedError` rejection. Mode/method/filter/coefficient/output-size
facts select Default or the declared AVX2/AVX/SSE2/NEON/WASM SIMD128 path;
MESH remains Default. Prepared execution composes hidden lanes after the
selected kernel, including direct-destination and mesh paths. Wrapper input
order, empty-mesh palette retention, sampler parity, fallback receipts, and
complete mode/architecture parity remain open. Source review only; no
verification ran.

2026-11-12 lazy Mode resolver source crosswalk: enumerated all 87
`PipelineOp` variants and followed the exhaustive `operation_id()` match to 75
operation identities; `PipelineOp::support()` delegates directly to those
family-owned declarations. The grouped forms are Convert/ConvertMatrix,
Equalize/EqualizeMasked, three Pad forms, two Expand forms, and ten Draw forms.
The normal `Image::push_op` route creates its output through
`pipeline_output_metadata -> pipeline_output_mode`; prefix reuse is limited to
same-mode preserving chains. `push_mode_changing_op` is used by Color3DLut and
seeds the cache from its typed target. The other direct cache-seeding
constructors are the prepared Convert-to-P wrapper, Quantize-to-P output, and
palette-only metadata layer; each stores an exact P/current `Mode` with no
queued operation. A mode query plans when the cache has no known output
identity, resolving the upstream image first. Lazy materialization verifies
that the producer's exact `DynamicImage` variant matches a declared identity.
PA expansion is always RGBA, while P expansion requires an attached palette
and resolves to RGB/RGBA from its alpha table.
The declared target rules are Convert/ConvertMatrix, Merge, and Color3DLut;
PutData is the only `ByPayload` pipeline operation; and no raster-producing
PipelineOp maps to `NoRasterResult`. The only remaining materialization
boundaries are unresolved upstream pipelines, palette-dependent P operations
without an attached palette, rejected raw descriptors without a declared
output, and PutData payloads outside its 16-entry table. The defensive
target/payload mismatch branches have no active mapping in the reviewed
declarations. The source crosswalk is complete; exact mode, fault-order, and
fallback-receipt parity remains gated on full Stage 5 closure. The GPU graph
planner is a separate caller of `known_pipeline_op_mode`; its current
string-to-Mode conversion and missing palette-expansion input stay assigned to
the later GPU stage. Updated the source TODO and inventory. No runtime
validation ran.

2026-11-12 CPU architecture selector source review: inspected
`Architecture` naming/folder policy, process-wide capability capture and host
preference, `ExecutionContext` routing and lower-architecture eligibility,
`ImplementationSupport` construction, and `ModeSupport` assessment/selection.
The selector prioritizes the requested architecture, then considers only
declared architectures permitted by the injected context; it checks build
availability, captured hardware requirements, and request guards before
selecting a strategy. A selected strategy retains its guard, while fallback
records the first applicable reason. Source VERIFY markers now record this
review for the 19 relevant functions. No behavior changed; exact/lower,
hardware/build-unavailable, guard, and Default-fallback receipts remain TODO
for Stage 7. No build, formatter, checkstyle, lint, test, parity, benchmark,
or coverage command ran.


2026-11-12 Quantize supplied-palette source follow-up: traced the Python
validation order through the PyO3 entry point, typed Quantize input, request
guard, exact-mode selector, and palette mapper. Visible wrapper mode controls
public palette admission; actual DynamicImage identity controls source
strategy selection, with independent support-contract checks for each. The
PyO3 adapter now accepts foreign ImagingCore-compatible P carriers by reading
their attached palette mode and packed table, then stores that metadata in a
small Rust-owned P image passed to the existing mapper. No foreign pixels are
materialized and no second quantizer was added. External-carrier output/fault
parity, palette alpha, and subclass mode overrides remain pending; no
validation command ran.

At the time of this source checkpoint, a foreign P ImagingCore whose internal
palette pointer was absent still needed Pillow 13.0's source-mode-dependent
grayscale/WEB fallback. The 2026-11-12 follow-up below resolves that source
route using the actual materialized DynamicImage Mode. Its output, fault, and
fallback-receipt parity remain pending. Attached zero-entry palettes remain
distinct and reject through the typed Quantize input contract as
`ValueError("empty palette")`.

At that source checkpoint, Pillow 13.0's `topalette()` behavior was recorded:
an attached zero-entry palette raises `ValueError("empty palette")` before
mapping pixels. The existing exact-mode mapper applies this guard to native
and adapted P carriers. The native typed-carrier guard and supplied-palette
mapping passed the five focused parity cases listed in the inventory. The
foreign adapter and no-palette fallback source was added in the later
2026-11-12 follow-up below; runtime parity remains unverified.

2026-11-12 Quantize supplied-palette push checkpoint: `make quality` passed
after a formatting correction; `make repo-map-update`,
`make implementation-structure-audit implementation-structure-inventory
repo-map-check` passed with zero unclassified paths. The release
`test-api` parity build succeeded. Run
`quantize-supplied-palette-20261112-cpu-default` passed all five selected
supplied-palette cases on CPU Default, including the empty-palette and
unsupported-RGBA fault paths. No benchmark or full CPU parity matrix ran;
those gates remain after Stage 5 source closure. The foreign ImagingCore
carrier and absent-palette fallback remain TODO under `pillow-rs#3ptb`.

2026-11-12 Quantize missing-palette source follow-up: the foreign ImagingCore adapter now distinguishes the C `ValueError("image has no palette")` response from an attached zero-entry table. The exact-mode CPU helper creates the same grayscale table for L sources or WEB table for supported multiband sources from the materialized DynamicImage identity, then reuses the existing palette mapper. This corresponds to Pillow 13.0's null-pointer and zero-entry `topalette()` branches ([Convert.c](https://github.com/python-pillow/Pillow/blob/13.0.0/src/libImaging/Convert.c#L1056-L1087)); `ImagingCore.getpalettemode()` reports a null pointer as `ValueError("image has no palette")` ([`_imaging.c` error string](https://github.com/python-pillow/Pillow/blob/13.0.0/src/_imaging.c#L352), [accessor](https://github.com/python-pillow/Pillow/blob/13.0.0/src/_imaging.c#L1356-L1364)); the Python wrapper still asserts `palette.palette is not None` after conversion ([Image.py](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L1326-L1338)). Source review only; foreign/core-only output and fault parity, including post-conversion assertion order and fallback receipts, remain TODO under `pillow-rs#3ptb`. No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage command ran.

2026-11-12 Quantize missing-palette parity checkpoint: regenerated and
validated the current 176,655-case input corpus. The target compiled with
`make build-parity`; focused CPU Default run
`migration-parity-8c3385ccbd7043bcbc13eebfa0daf52f` passed all six selected
Quantize cases, including the foreign P carrier with no palette table and the
attached-empty rejection. This verifies the RGB WEB-palette fallback only.
L-source fallback, other foreign-carrier fault/metadata order, fallback
receipts, architecture selectors, full CPU parity, and benchmarks remain
pending. The static source crosswalk maps 217 public operations, 216 Python
bindings, four target-only contracts (one direct PyO3), 87 pipeline variants,
and all recorded Python/PyO3 call routes; it has zero unresolved module edges
or public manifest references. Seventeen active registry-dispatched
`prepare_pipeline` symbols still lack a direct public graph path, while all
280 generic source-unreached candidates remain under inert `pool_cpu/legacy`.
These static graph limits do not establish runtime reachability. Stage 5
source closure and full CPU acceptance remain open under `pillow-rs#3ptb`.

2026-11-13 direct eager-route/context source audit: reviewed every maintained
`cpu_arch_execution_route*` call site outside `pool_cpu/legacy`, plus the
draw-owned context adapter and prepared GetProjection PutPixel terminal. The
review covered pixel and data reads/writes, channel identity/eager-child paths,
statistics and analysis, projection, fusion admission, conversion, quantize,
CPU-only filters, split, drawing, flood fill, and Transform. Mode selectors use
the exact `DynamicImage::mode()` for materialized algorithms; pre-materialized
mutation/Transform entry points carry the exact typed wrapper Mode and then
validate the materialized raster before executing. The same `ExecutionContext`
flows into family selection/preparation, is captured once in callback-oriented
PutData and Transform paths, and is reused for each MESH record. Copy-only
GetChannel/Split routes intentionally do not choose a pixel architecture;
their non-identity ExtractBand children defer to ordinary pipeline dispatch.
The GetProjection PutPixel fusion path captures a prepared context and passes
it to its CPU Default/architecture terminal selector. Existing GPU-only
terminal eligibility/context TODOs remain assigned to later GPU work.

Added source-only VERIFY markers to the reviewed route callers and helpers. The
direct route helper now has a Stage 7 TODO for receipts across the frozen CPU
mode/architecture matrix. No route caller was left unreviewed; existing
per-path parity and backend TODOs remain attached to their operation owners.
This audit does not establish output, fault-order, fallback-receipt, or latency
parity. Stage 5 still has family/helper/call-graph source review and exact
function/file-size proof outstanding. `make docs-lint`, `make fmt`, and
`make clippy` passed on 2026-11-13; Clippy reported the repository's existing
warning diagnostics. No tests, parity, benchmark, or coverage command ran.

2026-11-13 CPU preparation-entry source follow-up: inspected the previously
unmarked Composite, Brightness/ColorSaturation, Contrast/Sharpness, Filter, and
Convert preparation paths against their exact-mode support assessment and
injected `ExecutionContext`. Filter carries typed request parameters into its
declared facts; architecture-specific rank-window facts are only an eligibility
guard, and `PreparedFilter` retains the same context. Composite validates the
operation variant and primary-mode contract before materializing its secondary
image, then composes the exact LA or RGBA strategy. Convert preserves its typed
target and separate direct/matrix declarations; its registry alias points to
the same prepared helper. Enhancement's macro-generated mode selectors match
the `DynamicImage` variants and call exact mode owners; Contrast records its
conversion-first routes, while Sharpness has exhaustive exact-mode support.
Those prepared routes keep the injected context through their mode strategies.
All 19 `prepare_pipeline` definitions under `pool_cpu/ops` and the reviewed
Contrast/Sharpness wrappers now carry adjacent source-review markers. The 17
static crosswalk route candidates remain candidates rather than dead-code
findings; registry ownership is explicit, while compiler-backed reachability
remains a later gate. An exact workspace-root file scan covered 5,495 tracked
Rust files, including tests, examples, benches, build scripts, and the legacy
archives: the largest is `pipeline.rs` at 982 physical lines, and none exceed
1,000. The preceding `make clippy` report counted zero functions over 100
Clippy body lines; this follow-up changes only comments and docs, which do not
add counted body lines. The automated size gate still reruns after source
closure. VERIFY markers record source review only; parity and benchmark
evidence are still pending. No build, formatting, checkstyle, lint, test,
parity, benchmark, or coverage command ran for this source-only follow-up.

2026-11-13 parity missing-implementation classification source audit: the
parity runner recognizes a target `NotImplementedError` as pending only when
the reference step succeeded and every earlier observed step has the same
status and value/error under the declared comparison policy. The target error
remains in the comparison record. A target-only expected-output workflow that
reaches an unimplemented route is also `not_run`; ordinary completed-result,
wrong-error, or wrong-rejection differences remain failures. Updated VERIFY
dates on the classifier and added a source marker to the comparison-record
function. This confirms runner source behavior only; its focused contracts and
the full CPU parity campaign remain pending. No validation command ran.

2026-11-13 F32 rank comparator source follow-up: source-reviewed the exact-mode
support, content classification, request guard, Default partition selector,
and vector order-statistic path against the preserved CPU/SIMD implementations.
Expanded the public workflows for MaxFilter, MinFilter, MedianFilter, and
RankFilter over NaN payloads, mixed signed zero, and vector-compatible
infinities from size 3 to sizes 3, 5, 9, and 11. RankFilter exercises the
minimum, median, and maximum rank at sizes 5, 9, and 11. Non-Default requests
fall back to Default for NaN, mixed signed-zero, and sizes above the vector
limit; infinity-only ordering stays eligible for the vector path. The 66-case
selection digest is `a45192ea9a16a1488a8d23ca9a457647ac10bd45b688e4ce6a7d5f7736ef591e`.
All cases passed under each of the nine declared CPU architecture selectors
(594 executions, zero failures or infrastructure errors). Neon executed the
vector path for 16 cases and selected Default for the other 50. On this arm64
host, x86 selectors fell back to Default with `unavailable_hardware`, so
native x86 execution is still unverified. The selector run IDs are:

- Default: `migration-parity-26066f371e8c4e0a80be534279849518`
- Neon: `migration-parity-de042d8a4480460c8d2be1f284c38af6`
- SSE2: `migration-parity-a306f39f668945e6a7d206a3c97ed39c`
- SSE3: `migration-parity-3958196c6ac645f2889ff3e45d1b6246`
- SSSE3: `migration-parity-aeda966f29374d7b9d4ac5df74010d3a`
- AVX: `migration-parity-ab34044f3e2d415d9fc51a1ab1d6bd3c`
- AVX2: `migration-parity-eaf92143cabd483397738af668b4abe6`
- AVX+FMA: `migration-parity-b01318cce2544043827ef86b81c8f8b3`
- AVX2+FMA: `migration-parity-8a9ee62b0c684bf783b388c8e7af7d73`

The complete receipts are under
`build/migration-parity/cpu-architecture/f32-rank-window-matrix-20261113/`.
VERIFY markers now reference the actual scoped receipts. No benchmark or
coverage ran. The native x86 run and broader source-to-requirement mapping
remain open before Stage 6. Pre-push checks `make migration-parity-inputs-check`
and `make fmt clippy docs-lint` passed; Clippy reported the repository's
existing warning diagnostics.

2026-11-13 Chops support-declaration source review: traced all 25 exact-mode
owners from `support_for_mode` to their `ModeSupport` declarations. Their
architecture lists and Default-only routes compose with the same operation
contracts used by request validation and prepared selection. The review
confirmed the 1/2/3/4-byte Chops mode groups, mode-specific invert/logical
routes, native I;16 Offset architecture declarations, and CoreOnly high-depth
extensions. The LAB-secondary guard keeps the current signed-chroma-aware
three-band formula on Default; in-place execution remains limited to admitted
vector Add/Sub modes, and Multiply/Screen fusion independently assesses both
operation declarations and requires the same selected architecture. Added
source-only VERIFY markers to the 25 mode `support()` functions and refreshed
the reviewed Chops orchestration markers. This closes the support-declaration
review only; per-mode output/fault paths, kernel and sidecar review, exact
architecture receipts, parity, and performance remain open. No build,
formatter, checkstyle, lint, test, parity, benchmark, or coverage command ran.




2026-11-13 Chops Default strategy review: manually reviewed all 25 mode-owned
`default/ops.rs` files and their 93 functions. Each entry validates the exact
primary `DynamicImage` variant and returns the matching mode identity; binary
secondary variants match the declared 1/2/3/4-byte storage groups; RGB,
YCbCr, and HSV use the three-band owner, while LAB primary uses its
signed-chroma-aware owner. I;16 modes pass native typed buffers to Offset and
Invert helpers, and CoreOnly high-depth extensions are marked
`NO_PILLOW_SUPPORT`. Fused default entrypoints keep typed pixel carriers. Added
source VERIFY markers to all 93 reviewed functions. This closes the mode-owned
Default entrypoint review only: shared output/hidden-lane behavior and
lazy-fault order remain open, as do architecture execution receipts, parity,
and benchmark evidence. All reviewed files are below the file-size limit and
the routing functions remain below the function-size limit. No build,
formatter, checkstyle, lint, test, parity, benchmark, or coverage command ran.


2026-11-13 Chops hidden-lane source review: traced the shared `HiddenPixelLanes`
constant/per-pixel representation, clipping and pixel remapping, mode-derived
fallbacks, and buffer extent checks through Chops' post-kernel output composer.
Binary Chops applies the same byte operation to retained omitted lanes; missing
producer sidecars resolve through the canonical mode fallback. LA/La/PA derive
the repeated lane from each logical luma/index sample; RGB/YCbCr/LAB/HSV keep
the canonical fourth byte without allocating when it remains 255. Empty clipped
outputs retain no sidecar, and non-empty output sidecars are checked against
the output extent before attachment. Added source VERIFY markers to the
reviewed shared helper and storage functions. This is source review only: raw
byte follow-on Paste parity across producers and declared architectures remains
open, as do direct/in-place output receipts, lazy fault order, and benchmarks.
No validation command ran.


2026-11-13 Chops source-closure checkpoint: the 25 exact-mode support
declarations, Default owner entrypoints (93 functions), architecture dispatch
leaves, shared byte/typed kernels, LAB three-band path, fusion/in-place
selection, hidden-lane composition, and lazy secondary preparation have now
received source review and function VERIFY markers where reviewed. Remaining
Chops work is runtime-only: frozen output/error-order cases, selected and
fallback receipts across every declared architecture, hidden-lane follow-on
Paste parity, and performance evidence. The Chops source review is complete;
CPU Stage 5 remains open for other operations/helpers and the full manual size
review. No build, formatter, checkstyle, lint, test, parity, benchmark, or
coverage command ran.


2026-11-13 maintained CPU scheduling source audit: searched active
`pool_cpu` Rust files while excluding the inert `legacy` tree for Rayon APIs,
parallel-iterator traits, parallel scheduling methods, and `parallel` feature
gates. No executable-code references remain. TODOs that mention moving archived
Rayon paths describe the later Parallel CPU stage; they do not represent active
CPU scheduling. This closes the no-Rayon source criterion only; it does not
prove module reachability, compilation, parity, or performance. No validation
command ran.


2026-11-13 Composite orchestration source closure: reviewed all 31 function
definitions in `pool_cpu/ops/composite/ops.rs`, including exact-mode support,
architecture declarations, prepared-child composition, error-order-sensitive
pipeline preparation, and mode execution routing. Added source VERIFY markers
to every function in that entrypoint; parity and benchmark remain pending.
Moved `MergeLayout` into `composite/helpers/merge.rs` and re-exported it from
the family entrypoint so helper code no longer depends on `ops.rs`. Moved
`CompositeLayout` into `composite/helpers/composite/mod.rs`, and moved the
shared support assessment and secondary-image facts into
`composite/helpers/strategy.rs`. These are placement-only changes; mode leaf
imports and algorithm selection remain unchanged. The entrypoint is now 960
physical lines. Runtime output and mode identity, lazy fault order, hidden
lanes, architecture/fallback receipts, parity, and latency remain open. No
build, formatter, checkstyle, lint, test, parity, benchmark, or coverage
command ran.


2026-11-13 batch executor source review: reviewed the draw-run boundary,
AlphaComposite+Mirror fusion admission and serial/vector kernels, Multiply+Screen
fusion admission and shared arithmetic, RGB PutPixel+Thumbnail preparation
through its native reduction kernel, transpose composition, point-LUT
composition, PutPixel batch dispatch, and fused-operation telemetry. All 22
`batch_execution.rs` functions now have source VERIFY markers; reviewed RGB
fusion leaves and preparation helpers are tagged as well. Parity and benchmark
remain pending. Keep the batch TODO open for output/mode identity, fault order,
selection/fallback receipts, and failure telemetry. In particular,
unprepared failures are currently labeled `cpu` without a selected strategy,
and RGB PutPixel+Thumbnail preparation errors return before per-operation
telemetry begins. No validation command ran.


2026-11-13 GetPixel helper source review: traced the 25-mode support and
execution dispatch through `Image::getpixel_formatted`, the fixed RGBA8
accessor, and the loaded-RGB binding fast path. Reviewed all 19 functions in
`analysis/helpers/pixel_access.rs`: Pillow scalar/tuple shapes, LAB signed
offsets, native I;16/I/F samples, CoreOnly high-depth component values, and
telemetry byte counts match their declared types. Public accessors check
coordinates before entering these private readers; scalar carrier helpers
still validate checked offsets and extents. Added source VERIFY markers.
Parity, output receipt, and performance evidence remain pending. No
validation command ran.


2026-11-13 Histogram helper source review: traced the exact-mode support and
Default dispatch for all 25 identities, mask validation, optional I/F extrema,
and native byte/u16/f32 sample scans. Added source VERIFY markers to all 20
function definitions in `analysis/helpers/histogram_execution.rs` and all 17
in `analysis/helpers/histogram_samples.rs`. The native high-depth bins avoid
projected rasters; Pillow I;16 endian byte-prefix behavior remains a separate
mode-specific path. The Histogram mask contract now explicitly lists the 23
source modes that accept masks and declares the existing `image has wrong
mode` rejection for I/F sources. Validation keeps size mismatch and invalid
mask-mode errors ahead of source-mode rejection, then preserves operation-mode
rejection. This is source-only; parity and benchmark remain pending. Existing
CoreOnly, I;16, I32, and F32 parity TODOs remain open. No validation command
ran.


2026-11-13 Hidden-lane storage source review: traced the mode-free
`HiddenPixelLanes` payload through its compact carrier, typed-buffer extent
checks, `DynamicImage` mode groups, same-mode remapping, raw decode attachment,
and the active CPU consumers that materialize, update, or invalidate omitted
physical bytes. `DynamicImage::replace_hidden_pixel_lanes` now checks the
sidecar lane count against the exact mode before delegating extent validation
to `ImageBuffer`; invalid sidecars can no longer be attached to a different
compact carrier layout. The shared byte helper's canonical fallbacks and lazy
per-pixel materialization were reviewed. Source only: hidden-byte output
parity, mutation invalidation, and all producer-to-Paste follow-on receipts
remain pending. No build, formatter, checkstyle, lint, test, parity, benchmark,
or coverage command ran.


2026-11-13 ImageBuffer checked-access source review: `from_raw` permits trailing
samples, so a slice-bound-only lookup could return an off-image row from both
checked accessors; the mutable form could also invalidate hidden-lane state
while mutating that trailing storage. Read and mutable checked lookups now
reject either coordinate outside the declared image dimensions before offset
calculation. Reviewed the buffer's hidden-lane constructors, mutable escapes,
clone paths, and pixel/row iteration bounds and marked those functions
source-reviewed. Parity and benchmark remain pending. No validation command ran.


2026-11-13 GetData execution source review: traced mode support queries, typed
selection validation, exhaustive `DynamicImage` dispatch, full-buffer reads,
allocated and caller-owned band writes, palette-index views, and native I;16/I/F
sample access. Added source VERIFY markers to all 14 functions in
`analysis/helpers/get_data_execution.rs`. Output, error-order, exact architecture
receipt, and latency evidence remain pending. The hard-coded GPU terminal
eligibility branch remains an explicit later-GPU TODO. No validation command ran.


2026-11-13 DynamicImage mutation source review: traced the generic pixel writer
through exact mode groups and confirmed compact-carrier writes use the
lane-preserving buffer method, while general mutable buffer access invalidates
its sidecar. Reviewed `crop_imm`/`crop_buffer` mode dispatch and sidecar remap,
plus overlapping `copy_within`, which computes retained physical-lane output
from the original sidecar before writing logical pixels and validates the
mode-specific lane count and image extent. Generated DynamicImage clone methods
preserve sidecars through ImageBuffer Clone. Added source VERIFY markers to
these paths. Parity and benchmark remain pending. No validation command ran.


2026-11-13 GetBBox execution source review: traced the prepared operation's
exact-mode context selection, retained mode/architecture choice, empty-image
return, and exhaustive 25-variant dispatch. Added source VERIFY markers to all
three functions in `analysis/helpers/bbox_execution.rs`. Alpha-only output,
selection/fallback receipts, parity, and latency remain pending under the
existing GetBBox TODO. No validation command ran.


2026-11-13 byte GetExtrema execution source review: validated exact-mode
selection and Default-only execution, the exhaustive `DynamicImage` dispatcher,
the generated per-mode owners' operation/mode guard, and the matching Pillow vs
CoreOnly support declaration. Added source VERIFY markers to all six prepared
executor, dispatcher, support, and generated-owner functions. All mode output,
error, route-receipt, parity, and latency evidence remains pending. No
validation command ran.


2026-11-13 typed GetExtrema execution source review: traced its Pillow,
CoreOnly, and rejected mode declarations; context-selected Default route;
empty-image result shapes; and exact DynamicImage dispatch, including the
I;16L/B/N error contract. Added source VERIFY markers to all four functions in
`analysis/helpers/extrema_execution.rs`. Mode-result, fault-order, receipt,
parity, and latency evidence remain pending. No validation command ran.


2026-11-13 GetColors and Stat execution source review: traced GetColors'
Default-only selection and exhaustive exact-mode dispatch, plus Stat's
Pillow/CoreOnly support builder, I/F scalar path, same-context Histogram child,
and masked formatted-result route. Added source VERIFY markers to all five
GetColors direct/generated functions and all ten Stat direct/generated
functions. Result/fault order, child selection receipts, parity, and latency
remain pending. No validation command ran.


2026-11-13 GetColors native counter source review: traced byte histogram and
Pillow slot/probe ordering, the exact RGB grayscale shortcut, I/F scalar
sample identity, repeated scalar run handling, CoreOnly typed u16/f32 keys,
and the shared output formatting paths. Added source VERIFY markers to all 15
functions in `analysis/helpers/getcolors.rs`, all 13 in
`image/color_count/pillow_table.rs`, and all eight in
`image/color_count/specialized.rs`. Output/fault parity for every declared
mode and latency remain pending; no validation command ran.

2026-11-13 GetColors mode-owner source review: reviewed all 21 active exact-mode
Default leaves. One, L, and P use the native luma counter; RGB uses its native RGB
shortcut; other supported byte layouts use direct band counting; I and F retain
scalar sample counting; and the CoreOnly high-depth modes use native u16/f32
counting. I;16, I;16L, I;16B, and I;16N remain explicit operation rejections
with no implementation folder. Added source VERIFY markers to all 21 owner
functions. Per-mode output/fault and selection/fallback receipts, parity, and
latency remain pending. No build, formatting, checkstyle, lint, test, parity,
benchmark, or coverage command ran.

2026-11-13 Thumbnail source review: traced all remaining mode-policy helpers,
Default and AVX-FMA I32 kernels, and the streamed I32 reduction/resample path
against the archived implementation. The mode strategy keeps `1` and `P` on
nearest, routes straight `LA`/`RGBA` through the alpha-aware ResizeBoxed gap
path, and allows premultiplied `La` plus other modes to use native reducing-gap
reduction. Pillow 13.0 applies reducing-gap to straight `LA`/`RGBA` by resizing
their premultiplied forms; the active route reflects that contract
([Image.py](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L2212-L2217),
[13.0 release notes](https://github.com/python-pillow/Pillow/blob/13.0.0/docs/releasenotes/13.0.0.rst#L336-L338)).
The streamed I32 path preserves wrapped quartet sums, per-pass int32 rounding,
and architecture-selected multiply-add behavior while avoiding a full reduced
frame. Added source VERIFY markers to the remaining mode-policy, I32 strategy,
kernel, and streaming functions. No behavior changed. PA route receipts, all
mode output/fault receipts, parity, and latency remain pending. No build,
formatting, checkstyle, lint, test, parity, benchmark, or coverage command ran.

2026-11-13 Histogram mode-owner source review: traced all 27 functions in the
25 exact-mode Default leaves to the already reviewed native sample scanners.
Byte modes use direct per-band bins, with the LA versus La/PA masked
second-band policy selected explicitly at each mode owner. I;16, I;16L, I;16B,
and I;16N select native, little-endian, big-endian, and native-order byte
prefix scans respectively. I/F retain scalar sample bins and their extrema
variants; I/F masks remain rejected at the operation contract. CoreOnly
LumaA16/RGB16/RGBA16 use native u16 scans, RGB32F uses the native RGB plus
opaque-alpha result, and RGBA32F uses native f32 scans, all with
`NO_PILLOW_SUPPORT`. Added source VERIFY markers to all 27 leaf functions.
Every output, mask/fault, hidden-lane, mode and selection receipt, parity, and
latency remains pending. No validation command ran.

2026-11-13 GetData mode-owner source review: reviewed all 61 functions in the
25 exact-mode Default leaves against the prepared executor and native helper
contracts. The leaves preserve raw byte layouts, special LAB signed a/b band
reads, zero-copy One/L/P planes and the owned P index plane, typed I;16 sample
access, I/F scalar samples, and CoreOnly native-sample adapters. Every selected
mode's read and band path is explicit; CoreOnly modes remain target-only. Added
source VERIFY markers to the leaf functions. Typed output, band/error order,
mode and selection receipts, parity, and latency remain pending. No validation
command ran.

2026-11-13 GetData mode-strategy source review: reviewed the 25 exact-mode
`execute_get_data` owners and all eleven additional view/scalar functions for
One/L/P, palette indices, I;16 integer samples, and I/F scalar samples. Every
owner validates its exact operation/mode selection, executes only the declared
Default implementation, and reports a missing architecture explicitly. Palette
index views and caller-owned writes retain the GetData or GetDataBandInto
selection they received. These modes have no GetData architecture-specific
leaf; SIMD terminal eligibility remains the existing deferred ExtractBand
route, and GPU terminal eligibility remains a later-GPU TODO. Added or
confirmed source VERIFY markers. Output, band/error order, selection/fallback
receipts, parity, and latency remain pending. No validation command ran.

2026-11-13 GetPixel mode-owner source review: reviewed all 26 functions in the
25 exact-mode Default leaves. Single-band `1`/L/P modes return native scalar
samples; byte component modes use their stored channels; LAB uses its signed
a/b mapping; I;16 identities retain typed u16 samples; I/F return native scalar
values; and internal high-depth modes use native u16/f32 samples with
`NO_PILLOW_SUPPORT`. RGB's fixed three-byte tuple path remains a separate
allocation-free leaf. The leaves compose with the previously reviewed
coordinate, formatting, and output-shape helpers without converting source
modes. Added source VERIFY markers. Mode output, coordinate/error order,
selection receipts, parity, and latency remain pending. No validation command
ran.

2026-11-13 GetPixel mode-strategy source review: reviewed the 25 exact-mode
`execute_get_pixel` owners and RGB's fixed-tuple owner. Each validates the
`GetPixel` selection against its concrete Mode, then forwards only to that
mode's native Default leaf; RGB tuple reads use the separate fixed-size leaf.
The mode dispatcher remains exhaustive and directly matches each
`DynamicImage` variant. Added source VERIFY markers. Coordinate/error ordering,
selection receipts, output parity, and latency remain pending. No validation
command ran.

2026-11-13 GetProjection source review: reviewed all 25 exact-mode support
owners and the prepared direct/fused routes. Twenty Pillow modes declare a
serial Default strategy; five internal high-depth modes are explicitly
CoreOnly. RGB8 and CMYK8 additionally declare SSE2, Neon, and WASM SIMD128,
with all other architecture requests falling back through the selected
Default strategy. The prepared selector uses the exact `DynamicImage::mode()`;
execution retains that mode and matches the concrete raster variant. Native
Default scans preserve the LAB `(0, 128, 128)` neutral point, I/F stored-word
semantics, and direct native-byte checks for other carriers. RGB and CMYK SIMD
loops and CMYK positive-write fusion were compared with the archived source;
the 48-byte RGB blocks, grouped CMYK detection, scalar edges, and known-zero
write eligibility are preserved. Added source VERIFY markers across support,
mode, architecture, kernel, and fused-owner functions. Axis output, operation
receipts, empty/tail cases, every mode × architecture result, and latency remain
pending; x86 kernel execution is unavailable on the current ARM host. No
validation command ran.

2026-11-13 Resize reducing-gap lane source follow-up: the prepared straight-
alpha `ResizeBoxed` branch returned immediately after constructing its typed
LA/RGBA output, bypassing the shared `hidden_lanes::finish` used by ordinary
and tall-boxed Resize. LA stores two logical samples in a four-byte physical
pixel, so the output lost the two omitted bytes required by later public raw-
pixel operations. The branch now runs the shared lane composer with the same
filter and validated bounds before returning; filtered LA derives omitted
bytes from final luma. Updated VERIFY source dates on the touched executor and
lane composer. This closes only this source finding; broader Resize source
review and all runtime parity/receipt/latency evidence remain pending. No build,
formatter, checkstyle, lint, test, parity, benchmark, or coverage command ran.

2026-11-13 Resize strategy source follow-up: reviewed the remaining exact-mode
Resize owner dispatches, shared byte/alpha/typed sample kernels, coefficient
and boxed-crop planning, native nearest and Luma8 vector paths, and the native-
byte vector planner/convolution/boxed and bounded-i32 routes. Added source
VERIFY markers to the reviewed functions. The I;16 vector helper now returns an
empty typed buffer when either requested output axis is zero; its strategy
guard rejects zero-sized output and empty source layouts, so those public
requests must use Default. TODO(pillow-rs#3ptb): verify empty I;16 source and
zero-axis output fallback receipts across CPU selectors. This review does not
close Resize source review: architecture leaves and remaining operation/helper
closure, parity receipts, and latency evidence are still pending. No build,
formatter, checkstyle, lint, test, parity, benchmark, or coverage command ran.

2026-11-13 I32/F32 Resize strategy source follow-up: traced ImageI32's native
nearest and ordered f64 Default paths, x86 AVX/FMA token injection, and the
NEON checked tap plan with its packed and portable fallback blocks. Reviewed
all NEON raw-load bounds, alignment, and immutable-slice assumptions; no unsafe
defect was found. Traced ImageF32's exact carrier, identity and constant paths,
ordered filtered passes, AVX/FMA selection, boxed staging, and tall-image
composition. Added source VERIFY markers to reviewed functions. The F32
x86/Wasm product-block-width TODO remains open; all I32/F32 parity, fallback
receipts, and latency are pending. No build, formatter, checkstyle, lint, test,
parity, benchmark, or coverage command ran.

2026-11-13 Resize strategy-subtree source review: reviewed the remaining Default and
architecture leaves for Luma8, One, Palette, CMYK, HSV, LA/La, LAB, LumaA,
PA, RGB/RGBa/RGBA/RGBX, YCbCr, and typed 16-bit/float modes. This includes
the narrow fixed-point Luma8/CMYK/HSV kernels, vector wrappers, RGB sparse-row
and RGBX output-window paths, and hidden-lane composition. The earlier
`*_impl.rs` audit had missed non-`*_impl.rs` kernels and helpers; the full
Resize subtree function audit now finds no unmarked function. Added current
VERIFY markers to the reviewed leaves. This closes source review of the
maintained `compute/pool_cpu/ops/resize/**` strategy subtree only. The older
whole-image and boxed dispatcher files named in the inventory are absent, and
source search found no active calls to those names outside `legacy`. Boxed
validation/error ordering remains open at the public Resize boundary; the
operation TODO now names it. Output/error parity, zero/empty fallback receipts,
architecture receipts, and per-operation latency remain pending. TODOs remain for empty/zero-axis
I;16 fallback receipts and F32 x86/Wasm product block width. Keep the parity
input-base work as a prototype; the shared parity and fault-contract standard
will be designed later. No build, formatter, checkstyle, lint, test, parity,
benchmark, or coverage command ran.

2026-11-13 Resize public-boundary source review correction: the Python
`Image.resize` facade calls `_resize_filter_code` and compares `reducing_gap`
before default-box resolution and `tuple(size)`, matching Pillow 13.0's
[validation order](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L2173-L2220).
Only after the same-size check and `self.load()` does it call the private PyO3
resize method, so its typed conversions do not precede the public filter/gap
checks. Removed the inaccurate low-level TODO. The wrapper now reads the current
palette dirty state only after the nonidentity comparison, so callbacks during
size normalization cannot leave an earlier palette snapshot in use. The
same-size test now preserves Pillow's short-circuit property-access order and
starred tuple expansion for custom iterable sizes, then returns through the
overridable `self.copy()` path before palette/mode checks; `copy()` calls
`load()`, which commits a dirty palette. Reentrant custom
size/box/gap callbacks and overridden mode-property behavior remain under the
Python facade's TODO; I/O/decode, bounds, and prepared-execution fault order
still need parity review. No validation command ran.

2026-11-13 CPU operation-source crosswalk follow-up: manually traced the eight
active `pool_cpu` files that the generated graph marked as registry-reached but
without a complete manifest public-operation path. The seven non-Draw owners
are called by their typed core operation paths and registrations; Draw's ten
queued geometry operations enter through the registry, while Bitmap, native
Text, and FloodFill call the same owner directly. The graph gap reflects its
incomplete join from high-level Python and typed Rust API roots to registry
keys, not a dead-code finding. Contrast retains two distinct entry contracts:
the stateful Python enhancer caches its degenerate image at construction, while
the one-shot Rust method recomputes from the current image. The detailed route
map is in `docs/CPU_REFACTOR_INVENTORY.md`. Manual source-route review of these
eight candidates is complete; compiler-backed reachability/dead-code proof and
the full Stage 5 source and size review remain pending. No build, formatting,
checkstyle, lint, test, parity, benchmark, or coverage command ran.

2026-11-13 F32 Resize arithmetic source review: Pillow 13.0's horizontal and
vertical F32 routines use ordered `sum += sample * coefficient` loops; the
source does not declare an architecture-specific product block width or
explicit FMA. The maintained implementation retains the archived width 16
outside AArch64, with the existing AArch64 width 4 specialization. Compiler
contraction can change the observed result, so source inspection cannot settle
x86_64 or Wasm parity. Keep the narrow kernel TODO for those oracle runs after
Stage 5; do not change arithmetic based on an assumed compiler lowering.
[Pillow 13.0 Resample.c](https://github.com/python-pillow/Pillow/blob/13.0.0/src/libImaging/Resample.c#L3248-L3436).
No validation command ran.

2026-11-13 ImageOps public-boundary source review: traced `crop`, `scale`,
`contain`, `cover`, `fit`, and `pad` from the Python facade through PyO3 into
the core-owned geometry and route callbacks. Python retains host protocols and
public wrapper calls; the core retains geometry, route selection, and Pad's
ordered native preparation. Source VERIFY markers now identify the reviewed
facade and geometry functions. Pad checks that the module-level `contain`
function is still the captured base implementation before attempting its
native route; a replacement takes the existing callback fallback, which calls
the current function. Output/error parity for replacement behavior, route
receipts, and latency remain pending under the existing ImageOps TODOs. No
shared parity or fault-contract standardization was done.

The generated parity input tree currently occupies about 14 GB across 5,144
JSON documents. A temporary SQLite/zlib prototype compressed three matrix
shards from 158.5 MB to 20.1 MB and verified their SHA-256 values after
readback. Use a campaign-local compressed SQLite cache after CPU source
closure; preserve the existing input contract and defer shared parity/fault
format standardization. The parity runner already supports gzip-compressed
result output via an output path ending in `.gz`.

2026-11-13 campaign input-base snapshot: the full indexed input set was
compressed into the ignored local artifact
`build/migration-parity/cpu-input-base-2026-11-13.sqlite3`. It contains 5,110
documents and 177,882 indexed cases. The JSON source documents total
12,990,383,313 bytes; per-document zlib payloads total 5,114,096,433 bytes
(about 2.54× before SQLite indexes and page overhead). This confirms a
campaign-local cache can reduce storage, but the parity runner does not read
the database yet and no parity run used it. Keep this as an input-storage
prototype only: preserve the present case/fault contract and defer its shared
format standardization until later. The artifact is untracked build output.

2026-11-13 Resize wrapper delegation source correction: Pillow 13.0 calls the
current public `self.reduce` when a valid `reducing_gap` selects a reduction
factor above one. The Python facade now invokes that method at Pillow's call
point in both the protocol-preserving preflight route and the ordinary typed
Rust planning route; the reducer's returned image supplies the carrier for the
final Rust resize and `_new` construction. The earlier source TODO to add this
callback boundary is closed. Output, callback mutation, unusual reducer
results, non-callable overrides, and fault-order parity remain pending in the
existing CPU campaign. The wrapper calls the current attribute directly so a
non-callable replacement raises at Pillow's call point instead of falling back
to the base implementation.
No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage
command ran.

Pillow 13.0 also computes reducing-gap factors before native size validation.
The core now preflights factor conversion in x-then-y order for non-positive
requested dimensions when a non-nearest reducing gap applies, preserving the
zero-axis `ZeroDivisionError` point. This source correction is unverified. The
Resize TODO keeps zero-axis parity, I;16 reducer rejection, bounds/error
precedence, lazy decode/mode lookup, and prepared-route ordering open.

Reduce now follows Pillow's factor-one delegation: after scalar normalization,
exact tuple `(1, 1)` calls the current public `self.crop(box)` before the native
reducer path. The Python facade retains this public dispatch for default and
explicit boxes; list `[1, 1]` remains on the native parser path. Source review
only; output and callback fault-order parity remain pending. The separate
Resize `reducing_gap` route now uses a core geometry plan and a thin Python
callback boundary for the current public `self.reduce`, then performs the final
resize in Rust. The facade keeps Pillow's recursive premultiplied conversion
path for LA/RGBA when subclass or instance conversion, reduction, or resize
methods are overridden; the ordinary exact `Image` case stays on the fused
Rust route. The callback planner retains Pillow's safe-box-before-reducer
validation order with signed bounds. Custom box-number faults and unusual
callback results remain TODOs; the original size/filter/box flow into a second
core boundary after the callback so Rust computes translated bounds from the
current box. No checks ran. The
campaign-local compressed SQLite input cache waits until source closure; shared
parity and fault-contract standardization remains deferred by user direction.

2026-11-13 Stage 5 source-review closure: the operation-family table and the
latest source follow-ups now cover all maintained CPU operation owners,
mode/architecture strategies, Python callback boundaries, and active caller
routes. The legacy declaration/file maps, execution-context flow, support
contracts, and CPU/Parallel separation are source-accounted. The final
workspace scan found no Rust files over 1,000 physical lines; a preliminary
function-body scan found no functions above 100 lines (15 candidates were
95–99 lines under that scan). Stage 5 is complete. Exact Clippy-counted
function-size proof, compiler-backed reachability/dead-code review, structural
checks, and compilation are Stage 6 gates; parity and performance remain later
gates. No validation command ran for this source-review closure. The SQLite
input-base remains a local prototype, and the shared parity/fault-contract
standard remains deferred.

2026-11-13 Stage 6 static and compilation gates: the implementation-structure
audit covered 6,122 paths with zero unclassified paths; the inventory and
repository-map checks passed. `make quality` passed, including documentation,
formatting, Clippy, source/backend boundaries, and full input/inventory/contract
checks. Clippy counted zero functions over 100 lines; the Rust file-size check
scanned 5,499 files and found zero over 1,000 lines. `make build-parity` and
`make build-parity-cpu-architecture` both passed. Clippy emitted existing
warning diagnostics. No unit-test suite, parity, benchmark, or coverage campaign
ran. Stage 7 is now unblocked.

2026-11-22 CPU source follow-up: rechecked Default byte-blur against the
archived serial helper. Three-pass fusion is now gated by the exact L, LA,
RGB, and RGBA mode identities for visible and retained lanes; other modes use
the repeated full-row path. The Filter inventory records the correction and
its pending runtime evidence. Convert now uses a core-owned staged plan before
the Python binding extracts matrix, dither, or color-count inputs. It preserves
the copy and matrix argument boundaries, routes RGBA-to-P and adaptive-P
requests through the existing quantizer, and follows Pillow's matrix length
then float32 item conversion. The remaining Convert TODOs are arbitrary mode
truth/equality overrides, custom matrix sequence callbacks, palette comparison
ordering through transparent routes, adaptive-P transparency palette remapping,
and custom dither `__index__` exceptions; all runtime evidence remains pending.
CPU Stage 5 remains open. No build, formatting,
checkstyle, lint, test, parity, benchmark, or coverage ran.

2026-11-22 Pillow 13.0 Filter host-boundary source follow-up: `Image.filter`
imports ImageFilter, then loads before callable construction and the duck-typed
`.filter` check, sends
single-band and `MultibandFilter` inputs directly, and otherwise extracts,
filters, and wraps bands one at a time before merging. Built-in GaussianBlur
and BoxBlur preserve their exact-zero copy shortcut and scalar-to-pair
coercion; BoxBlur validates nonnegative radii before the `2**31` upper bound.
Kernel computes `sum(kernel)` before validating coefficient count. Private
PyImage filter adapters now live in `pillow-rs-py/src/image/filters.rs`; their
signatures match the reviewed Pillow calls for one-margin
`expand`, required Gaussian/Box blur radii with optional C-int pass counts,
required UnsharpMask and ModeFilter arguments, C-int conversion of private
rank/expand inputs, and
`getpalettemode`'s string-or-no-palette-error result. Private ImageChops
`offset` and `chop_*` methods now delegate to existing core operations and
materialize before returning, matching the eager ImagingCore callback boundary.
Direct ImagingCore `paste(source, box, mask=None)` now uses the prepared
mode-owned Paste path and active execution context; its output, mutation timing,
faults, and route receipts remain pending parity. The direct
`ImagingCore.transform` callback is an internal method, not a standalone CPU
Transform deliverable. Review it only when a public wrapper's observable
contract reaches that path; public `Image.transform` parity uses its own
prepared apply/MESH route. Remaining source TODOs are Arrow methods and
invalid-return fault order; `convert_into` has an implementation whose parity
and fallback receipts remain pending. `convert_transparent` now has a
direct PyImage adapter and native mode-specific CPU Default strategies; output,
argument faults, and route receipts remain pending parity. PyImage.point
already carries the active filter route through ImagePointTransform; output and callback-fault parity remain
pending. `PyImage.setalpha` now uses the existing exact-mode PutAlpha strategies
for RGB and RGBX and updates the live image handle; its mode, fault, output, and
route parity remains pending. `PyImage.getpalette` also has its direct rawmode
adapter; packed output and argument-fault parity remain pending. Raw pointer
access is excluded. `PyImage.pixel_access()` now creates the existing
PixelAccess carrier with the active route; factory, pixel-fault, mutation, and
receipt parity remain pending. `PyImage.convert_matrix()` now uses the typed
Convert matrix plan; mode-arity faults, output, and architecture/fallback
receipts remain pending. Runtime outputs,
fault order, fallback receipts, and latency
remain pending the frozen CPU gates. References: [Pillow 13.0 Image.filter](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L1325-L1353),
[Image._new](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L3219-L3243),
[Pillow 13.0 ImageFilter](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/ImageFilter.py#L1421-L1707),
and Pillow 13.0 [ImagingCore method table](https://github.com/python-pillow/Pillow/blob/13.0.0/src/_imaging.c#L4009-L4085),
[expand](https://github.com/python-pillow/Pillow/blob/13.0.0/src/_imaging.c#L1147-L1165),
[ModeFilter](https://github.com/python-pillow/Pillow/blob/13.0.0/src/_imaging.c#L1525-L1539),
[getpalettemode](https://github.com/python-pillow/Pillow/blob/13.0.0/src/_imaging.c#L1286-L1301),
and [blur/UnsharpMask bindings](https://github.com/python-pillow/Pillow/blob/13.0.0/src/_imaging.c#L2377-L2439).
2026-11-22 ImagingCore blur-pass source follow-up: private `gaussian_blur`
and `box_blur` accept optional C-int pass counts (defaults 3 and 1). The Rust
adapters now forward those counts into CPU core methods; non-default Gaussian
passes lower the C Gaussian-to-box radius into the existing `BoxBlurXY` pass
strategy. Empty images copy before pass/radius/mode checks; otherwise pass
count, radius, and mode checks follow Pillow 13.0 order. The shared Gaussian
radius calculation preserves Pillow's float32 `sigma²` and fractional-radius
arithmetic plus its double-precision `sqrt`/`floor` conversion points. Output,
fault order, strategy receipts, and latency remain pending the frozen CPU
gates. Source review only; no validation ran. Reference: [Pillow 13.0
BoxBlur.c](https://github.com/python-pillow/Pillow/blob/13.0.0/src/libImaging/BoxBlur.c#L1230-L1396).

2026-11-22 ImagingCore `isblock` source follow-up: added the no-argument
`PyImage.isblock()` adapter and a core query for Pillow's explicit
single-block allocator state. Pillow returns whether `Imaging.block` is
non-null; Rust-owned raster vectors do not use Pillow's allocator and report
false. This property read has no pixel strategy or backend selection. Source
review only; exact public return parity remains pending the frozen CPU matrix.
No validation ran. Sources: [Pillow 13.0 `_isblock`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/_imaging.c#L2442-L2452),
[block allocation](https://github.com/python-pillow/Pillow/blob/13.0.0/src/libImaging/Storage.c#L458-L478),
and [array allocation](https://github.com/python-pillow/Pillow/blob/13.0.0/src/libImaging/Storage.c#L388-L423).

2026-11-22 ImageChops offset default correction: Pillow 13.0 assigns
`yoffset = xoffset` when omitted before loading the image, then calls the
private ImagingCore offset with both values. The thin facade now applies that
default before its load-and-dispatch helper, retaining the original value for
both C-int conversions. Unary and binary ImagingCore operations dispatch
through private PyImage methods, pass the wrapper's captured backend and
architecture context, and eagerly materialize at the ImagingCore callback
boundary. `constant` and `duplicate` use the public `Image.new` and `copy`
routes; `blend` and `composite` use the corresponding Image operations. The
duplicate module-level Chops binding path was removed. TODO (`pillow-rs#3ptb`):
verify Chops output and mode/fault order, omitted and explicit y offsets, C-int
faults, and selected/fallback receipts in the frozen CPU matrix. Source review
only; no build, formatter, checkstyle, lint, test, parity, benchmark, or
coverage ran.
Sources: [Pillow 13.0 ImageChops](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/ImageChops.py#L760-L1088),
[Image.composite](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L3460-L3473),
and [ImagingCore method table](https://github.com/python-pillow/Pillow/blob/13.0.0/src/_imaging.c#L4009-L4085).

Direct Chops calls made by a Python callback now resolve the active wrapper
ContextVar at the PyO3 binding boundary when no explicit route is supplied;
they ask the scoped owner to capture the route if that scope has not captured it
yet.
Direct ImagingCore SetMode, PutBand, FillBand, palette mutations,
`apply_transparency()`, and unpacked-byte reads now inherit that route without
replacing the mutable PyImage handle. Their pending sources are routed before
materialization; SetMode and band writes also pass Backend/Architecture into
their eager selectors. PutAlpha retains the full route, including pinned
backend, through its context-taking core calls. One-shot Contrast/Color
fallback methods now take the captured context before eager base construction
and bind it to the returned Blend graph. Transform source review covers
premultiplied recursion, custom handlers, `getdata` callbacks, prepared output,
and MESH record routing. Callback faults, outputs, and receipts remain TODOs.

2026-11-22 Crop carrier decision reaffirmed: keep Pillow's source-box equality
and coordinate comparisons, load, `round()`, `int()`, and arbitrary-precision
extent policy in Python, then pass four normalized built-in integers through
the fixed C-int carrier. The maintained wrapper already follows this order;
do not route coordinates through `f64` or add generic Python callbacks to
Rust. Output, C-int overflow faults, and selected/fallback receipts remain in
the frozen CPU parity work. Source review only; no validation ran.

2026-11-22 ImageChops Add/Subtract C-int adapter correction: the private
PyImage methods now receive offset as a Python object and call the shared
`c_int_from_python` converter, preserving the prior C-int range and integer
protocol boundary while carrying the captured execution context. The scale
argument remains a float32 carrier pending exact PyArg conversion/fault-order
parity. Source review only; no build, formatter, checkstyle, lint, test,
parity, benchmark, or coverage ran.

2026-11-22 direct ImagingCore route handoff: `Image.im.setmode()`,
`putband()`, and `fillband()` pass the active Backend/Architecture context to
the core mode strategy selectors. Palette mutations and transparency commits
route pending inputs before loading while retaining the same PyImage handle.
PutAlpha now calls the context-taking Rust API and preserves pinned-backend
state on successful and failing mutations. `tobytes_unpacked()` applies the
active route before materializing a pending image. Crop retains the selected
fixed built-in-integer carrier after Python performs Pillow's comparisons,
load, `round()`, `int()`, and arbitrary-precision size policy. Source review
only; output/fault parity, selection receipts, and latency remain pending. No
build, formatter, checkstyle, lint, test, parity, benchmark, or coverage ran.

2026-11-22 Enhance fallback context handoff: added context-taking Contrast and
Color core entry points for the one-shot fallback modes whose base construction
is eager. The thin PyO3 route supplies Backend/Architecture before fallback
work, then binds the same context to the returned lazy Blend graph. Output,
error order, selection receipts, and latency remain pending; source review
only. No build, formatter, checkstyle, lint, test, parity, benchmark, or
coverage ran.

2026-11-22 direct mutation route-order correction: ImagingCore SetMode,
PutBand, and FillBand now leave the destination unchanged until core mode,
band, layout, and strategy checks pass, then apply the captured pinned Backend
and Architecture before materialization. PutBand applies that route to its
source at the same validated boundary. Palette/transparency and PutAlpha
bindings pass routes into core methods that place route injection after their
own validations, retaining the staged destination promotion required by
PutAlpha. Thumbnail passes its route into core, which validates filter,
reducing-gap, and no-op geometry before routing a source that needs
materialization. This keeps rejected calls from changing a pending graph's
route. Output, fault, route receipt, and latency parity remain TODO under
`pillow-rs#3ptb`. Source review only; no build, formatter, checkstyle, lint,
test, parity, benchmark, or coverage ran.

2026-11-22 source follow-up: the earlier Filter host-boundary note listed
ImageChops Add/Subtract float32 conversion as an open source item. Review of
Pillow 13.0's `O!|fi` adapter and PyO3's `f32` extraction confirms that both
call Python float conversion and then cast the `f64` result to `f32` without
range checking, before converting the optional C-int offset. The existing
adapter preserves that order; only output/fault parity remains pending.
The Crop decision is fixed at a four-built-in-int C-int carrier after Python
has handled comparisons, load, `round()`, `int()`, and arbitrary-precision
extent policy. ImageF32 Resize retains portable Default width 16 and AArch64
baseline width 4 as distinct strategies, per the user's decision; the output,
selection-receipt, and per-architecture benchmark matrix remains pending.
During manual file-size review, moved ImagingCore SetMode/PutBand/FillBand
methods from `image/data_access/put.rs` into `image/data_access/imaging_core.rs`;
the files are now 738 and 269 lines. A physical-line scan found no maintained
Rust source file over 1,000 lines. This source review does not replace Stage 6's
Clippy-counted function-size proof or other static gates. No build, formatter,
checkstyle, lint, test, parity, benchmark, or coverage ran.

2026-11-22 ImagingCore Paste callback source follow-up: added direct
`PyImage.paste(source, box, mask=None)` using Pillow's low-level nested
four-coordinate box parser, the existing typed Paste source/mask contracts,
and the active Backend/Architecture context. The new input adapter accepts a
PyImage as an ImagingCore source but does not reinterpret a public Image wrapper
as one; high-level `Image.paste` retains its existing wrapper-aware path.
Destination mutation stays on the prepared mode-specific Paste operation. This
removes direct Paste from the custom-filter source TODO. Output, mutation
timing, argument faults, and selected/fallback receipts remain pending CPU
parity.
Source review only; no build, formatter, checkstyle, lint, test, parity,
benchmark, or coverage ran. Reference: [Pillow 13.0 `_paste_impl`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/_imaging.c#L1561-L1604).

2026-11-22 ImagingCore `convert_transparent` source contract: Pillow 13.0
admits RGB to RGBA, RGBa, LA, or La; and One, I32, native I16, or L to RGBA
or LA. It compares the transparency key against original RGB for RGB-to-LA/La
and against converted RGB for scalar-to-RGBA/LA; premultiplied targets apply
their alpha arithmetic after that key decision. The maintained PyImage adapter
and mode-specific CPU Default strategies are now written for One/L/RGB/I/I;16
sources. They do not route through ordinary Convert or cast the source mode.
Source review only; output, argument fault, and selected/fallback receipt
parity remain pending. No validation ran. References: [Pillow 13.0
`_convert_transparent`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/_imaging.c#L1091-L1116)
and [ImagingConvertTransparent](https://github.com/python-pillow/Pillow/blob/13.0.0/src/libImaging/Convert.c#L1550-L1616).

2026-11-22 Convert transparency source continuation: typed preflight now covers
P-to-alpha palette promotion and L/RGB transparent-alpha conversion; matrix
transparency metadata is transformed after successful pixel conversion and
`_new` info copying. Output, callback/fault order, metadata, and architecture-
receipt parity remain pending. No build, formatter, checkstyle, lint, test,
parity, benchmark, or coverage ran.

2026-11-22 palette source continuation: P-source Convert now decodes the
currently supported truthy raw-palette layouts through the typed Rust palette
path before invoking Python `ImagePalette.getcolor` with the original image for
histogram callbacks. Remaining Pillow raw-mode unpackers, malformed-buffer
fault order, and callback/route parity remain open. CMYK alpha mutation now
reinterprets complete C/M/Y/K entries as RGBA, retaining K as alpha; packed
CMYK;L and CMYK;I outputs ignore incomplete byte tails while direct raw CMYK
access retains them. These are source corrections only: palette byte/fault
parity remains pending. No build, formatter, checkstyle, lint, test, parity,
benchmark, or coverage ran.

2026-11-22 Transform QUAD prepared-route correction: Pillow's host
`Image.transform` computes the eight QUAD mapping coefficients in Python after
the observable sequence slicing and numeric operations. The Python adapter
already preserves those operations, but the prepared Rust output route treated
the resulting coefficients as corners and converted them a second time. It
now passes the eight normalized values directly to the QUAD strategy. The
Rust-facing raw-data route still converts raw corners once, and streamed MESH
records already pass their normalized coefficients directly. Source review
also traced the captured Backend/Architecture through plan allocation, active
callback scope, load, prepared execution, and each MESH record. Transform
mode/method/output/fault parity, callback mutations, and selected/fallback
receipts remain pending; broader CPU source review is still open. No build,
formatter, checkstyle, lint, test, parity, benchmark, or coverage ran. Source:
[Pillow 13.0 `Image.__transformer`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L2765-L2851).

2026-11-22 Transform palette source correction: Pillow copies an attached
palette for both P and PA before processing transform data. The Python wrapper
previously copied only P's wrapper-owned palette object; it now handles PA too.
The Rust destination allocator already preserves PA's typed palette and alpha
table, so this change aligns the wrapper metadata copy with the existing core
output. Palette callback order, object identity, bytes, and mode/architecture
receipts remain pending parity. Source-only; no validation ran.

2026-11-22 indexed palette materialization correction: a native RGB/RGBA/CMYK
palette replacement on P/PA now updates the typed palette metadata without
materializing the pixel pipeline. Non-native raw layouts resolve the source
before raw palette decoding so source-load errors take precedence. The public
Python `Image.putpalette` wrapper still needs its dirty palette state and
partial mode/failure mutations reconciled with this core path. Palette
rawmode/output/fault contracts remain under source review; no build, formatter,
checkstyle, lint, test, parity, benchmark, or coverage ran.

2026-11-22 typed palette sidecar propagation: lazy pipeline materialization now
prefers the pipeline node's updated `PaletteData` when constructing its loaded
P/PA result. This preserves native CMYK palette identity through metadata-only
replacement and subsequent loading. Source-reviewed; pipeline output and fault
parity remain pending. No validation ran.

2026-11-22 Draw palette sidecar preservation: Paletted Draw Text and Bitmap
outputs now carry the existing typed `PaletteData` model along with palette
bytes and alpha metadata, preserving CMYK identity through index-preserving
drawing. Source reviewed; output, fault order, route receipts, parity, and
benchmarks remain pending. No validation ran.

2026-11-22 public palette wrapper source correction: `Image.putpalette` now
copies incoming `ImagePalette` objects, marks them dirty, and stages the L/LA
to P/PA mode transition in Rust before rawmode processing. Native P/PA palette
attachment remains deferred; L/LA transitions and raw layouts realize through
a metadata-only typed Rust path before pixel loading. This preserves the
mode/palette mutation if later source loading fails. Output, failure mutation,
malformed-layout order, and route receipts remain pending; no validation ran.
See the [CPU refactor inventory](CPU_REFACTOR_INVENTORY.md) for the source
reference.

2026-11-22 ImagePalette input correction: Pillow rebuilds a supplied palette
through `ImagePalette.raw(data.rawmode or "RGB", data.palette)` before applying
the `putpalette` rawmode argument. `Image.putpalette` now uses this path instead
of preserving the supplied object's mode and dirty bookkeeping. This corrects
the preceding note's shorthand “copies incoming ImagePalette objects.” The
function source was re-reviewed; output and callback/fault-order parity remain
pending. No validation command ran. Source: [Pillow 13.0
`Image.putpalette`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L1954-L1996).

2026-11-22 native palette entry-count correction: Pillow's low-level
`putpalette` checks `floor(input_bytes * 8 / rawmode_bits) <= 256` before
installing the palette, and its unpacker writes only those complete entries.
Native RGB/RGBA/CMYK Rust palettes now pass through the same prepared raw
decoder as converted layouts, enforcing the limit and dropping a partial tail
from the core palette. P/PA native replacement still updates palette metadata
without materializing its pixel pipeline. The Python `ImagePalette` byte
buffer remains separate wrapper state. Output, deferred fault order, and route
parity remain pending; no validation ran. Sources: [Pillow 13.0
`_putpalette_impl`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/_imaging.c#L1905-L1967)
and [`_getpalette_impl`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/_imaging.c#L1238-L1275).

2026-11-23 flattened pipeline context handoff review: `PipelineOps::append`
retains its injected `ExecutionContext`, `append_pipeline_op` carries that
state through prefix-cache and palette metadata updates, and `new_pipeline_node`
prefers the retained context before inheriting one from its source. Mode-changing
nodes separately inherit the source context. Rebinding an existing graph
recursively updates nested image operands and deliberately clears materialized,
prefix, and palette caches. This source review found no additional context-drop
path; runtime nested-output and selected/fallback receipt evidence remains
pending. No validation command ran.


2026-11-23 RGB8 Convert architecture source audit: prepared selection uses the
exact RGB8 source, destination mode, and injected context. The RGB8 owner now
declares 35 architecture guards: AVX2/SSSE3/SSE2/NEON/Wasm SIMD128 for L, and
SSE2/NEON/Wasm SIMD128 for LA, La, CMYK, YCbCr, HSV, I32, F32, RGBA, RGBa, and
RGBX. P/PA, One, I;16 variants, and LAB remain on their Default or
wrapper-owned routes. Same-mode RGB8 returns before mode dispatch, so its
three unreachable architecture entries, identity arms, and private vector-
layout helper were removed. The remaining architecture arms match their mode
guards; SSE2/NEON/Wasm share the wide conversion helpers, while L uses the
corresponding grayscale kernels. NEON RGB-to-RGBA uses its gated 16-pixel leaf
and falls back to the shared layout helper if its output reservation fails.
Source review only: target builds, runtime selection/fallback receipts, exact
output/fault parity, and latency remain pending. No build, formatter,
checkstyle, lint, test, parity, benchmark, or coverage ran.

2026-11-23 Color Convert architecture-owner source review: the 25 mode
identities map one-to-one through `Mode`, `DynamicImage`, classification, and
prepared mode execution. The 13 architecture-bearing source owners reconcile
320 declared architecture rows with their guards, module cfgs, and exact-mode
execute arms; the other 12 owners remain Default-only for Convert. Same-mode
identity and palette routes remain separate. The RGBa SSSE3 cfg was rechecked
against the baseline: both contain one x86/x86_64 predicate, correcting the
earlier false duplicate report. This closes the Convert source crosswalk only;
target builds, instruction safety, runtime selection/fallback receipts,
output/fault parity, and latency remain pending. The all-family source and size
review remains open. GPU review remains deferred; the user selected the
injected CPU Architecture for GPU host-semantic/preflight fallbacks when
available. No build, formatter, checkstyle, lint, test, parity, benchmark, or
coverage ran.

2026-11-23 Put architecture crosscheck: five prepared operation families
(ExtractBand, PutData, PutPixel, PutAlpha, and PutAlphaData) reconcile 171
architecture support rows with their mode declarations, request guards,
architecture modules, and exact-mode execute arms. The rows cover SSE2, NEON,
Wasm SIMD128, and SSSE3 across 57 unique `put/<mode>/<arch>` folders. The SSSE3
ExtractBand raw gather path received a source-level bounds and allocation
review. No source-level dispatch mismatch or unsafe memory defect was found.
This closes only those architecture-owner and guard traces. PutBand/FillBand
outputs and errors, wrapper/fallback receipts, hidden-lane behavior, runtime
architecture selection, target builds, parity, and latency remain pending.
ImageOps and Transpose macro routes and the all-family source/size audit remain
open. No build, formatter, checkstyle, lint, test, parity, benchmark, or
coverage ran.

2026-11-23 Transpose source crosscheck: all 25 mode identities map to their
exact owners and preserve the input mode. The mode support declarations,
request guards, target module cfgs, architecture folders, and prepared execute
arms reconcile across Flip, Mirror, and Transpose. The three operations declare
122 architecture rows across 56 mode/architecture folders. Native-byte and
I;16 leaves use shared portable-wide helpers; RGBA also has AVX2 axis-swap and
x86_64-baseline tiled Rotate270 paths. Source review found no support/dispatch
or unsafe bounds mismatch. Empty/trailing-storage parity, output and fault
parity, receipts, target builds, and latency remain open. This closes the
Transpose architecture crosswalk only; ImageOps and the all-family source/size
review remain pending. No build, formatter, checkstyle, lint, test, parity,
benchmark, or coverage ran.

2026-11-23 ImageOps Invert macro crosswalk: its 14 mode owners, 24
architecture folders, support predicates, guards, and exact-mode execute
routes have now been traced and are recorded in
`docs/CPU_REFACTOR_INVENTORY.md`. The shared portable-wide kernel and all 14
Default leaves were source-reviewed. The direct-Rust trailing-storage
Default/architecture discrepancy has a `pillow-rs#3ptb` TODO and remains for
the frozen parity phase. No behavior validation ran. ImageOps wrapper behavior,
PutBand/FillBand, all-family source and size review, runtime selection receipts,
target builds, parity, and benchmarks remain pending.

2026-11-23 ImagingCore PutBand/FillBand source crosswalk: exact-mode support,
input validation order, serial Default selection, mutation, and compact
hidden-lane preservation have been traced and recorded in
`docs/CPU_REFACTOR_INVENTORY.md`. Pillow first checks native type and size for
single-band PutBand, then `ImagingCopy2` rejects a source whose mode differs;
Rust's earlier exact-mode rejection matches the final outcome. Exact-mode P
PutBand has an unresolved source-palette transfer difference. Direct Rust
overlong buffers have distinct behavior: the one-band shortcut copies the
entire source vector, while multiband PutBand and FillBand require exact
storage. These paths remain in CPU parity and fault-contract work; no runtime
verification ran.

2026-11-23 ImageOps Invert wrapper source crosswalk: the Rust callback sequence
matches Pillow 13.0's `1` point path and P palette-aware `_lut` path, including
the public palette calls. The top-level route matrix still declares six modes
as producing that Pillow's `_lut` rejects, and Rust direct `invert_ops` takes
the low-level P index path rather than the Python palette path. Decide which
direct Rust routes are intentional `CoreOnly` extensions and mark them
`NO_PILLOW_SUPPORT`; align or document the P direct-Rust contract. Callback
fault parity, mode-override behavior, receipts, and the corrected support
matrix remain pending. No behavior code or runtime verification ran.

2026-11-23 ImageOps Posterize/Solarize callback and mode-route source review is
recorded in `docs/CPU_REFACTOR_INVENTORY.md`. Wrapper admission matches Pillow:
L/RGB use `point`, P transforms palette entries, and other modes reject.
Direct Rust P index operations remain explicitly `NO_PILLOW_SUPPORT` CoreOnly
strategies. Output/fault parity, route receipts, and latency remain pending.

2026-11-23 ImageOps Invert route follow-up: the wrapper support descriptor now
rejects `La`, `RGBa`, `RGBX`, `YCbCr`, `LAB`, and `HSV` before Eval admission,
matching Pillow's `_lut` mode gate. Direct Rust `Invert` CoreOnly strategies
remain separate. Output/fault and route-receipt parity remain pending.

2026-11-23 ImageOps Flip/Mirror follow-through: the public `Image.transpose`
wrapper now loads before PyO3 method parsing, matching Pillow's source fault
order while retaining queued Rust output work. The `load()` route owns encoded
P/PA palette timing. Frozen error-order, output, override, receipt, and latency
cases remain pending.


2026-11-23 PutBand palette follow-through: the single-band ImagingCore P
route now transfers attached source palette metadata after the exact-mode
native copy, preserving the destination when the source has no palette. This
closes the source-level PutBand palette gap; output/fault/route parity remains
pending the frozen CPU matrix. No validation command ran.


2026-11-23 typed Mode API source audit: confirmed that maintained CPU Rust
constructors, raw-byte input, generated image paths, pixel writes, and Draw
identity take `Mode`; Python/JavaScript own Pillow string parsing. The pending
ledger's older string-mode API TODO was stale and has been corrected. GPU
result metadata retains its separate mode-string TODO for Stage 7.


2026-11-23 Draw multiline context follow-through: source-traced the injected
route through direct multiline text, per-line custom stroke/fill masks, and
reentrant Bitmap callbacks. The callbacks install the parent's backend and
architecture and restore the previous override. This corrects a stale inventory
claim; callback/output/receipt evidence remains for the frozen CPU parity gate.

2026-11-23 Convert transparency callback follow-through: the generic
`__index__` TODO for P-source scalar transparency was removed after Pillow 13
source review: those paths accept `int` instances and do not coerce arbitrary
indexable objects. Output P-palette transparency now returns its RGB key from
Rust and invokes the public `ImagePalette.getcolor` callback after `_new`, so
custom `background`/`transparency` equality methods run at Pillow's point while
palette data mutation stays in Rust. Direct Rust Convert keeps its native
numeric allocator. Source review is complete for this path; exact output,
callback fault/side-effect behavior, route receipts, and latency remain pending
the frozen Convert parity cases. Source: [Pillow 13.0 `Image.convert`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L1080-L1112),
[`_imaging.c` `getink`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/_imaging.c#L558-L594),
and [`ImagePalette._new_color_index`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/ImagePalette.py#L123-L151).

2026-11-23 ImagePalette 13.0 reserved-slot correction: the earlier no-padding
note was specific to Pillow 12.2. Pillow 13 appends a zero palette entry for
each skipped reserved index before selecting the new slot. The ImagePalette
adapter now preserves this in its native fast path and its callback path;
custom equality checks still execute in Python order. This shared source path
affects Draw, PutPixel, ImageOps, and Convert. Output, mutation, fallback, and
latency evidence remain pending the frozen CPU matrix. Source: [Pillow 13.0
`ImagePalette._new_color_index`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/ImagePalette.py#L123-L151).

2026-11-24 ImageOps mode crosswalk follow-up: the support and ownership map
now traces all 25 exact identities through Expand, Quantize, and ImagingCore
edge expansion, alongside the existing Invert and Posterize/Solarize reviews.
This does not close the per-leaf archive comparison or runtime gate. P Equalize
still performs Pillow's required P-to-RGB output transition, but its Default
and selected architecture strategies now apply the RGB LUT in place on that
owned output buffer, removing a second full-frame allocation. Output, fault,
architecture/fallback receipt, and latency checks remain pending the frozen
CPU phase. No build, formatter, checkstyle, lint, test, parity, benchmark, or
coverage ran.

2026-11-24 Quantize output construction: the prepared CPU result now adopts the
quantizer's index vector directly with `GrayImage::from_raw`, avoiding a
second full-frame allocation and copy. A wrong index-buffer shape now produces
an internal error instead of silently filling missing indices with zero.
Source-reviewed only; no runtime verification ran.

2026-11-24 source inventory: no first-party Rust source file exceeds the
1,000-line limit; one is exactly 1,000. Thirty-seven files are at or above 800
lines, including nine in `pool_cpu`. Those nine CPU files have now been read
through for source structure and operation ownership. Two functions were over
the 100-line source-span estimate and were split; authoritative Clippy-based
function-size verification remains deferred to Stage 6. A search of maintained
`pillow-rs/src` outside `pool_cpu/legacy` found no legacy imports or module
declarations; the archive remains reference-only. No maintained Rust unit-test
attributes or non-legacy Rust `tests/` trees were found; legacy test sources
remain preserved, with parity and benchmark tooling separate.

2026-11-24 Colorize source crosswalk: the exact-L strategy's LUT builder
preserves the archived floor-division interpolation, endpoint handling, and
channel order. The selected SIMD strategies retain the archived nibble-table
selection and packed RGB output; `shuffle` receives only 0–15 indices here,
matching the archived `swizzle_relaxed` result for every lookup. The public
ImageOps wrapper remains distinct: it checks the Pillow mode, builds the LUT
through ordered host callbacks, calls public `convert("RGB")`, and then routes
the RGB result through the LUT/Eval child. Native Colorize, this wrapper
composition, architecture/fallback receipts, fault order, exact output, and
latency remain pending the frozen CPU parity and benchmark gates. No runtime
verification ran.

2026-11-24 ImageOps Invert source review (`pillow-rs#3ptb`): Pillow's wrapper
builds the reverse LUT before testing mode `1`, calls public `point()` for `1`,
routes `P` through copied palette metadata, and uses public `point()` for the
other admitted modes. The Rust callback adapter preserves this order; the
wrapper support matrix remains separate from direct Rust `Invert`'s CoreOnly
mode extensions. Default and architecture strategies select by exact mode and
injected context. Point-run fusion is limited to L/LA/RGB/RGBA; its identity
shortcut previously cloned hidden physical lanes, unlike Pillow 13.0's new
zero-initialized `ImagingPoint` output. The fused identity path now initializes
those omitted lanes to zero before returning the clone. Noncanonical raw-suffix
behavior still differs between Default and vector Invert and remains tracked
for the frozen parity decision. Output/fault parity, route receipts, and
latency remain pending. Pillow source: [Point.c](https://github.com/python-pillow/Pillow/blob/13.0.0/src/libImaging/Point.c)
and [Storage.c](https://github.com/python-pillow/Pillow/blob/13.0.0/src/libImaging/Storage.c).
No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage ran.


2026-11-24 source-audit update: completed source-level declaration → compile
gate → prepared selection → execution crosschecks for the maintained direct
intrinsic leaves in Convert, shared SSSE3 channel gather, RGB Eval, Reduce, and
Composite Blend. No source dispatch mismatch was found in these slices. The
26-file direct-instruction sweep is now crosswalked; the broader CPU operation,
mode, fallback, wrapper, and 100-line function review is still in progress.
Target compilation, emitted-code confirmation, runtime receipts, CPU parity,
and benchmarks are still pending. No validation command ran in this update.

User decision for the later GPU refactor (2026-11-24): internal host-semantic
and preflight CPU fallbacks must receive the pipeline's injected CPU
architecture when one is available, rather than silently constructing a new
CPU Default context. GPU implementation remains sequenced after CPU completion.

2026-11-24 source/size review follow-up: the latest physical-line inventory
finds 37 first-party Rust files at or above 800 lines, including nine under
`pool_cpu`, with none above 1,000; one file is exactly 1,000. Two active
functions exceeded the 100-line ceiling in a source-span scan; Pad centering
resolution and pipeline ICC profile selection are now separate helpers and
both reviewed bodies are under the limit by that scan. Clippy remains deferred
until Stage 5 source review is complete. The earlier 35-file count is
superseded.

2026-11-24 ImageOps source follow-up: the exhaustive 25-mode `CropBorder`
aggregator and its prepared selection were traced through the typed Crop leaf.
All non-I;16 routes use Default; the four I;16 aliases use matching Crop and
CropBorder architecture sets and guards for admitted nonempty crops, while
empty results remain Default. Direct Rust Equalize explicitly accepts L/RGB
and the operation-defined P-to-RGB route; Posterize and Solarize accept native
L/RGB samples plus Rust-only native P indices. All other exact Modes are
deliberate rejections in the exhaustive support match. The former convenience
RGB-conversion fallback is not pending work and must not be restored to reuse
an algorithm. A future Rust-only mode extension needs a native exact-mode
strategy and a `NO_PILLOW_SUPPORT` declaration when Pillow has no route. Mode
outputs, rejection fault order, and architecture/fallback receipts remain
unverified in CPU parity.

2026-11-24 large-file source-review checkpoint: reviewed the remaining
`pool_cpu` files in the >=800-line band (Composite, Enhance, Filter, Analysis,
the ImageOps histogram helper, and resize byte samples). Combined with the
previously reviewed batch, Put, and ImageOps owners, this covers all nine
maintained CPU files in that band. This closes the physical-size review slice;
it does not close the full CPU operation/mode/fallback audit or Stage 5. Source
maps showed no orchestration mismatch in the reviewed slices. RGB Merge source
follow-through confirms the receipt contract: an ineligible SSSE3/NEON
interleave guard falls through to the same-architecture `MergeNativeInterleave`
route, retains the ineligible-request fallback reason, and records
`wide-shuffle` from the portable-wide helper. Parity must still confirm that
combined receipt on short RGB inputs. Outputs, callback faults, other runtime
receipts, target code, parity, and performance remain unverified. No validation
command ran.

2026-11-25 Arrow import scope follow-up (`pillow-rs#3ptb`): public
`PIL.Image.fromarrow` now calls the producer once from the thin Python wrapper,
then validates the C-data mode/schema/shape and copies bytes into owned
mode-tagged Rust storage. Compact carriers preserve omitted lanes, and P/PA
images receive the expected empty palette object. This produces the image
pixels without a second full physical-buffer allocation. Pillow documents a
pixels without a second full physical-buffer allocation. The wrapper now
exposes Pillow's initial `readonly == 1` state and clears it at the first
`load()` boundary; because import already owns a copy, this transition does
not require another detach copy. The importer still does not share the
producer's allocation, so the documented zero-copy memory behavior and
large-buffer latency remain different and unmeasured. CheckedDims also enforces
an allocation limit that Pillow's borrowed path does not. Keep allocation
limit and C-data fault ordering as TODOs for focused CPU parity. The importer
rejects root and fixed-list child structs with null release callbacks before
reading their schema, shape, or buffer fields. Pointer validity and buffer
extent still depend on a compliant Arrow C Data Interface producer; a forged
capsule cannot be made safe by checking its name. Do not add a standalone lane
for private ImagingCore entry points. No build, formatter, checkstyle, lint,
test, parity, benchmark, or coverage ran.

2026-11-25 Transform filter-mode source correction (`pillow-rs#3ptb`):
Pillow 13.0 validates the requested filter, loads the images, and forces nearest
only for source modes `1` and `P`; PA keeps the requested nearest, bilinear, or
bicubic filter. The Rust public Transform adapter now follows that exact policy.
The existing mode generator covers each of the 20 modes with all three filters
across affine, extent, perspective, quad, and mesh; parity inputs and receipts
remain pending. This applies to public `Image.transform`; direct
`ImagingCore.transform` has no independent parity lane. No validation ran.

2026-11-25 public Save stdout source follow-through (`pillow-rs#3ptb`): Pillow
redirects `sys.stdout` to its binary buffer when stdout is a `TextIOWrapper`;
the Rust adapter now selects that writer before inspecting `.name` or writing
encoded bytes. For unnamed file-like outputs, it preserves Pillow's separate
`hasattr`, pathlike check, and final `os.fspath` access to `.name`. Save target,
format, file creation, and execution-route preparation are split into small
helpers so the public adapter stays within
the function-size policy. Truthy explicit formats bypass extension parsing as
in Pillow; inferred formats lowercase the suffix, using bytes-lower/decode for
byte paths. The host path carrier retains the `os.fspath` value and its
text-versus-bytes identity for format inference and filename-bearing OS
errors; null-path errors are deferred to the filesystem boundary. Cross-
platform path edge cases and runtime behavior remain unverified. Parity remains
pending for stdout, path faults, writer callbacks/partial writes, encoder
faults, and route receipts.
No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage ran.
Source: [Pillow 13.0 `Image.save`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L2376-L2496).

2026-11-25 HostPath Windows source follow-through (`pillow-rs#3ptb`): Windows
byte paths now pass through Python `os.fsdecode` and both byte- and text-based
paths are transferred as UTF-16 code units with `surrogatepass`, preserving
filesystem surrogate names in Rust's `OsString`. This also retains the original
`os.fspath` value for Python-visible suffix and error handling. The fallback
for non-Unix, non-Windows targets remains UTF-8-only. Cross-target compilation
and Windows open/save/font path parity are pending; no validation ran.


### Public Paste mutation boundary source follow-through

Pillow's public `Image.paste` calls `_ensure_mutable()` and immediately applies
`self.im.paste(...)` after source conversion and destination/mask loading. The
Python binding now runs typed Paste on a cloned Rust image handle with the
same captured backend/architecture context, materializes it before committing
the handle, and leaves the original destination untouched on failure. This also
makes the nested Paste in ImageOps.expand complete its canvas fill before the
wrapper returns; direct Rust Paste and Expand graph APIs remain lazy. Pixel
output, mutation isolation, fault timing, metadata, architecture receipts, and
latency remain pending. No build, formatter, checkstyle, lint, test, parity, benchmark, or
coverage ran. Source: [Pillow 13.0 `Image.paste`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L1694-L1775).


### Public in-place pixel mutation source follow-through

Pillow 13's public `Image.putpixel` and `Image.putdata` call `_ensure_mutable()`
before their input work and apply pixel writes during the call. PutData's
callback-sensitive Rust path already writes synchronously and retains accepted
prefix writes if a later value faults; the bulk path now materializes only
after the typed bulk call succeeds. Public PutPixel and PixelAccess setters now
execute queued writes with their captured context before returning. Public
`putalpha` now materializes mode promotion before callbacks that observe the
promoted destination, and completes any final or partial mutation in its
existing `finally` boundary. Direct Rust graph APIs remain lazy. Pixel output,
partial mutation/fault order, metadata, selected/fallback receipts, and latency
remain pending. No build, formatter, checkstyle, lint, test, parity, benchmark,
or coverage ran. Sources: [Pillow 13.0 `Image.putalpha`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L1882-L1931), [`Image.putdata`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L1932-L1953), [`Image.putpixel`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L1997-L2035), and [`Image.load`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L3666-L3718).

2026-11-25 maintained Rust file-size source scan: the full first-party Rust
source listing initially found `pillow-rs/src/lib.rs` at 1,002 physical lines
after added public font exports. Removed only blank lines between its existing
font re-exports; all names, visibility attributes, and feature gates are
unchanged, and the file is now 923 lines. The current largest maintained file
is 993 lines. This read-only physical-line scan does not verify Clippy-counted
function bodies; that proof remains in the post-source-closure quality gate.
No formatter, checkstyle, lint, build, test, parity, benchmark, or coverage ran.

2026-11-25 public Transform strategy input follow-through (`pillow-rs#3ptb`):
the public mode template now adds an axis-aligned nearest translation for all
20 Pillow modes. It preserves a distinct in-bounds source pixel and a fill
column at the destination edge, so the public output checks the valid/fill
boundary under the CPU architecture selectors. The existing NaN case remains
a separate public output case; direct ImagingCore mutation has no independent
lane. The new templates have not been generated into parity
inputs, and output, fault, selected/fallback receipts, and latency remain
pending. No build, formatter, checkstyle, lint, test, parity, benchmark, or
coverage ran.

2026-11-25 public Transform alpha-filter scope correction (`pillow-rs#3ptb`):
Pillow 13's public `Image.transform` converts LA to La and RGBA to RGBa before
non-nearest filtering, then converts the result back to the original mode.
Therefore, the Geometry kernel's independent per-channel interpolation does
not define the public LA/RGBA result by itself. The existing wrapper converts
to the exact La/RGBa mode before routing Transform and converts back afterward;
that mode's strategy owns the sampling route. No leaf-level alpha flag change
is warranted from the Geometry source alone. The new LA/RGBA midpoint cases
remain useful to exercise the public premultiplication round trip. Case
generation, public output parity, mode/architecture selection receipts, and
latency remain pending. No build, formatter, checkstyle, lint, test, parity,
benchmark, or coverage ran. Source: [Pillow 13.0 public
`Image.transform`](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L2733-L2751).

2026-11-25 Reduce typed-mode source pass (`pillow-rs#3ptb`): checked the
exhaustive 25-mode classification/preparation against the exact-mode dispatch.
The four I;16 identities are separate CoreOnly/Default owners and call the
native `u16` reducer, which returns the same exact identity without mode
conversion. The ImageF32 Default reducer matches the archived F carrier's
2x2 float addition order, wider accumulation, partial-edge route, and output
identity. Its declared AVX, SSE2, NEON, and Wasm strategies match their
compile-target module gates; the NEON 2x2 specialization is admitted only for
complete blocks and otherwise falls back to the mode's portable wide route.
The straight-alpha LA/RGBA Default path retains the archived premultiply,
average, and unpremultiply ordering. This source pass does not cover all other
Reduce mode leaves or prove runtime architecture reachability. CoreOnly I;16
expected outputs, full Reduce mode/architecture parity, receipts, and latency
remain pending. No build, formatter, checkstyle, lint, test, parity, benchmark,
or coverage ran.

2026-11-25 ImageI32 Reduce source pass (`pillow-rs#3ptb`): its exact-mode
owner admits only the Default architecture and preserves `ImageI32` through
identity and reduced outputs. The scalar reducer matches the archived I
carrier's signed little-endian sample reads, wrapping pair/quartet accumulation
order for complete interior blocks, wider accumulation on partial edge blocks,
and shared rounding rule. No architecture-specific reducer is declared. This
is source review only; I-mode output parity, faults, architecture receipts, and
latency remain pending. No build, formatter, checkstyle, lint, test, parity,
benchmark, or coverage ran.

2026-11-25 ImageLuma8 Reduce source pass (`pillow-rs#3ptb`): the exact-mode
owner preserves ImageLuma8 and declares Default plus AVX2, SSE2, NEON, and
Wasm SIMD strategies under their matching target gates. Default routes through
the one-channel native byte reducer, retaining ceil-sized output, separate
full/right/bottom/corner block geometry, Pillow's reciprocal rounding, and the
rounded-division path when the reciprocal is zero. Vector routes admit only
valid native byte buffers and bounded factor areas; they process complete
eight-output groups through vector arithmetic and finish row tails with the
same native Default row reducer. Unsupported or ineligible requested
architectures select Default through the shared support declaration. The
source review does not refresh prior parity evidence: complete Luma8 mode and
architecture outputs, fallback receipts, and latency remain to be verified.
No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage ran.

2026-11-25 public One/P Reduce support review (`pillow-rs#3ptb`): Pillow
13.0's public `Image.reduce` returns a crop for factor `(1, 1)` and delegates
other factors to ImagingCore. Its imaging core rejects non-identity Reduce for
mode `1` and `P` with the public `ValueError` message `image has wrong mode`.
The Rust One and ImagePalette8 support declarations are therefore correctly
identity-only; adding byte averaging would diverge from the public method.
Factor-one copy/crop behavior and the non-identity fault's output, timing, and
Python-visible parity remain to be verified in the frozen CPU suite. Source:
[Pillow 13.0 Image.reduce](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L2239-L2270)
and [ImagingReduce mode rejection](https://github.com/python-pillow/Pillow/blob/13.0.0/src/libImaging/Reduce.c#L1415-L1423).
No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage ran.

2026-11-25 ImagePaletteAlpha8 (PA) Reduce source pass (`pillow-rs#3ptb`):
Pillow's public Image.reduce does not normalize PA before reduction. The Rust
PA Default leaf independently averages its stored palette-index and alpha bytes
through the native two-channel reducer, preserves ImagePaletteAlpha8 identity,
and declares no architecture strategy. Pipeline metadata retention keeps the
source palette/transparency alongside a preserved PA output. This is source
review only; public pixels, palette/transparency metadata, fault order, and
receipts remain to be verified in the frozen CPU matrix. Sources: [Pillow 13.0
Image.reduce](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L2239-L2270)
and [PA byte-channel reduction path](https://github.com/python-pillow/Pillow/blob/13.0.0/src/libImaging/Reduce.c#L47-L84).
No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage ran.

2026-11-25 straight-alpha LA/RGBA Reduce source pass (`pillow-rs#3ptb`):
Pillow 13's public `Image.reduce` converts LA to La and RGBA to RGBa, reduces
those premultiplied samples, then converts back to the original mode. The Rust
LA/RGBA owners preserve their respective `DynamicImage` identities while
fusing that same rounded premultiply → average → unpremultiply sequence into
their native two- and four-byte reducers. The RGBA Default path uses its
specialized four-channel sum; vector paths apply the same sample sequence,
with Default row handling for non-vector tails. Their declared AVX2, SSE2,
NEON, and Wasm strategies follow the exact build gates and share the native
byte request guard. This is source review only; public pixel output, alpha-zero
rounding, architecture/fallback receipts, and latency remain to be verified
in the frozen CPU suite. Source: [Pillow 13.0 public Image.reduce](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L2255-L2270).
No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage ran.

2026-11-25 ImageCmyk8 Reduce source pass (`pillow-rs#3ptb`): Pillow's
public `Image.reduce` delegates CMYK samples without a mode conversion. The
Rust exact-mode owner preserves `ImageCmyk8`; Default independently averages
its four native bytes, and the AVX2/SSE2/NEON/Wasm selectors use the same
four-channel contract under their target gates. Vector tails return to the
same native Default row reducer, while the shared byte guard excludes invalid
input buffers and factor areas that exceed the lane bound. This is source
review only; public output parity, fallback receipts, and latency remain to be
verified in the frozen CPU suite. Sources: [Pillow 13.0 public Image.reduce](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L2255-L2270)
and [four-byte channel reduction](https://github.com/python-pillow/Pillow/blob/13.0.0/src/libImaging/Reduce.c#L128-L179).
No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage ran.

2026-11-25 CoreOnly typed Reduce source pass (`pillow-rs#3ptb`):
LumaA16, RGB16, RGBA16, RGB32F, and RGBA32F each have an exact-mode
cpu_core_only support declaration, a single Default strategy, and a typed
native reducer that preserves the same `DynamicImage` identity. U16 channels
are independently accumulated in `u128` and rounded half-up; f32 channels are
independently accumulated in `f64` and returned as f32. No mode casts or
Pillow-carrier projections are used. These are explicit `NO_PILLOW_SUPPORT`
Rust extensions, so successful output verification must compare against
recorded typed expected outputs rather than Pillow. Those expected-output
cases and latency remain pending. No build, formatter, checkstyle, lint, test,
parity, benchmark, or coverage ran.

2026-11-25 ImageRgbx8 Reduce source pass (`pillow-rs#3ptb`): RGBX's
fourth byte is part of its exact public four-byte raster, not an omitted lane
in the Rust carrier. Pillow's public `Image.reduce` delegates RGBX without a
mode conversion, and its four-band output averages the stored X byte along
with RGB. The Rust RGBX Default leaf averages all four bytes and returns
`ImageRgbx8`; `hidden_pixel_lane_count` is `None` for this full four-byte
carrier, so hidden-lane completion leaves that computed byte intact. This is
source review only; all four public bytes and exact-mode output remain to be
verified in the frozen CPU matrix. Sources: [Pillow 13.0 public Image.reduce](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L2255-L2270)
and [four-band reduction](https://github.com/python-pillow/Pillow/blob/13.0.0/src/libImaging/Reduce.c#L128-L179).
No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage ran.

2026-11-25 ImageRgb8 Reduce source pass (`pillow-rs#3ptb`): the public
`Image.reduce` route keeps RGB samples in RGB and returns the same mode. Rust's
Default strategy reduces the three native channels with Pillow's fixed-point
rounding and separate full/edge block divisors; the sparse even 2x2 path and
dense 2x2 specialization preserve the same per-channel means. AVX2 uses its
mode-specialized vector kernel, while SSE2, NEON, and Wasm SIMD use the shared
wide-lane kernel; vector tails use the same native block context. The common
sparse 2x2 helper may run under any selected architecture. Prepared support
excludes identity/empty or oversized vector requests so they fall back to
Default. This is source review only; the RGB TODO remains for public output,
fallback receipts, and latency in the frozen CPU gate. Sources: [Pillow 13.0
public Image.reduce](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L2239-L2270)
and [RGB channel reduction](https://github.com/python-pillow/Pillow/blob/13.0.0/src/libImaging/Reduce.c#L85-L127).
No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage ran.

2026-11-25 ImageYCbCr8 Reduce source pass (`pillow-rs#3ptb`): Pillow's
public reducer keeps YCbCr samples in YCbCr and reduces all three stored bytes
independently. The Rust Default leaf uses the shared native-byte block reducer;
AVX2 uses the exact three-channel byte kernel, and SSE2, NEON, and Wasm SIMD
use its portable wide-lane route. All preserve `ImageYCbCr8`; architecture
guards and prepared selection retain Default for identity, empty-output, and
out-of-bound requests. This is source review only; public bytes, edge/tail
cases, fallback receipts, and latency remain pending the frozen CPU gate.
Sources: [Pillow 13.0 public Image.reduce](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L2239-L2270)
and [three-band native reduction](https://github.com/python-pillow/Pillow/blob/13.0.0/src/libImaging/Reduce.c#L85-L127).
No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage ran.

2026-11-25 ImageLab8 Reduce source pass (`pillow-rs#3ptb`): public
`Image.reduce` keeps LAB in its original mode, and Pillow's native three-band
reducer averages the stored bytes directly. Rust preserves `ImageLab8` and
uses the native three-channel Default reducer, including the stored chroma
bytes; its support declaration has no architecture specializations, so
non-Default architecture requests retain a same-mode Default selection and
receipt. Public byte/mode parity, fallback receipts, and latency remain
pending the frozen CPU gate. Sources: [Pillow 13.0 public Image.reduce](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L2263-L2270)
and [three-band native reduction](https://github.com/python-pillow/Pillow/blob/13.0.0/src/libImaging/Reduce.c#L85-L127).
No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage ran.

2026-11-25 premultiplied La/RGBa Reduce source pass (`pillow-rs#3ptb`):
Pillow's public `Image.reduce` converts only straight `LA` and `RGBA` before
filtering; premultiplied `La` and `RGBa` go directly to the reducer and retain
their modes. Rust's separate exact-mode owners preserve those identities and
independently average the two stored La samples or all four RGBa samples in
Default. Neither owner declares a vector specialization, so architecture
requests retain same-mode Default selections. Public samples/modes, fallback
receipts, and latency remain pending the frozen CPU gate. Sources: [Pillow 13.0 public Image.reduce](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L2263-L2270),
[two-band reduction](https://github.com/python-pillow/Pillow/blob/13.0.0/src/libImaging/Reduce.c#L47-L84),
and [four-band reduction](https://github.com/python-pillow/Pillow/blob/13.0.0/src/libImaging/Reduce.c#L128-L179).
No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage ran.

2026-11-25 ImageHsv8 Reduce source pass (`pillow-rs#3ptb`): Pillow's public
`Image.reduce` leaves HSV in HSV. The Rust exact-mode owner preserves
`ImageHsv8` and applies native three-byte averaging for Default, AVX2, SSE2,
NEON, and Wasm SIMD; vector tails use the shared block context and the
operation guard retains Default for ineligible architecture requests. The
public output, edge/tail cases, fallback receipts, and latency remain pending
the frozen CPU gate. Sources: [Pillow 13.0 public Image.reduce](https://github.com/python-pillow/Pillow/blob/13.0.0/src/PIL/Image.py#L2263-L2270)
and [three-band native reduction](https://github.com/python-pillow/Pillow/blob/13.0.0/src/libImaging/Reduce.c#L85-L127).
No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage ran.

2026-11-25 Reduce exact-mode source review closure (`pillow-rs#3ptb`): all
25 maintained mode owners now have an exact-mode source pass covering their
public mode result, Default route, declared architecture routes, and
same-mode fallback selection where applicable. This closes only the Reduce
mode-owner source review; it does not close the operation's parser/wrapper
fault order, runtime parity, architecture receipts, or latency. Per-mode TODOs
remain authoritative for those outputs and evidence. No build, formatter,
checkstyle, lint, test, parity, benchmark, or coverage ran.

2026-11-25 AVX-512F four-byte Reduce strategies (`pillow-rs#3ptb`): added
exact RGBA, RGBa, RGBX, and CMYK leaves for 2x2 blocks with complete 16-pixel
output rows. RGBA preserves Pillow's straight-alpha arithmetic; the other
modes average all four stored lanes. Mode owners declare their own AVX-512F
strategy and retain lower-ISA or Default fallback routes. Complete-block,
fallback, and material benchmark inputs are declared but have not been
generated or executed. Native hardware builds, route receipts, parity, and
latency remain pending; no build, formatter, checkstyle, lint, test, parity,
benchmark, or coverage ran.

2026-11-25 AVX-512F Chops Invert source pass (`pillow-rs#3ptb`): added an
operation-specific architecture support entry and exact-mode adapters for
One, L, LA, P, PA, RGB, RGBA, and CMYK. The adapters reuse the shared
`pool_cpu/helpers/bytes/native_invert/avx512f.rs` leaf and preserve each
`DynamicImage` identity; Chops inverts every stored channel, including alpha.
The existing row-width guard selects AVX-512F only when a row contains a full
64-byte block, leaving narrower rows on their declared lower strategy. The
generator now defines below-block, full-block, and row-tail cases for each of
the eight modes. Inputs have not been generated, and output parity, fallback
receipts, AVX-512 hardware builds, and per-mode latency remain pending. No
build, formatter, checkstyle, lint, test, parity, benchmark, or coverage ran.
