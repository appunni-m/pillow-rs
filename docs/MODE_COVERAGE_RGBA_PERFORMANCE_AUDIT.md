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
| P4 | Medium | Scalar conversions | I/F conversion helpers clone their four-byte source carrier before reading it. |
| P5 | Medium | Python data access | getdata slices cross the Python/Rust boundary once per pixel; iteration also slices bytes per pixel. |
| P6 | Medium | Array input | Common fromarray input is copied through Python bytes, Rust Vec, and raster storage. |
| P7 | Low–Medium | Color transforms | HSV and YCbCr transforms clone an already-RGB-shaped input before writing output. |
| P8 | Conditional | CPU Paste | Masked same-mode Paste outside the native allowlist converts source and destination to RGBA. |
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

### P3. Median-cut quantization duplicates native RGB data

pillow-rs/src/ops/quantize.rs:2334–2346 calls to_rgb8(), constructs a Vec<[u8; 3]>, then flattens that into another RGB byte Vec for median_cut_quantize_rgb. The pixel list is only consumed by k-means refinement at :2349–2353, so it is unused when kmeans is zero. On an already RGB8 image, to_rgb8() itself clones the source. Method 1 also owns the to_rgb8() buffer before read-only quantization at :2334–2339.

**Assessment:** high-confidence extra allocation/copy. The median-cut input can use borrowed RGB bytes for native RGB and create the pixel list only when refinement needs it.

### P4. I/F color conversions clone the source carrier

The scalar conversion helpers i_to_rgb, f_to_rgb, i_to_l, f_to_l, i_to_f, and f_to_i call to_rgba8() (pillow-rs/src/color.rs:1214–1298). Public I/F images already use ImageRgba8 as their four-byte scalar carrier. DynamicImage::to_rgba8 clones ImageRgba8 at pillow-rs/src/raster/dynamic.rs:341–353, after which each helper separately allocates its output.

**Assessment:** high-confidence avoidable full-frame source copy. Read the existing carrier bytes directly while keeping the current numeric conversion and byte order.

### P5. Python getdata access does per-pixel work for slices and multiband iteration

For _ImageDataSequence, a slice is expanded through self[i] for every pixel (pillow-rs-py/python/pillow_rs/image.py:85–105); each indexed access calls getpixel_formatted separately. The iterator already obtains all formatted values in one getdata_formatted call at :76–83. A large slice therefore crosses the Python/Rust boundary once per pixel.

For multiband bytes, _CompactImageDataSequence.__iter__ creates a bytes slice for each pixel before constructing the required tuple (:25–40). get_flattened_data already uses struct.iter_unpack on the full buffer (:523–538), demonstrating a bulk path without the intermediate byte slices.

**Assessment:** high confidence in the per-pixel call/allocation pattern; medium priority for large images. Preserve the current live sequence and return-list behavior if consolidating the slice path.

### P6. fromarray copies common byte layouts through three buffers

The Python binding calls memoryview(...).tobytes() and extracts the resulting Python bytes into Vec<u8> (pillow-rs-py/src/lib.rs:2342–2349, 2382–2390). Core from_resolved_array_interface then calls Image::frombytes with borrowed data (pillow-rs/src/ops/array.rs:267–275), whose ordinary frombytes path copies into owned raster storage (pillow-rs/src/image.rs:2059–2072, 2090–2096).

For contiguous byte arrays with an already-supported layout, this produces a Python bytes copy, a Rust Vec copy, and the final owned image copy. Non-contiguous arrays and dtype normalization still need a normalization step, so a borrowed or owned fast path should be limited to compatible layouts.

**Assessment:** high confidence in the copies on this route; medium priority. A buffer-protocol borrow or owned-data constructor could remove intermediate copies for compatible input.

### P7. HSV/YCbCr transforms clone RGB-shaped input

hsv_to_rgb, rgb_to_hsv, rgb_to_ycbcr, and ycbcr_to_rgb call to_rgb8() before writing their output (pillow-rs/src/color.rs:941–945, 991–995, 1075–1078, 1188–1192). Their mode representation is already an RGB8 buffer holding the source channels; DynamicImage::to_rgb8 clones that buffer in the native RGB case (raster/dynamic.rs:321–339).

**Assessment:** high confidence in one avoidable full-frame copy per conversion. The output buffer is required; the source copy is not.

### P8. CPU masked Paste has more same-mode RGBA fallback cases

The native masked-paste allowlist is L, LA, PA, RGB, RGBA, and CMYK (pillow-rs/src/compute/pool_cpu/ops/effects.rs:651–668). Other same-mode combinations that reach the generic path convert both source and destination with to_rgba8() at :966–967, then blend four channels. Candidate 8-bit modes include RGBa, RGBX, HSV, YCbCr, and LAB; indexed and premultiplied modes need separate contract checks before adding a raw-channel kernel. I/F and I;16 should not be treated as byte-channel candidates.

**Assessment:** high confidence that the fallback conversion occurs; low-to-medium confidence that omitted formats are safe for a shared native blend. Current uncommitted changes already add PA to this allowlist in CPU, SIMD, and GPU code.

### P9. Some mode-aware GPU shaders still express work on unused channels

The shaders often compute or load all four packed channels and then select the channels required by the logical mode. For example, the 3x3 and 5x5 convolution shaders form per-channel results before mode selection (`filter_3x3.wgsl:240`, `filter_5x5.wgsl:308`); box and Gaussian blur kernels accumulate R/G/B/A for each sample before selecting (`box_blur_h.wgsl:60`, `box_blur_v.wgsl:58`, `box_blur.wgsl:48`, `gaussian_blur.wgsl:75`); and median/rank filters allocate, fill, and sort four channel arrays (`median_filter.wgsl:98`, `rank_filter.wgsl:101`). Resize convolution constructs four filtered component values (`resize_convolution_h.wgsl:1216`, `resize_convolution_v.wgsl:1258`). L uses only one component and LA uses two logical components, though LA alpha is transported in byte 3. Thumbnail, contain, cover, fit, and rotate shaders have similar packed-channel sampling/interpolation patterns.

Some L-specific packed kernels exist, but admission is narrow: for example, the GPU path has packed-L specializations for limited GaussianBlur/BoxBlur and MedianFilter/RankFilter cases (`pool_gpu/mod.rs:13396–13422`, `:13495+`). The shader source therefore exposes possible work that could be specialized or expressed over only the used channels. A GPU compiler may eliminate some unused calculations; no timing or generated-shader inspection was done, so runtime benefit is unmeasured.

**Assessment:** high confidence that the source expresses extra channel work in these general kernels; medium confidence that it costs runtime on a given adapter. Profile representative L/LA workloads before adding specialization, and preserve each operation's rounding and alpha behavior.

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

## Suggested order for follow-up

1. Measure P1 GPU transport by operation and native mode; specialize only a
   demonstrated bandwidth or staging bottleneck while keeping each kernel's
   current mode contract.
2. Measure P3 median-cut allocation and copy cost, then remove duplicate RGB
   materialization while preserving the k-means input and exact palette parity.
3. Continue with P4 scalar conversions, P7 color transforms, and the host
   access findings using full-call benchmarks and exact parity cases.
