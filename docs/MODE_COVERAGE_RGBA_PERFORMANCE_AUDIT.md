# Mode Coverage, RGBA Conversion, and Performance Audit

**Date:** 2026-09-30
**Checkout:** /Users/lazytrot/work/pillow-rs
**Purpose:** Find places where a logical image mode is not handled directly, where general RGBA staging or redundant copies cost work, and where a generic implementation may be masking missing mode-specific paths.

## Scope and method

This is a static review of first-party image code in pillow-rs core, CPU/SIMD/GPU execution, GPU shader assets, and the Python and JavaScript runtime bindings. It excludes node_modules, generated packages, fixtures, and the separate fontdone checkout. Reviews were split across core operations and raster storage, backend execution, language bindings, and WGSL shaders.

The initial audit was static and made no source changes or test/benchmark runs. Its cost estimates came from visible allocations, copies, and byte layouts, not measured timing. Source code can show that a general RGBA path is used, but it cannot establish that maintainers selected it to reduce implementation size. Findings describe observable behavior and candidate work, not inferred author intent; verified follow-up results are recorded separately below.

The checkout already has several uncommitted changes. They were treated as user-owned. In particular, the current changes add native PA masked-paste handling, so PA is not listed as an outstanding gap.

## Prioritized findings

| ID | Priority | Area | Finding |
|---|---|---|---|
| M1 | High | Core resize | Filtered resize treats several typed buffers as four byte channels. |
| M2 | Medium | Core split | Split counts the RGBA storage carrier for logical I/F scalar images. |
| P1 | High | GPU transport | General GPU batches widen many source modes and read back as RGBA8. |
| P2 | Medium | GPU operands | Secondary GPU images also get RGBA8 conversion and storage. |
| P3 | Medium | Quantization | Median-cut builds two full RGB views, including an unused one when kmeans is off. |
| P4 | Medium | Scalar conversions | Source-carrier clones are removed; serial CPU I→L still trails Pillow. |
| P5 | Medium | Python data access | Multiband getdata iteration slices the compact byte buffer per pixel; target slicing also diverged from Pillow. |
| P6 | High | Array input | Contiguous L/RGBA fromarray inputs are copied instead of sharing Pillow's read-only buffer view. |
| P7 | Low–Medium | Color transforms | RGB→HSV, RGB→YCbCr, and HSV→RGB now borrow; YCbCr→RGB copy-removal is checkpointed after three variants showed no repeatable whole-call gain. |
| P8 | Conditional | Masked Paste | Several same-mode byte formats missed native masked paths; HSV is now native on CPU, SIMD, and GPU. Other candidates remain. |
| P9 | Medium | GPU shaders | Several mode-aware L/LA kernels express four-channel arithmetic or sorting before selecting the channels they use. |
| P10 | Low–Medium | JavaScript data access | Formatted multiband getdata builds a nested JavaScript array and a per-pixel array. |

### M1. Filtered resize does not preserve several typed pixel layouts

In pillow-rs/src/ops/pil_resize.rs:2006–2011, only ImageLuma16 enters a typed resize implementation. The following channel count in :2037–2043 recognizes L8, LA8, and RGB8, then assumes four channels for every other DynamicImage. The filtered passes at :2161–2189 read img.as_bytes() using that byte-channel layout, and pil_preserve_mode at :1964–1975 only restores 8-bit source types.

The decoder can produce LA16, RGB16, RGBA16, RGB32F, and RGBA32F DynamicImage variants (pillow-rs/src/raster/dynamic.rs:953–991; pillow-rs/src/image.rs:6947–6955). Those inputs have two-byte or float samples, so interpreting their storage as four byte channels can misindex samples and return a byte RGBA result. The boxed resize path has the same typed-layout risk (pil_resize.rs:2565–2591, 2700–2735). Nearest resize uses a separate pixel-access path; this finding is about filtered resize.

**Assessment:** high-confidence layout mismatch in the current implementation; public reachability and exact codec coverage should be confirmed with parity cases before fixing.

### M2. split() derives band count from storage type for I/F

pillow-rs/src/ops/split.rs:35–47 derives band count from img.color(). Logical I and F are stored internally in ImageRgba8 as one four-byte scalar, so the fallback count is four and split queues four byte ExtractBand operations.

The repository already defines I/F as one-band modes in pillow-rs/src/image.rs:998–1004. Its getchannel implementation also explicitly avoids exposing the four storage bytes for I/F at :5862–5868. Pillow's Image.split implementation returns a copy when the core image has one band, preserving the single-band image contract ([Pillow Image.split source](https://github.com/python-pillow/Pillow/blob/main/src/PIL/Image.py)).

**Assessment:** likely mode-contract bug for I/F, with medium severity because these modes are a narrower workload. Confirm output mode and scalar bytes in parity before selecting the repair path.

### P1. General GPU batches use RGBA8 transport for many native modes

The ordinary GPU input path in pillow-rs/src/compute/pool_gpu/mod.rs:4288–4302 preserves RGBA but calls to_rgba8() for other layouts. RGB has its own uploader, but it expands each three-byte pixel into four-byte staging data (:4264–4285). The pipeline selects a set of packed-L, native-channel, and typed exceptions before falling through to upload_standard_image (:21803–21865).

Generic GPU readback is four bytes per pixel (:12385–12410), and output mode preservation then narrows or repacks the result. Thus a non-specialized L route can move four bytes per pixel in each direction instead of one; LA moves four instead of two. The input conversion or staging work is also visible on the host. This is an optimization and bandwidth opportunity, not evidence of a semantic error: each native kernel must retain its mode contract.

**Assessment:** high confidence in the extra data movement; high priority for large or bandwidth-bound operations. The native exceptions are operation-specific, so this is a roadmap item rather than a single safe global switch.

### P2. GPU auxiliary images also normalize to RGBA8

pillow-rs/src/compute/pool_gpu/mod.rs:5222–5251 converts every non-RGBA image in pack_dynamic_rgba and append_dynamic_rgba. Ordinary second and third operands reach these helpers at :8727–8795, with separate cases for some typed data and tables.

This adds a full-frame converted buffer and four-byte-per-pixel auxiliary storage for modes whose shader path does not have a native operand representation.

**Assessment:** high confidence in the staging cost; medium priority, depending on image size and reuse. A compact operand layout is operation-specific because masks, alpha, and scalar samples have different contracts.

### P3. Median-cut quantization duplicates native RGB data — partially addressed

The median-cut route in `pillow-rs/src/ops/quantize.rs` now borrows the byte
slice for concrete RGB8 storage when the logical mode is unspecified or RGB.
It passes those bytes directly to `median_cut_quantize_rgb`, avoiding both the
same-format `to_rgb8()` clone and the flattened byte vector. It creates the
`Vec<[u8; 3]>` only when `kmeans > 0`, where refinement actually consumes it.
Other modes retain the reference-compatible RGB conversion. The separate
MAXCOVERAGE method still materializes its converted RGB buffer.

**Assessment:** the redundant full-frame buffers are removed from native RGB
median-cut with `kmeans=0`; the benchmark evidence does not establish a
repeatable latency win. See the 2026-10-01 checkpoint in
`PERFORMANCE_CAMPAIGN.md`. Keep P3 open for measured impact and remaining
quantization paths rather than treating fewer allocations as proof of speed.

### P4. I/F color conversions clone the source carrier — CPU path addressed

Public I/F images already use `ImageRgba8` as their four-byte scalar carrier.
`i_to_rgb` and `f_to_rgb` already borrow that carrier. The remaining helpers
`i_to_l`, `f_to_l`, `i_to_f`, and `f_to_i` now borrow `as_raw()` for matching
`ImageRgba8` inputs while retaining the previous conversion fallback for
other physical variants. The L outputs also avoid zero-filling an image that
is immediately overwritten. The numeric conversion and little-endian byte
contracts are unchanged.

**Assessment:** the avoidable source copies are removed from the four remaining
helpers. Strict parity passed for all four public conversions. I→L now shares
the validated I carrier instead of cloning it, then reads aligned little-endian
words directly when possible. On 1024 × 768, the conversion phase measures
0.041–0.043 ms against Pillow at 0.089–0.097 ms; the full workflow measures
0.217–0.218 ms against Pillow at 0.456–0.507 ms. The CPU latency gap is closed.
The eager color helpers run outside backend dispatch and schedule no Rayon, so
SIMD, GPU, and Parallel CPU profiles do not represent those executors. Details
and receipts are in `PERFORMANCE_CAMPAIGN.md`.

### P5. Python getdata access — corrected parity and bulk iteration

The initial audit treated slicing as supported because `_ImageDataSequence`
implemented it by calling `getpixel_formatted` for every index. A direct probe
against the pinned Pillow 12.2 oracle showed that `Image.getdata()[slice]`
raises `TypeError: sequence index must be integer, not 'slice'`. Returning a
list was therefore both slower and a parity bug. The wrapper now matches
Pillow's slice, invalid-index-type, and positive out-of-range errors; it keeps
ordinary integer access and live-view behavior.

The valid full-iteration path for multiband bytes previously sliced a new
`bytes` object for every pixel before building a tuple. It now uses
`struct.iter_unpack` over the original buffer. A fallback retains the previous
trailing-byte behavior for malformed internal buffers. This path operates on
native L/LA/RGB/RGBA bytes and performs no mode conversion.

**Assessment:** the unsupported-slice mismatch is fixed, and supported
multiband iteration no longer allocates one byte slice per pixel. In paired
256 × 256 full-materialization probes, all measured L, LA, RGB, and RGBA outputs
matched Pillow and CPU medians were faster; RGB improved from 6.93 ms to about
2.65 ms after the iterator change. These host-side timings do not prove SIMD,
GPU, or Parallel CPU execution. Full evidence is in
`PERFORMANCE_CAMPAIGN.md`.

### P6. `fromarray` misses Pillow's shared-buffer path — parity blocker

The binding calls `memoryview(...).tobytes()`, extracts a Python-bytes copy into
`Vec<u8>`, then core `Image::frombytes` copies borrowed bytes into owned raster
storage. An owned-vector constructor exists, but it is not used here. That
three-buffer path is visible in `pillow-rs-py/src/lib.rs` and
`pillow-rs/src/ops/array.rs` / `pillow-rs/src/image.rs`.

The performance gap is larger than those copies explain. Pillow's contiguous
array route delegates to `frombuffer`; for supported modes such as L and RGBA,
Pillow holds a read-only image view over the exporter. A 1024 × 768 NumPy
array probe confirmed `readonly == 1` and that changing the source array is
visible through the image until the image itself is mutated. pillow-rs eagerly
copies the array, reports no read-only view, and does not observe later source
array writes. Matching initial pixels is not complete parity.

Three candidate shortcuts do not solve this safely: moving the extracted
`Vec<u8>` into core removes one copy but still detaches from the source array;
retaining the Python owner in `PyImage` leaves the core raster stale; rebuilding
the raster before each API call preserves some reads but copies repeatedly and
complicates Pillow's detach-on-image-write behavior. A viable fast path needs a
core storage representation that retains the exported buffer's owner and
stable lifetime, reads external writes, and detaches on image mutation without
creating unsynchronized access. No code change was retained. Keep P6 blocked
until that ownership contract is designed and parity-tested.

### P7. YCbCr→RGB copy-removal attempt checkpointed

The initial inventory overstated the affected functions: `rgb_to_hsv` already
borrowed native RGB storage. `rgb_to_ycbcr` now borrows `ImageRgb8`, reserves
the final byte buffer, and writes the existing Pillow-rounded triples without
cloning the source or zero-filling `RgbImage::new`. `hsv_to_rgb` now borrows
its HSV bytes and uses a 256×256 hue/saturation factor table initialized once;
the per-pixel loop no longer divides, computes the hue sector, branches, or
selects a sector. The table stores the same `f32` factors and preserves the
final multiply, round, clamp, and channel order. Both functions retain the
existing `to_rgb8()` conversion for other physical input variants.
`ycbcr_to_rgb` also reads YCbCr triplets from `ImageRgb8`; its current path
still clones them through `to_rgb8()` and zero-fills `RgbImage::new` before
overwriting every output pixel.

RGB→YCbCr, nonzero RGB→YCbCr, and the RGB→HSV control pass on CPU, strict SIMD,
and strict GPU (3/3 per backend). For RGB→YCbCr, an interleaved seven-pair
comparison of the exact pre-change and candidate CPU extensions used
1024×768 deterministic RGB input, three warmups, nine samples of five full
`convert("YCbCr").tobytes()` calls, and complete output digests. The digests all
matched. Median latency was 2246.7 μs before and 2189.7 μs after, a 2.5% paired
reduction; candidate CPU measured 0.87× Pillow latency.

HSV→RGB passes seven focused sector and nonzero cases on CPU, strict SIMD, and
strict GPU (7/7 per backend). A separate exhaustive CPU/Pillow comparison
covered every one of the 16,777,216 HSV byte triples; the full 50,331,648-byte
RGB outputs had identical SHA-256 digests. For 1024×768 materialized
`convert("RGB")`, a seven-pair interleaved comparison of the direct-float CPU
baseline and factor-table candidate produced identical output digests. The
candidate/baseline warm-latency ratio was 0.495, and candidate CPU latency was
0.77× Pillow (about 1.29× faster). The cold first conversion, including table
initialization, was also faster than Pillow. A five-size run found the CPU
faster than Pillow from 1×1 through 1024×768, both for the cold first call and
warm repeated calls. These are single-caller CPU latency results; neither
operation changed SIMD/GPU code, and no concurrent-throughput claim is made.

YCbCr→RGB was benchmarked separately at 1024×768 using deterministic,
nonuniform YCbCr bytes. Input construction was outside the timer; each process
used three warmups and nine samples of five materialized
`convert("RGB").tobytes()` calls. Seven interleaved Pillow/target process pairs
used identical inputs, and all output SHA-256 digests matched. The pre-change
median was 1.410 ms versus Pillow at 1.485 ms (0.95× Pillow latency). Three
native-source borrow variants measured 1.566 ms (byte-slice input plus reserved
output vector), 1.455 ms (byte-slice input with the existing image output),
and 1.467 ms (typed-image borrow with the existing pixel loop), against paired
Pillow medians of 1.521, 1.463, and 1.476 ms respectively. The vector-output
variant regressed; the other two were effectively at Pillow latency and did
not establish a repeatable gain over the old implementation. All three
variants were discarded. Keep YCbCr→RGB checkpointed; do not retain a copy
reduction based on allocation counts alone.

**Assessment:** RGB→YCbCr removes two full-frame memory passes and measured a
small CPU gain. HSV→RGB replaces repeated per-pixel hue/saturation arithmetic
with a compact factor lookup and roughly halves latency versus its direct-float
baseline while preserving every possible byte input. For YCbCr→RGB, the typed
source borrow did not pay off reliably at the full-call boundary; the existing
serial CPU path remained at or faster than Pillow in the recorded runs. No
SIMD- or GPU-specific conversion kernel was changed or measured, and no
concurrent-throughput claim is made. Move on after this bounded three-variant
checkpoint rather than spending more time on an unproven copy-removal tweak.

### P8. Masked Paste had same-mode byte formats outside native paths

The initial audit found same-mode images that missed native masked Paste. HSV is stored in the same three-byte `ImageRgb8` carrier as RGB, but CPU and strict-SIMD admission excluded HSV. The GPU native planner admitted RGB only, so HSV reached the generic converted path. The RGB and HSV contracts blend their stored channels independently with an L mask; they do not require RGBA staging or color-space conversion. HSV now uses native three-byte paths on CPU, SIMD, and GPU, including zero GPU mode conversions. Same-mode `RGBa`, `RGBX`, `YCbCr`, and `LAB` remain candidates for separate contract checks. Indexed and premultiplied modes need their own semantics; I/F and I;16 are typed-sample cases rather than byte-channel candidates.

**Assessment:** the HSV storage and fallback gap was confirmed and fixed without changing its logical mode. The focused HSV and shared RGB parity workloads pass on CPU, strict SIMD, and GPU. Other omitted formats remain unverified and should not be admitted by analogy alone.

### P9. Some mode-aware GPU shaders still express work on unused channels

The shaders often compute or load all four packed channels and then select the channels required by the logical mode. For example, the 3x3 and 5x5 convolution shaders form per-channel results before mode selection (`filter_3x3.wgsl:240`, `filter_5x5.wgsl:308`); box and Gaussian blur kernels accumulate R/G/B/A for each sample before selecting (`box_blur_h.wgsl:60`, `box_blur_v.wgsl:58`, `box_blur.wgsl:48`, `gaussian_blur.wgsl:75`); and median/rank filters allocate, fill, and sort four channel arrays (`median_filter.wgsl:98`, `rank_filter.wgsl:101`). Resize convolution constructs four filtered component values (`resize_convolution_h.wgsl:1216`, `resize_convolution_v.wgsl:1258`). L uses only one component and LA uses two logical components, though LA alpha is transported in byte 3. Thumbnail, contain, cover, fit, and rotate shaders have similar packed-channel sampling/interpolation patterns.

Some mode-specific packed kernels exist, but admission is narrow: GPU has packed-L specializations for limited GaussianBlur/BoxBlur and MedianFilter/RankFilter cases, and native two-channel LA blur kernels. The 2026-10-02 LA MedianFilter(3) checkpoint adds a native LA GPU path: each word stores two `[L, A]` pixels, and each vector lane processes one of those two pixels' channels. It keeps the alpha channel independent, accounts for odd-width row crossings, and reads/writes native LA bytes. On seeded 1024 × 768 LA noise, it reduced median latency from 5.198 ms to 2.284 ms, transfer bytes from 3,145,728 to 1,572,864 in each direction, and mode conversions from one to zero. Exact parity passed against Pillow, CPU, and SIMD; an odd-width GPU test also matched every CPU byte.

**Assessment:** the general kernels still express extra channel work, and measurements now confirm a material cost for LA MedianFilter(3) on this adapter. Other convolution, resize, and thumbnail paths remain unmeasured; profile each format-specific workload before generalizing. Preserve each operation's rounding, edge behavior, and alpha semantics.

### P10. JavaScript formatted multiband data creates per-pixel arrays

`formatted_image_data_to_js` (`pillow-rs-js/src/lib.rs:76–102`) walks the formatted component vectors and creates a new JavaScript array for the whole result plus another JavaScript array for each pixel, pushing channel values one at a time. This matches the nested-array result shape, but it creates O(pixel count) JS arrays for multiband data. The byte-oriented `getdata` method at :1673–1675 provides a flat path when callers can consume storage bytes instead of formatted pixel tuples.

**Assessment:** high confidence in the allocation pattern; low-to-medium priority and constrained by the existing formatted API's return shape. A separately named flat typed-array API could serve large-image consumers without changing the existing contract.

## Backend mode coverage that is intentionally restricted

CPU is documented as the universal PipelineOp fallback (pillow-rs/src/compute/pool_cpu/mod.rs:16). SIMD adapters use explicit mode/layout allowlists, and the contextual capability gate prevents scalar-only implementations from being admitted as SIMD. The review found no admitted SIMD path that silently executes its scalar fallback.

GPU has explicit mode/operation restrictions. For example, gpu_operation_mode_requires_cpu routes AlphaComposite on L/RGB, Posterize/Solarize on LA/RGBA, Colorize outside L, and Autocontrast/Equalize outside L/RGB to CPU (pool_gpu/mod.rs:17033–17095). L16 geometry and filtered resize have narrow exactness admissions (:15587–15635), and F/I filtering requires typed-sample-specific predicates (:16235–16285). These are confirmed acceleration-coverage limits, but the code comments explain semantic differences, so they are not automatically missing implementations.

Several established RGBA conversions are intentional: requested convert("RGBA") results, palette quantization inputs, and Qt's RGBA/BGRA presentation mapping. They are not counted as avoidable paths here.

## Checkout state correction

The conflict list recorded by the earlier audit snapshot is stale for the main
checkout evaluated on 2026-09-30. The index has no unmerged entries, and the
listed files contain no literal conflict markers. Conflict reconciliation is
not a current blocker; preserve the files and proceed with the findings below.

## Follow-up result: single-band `split()`

The I/F split finding was confirmed against the pinned Pillow 12.2.0 oracle.
For a scalar image, Pillow returns one image in the original mode with the
original sample bytes. Before the fix, pillow-rs returned four L images by
splitting the RGBA storage carrier. The same logical-versus-physical band
mismatch also affected packed mode 1 and `I;16`, `I;16L`, `I;16B`, and `I;16N`.

`Image::split()` now uses `getbands()` to detect a single logical band and
returns a same-mode materialized branch. The branch shares immutable raster
storage and detaches on mutation; it avoids copying the full source buffer and
avoids byte-oriented channel extraction. Regression coverage checks exact
modes and bytes for those formats, mutation isolation, in-memory I/F images,
and I/F images opened from TIFF.

The generated parity cases passed on CPU, SIMD, and GPU selections: 18/18 on
each. The focused Rust test and generated-input checks also passed. No coverage
was collected.

Two 1024×768 call-only split workloads were added to the correctness-gated
benchmark. The maintained quick-profile command measured six workloads in
total; all six completed and the associated parity receipt passed 10/10. Each
split workload collected 100 samples per subject:

| Mode | Pillow median | CPU median | SIMD median | GPU-requested median |
|---|---:|---:|---:|---:|
| I | 0.204646 ms | 0.001917 ms (106.8× faster) | 0.001958 ms (104.5× faster) | 0.001917 ms |
| F | 0.186292 ms | 0.001917 ms (97.2× faster) | 0.001958 ms (95.1× faster) | 0.001917 ms |

The GPU-requested timing is not evidence of GPU execution: its receipt has no
actual backend or dispatch. A one-band split now shares an immutable CPU-side
buffer in constant time, so SIMD arithmetic and GPU dispatch cannot improve
this operation without adding work. This mode-specific result meets the CPU
and SIMD latency goals; GPU acceleration is not applicable to the path.

## Follow-up result: typed filtered resize reachability

The M1 layout mismatch is real inside `pil_resize()` if it receives a synthetic
`DynamicImage::ImageLumaA16`, `ImageRgb16`, `ImageRgba16`, `ImageRgb32F`, or
`ImageRgba32F`. Those representations are not currently reachable through the
public Pillow-style resize API, however:

- The pinned Pillow 12.2.0 oracle rejects `LA;16`, `RGB;16`, `RGBA;16`,
  `RGB32F`, and `RGBA32F` in `Image.frombytes()` as unrecognized modes. It
  accepts `I;16` and `F`, which use pillow-rs's existing scalar-specific paths.
- The pinned `image-slash-star` decoder emits `L16`, `I32`, and `F32` for the
  corresponding high-depth scalar TIFF layouts, but does not emit `La16`,
  `Rgb16`, `Rgba16`, `Rgb32F`, or `Rgba32F`. Its 16-bit multi-channel PNG
  inputs normalize to the same 8-bit public modes as Pillow, including
  grayscale-alpha PNG loading as RGBA.
- `DynamicImage::from_decoded()` can construct the typed variants from a
  synthetic `DecodedImage`, but `pil_resize()` and the `ops` module are private.
  Public `Image` constructors and current decoders do not provide a route to
  pass those variants into filtered resize.

The reachability check used the isolated Pillow 12.2.0 environment and
`make build-parity`. Pillow and pillow-rs loaded the same 16-bit LA, RGB, and
RGBA PNG samples; decoded modes and filtered resize bytes agreed. No parity
fixture or implementation change was added because Pillow has no public mode
contract for these synthetic layouts. Close M1 for the current public surface.
If a supported decoder begins returning one of these variants, add a
native-sample resize implementation and mode-specific parity cases first.

## Follow-up status — 2026-10-02

P1 remains open across the general GPU operation set. The Cover path is a
verified partial reduction: LA source upload stays native at two bytes per
pixel, and its vertical resize result now reads back packed LA instead of
four-byte RGBA. On the maintained material LA Cover workload, strict CPU,
SIMD, and GPU parity passed 3/3; GPU used two dispatches, had no fallback, and
reduced readback from 5,591,040 to 2,797,568 bytes. This does not remove the
generic RGBA transport from other operations or modes. CPU and SIMD latency
have since improved: a direct two-byte CPU path now avoids the generic i64
channel loops and intermediate transpose, making serial CPU about 1.9× faster
than Pillow. SIMD now premultiplies each LA source row once instead of
recomputing luma×alpha per filter tap, but remains about 1.1× slower than
Pillow and far below the 5× target. GPU source was unchanged and is already
faster than SIMD for this single-request case. See the measured checkpoint in
`PERFORMANCE_CAMPAIGN.md`.

L AutoContrast is another operation-specific P1 reduction: its lowered
single-LUT route now uploads and reads back native one-byte L instead of RGBA,
removing 3/4 of the transfer bytes and the mode conversion. Strict CPU, SIMD,
and GPU parity passed for materialized L and cutoff-clamped L. At queue depth 1,
the measured GPU latency improved from 2.109 ms to 0.476 ms; its throughput is
still behind SIMD at queue depths 2 and 4, and the generic RGBA transport
remains for other operations and modes. See `PERFORMANCE_CAMPAIGN.md` for the
three-attempt measurements and exact commands.

RGB `Image.transform` now uploads packed native triples and both affine and
projective transform shaders reconstruct RGB samples on demand. At 1024 × 768,
this cuts input transfer by 25% and removes host-side RGB-to-RGBA widening;
readback remains four bytes per output pixel. Exact CPU/SIMD/GPU parity passed
14,400/14,400 measured outputs in each of two runs. The q1 latency medians were
lower in both candidate runs, but throughput did not improve consistently and
GPU remained slower than SIMD. Keep this as a partial P1 transport win and move
on; P1 remains open for other operations and modes. See
`PERFORMANCE_CAMPAIGN.md` for timings and receipts.

RGB `ImageOps.autocontrast` now maps each native RGB triplet directly into its
output on serial CPU instead of cloning then rescanning bytes with per-byte
channel selection. On the 1024 × 768 material case, CPU throughput increased
from 354.2 to 774.3 images/s and every measured CPU/SIMD/GPU output matched
live Pillow. Its GPU path now admits only the host-derived unmasked RGB LUT to
a compact three-byte upload, channel-specific packed-LUT shader, and compact
three-byte readback; this cuts both transfers from 3,145,728 to 2,359,296
bytes and removes the input mode conversion. Repeated q1 runs put GPU at
1.04–1.14× SIMD throughput with exact Pillow parity. The exact-source q2
result is even with SIMD; q4 is still 18% slower, with earlier runs varying
from 0.77× to 1.08×. Serial CPU is faster than Pillow on this case; SIMD
remains about 1.3× Pillow, far below the 5× target. See
`PERFORMANCE_CAMPAIGN.md` for the route details, all queue-depth measurements,
and receipts.

L `ImageOps.cover` now has a direct single-byte CPU resize path guarded by
coefficient and source-extent bounds. The final 1024 × 768 material workload
passes CPU, SIMD, and GPU parity; serial CPU latency is 1.955 ms against
Pillow's 4.840 ms, and GPU is 0.783 ms with two dispatches, native L upload,
packed output, and zero mode conversions. The SIMD path remains only 1.07×
Pillow; its fourth vector-kernel trial did not show a repeatable whole-call
gain and was discarded. This closes the CPU target and confirms the existing
native GPU route for this specific L Cover case, not P1 across the operation
set. Detailed attempts and receipts are in `PERFORMANCE_CAMPAIGN.md`.

RGB `ImageFilter.MaxFilter(3)` now has an operation-specific compact RGB GPU
route: it uploads and reads back packed RGB triples, removing the general
RGBA widening and four-byte-per-pixel transport at both boundaries. Its
row-tiled shader reuses overlapping horizontal samples for four output pixels
while preserving independent channel maxima and clamped edges. On the
material 1024 × 768 RGB workload, strict CPU, SIMD, and GPU parity passed; GPU
upload/readback fell from 3,145,728 to 2,359,296 bytes and conversion count
from one to zero. This is another partial P1 reduction, limited to RGB
MaxFilter size 3; other modes and filter sizes remain operation-specific.
Timings and exact receipts are in `PERFORMANCE_CAMPAIGN.md`.

Native-L `ImageFilter.MaxFilter(3)` now uses the existing packed-L upload and
readback contract with a one-byte maximum shader. One invocation owns each
four-sample output word; clamped edges, odd widths, and the final partial word
are covered. On the material 1024 × 768 L workload, CPU, strict SIMD, strict
GPU, and opt-in Parallel CPU each passed all three Pillow parity cases (1 × 1,
33 × 35, and material size). GPU upload and readback each fell from 3,145,728
to 786,432 bytes, mode conversions fell from one to zero, and latency moved
from 2.384 ms to 1.028 ms. CPU was 3.14× faster than Pillow, SIMD 7.02× faster,
and GPU latency was 4.43× lower than SIMD with 4.43× its measured throughput.
The L MaxFilter(3) case meets the measured backend goals; P1 remains open for
other operations, modes, and filter sizes. Details and exact receipts are in
`PERFORMANCE_CAMPAIGN.md`.

Native-L `ImageFilter.MinFilter(3)` now uses a corresponding packed-byte
minimum shader on the same checked singleton route. CPU, strict SIMD, strict
GPU, and opt-in Parallel CPU each passed all three live-Pillow cases (1 × 1,
33 × 35, and 1024 × 768). On the material input, GPU upload/readback dropped
from 3,145,728 to 786,432 bytes each, mode conversions dropped from one to
zero, and median latency fell from 2.292 ms to 1.030 ms. Serial CPU was 3.11×
faster than Pillow, SIMD 6.98× faster, and GPU was 4.26× lower latency than
SIMD with 4.26× its measured serial-call throughput. This closes the measured
L MinFilter(3) case only; other modes and filter sizes keep their own proofs.
See `PERFORMANCE_CAMPAIGN.md` for the separate Parallel CPU result and exact
receipts.

P3 is partially addressed on main. The median-cut path borrows bytes for native
RGB and avoids the unused tuple and flattened-byte buffers when `kmeans=0`;
non-RGB modes retain conversion. Focused parity passed for default median-cut,
k-means refinement, and L-mode input. Six-sample whole-call measurements do not
demonstrate a reliable latency improvement, and the requested SIMD/GPU
profiles did not prove backend execution for this eager quantizer. Preserve
this allocation reduction, but leave the finding open until a repeatable
latency or throughput gain is measured. See `PERFORMANCE_CAMPAIGN.md` for the
receipts and limits.

P4's I/F source-carrier copies are removed, and all four conversion paths now
beat Pillow on the measured 1024 × 768 CPU whole-call workloads. I→L's focused
parity and repeat benchmark are recorded in `PERFORMANCE_CAMPAIGN.md`. The
conversion family is eager host-side work, so backend-labeled timings are not
SIMD, GPU, or Rayon execution evidence. P4 is closed for the CPU target.

M1 and M2 are closed for the current public surface by the follow-up checks
above. The split fix has mode and byte parity; the typed filtered-resize
variants are unreachable through current public constructors and decoders.
P2 and P5–P10 remain audit candidates without measured follow-up in this
checkpoint. P4's serial CPU target is closed; it has no applicable SIMD, GPU,
or Parallel CPU route.

## Suggested order for follow-up

1. Continue P1 after the RGB Transform, RGB AutoContrast, RGB/L MaxFilter(3),
   L MinFilter(3), and Cover checkpoints by selecting the next uncheckpointed
   operation/mode with measurable RGBA staging cost; preserve each kernel's
   rounding and channel contract. RGB AutoContrast's native GPU transfers are
   implemented, but queued throughput and the SIMD speedup target remain
   open. L and LA Cover serial CPU targets are closed on their material cases;
   LA SIMD remains below its 5× target after one measured
   redundant-premultiplication removal.
2. Continue with P7 color transforms and the host access findings using
   full-call benchmarks and exact parity cases. P3 remains open for evidence
   beyond its allocation reduction.

## LA GaussianBlur transport checkpoint — 2026-10-02

The former packed-L-only `ImageFilter.GaussianBlur` route now admits native LA
for the singleton radius-bounded blur. Two interleaved `[L, A]` pixels travel
per u32 through separate horizontal and vertical GPU shaders. The shaders
derive each pixel's x/y from its flat index, so odd-width rows and the final
partial word preserve both channels. The 1024 × 768 material case passed live
Pillow parity on CPU, SIMD, and GPU (3/3); the GPU recorded six dispatches, no
fallback, no mode conversion, and native upload/readback of 1,572,864 bytes
each, half the former RGBA transfer. A 65 × 47 runtime GPU test additionally
checks odd-width boundaries and exact alpha bytes.

The exact parity-gated benchmark measured GPU median latency at 2.500 ms versus
4.349 ms SIMD and 5.652 ms Pillow. Concurrency was one, so reciprocal latency
does not establish saturated throughput. SIMD improved from 17.955 ms to
4.241 ms in its focused implementation attempt, but remains far below 5×
Pillow. Serial CPU remains slower than Pillow (6.985 ms vs 5.652 ms); after
four bounded candidates, direct scalar radius-one passes were discarded
because they raised CPU latency to 7.727 ms. Keep the native SIMD and GPU
paths, record serial CPU as a blocker, and continue the P1/P9 operation/mode
review. `Image.blend` LA remains covered by
`gpu_native_byte_op_channels`' same-mode native transport. Detailed attempts,
commands, and profile-separated Parallel CPU results are in
`PERFORMANCE_CAMPAIGN.md`.
